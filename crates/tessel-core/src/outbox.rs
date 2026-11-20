//! The send queue (story 02-07).
//!
//! Every message goes through it, online or not: [`crate::Client::queue_message`] puts
//! it in the history and in the queue, both persisted; the host then sends each
//! group's queue in order ([`crate::Client::next_outbound`]) and removes a message
//! once the server has acknowledged it ([`crate::Client::outbound_sent`]). A message
//! written offline therefore leaves by itself when the connection comes back.
//!
//! - **Encrypted at send time, not when queued.** A message waits for an unknown time:
//!   meanwhile, members may join or leave, and MLS forbids sending while proposals are
//!   pending. Encrypting it at the last moment, in the group's current epoch, keeps a
//!   removed member out and never sends a message in an epoch the members no longer
//!   read. The queue keeps the plaintext, sealed in the local database like the history.
//! - **Once.** Each queued message carries a send identifier, random, which the server
//!   uses to ignore a message it already has (story 04-01): a retry after a lost
//!   acknowledgement is encrypted again, but stored once. Receivers also ignore a
//!   message identifier their sender already used ([`crate::Ignored::DuplicateId`]).
//! - **In order, per group.** Only the head of a group's queue is handed out; the next
//!   one follows once the head is acknowledged. Groups are independent.

use std::collections::{HashMap, VecDeque};
use std::fmt;

use openmls_rust_crypto::RustCrypto;
use openmls_traits::random::OpenMlsRand as _;
use zeroize::Zeroizing;

use crate::codec::{Reader, Writer};
use crate::groups::GroupId;
use crate::messages::{Message, MessageId, read_id};
use crate::store::{Changes, PartitionId, Sealer, StoreError};

/// Label of a queued message, in the state partition.
pub(crate) const OUTBOX: &[u8] = b"outbox";

/// The identifier the server uses to ignore a message it already has. Random, and
/// distinct from the message identifier, which only the members see.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct SendId([u8; 16]);

impl SendId {
    fn random() -> Result<Self, StoreError> {
        RustCrypto::default()
            .random_array()
            .map(Self)
            .map_err(|_| StoreError::NoRandomness)
    }

    /// Wraps a known identifier.
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// The identifier's 16 bytes.
    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

impl fmt::Debug for SendId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SendId(")?;
        self.0.iter().try_for_each(|b| write!(f, "{b:02x}"))?;
        write!(f, ")")
    }
}

/// The head of a group's queue, encrypted for the group's current epoch: what the host
/// sends to the server.
#[derive(Debug, PartialEq, Eq)]
pub struct Outbound {
    /// The group.
    pub group: GroupId,
    /// What the server deduplicates on.
    pub send_id: SendId,
    /// The message it carries, for the app to match the acknowledgement.
    pub message_id: MessageId,
    /// The encrypted message (`MLSMessage`).
    pub encrypted: Vec<u8>,
}

/// A message waiting to be sent.
#[derive(Debug)]
pub(crate) struct Queued {
    /// Position across all queues: the order messages were written in.
    seq: u64,
    pub(crate) send_id: SendId,
    pub(crate) message_id: MessageId,
    /// The encoded message, padding included.
    pub(crate) encoded: Zeroizing<Vec<u8>>,
}

/// The queues of every group and the records they still have to hand over.
#[derive(Debug, Default)]
pub(crate) struct Outbox {
    queues: HashMap<GroupId, VecDeque<Queued>>,
    next_seq: u64,
    pending: Changes,
}

