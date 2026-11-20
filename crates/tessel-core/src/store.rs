//! The encrypted local database (story 02-06).
//!
//! # The contract
//!
//! The core keeps its working state in memory and hands the host **sealed records** to
//! persist. A host database (SQLite on native targets, IndexedDB in the browser) only
//! ever stores opaque bytes:
//!
//! - a record lives in a **partition** and has an **identifier**, both 32 bytes derived
//!   from the store key by HMAC: the host cannot tell what a record is, nor which group
//!   it belongs to;
//! - its **value** is sealed with ChaCha20-Poly1305 (via the OpenMLS crypto provider: no
//!   home-made cryptography), its partition and identifier as associated data, so a
//!   record moved or swapped by someone with access to the disk no longer opens;
//! - its plaintext is padded to its `Padmé` bucket, like messages (story 02-05).
//!
//! After every call that changes the client (`&mut self`), the host calls
//! [`crate::Client::take_changes`] and applies the [`Changes`] **in one transaction,
//! before acting on the call's output** (sending a commit, a message, acknowledging a
//! delivery). If it cannot persist them, it must stop and reopen from the database:
//! otherwise MLS state already used would be lost and the next start would reuse it.
//!
//! The **state** partition ([`StoreKey::state_partition`]) holds everything the client
//! needs to start: the device and its key, the *KeyPackage* pool, the groups and their
//! MLS state, and the index of recent messages. The host reads it whole at startup and
//! gives it to [`crate::Client::open`]. Each group has its own **history** partition
//! ([`crate::Client::history_partition`]), read on demand.
//!
//! # The store key
//!
//! A random 32-byte key, drawn by the core ([`StoreKey::generate`]). The host never
//! keeps it in clear: it seals it with a key of the platform's key store (a
//! non-extractable WebCrypto key in the browser, the Android Keystore on the phone) and
//! keeps only the sealed form, which it unseals at startup to call
//! [`crate::Client::open`]. It is the only secret that crosses the binding, because only
//! the platform's key store can protect it at rest.

use std::fmt;

use openmls_rust_crypto::RustCrypto;
use openmls_traits::crypto::OpenMlsCrypto;
use openmls_traits::random::OpenMlsRand;
use openmls_traits::types::{AeadType, HashType};
use zeroize::Zeroizing;

use crate::codec::{CodecError, Reader, Writer};
use crate::messages::padme;

#[cfg(all(not(target_arch = "wasm32"), any(test, feature = "sqlite")))]
mod sqlite;
#[cfg(all(not(target_arch = "wasm32"), any(test, feature = "sqlite")))]
pub use sqlite::{SQLITE_SCHEMA_VERSION, SqliteStore};

/// Version of the sealed record format written by this core.
pub const STORE_FORMAT: u8 = 1;

/// Size of the store key, of partition and record identifiers.
const KEY_LEN: usize = 32;

/// ChaCha20-Poly1305 nonce size. Drawn at random: a collision is negligible for the
/// number of records a device writes in its life.
const NONCE_LEN: usize = 12;

/// Salt of the key derivation, specific to Tessel's local database.
const KDF_SALT: &[u8] = b"Tessel 1.0 local store";

/// Associated data label of a sealed record.
const AAD_LABEL: &[u8] = b"Tessel 1.0 record";

/// Label of the state partition.
const STATE: &[u8] = b"state";

/// What the local database can reject.
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum StoreError {
    /// No randomness available on this host.
    #[error("no randomness available")]
    NoRandomness,
    /// The store key does not have 32 bytes.
    #[error("invalid store key")]
    InvalidKey,
    /// The record does not open with this key: another key, or altered bytes.
    #[error("record does not open with this key, or was altered")]
    Unreadable,
    /// A record written by a newer core: the app prompts the user to update.
    #[error("unsupported record format {0}")]
    UnsupportedFormat(u8),
    /// The record opens but its content is not valid.
    #[error("malformed record")]
    Malformed,
    /// The state lacks a record it needs.
    #[error("the state lacks a record: {0}")]
    Missing(&'static str),
    /// A record handed to the core belongs to another partition.
    #[error("record from another partition")]
    WrongPartition,
    /// The crypto provider failed.
    #[error("crypto provider failure")]
    Crypto,
    /// The database was written by a newer core (its schema version).
    #[error("database schema {0} is newer than this core")]
    UnsupportedSchema(u32),
    /// The host database failed.
    #[error("database: {0}")]
    Backend(String),
}

impl From<CodecError> for StoreError {
    fn from(_: CodecError) -> Self {
        Self::Malformed
    }
}

/// The key of the local database. Wiped from memory when dropped, never displayed.
pub struct StoreKey(Zeroizing<[u8; KEY_LEN]>);

impl StoreKey {
    /// Draws a new key, when the device is created.
    pub fn generate() -> Result<Self, StoreError> {
        RustCrypto::default()
            .random_array()
            .map(|bytes| Self(Zeroizing::new(bytes)))
            .map_err(|_| StoreError::NoRandomness)
    }

    /// The key the host unsealed with its platform key store.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, StoreError> {
        let bytes: [u8; KEY_LEN] = bytes.try_into().map_err(|_| StoreError::InvalidKey)?;
        Ok(Self(Zeroizing::new(bytes)))
    }

    /// The key's bytes, **only** for the host to seal them with its platform key store
    /// right away (see the module documentation). Wiped when dropped.
    pub fn to_bytes(&self) -> Zeroizing<[u8; KEY_LEN]> {
        self.0.clone()
    }

    /// The partition the host reads whole at startup.
    pub fn state_partition(&self) -> Result<PartitionId, StoreError> {
        Sealer::new(self)?.partition(STATE, &[])
    }
}

