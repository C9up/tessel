//! WASM bindings of `tessel-core` for the web client (story 02-10).
//!
//! A thin layer: every exported function calls `tessel-api`, which the mobile binding
//! shares, and decides nothing. It only converts the page's values to the API's and
//! back. It runs in a Web Worker (`packages/core-web/src/worker.ts`), which persists
//! what [`Core::take_changes`] returns after every call, before answering the page.
//!
//! No private key crosses this boundary. The store key does, once, at startup: only the
//! browser's key store can protect it at rest (story 02-06). A media's key never does:
//! the page holds it sealed (`tessel_core::MediaSeal`).

mod error;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_bytes::ByteBuf;
use tessel_api::Session;
use tessel_api::dto::{BodyDto, GroupChangeIn, ProfileDto, RemovalIn, StoredRecord};
use wasm_bindgen::prelude::{JsError, JsValue, wasm_bindgen};

use crate::error::OrJs as _;

/// Core version.
#[wasm_bindgen]
pub fn version() -> String {
    tessel_api::version().to_owned()
}

/// IANA name of Tessel's MLS suite.
#[wasm_bindgen]
pub fn ciphersuite() -> String {
    tessel_api::ciphersuite().to_owned()
}

/// Checks that the core can work in this browser (suite supported, randomness
/// available). Throws an error otherwise.
#[wasm_bindgen(js_name = selfCheck)]
pub fn self_check() -> Result<(), JsError> {
    tessel_api::self_check().map_err(|e| JsError::new(&e.message))
}

/// Draws a new key for the local database (story 02-06). It is the one secret that
/// crosses this boundary: the page seals it at once with a non-extractable WebCrypto key
/// (`loadStoreKey`) and only hands it back to the core at startup.
#[wasm_bindgen(js_name = generateStoreKey)]
pub fn generate_store_key() -> Result<Vec<u8>, JsError> {
    tessel_api::generate_store_key().map_err(|e| JsError::new(&e.message))
}

/// The partition of the state records, for the host to read them at startup.
#[wasm_bindgen(js_name = statePartition)]
pub fn state_partition(store_key: &[u8]) -> Result<Vec<u8>, JsValue> {
    tessel_api::state_partition(store_key).js()
}

/// Checks a *KeyPackage* fetched from the directory: what device it belongs to.
#[wasm_bindgen(js_name = verifyKeyPackage)]
pub fn verify(key_package: &[u8]) -> Result<JsValue, JsValue> {
    to_js(&tessel_api::verify_key_package(key_package).js()?)
}

/// Checks a device certificate (its account key's signature): what device it names.
/// The server calls it when an account is created (story 03-01).
#[wasm_bindgen(js_name = verifyCertificate)]
pub fn verify_certificate(certificate_bytes: &[u8]) -> Result<JsValue, JsValue> {
    to_js(&tessel_api::verify_certificate(certificate_bytes).js()?)
}

/// The public header of an envelope (story 04-01): kind, group, epoch. The server routes
/// on it and reads nothing else. Throws `envelope:malformed` for anything that is not a
/// group's MLS message.
#[wasm_bindgen(js_name = inspectEnvelope)]
pub fn inspect_envelope(envelope: &[u8]) -> Result<JsValue, JsValue> {
    to_js(&tessel_api::inspect_envelope(envelope).js()?)
}

/// Checks an account deletion (story 03-07): the signature, by `account_key` (the one the
/// caller knows for the account), deleting `account`. Throws otherwise.
#[wasm_bindgen(js_name = verifyAccountDeletion)]
pub fn verify_account_deletion(
    account: &str,
    account_key: &[u8],
    deletion: &[u8],
) -> Result<(), JsValue> {
    tessel_api::verify_account_deletion(account, account_key, deletion).js()
}