impl Outbox {
    /// Queues a message at the end of the group's queue.
    pub(crate) fn push(
        &mut self,
        sealer: &Sealer,
        state: PartitionId,
        group: &GroupId,
        message: &Message,
    ) -> Result<SendId, StoreError> {
        let queued = Queued {
            seq: self.next_seq,
            send_id: SendId::random()?,
            message_id: message.id,
            encoded: Zeroizing::new(message.encode().map_err(|_| StoreError::Malformed)?),
        };
        let mut body = Writer::default();
        body.vec16(group.as_bytes())?;
        body.u64(queued.seq);
        body.bytes(queued.send_id.as_bytes());
        body.bytes(queued.message_id.as_bytes());
        body.vec32(&queued.encoded)?;
        let body = Zeroizing::new(body.0);
        let record = sealer.seal(state, OUTBOX, queued.send_id.as_bytes(), &body)?;

        self.pending.put.push(record);
        self.next_seq += 1;
        let send_id = queued.send_id;
        self.queues
            .entry(group.clone())
            .or_default()
            .push_back(queued);
        Ok(send_id)
    }

    /// The head of the group's queue.
    pub(crate) fn head(&self, group: &GroupId) -> Option<&Queued> {
        self.queues.get(group).and_then(VecDeque::front)
    }

    /// Removes an acknowledged message. Acknowledging twice, or a message no longer
    /// queued, changes nothing: acknowledgements can arrive again after a retry.
    pub(crate) fn remove(
        &mut self,
        sealer: &Sealer,
        state: PartitionId,
        group: &GroupId,
        send_id: &SendId,
    ) -> Result<(), StoreError> {
        let Some(queue) = self.queues.get_mut(group) else {
            return Ok(());
        };
        let Some(position) = queue.iter().position(|q| q.send_id == *send_id) else {
            return Ok(());
        };
        let id = sealer.record_id(&state, OUTBOX, send_id.as_bytes())?;
        queue.remove(position);
        if queue.is_empty() {
            self.queues.remove(group);
        }
        self.pending.delete.push((state, id));
        Ok(())
    }

    /// The device left the group: its queue goes, and the messages that will never
    /// leave are returned.
    pub(crate) fn forget(
        &mut self,
        sealer: &Sealer,
        state: PartitionId,
        group: &GroupId,
    ) -> Result<Vec<MessageId>, StoreError> {
        let Some(queue) = self.queues.get(group) else {
            return Ok(Vec::new());
        };
        let ids = queue
            .iter()
            .map(|q| sealer.record_id(&state, OUTBOX, q.send_id.as_bytes()))
            .collect::<Result<Vec<_>, _>>()?;
        let unsent = self
            .queues
            .remove(group)
            .unwrap_or_default()
            .into_iter()
            .map(|q| q.message_id)
            .collect();
        self.pending
            .delete
            .extend(ids.into_iter().map(|id| (state, id)));
        Ok(unsent)
    }

    /// The groups with messages to send.
    pub(crate) fn groups(&self) -> impl Iterator<Item = &GroupId> {
        self.queues.keys()
    }

    /// The messages of the group still waiting, in order.
    pub(crate) fn waiting(&self, group: &GroupId) -> Vec<MessageId> {
        self.queues
            .get(group)
            .map(|queue| queue.iter().map(|q| q.message_id).collect())
            .unwrap_or_default()
    }

    /// The records waiting to be persisted.
    pub(crate) fn take(&mut self) -> Changes {
        std::mem::take(&mut self.pending)
    }

    /// Reloads a queued message from the state, at startup.
    pub(crate) fn load(&mut self, body: &[u8]) -> Result<(), StoreError> {
        let mut input = Reader(body);
        let group = GroupId::from_bytes(input.vec16()?);
        let queued = Queued {
            seq: input.u64()?,
            send_id: SendId(input.array()?),
            message_id: read_id(&mut input)?,
            encoded: Zeroizing::new(input.vec32()?.to_vec()),
        };
        input.finish()?;
        self.next_seq = self.next_seq.max(queued.seq + 1);
        self.queues.entry(group).or_default().push_back(queued);
        Ok(())
    }

    /// Puts each queue back in the order its messages were written, once everything is
    /// loaded.
    pub(crate) fn sort(&mut self) {
        for queue in self.queues.values_mut() {
            queue.make_contiguous().sort_by_key(|q| q.seq);
        }
    }
}

#[cfg(test)]
mod tests;
