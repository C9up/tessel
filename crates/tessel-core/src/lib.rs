//! Tessel client core.
//!
//! Everything that touches keys and encryption lives here, once, for the web (WASM)
//! as well as for mobile (native). No private key leaves this crate, except sealed into
//! the local database ([`store`]).

mod client;
mod codec;
mod groups;
mod history;
mod identity;
mod key_packages;
mod media;
mod messages;
mod outbox;
mod safety;
pub mod store;
mod suite;
#[cfg(test)]
mod test_support;

pub use client::{Client, ClientError};
pub use groups::{GroupError, GroupId, Incoming, Outgoing, Removal};
pub use history::{Content, EDITABLE_MESSAGES, Effect, HistoryEntry, Ignored};
pub use identity::{
    AccountId, AccountKey, CHALLENGE_LEN, Device, DeviceCertificate, DeviceId, DeviceKey,
    IdentityError,
};
pub use key_packages::{
    KEY_PACKAGE_LIFETIME, KEY_PACKAGE_POOL, KEY_PACKAGE_RENEW_BEFORE, KeyPackageError,
    KeyPackageUpdate, VerifiedKeyPackage, verify_key_package,
};
pub use media::{
    EncryptedMedia, MEDIA_CHUNK, MEDIA_FORMAT, MediaDecryptor, MediaEncryptor, MediaError,
    MediaSeal, decrypt_media, encrypt_media, encrypted_len,
};
pub use messages::{
    Body, MAX_RECEIPT, MAX_TEXT, MESSAGE_VERSION, MIN_PADDED, MediaKey, MediaRef, Message,
    MessageError, MessageId, ReceiptKind,
};
pub use outbox::{Outbound, SendId};
pub use safety::{SAFETY_NUMBER_DIGITS, SAFETY_QR_VERSION, SafetyError, SafetyNumber, Trust};
pub use store::{
    Changes, PartitionId, Record, RecordId, STORE_FORMAT, Store, StoreError, StoreKey,
};
pub use suite::{CIPHERSUITE_ID, CIPHERSUITE_NAME, CoreError, self_check};

/// Core version, as published by Cargo.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_the_crate_version() {
        assert_eq!(version(), env!("CARGO_PKG_VERSION"));
    }
}