/// Checks a device revocation (story 03-05): the signature, by `account_key` (the one
/// the caller knows for the account), revoking `device_id` of `account`. Throws otherwise.
#[wasm_bindgen(js_name = verifyRevocation)]
pub fn verify_revocation(
    account: &str,
    device_id: &[u8],
    account_key: &[u8],
    revocation: &[u8],
) -> Result<(), JsValue> {
    tessel_api::verify_revocation(account, device_id, account_key, revocation).js()
}

/// Checks a sign-in proof (story 03-02): the signature, by the key `certificate_bytes`
/// certifies, of `challenge` for that device. The server calls it with the certificate
/// it keeps for the device, and the challenge it drew.
#[wasm_bindgen(js_name = verifySignIn)]
pub fn verify_sign_in(
    certificate_bytes: &[u8],
    challenge: &[u8],
    proof: &[u8],
) -> Result<(), JsValue> {
    tessel_api::verify_sign_in(certificate_bytes, challenge, proof).js()
}

fn to_js(value: &impl Serialize) -> Result<JsValue, JsValue> {
    value.serialize(&serde_wasm_bindgen::Serializer::new()).js()
}

fn from_js<T: DeserializeOwned>(value: JsValue) -> Result<T, JsValue> {
    serde_wasm_bindgen::from_value(value).js()
}

/// A JS number the API takes as an integer (Unix seconds, milliseconds).
fn integer(value: f64) -> Result<u64, JsValue> {
    from_js::<u64>(JsValue::from_f64(value))
}

/// A new device waiting to be linked to an account by QR code (story 03-04). Its
/// private key stays in here until [`PendingLink::accept`] makes it a [`Core`].
#[wasm_bindgen]
pub struct PendingLink {
    link: tessel_api::PendingLink,
}

#[wasm_bindgen]
impl PendingLink {
    /// A new device's key and identifier, and a fresh link.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<PendingLink, JsValue> {
        Ok(Self {
            link: tessel_api::PendingLink::new().js()?,
        })
    }

    /// What the QR code shows. It holds the link's secret: shown, never sent.
    pub fn qr(&self) -> Result<Vec<u8>, JsValue> {
        self.link.qr().js()
    }

    /// The server's mailbox where the grant arrives.
    pub fn link(&self) -> Result<Vec<u8>, JsValue> {
        self.link.link().js()
    }

    /// Opens the grant and starts the device, with a new database sealed with
    /// `store_key`. Usable once.
    pub fn accept(&mut self, grant: &[u8], store_key: &[u8]) -> Result<Core, JsValue> {
        Ok(Core {
            session: self.link.accept(grant, store_key).js()?,
        })
    }
}

/// A running device: the core's client, and the seal of the media secrets handed to
/// the page.
#[wasm_bindgen]
pub struct Core {
    session: Session,
}

#[wasm_bindgen]
impl Core {
    /// The first device of a new account `name@domain`, with a new database sealed
    /// with `store_key`. Its account key stays in that database.
    #[wasm_bindgen(js_name = createAccount)]
    pub fn create_account(account: &str, store_key: &[u8]) -> Result<Self, JsValue> {
        Ok(Self {
            session: Session::create_account(account, store_key).js()?,
        })
    }

    /// Reopens a device from the records of its state partition.
    pub fn open(store_key: &[u8], state: JsValue) -> Result<Self, JsValue> {
        let state = from_js::<Vec<StoredRecord>>(state)?;
        Ok(Self {
            session: Session::open(store_key, state).js()?,
        })
    }

    /// What to persist, in one transaction, before acting on the last calls' output.
    #[wasm_bindgen(js_name = takeChanges)]
    pub fn take_changes(&mut self) -> Result<JsValue, JsValue> {
        to_js(&self.session.take_changes().js()?)
    }

    /// This device.
    pub fn device(&self) -> Result<JsValue, JsValue> {
        to_js(&self.session.device().js()?)
    }

    /// Signs the server's sign-in challenge with the device key (story 03-02).
    #[wasm_bindgen(js_name = signIn)]
    pub fn sign_in(&self, challenge: &[u8]) -> Result<Vec<u8>, JsValue> {
        self.session.sign_in(challenge).js()
    }

