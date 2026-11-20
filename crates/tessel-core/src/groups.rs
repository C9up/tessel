//! MLS groups (story 02-04).
//!
//! A group brings together devices, not accounts: adding someone means adding each of
//! their devices; an account has several. Every leaf that joins or changes must carry
//! a valid device certificate (story 02-02), otherwise the commit or the *Welcome* is
//! rejected.
//!
//! The server orders commits (story 04-02): only one wins per epoch. A commit is
//! prepared ([`Client::add_devices`], [`Client::remove_devices`]…), sent to the
//! server, then the client learns whether it was accepted ([`Client::commit_accepted`])
//! or rejected ([`Client::commit_rejected`]). When rejected, the client processes the
//! commits that beat it, then [`Client::retry_commit`] replays the same intent on the
//! up-to-date state.
//!
//! Everything is sent encrypted (`PrivateMessage`), commits included: the server only
//! sees the group identifier, the epoch and the message type.

use std::fmt;

use openmls::prelude::{
    KeyPackage, LeafNodeIndex, Member, MlsGroup, MlsGroupCreateConfig, MlsGroupJoinConfig,
    MlsMessageBodyIn, MlsMessageIn, MlsMessageOut, PURE_CIPHERTEXT_WIRE_FORMAT_POLICY,
    ProcessedMessageContent, Sender, StagedCommit, StagedWelcome,
};
use openmls_rust_crypto::OpenMlsRustCrypto;
use openmls_traits::OpenMlsProvider;
use tls_codec::{Deserialize as _, Serialize as _};

use crate::client::Client;
use crate::codec::{CodecError, Reader, Writer};
use crate::history::{Effect, Ignored};
use crate::identity::{AccountId, Device, DeviceCertificate, DeviceId, IdentityError};
use crate::key_packages::{VerifiedKeyPackage, capabilities, verify_key_package};
use crate::messages::{Message, MessageError, MessageId};
use crate::outbox::{Outbound, SendId};
use crate::store::StoreError;
use crate::suite::CIPHERSUITE;

/// Number of past epochs whose keys are kept, to read a message encrypted just before a
/// commit and arriving just after it. Beyond that, the secret is erased (forward
/// secrecy).
const MAX_PAST_EPOCHS: usize = 3;

/// What groups can reject.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum GroupError {
    /// This group does not exist on this device (or the device was removed from it).
    #[error("unknown group")]
    UnknownGroup,
    /// The message is not an MLS message expected here.
    #[error("malformed or unexpected MLS message")]
    Malformed,
    /// The *Welcome* or the group is not in Tessel's ciphersuite (ADR-001).
    #[error("group uses another ciphersuite")]
    WrongCiphersuite,
    /// A commit from this device is already awaiting the server's answer.
    #[error("a commit is already pending")]
    PendingCommit,
    /// No pending commit.
    #[error("no pending commit")]
    NoPendingCommit,
    /// Proposals await their commit (a member leaving): MLS forbids sending a message
    /// until they are committed.
    #[error("proposals await their commit")]
    PendingProposals,
    /// A message from this very device, sent back by the server.
    #[error("own message")]
    OwnMessage,
    /// The operation changes nothing in the group.
    #[error("nothing to commit")]
    NothingToCommit,
    /// OpenMLS rejected the operation or the message.
    #[error("MLS: {0}")]
    Mls(String),
    /// A leaf does not carry a valid device certificate.
    #[error(transparent)]
    Identity(#[from] IdentityError),
    /// The message to send violates the format (story 02-05).
    #[error(transparent)]
    Message(#[from] MessageError),
    /// The edit or deletion to send would be ignored by every member (story 02-06):
    /// it is not sent.
    #[error("message would be ignored: {0:?}")]
    Ignored(Ignored),
    /// The local database failed (story 02-06).
    #[error(transparent)]
    Store(#[from] StoreError),
}

fn mls(error: impl fmt::Display) -> GroupError {
    GroupError::Mls(error.to_string())
}

/// A group's identifier: random, revealing nothing about its contents.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct GroupId(Vec<u8>);

impl GroupId {
    /// Wraps a known identifier.
    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self(bytes.to_vec())
    }

    /// The identifier's bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    fn of(group: &MlsGroup) -> Self {
        Self(group.group_id().as_slice().to_vec())
    }

    pub(crate) fn to_openmls(&self) -> openmls::prelude::GroupId {
        openmls::prelude::GroupId::from_slice(&self.0)
    }
}

