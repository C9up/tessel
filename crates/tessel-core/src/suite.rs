//! Tessel's cryptographic suite and the provider that implements it (ADR-001).

use openmls::prelude::Ciphersuite;
use openmls_rust_crypto::OpenMlsRustCrypto;
use openmls_traits::OpenMlsProvider;
use openmls_traits::crypto::OpenMlsCrypto;
use openmls_traits::random::OpenMlsRand;

/// `MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519`, the mandatory suite of RFC 9420.
pub(crate) const CIPHERSUITE: Ciphersuite =
    Ciphersuite::MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519;

/// IANA identifier of the suite (RFC 9420, §17.1).
pub const CIPHERSUITE_ID: u16 = 0x0001;

/// IANA name of the suite.
pub const CIPHERSUITE_NAME: &str = "MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519";

/// What the core can reject.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CoreError {
    /// The crypto provider does not support Tessel's suite.
    #[error("the crypto provider does not support {CIPHERSUITE_NAME}")]
    UnsupportedCiphersuite,
    /// The host provides no randomness (in the browser: `crypto.getRandomValues`).
    #[error("no randomness available on this host")]
    NoRandomness,
}

/// Tessel's provider: RustCrypto primitives, in pure Rust, on every target.
pub(crate) fn provider() -> OpenMlsRustCrypto {
    OpenMlsRustCrypto::default()
}

/// Checks, on the current host, that the core can work: the suite is supported and
/// randomness is available. To call once when the core loads.
pub fn self_check() -> Result<(), CoreError> {
    let provider = provider();
    provider
        .crypto()
        .supports(CIPHERSUITE)
        .map_err(|_| CoreError::UnsupportedCiphersuite)?;
    let a: [u8; 32] = provider
        .rand()
        .random_array()
        .map_err(|_| CoreError::NoRandomness)?;
    let b: [u8; 32] = provider
        .rand()
        .random_array()
        .map_err(|_| CoreError::NoRandomness)?;
    if a == b {
        return Err(CoreError::NoRandomness);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_suite_est_la_suite_obligatoire_de_rfc_9420() {
        assert_eq!(u16::from(CIPHERSUITE), CIPHERSUITE_ID);
        assert_eq!(format!("{CIPHERSUITE:?}"), CIPHERSUITE_NAME);
    }

    #[test]
    fn le_coeur_se_verifie_sur_l_hote() {
        assert_eq!(self_check(), Ok(()));
    }
}
