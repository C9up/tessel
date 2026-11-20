//! The message history (story 02-06).
//!
//! Every text and media, received or sent, is written to the group's history partition
//! as a sealed record. Edits and deletions (story 02-05) apply **only when their sender
//! belongs to the account that wrote the targeted message**; otherwise they are ignored
//! and the reason is returned.
//!
//! The check has to answer synchronously, while the message is processed, and the
//! browser's database only answers asynchronously. So the core keeps, in its state, an
//! index of the last [`EDITABLE_MESSAGES`] messages of each group (identifier, author,
//! position): an edit or a deletion targets one of them, or it is ignored. Records are
//! also named after their author: a member who reuses the identifier of someone else's
//! message creates another record, and never overwrites theirs.
//!
//! A deletion erases the message and its edit from the database, and leaves a tombstone
//! ("message deleted") in its place.

use std::collections::{HashMap, VecDeque};
use std::str::FromStr as _;

use crate::codec::{Reader, Writer};
use crate::groups::GroupId;
use crate::identity::{AccountId, DeviceCertificate, DeviceId};
use crate::messages::{Body, MediaRef, Message, MessageId, read_id};
use crate::store::{Changes, PartitionId, Record, Sealer, StoreError};

/// Number of recent messages per group that an edit or a deletion can target.
pub const EDITABLE_MESSAGES: usize = 1024;

const HISTORY: &[u8] = b"history";
const MESSAGE: &[u8] = b"message";
const EDIT: &[u8] = b"edit";
const DELETED: &[u8] = b"deleted";
pub(crate) const RECENT: &[u8] = b"recent";

/// What a received or sent message changed in the history.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    /// A text or a media, added to the history.
    Added,
    /// The targeted message was edited.
    Edited,
    /// The targeted message was deleted.
    Deleted,
    /// Nothing to keep: a receipt, or an unreadable message.
    Nothing,
    /// An edit, a deletion or a message that was not applied.
    Ignored(Ignored),
}

/// Why a message was not applied to the history.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ignored {
    /// The targeted message was written by another account.
    NotAuthor,
    /// The targeted message is not among the recent messages of the group (unknown,
    /// never received, or too old to be changed).
    UnknownTarget,
    /// The targeted message is already deleted.
    AlreadyDeleted,
    /// The account already sent a message with this identifier.
    DuplicateId,
}

/// A message of the history, as the app displays it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryEntry {
    /// Its identifier.
    pub id: MessageId,
    /// The account that wrote it.
    pub author: AccountId,
    /// The device it was sent from.
    pub device: DeviceId,
    /// When it was sent, according to its author's clock (milliseconds).
    pub sent_at: u64,
    /// What it says now.
    pub content: Content,
}

/// The content of a history message, its last edit applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Content {
    /// A text.
    Text {
        /// The text, edited or not.
        text: String,
        /// The message it replies to.
        reply_to: Option<MessageId>,
        /// When it was last edited (author's clock, milliseconds).
        edited_at: Option<u64>,
    },
    /// A media.
    Media {
        /// The media.
        media: MediaRef,
        /// The caption, edited or not.
        caption: String,
        /// The message it replies to.
        reply_to: Option<MessageId>,
        /// When its caption was last edited (author's clock, milliseconds).
        edited_at: Option<u64>,
    },
    /// Deleted by its author.
    Deleted,
}

/// A recent message, in the index the edits and deletions are checked against.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Recent {
    id: MessageId,
    author: AccountId,
    device: DeviceId,
    /// Position in the group's history, in the order the server delivered.
    seq: u64,
    sent_at: u64,
    deleted: bool,
}

/// The history index and the records it still has to hand over.
#[derive(Debug, Default)]
pub(crate) struct History {
    recent: HashMap<GroupId, VecDeque<Recent>>,
    pending: Changes,
}

/// The name of a message's records: its author, then its identifier.
fn message_name(author: &AccountId, id: &MessageId) -> Result<Vec<u8>, StoreError> {
    let mut out = Writer::default();
    out.vec16(author.to_string().as_bytes())?;
    out.bytes(id.as_bytes());
    Ok(out.0)
}

fn recent_name(group: &GroupId, author: &AccountId, id: &MessageId) -> Result<Vec<u8>, StoreError> {
    let mut out = Writer::default();
    out.vec16(group.as_bytes())?;
    out.bytes(&message_name(author, id)?);
    Ok(out.0)
}

fn read_account(input: &mut Reader<'_>) -> Result<AccountId, StoreError> {
    let account = std::str::from_utf8(input.vec16()?).map_err(|_| StoreError::Malformed)?;
    AccountId::from_str(account).map_err(|_| StoreError::Malformed)
}

fn read_device(input: &mut Reader<'_>) -> Result<DeviceId, StoreError> {
    Ok(DeviceId::from_bytes(input.array()?))
}

/// The partition of a group's history.
pub(crate) fn partition(sealer: &Sealer, group: &GroupId) -> Result<PartitionId, StoreError> {
    sealer.partition(HISTORY, group.as_bytes())
}

