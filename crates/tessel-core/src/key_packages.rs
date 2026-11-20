//! *KeyPackages* (RFC 9420, §10 ; story 02-03).
//!
//! Un appareil dépose sur le serveur une réserve de *KeyPackages* à usage unique, plus
//! un *KeyPackage* de dernier recours, qui sert quand la réserve est vide. Quiconque
//! veut l'ajouter à un groupe en prend un. La réserve se renouvelle :
//! - un *KeyPackage* consommé (son secret a servi à rejoindre un groupe) est remplacé ;
//! - un *KeyPackage* proche de sa fin de validité est retiré du serveur et remplacé, puis
//!   son secret est effacé une fois la date passée (un *Welcome* peut encore être en route).
//!
//! L'heure vient de l'hôte (`now`, en secondes Unix) : le cœur ne lit pas d'horloge.

use openmls::prelude::{
    Capabilities, ExtensionType, KeyPackage, KeyPackageBundle, KeyPackageIn, KeyPackageRef,
    Lifetime, MlsMessageBodyIn, MlsMessageIn, MlsMessageOut, ProtocolVersion,
};
use openmls_rust_crypto::OpenMlsRustCrypto;
use openmls_traits::OpenMlsProvider;
use openmls_traits::storage::StorageProvider;
use tls_codec::{Deserialize as _, Serialize as _};

use crate::identity::{Device, DeviceCertificate, IdentityError};
use crate::suite::CIPHERSUITE;

/// Taille de la réserve de *KeyPackages* à usage unique.
pub const KEY_PACKAGE_POOL: usize = 100;

/// Durée de validité d'un *KeyPackage* : 12 semaines, la valeur par défaut d'OpenMLS.
pub const KEY_PACKAGE_LIFETIME: u64 = 60 * 60 * 24 * 7 * 12;

/// Un *KeyPackage* est remplacé quand il lui reste moins de 2 semaines.
pub const KEY_PACKAGE_RENEW_BEFORE: u64 = 60 * 60 * 24 * 7 * 2;

/// Tolérance sur l'horloge des autres : un *KeyPackage* est valide une heure avant sa
/// création (comme `Lifetime::new` d'OpenMLS).
const CLOCK_SKEW: u64 = 60 * 60;

/// Ce que la gestion des *KeyPackages* peut refuser.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum KeyPackageError {
    /// OpenMLS n'a pas pu créer le *KeyPackage*, ou le stockage a échoué.
    #[error("key package could not be created or stored")]
    Creation,
    /// Les octets ne sont pas un *KeyPackage* MLS.
    #[error("not an MLS key package")]
    Malformed,
    /// Le *KeyPackage* n'est pas dans la suite de Tessel (ADR-001).
    #[error("key package uses another ciphersuite")]
    WrongCiphersuite,
    /// Signature, version, durée de validité ou extensions invalides.
    #[error("key package is invalid: {0}")]
    Invalid(String),
    /// Le *credential* ne porte pas un certificat d'appareil valide.
    #[error(transparent)]
    Identity(#[from] IdentityError),
}

/// Un *KeyPackage* que l'appareil a déposé.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Published {
    reference: KeyPackageRef,
    not_after: u64,
    last_resort: bool,
    /// Retiré du serveur ; son secret attend la fin de validité pour être effacé.
    withdrawn: bool,
}

/// Ce que l'appareil doit envoyer à l'annuaire (story 03-03) après un renouvellement.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct KeyPackageUpdate {
    /// Nouveaux *KeyPackages* à usage unique, sérialisés en `MLSMessage`.
    pub publish: Vec<Vec<u8>>,
    /// Nouveau *KeyPackage* de dernier recours, s'il change.
    pub last_resort: Option<Vec<u8>>,
    /// Références (`KeyPackageRef`) des *KeyPackages* à retirer du serveur.
    pub withdraw: Vec<Vec<u8>>,
}

impl KeyPackageUpdate {
    /// Rien à envoyer.
    pub fn is_empty(&self) -> bool {
        self.publish.is_empty() && self.last_resort.is_none() && self.withdraw.is_empty()
    }
}

/// La réserve de *KeyPackages* d'un appareil.
#[derive(Debug, Default)]
pub(crate) struct KeyPackagePool {
    published: Vec<Published>,
}

impl KeyPackagePool {
    /// Remet la réserve en état à l'instant `now` : remplace ce qui est consommé ou
    /// proche de sa fin, efface les secrets expirés, et rend ce qu'il faut envoyer.
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

/// Ce que la feuille d'un appareil Tessel déclare savoir faire : la seule suite de
/// Tessel, et l'extension *last_resort* qu'un *KeyPackage* de dernier recours porte.
pub(crate) fn capabilities() -> Capabilities {
    Capabilities::builder()
        .ciphersuites(vec![CIPHERSUITE])
        .extensions(vec![ExtensionType::LastResort])
        .build()
}

/// Un *KeyPackage* reçu et vérifié : signatures, durée de validité, suite de Tessel,
/// et certificat de l'appareil.
#[derive(Debug, Clone)]
pub struct VerifiedKeyPackage {
    key_package: KeyPackage,
    certificate: DeviceCertificate,
    reference: KeyPackageRef,
}

impl VerifiedKeyPackage {
    /// L'appareil qui l'a publié. Sa clé de compte reste à comparer à celle que l'on
    /// connaît du compte (voir [`DeviceCertificate`]).
    pub fn certificate(&self) -> &DeviceCertificate {
        &self.certificate
    }

    /// Sa référence (`KeyPackageRef`).
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

    /// Un *KeyPackage* de dernier recours.
    pub fn is_last_resort(&self) -> bool {
        self.key_package.last_resort()
    }
}

/// Lit et vérifie un *KeyPackage* sérialisé en `MLSMessage`.
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