impl fmt::Debug for GroupId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "GroupId(")?;
        self.0.iter().try_for_each(|b| write!(f, "{b:02x}"))?;
        write!(f, ")")
    }
}

/// Whom to remove from a group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Removal {
    /// All the devices of an account.
    Account(AccountId),
    /// One device of an account.
    Device(AccountId, DeviceId),
}

impl Removal {
    fn matches(&self, certificate: &DeviceCertificate) -> bool {
        match self {
            Self::Account(account) => certificate.account() == account,
            Self::Device(account, device) => {
                certificate.account() == account && certificate.device_id() == *device
            }
        }
    }
}

/// A commit intent, kept until the server accepts it, so it can be replayed if another
/// commit beat it.
#[derive(Debug, Clone)]
pub(crate) enum Operation {
    Add(Vec<VerifiedKeyPackage>),
    Remove(Vec<Removal>),
    CommitProposals,
}

/// Writes a pending intent into the client record of the local database (story 02-06).
pub(crate) fn write_operation(
    out: &mut Writer,
    operation: Option<&Operation>,
) -> Result<(), CodecError> {
    let count = |n: usize| u16::try_from(n).map_err(|_| CodecError::TooLong);
    match operation {
        None => out.u8(0),
        Some(Operation::Add(key_packages)) => {
            out.u8(1);
            out.u16(count(key_packages.len())?);
            for key_package in key_packages {
                let bytes = MlsMessageOut::from(key_package.key_package().clone())
                    .tls_serialize_detached()
                    .map_err(|_| CodecError::Malformed)?;
                out.vec32(&bytes)?;
            }
        }
        Some(Operation::Remove(removals)) => {
            out.u8(2);
            out.u16(count(removals.len())?);
            for removal in removals {
                let (account, device) = match removal {
                    Removal::Account(account) => (account, None),
                    Removal::Device(account, device) => (account, Some(device)),
                };
                out.vec16(account.to_string().as_bytes())?;
                match device {
                    None => out.u8(0),
                    Some(device) => {
                        out.u8(1);
                        out.bytes(device.as_bytes());
                    }
                }
            }
        }
        Some(Operation::CommitProposals) => out.u8(3),
    }
    Ok(())
}

/// Reads back an intent written by [`write_operation`]. A *KeyPackage* is verified again;
/// one that no longer is (its validity ended meanwhile) is dropped from the intent.
pub(crate) fn read_operation(input: &mut Reader<'_>) -> Result<Option<Operation>, CodecError> {
    Ok(match input.u8()? {
        0 => None,
        1 => {
            let mut key_packages = Vec::new();
            for _ in 0..input.u16()? {
                if let Ok(key_package) = verify_key_package(input.vec32()?) {
                    key_packages.push(key_package);
                }
            }
            Some(Operation::Add(key_packages))
        }
        2 => {
            let mut removals = Vec::new();
            for _ in 0..input.u16()? {
                let account = std::str::from_utf8(input.vec16()?)
                    .map_err(|_| CodecError::Malformed)?
                    .parse::<AccountId>()
                    .map_err(|_| CodecError::Malformed)?;
                removals.push(match input.u8()? {
                    0 => Removal::Account(account),
                    1 => Removal::Device(account, DeviceId::from_bytes(input.array()?)),
                    _ => return Err(CodecError::Malformed),
                });
            }
            Some(Operation::Remove(removals))
        }
        3 => Some(Operation::CommitProposals),
        _ => return Err(CodecError::Malformed),
    })
}

