//! Account and device identity (ADR-002, story 02-02).
//!
//! An account `name@domain` has an identity key (Ed25519). Each device has its own
//! MLS signing key, and a certificate signed by the account key that binds the device
//! to the account. The certificate travels in the device's MLS *BasicCredential*:
//! any member of a group can verify it.
//!
//! What verification does not tell: that the account key really is the one of
//! `name@domain`. That link comes from the server directory (story 03-01) and from the
//! security codes (story 02-09); the caller compares `account_key` with the key it
//! knows.
//!
//! Private keys never leave this module, except sealed into the local database (story
//! 02-06): no public type returns, serializes or displays them, and they are wiped from
//! memory when dropped.

use std::fmt;
use std::str::FromStr;

use openmls::prelude::{BasicCredential, Credential, CredentialWithKey};
use openmls_rust_crypto::RustCrypto;
use openmls_traits::crypto::OpenMlsCrypto;
use openmls_traits::random::OpenMlsRand;
use openmls_traits::signatures::{Signer, SignerError};
use openmls_traits::types::SignatureScheme;
use tls_codec::{
    DeserializeBytes, Serialize as _, TlsByteVecU16, TlsDeserializeBytes, TlsSerialize, TlsSize,
    VLByteSlice,
};
use zeroize::Zeroizing;

use crate::codec::{CodecError, Reader, Writer};

/// Signature scheme of account and device keys: the one of suite 0x0001.
const SCHEME: SignatureScheme = SignatureScheme::ED25519;

/// Signature label of the certificate, modeled on `SignWithLabel` (RFC 9420,
/// §5.1.2), in a namespace specific to Tessel.
const CERTIFICATE_LABEL: &[u8] = b"Tessel 1.0 DeviceCertificate";

/// Certificate format version.
const CERTIFICATE_VERSION: u8 = 1;

/// Size of an Ed25519 public key.
const PUBLIC_KEY_LEN: usize = 32;

/// Size of an Ed25519 signature.
const SIGNATURE_LEN: usize = 64;

type PublicKey = [u8; PUBLIC_KEY_LEN];

/// What identity handling can reject.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum IdentityError {
    /// The account is not a valid `name@domain` (ADR-002).
    #[error("invalid account: {0}")]
    InvalidAccount(&'static str),
    /// The crypto provider failed (randomness, key generation or signing).
    #[error("crypto provider failure")]
    Crypto,
    /// A public key does not have the size of an Ed25519 key.
    #[error("invalid public key")]
    InvalidKey,
    /// The certificate cannot be decoded, or is in an unknown version.
    #[error("malformed device certificate")]
    MalformedCertificate,
    /// The certificate signature does not verify with the announced account key.
    #[error("device certificate signature does not verify")]
    BadSignature,
    /// The device signing key is not the one the certificate certifies.
    #[error("device key does not match its certificate")]
    KeyMismatch,
    /// The MLS *credential* is not a *BasicCredential*.
    #[error("not a basic credential")]
    NotBasicCredential,
}

/// An account: `name@domain` (ADR-002).
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct AccountId {
    name: String,
    domain: String,
}

impl AccountId {
    /// Parses and validates `name@domain`. The name: 3 to 32 characters among `a-z`,
    /// `0-9`, `.`, `_`, `-`, starting with a letter. The domain: a lowercase host
    /// name (labels `a-z`, `0-9`, `-`).
    pub fn parse(value: &str) -> Result<Self, IdentityError> {
        let (name, domain) = value
            .split_once('@')
            .ok_or(IdentityError::InvalidAccount("missing @"))?;
        validate_name(name)?;
        validate_domain(domain)?;
        Ok(Self {
            name: name.to_owned(),
            domain: domain.to_owned(),
        })
    }

    /// The name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The instance domain.
    pub fn domain(&self) -> &str {
        &self.domain
    }
}

fn validate_name(name: &str) -> Result<(), IdentityError> {
    if !(3..=32).contains(&name.len()) {
        return Err(IdentityError::InvalidAccount(
            "name must be 3 to 32 characters",
        ));
    }
    if !name.starts_with(|c: char| c.is_ascii_lowercase()) {
        return Err(IdentityError::InvalidAccount("name must start with a-z"));
    }
    if !name
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._-".contains(&b))
    {
        return Err(IdentityError::InvalidAccount(
            "name allows a-z 0-9 . _ - only",
        ));
    }
    Ok(())
}

