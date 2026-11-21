//! *KeyPackages* (RFC 9420, §10; story 02-03).
//!
//! A device uploads to the server a pool of single-use *KeyPackages*, plus a
//! last-resort *KeyPackage* that is used when the pool is empty. Anyone who wants to
//! add the device to a group takes one. The pool is renewed:
//! - a consumed *KeyPackage* (its secret was used to join a group) is replaced;
//! - a *KeyPackage* close to the end of its validity is withdrawn from the server and
//!   replaced, then its secret is wiped once the date has passed (a *Welcome* may still
//!   be in flight).
//!
//! The time comes from the host (`now`, in Unix seconds): the core reads no clock.

use openmls::prelude::{
    Capabilities, ExtensionType, KeyPackage, KeyPackageBundle, KeyPackageIn, KeyPackageRef,
    Lifetime, MlsMessageBodyIn, MlsMessageIn, MlsMessageOut, ProtocolVersion,
};
use openmls_rust_crypto::OpenMlsRustCrypto;
use openmls_traits::OpenMlsProvider;
use openmls_traits::storage::StorageProvider;
use tls_codec::{Deserialize as _, Serialize as _};

use crate::codec::{CodecError, Reader, Writer};
use crate::identity::{Device, DeviceCertificate, IdentityError};
use crate::suite::CIPHERSUITE;

/// Size of the pool of single-use *KeyPackages*.
pub const KEY_PACKAGE_POOL: usize = 100;

/// Validity period of a *KeyPackage*: 12 weeks, the OpenMLS default.
pub const KEY_PACKAGE_LIFETIME: u64 = 60 * 60 * 24 * 7 * 12;

/// A *KeyPackage* is replaced when less than 2 weeks of validity remain.
pub const KEY_PACKAGE_RENEW_BEFORE: u64 = 60 * 60 * 24 * 7 * 2;

/// Tolerance for other parties' clocks: a *KeyPackage* is valid one hour before its
/// creation (like OpenMLS's `Lifetime::new`).
const CLOCK_SKEW: u64 = 60 * 60;

/// What *KeyPackage* handling can reject.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum KeyPackageError {
    /// OpenMLS could not create the *KeyPackage*, or storage failed.
    #[error("key package could not be created or stored")]
    Creation,
    /// The bytes are not an MLS *KeyPackage*.
    #[error("not an MLS key package")]
    Malformed,
    /// The *KeyPackage* is not in Tessel's suite (ADR-001).
    #[error("key package uses another ciphersuite")]
    WrongCiphersuite,
    /// Invalid signature, version, validity period or extensions.
    #[error("key package is invalid: {0}")]
    Invalid(String),
    /// The *credential* does not carry a valid device certificate.
    #[error(transparent)]
    Identity(#[from] IdentityError),
}

/// A *KeyPackage* the device has uploaded.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Published {
    reference: KeyPackageRef,
    not_after: u64,
    last_resort: bool,
    /// Withdrawn from the server; its secret waits for the end of validity to be wiped.
    withdrawn: bool,
}

/// What the device must send to the directory (story 03-03) after a renewal.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct KeyPackageUpdate {
    /// New single-use *KeyPackages*, serialized as `MLSMessage`.
    pub publish: Vec<Vec<u8>>,
    /// New last-resort *KeyPackage*, if it changes.
    pub last_resort: Option<Vec<u8>>,
    /// References (`KeyPackageRef`) of the *KeyPackages* to withdraw from the server.
    pub withdraw: Vec<Vec<u8>>,
}

impl KeyPackageUpdate {
    /// Nothing to send.
    pub fn is_empty(&self) -> bool {
        self.publish.is_empty() && self.last_resort.is_none() && self.withdraw.is_empty()
    }
}

/// The *KeyPackage* pool of a device.
#[derive(Debug, Default)]
pub(crate) struct KeyPackagePool {
    published: Vec<Published>,
}

impl KeyPackagePool {
    /// Brings the pool back in shape at time `now`: replaces what is consumed or close
    /// to its end, wipes expired secrets, and returns what needs to be sent.
    pub(crate) fn refresh(
        &mut self,
        provider: &OpenMlsRustCrypto,
        device: &Device,
        now: u64,
    ) -> Result<KeyPackageUpdate, KeyPackageError> {
        let mut update = KeyPackageUpdate::default();
        let storage = provider.storage();

        let mut kept = Vec::with_capacity(self.published.len());
        for mut entry in std::mem::take(&mut self.published) {
            if now >= entry.not_after {
                storage
                    .delete_key_package(&entry.reference)
                    .map_err(|_| KeyPackageError::Creation)?;
                if !entry.withdrawn {
                    update.withdraw.push(entry.reference.as_slice().to_vec());
                }
                continue;
            }
            if entry.withdrawn {
                kept.push(entry);
                continue;
            }
            let consumed = storage
                .key_package::<KeyPackageRef, KeyPackageBundle>(&entry.reference)
                .map_err(|_| KeyPackageError::Creation)?
                .is_none();
            if consumed {
                continue;
            }
            if entry.not_after - now < KEY_PACKAGE_RENEW_BEFORE {
                update.withdraw.push(entry.reference.as_slice().to_vec());
                entry.withdrawn = true;
            }
            kept.push(entry);
        }
        self.published = kept;

        let live = |p: &&Published| !p.withdrawn;
        if !self.published.iter().filter(live).any(|p| p.last_resort) {
            let (bytes, entry) = create(provider, device, now, true)?;
            update.last_resort = Some(bytes);
            self.published.push(entry);
        }
        let missing = KEY_PACKAGE_POOL.saturating_sub(
            self.published
                .iter()
                .filter(live)
                .filter(|p| !p.last_resort)
                .count(),
        );
        for _ in 0..missing {
            let (bytes, entry) = create(provider, device, now, false)?;
            update.publish.push(bytes);
            self.published.push(entry);
        }
        Ok(update)
    }

