//! What crosses the boundary: plain objects of bytes, strings and numbers, that the
//! page can copy to and from a Web Worker. Their TypeScript types are in
//! `packages/core-web/src/api.ts`; the contract tests there check both agree.
//!
//! A media's key never crosses: the page holds a `secret`, sealed by the core for the
//! time it runs ([`MediaSeal`]), and hands it back to decrypt or to send.

use crate::error::{ApiError, Code as _, OrApi as _, ignored, malformed};
use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;
use tessel_core::{
    AccountId, Avatar, Body, Content, Conversation, DeviceCertificate, DeviceId, Effect,
    EncryptedMedia, EnvelopeHeader, EnvelopeKind, GroupChange, GroupId, GroupSettings,
    HistoryEntry, Incoming, KeyPackageUpdate, MediaRef, MediaSeal, Message, MessageId, Outbound,
    Outgoing, PartitionId, Profile, ReceiptKind, Record, RecordId, Removal, SafetyNumber, Status,
    Trust,
};

fn bytes(value: impl Into<Vec<u8>>) -> ByteBuf {
    ByteBuf::from(value.into())
}

pub(crate) fn id16(value: &[u8], what: &str) -> Result<[u8; 16], ApiError> {
    value.try_into().map_err(|_| malformed(what))
}

pub(crate) fn message_id(value: &[u8]) -> Result<MessageId, ApiError> {
    id16(value, "message id").map(MessageId::from_bytes)
}

/// A device, as the page shows it and hands it back: public data only.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    account: String,
    device_id: ByteBuf,
    account_key: ByteBuf,
    device_key: ByteBuf,
    /// The signed certificate, to hand back to the calls about this device.
    certificate: ByteBuf,
}

impl DeviceInfo {
    pub(crate) fn of(certificate: &DeviceCertificate) -> Result<Self, ApiError> {
        Ok(Self {
            account: certificate.account().to_string(),
            device_id: bytes(certificate.device_id().as_bytes().to_vec()),
            account_key: bytes(certificate.account_key()),
            device_key: bytes(certificate.device_key()),
            certificate: bytes(certificate.to_bytes().api()?),
        })
    }

    pub(crate) fn all(certificates: &[DeviceCertificate]) -> Result<Vec<Self>, ApiError> {
        certificates.iter().map(Self::of).collect()
    }
}

/// A certificate handed back by the page: parsed and verified again.
pub(crate) fn certificate(value: &[u8]) -> Result<DeviceCertificate, ApiError> {
    let certificate = DeviceCertificate::from_bytes(value).api()?;
    certificate.verify().api()?;
    Ok(certificate)
}

/// A sealed record, both ways.
#[derive(Serialize, Deserialize)]
pub struct StoredRecord {
    partition: ByteBuf,
    id: ByteBuf,
    value: ByteBuf,
}

impl StoredRecord {
    pub(crate) fn into_record(self) -> Result<Record, ApiError> {
        Ok(Record {
            partition: PartitionId::from_bytes(&self.partition).api()?,
            id: RecordId::from_bytes(&self.id).api()?,
            value: self.value.into_vec(),
        })
    }
}

#[derive(Serialize)]
pub struct RecordKey {
    partition: ByteBuf,
    id: ByteBuf,
}

/// What to persist after a call.
#[derive(Serialize)]
pub struct StoreChanges {
    put: Vec<StoredRecord>,
    delete: Vec<RecordKey>,
}