impl History {
    /// Why a message from `author` would be ignored, if it would.
    pub(crate) fn verdict(
        &self,
        group: &GroupId,
        author: &AccountId,
        message: &Message,
    ) -> Option<Ignored> {
        let recent = self.recent.get(group);
        let find = |id: &MessageId| {
            recent.and_then(|recent| recent.iter().find(|r| r.id == *id && &r.author == author))
        };
        match &message.body {
            Body::Receipt { .. } => None,
            Body::Text { .. } | Body::Media { .. } => {
                find(&message.id).map(|_| Ignored::DuplicateId)
            }
            Body::Edit { target, .. } | Body::Delete { target } => match find(target) {
                Some(entry) if entry.deleted => Some(Ignored::AlreadyDeleted),
                Some(_) => None,
                None if recent.is_some_and(|recent| recent.iter().any(|r| r.id == *target)) => {
                    Some(Ignored::NotAuthor)
                }
                None => Some(Ignored::UnknownTarget),
            },
        }
    }

    /// Records a message received from `sender` (or sent by this device) in the group.
    pub(crate) fn apply(
        &mut self,
        sealer: &Sealer,
        state: PartitionId,
        group: &GroupId,
        sender: &DeviceCertificate,
        message: &Message,
    ) -> Result<Effect, StoreError> {
        let author = sender.account();
        if let Some(reason) = self.verdict(group, author, message) {
            return Ok(Effect::Ignored(reason));
        }
        let history = partition(sealer, group)?;
        let mut changes = Changes::default();
        let recent = self.recent.entry(group.clone()).or_default();

        // Everything is sealed first; the index only changes once nothing can fail.
        let effect = match &message.body {
            Body::Receipt { .. } => Effect::Nothing,
            Body::Text { .. } | Body::Media { .. } => {
                let entry = Recent {
                    id: message.id,
                    author: author.clone(),
                    device: sender.device_id(),
                    seq: recent.back().map_or(0, |r| r.seq + 1),
                    sent_at: message.sent_at,
                    deleted: false,
                };
                let mut body = Writer::default();
                body.u64(entry.seq);
                body.vec16(&sender.to_bytes().map_err(|_| StoreError::Malformed)?)?;
                body.vec32(&message.encode().map_err(|_| StoreError::Malformed)?)?;
                let name = message_name(author, &message.id)?;
                changes
                    .put
                    .push(sealer.seal(history, MESSAGE, &name, &body.0)?);
                changes.put.push(seal_recent(sealer, state, group, &entry)?);
                let evicted = match recent.front() {
                    Some(old) if recent.len() >= EDITABLE_MESSAGES => {
                        let name = recent_name(group, &old.author, &old.id)?;
                        changes
                            .delete
                            .push((state, sealer.record_id(&state, RECENT, &name)?));
                        true
                    }
                    _ => false,
                };
                if evicted {
                    recent.pop_front();
                }
                recent.push_back(entry);
                Effect::Added
            }
            Body::Edit { target, .. } | Body::Delete { target } => {
                let entry = recent
                    .iter_mut()
                    .find(|r| r.id == *target && &r.author == author)
                    .ok_or(StoreError::Malformed)?;
                let name = message_name(author, target)?;
                if matches!(message.body, Body::Edit { .. }) {
                    let mut body = Writer::default();
                    body.vec16(author.to_string().as_bytes())?;
                    body.vec32(&message.encode().map_err(|_| StoreError::Malformed)?)?;
                    changes
                        .put
                        .push(sealer.seal(history, EDIT, &name, &body.0)?);
                    Effect::Edited
                } else {
                    for label in [MESSAGE, EDIT] {
                        changes
                            .delete
                            .push((history, sealer.record_id(&history, label, &name)?));
                    }
                    let mut body = Writer::default();
                    body.bytes(entry.id.as_bytes());
                    body.vec16(entry.author.to_string().as_bytes())?;
                    body.bytes(entry.device.as_bytes());
                    body.u64(entry.seq);
                    body.u64(entry.sent_at);
                    changes
                        .put
                        .push(sealer.seal(history, DELETED, &name, &body.0)?);
                    let deleted = Recent {
                        deleted: true,
                        ..entry.clone()
                    };
                    changes
                        .put
                        .push(seal_recent(sealer, state, group, &deleted)?);
                    *entry = deleted;
                    Effect::Deleted
                }
            }
        };
        self.pending.append(changes);
        Ok(effect)
    }

    /// The device left the group: its index goes. Its history stays readable.
    pub(crate) fn forget(
        &mut self,
        sealer: &Sealer,
        state: PartitionId,
        group: &GroupId,
    ) -> Result<(), StoreError> {
        for entry in self.recent.remove(group).unwrap_or_default() {
            let name = recent_name(group, &entry.author, &entry.id)?;
            self.pending
                .delete
                .push((state, sealer.record_id(&state, RECENT, &name)?));
        }
        Ok(())
    }

    /// The records waiting to be persisted.
    pub(crate) fn take(&mut self) -> Changes {
        std::mem::take(&mut self.pending)
    }