/// A commit to send to the server, and the *Welcome* for the added devices.
#[derive(Debug, PartialEq, Eq)]
pub struct Outgoing {
    /// The commit (`MLSMessage`), for the current members.
    pub commit: Vec<u8>,
    /// The *Welcome* (`MLSMessage`), for the added devices.
    pub welcome: Option<Vec<u8>>,
}

/// What a received message produced.
#[derive(Debug, PartialEq, Eq)]
pub enum Incoming {
    /// A message from a member (story 02-05). The sender is authenticated even when the
    /// message is unreadable: the app can say "message from X, unreadable" or "update
    /// Tessel" rather than silently dropping it.
    Message {
        /// The group.
        group: GroupId,
        /// The device that sent it.
        sender: DeviceCertificate,
        /// The message, or why it cannot be read.
        message: Result<Message, MessageError>,
        /// What it changed in the history (story 02-06).
        effect: Effect,
    },
    /// A proposal (a member leaving), kept until the next commit.
    Proposal {
        /// The group.
        group: GroupId,
        /// The device that sent it.
        sender: DeviceCertificate,
    },
    /// An applied commit: the group moves to the next epoch.
    Commit {
        /// The group.
        group: GroupId,
        /// The device that made it.
        sender: DeviceCertificate,
        /// The devices that were added.
        added: Vec<DeviceCertificate>,
        /// The devices that were removed.
        removed: Vec<DeviceCertificate>,
        /// This very device was removed: the group is erased from the device.
        removed_self: bool,
        /// When this device was removed, its messages still queued for the group: they
        /// will never be sent (story 02-07).
        unsent: Vec<MessageId>,
    },
}

/// A group's state on the device.
#[derive(Debug)]
pub(crate) struct Group {
    pub(crate) mls: MlsGroup,
    pub(crate) pending: Option<Operation>,
}

fn create_config() -> MlsGroupCreateConfig {
    MlsGroupCreateConfig::builder()
        .ciphersuite(CIPHERSUITE)
        .capabilities(capabilities())
        .use_ratchet_tree_extension(true)
        .wire_format_policy(PURE_CIPHERTEXT_WIRE_FORMAT_POLICY)
        .max_past_epochs(MAX_PAST_EPOCHS)
        .build()
}

fn join_config() -> MlsGroupJoinConfig {
    MlsGroupJoinConfig::builder()
        .use_ratchet_tree_extension(true)
        .wire_format_policy(PURE_CIPHERTEXT_WIRE_FORMAT_POLICY)
        .max_past_epochs(MAX_PAST_EPOCHS)
        .build()
}

fn certificate_of(member: &Member) -> Result<DeviceCertificate, GroupError> {
    Ok(DeviceCertificate::from_credential(
        &member.credential,
        member.signature_key.as_slice(),
    )?)
}

fn certificate_at(group: &MlsGroup, index: LeafNodeIndex) -> Result<DeviceCertificate, GroupError> {
    certificate_of(&group.member_at(index).ok_or(GroupError::Malformed)?)
}

fn sender_certificate(group: &MlsGroup, sender: &Sender) -> Result<DeviceCertificate, GroupError> {
    match sender {
        Sender::Member(index) => certificate_at(group, *index),
        _ => Err(GroupError::Malformed),
    }
}

/// Checks every leaf that a commit adds or changes, and returns the added and removed
/// devices (read from the state before the commit).
fn summarize(
    group: &MlsGroup,
    commit: &StagedCommit,
) -> Result<(Vec<DeviceCertificate>, Vec<DeviceCertificate>), GroupError> {
    let mut added = Vec::new();
    for add in commit.add_proposals() {
        let key_package = add.add_proposal().key_package();
        if key_package.ciphersuite() != CIPHERSUITE {
            return Err(GroupError::WrongCiphersuite);
        }
        let leaf = key_package.leaf_node();
        added.push(DeviceCertificate::from_credential(
            leaf.credential(),
            leaf.signature_key().as_slice(),
        )?);
    }
    let updated = commit
        .update_proposals()
        .map(|update| update.update_proposal().leaf_node().clone())
        .chain(commit.update_path_leaf_node().cloned());
    for leaf in updated {
        DeviceCertificate::from_credential(leaf.credential(), leaf.signature_key().as_slice())?;
    }
    let removed = commit
        .remove_proposals()
        .map(|r| certificate_at(group, r.remove_proposal().removed()))
        .collect::<Result<Vec<_>, _>>()?;
    Ok((added, removed))
}