fn validate_domain(domain: &str) -> Result<(), IdentityError> {
    const BAD: IdentityError = IdentityError::InvalidAccount("invalid domain");
    if domain.is_empty() || domain.len() > 253 {
        return Err(BAD);
    }
    for label in domain.split('.') {
        let ok = (1..=63).contains(&label.len())
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        if !ok {
            return Err(BAD);
        }
    }
    Ok(())
}

impl FromStr for AccountId {
    type Err = IdentityError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl fmt::Display for AccountId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{}", self.name, self.domain)
    }
}

impl fmt::Debug for AccountId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AccountId({self})")
    }
}

/// Device identifier: 128 random bits, unique within its account (ADR-002).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct DeviceId([u8; 16]);

impl DeviceId {
    /// Draws a random identifier.
    pub fn random() -> Result<Self, IdentityError> {
        RustCrypto::default()
            .random_array()
            .map(Self)
            .map_err(|_| IdentityError::Crypto)
    }

    /// Restores a known identifier.
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// The 16 bytes of the identifier.
    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

impl fmt::Display for DeviceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.iter().try_for_each(|b| write!(f, "{b:02x}"))
    }
}

impl fmt::Debug for DeviceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DeviceId({self})")
    }
}

/// An Ed25519 key pair whose private half never leaves the crate.
struct SigningKey {
    private: Zeroizing<Vec<u8>>,
    public: Vec<u8>,
}

impl SigningKey {
    fn generate() -> Result<Self, IdentityError> {
        let (private, public) = RustCrypto::default()
            .signature_key_gen(SCHEME)
            .map_err(|_| IdentityError::Crypto)?;
        Ok(Self {
            private: Zeroizing::new(private),
            public,
        })
    }

    fn sign(&self, data: &[u8]) -> Result<Vec<u8>, IdentityError> {
        RustCrypto::default()
            .sign(SCHEME, data, &self.private)
            .map_err(|_| IdentityError::Crypto)
    }
}

impl fmt::Debug for SigningKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SigningKey")
            .field("public", &self.public)
            .finish_non_exhaustive()
    }
}

/// The identity key of an account. It signs the certificates of its devices.
#[derive(Debug)]
pub struct AccountKey(SigningKey);

impl AccountKey {
    /// Creates a new account key.
    pub fn generate() -> Result<Self, IdentityError> {
        SigningKey::generate().map(Self)
    }

    /// The public key, which the directory publishes and the security codes compare.
    pub fn public_key(&self) -> &[u8] {
        &self.0.public
    }

    /// Certifies that a device belongs to the account. `device_key` is the device's
    /// public signing key (on the new device: [`DeviceKey::public_key`]).
    pub fn certify(
        &self,
        account: &AccountId,
        device_id: DeviceId,
        device_key: &[u8],
    ) -> Result<DeviceCertificate, IdentityError> {
        let mut certificate = DeviceCertificate {
            account: account.clone(),
            device_id,
            account_key: public_key(&self.0.public)?,
            device_key: public_key(device_key)?,
            signature: [0; SIGNATURE_LEN],
        };
        let signature = self.0.sign(&certificate.to_be_signed()?)?;
        certificate.signature = signature.try_into().map_err(|_| IdentityError::Crypto)?;
        Ok(certificate)
    }

    /// Writes the key, private half included, into the client record. Only the store
    /// calls this, and it seals the record before it leaves the core.
    pub(crate) fn write_to(&self, out: &mut Writer) -> Result<(), CodecError> {
        out.vec8(&self.0.private)?;
        out.vec8(&self.0.public)
    }

    /// Reads back a key written by [`AccountKey::write_to`].
    pub(crate) fn read_from(input: &mut Reader<'_>) -> Result<Self, CodecError> {
        let private = Zeroizing::new(input.vec8()?.to_vec());
        let public = input.vec8()?.to_vec();
        Ok(Self(SigningKey { private, public }))
    }
}

/// The MLS signing key of a device, before it is certified.
#[derive(Debug)]
pub struct DeviceKey(SigningKey);