    /// Whether this device holds its account's key.
    #[wasm_bindgen(js_name = holdsAccountKey)]
    pub fn holds_account_key(&self) -> bool {
        self.session.holds_account_key()
    }

    /// Renews the *KeyPackage* pool at `now` (Unix seconds).
    #[wasm_bindgen(js_name = refreshKeyPackages)]
    pub fn refresh_key_packages(&mut self, now: f64) -> Result<JsValue, JsValue> {
        let now = integer(now)?;
        to_js(&self.session.refresh_key_packages(now).js()?)
    }

    #[wasm_bindgen(js_name = createGroup)]
    pub fn create_group(&mut self) -> Result<Vec<u8>, JsValue> {
        self.session.create_group().js()
    }

    /// Adds devices through their *KeyPackages* (as fetched from the directory).
    #[wasm_bindgen(js_name = addDevices)]
    pub fn add_devices(
        &mut self,
        group_id: &[u8],
        key_packages: JsValue,
    ) -> Result<JsValue, JsValue> {
        let key_packages = from_js::<Vec<ByteBuf>>(key_packages)?;
        to_js(&self.session.add_devices(group_id, &key_packages).js()?)
    }

    /// Removes accounts or devices: `[{ account, deviceId? }]`.
    #[wasm_bindgen(js_name = removeDevices)]
    pub fn remove_devices(
        &mut self,
        group_id: &[u8],
        removals: JsValue,
    ) -> Result<JsValue, JsValue> {
        let removals = from_js::<Vec<RemovalIn>>(removals)?;
        to_js(&self.session.remove_devices(group_id, removals).js()?)
    }

    /// Changes the group's settings (story 05-02): `{ type: 'rename', name }`,
    /// `{ type: 'admin', account, admin }` or `{ type: 'restrict', restricted }`.
    #[wasm_bindgen(js_name = configureGroup)]
    pub fn configure_group(
        &mut self,
        group_id: &[u8],
        change: JsValue,
    ) -> Result<JsValue, JsValue> {
        let change = from_js::<GroupChangeIn>(change)?;
        to_js(&self.session.configure_group(group_id, change).js()?)
    }

    /// The group's settings: `{ name, admins, restricted }`.
    #[wasm_bindgen(js_name = groupSettings)]
    pub fn group_settings(&self, group_id: &[u8]) -> Result<JsValue, JsValue> {
        to_js(&self.session.group_settings(group_id).js()?)
    }

    #[wasm_bindgen(js_name = commitPendingProposals)]
    pub fn commit_pending_proposals(&mut self, group_id: &[u8]) -> Result<JsValue, JsValue> {
        to_js(&self.session.commit_pending_proposals(group_id).js()?)
    }

    #[wasm_bindgen(js_name = commitAccepted)]
    pub fn commit_accepted(&mut self, group_id: &[u8]) -> Result<(), JsValue> {
        self.session.commit_accepted(group_id).js()
    }

    #[wasm_bindgen(js_name = commitRejected)]
    pub fn commit_rejected(&mut self, group_id: &[u8]) -> Result<(), JsValue> {
        self.session.commit_rejected(group_id).js()
    }

    #[wasm_bindgen(js_name = retryCommit)]
    pub fn retry_commit(&mut self, group_id: &[u8]) -> Result<JsValue, JsValue> {
        to_js(&self.session.retry_commit(group_id).js()?)
    }

    #[wasm_bindgen(js_name = leaveGroup)]
    pub fn leave_group(&mut self, group_id: &[u8]) -> Result<Vec<u8>, JsValue> {
        self.session.leave_group(group_id).js()
    }

    #[wasm_bindgen(js_name = joinGroup)]
    pub fn join_group(&mut self, welcome: &[u8]) -> Result<Vec<u8>, JsValue> {
        self.session.join_group(welcome).js()
    }

