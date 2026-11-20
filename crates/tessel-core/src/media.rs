//! Media encryption (story 02-08).
//!
//! Every file is encrypted on the device with its own random key, before it is uploaded.
//! The key and the digest of the encrypted blob only travel inside the MLS message
//! ([`MediaRef`], story 02-05): the server stores a blob it cannot read.
//!
//! The blob is encrypted in chunks, so that a large file (a video) never has to sit
//! whole in memory, with the STREAM construction (Hoang, Reyhanitabar, Rogaway, Vizar,
//! "Online Authenticated-Encryption and its Nonce-Reuse Misuse-Resistance", 2015) over
//! ChaCha20-Poly1305, as implemented by RustCrypto (`aead::stream`, big-endian 32-bit
//! counter). STREAM authenticates every chunk and its position, and flags the last
//! one: a chunk altered, removed, moved or added, or a blob cut short, fails.
//!
//! ```text
//! u8          format (MEDIA_FORMAT)
//! [u8; 7]     nonce prefix, random
//! chunks      MEDIA_CHUNK bytes of the file each, the last one shorter (or empty),
//!             each followed by its 16-byte tag
//! ```
//!
//! The header (format and nonce prefix) is the associated data of every chunk.
//!
//! Decryption hands out each chunk once its tag is verified: what comes out is what the
//! sender encrypted, from the start of the file. The file is only complete, and only
//! matches the digest and the size the message announced, once
//! [`MediaDecryptor::finish`] succeeds; on any error, everything already handed out is
//! to be discarded.

use chacha20poly1305::aead::stream::{DecryptorBE32, EncryptorBE32};
use chacha20poly1305::aead::{KeyInit as _, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Key};
use openmls_rust_crypto::RustCrypto;
use openmls_traits::random::OpenMlsRand as _;
use sha2::{Digest as _, Sha256};
use zeroize::Zeroizing;

use crate::messages::{MediaKey, MediaRef};

/// Version of the blob format.
pub const MEDIA_FORMAT: u8 = 1;

/// Size of a chunk of the file, in bytes (64 KiB).
pub const MEDIA_CHUNK: usize = 64 * 1024;

const PREFIX_LEN: usize = 7;
const HEADER_LEN: usize = 1 + PREFIX_LEN;
const TAG_LEN: usize = 16;

/// What media encryption can reject.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum MediaError {
    /// No randomness available to draw the key.
    #[error("no randomness")]
    NoRandomness,
    /// The blob is in a format this version does not know (written by a newer Tessel).
    #[error("unsupported media format {0}")]
    UnsupportedFormat(u8),
    /// The blob ends too early.
    #[error("truncated media")]
    Truncated,
    /// The blob was modified, or does not match the digest or the size announced.
    #[error("altered media")]
    Altered,
    /// The file exceeds what the format can count (2³² chunks).
    #[error("media too large")]
    TooLarge,
}

/// Size of the encrypted blob of a `size`-byte file.
pub fn encrypted_len(size: u64) -> u64 {
    let chunk = MEDIA_CHUNK as u64;
    // The last chunk is never empty unless the file is.
    let chunks = size.div_ceil(chunk).max(1);
    HEADER_LEN as u64 + size + chunks * TAG_LEN as u64
}

fn cipher(key: &MediaKey) -> ChaCha20Poly1305 {
    ChaCha20Poly1305::new(Key::from_slice(key.as_bytes()))
}

/// A file being encrypted, chunk by chunk.
pub struct MediaEncryptor {
    key: MediaKey,
    header: [u8; HEADER_LEN],
    stream: EncryptorBE32<ChaCha20Poly1305>,
    /// The file's bytes not yet encrypted: the last chunk is only known at the end.
    pending: Zeroizing<Vec<u8>>,
    header_written: bool,
    digest: Sha256,
    size: u64,
}

impl std::fmt::Debug for MediaEncryptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MediaEncryptor")
            .field("size", &self.size)
            .finish_non_exhaustive()
    }
}

