//! The client: a certified device, its MLS state, its groups and its pool of
//! *KeyPackages*.
//!
//! For now the state lives in memory; the encrypted local database (story 02-06)
//! will make it durable.

use std::collections::HashMap;

use openmls_rust_crypto::OpenMlsRustCrypto;

use crate::groups::{Group, GroupId};
use crate::identity::Device;
use crate::key_packages::{KeyPackageError, KeyPackagePool, KeyPackageUpdate};

/// A running Tessel device.
#[derive(Debug)]
pub struct Client {
    pub(crate) provider: OpenMlsRustCrypto,
    pub(crate) device: Device,
    key_packages: KeyPackagePool,
    pub(crate) groups: HashMap<GroupId, Group>,
}

impl Client {
    /// Starts a client for a certified device.
    pub fn new(device: Device) -> Self {
        Self {
            provider: OpenMlsRustCrypto::default(),
            device,
            key_packages: KeyPackagePool::default(),
            groups: HashMap::new(),
        }
    }

    /// The device.
    pub fn device(&self) -> &Device {
        &self.device
    }

    /// Renews the *KeyPackage* pool at time `now` (Unix seconds) and returns what must
    /// be published and withdrawn on the server. To call at startup, then
    /// periodically.
    pub fn refresh_key_packages(&mut self, now: u64) -> Result<KeyPackageUpdate, KeyPackageError> {
        self.key_packages.refresh(&self.provider, &self.device, now)
    }

    #[cfg(test)]
    pub(crate) fn provider(&self) -> &OpenMlsRustCrypto {
        &self.provider
    }
}
