//! Encrypted profiles (story 03-06).
//!
//! An account's display name and avatar are sealed under its **profile key**, a 256-bit
//! key drawn when the account is created. The server keeps the sealed blob only. The
//! key travels to the account's other devices in the link grant (story 03-04), and to
//! its contacts in their MLS groups ([`crate::Body::ProfileKey`]): a contact who has the
//! key opens the blob the server hands out.
//!
//! The blob is bound to its account (associated data): the server cannot hand Bob's
//! profile out as Alice's. Its plaintext is padded like a message, so its size says
//! little about the name.

use std::collections::HashMap;
use std::str::FromStr as _;

use chacha20poly1305::aead::{Aead as _, KeyInit as _, Payload};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use openmls_rust_crypto::RustCrypto;
use openmls_traits::crypto::OpenMlsCrypto;
use openmls_traits::random::OpenMlsRand;
use openmls_traits::types::HashType;
use zeroize::Zeroizing;

use crate::client::Client;
use crate::codec::{CodecError, Reader, Writer};
use crate::identity::AccountId;
use crate::messages::padded_len;
use crate::store::{Changes, PartitionId, Sealer, StoreError};

/// Version of a sealed profile.
pub const PROFILE_VERSION: u8 = 1;

/// Longest display name, in bytes of UTF-8.
pub const MAX_DISPLAY_NAME: usize = 128;

/// Largest avatar: a small picture, carried in the profile itself.
pub const MAX_AVATAR: usize = 64 * 1024;

/// Longest avatar media type.
const MAX_AVATAR_TYPE: usize = 64;

/// HKDF label of the key a profile is sealed under.
const PROFILE_LABEL: &[u8] = b"Tessel 1.0 profile";

/// Label of a contact's profile key, in the state partition.
pub(crate) const PROFILE_KEYS: &[u8] = b"profile-key";

const NONCE_LEN: usize = 24;

/// An account's profile key. Never shown: the core hands it to the account's devices and
/// contacts, sealed, and keeps it in its encrypted database.
#[derive(Clone, PartialEq, Eq)]
pub struct ProfileKey(Zeroizing<[u8; 32]>);

impl std::fmt::Debug for ProfileKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ProfileKey(..)")
    }
}

impl ProfileKey {
    pub(crate) fn generate() -> Result<Self, ProfileError> {
        Ok(Self(Zeroizing::new(
            RustCrypto::default()
                .random_array()
                .map_err(|_| ProfileError::Crypto)?,
        )))
    }

    pub(crate) fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(Zeroizing::new(bytes))
    }

    pub(crate) fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    fn cipher(&self) -> Result<XChaCha20Poly1305, ProfileError> {
        let crypto = RustCrypto::default();
        let prk = crypto
            .hkdf_extract(HashType::Sha2_256, &[], self.0.as_slice())
            .map_err(|_| ProfileError::Crypto)?;
        let okm = crypto
            .hkdf_expand(HashType::Sha2_256, prk.as_slice(), PROFILE_LABEL, 32)
            .map_err(|_| ProfileError::Crypto)?;
        Ok(XChaCha20Poly1305::new(Key::from_slice(okm.as_slice())))
    }
}

/// A profile's picture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Avatar {
    /// Its media type (`image/webp`, ...).
    pub content_type: String,
    /// The image, at most [`MAX_AVATAR`] bytes.
    pub image: Vec<u8>,
}

/// What an account shows its contacts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    /// 1 to [`MAX_DISPLAY_NAME`] bytes.
    pub name: String,
    pub avatar: Option<Avatar>,
}

