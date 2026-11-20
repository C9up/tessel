//! Cœur client de Tessel.
//!
//! Tout ce qui touche aux clés et au chiffrement vit ici, une seule fois, pour le
//! web (WASM) comme pour le mobile (natif). Aucune clé privée ne sort de ce crate.

mod client;
mod groups;
mod identity;
mod key_packages;
mod suite;

pub use client::Client;
pub use groups::{GroupError, GroupId, Incoming, Outgoing, Removal};
pub use identity::{
    AccountId, AccountKey, Device, DeviceCertificate, DeviceId, DeviceKey, IdentityError,
};
pub use key_packages::{
    KEY_PACKAGE_LIFETIME, KEY_PACKAGE_POOL, KEY_PACKAGE_RENEW_BEFORE, KeyPackageError,
    KeyPackageUpdate, VerifiedKeyPackage, verify_key_package,
};
pub use suite::{CIPHERSUITE_ID, CIPHERSUITE_NAME, CoreError, self_check};

/// Version du cœur, telle que publiée par Cargo.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_est_celle_du_crate() {
        assert_eq!(version(), env!("CARGO_PKG_VERSION"));
    }
}
