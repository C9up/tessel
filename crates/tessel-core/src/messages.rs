//! Messages applicatifs (story 02-05).
//!
//! Ce que les membres d'un groupe s'envoient, une fois déchiffré : texte, média, accusé,
//! édition, suppression. Le serveur n'en voit rien ; il ne voit que la taille du message
//! chiffré, que le remplissage brouille.
//!
//! # Format (version 1)
//!
//! Entiers en gros-boutiste, comme en TLS. Chaque champ variable est précédé de sa
//! longueur ; rien n'est implicite, donc un message se lit sans ambiguïté.
//!
//! ```text
//! u8   version          (1)
//! u8   genre            (1 texte, 2 média, 3 accusé, 4 édition, 5 suppression)
//! [16] identifiant      (tiré au hasard par l'expéditeur)
//! u64  envoyé à         (millisecondes depuis l'époque Unix, horloge de l'expéditeur)
//! ...  corps            (selon le genre)
//! ...  remplissage      (des zéros, jusqu'à la taille de son palier)
//! ```
//!
//! Le décodage est strict : version inconnue, genre inconnu, longueur qui déborde,
//! UTF-8 invalide, remplissage non nul ou de la mauvaise taille, tout est refusé avec une
//! erreur, jamais une panique. Le lecteur est écrit à la main (sans `tls_codec`, dont
//! `VLBytes` panique en debug sur une entrée tronquée) et ne fait que des lectures bornées.
//!
//! # Remplissage
//!
//! Le message encodé est complété par des zéros jusqu'à au moins [`MIN_PADDED`] octets,
//! puis jusqu'au palier Padmé de sa taille (Nikitin et al., *Reducing Metadata Leakage
//! from Encrypted Files and Communication with PURBs*, PETS 2019) : au plus 12 % de
//! surcoût, et la taille ne trahit qu'un ordre de grandeur. Tous les messages courts
//! (« ok », « j'arrive ») ont la même taille chiffrée.

use std::fmt;

use openmls_rust_crypto::RustCrypto;
use openmls_traits::random::OpenMlsRand;
use zeroize::Zeroizing;

/// Version du format écrit par ce cœur.
pub const MESSAGE_VERSION: u8 = 1;

/// Taille minimale d'un message encodé, remplissage compris.
pub const MIN_PADDED: usize = 256;

/// Longueur maximale d'un texte, d'une légende ou d'une édition, en octets UTF-8.
pub const MAX_TEXT: usize = u16::MAX as usize;

/// Nombre maximal de messages qu'un accusé couvre.
pub const MAX_RECEIPT: usize = 256;

/// Longueur maximale de l'identifiant d'un blob, du type d'un média, de son nom.
const MAX_BLOB_ID: usize = 64;
const MAX_CONTENT_TYPE: usize = 127;
const MAX_FILE_NAME: usize = 255;

const KIND_TEXT: u8 = 1;
const KIND_MEDIA: u8 = 2;
const KIND_RECEIPT: u8 = 3;
const KIND_EDIT: u8 = 4;
const KIND_DELETE: u8 = 5;

/// Ce que le format peut refuser.
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum MessageError {
    /// Un message d'une version plus récente que ce cœur : l'appli invite à se mettre à jour.
    #[error("unsupported message version {0}")]
    UnsupportedVersion(u8),
    /// Un genre de message que ce cœur ne connaît pas encore.
    #[error("unknown message kind {0}")]
    UnknownKind(u8),
    /// Le message est tronqué, déborde ou viole une règle du format.
    #[error("malformed message")]
    Malformed,
    /// Le message à envoyer viole une règle du format (texte vide ou trop long…).
    #[error("invalid message: {0}")]
    Invalid(&'static str),
    /// Pas d'aléa pour tirer l'identifiant.
    #[error("no randomness")]
    NoRandomness,
}

/// L'identifiant d'un message, tiré au hasard par son expéditeur.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct MessageId([u8; 16]);

impl MessageId {
    /// Tire un identifiant au hasard.
    pub fn random() -> Result<Self, MessageError> {
        RustCrypto::default()
            .random_array()
            .map(Self)
            .map_err(|_| MessageError::NoRandomness)
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

impl fmt::Display for MessageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.iter().try_for_each(|b| write!(f, "{b:02x}"))
    }
}

impl fmt::Debug for MessageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MessageId({self})")
    }
}