/// What profile handling can reject.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ProfileError {
    /// The profile breaks a limit: empty or long name, large avatar.
    #[error("invalid profile: {0}")]
    Invalid(&'static str),
    /// The blob does not open with the account's profile key: altered, of another
    /// account, or sealed under a key this device does not have.
    #[error("the profile does not open")]
    Altered,
    /// This device has no profile key for the account: the contact has not shared it.
    #[error("no profile key for this account")]
    UnknownKey,
    /// The blob is of a version this core does not know.
    #[error("unsupported profile version {0}")]
    UnsupportedVersion(u8),
    /// Randomness or key derivation failed.
    #[error("crypto provider failure")]
    Crypto,
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl From<CodecError> for ProfileError {
    fn from(_: CodecError) -> Self {
        Self::Altered
    }
}

fn aad(account: &AccountId) -> Vec<u8> {
    let mut aad = PROFILE_LABEL.to_vec();
    aad.extend_from_slice(account.to_string().as_bytes());
    aad
}

impl Profile {
    fn validate(&self) -> Result<(), ProfileError> {
        if self.name.is_empty() || self.name.len() > MAX_DISPLAY_NAME {
            return Err(ProfileError::Invalid("display name"));
        }
        if let Some(avatar) = &self.avatar {
            if avatar.image.is_empty() || avatar.image.len() > MAX_AVATAR {
                return Err(ProfileError::Invalid("avatar size"));
            }
            if avatar.content_type.is_empty() || avatar.content_type.len() > MAX_AVATAR_TYPE {
                return Err(ProfileError::Invalid("avatar type"));
            }
        }
        Ok(())
    }

    fn encode(&self) -> Result<Zeroizing<Vec<u8>>, ProfileError> {
        self.validate()?;
        let mut out = Writer::default();
        out.vec8(self.name.as_bytes())
            .map_err(|_| ProfileError::Invalid("display name"))?;
        match &self.avatar {
            None => out.u8(0),
            Some(avatar) => {
                out.u8(1);
                out.vec8(avatar.content_type.as_bytes())
                    .map_err(|_| ProfileError::Invalid("avatar type"))?;
                out.vec32(&avatar.image)
                    .map_err(|_| ProfileError::Invalid("avatar size"))?;
            }
        }
        let mut bytes = Zeroizing::new(out.0);
        let padded = padded_len(bytes.len());
        bytes.resize(padded, 0);
        Ok(bytes)
    }

    fn decode(bytes: &[u8]) -> Result<Self, ProfileError> {
        let mut input = Reader(bytes);
        let name = input.string8()?;
        let avatar = match input.u8()? {
            0 => None,
            1 => Some(Avatar {
                content_type: input.string8()?,
                image: input.vec32()?.to_vec(),
            }),
            _ => return Err(ProfileError::Altered),
        };
        // The padding: zeros only, up to the content's bucket.
        let content = bytes.len() - input.0.len();
        if bytes.len() != padded_len(content) || input.0.iter().any(|&b| b != 0) {
            return Err(ProfileError::Altered);
        }
        let profile = Self { name, avatar };
        profile.validate().map_err(|_| ProfileError::Altered)?;
        Ok(profile)
    }
}

/// Seals `profile` of `account` under `key`: the blob the server keeps.
pub(crate) fn seal_profile(
    key: &ProfileKey,
    account: &AccountId,
    profile: &Profile,
) -> Result<Vec<u8>, ProfileError> {
    let plaintext = profile.encode()?;
    let nonce: [u8; NONCE_LEN] = RustCrypto::default()
        .random_array()
        .map_err(|_| ProfileError::Crypto)?;
    let sealed = key
        .cipher()?
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: &plaintext,
                aad: &aad(account),
            },
        )
        .map_err(|_| ProfileError::Crypto)?;
    let mut out = Vec::with_capacity(1 + NONCE_LEN + sealed.len());
    out.push(PROFILE_VERSION);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&sealed);
    Ok(out)
}

