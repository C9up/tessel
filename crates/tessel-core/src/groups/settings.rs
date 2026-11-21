//! A group's settings (story 05-02): its name, its admins, and whether only they may
//! change who is in it.
//!
//! They live in the MLS group context, under an extension of Tessel's own: every member
//! holds the same settings at each epoch, only a commit changes them, and the server
//! never sees them (commits are `PrivateMessage`s, a *Welcome* encrypts its
//! `GroupInfo`). A `RequiredCapabilities` extension keeps out a device that would not
//! understand them.
//!
//! The rules are checked twice, by the same function: when this device prepares a
//! commit, and when it processes a commit or a proposal it received. What a modified
//! client would try, honest ones refuse.

use openmls::prelude::{
    Extension, ExtensionType, Extensions, GroupContext, MlsGroup, Proposal,
    RequiredCapabilitiesExtension, UnknownExtension,
};

use crate::codec::{Reader, Writer};
use crate::identity::{AccountId, DeviceCertificate};

use super::GroupError;

/// The group context extension holding the settings (MLS private-use range).
pub const GROUP_SETTINGS_EXTENSION: u16 = 0xF7A0;

/// A group name's maximum length, in UTF-8 bytes.
pub const MAX_GROUP_NAME: usize = 128;

/// Version of the encoded settings.
const SETTINGS_VERSION: u8 = 1;

/// A group's settings, as every member holds them.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GroupSettings {
    /// Its name; empty for none (a conversation between two people).
    pub name: String,
    /// The accounts that administer it. The creator is the first.
    pub admins: Vec<AccountId>,
    /// Only admins add or remove other accounts, and rename it.
    pub restricted: bool,
}

/// A change to the settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupChange {
    /// A new name (empty: none).
    Rename(String),
    /// Makes an account admin (`true`) or not (`false`).
    Admin(AccountId, bool),
    /// Reserves (`true`) or opens (`false`) membership changes to admins.
    Restrict(bool),
}

fn invalid_name(name: &str) -> bool {
    name.len() > MAX_GROUP_NAME || name.chars().any(char::is_control)
}

impl GroupSettings {
    /// The settings of a group this account creates: unnamed, administered by it.
    pub(crate) fn created_by(account: &AccountId) -> Self {
        Self {
            name: String::new(),
            admins: vec![account.clone()],
            restricted: false,
        }
    }

    /// The settings with `change` applied.
    pub(crate) fn with(&self, change: &GroupChange) -> Result<Self, GroupError> {
        let mut next = self.clone();
        match change {
            GroupChange::Rename(name) => {
                if invalid_name(name) {
                    return Err(GroupError::InvalidName);
                }
                next.name.clone_from(name);
            }
            GroupChange::Admin(account, true) => {
                if !next.admins.contains(account) {
                    next.admins.push(account.clone());
                }
            }
            GroupChange::Admin(account, false) => next.admins.retain(|a| a != account),
            GroupChange::Restrict(restricted) => next.restricted = *restricted,
        }
        Ok(next)
    }

    fn encode(&self) -> Result<Vec<u8>, GroupError> {
        let mut out = Writer::default();
        out.u8(SETTINGS_VERSION);
        out.vec16(self.name.as_bytes())
            .map_err(|_| GroupError::InvalidName)?;
        out.u8(u8::from(self.restricted));
        out.u16(u16::try_from(self.admins.len()).map_err(|_| GroupError::Malformed)?);
        for admin in &self.admins {
            out.vec16(admin.to_string().as_bytes())
                .map_err(|_| GroupError::Malformed)?;
        }
        Ok(out.0)
    }

    fn decode(bytes: &[u8]) -> Result<Self, GroupError> {
        let malformed = |_| GroupError::Malformed;
        let mut input = Reader(bytes);
        if input.u8().map_err(malformed)? != SETTINGS_VERSION {
            return Err(GroupError::Malformed);
        }
        let name = input.string16().map_err(malformed)?;
        let restricted = input.bool().map_err(malformed)?;
        let mut admins = Vec::new();
        for _ in 0..input.u16().map_err(malformed)? {
            let account = input.string16().map_err(malformed)?;
            admins.push(account.parse().map_err(|_| GroupError::Malformed)?);
        }
        input.finish().map_err(malformed)?;
        if invalid_name(&name) {
            return Err(GroupError::Malformed);
        }
        Ok(Self {
            name,
            admins,
            restricted,
        })
    }