fn serialize(message: MlsMessageOut) -> Result<Vec<u8>, GroupError> {
    message.tls_serialize_detached().map_err(mls)
}

impl Group {
    /// Prepares the commit for an intent on the group's current state. `None` when the
    /// intent no longer has a purpose (the devices are already there, or already gone).
    fn stage(
        &mut self,
        provider: &OpenMlsRustCrypto,
        device: &Device,
        operation: Operation,
    ) -> Result<Option<Outgoing>, GroupError> {
        if self.mls.pending_commit().is_some() {
            return Err(GroupError::PendingCommit);
        }
        let members: Vec<(LeafNodeIndex, DeviceCertificate)> = self
            .mls
            .members()
            .map(|m| Ok((m.index, certificate_of(&m)?)))
            .collect::<Result<_, GroupError>>()?;
        let own = self.mls.own_leaf_index();

        let (commit, welcome) = match &operation {
            Operation::Add(key_packages) => {
                let fresh: Vec<KeyPackage> = key_packages
                    .iter()
                    .filter(|kp| {
                        let wanted = kp.certificate();
                        !members.iter().any(|(_, c)| {
                            c.account() == wanted.account() && c.device_id() == wanted.device_id()
                        })
                    })
                    .map(|kp| kp.key_package().clone())
                    .collect();
                if fresh.is_empty() {
                    return Ok(None);
                }
                let (commit, welcome, _) = self
                    .mls
                    .add_members(provider, device, &fresh)
                    .map_err(mls)?;
                (commit, Some(welcome))
            }
            Operation::Remove(removals) => {
                let leaves: Vec<LeafNodeIndex> = members
                    .iter()
                    .filter(|(index, c)| *index != own && removals.iter().any(|r| r.matches(c)))
                    .map(|(index, _)| *index)
                    .collect();
                if leaves.is_empty() {
                    return Ok(None);
                }
                let (commit, welcome, _) = self
                    .mls
                    .remove_members(provider, device, &leaves)
                    .map_err(mls)?;
                (commit, welcome)
            }
            Operation::CommitProposals => {
                if self.mls.pending_proposals().next().is_none() {
                    return Ok(None);
                }
                let (commit, welcome, _) = self
                    .mls
                    .commit_to_pending_proposals(provider, device)
                    .map_err(mls)?;
                (commit, welcome)
            }
        };
        self.pending = Some(operation);
        Ok(Some(Outgoing {
            commit: serialize(commit)?,
            welcome: welcome.map(serialize).transpose()?,
        }))
    }
}

impl Client {
    fn commit(&mut self, group: &GroupId, operation: Operation) -> Result<Outgoing, GroupError> {
        let (provider, device) = (&self.provider, &self.device);
        let state = self.groups.get_mut(group).ok_or(GroupError::UnknownGroup)?;
        state
            .stage(provider, device, operation)?
            .ok_or(GroupError::NothingToCommit)
    }

    /// Creates a group of which this device is the only member.
    pub fn create_group(&mut self) -> Result<GroupId, GroupError> {
        let mls = MlsGroup::new(
            &self.provider,
            &self.device,
            &create_config(),
            self.device.credential_with_key()?,
        )
        .map_err(mls)?;
        let id = GroupId::of(&mls);
        self.groups.insert(id.clone(), Group { mls, pending: None });
        Ok(id)
    }

