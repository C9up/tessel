//! Identité de compte et d'appareil (ADR-002, story 02-02).
//!
//! Un compte `pseudo@domaine` a une clé d'identité (Ed25519). Chaque appareil a sa
//! propre clé de signature MLS, et un certificat signé par la clé du compte qui lie
//! l'appareil au compte. Le certificat voyage dans le *BasicCredential* MLS de
//! l'appareil : tout membre d'un groupe peut le vérifier.
//!
//! Ce que la vérification ne dit pas : que la clé de compte est bien celle de
//! `pseudo@domaine`. Ce lien vient de l'annuaire du serveur (story 03-01) et des codes
//! de sécurité (story 02-09) ; l'appelant compare `account_key` à la clé qu'il connaît.
//!
//! Les clés privées ne sortent jamais d'ici : aucun type public ne les rend, ne les
//! sérialise ni ne les affiche, et elles sont effacées de la mémoire à la libération.

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

/// Schéma de signature des clés de compte et d'appareil : celui de la suite 0x0001.
const SCHEME: SignatureScheme = SignatureScheme::ED25519;

/// Étiquette de signature du certificat, sur le modèle de `SignWithLabel` (RFC 9420,
/// §5.1.2), dans un espace de noms propre à Tessel.
const CERTIFICATE_LABEL: &[u8] = b"Tessel 1.0 DeviceCertificate";

/// Version du format du certificat.
const CERTIFICATE_VERSION: u8 = 1;

/// Taille d'une clé publique Ed25519.
const PUBLIC_KEY_LEN: usize = 32;

/// Taille d'une signature Ed25519.
const SIGNATURE_LEN: usize = 64;

type PublicKey = [u8; PUBLIC_KEY_LEN];

/// Ce que l'identité peut refuser.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum IdentityError {
    /// Le compte n'est pas un `pseudo@domaine` valide (ADR-002).
    #[error("invalid account: {0}")]
    InvalidAccount(&'static str),
    /// Le fournisseur crypto a échoué (aléa, génération ou signature).
    #[error("crypto provider failure")]
    Crypto,
    /// Une clé publique n'a pas la taille d'une clé Ed25519.
    #[error("invalid public key")]
    InvalidKey,
    /// Le certificat n'est pas décodable, ou dans une version inconnue.
    #[error("malformed device certificate")]
    MalformedCertificate,
    /// La signature du certificat ne vérifie pas avec la clé de compte annoncée.
    #[error("device certificate signature does not verify")]
    BadSignature,
    /// La clé de signature de l'appareil n'est pas celle que le certificat certifie.
    #[error("device key does not match its certificate")]
    KeyMismatch,
    /// Le *credential* MLS n'est pas un *BasicCredential*.
    #[error("not a basic credential")]
    NotBasicCredential,
}

/// Un compte : `pseudo@domaine` (ADR-002).
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct AccountId {
    name: String,
    domain: String,
}

impl AccountId {
    /// Lit et valide `pseudo@domaine`. Le pseudo : 3 à 32 caractères parmi `a-z`,
    /// `0-9`, `.`, `_`, `-`, une lettre en premier. Le domaine : un nom d'hôte en
    /// minuscules (étiquettes `a-z`, `0-9`, `-`).
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

    /// Le pseudo.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Le domaine de l'instance.
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

/// Identifiant d'appareil : 128 bits aléatoires, unique dans son compte (ADR-002).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct DeviceId([u8; 16]);

impl DeviceId {
    /// Tire un identifiant au hasard.
    pub fn random() -> Result<Self, IdentityError> {
        RustCrypto::default()
            .random_array()
            .map(Self)
            .map_err(|_| IdentityError::Crypto)
    }

    /// Reprend un identifiant connu.
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// Les 16 octets de l'identifiant.
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

/// Une paire de clés Ed25519 dont la moitié privée ne sort pas du crate.
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

/// La clé d'identité d'un compte. Elle signe les certificats de ses appareils.
#[derive(Debug)]
pub struct AccountKey(SigningKey);

impl AccountKey {
    /// Crée une nouvelle clé de compte.
    pub fn generate() -> Result<Self, IdentityError> {
        SigningKey::generate().map(Self)
    }

    /// La clé publique, que l'annuaire publie et que les codes de sécurité comparent.
    pub fn public_key(&self) -> &[u8] {
        &self.0.public
    }

    /// Certifie qu'un appareil appartient au compte. `device_key` est la clé publique
    /// de signature de l'appareil (sur le nouvel appareil : [`DeviceKey::public_key`]).
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
}

/// La clé de signature MLS d'un appareil, avant qu'il soit certifié.
#[derive(Debug)]
pub struct DeviceKey(SigningKey);

impl DeviceKey {
    /// Crée une nouvelle clé d'appareil.
    pub fn generate() -> Result<Self, IdentityError> {
        SigningKey::generate().map(Self)
    }