/// An encrypted file, ready to be referenced in a message once its blob is uploaded.
/// Its key cannot be read: it only leaves inside a [`MediaRef`].
#[derive(Debug)]
pub struct EncryptedMedia {
    key: MediaKey,
    digest: [u8; 32],
    size: u64,
}

impl EncryptedMedia {
    /// The SHA-256 digest of the encrypted blob.
    pub fn digest(&self) -> &[u8; 32] {
        &self.digest
    }

    /// The size of the file, in bytes.
    pub fn size(&self) -> u64 {
        self.size
    }

    /// The reference to put in the message, once the server stored the blob under
    /// `blob`.
    pub fn into_ref(self, blob: Vec<u8>, content_type: String, name: String) -> MediaRef {
        MediaRef {
            blob,
            key: self.key,
            digest: self.digest,
            size: self.size,
            content_type,
            name,
        }
    }
}

impl MediaEncryptor {
    /// Starts encrypting a file, with a new random key.
    pub fn new() -> Result<Self, MediaError> {
        let random = RustCrypto::default();
        let key = MediaKey::from_bytes(
            random
                .random_array()
                .map_err(|_| MediaError::NoRandomness)?,
        );
        let prefix: [u8; PREFIX_LEN] = random
            .random_array()
            .map_err(|_| MediaError::NoRandomness)?;
        let mut header = [0; HEADER_LEN];
        header[0] = MEDIA_FORMAT;
        header[1..].copy_from_slice(&prefix);
        Ok(Self {
            stream: EncryptorBE32::from_aead(cipher(&key), (&prefix).into()),
            key,
            header,
            pending: Zeroizing::new(Vec::new()),
            header_written: false,
            digest: Sha256::new(),
            size: 0,
        })
    }

    /// Feeds the next bytes of the file; returns the next bytes of the blob (possibly
    /// none, until a whole chunk is there).
    pub fn update(&mut self, data: &[u8]) -> Result<Vec<u8>, MediaError> {
        self.pending.extend_from_slice(data);
        self.size = self
            .size
            .checked_add(data.len() as u64)
            .ok_or(MediaError::TooLarge)?;
        let mut out = self.start();
        // A full chunk is only encrypted once more bytes follow: otherwise it may be
        // the last one.
        let mut done = 0;
        while self.pending.len() - done > MEDIA_CHUNK {
            let chunk = &self.pending[done..done + MEDIA_CHUNK];
            let sealed = self
                .stream
                .encrypt_next(Payload {
                    msg: chunk,
                    aad: &self.header,
                })
                .map_err(|_| MediaError::TooLarge)?;
            out.extend_from_slice(&sealed);
            done += MEDIA_CHUNK;
        }
        self.pending.drain(..done);
        self.digest.update(&out);
        Ok(out)
    }

    /// Encrypts the end of the file; returns the last bytes of the blob, and what the
    /// message needs.
    pub fn finish(mut self) -> Result<(Vec<u8>, EncryptedMedia), MediaError> {
        let mut out = self.start();
        let sealed = self
            .stream
            .encrypt_last(Payload {
                msg: &self.pending,
                aad: &self.header,
            })
            .map_err(|_| MediaError::TooLarge)?;
        out.extend_from_slice(&sealed);
        self.digest.update(&out);
        let media = EncryptedMedia {
            key: self.key,
            digest: self.digest.finalize().into(),
            size: self.size,
        };
        Ok((out, media))
    }

    /// The header, the first time some output leaves.
    fn start(&mut self) -> Vec<u8> {
        if self.header_written {
            return Vec::new();
        }
        self.header_written = true;
        self.header.to_vec()
    }
}

/// A blob being decrypted, chunk by chunk.
pub struct MediaDecryptor {
    key: MediaKey,
    expected_digest: [u8; 32],
    expected_size: u64,
    header: Option<[u8; HEADER_LEN]>,
    stream: Option<DecryptorBE32<ChaCha20Poly1305>>,
    /// The blob's bytes not yet decrypted: the last chunk is only known at the end.
    pending: Vec<u8>,
    digest: Sha256,
    size: u64,
}