    /// Prepares the addition of devices (those of a new member, or a new device of an
    /// existing member), through their verified *KeyPackages*. Devices already present
    /// are ignored.
    pub fn add_devices(
        &mut self,
        group: &GroupId,
        key_packages: &[VerifiedKeyPackage],
    ) -> Result<Outgoing, GroupError> {
        self.commit(group, Operation::Add(key_packages.to_vec()))
    }

    /// Prepares the removal of devices or of whole accounts. This very device is never
    /// removed this way: it leaves with [`Client::leave_group`].
    pub fn remove_devices(
        &mut self,
        group: &GroupId,
        removals: &[Removal],
    ) -> Result<Outgoing, GroupError> {
        self.commit(group, Operation::Remove(removals.to_vec()))
    }

    /// Prepares the commit of the received proposals (a member leaving).
    pub fn commit_pending_proposals(&mut self, group: &GroupId) -> Result<Outgoing, GroupError> {
        self.commit(group, Operation::CommitProposals)
    }

    /// The server accepted the commit: the group moves to the next epoch.
    pub fn commit_accepted(&mut self, group: &GroupId) -> Result<(), GroupError> {
        let provider = &self.provider;
        let state = self.groups.get_mut(group).ok_or(GroupError::UnknownGroup)?;
        if state.mls.pending_commit().is_none() {
            return Err(GroupError::NoPendingCommit);
        }
        state.mls.merge_pending_commit(provider).map_err(mls)?;
        state.pending = None;
        Ok(())
    }

    /// The server rejected the commit (another one beat it in this epoch). The commit
    /// is dropped, the intent kept: process the commits that beat it, then
    /// [`Client::retry_commit`].
    pub fn commit_rejected(&mut self, group: &GroupId) -> Result<(), GroupError> {
        let provider = &self.provider;
        let state = self.groups.get_mut(group).ok_or(GroupError::UnknownGroup)?;
        if state.mls.pending_commit().is_none() {
            return Err(GroupError::NoPendingCommit);
        }
        state
            .mls
            .clear_pending_commit(provider.storage())
            .map_err(mls)
    }

    /// Replays the intent of a rejected commit on the up-to-date state. `None` when it
    /// no longer has a purpose (the commits received in the meantime already fulfilled
    /// it).
    pub fn retry_commit(&mut self, group: &GroupId) -> Result<Option<Outgoing>, GroupError> {
        let (provider, device) = (&self.provider, &self.device);
        let state = self.groups.get_mut(group).ok_or(GroupError::UnknownGroup)?;
        let Some(operation) = state.pending.take() else {
            return Ok(None);
        };
        state.stage(provider, device, operation)
    }

    /// Leaves a group: returns a proposal to send to the members. Another member
    /// includes it through a commit; this device is removed when processing that commit.
    pub fn leave_group(&mut self, group: &GroupId) -> Result<Vec<u8>, GroupError> {
        let (provider, device) = (&self.provider, &self.device);
        let state = self.groups.get_mut(group).ok_or(GroupError::UnknownGroup)?;
        serialize(state.mls.leave_group(provider, device).map_err(mls)?)
    }

    /// Joins a group through the received *Welcome*. Rejects any ciphersuite other than
    /// Tessel's, and any group in which a member does not carry a valid certificate.
    pub fn join_group(&mut self, welcome: &[u8]) -> Result<GroupId, GroupError> {
        let message =
            MlsMessageIn::tls_deserialize_exact(welcome).map_err(|_| GroupError::Malformed)?;
        let MlsMessageBodyIn::Welcome(welcome) = message.extract() else {
            return Err(GroupError::Malformed);
        };
        // Before touching any secret: another ciphersuite does not get in (ADR-001).
        if welcome.ciphersuite() != CIPHERSUITE {
            return Err(GroupError::WrongCiphersuite);
        }
        let staged = StagedWelcome::new_from_welcome(&self.provider, &join_config(), welcome, None)
            .map_err(mls)?;
        for member in staged.members() {
            certificate_of(&member)?;
        }
        let mls = staged.into_group(&self.provider).map_err(mls)?;
        let id = GroupId::of(&mls);
        self.groups.insert(id.clone(), Group { mls, pending: None });
        Ok(id)
    }