/// La clé d'un média (story 02-08) : elle voyage dans le message chiffré, jamais à part,
/// et s'efface de la mémoire à la libération.
#[derive(Clone, PartialEq, Eq)]
pub struct MediaKey(Zeroizing<[u8; 32]>);

impl MediaKey {
    // Seuls le décodage et le chiffrement des médias (story 02-08) créent et lisent une
    // clé ; aucune fonction publique ne la rend.
    pub(crate) fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(Zeroizing::new(bytes))
    }

    fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for MediaKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("MediaKey(..)")
    }
}

/// Un média déposé chiffré sur le serveur (story 02-08) : de quoi le retrouver, le
/// déchiffrer et vérifier qu'il n'a pas été modifié.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaRef {
    /// L'identifiant du blob chiffré sur le serveur (1 à 64 octets).
    pub blob: Vec<u8>,
    /// La clé du blob.
    pub key: MediaKey,
    /// L'empreinte SHA-256 du blob chiffré.
    pub digest: [u8; 32],
    /// La taille du fichier en clair, en octets.
    pub size: u64,
    /// Son type (`image/jpeg`…), 127 octets au plus.
    pub content_type: String,
    /// Son nom, 255 octets au plus ; vide s'il n'en a pas.
    pub name: String,
}

/// Ce qu'un accusé atteste.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReceiptKind {
    /// Le message est arrivé sur l'appareil.
    Delivered,
    /// Le message a été lu.
    Read,
}

/// Le corps d'un message.
///
/// Une édition ou une suppression vise un message par son identifiant ; seul l'auteur
/// du message visé peut le changer. Ce cœur ne garde pas encore l'historique : la
/// vérification se fera avec la base locale (story 02-06), en comparant l'expéditeur
/// des deux messages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Body {
    /// Un texte, éventuellement en réponse à un autre message.
    Text {
        /// Le texte, 1 à [`MAX_TEXT`] octets.
        text: String,
        /// Le message auquel il répond.
        reply_to: Option<MessageId>,
    },
    /// Un média, avec une légende, éventuellement en réponse à un autre message.
    Media {
        /// Le média.
        media: MediaRef,
        /// La légende, vide s'il n'y en a pas.
        caption: String,
        /// Le message auquel il répond.
        reply_to: Option<MessageId>,
    },
    /// Un accusé de réception ou de lecture.
    Receipt {
        /// Réception ou lecture.
        kind: ReceiptKind,
        /// Les messages accusés, 1 à [`MAX_RECEIPT`].
        messages: Vec<MessageId>,
    },
    /// Le nouveau texte d'un message.
    Edit {
        /// Le message édité.
        target: MessageId,
        /// Son nouveau texte, 1 à [`MAX_TEXT`] octets.
        text: String,
    },
    /// La suppression d'un message, pour tous.
    Delete {
        /// Le message supprimé.
        target: MessageId,
    },
}

/// Un message applicatif.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    /// Son identifiant.
    pub id: MessageId,
    /// Quand il a été envoyé, en millisecondes depuis l'époque Unix, selon l'horloge de
    /// l'expéditeur (qui peut mentir : seul l'ordre du serveur fait foi).
    pub sent_at: u64,
    /// Son corps.
    pub body: Body,
}

impl Message {
    /// Un nouveau message, avec un identifiant tiré au hasard.
    pub fn new(sent_at: u64, body: Body) -> Result<Self, MessageError> {
        Ok(Self {
            id: MessageId::random()?,
            sent_at,
            body,
        })
    }

