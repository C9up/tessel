//! The API of `tessel-core` that every binding exposes (story 02-13): the web's WASM
//! binding (`tessel-wasm`) and the mobile one share it, so both offer exactly the same
//! calls, arguments, results and error codes.
//!
//! A thin layer: every function calls the core and decides nothing. Its inputs and
//! results are the plain objects of bytes, strings and numbers of [`dto`]; its errors an
//! [`ApiError`] with a stable code ([`error`]). A binding only converts them to and from
//! its platform's values.
//!
//! No private key crosses this boundary. The store key does, once, at startup: only the
//! platform's key store can protect it at rest (story 02-06). A media's key never does:
//! the page holds it sealed ([`tessel_core::MediaSeal`]).

pub mod dto;
pub mod error;

use serde_bytes::ByteBuf;
use tessel_core::{
    AccountId, Changes, Client, DeviceId, GroupId, LinkRequest, MediaSeal, Message, Profile,
    Record, SendId, StoreKey, decrypt_media, encrypt_media,
};

use crate::dto::{
    BodyDto, Commit, ConversationDto, DeviceInfo, EncryptedFile, EnvelopeHeaderDto, GroupChangeIn,
    GroupSettingsDto, HistoryEntryDto, IncomingDto, KeyPackageInfo, KeyPackages, LinkGrantDto,
    OutboundDto, ProfileDto, Queued, RemovalIn, SafetyDto, StoreChanges, StoredRecord, certificate,
    id16,
};
pub use crate::error::{ApiError, Code};
use crate::error::{OrApi as _, fail};

/// Core version.
pub fn version() -> &'static str {
    tessel_core::version()
}

/// IANA name of Tessel's MLS suite.
pub fn ciphersuite() -> &'static str {
    tessel_core::CIPHERSUITE_NAME
}

/// Checks that the core can work on this host (suite supported, randomness
/// available). Fails otherwise.
pub fn self_check() -> Result<(), ApiError> {
    tessel_core::self_check().api()
}

/// Draws a new key for the local database (story 02-06). It is the one secret that
/// crosses this boundary: the page seals it at once with a non-extractable WebCrypto key
/// (`loadStoreKey`) and only hands it back to the core at startup.
pub fn generate_store_key() -> Result<Vec<u8>, ApiError> {
    let key = StoreKey::generate().api()?;
    Ok(key.to_bytes().to_vec())
}

/// The partition of the state records, for the host to read them at startup.
pub fn state_partition(store_key: &[u8]) -> Result<Vec<u8>, ApiError> {
    let key = StoreKey::from_bytes(store_key).api()?;
    Ok(key.state_partition().api()?.as_bytes().to_vec())
}

/// Checks a *KeyPackage* fetched from the directory: what device it belongs to.
pub fn verify_key_package(key_package: &[u8]) -> Result<KeyPackageInfo, ApiError> {
    let verified = tessel_core::verify_key_package(key_package).api()?;
    Ok(KeyPackageInfo {
        device: DeviceInfo::of(verified.certificate())?,
        reference: ByteBuf::from(verified.reference().to_vec()),
        last_resort: verified.is_last_resort(),
        not_after: verified.not_after(),
    })
}

/// Checks a device certificate (its account key's signature): what device it names.
/// The server calls it when an account is created (story 03-01).
pub fn verify_certificate(certificate_bytes: &[u8]) -> Result<DeviceInfo, ApiError> {
    DeviceInfo::of(&certificate(certificate_bytes)?)
}

/// The public header of an envelope (story 04-01): kind, group, epoch. The server routes
/// on it and reads nothing else. Fails with `envelope:malformed` for anything that is not
/// a group's MLS message.
pub fn inspect_envelope(envelope: &[u8]) -> Result<EnvelopeHeaderDto, ApiError> {
    Ok(EnvelopeHeaderDto::from(
        tessel_core::inspect_envelope(envelope).api()?,
    ))
}

/// Checks an account deletion (story 03-07): the signature, by `account_key` (the one the
/// caller knows for the account), deleting `account`. Fails otherwise.
pub fn verify_account_deletion(
    account: &str,
    account_key: &[u8],
    deletion: &[u8],
) -> Result<(), ApiError> {
    let account = AccountId::parse(account).api()?;
    tessel_core::verify_account_deletion(&account, account_key, deletion).api()
}

/// Checks a device revocation (story 03-05): the signature, by `account_key` (the one
/// the caller knows for the account), revoking `device_id` of `account`. Fails otherwise.
pub fn verify_revocation(
    account: &str,
    device_id: &[u8],
    account_key: &[u8],
    revocation: &[u8],
) -> Result<(), ApiError> {
    let account = AccountId::parse(account).api()?;
    let device = DeviceId::from_bytes(id16(device_id, "device id")?);
    tessel_core::verify_revocation(&account, device, account_key, revocation).api()
}