    /// Sends a message to the group: records it in the history (story 02-06) and puts
    /// it at the end of the group's send queue (story 02-07), online or not. The host
    /// then sends it with [`Client::next_outbound`]. An edit or a deletion of a message
    /// this account did not write is refused: every member would ignore it.
    pub fn queue_message(
        &mut self,
        group: &GroupId,
        message: &Message,
    ) -> Result<SendId, GroupError> {
        if !self.groups.contains_key(group) {
            return Err(GroupError::UnknownGroup);
        }
        let author = self.device.certificate().account();
        if let Some(reason) = self.history.verdict(group, author, message) {
            return Err(GroupError::Ignored(reason));
        }
        // The format is checked before anything is recorded.
        message.encode()?;
        let sender = self.device.certificate().clone();
        self.record(group, &sender, message)?;
        Ok(self.enqueue(group, message)?)
    }

    /// The head of the group's send queue, encrypted for the group's current epoch;
    /// `None` when nothing waits. To send once the group is up to date (incoming
    /// commits processed), then [`Client::outbound_sent`] on the server's
    /// acknowledgement; until then, the same message comes back, encrypted again.
    /// While a member's departure awaits its commit, MLS forbids sending:
    /// [`GroupError::PendingProposals`], to resolve with
    /// [`Client::commit_pending_proposals`].
    pub fn next_outbound(&mut self, group: &GroupId) -> Result<Option<Outbound>, GroupError> {
        let state = self.groups.get(group).ok_or(GroupError::UnknownGroup)?;
        let Some(head) = self.outbox.head(group) else {
            return Ok(None);
        };
        if state.mls.pending_proposals().next().is_some() {
            return Err(GroupError::PendingProposals);
        }
        let (send_id, message_id) = (head.send_id, head.message_id);
        let encoded = head.encoded.clone();
        Ok(Some(Outbound {
            group: group.clone(),
            send_id,
            message_id,
            encrypted: self.encrypt(group, &encoded)?,
        }))
    }

    /// The server acknowledged a message: it leaves the queue, and the next one can be
    /// sent. A repeated acknowledgement changes nothing.
    pub fn outbound_sent(&mut self, group: &GroupId, send_id: &SendId) -> Result<(), GroupError> {
        Ok(self.dequeue(group, send_id)?)
    }

    /// The groups whose queue holds messages: what to send when the connection comes
    /// back.
    pub fn outbound_groups(&self) -> Vec<GroupId> {
        self.outbox.groups().cloned().collect()
    }

    /// The group's messages still waiting to be sent, in order (shown as pending).
    pub fn waiting_messages(&self, group: &GroupId) -> Vec<MessageId> {
        self.outbox.waiting(group)
    }

    /// Queues a message and sends it at once, as a host that is online does.
    #[cfg(test)]
    pub(crate) fn send_message(
        &mut self,
        group: &GroupId,
        message: &Message,
    ) -> Result<Vec<u8>, GroupError> {
        let send_id = self.queue_message(group, message)?;
        let outbound = self.next_outbound(group)?.ok_or(GroupError::UnknownGroup)?;
        assert_eq!(outbound.send_id, send_id, "the queue was not empty");
        self.outbound_sent(group, &send_id)?;
        Ok(outbound.encrypted)
    }

    /// Encrypts bytes for the group. Everything goes through [`Client::send_message`],
    /// which guarantees the format and the padding; raw bytes are only for tests.
    pub(crate) fn encrypt(&mut self, group: &GroupId, data: &[u8]) -> Result<Vec<u8>, GroupError> {
        let (provider, device) = (&self.provider, &self.device);
        let state = self.groups.get_mut(group).ok_or(GroupError::UnknownGroup)?;
        serialize(
            state
                .mls
                .create_message(provider, device, data)
                .map_err(mls)?,
        )
    }