impl fmt::Debug for StoreKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("StoreKey(..)")
    }
}

fn hex(f: &mut fmt::Formatter<'_>, name: &str, bytes: &[u8]) -> fmt::Result {
    write!(f, "{name}(")?;
    bytes.iter().try_for_each(|b| write!(f, "{b:02x}"))?;
    write!(f, ")")
}

/// A partition of the database: opaque to the host.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PartitionId([u8; KEY_LEN]);

/// A record's identifier within its partition: opaque to the host.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RecordId([u8; KEY_LEN]);

macro_rules! opaque_id {
    ($name:ident) => {
        impl $name {
            /// Wraps the bytes the host stored.
            pub fn from_bytes(bytes: &[u8]) -> Result<Self, StoreError> {
                bytes
                    .try_into()
                    .map(Self)
                    .map_err(|_| StoreError::Malformed)
            }

            /// The identifier's 32 bytes.
            pub fn as_bytes(&self) -> &[u8; KEY_LEN] {
                &self.0
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                hex(f, stringify!($name), &self.0)
            }
        }
    };
}

opaque_id!(PartitionId);
opaque_id!(RecordId);

/// A sealed record, as the host stores it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    /// Its partition.
    pub partition: PartitionId,
    /// Its identifier within the partition.
    pub id: RecordId,
    /// Its sealed value.
    pub value: Vec<u8>,
}

/// What the host must persist, in one transaction: records to write (replacing a
/// record with the same partition and identifier) and records to delete.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Changes {
    /// Records to write.
    pub put: Vec<Record>,
    /// Records to delete.
    pub delete: Vec<(PartitionId, RecordId)>,
}

impl Changes {
    /// Nothing to persist.
    pub fn is_empty(&self) -> bool {
        self.put.is_empty() && self.delete.is_empty()
    }

    pub(crate) fn append(&mut self, other: Self) {
        self.put.extend(other.put);
        self.delete.extend(other.delete);
    }
}

/// A host database, for hosts that can answer synchronously (native targets). The
/// browser host implements the same contract in TypeScript, over IndexedDB.
pub trait Store {
    /// Applies the changes atomically: all of them, or none.
    fn apply(&mut self, changes: &Changes) -> Result<(), StoreError>;

    /// Every record of a partition, in any order.
    fn list(&self, partition: &PartitionId) -> Result<Vec<Record>, StoreError>;
}

/// Seals and opens records with the keys derived from the store key.
pub(crate) struct Sealer {
    crypto: RustCrypto,
    encryption: Zeroizing<Vec<u8>>,
    identifiers: Zeroizing<Vec<u8>>,
}

impl Sealer {
    pub(crate) fn new(key: &StoreKey) -> Result<Self, StoreError> {
        let crypto = RustCrypto::default();
        let prk = crypto
            .hkdf_extract(HashType::Sha2_256, KDF_SALT, key.0.as_slice())
            .map_err(|_| StoreError::Crypto)?;
        let derive = |info: &[u8]| {
            crypto
                .hkdf_expand(HashType::Sha2_256, prk.as_slice(), info, KEY_LEN)
                .map(|okm| Zeroizing::new(okm.as_slice().to_vec()))
                .map_err(|_| StoreError::Crypto)
        };
        let encryption = derive(b"record encryption")?;
        let identifiers = derive(b"record identifiers")?;
        Ok(Self {
            crypto,
            encryption,
            identifiers,
        })
    }