    /// Reloads an index entry from the state, at startup.
    pub(crate) fn load(&mut self, body: &[u8]) -> Result<(), StoreError> {
        let mut input = Reader(body);
        let group = GroupId::from_bytes(input.vec16()?);
        let entry = Recent {
            id: read_id(&mut input)?,
            author: read_account(&mut input)?,
            device: read_device(&mut input)?,
            seq: input.u64()?,
            sent_at: input.u64()?,
            deleted: input.bool()?,
        };
        input.finish()?;
        self.recent.entry(group).or_default().push_back(entry);
        Ok(())
    }

    /// Puts each group's index back in history order, once everything is loaded.
    pub(crate) fn sort(&mut self) {
        for recent in self.recent.values_mut() {
            recent.make_contiguous().sort_by_key(|r| r.seq);
        }
    }

    #[cfg(test)]
    pub(crate) fn recent_len(&self, group: &GroupId) -> usize {
        self.recent.get(group).map_or(0, VecDeque::len)
    }
}

fn seal_recent(
    sealer: &Sealer,
    state: PartitionId,
    group: &GroupId,
    entry: &Recent,
) -> Result<Record, StoreError> {
    let mut body = Writer::default();
    body.vec16(group.as_bytes())?;
    body.bytes(entry.id.as_bytes());
    body.vec16(entry.author.to_string().as_bytes())?;
    body.bytes(entry.device.as_bytes());
    body.u64(entry.seq);
    body.u64(entry.sent_at);
    body.bool(entry.deleted);
    let name = recent_name(group, &entry.author, &entry.id)?;
    sealer.seal(state, RECENT, &name, &body.0)
}

/// A message record, opened.
struct Stored {
    seq: u64,
    sender: DeviceCertificate,
    message: Message,
}

/// Opens a group's history records and returns its messages in history order, their
/// last edit applied.
pub(crate) fn read(
    sealer: &Sealer,
    partition: PartitionId,
    records: &[Record],
) -> Result<Vec<HistoryEntry>, StoreError> {
    let mut messages: HashMap<(AccountId, MessageId), Stored> = HashMap::new();
    let mut edits: HashMap<(AccountId, MessageId), Message> = HashMap::new();
    let mut entries: Vec<(u64, HistoryEntry)> = Vec::new();

    for record in records {
        if record.partition != partition {
            return Err(StoreError::WrongPartition);
        }
        let opened = sealer.open(record)?;
        let mut input = Reader(&opened.body);
        match opened.label.as_slice() {
            MESSAGE => {
                let seq = input.u64()?;
                let sender = DeviceCertificate::from_bytes(input.vec16()?)
                    .map_err(|_| StoreError::Malformed)?;
                let message = Message::decode(input.vec32()?).map_err(|_| StoreError::Malformed)?;
                input.finish()?;
                let key = (sender.account().clone(), message.id);
                messages.insert(
                    key,
                    Stored {
                        seq,
                        sender,
                        message,
                    },
                );
            }
            EDIT => {
                let author = read_account(&mut input)?;
                let edit = Message::decode(input.vec32()?).map_err(|_| StoreError::Malformed)?;
                input.finish()?;
                let Body::Edit { target, .. } = &edit.body else {
                    return Err(StoreError::Malformed);
                };
                edits.insert((author, *target), edit);
            }
            DELETED => {
                let id = read_id(&mut input)?;
                let author = read_account(&mut input)?;
                let device = read_device(&mut input)?;
                let seq = input.u64()?;
                let sent_at = input.u64()?;
                input.finish()?;
                entries.push((
                    seq,
                    HistoryEntry {
                        id,
                        author,
                        device,
                        sent_at,
                        content: Content::Deleted,
                    },
                ));
            }
            _ => return Err(StoreError::Malformed),
        }
    }
    // An edit only ever pairs with a message of its own author: the index checked it
    // before writing, and the key checks it again here.
    entries.extend(messages.into_iter().map(|(key, stored)| {
        let edit = edits.get(&key);
        (stored.seq, entry(stored, edit))
    }));
    entries.sort_by_key(|(seq, _)| *seq);
    Ok(entries.into_iter().map(|(_, entry)| entry).collect())
}

fn entry(stored: Stored, edit: Option<&Message>) -> HistoryEntry {
    let edited = edit.and_then(|edit| match &edit.body {
        Body::Edit { text, .. } => Some((text.clone(), edit.sent_at)),
        _ => None,
    });
    let edited_at = edited.as_ref().map(|(_, at)| *at);
    let content = match stored.message.body {
        Body::Text { text, reply_to } => Content::Text {
            text: edited.map_or(text, |(text, _)| text),
            reply_to,
            edited_at,
        },
        Body::Media {
            media,
            caption,
            reply_to,
        } => Content::Media {
            media,
            caption: edited.map_or(caption, |(text, _)| text),
            reply_to,
            edited_at,
        },
        // Only texts and media are written as messages.
        _ => Content::Deleted,
    };
    HistoryEntry {
        id: stored.message.id,
        author: stored.sender.account().clone(),
        device: stored.sender.device_id(),
        sent_at: stored.message.sent_at,
        content,
    }
}

#[cfg(test)]
mod tests;