    /// Processes a message received from a group: data, proposal or commit.
    pub fn process(&mut self, message: &[u8]) -> Result<Incoming, GroupError> {
        let message =
            MlsMessageIn::tls_deserialize_exact(message).map_err(|_| GroupError::Malformed)?;
        let message = message
            .try_into_protocol_message()
            .map_err(|_| GroupError::Malformed)?;
        let id = GroupId(message.group_id().as_slice().to_vec());
        let provider = &self.provider;
        let state = self.groups.get_mut(&id).ok_or(GroupError::UnknownGroup)?;
        let processed = state.mls.process_message(provider, message).map_err(mls)?;
        let sender = sender_certificate(&state.mls, processed.sender())?;

        match processed.into_content() {
            ProcessedMessageContent::ApplicationMessage(data) => {
                let message = Message::decode(&data.into_bytes());
                let effect = match &message {
                    Ok(message) => self.record(&id, &sender, message)?,
                    Err(_) => Effect::Nothing,
                };
                Ok(Incoming::Message {
                    group: id,
                    sender,
                    message,
                    effect,
                })
            }
            ProcessedMessageContent::ProposalMessage(proposal) => {
                state
                    .mls
                    .store_pending_proposal(provider.storage(), *proposal)
                    .map_err(mls)?;
                Ok(Incoming::Proposal { group: id, sender })
            }
            ProcessedMessageContent::ExternalJoinProposalMessage(_) => Err(GroupError::Malformed),
            ProcessedMessageContent::StagedCommitMessage(commit) => {
                let (added, removed) = summarize(&state.mls, &commit)?;
                let removed_self = commit.self_removed();
                // Another member's commit won this epoch: ours, if there was one, is
                // void; its intent stays for `retry_commit`.
                if state.mls.pending_commit().is_some() {
                    state
                        .mls
                        .clear_pending_commit(provider.storage())
                        .map_err(mls)?;
                }
                state
                    .mls
                    .merge_staged_commit(provider, *commit)
                    .map_err(mls)?;
                let unsent = if removed_self {
                    self.forget(&id)?
                } else {
                    Vec::new()
                };
                Ok(Incoming::Commit {
                    group: id,
                    sender,
                    added,
                    removed,
                    removed_self,
                    unsent,
                })
            }
            // Our own commit sent back by the server: it accepted it. (Only as a
            // `PublicMessage`; when encrypted, its author cannot read it back and the
            // acceptance goes through `commit_accepted`.)
            ProcessedMessageContent::OwnPendingCommit => {
                let commit = state
                    .mls
                    .pending_commit()
                    .ok_or(GroupError::NoPendingCommit)?;
                let (added, removed) = summarize(&state.mls, commit)?;
                state.mls.merge_pending_commit(provider).map_err(mls)?;
                state.pending = None;
                Ok(Incoming::Commit {
                    group: id,
                    sender,
                    added,
                    removed,
                    removed_self: false,
                    unsent: Vec::new(),
                })
            }
            ProcessedMessageContent::OwnPrivateMessage => Err(GroupError::OwnMessage),
        }
    }

    /// The devices that are members of the group.
    pub fn members(&self, group: &GroupId) -> Result<Vec<DeviceCertificate>, GroupError> {
        let state = self.groups.get(group).ok_or(GroupError::UnknownGroup)?;
        state.mls.members().map(|m| certificate_of(&m)).collect()
    }

    /// The group's current epoch.
    pub fn epoch(&self, group: &GroupId) -> Result<u64, GroupError> {
        let state = self.groups.get(group).ok_or(GroupError::UnknownGroup)?;
        Ok(state.mls.epoch().as_u64())
    }

    /// Erases a group from the device, MLS state, history index and send queue
    /// included; returns the messages that will never leave. Its history stays
    /// readable.
    fn forget(&mut self, group: &GroupId) -> Result<Vec<MessageId>, GroupError> {
        if let Some(mut state) = self.groups.remove(group) {
            state.mls.delete(self.provider.storage()).map_err(mls)?;
        }
        Ok(self.forget_local(group)?)
    }
}

#[cfg(test)]
mod tests;