impl DeviceKey {
    /// Creates a new device key.
    pub fn generate() -> Result<Self, IdentityError> {
        SigningKey::generate().map(Self)
    }

    /// The public key, to be certified by the account key.
    pub fn public_key(&self) -> &[u8] {
        &self.0.public
    }
}

/// The signed content of a certificate. Keys and signature have a fixed size; only
/// the account has a length, on 16 bits. (The variable-length vectors of tls_codec
/// 0.5, `VLBytes`, panic in debug builds on a truncated input.)
#[derive(TlsSerialize, TlsDeserializeBytes, TlsSize)]
struct CertificateContent {
    version: u8,
    account: TlsByteVecU16,
    device_id: [u8; 16],
    account_key: PublicKey,
    device_key: PublicKey,
}

/// The certificate as it travels in the *BasicCredential*.
#[derive(TlsSerialize, TlsDeserializeBytes, TlsSize)]
struct CertificateWire {
    content: CertificateContent,
    signature: [u8; SIGNATURE_LEN],
}

fn public_key(bytes: &[u8]) -> Result<PublicKey, IdentityError> {
    bytes.try_into().map_err(|_| IdentityError::InvalidKey)
}

/// `SignContent` of RFC 9420, §5.1.2.
#[derive(TlsSerialize, TlsSize)]
struct SignContent<'a> {
    label: VLByteSlice<'a>,
    content: VLByteSlice<'a>,
}

/// A device certificate: the account key attests that the signing key `device_key`
/// is the one of device `device_id` of account `account`.
#[derive(Clone, PartialEq, Eq)]
pub struct DeviceCertificate {
    account: AccountId,
    device_id: DeviceId,
    account_key: PublicKey,
    device_key: PublicKey,
    signature: [u8; SIGNATURE_LEN],
}

impl DeviceCertificate {
    /// The announced account.
    pub fn account(&self) -> &AccountId {
        &self.account
    }

    /// The device.
    pub fn device_id(&self) -> DeviceId {
        self.device_id
    }

    /// The account key that signed: to compare with the one known for the account.
    pub fn account_key(&self) -> &[u8] {
        &self.account_key
    }

    /// The device's MLS signing key.
    pub fn device_key(&self) -> &[u8] {
        &self.device_key
    }

    fn content(&self) -> CertificateContent {
        CertificateContent {
            version: CERTIFICATE_VERSION,
            account: self.account.to_string().into_bytes().into(),
            device_id: self.device_id.0,
            account_key: self.account_key,
            device_key: self.device_key,
        }
    }

    fn to_be_signed(&self) -> Result<Vec<u8>, IdentityError> {
        let content = self
            .content()
            .tls_serialize_detached()
            .map_err(|_| IdentityError::MalformedCertificate)?;
        SignContent {
            label: VLByteSlice(CERTIFICATE_LABEL),
            content: VLByteSlice(&content),
        }
        .tls_serialize_detached()
        .map_err(|_| IdentityError::MalformedCertificate)
    }

    /// Verifies the certificate signature with the account key it announces.
    pub fn verify(&self) -> Result<(), IdentityError> {
        RustCrypto::default()
            .verify_signature(
                SCHEME,
                &self.to_be_signed()?,
                &self.account_key,
                &self.signature,
            )
            .map_err(|_| IdentityError::BadSignature)
    }

    /// The serialized certificate (TLS), as it travels.
    pub fn to_bytes(&self) -> Result<Vec<u8>, IdentityError> {
        CertificateWire {
            content: self.content(),
            signature: self.signature,
        }
        .tls_serialize_detached()
        .map_err(|_| IdentityError::MalformedCertificate)
    }

    /// Parses a serialized certificate without verifying it: see [`DeviceCertificate::verify`].
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, IdentityError> {
        const MALFORMED: IdentityError = IdentityError::MalformedCertificate;
        if bytes.first() != Some(&CERTIFICATE_VERSION) {
            return Err(MALFORMED);
        }
        let (wire, rest) = CertificateWire::tls_deserialize_bytes(bytes).map_err(|_| MALFORMED)?;
        if !rest.is_empty() {
            return Err(MALFORMED);
        }
        let account = std::str::from_utf8(wire.content.account.as_slice())
            .map_err(|_| MALFORMED)?
            .parse()
            .map_err(|_| MALFORMED)?;
        Ok(Self {
            account,
            device_id: DeviceId(wire.content.device_id),
            account_key: wire.content.account_key,
            device_key: wire.content.device_key,
            signature: wire.signature,
        })
    }