impl std::fmt::Debug for MediaDecryptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MediaDecryptor")
            .field("size", &self.size)
            .finish_non_exhaustive()
    }
}

impl MediaDecryptor {
    /// Starts decrypting the blob of a media received in a message.
    pub fn new(media: &MediaRef) -> Self {
        Self {
            key: media.key.clone(),
            expected_digest: media.digest,
            expected_size: media.size,
            header: None,
            stream: None,
            pending: Vec::new(),
            digest: Sha256::new(),
            size: 0,
        }
    }

    /// Feeds the next bytes of the blob; returns the next verified bytes of the file
    /// (possibly none, until a whole chunk is there).
    pub fn update(&mut self, data: &[u8]) -> Result<Vec<u8>, MediaError> {
        self.digest.update(data);
        self.pending.extend_from_slice(data);
        self.read_header()?;
        let (Some(header), Some(stream)) = (&self.header, &mut self.stream) else {
            return Ok(Vec::new());
        };
        let mut out = Vec::new();
        let mut done = 0;
        let sealed_chunk = MEDIA_CHUNK + TAG_LEN;
        while self.pending.len() - done > sealed_chunk {
            let chunk = stream
                .decrypt_next(Payload {
                    msg: &self.pending[done..done + sealed_chunk],
                    aad: header,
                })
                .map_err(|_| MediaError::Altered)?;
            out.extend_from_slice(&chunk);
            done += sealed_chunk;
        }
        self.pending.drain(..done);
        self.size += out.len() as u64;
        if self.size > self.expected_size {
            return Err(MediaError::Altered);
        }
        Ok(out)
    }

    /// Decrypts the end of the blob and checks it whole: digest and size. Returns the
    /// last bytes of the file.
    pub fn finish(mut self) -> Result<Vec<u8>, MediaError> {
        self.read_header()?;
        let (Some(header), Some(stream)) = (self.header, self.stream.take()) else {
            return Err(MediaError::Truncated);
        };
        if self.pending.len() < TAG_LEN {
            return Err(MediaError::Truncated);
        }
        let last = stream
            .decrypt_last(Payload {
                msg: &self.pending,
                aad: &header,
            })
            .map_err(|_| MediaError::Altered)?;
        let digest: [u8; 32] = self.digest.finalize().into();
        if digest != self.expected_digest || self.size + last.len() as u64 != self.expected_size {
            return Err(MediaError::Altered);
        }
        Ok(last)
    }

    /// Reads the header once it is there.
    fn read_header(&mut self) -> Result<(), MediaError> {
        if self.header.is_none() && self.pending.len() >= HEADER_LEN {
            if self.pending[0] != MEDIA_FORMAT {
                return Err(MediaError::UnsupportedFormat(self.pending[0]));
            }
            let mut header = [0; HEADER_LEN];
            header.copy_from_slice(&self.pending[..HEADER_LEN]);
            let prefix: [u8; PREFIX_LEN] =
                header[1..].try_into().map_err(|_| MediaError::Truncated)?;
            self.stream = Some(DecryptorBE32::from_aead(
                cipher(&self.key),
                (&prefix).into(),
            ));
            self.header = Some(header);
            self.pending.drain(..HEADER_LEN);
        }
        Ok(())
    }
}

/// Encrypts a whole file at once.
pub fn encrypt_media(file: &[u8]) -> Result<(Vec<u8>, EncryptedMedia), MediaError> {
    let mut encryptor = MediaEncryptor::new()?;
    let mut blob = encryptor.update(file)?;
    let (end, media) = encryptor.finish()?;
    blob.extend_from_slice(&end);
    Ok((blob, media))
}

/// Decrypts a whole blob at once, and checks it.
pub fn decrypt_media(media: &MediaRef, blob: &[u8]) -> Result<Vec<u8>, MediaError> {
    let mut decryptor = MediaDecryptor::new(media);
    let mut file = decryptor.update(blob)?;
    file.extend_from_slice(&decryptor.finish()?);
    Ok(file)
}

#[cfg(test)]
mod tests;