    /// Encode le message, remplissage compris.
    pub fn encode(&self) -> Result<Vec<u8>, MessageError> {
        self.validate().map_err(MessageError::Invalid)?;
        let mut out = Writer(Vec::with_capacity(MIN_PADDED));
        out.u8(MESSAGE_VERSION);
        out.u8(self.body.kind());
        out.bytes(&self.id.0);
        out.u64(self.sent_at);
        match &self.body {
            Body::Text { text, reply_to } => {
                out.optional_id(reply_to.as_ref());
                out.vec16(text.as_bytes())?;
            }
            Body::Media {
                media,
                caption,
                reply_to,
            } => {
                out.optional_id(reply_to.as_ref());
                out.vec8(&media.blob)?;
                out.bytes(media.key.as_bytes());
                out.bytes(&media.digest);
                out.u64(media.size);
                out.vec8(media.content_type.as_bytes())?;
                out.vec8(media.name.as_bytes())?;
                out.vec16(caption.as_bytes())?;
            }
            Body::Receipt { kind, messages } => {
                out.u8(match kind {
                    ReceiptKind::Delivered => 1,
                    ReceiptKind::Read => 2,
                });
                let count = u16::try_from(messages.len())
                    .map_err(|_| MessageError::Invalid("too many receipts"))?;
                out.u16(count);
                messages.iter().for_each(|id| out.bytes(&id.0));
            }
            Body::Edit { target, text } => {
                out.bytes(&target.0);
                out.vec16(text.as_bytes())?;
            }
            Body::Delete { target } => out.bytes(&target.0),
        }
        let mut bytes = out.0;
        bytes.resize(padded_len(bytes.len()), 0);
        Ok(bytes)
    }

    /// Décode un message reçu ; refuse, sans paniquer, tout ce qui n'est pas un message
    /// valide de ce format.
    pub fn decode(bytes: &[u8]) -> Result<Self, MessageError> {
        let mut input = Reader(bytes);
        let version = input.u8()?;
        if version != MESSAGE_VERSION {
            return Err(MessageError::UnsupportedVersion(version));
        }
        let kind = input.u8()?;
        if !(KIND_TEXT..=KIND_DELETE).contains(&kind) {
            return Err(MessageError::UnknownKind(kind));
        }
        let id = input.id()?;
        let sent_at = input.u64()?;
        let body = match kind {
            KIND_TEXT => Body::Text {
                reply_to: input.optional_id()?,
                text: input.string16()?,
            },
            KIND_MEDIA => {
                let reply_to = input.optional_id()?;
                let media = MediaRef {
                    blob: input.vec8()?.to_vec(),
                    key: MediaKey::from_bytes(input.array()?),
                    digest: input.array()?,
                    size: input.u64()?,
                    content_type: input.string8()?,
                    name: input.string8()?,
                };
                Body::Media {
                    media,
                    caption: input.string16()?,
                    reply_to,
                }
            }
            KIND_RECEIPT => {
                let kind = match input.u8()? {
                    1 => ReceiptKind::Delivered,
                    2 => ReceiptKind::Read,
                    _ => return Err(MessageError::Malformed),
                };
                let count = usize::from(input.u16()?);
                if count == 0 || count > MAX_RECEIPT {
                    return Err(MessageError::Malformed);
                }
                let messages = (0..count).map(|_| input.id()).collect::<Result<_, _>>()?;
                Body::Receipt { kind, messages }
            }
            KIND_EDIT => Body::Edit {
                target: input.id()?,
                text: input.string16()?,
            },
            _ => Body::Delete {
                target: input.id()?,
            },
        };

        // Le remplissage : rien que des zéros, et exactement jusqu'au palier du contenu.
        // Une seule écriture possible par message, donc pas de canal caché.
        let content = bytes.len() - input.0.len();
        if bytes.len() != padded_len(content) || input.0.iter().any(|&b| b != 0) {
            return Err(MessageError::Malformed);
        }

        let message = Self { id, sent_at, body };
        message.validate().map_err(|_| MessageError::Malformed)?;
        Ok(message)
    }

    /// Les règles que l'encodage ne garantit pas à lui seul.
    fn validate(&self) -> Result<(), &'static str> {
        fn text(text: &str, max: usize, empty_ok: bool) -> Result<(), &'static str> {
            match text.len() {
                0 if !empty_ok => Err("empty text"),
                n if n > max => Err("text too long"),
                _ => Ok(()),
            }
        }
        match &self.body {
            Body::Text { text: t, .. } | Body::Edit { text: t, .. } => text(t, MAX_TEXT, false),
            Body::Media { media, caption, .. } => {
                if media.blob.is_empty() || media.blob.len() > MAX_BLOB_ID {
                    return Err("bad blob id");
                }
                text(&media.content_type, MAX_CONTENT_TYPE, false)?;
                text(&media.name, MAX_FILE_NAME, true)?;
                text(caption, MAX_TEXT, true)
            }
            Body::Receipt { messages, .. } => match messages.len() {
                0 => Err("empty receipt"),
                n if n > MAX_RECEIPT => Err("too many receipts"),
                _ => Ok(()),
            },
            Body::Delete { .. } => Ok(()),
        }
    }
}

