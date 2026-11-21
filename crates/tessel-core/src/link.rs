//! Adding a device by QR code (story 03-04).
//!
//! 1. The new device draws its key, its identifier, a link identifier and a 256-bit
//!    secret, and shows them in a QR code ([`LinkRequest::qr`]). The private key never
//!    leaves it; the QR carries the public key only.
//! 2. A device of the account that holds the account key scans it
//!    ([`Client::grant_link`]): it certifies the new device's key and seals the
//!    certificate, with the account's profile key (story 03-06), under a key derived
//!    from the secret. The server relays the sealed grant
//!    under the link identifier, a mailbox it cannot read.
//! 3. The new device opens the grant ([`LinkRequest::accept`]): it checks that the
//!    certificate is valid and certifies its own key and identifier, then starts as a
//!    device of the account. It shows the account it joined, for the person to confirm:
//!    whoever filmed the QR could race the real device with a grant of their own account.
//!
//! Joining the account's groups then goes the MLS way: the granting device takes one of
//! the new device's KeyPackages per group from the directory (story 03-03) and adds it
//! ([`Client::add_devices`]); the new device joins with each *Welcome*.

use chacha20poly1305::aead::{Aead as _, KeyInit as _, Payload};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use openmls_rust_crypto::RustCrypto;
use openmls_traits::crypto::OpenMlsCrypto;
use openmls_traits::random::OpenMlsRand;
use openmls_traits::types::HashType;
use zeroize::Zeroizing;

use crate::client::Client;
use crate::codec::{Reader, Writer};
use crate::groups::GroupId;
use crate::identity::{Device, DeviceCertificate, DeviceId, DeviceKey, IdentityError};
use crate::profile::ProfileKey;
use crate::store::{StoreError, StoreKey};

/// Version of the QR code's content.
pub const LINK_QR_VERSION: u8 = 1;

/// Size of the QR code's content: version, link, secret, device identifier, device key.
pub const LINK_QR_LEN: usize = 1 + LINK_ID_LEN + SECRET_LEN + 16 + 32;

/// Size of a link identifier: the server's mailbox for the grant.
pub const LINK_ID_LEN: usize = 16;

const SECRET_LEN: usize = 32;
const NONCE_LEN: usize = 24;

/// HKDF label of the grant's key: one key per direction and per purpose.
const GRANT_LABEL: &[u8] = b"Tessel 1.0 DeviceLink grant";

/// What linking a device can fail on.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum LinkError {
    /// The QR code is not a Tessel link, or of an unknown version.
    #[error("not a device link QR code")]
    MalformedQr,
    /// The grant does not open with this link's secret: altered, or for another link.
    #[error("the grant does not open with this link")]
    BadGrant,
    /// The grant's certificate is invalid, or certifies another key or device.
    #[error("the grant does not certify this device")]
    WrongCertificate,
    /// This device does not hold the account key: only the one that does certifies.
    #[error("this device does not hold the account key")]
    NoAccountKey,
    /// Randomness or key derivation failed.
    #[error("crypto provider failure")]
    Crypto,
    #[error(transparent)]
    Identity(#[from] IdentityError),
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// The key a link's grant is sealed under.
fn grant_key(
    link: &[u8; LINK_ID_LEN],
    secret: &[u8; SECRET_LEN],
) -> Result<Zeroizing<[u8; 32]>, LinkError> {
    let crypto = RustCrypto::default();
    let prk = crypto
        .hkdf_extract(HashType::Sha2_256, link, secret)
        .map_err(|_| LinkError::Crypto)?;
    let okm = crypto
        .hkdf_expand(HashType::Sha2_256, prk.as_slice(), GRANT_LABEL, 32)
        .map_err(|_| LinkError::Crypto)?;
    let key: [u8; 32] = okm.as_slice().try_into().map_err(|_| LinkError::Crypto)?;
    Ok(Zeroizing::new(key))
}

/// The new device, waiting to be certified.
pub struct LinkRequest {
    key: DeviceKey,
    device_id: DeviceId,
    link: [u8; LINK_ID_LEN],
    secret: Zeroizing<[u8; SECRET_LEN]>,
}

impl std::fmt::Debug for LinkRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LinkRequest")
            .field("device_id", &self.device_id)
            .finish_non_exhaustive()
    }
}

impl LinkRequest {
    /// A new device's key and identifier, and a fresh link.
    pub fn new() -> Result<Self, LinkError> {
        let rand = RustCrypto::default();
        Ok(Self {
            key: DeviceKey::generate()?,
            device_id: DeviceId::random()?,
            link: rand.random_array().map_err(|_| LinkError::Crypto)?,
            secret: Zeroizing::new(rand.random_array().map_err(|_| LinkError::Crypto)?),
        })
    }

    /// What the QR code shows. It holds the link's secret: shown on screen, never sent.
    pub fn qr(&self) -> Zeroizing<Vec<u8>> {
        let mut out = Zeroizing::new(Vec::with_capacity(LINK_QR_LEN));
        out.push(LINK_QR_VERSION);
        out.extend_from_slice(&self.link);
        out.extend_from_slice(self.secret.as_slice());
        out.extend_from_slice(self.device_id.as_bytes());
        out.extend_from_slice(self.key.public_key());
        out
    }

    /// The server's mailbox where the grant arrives.
    pub fn link(&self) -> [u8; LINK_ID_LEN] {
        self.link
    }

