//! The conversation list (story 05-01): each group the device is in, its last message's
//! time, and how many messages it has not read. What was read is the device's own
//! business: the read markers are sealed in its database, never sent anywhere (a read
//! receipt, story 05-04, is a message the user chooses to send).

use std::collections::HashMap;

use crate::client::Client;
use crate::codec::{Reader, Writer};
use crate::groups::GroupId;
use crate::store::{Changes, PartitionId, Sealer, StoreError};

/// Label of a group's read marker, in the state partition.
pub(crate) const READ: &[u8] = b"read";

/// A group in the conversation list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conversation {
    pub group: GroupId,
    /// The group's name (story 05-02); empty for none.
    pub name: String,
    /// When its last message was sent (the sender's clock, in milliseconds); `None`
    /// before any message.
    pub last_sent_at: Option<u64>,
    /// Messages of other accounts since this device last read the group, within the
    /// recent window ([`crate::EDITABLE_MESSAGES`]).
    pub unread: usize,
}

/// The read markers: per group, the place of the first message not read yet (0: none
/// read).
#[derive(Debug, Default)]
pub(crate) struct ReadMarks {
    marks: HashMap<GroupId, u64>,
    pending: Changes,
}

impl ReadMarks {
    fn get(&self, group: &GroupId) -> u64 {
        self.marks.get(group).copied().unwrap_or(0)
    }

    fn put(
        &mut self,
        sealer: &Sealer,
        state: PartitionId,
        group: &GroupId,
        seq: u64,
    ) -> Result<(), StoreError> {
        let mut body = Writer::default();
        body.vec16(group.as_bytes())?;
        body.u64(seq);
        self.pending
            .put
            .push(sealer.seal(state, READ, group.as_bytes(), &body.0)?);
        self.marks.insert(group.clone(), seq);
        Ok(())
    }

    /// The records waiting to be persisted.
    pub(crate) fn take(&mut self) -> Changes {
        std::mem::take(&mut self.pending)
    }

    /// Reloads a read marker from the state, at startup.
    pub(crate) fn load(&mut self, body: &[u8]) -> Result<(), StoreError> {
        let mut input = Reader(body);
        let group = GroupId::from_bytes(input.vec16()?);
        let seq = input.u64()?;
        input.finish()?;
        self.marks.insert(group, seq);
        Ok(())
    }
}

impl Client {
    /// The conversation list: every group, the most recently active first.
    pub fn conversations(&self) -> Vec<Conversation> {
        let own = self.device.certificate().account();
        let mut list: Vec<Conversation> = self
            .groups()
            .into_iter()
            .map(|group| {
                let summary = self
                    .history
                    .summary(&group, own, self.read_marks.get(&group));
                Conversation {
                    name: self
                        .group_settings(&group)
                        .map(|settings| settings.name)
                        .unwrap_or_default(),
                    group,
                    last_sent_at: summary.last_sent_at,
                    unread: summary.unread,
                }
            })
            .collect();
        // Most recent first; groups without a message last, in a stable order.
        list.sort_by_key(|conversation| std::cmp::Reverse(conversation.last_sent_at));
        list
    }

    /// The user read the group up to its last message.
    pub fn mark_read(&mut self, group: &GroupId) -> Result<(), StoreError> {
        let own = self.device.certificate().account().clone();
        let summary = self.history.summary(group, &own, 0);
        if summary.last_sent_at.is_none() {
            return Ok(());
        }
        let unread_from = summary.last_seq + 1;
        if unread_from <= self.read_marks.get(group) {
            return Ok(());
        }
        let (marks, sealer, state) = self.read_marks_mut();
        marks.put(sealer, state, group, unread_from)
    }
}

#[cfg(test)]
mod tests;
