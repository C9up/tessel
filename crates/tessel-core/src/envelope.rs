//! What the server may read of an envelope (story 04-01): the public header of an MLS
//! message, nothing else.
//!
//! An application message, a proposal or a commit travels as a `PrivateMessage` (or a
//! `PublicMessage` for a commit sent in the clear): its group, its epoch and its content
//! type are public by design (RFC 9420, §6.3), its content and its sender are not. A
//! *Welcome* names no group in the clear. The server routes on this header, and orders
//! commits by epoch (story 04-02); it never decrypts.

use openmls::prelude::{ContentType, MlsMessageBodyIn, MlsMessageIn, ProtocolMessage};
use tls_codec::Deserialize as _;

use crate::client::Client;
use crate::groups::GroupId;
use crate::identity::DeviceId;

/// What kind of MLS message an envelope carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvelopeKind {
    /// An application message (story 02-05).
    Application,
    /// A proposal: a member leaving (story 02-04).
    Proposal,
    /// A commit: the group moves to the next epoch.
    Commit,
    /// A *Welcome*, for devices added to a group.
    Welcome,
}

/// The public header of an envelope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvelopeHeader {
    pub kind: EnvelopeKind,
    /// The group, for anything but a *Welcome*.
    pub group: Option<GroupId>,
    /// The epoch it belongs to, for anything but a *Welcome*.
    pub epoch: Option<u64>,
}

/// What reading an envelope's header can reject.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum EnvelopeError {
    /// Not an MLS message, or not one that travels between devices (a KeyPackage goes to
    /// the directory, story 03-03).
    #[error("not an MLS message for a group")]
    Malformed,
}

/// Reads the public header of an envelope, without decrypting anything.
pub fn inspect_envelope(bytes: &[u8]) -> Result<EnvelopeHeader, EnvelopeError> {
    let message =
        MlsMessageIn::tls_deserialize_exact(bytes).map_err(|_| EnvelopeError::Malformed)?;
    let protocol: ProtocolMessage = match message.extract() {
        MlsMessageBodyIn::PrivateMessage(message) => message.into(),
        MlsMessageBodyIn::PublicMessage(message) => message.into(),
        MlsMessageBodyIn::Welcome(_) => {
            return Ok(EnvelopeHeader {
                kind: EnvelopeKind::Welcome,
                group: None,
                epoch: None,
            });
        }
        _ => return Err(EnvelopeError::Malformed),
    };
    let kind = match protocol.content_type() {
        ContentType::Application => EnvelopeKind::Application,
        ContentType::Proposal => EnvelopeKind::Proposal,
        ContentType::Commit => EnvelopeKind::Commit,
    };
    Ok(EnvelopeHeader {
        kind,
        group: Some(GroupId::from_bytes(protocol.group_id().as_slice())),
        epoch: Some(protocol.epoch().as_u64()),
    })
}

impl Client {
    /// The devices an envelope for `group` goes to: every member but this device (its
    /// own other devices included). For a commit, to take before the commit is accepted:
    /// the devices it removes must hear of it too. A *Welcome* goes to the added devices.
    pub fn recipients(&self, group: &GroupId) -> Result<Vec<DeviceId>, crate::GroupError> {
        let own = self.device.certificate().device_id();
        Ok(self
            .members(group)?
            .iter()
            .map(|member| member.device_id())
            .filter(|device| *device != own)
            .collect())
    }
}

#[cfg(test)]
mod tests;