    /// The device's MLS *credential*: a *BasicCredential* carrying the certificate.
    pub fn to_credential(&self) -> Result<Credential, IdentityError> {
        Ok(BasicCredential::new(self.to_bytes()?).into())
    }

    /// What every group member does when facing a device's leaf: read the certificate
    /// from its *credential*, verify the account signature, and verify that the leaf's
    /// signing key is the one the certificate certifies.
    pub fn from_credential(
        credential: &Credential,
        signature_key: &[u8],
    ) -> Result<Self, IdentityError> {
        let basic = BasicCredential::try_from(credential.clone())
            .map_err(|_| IdentityError::NotBasicCredential)?;
        let certificate = Self::from_bytes(basic.identity())?;
        certificate.verify()?;
        if certificate.device_key.as_slice() != signature_key {
            return Err(IdentityError::KeyMismatch);
        }
        Ok(certificate)
    }
}

impl fmt::Debug for DeviceCertificate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DeviceCertificate")
            .field("account", &self.account)
            .field("device_id", &self.device_id)
            .finish_non_exhaustive()
    }
}

/// A certified device: its MLS signing key and its certificate.
#[derive(Debug)]
pub struct Device {
    key: DeviceKey,
    certificate: DeviceCertificate,
}

impl Device {
    /// The first device of an account: it draws its identifier, creates its key and has
    /// it certified by the account key it holds.
    pub fn create(account: &AccountId, account_key: &AccountKey) -> Result<Self, IdentityError> {
        let key = DeviceKey::generate()?;
        let certificate = account_key.certify(account, DeviceId::random()?, key.public_key())?;
        Self::new(key, certificate)
    }

    /// Assembles a device key and the certificate received for it (adding a device by
    /// QR code, story 03-04). Rejects an invalid certificate or one made for another
    /// key.
    pub fn new(key: DeviceKey, certificate: DeviceCertificate) -> Result<Self, IdentityError> {
        certificate.verify()?;
        if certificate.device_key.as_slice() != key.public_key() {
            return Err(IdentityError::KeyMismatch);
        }
        Ok(Self { key, certificate })
    }

    /// The device's certificate.
    pub fn certificate(&self) -> &DeviceCertificate {
        &self.certificate
    }

    /// The *credential* and public key the device presents in its MLS groups.
    pub fn credential_with_key(&self) -> Result<CredentialWithKey, IdentityError> {
        Ok(CredentialWithKey {
            credential: self.certificate.to_credential()?,
            signature_key: self.key.public_key().into(),
        })
    }

    /// Writes the device, private key included, into a record of the local database.
    /// Only the store (story 02-06) calls this, and it seals the record before it leaves
    /// the core.
    pub(crate) fn write_to(&self, out: &mut Writer) -> Result<(), CodecError> {
        out.vec8(&self.key.0.private)?;
        out.vec8(&self.key.0.public)?;
        let certificate = self
            .certificate
            .to_bytes()
            .map_err(|_| CodecError::Malformed)?;
        out.vec16(&certificate)
    }

    /// Reads back a device written by [`Device::write_to`], checking again that the
    /// certificate is valid and certifies this key.
    pub(crate) fn read_from(input: &mut Reader<'_>) -> Result<Self, CodecError> {
        let private = Zeroizing::new(input.vec8()?.to_vec());
        let public = input.vec8()?.to_vec();
        let certificate =
            DeviceCertificate::from_bytes(input.vec16()?).map_err(|_| CodecError::Malformed)?;
        Self::new(DeviceKey(SigningKey { private, public }), certificate)
            .map_err(|_| CodecError::Malformed)
    }
}

/// The device signs its MLS messages; OpenMLS only sees this trait, never the key.
impl Signer for Device {
    fn sign(&self, payload: &[u8]) -> Result<Vec<u8>, SignerError> {
        self.key
            .0
            .sign(payload)
            .map_err(|_| SignerError::SigningError)
    }

    fn signature_scheme(&self) -> SignatureScheme {
        SCHEME
    }
}

#[cfg(test)]
mod tests;
