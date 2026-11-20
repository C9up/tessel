//! Application messages (story 02-05).
//!
//! What the members of a group send each other, once decrypted: text, media, receipt,
//! edit, deletion. The server sees none of it; it only sees the size of the encrypted
//! message, which the padding blurs.
//!
//! # Format (version 1)
//!
//! Integers are big-endian, as in TLS. Every variable-length field is preceded by its
//! length; nothing is implicit, so a message reads without ambiguity.
//!
//! ```text
//! u8   version          (1)
//! u8   kind             (1 text, 2 media, 3 receipt, 4 edit, 5 deletion)
//! [16] identifier       (drawn at random by the sender)
//! u64  sent at          (milliseconds since the Unix epoch, sender's clock)
//! ...  body             (depends on the kind)
//! ...  padding          (zeros, up to the size of its bucket)
//! ```
//!
//! Decoding is strict: unknown version, unknown kind, overflowing length, invalid UTF-8,
//! padding that is nonzero or of the wrong size, everything is rejected with an error,
//! never a panic. The reader is hand-written (without `tls_codec`, whose `VLBytes`
//! panics in debug builds on truncated input) and only performs bounded reads.
//!
//! # Padding
//!
//! The encoded message is completed with zeros up to at least [`MIN_PADDED`] bytes, then
//! up to the `Padmé` bucket of its size (Nikitin et al., *Reducing Metadata Leakage from
//! Encrypted Files and Communication with PURBs*, PETS 2019): at most 12% overhead, and
//! the size only betrays an order of magnitude. All short messages ("ok", "on my way")
//! have the same encrypted size.

use std::fmt;

use openmls_rust_crypto::RustCrypto;
use openmls_traits::random::OpenMlsRand;
use zeroize::Zeroizing;

/// Version of the format written by this core.
pub const MESSAGE_VERSION: u8 = 1;

/// Minimum size of an encoded message, padding included.
pub const MIN_PADDED: usize = 256;

/// Maximum length of a text, a caption or an edit, in UTF-8 bytes.
pub const MAX_TEXT: usize = u16::MAX as usize;

/// Maximum number of messages a receipt covers.
pub const MAX_RECEIPT: usize = 256;

/// Maximum length of a blob identifier, of a media's content type, of its name.
const MAX_BLOB_ID: usize = 64;
const MAX_CONTENT_TYPE: usize = 127;
const MAX_FILE_NAME: usize = 255;

const KIND_TEXT: u8 = 1;
const KIND_MEDIA: u8 = 2;
const KIND_RECEIPT: u8 = 3;
const KIND_EDIT: u8 = 4;
const KIND_DELETE: u8 = 5;

/// What the format can reject.
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum MessageError {
    /// A message of a newer version than this core: the app prompts the user to update.
    #[error("unsupported message version {0}")]
    UnsupportedVersion(u8),
    /// A message kind this core does not know yet.
    #[error("unknown message kind {0}")]
    UnknownKind(u8),
    /// The message is truncated, overflows or violates a rule of the format.
    #[error("malformed message")]
    Malformed,
    /// The message to send violates a rule of the format (empty or too long text…).
    #[error("invalid message: {0}")]
    Invalid(&'static str),
    /// No randomness available to draw the identifier.
    #[error("no randomness")]
    NoRandomness,
}

/// A message's identifier, drawn at random by its sender.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct MessageId([u8; 16]);

impl MessageId {
    /// Draws an identifier at random.
    pub fn random() -> Result<Self, MessageError> {
        RustCrypto::default()
            .random_array()
            .map(Self)
            .map_err(|_| MessageError::NoRandomness)
    }

    /// Wraps a known identifier.
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// The identifier's 16 bytes.
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

/// A media key (story 02-08): it travels inside the encrypted message, never separately,
/// and is wiped from memory when dropped.
#[derive(Clone, PartialEq, Eq)]
pub struct MediaKey(Zeroizing<[u8; 32]>);

impl MediaKey {
    // Only media decoding and encryption (story 02-08) create and read a key; no public
    // function returns it.
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

/// A media stored encrypted on the server (story 02-08): what is needed to find it,
/// decrypt it and verify it was not modified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaRef {
    /// The identifier of the encrypted blob on the server (1 to 64 bytes).
    pub blob: Vec<u8>,
    /// The blob's key.
    pub key: MediaKey,
    /// The SHA-256 digest of the encrypted blob.
    pub digest: [u8; 32],
    /// The size of the plaintext file, in bytes.
    pub size: u64,
    /// Its content type (`image/jpeg`…), 127 bytes at most.
    pub content_type: String,
    /// Its name, 255 bytes at most; empty if it has none.
    pub name: String,
}

/// What a receipt attests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReceiptKind {
    /// The message arrived on the device.
    Delivered,
    /// The message was read.
    Read,
}

/// A message's body.
///
/// An edit or a deletion targets a message by its identifier; only the author of the
/// targeted message can change it. This core does not keep the history yet: the check
/// will be done with the local database (story 02-06), by comparing the senders of the
/// two messages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Body {
    /// A text, possibly in reply to another message.
    Text {
        /// The text, 1 to [`MAX_TEXT`] bytes.
        text: String,
        /// The message it replies to.
        reply_to: Option<MessageId>,
    },
    /// A media, with a caption, possibly in reply to another message.
    Media {
        /// The media.
        media: MediaRef,
        /// The caption, empty if there is none.
        caption: String,
        /// The message it replies to.
        reply_to: Option<MessageId>,
    },
    /// A delivery or read receipt.
    Receipt {
        /// Delivery or read.
        kind: ReceiptKind,
        /// The acknowledged messages, 1 to [`MAX_RECEIPT`].
        messages: Vec<MessageId>,
    },
    /// A message's new text.
    Edit {
        /// The edited message.
        target: MessageId,
        /// Its new text, 1 to [`MAX_TEXT`] bytes.
        text: String,
    },
    /// The deletion of a message, for everyone.
    Delete {
        /// The deleted message.
        target: MessageId,
    },
}

/// An application message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    /// Its identifier.
    pub id: MessageId,
    /// When it was sent, in milliseconds since the Unix epoch, according to the sender's
    /// clock (which can lie: only the server's order is authoritative).
    pub sent_at: u64,
    /// Its body.
    pub body: Body,
}

impl Message {
    /// A new message, with a randomly drawn identifier.
    pub fn new(sent_at: u64, body: Body) -> Result<Self, MessageError> {
        Ok(Self {
            id: MessageId::random()?,
            sent_at,
            body,
        })
    }

    /// Encodes the message, padding included.
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

    /// Decodes a received message; rejects, without panicking, anything that is not a
    /// valid message of this format.
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

        // The padding: nothing but zeros, and exactly up to the content's bucket. Only
        // one possible encoding per message, hence no covert channel.
        let content = bytes.len() - input.0.len();
        if bytes.len() != padded_len(content) || input.0.iter().any(|&b| b != 0) {
            return Err(MessageError::Malformed);
        }

        let message = Self { id, sent_at, body };
        message.validate().map_err(|_| MessageError::Malformed)?;
        Ok(message)
    }

    /// The rules that the encoding does not guarantee on its own.
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

/// The size of a `len`-byte message once padded: at least [`MIN_PADDED`], then the
/// `Padmé` bucket.
pub(crate) fn padded_len(len: usize) -> usize {
    padme(len.max(MIN_PADDED))
}

/// `Padmé`: rounds `len` by keeping only its `⌊log2 ⌊log2 len⌋⌋ + 1` most significant
/// bits; the following bits are set to zero and the value is rounded up.
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

/// A reader that never reads beyond what remains.
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