    pub fn members(&self, group_id: &[u8]) -> Result<JsValue, JsValue> {
        to_js(&self.session.members(group_id).js()?)
    }

    pub fn epoch(&self, group_id: &[u8]) -> Result<f64, JsValue> {
        let epoch = self.session.epoch(group_id).js()?;
        // An epoch stays far below 2^53.
        Ok(epoch as f64)
    }

    /// Processes a message received from a group.
    pub fn process(&mut self, message: &[u8]) -> Result<JsValue, JsValue> {
        to_js(&self.session.process(message).js()?)
    }

    /// Writes a message, sent at `sent_at` (milliseconds): recorded in the history and
    /// queued for sending.
    #[wasm_bindgen(js_name = queueMessage)]
    pub fn queue_message(
        &mut self,
        group_id: &[u8],
        body: JsValue,
        sent_at: f64,
    ) -> Result<JsValue, JsValue> {
        let body = from_js::<BodyDto>(body)?;
        let sent_at = integer(sent_at)?;
        to_js(&self.session.queue_message(group_id, body, sent_at).js()?)
    }

    /// The head of the group's send queue, encrypted; `undefined` when empty.
    #[wasm_bindgen(js_name = nextOutbound)]
    pub fn next_outbound(&mut self, group_id: &[u8]) -> Result<JsValue, JsValue> {
        to_js(&self.session.next_outbound(group_id).js()?)
    }

    #[wasm_bindgen(js_name = outboundSent)]
    pub fn outbound_sent(&mut self, group_id: &[u8], send_id: &[u8]) -> Result<(), JsValue> {
        self.session.outbound_sent(group_id, send_id).js()
    }

    /// The devices an envelope for the group goes to: every member but this device
    /// (story 04-01). For a commit, to take before it is accepted.
    pub fn recipients(&self, group_id: &[u8]) -> Result<JsValue, JsValue> {
        to_js(&self.session.recipients(group_id).js()?)
    }

    /// The conversation list, the most recently active first (story 05-01).
    pub fn conversations(&self) -> Result<JsValue, JsValue> {
        to_js(&self.session.conversations())
    }

    /// The user read the group up to its last message.
    #[wasm_bindgen(js_name = markRead)]
    pub fn mark_read(&mut self, group_id: &[u8]) -> Result<(), JsValue> {
        self.session.mark_read(group_id).js()
    }

    /// The groups this device is a member of.
    pub fn groups(&self) -> Result<JsValue, JsValue> {
        to_js(&self.session.groups())
    }

    /// Seals this account's profile under its profile key (story 03-06): the blob for
    /// the server.
    #[wasm_bindgen(js_name = sealProfile)]
    pub fn seal_profile(&self, profile: JsValue) -> Result<Vec<u8>, JsValue> {
        let profile = from_js::<ProfileDto>(profile)?;
        self.session.seal_profile(profile).js()
    }

    /// Opens an account's profile blob, with the key its devices shared.
    #[wasm_bindgen(js_name = openProfile)]
    pub fn open_profile(&self, account: &str, blob: &[u8]) -> Result<JsValue, JsValue> {
        to_js(&self.session.open_profile(account, blob).js()?)
    }

    /// Whether this device can open `account`'s profile.
    #[wasm_bindgen(js_name = knowsProfile)]
    pub fn knows_profile(&self, account: &str) -> Result<bool, JsValue> {
        self.session.knows_profile(account).js()
    }

    /// Queues this account's profile key in the group: its members can open the
    /// profile, and fetch it again. The key never reaches the page.
    #[wasm_bindgen(js_name = shareProfile)]
    pub fn share_profile(&mut self, group_id: &[u8], sent_at: f64) -> Result<Vec<u8>, JsValue> {
        let sent_at = integer(sent_at)?;
        self.session.share_profile(group_id, sent_at).js()
    }

    /// The account key's deletion of this account (story 03-07). Leave every group
    /// first: once the account is gone, the device can no longer reach them.
    #[wasm_bindgen(js_name = signAccountDeletion)]
    pub fn sign_account_deletion(&self) -> Result<Vec<u8>, JsValue> {
        self.session.sign_account_deletion().js()
    }