impl From<tessel_core::Changes> for StoreChanges {
    fn from(changes: tessel_core::Changes) -> Self {
        Self {
            put: changes
                .put
                .into_iter()
                .map(|record| StoredRecord {
                    partition: bytes(record.partition.as_bytes().to_vec()),
                    id: bytes(record.id.as_bytes().to_vec()),
                    value: bytes(record.value),
                })
                .collect(),
            delete: changes
                .delete
                .into_iter()
                .map(|(partition, id)| RecordKey {
                    partition: bytes(partition.as_bytes().to_vec()),
                    id: bytes(id.as_bytes().to_vec()),
                })
                .collect(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyPackages {
    publish: Vec<ByteBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_resort: Option<ByteBuf>,
    withdraw: Vec<ByteBuf>,
}

impl From<KeyPackageUpdate> for KeyPackages {
    fn from(update: KeyPackageUpdate) -> Self {
        Self {
            publish: update.publish.into_iter().map(bytes).collect(),
            last_resort: update.last_resort.map(bytes),
            withdraw: update.withdraw.into_iter().map(bytes).collect(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyPackageInfo {
    pub(crate) device: DeviceInfo,
    pub(crate) reference: ByteBuf,
    pub(crate) last_resort: bool,
    /// Unix seconds.
    pub(crate) not_after: u64,
}

#[derive(Serialize)]
pub struct Commit {
    commit: ByteBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    welcome: Option<ByteBuf>,
}

impl From<Outgoing> for Commit {
    fn from(outgoing: Outgoing) -> Self {
        Self {
            commit: bytes(outgoing.commit),
            welcome: outgoing.welcome.map(bytes),
        }
    }
}

/// Whom to remove: an account, or one of its devices.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemovalIn {
    account: String,
    device_id: Option<ByteBuf>,
}

impl RemovalIn {
    pub(crate) fn into_removal(self) -> Result<Removal, ApiError> {
        let account = AccountId::parse(&self.account).api()?;
        Ok(match self.device_id {
            None => Removal::Account(account),
            Some(device) => {
                Removal::Device(account, DeviceId::from_bytes(id16(&device, "device id")?))
            }
        })
    }
}

/// A media as the page sees it: what to show, and its sealed secret.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaInfo {
    blob: ByteBuf,
    size: u64,
    content_type: String,
    name: String,
    secret: ByteBuf,
}

impl MediaInfo {
    fn of(media: &MediaRef, seal: &MediaSeal) -> Result<Self, ApiError> {
        Ok(Self {
            blob: bytes(media.blob.clone()),
            size: media.size,
            content_type: media.content_type.clone(),
            name: media.name.clone(),
            secret: bytes(seal.seal(&EncryptedMedia::of(media)).api()?),
        })
    }
}

/// A media to send: the secret [`crate::Session::encrypt_media`] gave, and where the
/// server stored the blob.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaIn {
    secret: ByteBuf,
    blob: ByteBuf,
    content_type: String,
    #[serde(default)]
    name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EncryptedFile {
    pub(crate) encrypted: ByteBuf,
    pub(crate) size: u64,
    pub(crate) secret: ByteBuf,
}

fn receipt_kind(kind: ReceiptKind) -> &'static str {
    match kind {
        ReceiptKind::Delivered => "delivered",
        ReceiptKind::Read => "read",
    }
}

/// A message body, both ways. (A plain struct rather than a tagged enum: it reads the
/// page's bytes without going through serde's untyped buffer.)
#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BodyDto {
    #[serde(rename = "type")]
    kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reply_to: Option<ByteBuf>,
    #[serde(skip_deserializing, skip_serializing_if = "Option::is_none")]
    media: Option<MediaInfo>,
    #[serde(rename = "media", skip_serializing, default)]
    media_in: Option<MediaIn>,
    #[serde(skip_serializing_if = "Option::is_none")]
    caption: Option<String>,
    #[serde(rename = "kind", skip_serializing_if = "Option::is_none")]
    receipt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    messages: Option<Vec<ByteBuf>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    target: Option<ByteBuf>,
}

fn optional_id(id: Option<ByteBuf>) -> Result<Option<MessageId>, ApiError> {
    id.map(|id| message_id(&id)).transpose()
}

fn required<T>(value: Option<T>, what: &str) -> Result<T, ApiError> {
    value.ok_or_else(|| malformed(what))
}

impl BodyDto {
    pub(crate) fn of(body: &Body, seal: &MediaSeal) -> Result<Self, ApiError> {
        let id = |id: &MessageId| bytes(id.as_bytes().to_vec());
        Ok(match body {
            Body::Text { text, reply_to } => Self {
                kind: "text".into(),
                text: Some(text.clone()),
                reply_to: reply_to.as_ref().map(id),
                ..Self::default()
            },
            Body::Media {
                media,
                caption,
                reply_to,
            } => Self {
                kind: "media".into(),
                media: Some(MediaInfo::of(media, seal)?),
                caption: Some(caption.clone()),
                reply_to: reply_to.as_ref().map(id),
                ..Self::default()
            },
            Body::Receipt { kind, messages } => Self {
                kind: "receipt".into(),
                receipt: Some(receipt_kind(*kind).into()),
                messages: Some(messages.iter().map(id).collect()),
                ..Self::default()
            },
            Body::Edit { target, text } => Self {
                kind: "edit".into(),
                target: Some(id(target)),
                text: Some(text.clone()),
                ..Self::default()
            },
            Body::Delete { target } => Self {
                kind: "delete".into(),
                target: Some(id(target)),
                ..Self::default()
            },
            // The key stays in the core: the page only learns that one was shared.
            Body::ProfileKey { .. } => Self {
                kind: "profileKey".into(),
                ..Self::default()
            },
        })
    }

    pub(crate) fn into_body(self, seal: &MediaSeal) -> Result<Body, ApiError> {
        Ok(match self.kind.as_str() {
            "text" => Body::Text {
                text: required(self.text, "text")?,
                reply_to: optional_id(self.reply_to)?,
            },
            "media" => {
                let media = required(self.media_in, "media")?;
                let secret = seal.open(&media.secret).api()?;
                Body::Media {
                    media: secret.into_ref(media.blob.into_vec(), media.content_type, media.name),
                    caption: self.caption.unwrap_or_default(),
                    reply_to: optional_id(self.reply_to)?,
                }
            }
            "receipt" => Body::Receipt {
                kind: match required(self.receipt, "receipt kind")?.as_str() {
                    "delivered" => ReceiptKind::Delivered,
                    "read" => ReceiptKind::Read,
                    _ => return Err(malformed("receipt kind")),
                },
                messages: required(self.messages, "receipt messages")?
                    .iter()
                    .map(|id| message_id(id))
                    .collect::<Result<_, _>>()?,
            },
            "edit" => Body::Edit {
                target: message_id(&required(self.target, "edit target")?)?,
                text: required(self.text, "text")?,
            },
            "delete" => Body::Delete {
                target: message_id(&required(self.target, "delete target")?)?,
            },
            _ => return Err(malformed("message type")),
        })
    }
}

/// A received message.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageDto {
    id: ByteBuf,
    sent_at: u64,
    body: BodyDto,
}

impl MessageDto {
    fn of(message: &Message, seal: &MediaSeal) -> Result<Self, ApiError> {
        Ok(Self {
            id: bytes(message.id.as_bytes().to_vec()),
            sent_at: message.sent_at,
            body: BodyDto::of(&message.body, seal)?,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Queued {
    pub(crate) message_id: ByteBuf,
    pub(crate) send_id: ByteBuf,
}

impl Queued {
    pub(crate) fn new(message: &MessageId, send: &tessel_core::SendId) -> Self {
        Self {
            message_id: bytes(message.as_bytes().to_vec()),
            send_id: bytes(send.as_bytes().to_vec()),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutboundDto {
    group: ByteBuf,
    send_id: ByteBuf,
    message_id: ByteBuf,
    encrypted: ByteBuf,
}

impl From<Outbound> for OutboundDto {
    fn from(outbound: Outbound) -> Self {
        Self {
            group: bytes(outbound.group.as_bytes()),
            send_id: bytes(outbound.send_id.as_bytes().to_vec()),
            message_id: bytes(outbound.message_id.as_bytes().to_vec()),
            encrypted: bytes(outbound.encrypted),
        }
    }
}

pub(crate) fn ids(ids: &[MessageId]) -> Vec<ByteBuf> {
    ids.iter().map(|id| bytes(id.as_bytes().to_vec())).collect()
}

pub(crate) fn groups(groups: &[GroupId]) -> Vec<ByteBuf> {
    groups.iter().map(|group| bytes(group.as_bytes())).collect()
}

fn effect(effect: Effect) -> (&'static str, Option<&'static str>) {
    match effect {
        Effect::Added => ("added", None),
        Effect::Edited => ("edited", None),
        Effect::Deleted => ("deleted", None),
        Effect::Nothing => ("nothing", None),
        Effect::Ignored(reason) => ("ignored", Some(ignored(reason))),
        Effect::ProfileUpdated => ("profile-updated", None),
        Effect::Receipt => ("receipt", None),
    }
}

/// A profile, both ways (story 03-06).
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileDto {
    name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    avatar: Option<AvatarDto>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AvatarDto {
    content_type: String,
    image: ByteBuf,
}

impl From<Profile> for ProfileDto {
    fn from(profile: Profile) -> Self {
        Self {
            name: profile.name,
            avatar: profile.avatar.map(|avatar| AvatarDto {
                content_type: avatar.content_type,
                image: ByteBuf::from(avatar.image),
            }),
        }
    }
}

impl From<ProfileDto> for Profile {
    fn from(profile: ProfileDto) -> Self {
        Self {
            name: profile.name,
            avatar: profile.avatar.map(|avatar| Avatar {
                content_type: avatar.content_type,
                image: avatar.image.into_vec(),
            }),
        }
    }
}

/// What a received message produced.
#[derive(Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum IncomingDto {
    Message {
        group: ByteBuf,
        sender: DeviceInfo,
        /// Absent when unreadable.
        #[serde(skip_serializing_if = "Option::is_none")]
        message: Option<Box<MessageDto>>,
        /// Why it cannot be read (an error code).
        #[serde(skip_serializing_if = "Option::is_none")]
        unreadable: Option<String>,
        effect: &'static str,
        #[serde(skip_serializing_if = "Option::is_none")]
        ignored: Option<&'static str>,
    },
    Proposal {
        group: ByteBuf,
        sender: DeviceInfo,
    },
    Commit {
        group: ByteBuf,
        sender: DeviceInfo,
        added: Vec<DeviceInfo>,
        removed: Vec<DeviceInfo>,
        removed_self: bool,
        unsent: Vec<ByteBuf>,
        verified_removed: Vec<DeviceInfo>,
        /// The group's new settings, when the commit changed them (story 05-02).
        #[serde(skip_serializing_if = "Option::is_none")]
        settings: Option<GroupSettingsDto>,
    },
}

impl IncomingDto {
    pub(crate) fn of(incoming: Incoming, seal: &MediaSeal) -> Result<Self, ApiError> {
        Ok(match incoming {
            Incoming::Message {
                group,
                sender,
                message,
                effect: change,
            } => {
                let (effect, ignored) = effect(change);
                let (message, unreadable) = match message {
                    Ok(message) => (Some(Box::new(MessageDto::of(&message, seal)?)), None),
                    Err(error) => (None, Some(error.code())),
                };
                Self::Message {
                    group: bytes(group.as_bytes()),
                    sender: DeviceInfo::of(&sender)?,
                    message,
                    unreadable,
                    effect,
                    ignored,
                }
            }
            Incoming::Proposal { group, sender } => Self::Proposal {
                group: bytes(group.as_bytes()),
                sender: DeviceInfo::of(&sender)?,
            },
            Incoming::Commit {
                group,
                sender,
                added,
                removed,
                removed_self,
                unsent,
                verified_removed,
                settings,
            } => Self::Commit {
                group: bytes(group.as_bytes()),
                sender: DeviceInfo::of(&sender)?,
                added: DeviceInfo::all(&added)?,
                removed: DeviceInfo::all(&removed)?,
                removed_self,
                unsent: ids(&unsent),
                verified_removed: DeviceInfo::all(&verified_removed)?,
                settings: settings.map(GroupSettingsDto::from),
            },
        })
    }
}

/// The content of a history message.
#[derive(Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ContentDto {
    Text {
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        reply_to: Option<ByteBuf>,
        #[serde(skip_serializing_if = "Option::is_none")]
        edited_at: Option<u64>,
    },
    Media {
        media: MediaInfo,
        caption: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        reply_to: Option<ByteBuf>,
        #[serde(skip_serializing_if = "Option::is_none")]
        edited_at: Option<u64>,
    },
    Deleted,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntryDto {
    id: ByteBuf,
    author: String,
    device_id: ByteBuf,
    sent_at: u64,
    content: ContentDto,
    /// On this account's messages: who acknowledged it, and how far (story 05-04).
    receipts: Vec<ReceiptDto>,
    /// On this account's messages: `pending`, `sent`, `delivered` or `read`.
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<&'static str>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceiptDto {
    account: String,
    kind: &'static str,
}

fn status(status: Status) -> &'static str {
    match status {
        Status::Pending => "pending",
        Status::Sent => "sent",
        Status::Delivered => "delivered",
        Status::Read => "read",
    }
}

impl HistoryEntryDto {
    pub(crate) fn of(entry: HistoryEntry, seal: &MediaSeal) -> Result<Self, ApiError> {
        let id = |id: MessageId| bytes(id.as_bytes().to_vec());
        Ok(Self {
            id: id(entry.id),
            author: entry.author.to_string(),
            device_id: bytes(entry.device.as_bytes().to_vec()),
            sent_at: entry.sent_at,
            receipts: entry
                .receipts
                .iter()
                .map(|receipt| ReceiptDto {
                    account: receipt.account.to_string(),
                    kind: receipt_kind(receipt.kind),
                })
                .collect(),
            status: entry.status.map(status),
            content: match entry.content {
                Content::Text {
                    text,
                    reply_to,
                    edited_at,
                } => ContentDto::Text {
                    text,
                    reply_to: reply_to.map(id),
                    edited_at,
                },
                Content::Media {
                    media,
                    caption,
                    reply_to,
                    edited_at,
                } => ContentDto::Media {
                    media: MediaInfo::of(&media, seal)?,
                    caption,
                    reply_to: reply_to.map(id),
                    edited_at,
                },
                Content::Deleted => ContentDto::Deleted,
            },
        })
    }
}

#[derive(Serialize)]
pub struct SafetyDto {
    digits: String,
    qr: ByteBuf,
}

impl From<SafetyNumber> for SafetyDto {
    fn from(number: SafetyNumber) -> Self {
        Self {
            digits: number.digits,
            qr: bytes(number.qr),
        }
    }
}

pub(crate) fn trust(trust: Trust) -> &'static str {
    match trust {
        Trust::Unverified => "unverified",
        Trust::Verified => "verified",
        Trust::NewDevice => "new-device",
        Trust::KeyChanged => "key-changed",
    }
}

/// What the granting device hands the server (story 03-04).
#[derive(Serialize)]
pub struct LinkGrantDto {
    /// The server's mailbox for the grant.
    pub(crate) link: ByteBuf,
    /// The new device's certificate, to register it.
    pub(crate) certificate: ByteBuf,
    /// The sealed grant, for the mailbox.
    pub(crate) grant: ByteBuf,
}

/// The public header of an envelope (story 04-01): what the server routes on.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvelopeHeaderDto {
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    group: Option<ByteBuf>,
    /// The epoch fits a JavaScript number for as long as a group lives.
    #[serde(skip_serializing_if = "Option::is_none")]
    epoch: Option<u64>,
}

impl From<EnvelopeHeader> for EnvelopeHeaderDto {
    fn from(header: EnvelopeHeader) -> Self {
        Self {
            kind: match header.kind {
                EnvelopeKind::Application => "application",
                EnvelopeKind::Proposal => "proposal",
                EnvelopeKind::Commit => "commit",
                EnvelopeKind::Welcome => "welcome",
            },
            group: header.group.map(|group| bytes(group.as_bytes())),
            epoch: header.epoch,
        }
    }
}

pub(crate) fn devices(devices: &[DeviceId]) -> Vec<ByteBuf> {
    devices
        .iter()
        .map(|device| bytes(device.as_bytes()))
        .collect()
}

/// A group in the conversation list (story 05-01).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationDto {
    group: ByteBuf,
    /// The group's name; empty for none (story 05-02).
    name: String,
    /// Milliseconds, the sender's clock; absent before any message.
    #[serde(skip_serializing_if = "Option::is_none")]
    last_sent_at: Option<u64>,
    unread: usize,
}

impl From<Conversation> for ConversationDto {
    fn from(conversation: Conversation) -> Self {
        Self {
            group: bytes(conversation.group.as_bytes()),
            name: conversation.name,
            last_sent_at: conversation.last_sent_at,
            unread: conversation.unread,
        }
    }
}

/// A group's settings (story 05-02).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupSettingsDto {
    name: String,
    admins: Vec<String>,
    restricted: bool,
}

impl From<GroupSettings> for GroupSettingsDto {
    fn from(settings: GroupSettings) -> Self {
        Self {
            name: settings.name,
            admins: settings.admins.iter().map(ToString::to_string).collect(),
            restricted: settings.restricted,
        }
    }
}

/// A change to a group's settings, as the page sends it.
#[derive(Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum GroupChangeIn {
    Rename { name: String },
    Admin { account: String, admin: bool },
    Restrict { restricted: bool },
}

impl GroupChangeIn {
    pub(crate) fn into_change(self) -> Result<GroupChange, ApiError> {
        Ok(match self {
            Self::Rename { name } => GroupChange::Rename(name),
            Self::Admin { account, admin } => {
                GroupChange::Admin(AccountId::parse(&account).api()?, admin)
            }
            Self::Restrict { restricted } => GroupChange::Restrict(restricted),
        })
    }
}
