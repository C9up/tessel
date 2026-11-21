//! Safety numbers (story 02-09).
//!
//! Two people check that they are talking to each other, and not to someone the server
//! put in between, by comparing a code on both screens or by scanning each other's QR.
//! The code comes from their **account** identities (account and account key), not
//! from their devices: adding a device does not change it, but the device is reported
//! when the contact is verified.
//!
//! The derivation is the one of Signal's numeric fingerprints: for each person, SHA-512
//! iterated [`ITERATIONS`] times over the account key and the account; the first 30
//! bytes give 30 digits, 5 by 5. The code is both halves, the smaller first, so that it
//! reads the same on both sides. The QR carries both fingerprints in the shower's order;
//! the scanner checks they are the reverse of its own.
//!
//! Once a contact is verified, the core remembers its account key and its devices:
//! [`Client::trust`] then reports a device of that account with another account key
//! ([`Trust::KeyChanged`], the verification no longer holds) or a device not seen at
//! verification time ([`Trust::NewDevice`]).

use std::collections::HashMap;
use std::str::FromStr as _;

use sha2::{Digest as _, Sha512};

use crate::client::Client;
use crate::codec::{Reader, Writer};
use crate::identity::{AccountId, DeviceCertificate, DeviceId};
use crate::store::{Changes, PartitionId, Sealer, StoreError};

/// Number of hash iterations, as in Signal: slows down the search for another key with
/// the same code.
const ITERATIONS: usize = 5200;

/// Separates Tessel's fingerprints from any other use of the same keys.
const FINGERPRINT_LABEL: &[u8] = b"Tessel 1.0 safety number";

/// Version of the QR payload.
pub const SAFETY_QR_VERSION: u8 = 1;

/// Number of digits of a safety number: 30 per person.
pub const SAFETY_NUMBER_DIGITS: usize = 60;

const QR_FINGERPRINT_LEN: usize = 32;
const QR_LEN: usize = 1 + 2 * QR_FINGERPRINT_LEN;

/// Label of a verified contact, in the state partition.
pub(crate) const VERIFIED: &[u8] = b"verified";

/// What checking a scanned code can reject.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SafetyError {
    /// The scanned data is not a Tessel safety code.
    #[error("not a safety code")]
    Malformed,
    /// A safety code of a version this one does not know.
    #[error("unsupported safety code version {0}")]
    UnsupportedVersion(u8),
    /// The codes differ: the keys are not the ones the other person has, or the code is
    /// for another conversation.
    #[error("safety codes differ")]
    Mismatch,
    /// The device belongs to an account whose key changed since it was verified: the
    /// contact has to be verified again.
    #[error("account key changed since verification")]
    KeyChanged,
    /// The contact is not verified.
    #[error("contact not verified")]
    NotVerified,
    /// The local database failed.
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// The safety number of two accounts, the same on both sides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SafetyNumber {
    /// [`SAFETY_NUMBER_DIGITS`] digits, to show in groups of five.
    pub digits: String,
    /// What this device shows as a QR, for the other one to scan.
    pub qr: Vec<u8>,
}

/// What the verification of an account says about one of its devices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trust {
    /// The account was never verified.
    Unverified,
    /// Verified, and the device was known at verification time or acknowledged since.
    Verified,
    /// Verified account, same key, but a device added since: to report.
    NewDevice,
    /// The account key is not the verified one: to report, and to verify again.
    KeyChanged,
}

fn fingerprint(account: &AccountId, account_key: &[u8]) -> [u8; 64] {
    let mut digest = Sha512::new()
        .chain_update(FINGERPRINT_LABEL)
        .chain_update(account_key)
        .chain_update(account.to_string().as_bytes())
        .finalize();
    for _ in 0..ITERATIONS {
        digest = Sha512::new()
            .chain_update(digest)
            .chain_update(account_key)
            .finalize();
    }
    digest.into()
}

/// 30 digits: 6 groups of 5, each from 5 bytes of the fingerprint.
fn digits(fingerprint: &[u8; 64]) -> String {
    fingerprint[..30]
        .chunks(5)
        .map(|chunk| {
            let value = chunk.iter().fold(0u64, |acc, b| (acc << 8) | u64::from(*b));
            format!("{:05}", value % 100_000)
        })
        .collect()
}

