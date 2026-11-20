//! La suite cryptographique de Tessel et le fournisseur qui l'implémente (ADR-001).

use openmls::prelude::Ciphersuite;
use openmls_rust_crypto::OpenMlsRustCrypto;
use openmls_traits::OpenMlsProvider;
use openmls_traits::crypto::OpenMlsCrypto;
use openmls_traits::random::OpenMlsRand;

/// `MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519`, la suite obligatoire de RFC 9420.
pub(crate) const CIPHERSUITE: Ciphersuite =
    Ciphersuite::MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519;

/// Identifiant IANA de la suite (RFC 9420, §17.1).
pub const CIPHERSUITE_ID: u16 = 0x0001;

/// Nom IANA de la suite.
pub const CIPHERSUITE_NAME: &str = "MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519";

/// Ce que le cœur peut refuser.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CoreError {
    /// Le fournisseur crypto ne prend pas en charge la suite de Tessel.
    #[error("the crypto provider does not support {CIPHERSUITE_NAME}")]
    UnsupportedCiphersuite,
    /// L'hôte ne fournit pas d'aléa (dans le navigateur : `crypto.getRandomValues`).
    #[error("no randomness available on this host")]
    NoRandomness,
}

/// Le fournisseur de Tessel : primitives RustCrypto, en Rust pur, sur toutes les cibles.
pub(crate) fn provider() -> OpenMlsRustCrypto {
    OpenMlsRustCrypto::default()
}

/// Vérifie, sur l'hôte courant, que le cœur peut travailler : la suite est prise en
/// charge et l'aléa est disponible. À appeler une fois au chargement du cœur.
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