    /// Opens the grant and starts the new device's client, with a new store key. The
    /// certificate must be valid and certify this very key and identifier.
    pub fn accept(self, grant: &[u8], store_key: &StoreKey) -> Result<Client, LinkError> {
        let opened = Zeroizing::new(open(
            &*grant_key(&self.link, &self.secret)?,
            &self.link,
            grant,
        )?);
        // The certificate, then the account's profile key (story 03-06).
        let mut input = Reader(&opened);
        let certificate = input.vec16().map_err(|_| LinkError::BadGrant)?;
        let profile_key = ProfileKey::from_bytes(input.array().map_err(|_| LinkError::BadGrant)?);
        input.finish().map_err(|_| LinkError::BadGrant)?;
        let certificate =
            DeviceCertificate::from_bytes(certificate).map_err(|_| LinkError::WrongCertificate)?;
        if certificate.device_id() != self.device_id {
            return Err(LinkError::WrongCertificate);
        }
        let device = Device::new(self.key, certificate).map_err(|_| LinkError::WrongCertificate)?;
        let mut client = Client::new(device, store_key)?;
        client.set_profile_key(profile_key);
        Ok(client)
    }
}

/// A scanned link: what the granting device reads from the QR code.
struct ScannedLink {
    link: [u8; LINK_ID_LEN],
    secret: Zeroizing<[u8; SECRET_LEN]>,
    device_id: DeviceId,
    device_key: [u8; 32],
}

fn array<const N: usize>(bytes: &[u8]) -> Result<[u8; N], LinkError> {
    bytes.try_into().map_err(|_| LinkError::MalformedQr)
}

fn scan(qr: &[u8]) -> Result<ScannedLink, LinkError> {
    if qr.len() != LINK_QR_LEN || qr[0] != LINK_QR_VERSION {
        return Err(LinkError::MalformedQr);
    }
    let (link, rest) = qr[1..].split_at(LINK_ID_LEN);
    let (secret, rest) = rest.split_at(SECRET_LEN);
    let (device_id, device_key) = rest.split_at(16);
    Ok(ScannedLink {
        link: array(link)?,
        secret: Zeroizing::new(array(secret)?),
        device_id: DeviceId::from_bytes(array(device_id)?),
        device_key: array(device_key)?,
    })
}

/// What the granting device hands the server: the new device's certificate, to register
/// it, and the sealed grant, for its mailbox.
#[derive(Debug)]
pub struct LinkGrant {
    pub link: [u8; LINK_ID_LEN],
    pub certificate: DeviceCertificate,
    pub sealed: Vec<u8>,
}

fn seal(key: &[u8; 32], link: &[u8; LINK_ID_LEN], plaintext: &[u8]) -> Result<Vec<u8>, LinkError> {
    let nonce: [u8; NONCE_LEN] = RustCrypto::default()
        .random_array()
        .map_err(|_| LinkError::Crypto)?;
    let sealed = XChaCha20Poly1305::new(Key::from_slice(key))
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: plaintext,
                aad: link,
            },
        )
        .map_err(|_| LinkError::Crypto)?;
    let mut out = Vec::with_capacity(NONCE_LEN + sealed.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&sealed);
    Ok(out)
}

fn open(key: &[u8; 32], link: &[u8; LINK_ID_LEN], sealed: &[u8]) -> Result<Vec<u8>, LinkError> {
    if sealed.len() < NONCE_LEN {
        return Err(LinkError::BadGrant);
    }
    let (nonce, ciphertext) = sealed.split_at(NONCE_LEN);
    XChaCha20Poly1305::new(Key::from_slice(key))
        .decrypt(
            XNonce::from_slice(nonce),
            Payload {
                msg: ciphertext,
                aad: link,
            },
        )
        .map_err(|_| LinkError::BadGrant)
}

impl Client {
    /// The account key's revocation of another device of this account (story 03-05),
    /// for the server: only the device holding the account key can sign it. A device
    /// revokes itself without one.
    pub fn revoke_device(&self, device: DeviceId) -> Result<Vec<u8>, LinkError> {
        let account_key = self.account_key().ok_or(LinkError::NoAccountKey)?;
        Ok(account_key.revoke(self.device.certificate().account(), device)?)
    }

    /// The account key's deletion of this account (story 03-07), for the server. Before
    /// sending it, the device leaves each of its groups ([`Client::leave_group`]): once
    /// the account is gone, it can no longer reach them.
    pub fn sign_account_deletion(&self) -> Result<Vec<u8>, LinkError> {
        let account_key = self.account_key().ok_or(LinkError::NoAccountKey)?;
        Ok(account_key.delete_account(self.device.certificate().account())?)
    }

    /// The groups `device` is a member of: those to remove it from once revoked.
    pub fn groups_with(&self, device: DeviceId) -> Vec<GroupId> {
        let account = self.device.certificate().account();
        self.groups()
            .into_iter()
            .filter(|group| {
                self.members(group).is_ok_and(|members| {
                    members
                        .iter()
                        .any(|m| m.account() == account && m.device_id() == device)
                })
            })
            .collect()
    }

    /// Certifies the device whose QR code was scanned, as a device of this account, and
    /// seals its certificate for it. Only the device holding the account key can.
    pub fn grant_link(&self, qr: &[u8]) -> Result<LinkGrant, LinkError> {
        let scanned = scan(qr)?;
        let account_key = self.account_key().ok_or(LinkError::NoAccountKey)?;
        let certificate = account_key.certify(
            self.device.certificate().account(),
            scanned.device_id,
            &scanned.device_key,
        )?;
        // The certificate, then the account's profile key (story 03-06).
        let mut grant = Writer::default();
        grant
            .vec16(&certificate.to_bytes()?)
            .map_err(|_| LinkError::Crypto)?;
        grant.bytes(self.profile_key().as_bytes());
        let grant = Zeroizing::new(grant.0);
        let sealed = seal(
            &*grant_key(&scanned.link, &scanned.secret)?,
            &scanned.link,
            &grant,
        )?;
        Ok(LinkGrant {
            link: scanned.link,
            certificate,
            sealed,
        })
    }
}

#[cfg(test)]
mod tests;