impl Body {
    fn kind(&self) -> u8 {
        match self {
            Self::Text { .. } => KIND_TEXT,
            Self::Media { .. } => KIND_MEDIA,
            Self::Receipt { .. } => KIND_RECEIPT,
            Self::Edit { .. } => KIND_EDIT,
            Self::Delete { .. } => KIND_DELETE,
        }
    }
}

/// La taille d'un message de `len` octets une fois rempli : au moins [`MIN_PADDED`],
/// puis le palier Padmé.
pub(crate) fn padded_len(len: usize) -> usize {
    padme(len.max(MIN_PADDED))
}

/// Padmé : arrondit `len` en ne gardant que ses `⌊log2 ⌊log2 len⌋⌋ + 1` bits de poids
/// fort ; les bits suivants sont mis à zéro et la valeur arrondie vers le haut.
fn padme(len: usize) -> usize {
    if len < 2 {
        return len;
    }
    let exponent = len.ilog2(); // E = ⌊log2 L⌋
    let significant = exponent.ilog2() + 1; // S = ⌊log2 E⌋ + 1
    let mask = (1usize << (exponent - significant)) - 1;
    (len + mask) & !mask
}

struct Writer(Vec<u8>);

impl Writer {
    fn u8(&mut self, value: u8) {
        self.0.push(value);
    }

    fn u16(&mut self, value: u16) {
        self.0.extend_from_slice(&value.to_be_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.0.extend_from_slice(&value.to_be_bytes());
    }

    fn bytes(&mut self, bytes: &[u8]) {
        self.0.extend_from_slice(bytes);
    }

    fn vec8(&mut self, bytes: &[u8]) -> Result<(), MessageError> {
        let len = u8::try_from(bytes.len()).map_err(|_| MessageError::Invalid("field too long"))?;
        self.u8(len);
        self.bytes(bytes);
        Ok(())
    }

    fn vec16(&mut self, bytes: &[u8]) -> Result<(), MessageError> {
        let len =
            u16::try_from(bytes.len()).map_err(|_| MessageError::Invalid("field too long"))?;
        self.u16(len);
        self.bytes(bytes);
        Ok(())
    }

    fn optional_id(&mut self, id: Option<&MessageId>) {
        match id {
            None => self.u8(0),
            Some(id) => {
                self.u8(1);
                self.bytes(&id.0);
            }
        }
    }
}

/// Un lecteur qui ne lit jamais au-delà de ce qu'il reste.
struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], MessageError> {
        if n > self.0.len() {
            return Err(MessageError::Malformed);
        }
        let (head, tail) = self.0.split_at(n);
        self.0 = tail;
        Ok(head)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], MessageError> {
        self.take(N)?
            .try_into()
            .map_err(|_| MessageError::Malformed)
    }

    fn u8(&mut self) -> Result<u8, MessageError> {
        self.array::<1>().map(|[b]| b)
    }

    fn u16(&mut self) -> Result<u16, MessageError> {
        self.array().map(u16::from_be_bytes)
    }

    fn u64(&mut self) -> Result<u64, MessageError> {
        self.array().map(u64::from_be_bytes)
    }

    fn id(&mut self) -> Result<MessageId, MessageError> {
        self.array().map(MessageId)
    }

    fn optional_id(&mut self) -> Result<Option<MessageId>, MessageError> {
        match self.u8()? {
            0 => Ok(None),
            1 => self.id().map(Some),
            _ => Err(MessageError::Malformed),
        }
    }

    fn vec8(&mut self) -> Result<&'a [u8], MessageError> {
        let len = self.u8()?;
        self.take(usize::from(len))
    }

    fn vec16(&mut self) -> Result<&'a [u8], MessageError> {
        let len = self.u16()?;
        self.take(usize::from(len))
    }

    fn string8(&mut self) -> Result<String, MessageError> {
        let bytes = self.vec8()?;
        String::from_utf8(bytes.to_vec()).map_err(|_| MessageError::Malformed)
    }

    fn string16(&mut self) -> Result<String, MessageError> {
        let bytes = self.vec16()?;
        String::from_utf8(bytes.to_vec()).map_err(|_| MessageError::Malformed)
    }
}

#[cfg(test)]
mod tests;
