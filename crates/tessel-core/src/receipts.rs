//! Delivery and read receipts (story 05-04).
//!
//! A receipt is a message like any other, encrypted for the group: the server only sees
//! one more envelope. Each device owes one to the group for what it received (delivered)
//! and, when the user reads, for what was read. They are not sent one per message: the
//! core keeps what it owes, sealed in its state, and [`Client::queue_receipts`] turns it
//! into a few receipts of up to [`MAX_RECEIPT`] messages each, which go through the send
//! queue. The messenger calls it once a batch of envelopes is handled.
//!
//! On this account's messages, the receipts of the other accounts are recorded in the
//! group's history: a message is delivered once every other account of the group got it
//! on one device, read once every one read it. A read receipt from another device of
//! this account moves this device's read marker: what is read there is read here.
//!
//! Read receipts are reciprocal, as in WhatsApp: with them off
//! ([`Client::set_read_receipts`]), this device sends none, and records none of the
//! others'. Delivery receipts always go.

use std::collections::{BTreeMap, HashSet};

use crate::client::Client;
use crate::codec::{Reader, Writer};
use crate::groups::{GroupError, GroupId};
use crate::history::{EDITABLE_MESSAGES, Effect, HistoryEntry, Status};
use crate::identity::AccountId;
use crate::messages::{Body, MAX_RECEIPT, Message, MessageId, ReceiptKind, read_id};
use crate::store::{Changes, PartitionId, Sealer, StoreError};

/// Label of a group's owed receipts, in the state partition.
pub(crate) const OWED: &[u8] = b"owed-receipts";

/// Label of the read receipts setting, in the state partition.
pub(crate) const READ_RECEIPTS: &[u8] = b"read-receipts";

/// What a device owes a group.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct Owed {
    delivered: Vec<MessageId>,
    read: Vec<MessageId>,
}

impl Owed {
    fn is_empty(&self) -> bool {
        self.delivered.is_empty() && self.read.is_empty()
    }

    fn list(&mut self, kind: ReceiptKind) -> &mut Vec<MessageId> {
        match kind {
            ReceiptKind::Delivered => &mut self.delivered,
            ReceiptKind::Read => &mut self.read,
        }
    }
}

/// The receipts owed, the read receipts setting, and the records still to hand over.
#[derive(Debug, Default)]
pub(crate) struct Receipts {
    /// Ordered, so that receipts leave in the same order on every run.
    owed: BTreeMap<Vec<u8>, (GroupId, Owed)>,
    /// Read receipts off (on by default).
    reads_off: bool,
    pending: Changes,
}

fn write_ids(out: &mut Writer, ids: &[MessageId]) -> Result<(), StoreError> {
    out.u16(u16::try_from(ids.len()).map_err(|_| StoreError::Malformed)?);
    for id in ids {
        out.bytes(id.as_bytes());
    }
    Ok(())
}

fn read_ids(input: &mut Reader<'_>) -> Result<Vec<MessageId>, StoreError> {
    (0..input.u16()?)
        .map(|_| read_id(input).map_err(StoreError::from))
        .collect()
}

impl Receipts {
    /// Owes a receipt for `ids` to the group. Past [`EDITABLE_MESSAGES`] of a kind, the
    /// oldest go: no edit or receipt reaches them anyway.
    fn owe(
        &mut self,
        sealer: &Sealer,
        state: PartitionId,
        group: &GroupId,
        kind: ReceiptKind,
        ids: &[MessageId],
    ) -> Result<(), StoreError> {
        if ids.is_empty() || (kind == ReceiptKind::Read && self.reads_off) {
            return Ok(());
        }
        let (_, owed) = self
            .owed
            .entry(group.as_bytes().to_vec())
            .or_insert_with(|| (group.clone(), Owed::default()));
        let list = owed.list(kind);
        for id in ids {
            if !list.contains(id) {
                list.push(*id);
            }
        }
        let excess = list.len().saturating_sub(EDITABLE_MESSAGES);
        list.drain(..excess);
        let owed = owed.clone();
        self.seal(sealer, state, group, &owed)
    }

    fn seal(
        &mut self,
        sealer: &Sealer,
        state: PartitionId,
        group: &GroupId,
        owed: &Owed,
    ) -> Result<(), StoreError> {
        if owed.is_empty() {
            let id = sealer.record_id(&state, OWED, group.as_bytes())?;
            self.pending.delete.push((state, id));
            return Ok(());
        }
        let mut body = Writer::default();
        body.vec16(group.as_bytes())?;
        write_ids(&mut body, &owed.delivered)?;
        write_ids(&mut body, &owed.read)?;
        self.pending
            .put
            .push(sealer.seal(state, OWED, group.as_bytes(), &body.0)?);
        Ok(())
    }

    /// Takes what is owed, every group, and forgets it.
    fn take_owed(
        &mut self,
        sealer: &Sealer,
        state: PartitionId,
    ) -> Result<Vec<(GroupId, Owed)>, StoreError> {
        let owed: Vec<(GroupId, Owed)> = std::mem::take(&mut self.owed).into_values().collect();
        for (group, _) in &owed {
            self.seal(sealer, state, group, &Owed::default())?;
        }
        Ok(owed)
    }

    /// The device left the group: it owes it nothing more.
    pub(crate) fn forget(
        &mut self,
        sealer: &Sealer,
        state: PartitionId,
        group: &GroupId,
    ) -> Result<(), StoreError> {
        if self.owed.remove(group.as_bytes()).is_some() {
            self.seal(sealer, state, group, &Owed::default())?;
        }
        Ok(())
    }