/// The safety number between `own` and `contact`, as `own` shows it.
pub(crate) fn safety_number(own: &DeviceCertificate, contact: &DeviceCertificate) -> SafetyNumber {
    let mine = fingerprint(own.account(), own.account_key());
    let theirs = fingerprint(contact.account(), contact.account_key());
    let (a, b) = (digits(&mine), digits(&theirs));
    let digits = if a <= b { a + &b } else { b + &a };
    let mut qr = Vec::with_capacity(QR_LEN);
    qr.push(SAFETY_QR_VERSION);
    qr.extend_from_slice(&mine[..QR_FINGERPRINT_LEN]);
    qr.extend_from_slice(&theirs[..QR_FINGERPRINT_LEN]);
    SafetyNumber { digits, qr }
}

/// Checks the QR the contact shows: it must hold their fingerprint, then ours.
pub(crate) fn check_scanned(
    own: &DeviceCertificate,
    contact: &DeviceCertificate,
    scanned: &[u8],
) -> Result<(), SafetyError> {
    let (&version, rest) = scanned.split_first().ok_or(SafetyError::Malformed)?;
    if version != SAFETY_QR_VERSION {
        return Err(SafetyError::UnsupportedVersion(version));
    }
    if rest.len() != 2 * QR_FINGERPRINT_LEN {
        return Err(SafetyError::Malformed);
    }
    let (theirs, mine) = rest.split_at(QR_FINGERPRINT_LEN);
    let expected_theirs = fingerprint(contact.account(), contact.account_key());
    let expected_mine = fingerprint(own.account(), own.account_key());
    if theirs != &expected_theirs[..QR_FINGERPRINT_LEN]
        || mine != &expected_mine[..QR_FINGERPRINT_LEN]
    {
        return Err(SafetyError::Mismatch);
    }
    Ok(())
}

/// A verified account: its key, and its devices known at verification or acknowledged.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Verified {
    account_key: Vec<u8>,
    devices: Vec<DeviceId>,
}

/// The verified contacts and the records they still have to hand over.
#[derive(Debug, Default)]
pub(crate) struct Contacts {
    verified: HashMap<AccountId, Verified>,
    pending: Changes,
}

impl Contacts {
    fn trust(&self, device: &DeviceCertificate) -> Trust {
        match self.verified.get(device.account()) {
            None => Trust::Unverified,
            Some(v) if v.account_key != device.account_key() => Trust::KeyChanged,
            Some(v) if v.devices.contains(&device.device_id()) => Trust::Verified,
            Some(_) => Trust::NewDevice,
        }
    }

    fn put(
        &mut self,
        sealer: &Sealer,
        state: PartitionId,
        account: AccountId,
        mut verified: Verified,
    ) -> Result<(), StoreError> {
        verified.devices.sort_by_key(|d| *d.as_bytes());
        verified.devices.dedup();
        let mut body = Writer::default();
        body.vec16(account.to_string().as_bytes())?;
        body.vec16(&verified.account_key)?;
        body.u16(u16::try_from(verified.devices.len()).map_err(|_| StoreError::Malformed)?);
        for device in &verified.devices {
            body.bytes(device.as_bytes());
        }
        let name = account.to_string();
        self.pending
            .put
            .push(sealer.seal(state, VERIFIED, name.as_bytes(), &body.0)?);
        self.verified.insert(account, verified);
        Ok(())
    }

    fn remove(
        &mut self,
        sealer: &Sealer,
        state: PartitionId,
        account: &AccountId,
    ) -> Result<(), StoreError> {
        if self.verified.contains_key(account) {
            let id = sealer.record_id(&state, VERIFIED, account.to_string().as_bytes())?;
            self.pending.delete.push((state, id));
            self.verified.remove(account);
        }
        Ok(())
    }

    /// The records waiting to be persisted.
    pub(crate) fn take(&mut self) -> Changes {
        std::mem::take(&mut self.pending)
    }