    /// Writes the pool into a record of the local database (story 02-06). The secrets of
    /// the *KeyPackages* live in the OpenMLS state; this is only their bookkeeping.
    pub(crate) fn write_to(&self, out: &mut Writer) -> Result<(), CodecError> {
        out.u16(u16::try_from(self.published.len()).map_err(|_| CodecError::TooLong)?);
        for entry in &self.published {
            let reference = entry
                .reference
                .tls_serialize_detached()
                .map_err(|_| CodecError::Malformed)?;
            out.vec16(&reference)?;
            out.u64(entry.not_after);
            out.bool(entry.last_resort);
            out.bool(entry.withdrawn);
        }
        Ok(())
    }

    /// Reads back a pool written by [`KeyPackagePool::write_to`].
    pub(crate) fn read_from(input: &mut Reader<'_>) -> Result<Self, CodecError> {
        let count = input.u16()?;
        let published = (0..count)
            .map(|_| {
                Ok(Published {
                    reference: KeyPackageRef::tls_deserialize_exact(input.vec16()?)
                        .map_err(|_| CodecError::Malformed)?,
                    not_after: input.u64()?,
                    last_resort: input.bool()?,
                    withdrawn: input.bool()?,
                })
            })
            .collect::<Result<_, CodecError>>()?;
        Ok(Self { published })
    }
}

fn create(
    provider: &OpenMlsRustCrypto,
    device: &Device,
    now: u64,
    last_resort: bool,
) -> Result<(Vec<u8>, Published), KeyPackageError> {
    let not_after = now + KEY_PACKAGE_LIFETIME;
    let mut builder = KeyPackage::builder()
        .key_package_lifetime(Lifetime::init(now.saturating_sub(CLOCK_SKEW), not_after))
        .leaf_node_capabilities(capabilities());
    if last_resort {
        builder = builder.mark_as_last_resort();
    }
    let bundle = builder
        .build(CIPHERSUITE, provider, device, device.credential_with_key()?)
        .map_err(|_| KeyPackageError::Creation)?;
    let key_package = bundle.key_package();
    let reference = key_package
        .hash_ref(provider.crypto())
        .map_err(|_| KeyPackageError::Creation)?;
    let bytes = MlsMessageOut::from(key_package.clone())
        .tls_serialize_detached()
        .map_err(|_| KeyPackageError::Creation)?;
    Ok((
        bytes,
        Published {
            reference,
            not_after,
            last_resort,
            withdrawn: false,
        },
    ))
}

/// What the leaf of a Tessel device declares it supports: Tessel's only suite, and
/// the *last_resort* extension that a last-resort *KeyPackage* carries.
pub(crate) fn capabilities() -> Capabilities {
    Capabilities::builder()
        .ciphersuites(vec![CIPHERSUITE])
        // The group settings (story 05-02), which every group requires of its members.
        .extensions(vec![
            ExtensionType::LastResort,
            ExtensionType::Unknown(crate::groups::GROUP_SETTINGS_EXTENSION),
        ])
        .build()
}

/// A received and verified *KeyPackage*: signatures, validity period, Tessel suite,
/// and device certificate.
#[derive(Debug, Clone)]
pub struct VerifiedKeyPackage {
    key_package: KeyPackage,
    certificate: DeviceCertificate,
    reference: KeyPackageRef,
}

impl VerifiedKeyPackage {
    /// The device that published it. Its account key still has to be compared with the
    /// one known for the account (see [`DeviceCertificate`]).
    pub fn certificate(&self) -> &DeviceCertificate {
        &self.certificate
    }

    /// Its reference (`KeyPackageRef`).
    pub fn reference(&self) -> &[u8] {
        self.reference.as_slice()
    }

    pub(crate) fn key_package(&self) -> &KeyPackage {
        &self.key_package
    }

    #[cfg(test)]
    pub(crate) fn key_package_ref(&self) -> &KeyPackageRef {
        &self.reference
    }

    /// A last-resort *KeyPackage*.
    pub fn is_last_resort(&self) -> bool {
        self.key_package.last_resort()
    }

    /// The end of its validity (Unix seconds): the directory hands it out no later.
    pub fn not_after(&self) -> u64 {
        self.key_package.life_time().not_after()
    }
}

/// Parses and verifies a *KeyPackage* serialized as `MLSMessage`.
pub fn verify_key_package(bytes: &[u8]) -> Result<VerifiedKeyPackage, KeyPackageError> {
    let message =
        MlsMessageIn::tls_deserialize_exact(bytes).map_err(|_| KeyPackageError::Malformed)?;
    let MlsMessageBodyIn::KeyPackage(key_package) = message.extract() else {
        return Err(KeyPackageError::Malformed);
    };
    validate(key_package)
}

fn validate(key_package: KeyPackageIn) -> Result<VerifiedKeyPackage, KeyPackageError> {
    let provider = OpenMlsRustCrypto::default();
    let key_package = key_package
        .validate(provider.crypto(), ProtocolVersion::Mls10)
        .map_err(|e| KeyPackageError::Invalid(e.to_string()))?;
    if key_package.ciphersuite() != CIPHERSUITE {
        return Err(KeyPackageError::WrongCiphersuite);
    }
    let leaf = key_package.leaf_node();
    let certificate =
        DeviceCertificate::from_credential(leaf.credential(), leaf.signature_key().as_slice())?;
    let reference = key_package
        .hash_ref(provider.crypto())
        .map_err(|_| KeyPackageError::Malformed)?;
    Ok(VerifiedKeyPackage {
        key_package,
        certificate,
        reference,
    })
}

#[cfg(test)]
mod tests;