    /// La clé publique, à faire certifier par la clé du compte.
    pub fn public_key(&self) -> &[u8] {
        &self.0.public
    }
}

/// Le contenu signé d'un certificat. Les clés et la signature ont une taille fixe ;
/// seul le compte a une longueur, sur 16 bits. (Les vecteurs à longueur variable de
/// tls_codec 0.5, `VLBytes`, paniquent en debug sur une entrée tronquée.)
#[derive(TlsSerialize, TlsDeserializeBytes, TlsSize)]
struct CertificateContent {
    version: u8,
    account: TlsByteVecU16,
    device_id: [u8; 16],
    account_key: PublicKey,
    device_key: PublicKey,
}

/// Le certificat tel qu'il voyage dans le *BasicCredential*.
#[derive(TlsSerialize, TlsDeserializeBytes, TlsSize)]
struct CertificateWire {
    content: CertificateContent,
    signature: [u8; SIGNATURE_LEN],
}

fn public_key(bytes: &[u8]) -> Result<PublicKey, IdentityError> {
    bytes.try_into().map_err(|_| IdentityError::InvalidKey)
}

/// `SignContent` de RFC 9420, §5.1.2.
#[derive(TlsSerialize, TlsSize)]
struct SignContent<'a> {
    label: VLByteSlice<'a>,
    content: VLByteSlice<'a>,
}

/// Le certificat d'un appareil : la clé de compte atteste que la clé de signature
/// `device_key` est celle de l'appareil `device_id` du compte `account`.
#[derive(Clone, PartialEq, Eq)]
pub struct DeviceCertificate {
    account: AccountId,
    device_id: DeviceId,
    account_key: PublicKey,
    device_key: PublicKey,
    signature: [u8; SIGNATURE_LEN],
}

impl DeviceCertificate {
    /// Le compte annoncé.
    pub fn account(&self) -> &AccountId {
        &self.account
    }

    /// L'appareil.
    pub fn device_id(&self) -> DeviceId {
        self.device_id
    }

    /// La clé de compte qui a signé : à comparer à celle que l'on connaît du compte.
    pub fn account_key(&self) -> &[u8] {
        &self.account_key
    }

    /// La clé de signature MLS de l'appareil.
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

    /// Vérifie la signature du certificat avec la clé de compte qu'il annonce.
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

    /// Le certificat sérialisé (TLS), tel qu'il voyage.
    pub fn to_bytes(&self) -> Result<Vec<u8>, IdentityError> {
        CertificateWire {
            content: self.content(),
            signature: self.signature,
        }
        .tls_serialize_detached()
        .map_err(|_| IdentityError::MalformedCertificate)
    }

    /// Lit un certificat sérialisé, sans le vérifier : voir [`DeviceCertificate::verify`].
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

    /// Le *credential* MLS de l'appareil : un *BasicCredential* qui porte le certificat.
    pub fn to_credential(&self) -> Result<Credential, IdentityError> {
        Ok(BasicCredential::new(self.to_bytes()?).into())
    }

    /// Ce que fait tout membre d'un groupe devant la feuille d'un appareil : lire le
    /// certificat de son *credential*, vérifier la signature du compte, et vérifier que
    /// la clé de signature de la feuille est celle que le certificat certifie.
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

/// Un appareil certifié : sa clé de signature MLS et son certificat.
#[derive(Debug)]
pub struct Device {
    key: DeviceKey,
    certificate: DeviceCertificate,
}

impl Device {
    /// Le premier appareil d'un compte : il tire son identifiant, crée sa clé et la
    /// fait certifier par la clé du compte qu'il détient.
    pub fn create(account: &AccountId, account_key: &AccountKey) -> Result<Self, IdentityError> {
        let key = DeviceKey::generate()?;
        let certificate = account_key.certify(account, DeviceId::random()?, key.public_key())?;
        Self::new(key, certificate)
    }

    /// Assemble une clé d'appareil et le certificat reçu pour elle (ajout d'un appareil
    /// par QR, story 03-04). Refuse un certificat invalide ou fait pour une autre clé.
    pub fn new(key: DeviceKey, certificate: DeviceCertificate) -> Result<Self, IdentityError> {
        certificate.verify()?;
        if certificate.device_key.as_slice() != key.public_key() {
            return Err(IdentityError::KeyMismatch);
        }
        Ok(Self { key, certificate })
    }

    /// Le certificat de l'appareil.
    pub fn certificate(&self) -> &DeviceCertificate {
        &self.certificate
    }

    /// Le *credential* et la clé publique que l'appareil présente dans ses groupes MLS.
    pub fn credential_with_key(&self) -> Result<CredentialWithKey, IdentityError> {
        Ok(CredentialWithKey {
            credential: self.certificate.to_credential()?,
            signature_key: self.key.public_key().into(),
        })
    }
}

/// L'appareil signe ses messages MLS ; OpenMLS ne voit que ce trait, jamais la clé.
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