/// Checks a sign-in proof (story 03-02): the signature, by the key `certificate_bytes`
/// certifies, of `challenge` for that device. The server calls it with the certificate
/// it keeps for the device, and the challenge it drew.
pub fn verify_sign_in(
    certificate_bytes: &[u8],
    challenge: &[u8],
    proof: &[u8],
) -> Result<(), ApiError> {
    certificate(certificate_bytes)?
        .verify_sign_in(challenge, proof)
        .api()
}

fn records(records: Vec<StoredRecord>) -> Result<Vec<Record>, ApiError> {
    records.into_iter().map(StoredRecord::into_record).collect()
}

fn group(value: &[u8]) -> GroupId {
    GroupId::from_bytes(value)
}

fn link_used() -> ApiError {
    fail("link:used", "the link was already accepted")
}

/// A new device waiting to be linked to an account by QR code (story 03-04). Its
/// private key stays in here until [`PendingLink::accept`] makes it a [`Session`].
pub struct PendingLink {
    request: Option<LinkRequest>,
}

impl PendingLink {
    /// A new device's key and identifier, and a fresh link.
    pub fn new() -> Result<Self, ApiError> {
        Ok(Self {
            request: Some(LinkRequest::new().api()?),
        })
    }

    fn request(&self) -> Result<&LinkRequest, ApiError> {
        self.request.as_ref().ok_or_else(link_used)
    }

    /// What the QR code shows. It holds the link's secret: shown, never sent.
    pub fn qr(&self) -> Result<Vec<u8>, ApiError> {
        Ok(self.request()?.qr().to_vec())
    }

    /// The server's mailbox where the grant arrives.
    pub fn link(&self) -> Result<Vec<u8>, ApiError> {
        Ok(self.request()?.link().to_vec())
    }

    /// Opens the grant and starts the device, with a new database sealed with
    /// `store_key`. Usable once.
    pub fn accept(&mut self, grant: &[u8], store_key: &[u8]) -> Result<Session, ApiError> {
        let request = self.request.take().ok_or_else(link_used)?;
        let key = StoreKey::from_bytes(store_key).api()?;
        Session::start(request.accept(grant, &key).api()?)
    }
}

/// A running device: the core's client, and the seal of the media secrets handed to
/// the page.
pub struct Session {
    client: Client,
    seal: MediaSeal,
}

impl Session {
    fn start(client: Client) -> Result<Self, ApiError> {
        Ok(Self {
            client,
            seal: MediaSeal::new().api()?,
        })
    }

    /// The first device of a new account `name@domain`, with a new database sealed
    /// with `store_key`. Its account key stays in that database.
    pub fn create_account(account: &str, store_key: &[u8]) -> Result<Self, ApiError> {
        let account = AccountId::parse(account).api()?;
        let key = StoreKey::from_bytes(store_key).api()?;
        Self::start(Client::create_account(&account, &key).api()?)
    }

    /// Reopens a device from the records of its state partition.
    pub fn open(store_key: &[u8], state: Vec<StoredRecord>) -> Result<Self, ApiError> {
        let key = StoreKey::from_bytes(store_key).api()?;
        Self::start(Client::open(&key, &records(state)?).api()?)
    }

    /// What to persist, in one transaction, before acting on the last calls' output.
    pub fn take_changes(&mut self) -> Result<StoreChanges, ApiError> {
        Ok(StoreChanges::from(self.client.take_changes().api()?))
    }

    /// For a host that keeps the core's own store (the phone's SQLite, story 02-13):
    /// reopens a device from the records of its state partition, as they are stored.
    pub fn open_records(store_key: &[u8], state: &[Record]) -> Result<Self, ApiError> {
        let key = StoreKey::from_bytes(store_key).api()?;
        Self::start(Client::open(&key, state).api()?)
    }

    /// For such a host: what to persist, as the core's store applies it.
    pub fn take_store_changes(&mut self) -> Result<Changes, ApiError> {
        self.client.take_changes().api()
    }

    /// For such a host: the group's messages, from its history partition's records.
    pub fn read_history_records(
        &self,
        group_id: &[u8],
        history: &[Record],
    ) -> Result<Vec<HistoryEntryDto>, ApiError> {
        let entries = self.client.read_history(&group(group_id), history).api()?;
        entries
            .into_iter()
            .map(|entry| HistoryEntryDto::of(entry, &self.seal))
            .collect()
    }