    fn set_reads(
        &mut self,
        sealer: &Sealer,
        state: PartitionId,
        on: bool,
    ) -> Result<(), StoreError> {
        self.reads_off = !on;
        if !on {
            // What was owed is not sent either.
            let groups: Vec<GroupId> = self.owed.values().map(|(group, _)| group.clone()).collect();
            for group in groups {
                let Some((_, owed)) = self.owed.get_mut(group.as_bytes()) else {
                    continue;
                };
                owed.read.clear();
                let owed = owed.clone();
                if owed.is_empty() {
                    self.owed.remove(group.as_bytes());
                }
                self.seal(sealer, state, &group, &owed)?;
            }
        }
        let mut body = Writer::default();
        body.bool(on);
        self.pending
            .put
            .push(sealer.seal(state, READ_RECEIPTS, b"", &body.0)?);
        Ok(())
    }

    /// The records waiting to be persisted.
    pub(crate) fn take(&mut self) -> Changes {
        std::mem::take(&mut self.pending)
    }

    /// Reloads what a group is owed from the state, at startup.
    pub(crate) fn load_owed(&mut self, body: &[u8]) -> Result<(), StoreError> {
        let mut input = Reader(body);
        let group = GroupId::from_bytes(input.vec16()?);
        let owed = Owed {
            delivered: read_ids(&mut input)?,
            read: read_ids(&mut input)?,
        };
        input.finish()?;
        self.owed.insert(group.as_bytes().to_vec(), (group, owed));
        Ok(())
    }

    /// Reloads the read receipts setting from the state, at startup.
    pub(crate) fn load_setting(&mut self, body: &[u8]) -> Result<(), StoreError> {
        let mut input = Reader(body);
        self.reads_off = !input.bool()?;
        input.finish()?;
        Ok(())
    }
}

impl Client {
    /// Whether this device sends read receipts, and records the others' (on by default).
    pub fn read_receipts(&self) -> bool {
        !self.receipts.reads_off
    }

    /// Turns read receipts on or off. Off, the read receipts owed are dropped.
    pub fn set_read_receipts(&mut self, on: bool) -> Result<(), StoreError> {
        let (receipts, sealer, state) = self.receipts_mut();
        receipts.set_reads(sealer, state, on)
    }

    /// Puts what this device owes in the send queue of each group, as receipts of up to
    /// [`MAX_RECEIPT`] messages sent at `sent_at`; the groups whose queue got some, to
    /// send. A group the device left is owed nothing.
    pub fn queue_receipts(&mut self, sent_at: u64) -> Result<Vec<GroupId>, GroupError> {
        let (receipts, sealer, state) = self.receipts_mut();
        let owed = receipts.take_owed(sealer, state)?;
        let mut queued = Vec::new();
        for (group, owed) in owed {
            if !self.groups.contains_key(&group) {
                continue;
            }
            for (kind, ids) in [
                (ReceiptKind::Delivered, owed.delivered),
                (ReceiptKind::Read, owed.read),
            ] {
                for chunk in ids.chunks(MAX_RECEIPT) {
                    let body = Body::Receipt {
                        kind,
                        messages: chunk.to_vec(),
                    };
                    self.enqueue(&group, &Message::new(sent_at, body)?)?;
                }
            }
            queued.push(group);
        }
        Ok(queued)
    }

    /// Owes the group a receipt for `ids`.
    pub(crate) fn owe_receipt(
        &mut self,
        group: &GroupId,
        kind: ReceiptKind,
        ids: &[MessageId],
    ) -> Result<(), StoreError> {
        let (receipts, sealer, state) = self.receipts_mut();
        receipts.owe(sealer, state, group, kind, ids)
    }

    /// A receipt `from` an account, in the group: recorded on this account's messages,
    /// or, from another device of this account, a read that moves the read marker.
    pub(crate) fn receive_receipt(
        &mut self,
        group: &GroupId,
        from: &AccountId,
        kind: ReceiptKind,
        ids: &[MessageId],
    ) -> Result<Effect, StoreError> {
        let own = self.device.certificate().account().clone();
        if *from == own {
            if kind == ReceiptKind::Read
                && let Some(up_to) = self.history.read_up_to(group, &own, ids)
            {
                let (marks, sealer, state) = self.read_marks_mut();
                if up_to > marks.get(group) {
                    marks.put(sealer, state, group, up_to)?;
                }
            }
            return Ok(Effect::Nothing);
        }
        if kind == ReceiptKind::Read && self.receipts.reads_off {
            return Ok(Effect::Nothing);
        }
        let (history, sealer) = self.history_mut();
        Ok(
            if history.acknowledge(sealer, group, &own, from, kind, ids)? {
                Effect::Receipt
            } else {
                Effect::Nothing
            },
        )
    }

    /// Where each of this account's messages stands: pending while queued, then sent,
    /// delivered or read by every other account still in the group.
    pub(crate) fn set_statuses(&self, group: &GroupId, entries: &mut [HistoryEntry]) {
        let own = self.device.certificate().account();
        let waiting = self.waiting_messages(group);
        let others: HashSet<AccountId> = self
            .members(group)
            .unwrap_or_default()
            .iter()
            .map(|member| member.account().clone())
            .filter(|account| account != own)
            .collect();
        for entry in entries.iter_mut().filter(|entry| &entry.author == own) {
            let reached = |kind: ReceiptKind| {
                !others.is_empty()
                    && others.iter().all(|account| {
                        entry.receipts.iter().any(|receipt| {
                            &receipt.account == account
                                && (receipt.kind == kind || receipt.kind == ReceiptKind::Read)
                        })
                    })
            };
            entry.status = Some(if waiting.contains(&entry.id) {
                Status::Pending
            } else if reached(ReceiptKind::Read) {
                Status::Read
            } else if reached(ReceiptKind::Delivered) {
                Status::Delivered
            } else {
                Status::Sent
            });
        }
    }
}

#[cfg(test)]
mod tests;
