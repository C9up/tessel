//! Le client : un appareil certifié, son état MLS, ses groupes et sa réserve de
//! *KeyPackages*.
//!
//! L'état vit pour l'instant en mémoire ; la base locale chiffrée (story 02-06) le
//! rendra durable.

use std::collections::HashMap;

use openmls_rust_crypto::OpenMlsRustCrypto;

use crate::groups::{Group, GroupId};
use crate::identity::Device;
use crate::key_packages::{KeyPackageError, KeyPackagePool, KeyPackageUpdate};

/// Un appareil Tessel en fonctionnement.
#[derive(Debug)]
pub struct Client {
    pub(crate) provider: OpenMlsRustCrypto,
    pub(crate) device: Device,
    key_packages: KeyPackagePool,
    pub(crate) groups: HashMap<GroupId, Group>,
}

impl Client {
    /// Démarre un client pour un appareil certifié.
    pub fn new(device: Device) -> Self {
        Self {
            provider: OpenMlsRustCrypto::default(),
            device,
            key_packages: KeyPackagePool::default(),
            groups: HashMap::new(),
        }
    }

    /// L'appareil.
    pub fn device(&self) -> &Device {
        &self.device
    }

    /// Renouvelle la réserve de *KeyPackages* à l'instant `now` (secondes Unix) et rend
    /// ce qu'il faut publier et retirer sur le serveur. À appeler au démarrage, puis
    /// régulièrement.
    pub fn refresh_key_packages(&mut self, now: u64) -> Result<KeyPackageUpdate, KeyPackageError> {
        self.key_packages.refresh(&self.provider, &self.device, now)
    }

    #[cfg(test)]
    pub(crate) fn provider(&self) -> &OpenMlsRustCrypto {
        &self.provider
    }
}