    /// The group context extensions that carry these settings.
    pub(crate) fn extensions(&self) -> Result<Extensions<GroupContext>, GroupError> {
        Extensions::from_vec(vec![
            Extension::RequiredCapabilities(RequiredCapabilitiesExtension::new(
                &[ExtensionType::Unknown(GROUP_SETTINGS_EXTENSION)],
                &[],
                &[],
            )),
            Extension::Unknown(GROUP_SETTINGS_EXTENSION, UnknownExtension(self.encode()?)),
        ])
        .map_err(super::mls)
    }

    /// The settings carried by group context extensions; the defaults (no name, no
    /// admin) when there are none.
    pub(crate) fn of_context(extensions: &Extensions<GroupContext>) -> Result<Self, GroupError> {
        extensions
            .unknown(GROUP_SETTINGS_EXTENSION)
            .map_or_else(|| Ok(Self::default()), |e| Self::decode(&e.0))
    }

    /// The group's current settings.
    pub(crate) fn of(group: &MlsGroup) -> Result<Self, GroupError> {
        Self::of_context(group.extensions())
    }
}

/// What a member may do in a group, given its settings and its members (story 05-02).
pub(crate) struct Rules<'a> {
    settings: &'a GroupSettings,
    /// The admins that still have a device in the group.
    admins: Vec<&'a AccountId>,
}

impl<'a> Rules<'a> {
    pub(crate) fn new(settings: &'a GroupSettings, members: &[DeviceCertificate]) -> Self {
        let admins = settings
            .admins
            .iter()
            .filter(|admin| members.iter().any(|m| m.account() == *admin))
            .collect();
        Self { settings, admins }
    }

    fn is_admin(&self, account: &AccountId) -> bool {
        self.admins.contains(&account)
    }

    /// Reserved to admins: only while one of them is still in the group, so a group is
    /// never locked for good.
    fn restricted(&self) -> bool {
        self.settings.restricted && !self.admins.is_empty()
    }

    /// `author` adds or removes devices of `target`: its own always, another account's
    /// when the group is open or `author` administers it.
    pub(crate) fn membership(
        &self,
        author: &AccountId,
        target: &AccountId,
    ) -> Result<(), GroupError> {
        if author == target || !self.restricted() || self.is_admin(author) {
            Ok(())
        } else {
            Err(GroupError::NotAdmin)
        }
    }

    /// `author` changes the settings to `next`: the name as membership is ruled, the
    /// admins and the restriction only by an admin (anyone, once none is left).
    pub(crate) fn settings(
        &self,
        author: &AccountId,
        next: &GroupSettings,
    ) -> Result<(), GroupError> {
        let admin = self.is_admin(author);
        if next.name != self.settings.name && self.restricted() && !admin {
            return Err(GroupError::NotAdmin);
        }
        let roles_changed =
            next.admins != self.settings.admins || next.restricted != self.settings.restricted;
        if roles_changed && !admin && !self.admins.is_empty() {
            return Err(GroupError::NotAdmin);
        }
        Ok(())
    }

    /// Checks a proposal by `author`, alone or within a commit. `removed` is the device a
    /// Remove proposal removes.
    pub(crate) fn proposal(
        &self,
        author: &DeviceCertificate,
        proposal: &Proposal,
        removed: impl FnOnce() -> Result<DeviceCertificate, GroupError>,
    ) -> Result<(), GroupError> {
        match proposal {
            Proposal::Add(add) => {
                let leaf = add.key_package().leaf_node();
                let added = DeviceCertificate::from_credential(
                    leaf.credential(),
                    leaf.signature_key().as_slice(),
                )?;
                self.membership(author.account(), added.account())
            }
            Proposal::Remove(_) => self.membership(author.account(), removed()?.account()),
            Proposal::GroupContextExtensions(proposal) => {
                let next = GroupSettings::of_context(proposal.extensions())?;
                if proposal
                    .extensions()
                    .unknown(GROUP_SETTINGS_EXTENSION)
                    .is_none()
                {
                    // The settings cannot be dropped.
                    return Err(GroupError::Malformed);
                }
                self.settings(author.account(), &next)
            }
            // Its own key update: anyone.
            Proposal::Update(_) => Ok(()),
            // Tessel makes none of the others.
            _ => Err(GroupError::Malformed),
        }
    }
}