    /// The account key's revocation of another device of the account (story 03-05).
    #[wasm_bindgen(js_name = revokeDevice)]
    pub fn revoke_device(&self, device_id: &[u8]) -> Result<Vec<u8>, JsValue> {
        self.session.revoke_device(device_id).js()
    }

    /// The groups a device of the account is a member of: to remove it from, once revoked.
    #[wasm_bindgen(js_name = groupsWith)]
    pub fn groups_with(&self, device_id: &[u8]) -> Result<JsValue, JsValue> {
        to_js(&self.session.groups_with(device_id).js()?)
    }

    /// Certifies the device whose link QR code was scanned (story 03-04): its
    /// certificate for the server, and the grant sealed for it.
    #[wasm_bindgen(js_name = grantLink)]
    pub fn grant_link(&self, qr: &[u8]) -> Result<JsValue, JsValue> {
        to_js(&self.session.grant_link(qr).js()?)
    }

    #[wasm_bindgen(js_name = outboundGroups)]
    pub fn outbound_groups(&self) -> Result<JsValue, JsValue> {
        to_js(&self.session.outbound_groups())
    }

    #[wasm_bindgen(js_name = waitingMessages)]
    pub fn waiting_messages(&self, group_id: &[u8]) -> Result<JsValue, JsValue> {
        to_js(&self.session.waiting_messages(group_id))
    }

    /// The partition of a group's history, for the host to read it.
    #[wasm_bindgen(js_name = historyPartition)]
    pub fn history_partition(&self, group_id: &[u8]) -> Result<Vec<u8>, JsValue> {
        self.session.history_partition(group_id).js()
    }

    /// The group's messages, from the records of its history partition.
    #[wasm_bindgen(js_name = readHistory)]
    pub fn read_history(&self, group_id: &[u8], history: JsValue) -> Result<JsValue, JsValue> {
        let history = from_js::<Vec<StoredRecord>>(history)?;
        to_js(&self.session.read_history(group_id, history).js()?)
    }

    /// Encrypts a file before upload; its secret comes back sealed.
    #[wasm_bindgen(js_name = encryptMedia)]
    pub fn encrypt_media(&self, file: &[u8]) -> Result<JsValue, JsValue> {
        to_js(&self.session.encrypt_media(file).js()?)
    }

    /// Decrypts a downloaded blob with the sealed secret of its media.
    #[wasm_bindgen(js_name = decryptMedia)]
    pub fn decrypt_media(&self, secret: &[u8], encrypted: &[u8]) -> Result<Vec<u8>, JsValue> {
        self.session.decrypt_media(secret, encrypted).js()
    }

    #[wasm_bindgen(js_name = safetyNumber)]
    pub fn safety_number(&self, contact: &[u8]) -> Result<JsValue, JsValue> {
        to_js(&self.session.safety_number(contact).js()?)
    }

    #[wasm_bindgen(js_name = checkScanned)]
    pub fn check_scanned(&self, contact: &[u8], scanned: &[u8]) -> Result<(), JsValue> {
        self.session.check_scanned(contact, scanned).js()
    }

    #[wasm_bindgen(js_name = markVerified)]
    pub fn mark_verified(&mut self, contact: &[u8]) -> Result<(), JsValue> {
        self.session.mark_verified(contact).js()
    }

    pub fn unverify(&mut self, account: &str) -> Result<(), JsValue> {
        self.session.unverify(account).js()
    }

    pub fn trust(&self, device: &[u8]) -> Result<String, JsValue> {
        Ok(self.session.trust(device).js()?.to_owned())
    }

    #[wasm_bindgen(js_name = acknowledgeDevice)]
    pub fn acknowledge_device(&mut self, device: &[u8]) -> Result<(), JsValue> {
        self.session.acknowledge_device(device).js()
    }
}