    /// Reloads a verified contact from the state, at startup.
    pub(crate) fn load(&mut self, body: &[u8]) -> Result<(), StoreError> {
        let mut input = Reader(body);
        let account = std::str::from_utf8(input.vec16()?).map_err(|_| StoreError::Malformed)?;
        let account = AccountId::from_str(account).map_err(|_| StoreError::Malformed)?;
        let account_key = input.vec16()?.to_vec();
        let devices = (0..input.u16()?)
            .map(|_| Ok(DeviceId::from_bytes(input.array()?)))
            .collect::<Result<Vec<_>, StoreError>>()?;
        input.finish()?;
        self.verified.insert(
            account,
            Verified {
                account_key,
                devices,
            },
        );
        Ok(())
    }
}

impl Client {
    /// The safety number between this account and the contact's.
    pub fn safety_number(&self, contact: &DeviceCertificate) -> SafetyNumber {
        safety_number(self.device.certificate(), contact)
    }

    /// Checks the QR shown by the contact's device. On success, the app can mark the
    /// contact verified ([`Client::mark_verified`]).
    pub fn check_scanned(
        &self,
        contact: &DeviceCertificate,
        scanned: &[u8],
    ) -> Result<(), SafetyError> {
        check_scanned(self.device.certificate(), contact, scanned)
    }

    /// The user compared the codes, or scanned the QR: the contact's account is
    /// verified, with its key and its devices present in this device's groups.
    pub fn mark_verified(&mut self, contact: &DeviceCertificate) -> Result<(), SafetyError> {
        let account = contact.account();
        let mut devices = vec![contact.device_id()];
        for group in self.groups.values() {
            for member in group.mls.members() {
                let Ok(member) = DeviceCertificate::from_credential(
                    &member.credential,
                    member.signature_key.as_slice(),
                ) else {
                    continue;
                };
                // A device of the same account under another key is not vouched for.
                if member.account() == account && member.account_key() == contact.account_key() {
                    devices.push(member.device_id());
                }
            }
        }
        let verified = Verified {
            account_key: contact.account_key().to_vec(),
            devices,
        };
        let account = account.clone();
        let (contacts, sealer, state) = self.contacts_mut();
        contacts.put(sealer, state, account, verified)?;
        Ok(())
    }

    /// The contact is no longer verified.
    pub fn unverify(&mut self, account: &AccountId) -> Result<(), SafetyError> {
        let (contacts, sealer, state) = self.contacts_mut();
        contacts.remove(sealer, state, account)?;
        Ok(())
    }

    /// What the verification of its account says about a device: to check for each
    /// device a commit adds (`Incoming::Commit::added`), for the members of a joined
    /// group, and for the sender of a message.
    pub fn trust(&self, device: &DeviceCertificate) -> Trust {
        self.contacts.trust(device)
    }

    /// Among devices a commit removed, those of verified contacts that were vouched for:
    /// they leave their account's verified devices (story 03-05).
    pub(crate) fn forget_verified_devices(
        &mut self,
        removed: &[DeviceCertificate],
    ) -> Result<Vec<DeviceCertificate>, StoreError> {
        let mut forgotten = Vec::new();
        for device in removed {
            if self.contacts.trust(device) != Trust::Verified {
                continue;
            }
            let Some(mut verified) = self.contacts.verified.get(device.account()).cloned() else {
                continue;
            };
            verified.devices.retain(|id| *id != device.device_id());
            let (contacts, sealer, state) = self.contacts_mut();
            contacts.put(sealer, state, device.account().clone(), verified)?;
            forgotten.push(device.clone());
        }
        Ok(forgotten)
    }

    /// The user saw that a verified contact added a device: it is no longer reported.
    /// Refused when the account key changed: the contact has to be verified again.
    pub fn acknowledge_device(&mut self, device: &DeviceCertificate) -> Result<(), SafetyError> {
        let mut verified = match self.contacts.trust(device) {
            Trust::Unverified => return Err(SafetyError::NotVerified),
            Trust::KeyChanged => return Err(SafetyError::KeyChanged),
            Trust::Verified => return Ok(()),
            Trust::NewDevice => self
                .contacts
                .verified
                .get(device.account())
                .cloned()
                .ok_or(SafetyError::NotVerified)?,
        };
        verified.devices.push(device.device_id());
        let (contacts, sealer, state) = self.contacts_mut();
        contacts.put(sealer, state, device.account().clone(), verified)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