/// Opens a blob sealed by [`seal_profile`] for `account`.
pub(crate) fn open_profile(
    key: &ProfileKey,
    account: &AccountId,
    blob: &[u8],
) -> Result<Profile, ProfileError> {
    let (&version, rest) = blob.split_first().ok_or(ProfileError::Altered)?;
    if version != PROFILE_VERSION {
        return Err(ProfileError::UnsupportedVersion(version));
    }
    if rest.len() < NONCE_LEN {
        return Err(ProfileError::Altered);
    }
    let (nonce, ciphertext) = rest.split_at(NONCE_LEN);
    let plaintext = Zeroizing::new(
        key.cipher()?
            .decrypt(
                XNonce::from_slice(nonce),
                Payload {
                    msg: ciphertext,
                    aad: &aad(account),
                },
            )
            .map_err(|_| ProfileError::Altered)?,
    );
    Profile::decode(&plaintext)
}

/// The profile keys contacts shared, and the records they still have to hand over.
#[derive(Debug, Default)]
pub(crate) struct ProfileKeys {
    keys: HashMap<AccountId, ProfileKey>,
    pending: Changes,
}

impl ProfileKeys {
    fn get(&self, account: &AccountId) -> Option<&ProfileKey> {
        self.keys.get(account)
    }

    /// Keeps `key` for `account`; whether it is new or changed.
    fn put(
        &mut self,
        sealer: &Sealer,
        state: PartitionId,
        account: &AccountId,
        key: ProfileKey,
    ) -> Result<bool, StoreError> {
        if self.keys.get(account) == Some(&key) {
            return Ok(false);
        }
        let mut body = Writer::default();
        body.vec16(account.to_string().as_bytes())?;
        body.bytes(key.as_bytes());
        self.pending.put.push(sealer.seal(
            state,
            PROFILE_KEYS,
            account.to_string().as_bytes(),
            &body.0,
        )?);
        self.keys.insert(account.clone(), key);
        Ok(true)
    }

    /// The records waiting to be persisted.
    pub(crate) fn take(&mut self) -> Changes {
        std::mem::take(&mut self.pending)
    }

    /// Reloads a contact's profile key from the state, at startup.
    pub(crate) fn load(&mut self, body: &[u8]) -> Result<(), StoreError> {
        let mut input = Reader(body);
        let account = std::str::from_utf8(input.vec16()?).map_err(|_| StoreError::Malformed)?;
        let account = AccountId::from_str(account).map_err(|_| StoreError::Malformed)?;
        let key = ProfileKey::from_bytes(input.array()?);
        input.finish()?;
        self.keys.insert(account, key);
        Ok(())
    }
}

impl Client {
    /// Seals this account's profile under its profile key: the blob to hand the server.
    /// Then [`Client::share_profile`] in each group tells the contacts.
    pub fn seal_profile(&self, profile: &Profile) -> Result<Vec<u8>, ProfileError> {
        seal_profile(
            self.profile_key(),
            self.device.certificate().account(),
            profile,
        )
    }

    /// Opens the profile blob of `account`, with the key its devices shared (or this
    /// account's own key).
    pub fn open_profile(&self, account: &AccountId, blob: &[u8]) -> Result<Profile, ProfileError> {
        let key = if account == self.device.certificate().account() {
            self.profile_key()
        } else {
            self.profile_keys
                .get(account)
                .ok_or(ProfileError::UnknownKey)?
        };
        open_profile(key, account, blob)
    }

    /// Whether this device holds a profile key for `account`.
    pub fn knows_profile(&self, account: &AccountId) -> bool {
        account == self.device.certificate().account() || self.profile_keys.get(account).is_some()
    }

    /// A contact's device shared its account's profile key in a group: kept, and
    /// whether it is new or changed (the app then fetches the profile again).
    pub(crate) fn learn_profile_key(
        &mut self,
        account: &AccountId,
        key: &ProfileKey,
    ) -> Result<bool, StoreError> {
        if account == self.device.certificate().account() {
            return Ok(false);
        }
        let (keys, sealer, state) = self.profile_keys_mut();
        keys.put(sealer, state, account, key.clone())
    }
}

#[cfg(test)]
mod tests;