    /// This device.
    pub fn device(&self) -> Result<DeviceInfo, ApiError> {
        DeviceInfo::of(self.client.device().certificate())
    }

    /// Signs the server's sign-in challenge with the device key (story 03-02).
    pub fn sign_in(&self, challenge: &[u8]) -> Result<Vec<u8>, ApiError> {
        self.client.device().sign_in(challenge).api()
    }

    /// Whether this device holds its account's key.
    pub fn holds_account_key(&self) -> bool {
        self.client.holds_account_key()
    }

    /// Renews the *KeyPackage* pool at `now` (Unix seconds).
    pub fn refresh_key_packages(&mut self, now: u64) -> Result<KeyPackages, ApiError> {
        Ok(KeyPackages::from(
            self.client.refresh_key_packages(now).api()?,
        ))
    }

    pub fn create_group(&mut self) -> Result<Vec<u8>, ApiError> {
        Ok(self.client.create_group().api()?.as_bytes().to_vec())
    }

    /// Adds devices through their *KeyPackages* (as fetched from the directory).
    pub fn add_devices(
        &mut self,
        group_id: &[u8],
        key_packages: &[ByteBuf],
    ) -> Result<Commit, ApiError> {
        let key_packages = key_packages
            .iter()
            .map(|bytes| tessel_core::verify_key_package(bytes).api())
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Commit::from(
            self.client
                .add_devices(&group(group_id), &key_packages)
                .api()?,
        ))
    }

    /// Removes accounts or devices: `[{ account, deviceId? }]`.
    pub fn remove_devices(
        &mut self,
        group_id: &[u8],
        removals: Vec<RemovalIn>,
    ) -> Result<Commit, ApiError> {
        let removals = removals
            .into_iter()
            .map(RemovalIn::into_removal)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Commit::from(
            self.client
                .remove_devices(&group(group_id), &removals)
                .api()?,
        ))
    }

    /// Changes the group's settings (story 05-02): `{ type: 'rename', name }`,
    /// `{ type: 'admin', account, admin }` or `{ type: 'restrict', restricted }`.
    pub fn configure_group(
        &mut self,
        group_id: &[u8],
        change: GroupChangeIn,
    ) -> Result<Commit, ApiError> {
        Ok(Commit::from(
            self.client
                .configure_group(&group(group_id), change.into_change()?)
                .api()?,
        ))
    }

    /// The group's settings: its name, its admins, whether membership is reserved to them.
    pub fn group_settings(&self, group_id: &[u8]) -> Result<GroupSettingsDto, ApiError> {
        Ok(GroupSettingsDto::from(
            self.client.group_settings(&group(group_id)).api()?,
        ))
    }

    pub fn commit_pending_proposals(&mut self, group_id: &[u8]) -> Result<Commit, ApiError> {
        Ok(Commit::from(
            self.client
                .commit_pending_proposals(&group(group_id))
                .api()?,
        ))
    }

    pub fn commit_accepted(&mut self, group_id: &[u8]) -> Result<(), ApiError> {
        self.client.commit_accepted(&group(group_id)).api()
    }

    pub fn commit_rejected(&mut self, group_id: &[u8]) -> Result<(), ApiError> {
        self.client.commit_rejected(&group(group_id)).api()
    }

    pub fn retry_commit(&mut self, group_id: &[u8]) -> Result<Option<Commit>, ApiError> {
        Ok(self
            .client
            .retry_commit(&group(group_id))
            .api()?
            .map(Commit::from))
    }

    pub fn leave_group(&mut self, group_id: &[u8]) -> Result<Vec<u8>, ApiError> {
        self.client.leave_group(&group(group_id)).api()
    }

    pub fn join_group(&mut self, welcome: &[u8]) -> Result<Vec<u8>, ApiError> {
        Ok(self.client.join_group(welcome).api()?.as_bytes().to_vec())
    }

    pub fn members(&self, group_id: &[u8]) -> Result<Vec<DeviceInfo>, ApiError> {
        DeviceInfo::all(&self.client.members(&group(group_id)).api()?)
    }

    pub fn epoch(&self, group_id: &[u8]) -> Result<u64, ApiError> {
        self.client.epoch(&group(group_id)).api()
    }

    /// Processes a message received from a group.
    pub fn process(&mut self, message: &[u8]) -> Result<IncomingDto, ApiError> {
        let incoming = self.client.process(message).api()?;
        IncomingDto::of(incoming, &self.seal)
    }

    /// Writes a message, sent at `sent_at` (milliseconds): recorded in the history and
    /// queued for sending.
    pub fn queue_message(
        &mut self,
        group_id: &[u8],
        body: BodyDto,
        sent_at: u64,
    ) -> Result<Queued, ApiError> {
        let body = body.into_body(&self.seal)?;
        let message = Message::new(sent_at, body).api()?;
        let send_id = self
            .client
            .queue_message(&group(group_id), &message)
            .api()?;
        Ok(Queued::new(&message.id, &send_id))
    }

    /// The head of the group's send queue, encrypted; `None` when empty.
    pub fn next_outbound(&mut self, group_id: &[u8]) -> Result<Option<OutboundDto>, ApiError> {
        Ok(self
            .client
            .next_outbound(&group(group_id))
            .api()?
            .map(OutboundDto::from))
    }

    pub fn outbound_sent(&mut self, group_id: &[u8], send_id: &[u8]) -> Result<(), ApiError> {
        let send_id = SendId::from_bytes(id16(send_id, "send id")?);
        self.client.outbound_sent(&group(group_id), &send_id).api()
    }

    /// The devices an envelope for the group goes to: every member but this device
    /// (story 04-01). For a commit, to take before it is accepted.
    pub fn recipients(&self, group_id: &[u8]) -> Result<Vec<ByteBuf>, ApiError> {
        Ok(dto::devices(
            &self.client.recipients(&group(group_id)).api()?,
        ))
    }

    /// The conversation list, the most recently active first (story 05-01).
    pub fn conversations(&self) -> Vec<ConversationDto> {
        self.client
            .conversations()
            .into_iter()
            .map(ConversationDto::from)
            .collect()
    }

    /// The user read the group up to its last message.
    pub fn mark_read(&mut self, group_id: &[u8]) -> Result<(), ApiError> {
        self.client.mark_read(&group(group_id)).api()
    }

    /// The groups this device is a member of.
    pub fn groups(&self) -> Vec<ByteBuf> {
        dto::groups(&self.client.groups())
    }

    /// Seals this account's profile under its profile key (story 03-06): the blob for
    /// the server.
    pub fn seal_profile(&self, profile: ProfileDto) -> Result<Vec<u8>, ApiError> {
        let profile: Profile = profile.into();
        self.client.seal_profile(&profile).api()
    }

    /// Opens an account's profile blob, with the key its devices shared.
    pub fn open_profile(&self, account: &str, blob: &[u8]) -> Result<ProfileDto, ApiError> {
        let account = AccountId::parse(account).api()?;
        Ok(ProfileDto::from(
            self.client.open_profile(&account, blob).api()?,
        ))
    }

    /// Whether this device can open `account`'s profile.
    pub fn knows_profile(&self, account: &str) -> Result<bool, ApiError> {
        Ok(self.client.knows_profile(&AccountId::parse(account).api()?))
    }

    /// Queues this account's profile key in the group: its members can open the
    /// profile, and fetch it again. The key never reaches the page.
    pub fn share_profile(&mut self, group_id: &[u8], sent_at: u64) -> Result<Vec<u8>, ApiError> {
        let send_id = self.client.share_profile(&group(group_id), sent_at).api()?;
        // Its send identifier only: it has no history entry for the page to show.
        Ok(send_id.as_bytes().to_vec())
    }

    /// The account key's deletion of this account (story 03-07). Leave every group
    /// first: once the account is gone, the device can no longer reach them.
    pub fn sign_account_deletion(&self) -> Result<Vec<u8>, ApiError> {
        self.client.sign_account_deletion().api()
    }

    /// The account key's revocation of another device of the account (story 03-05).
    pub fn revoke_device(&self, device_id: &[u8]) -> Result<Vec<u8>, ApiError> {
        let device = DeviceId::from_bytes(id16(device_id, "device id")?);
        self.client.revoke_device(device).api()
    }

    /// The groups a device of the account is a member of: to remove it from, once revoked.
    pub fn groups_with(&self, device_id: &[u8]) -> Result<Vec<ByteBuf>, ApiError> {
        let device = DeviceId::from_bytes(id16(device_id, "device id")?);
        Ok(dto::groups(&self.client.groups_with(device)))
    }

    /// Certifies the device whose link QR code was scanned (story 03-04): its
    /// certificate for the server, and the grant sealed for it.
    pub fn grant_link(&self, qr: &[u8]) -> Result<LinkGrantDto, ApiError> {
        let grant = self.client.grant_link(qr).api()?;
        Ok(LinkGrantDto {
            link: ByteBuf::from(grant.link.to_vec()),
            certificate: ByteBuf::from(grant.certificate.to_bytes().api()?),
            grant: ByteBuf::from(grant.sealed),
        })
    }

    pub fn outbound_groups(&self) -> Vec<ByteBuf> {
        dto::groups(&self.client.outbound_groups())
    }

    pub fn waiting_messages(&self, group_id: &[u8]) -> Vec<ByteBuf> {
        dto::ids(&self.client.waiting_messages(&group(group_id)))
    }

    /// The partition of a group's history, for the host to read it.
    pub fn history_partition(&self, group_id: &[u8]) -> Result<Vec<u8>, ApiError> {
        Ok(self
            .client
            .history_partition(&group(group_id))
            .api()?
            .as_bytes()
            .to_vec())
    }

    /// The group's messages, from the records of its history partition.
    pub fn read_history(
        &self,
        group_id: &[u8],
        history: Vec<StoredRecord>,
    ) -> Result<Vec<HistoryEntryDto>, ApiError> {
        let entries = self
            .client
            .read_history(&group(group_id), &records(history)?)
            .api()?;
        entries
            .into_iter()
            .map(|entry| HistoryEntryDto::of(entry, &self.seal))
            .collect()
    }

    /// Encrypts a file before upload; its secret comes back sealed.
    pub fn encrypt_media(&self, file: &[u8]) -> Result<EncryptedFile, ApiError> {
        let (encrypted, media) = encrypt_media(file).api()?;
        Ok(EncryptedFile {
            size: media.size(),
            secret: ByteBuf::from(self.seal.seal(&media).api()?),
            encrypted: ByteBuf::from(encrypted),
        })
    }

    /// Decrypts a downloaded blob with the sealed secret of its media.
    pub fn decrypt_media(&self, secret: &[u8], encrypted: &[u8]) -> Result<Vec<u8>, ApiError> {
        // Only the key, the digest and the size matter to decrypt.
        let media =
            self.seal
                .open(secret)
                .api()?
                .into_ref(Vec::new(), String::new(), String::new());
        decrypt_media(&media, encrypted).api()
    }

    pub fn safety_number(&self, contact: &[u8]) -> Result<SafetyDto, ApiError> {
        Ok(SafetyDto::from(
            self.client.safety_number(&certificate(contact)?),
        ))
    }

    pub fn check_scanned(&self, contact: &[u8], scanned: &[u8]) -> Result<(), ApiError> {
        self.client
            .check_scanned(&certificate(contact)?, scanned)
            .api()
    }

    pub fn mark_verified(&mut self, contact: &[u8]) -> Result<(), ApiError> {
        self.client.mark_verified(&certificate(contact)?).api()
    }

    pub fn unverify(&mut self, account: &str) -> Result<(), ApiError> {
        let account = AccountId::parse(account).api()?;
        self.client.unverify(&account).api()
    }

    /// The trust of a device: `unverified`, `verified`, `new-device` or `key-changed`.
    pub fn trust(&self, device: &[u8]) -> Result<&'static str, ApiError> {
        Ok(dto::trust(self.client.trust(&certificate(device)?)))
    }

    pub fn acknowledge_device(&mut self, device: &[u8]) -> Result<(), ApiError> {
        self.client.acknowledge_device(&certificate(device)?).api()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store_key() -> Vec<u8> {
        generate_store_key().expect("store key")
    }

    fn code<T>(result: Result<T, ApiError>) -> String {
        match result {
            Ok(_) => panic!("expected an error"),
            Err(error) => error.code,
        }
    }

    #[test]
    fn errors_keep_their_code_and_message() {
        let error = match verify_revocation("alice@example.org", &[0; 3], &[], &[]) {
            Ok(()) => panic!("expected an error"),
            Err(error) => error,
        };
        assert_eq!(error.code, "input:malformed");
        assert_eq!(error.message, "malformed device id");
        assert_eq!(error.to_string(), "malformed device id");
        assert_eq!(
            code(Session::create_account("not an account", &store_key())),
            "identity:invalid-account"
        );
        assert_eq!(code(state_partition(&[1, 2, 3])), "store:invalid-key");
        assert_eq!(code(inspect_envelope(&[0xff])), "envelope:malformed");
    }

    #[test]
    fn a_pending_link_is_accepted_once() {
        let mut link = PendingLink::new().expect("link");
        assert_eq!(link.qr().expect("qr").len(), tessel_core::LINK_QR_LEN);
        assert_eq!(code(link.accept(&[0; 8], &store_key())), "link:bad-grant");
        assert_eq!(code(link.qr()), "link:used");
        assert_eq!(code(link.accept(&[0; 8], &store_key())), "link:used");
    }
}