    fn mac(&self, parts: &[&[u8]]) -> Result<[u8; KEY_LEN], StoreError> {
        let message: Vec<u8> = parts.concat();
        let tag = self
            .crypto
            .hmac(HashType::Sha2_256, &self.identifiers, &message)
            .map_err(|_| StoreError::Crypto)?;
        tag.as_slice().try_into().map_err(|_| StoreError::Crypto)
    }

    /// The partition named `name` under `label`.
    pub(crate) fn partition(&self, label: &[u8], name: &[u8]) -> Result<PartitionId, StoreError> {
        let len = [u8::try_from(label.len()).map_err(|_| StoreError::Malformed)?];
        self.mac(&[b"partition", &len, label, name])
            .map(PartitionId)
    }

    /// The identifier of the record named `name` under `label`, in `partition`.
    pub(crate) fn record_id(
        &self,
        partition: &PartitionId,
        label: &[u8],
        name: &[u8],
    ) -> Result<RecordId, StoreError> {
        let len = [u8::try_from(label.len()).map_err(|_| StoreError::Malformed)?];
        self.mac(&[b"record", &partition.0, &len, label, name])
            .map(RecordId)
    }

    /// Seals a record: its plaintext starts with its label, so that opening it tells
    /// what it is.
    pub(crate) fn seal(
        &self,
        partition: PartitionId,
        label: &[u8],
        name: &[u8],
        body: &[u8],
    ) -> Result<Record, StoreError> {
        let id = self.record_id(&partition, label, name)?;
        let mut plain = Writer::default();
        plain.vec8(label)?;
        plain.bytes(body);
        let content = plain.0.len();
        let mut padded = Zeroizing::new(Vec::with_capacity(padme(content + 4)));
        padded.extend_from_slice(
            &u32::try_from(content)
                .map_err(|_| StoreError::Malformed)?
                .to_be_bytes(),
        );
        padded.extend_from_slice(&plain.0);
        padded.resize(padme(content + 4), 0);
        drop(Zeroizing::new(plain.0));

        let nonce: [u8; NONCE_LEN] = self
            .crypto
            .random_array()
            .map_err(|_| StoreError::NoRandomness)?;
        let sealed = self
            .crypto
            .aead_encrypt(
                AeadType::ChaCha20Poly1305,
                &self.encryption,
                &padded,
                &nonce,
                &aad(&partition, &id),
            )
            .map_err(|_| StoreError::Crypto)?;
        let mut value = Vec::with_capacity(1 + NONCE_LEN + sealed.len());
        value.push(STORE_FORMAT);
        value.extend_from_slice(&nonce);
        value.extend_from_slice(&sealed);
        Ok(Record {
            partition,
            id,
            value,
        })
    }

    /// Opens a record: returns its label and its body.
    pub(crate) fn open(&self, record: &Record) -> Result<Opened, StoreError> {
        let Some((&format, rest)) = record.value.split_first() else {
            return Err(StoreError::Malformed);
        };
        if format != STORE_FORMAT {
            return Err(StoreError::UnsupportedFormat(format));
        }
        if rest.len() < NONCE_LEN {
            return Err(StoreError::Malformed);
        }
        let (nonce, sealed) = rest.split_at(NONCE_LEN);
        let padded = Zeroizing::new(
            self.crypto
                .aead_decrypt(
                    AeadType::ChaCha20Poly1305,
                    &self.encryption,
                    sealed,
                    nonce,
                    &aad(&record.partition, &record.id),
                )
                .map_err(|_| StoreError::Unreadable)?,
        );

        // As for messages: one encoding only, the padding is zeros up to the bucket.
        let mut input = Reader(&padded);
        let content = usize::try_from(input.u32()?).map_err(|_| StoreError::Malformed)?;
        let plain = input.take(content)?;
        if padded.len() != padme(content + 4) || input.0.iter().any(|&b| b != 0) {
            return Err(StoreError::Malformed);
        }
        let mut plain = Reader(plain);
        let label = plain.vec8()?.to_vec();
        Ok(Opened {
            label,
            body: Zeroizing::new(plain.0.to_vec()),
        })
    }
}

/// An opened record.
pub(crate) struct Opened {
    pub(crate) label: Vec<u8>,
    pub(crate) body: Zeroizing<Vec<u8>>,
}

fn aad(partition: &PartitionId, id: &RecordId) -> Vec<u8> {
    [AAD_LABEL, &[STORE_FORMAT], &partition.0, &id.0].concat()
}

#[cfg(test)]
mod tests;
