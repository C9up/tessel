//! WASM bindings of `tessel-core` for the web client (story 02-10).
//!
//! A thin layer: every exported function calls the core and decides nothing. It runs in
//! a Web Worker (`packages/core-web/src/worker.ts`), which persists what
//! [`Core::take_changes`] returns after every call, before answering the page.
//!
//! No private key crosses this boundary. The store key does, once, at startup: only the
//! browser's key store can protect it at rest (story 02-06). A media's key never does:
//! the page holds it sealed ([`tessel_core::MediaSeal`]).

mod dto;
mod error;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_bytes::ByteBuf;
use tessel_core::{
    AccountId, Client, GroupId, MediaSeal, Message, Record, SendId, StoreKey, decrypt_media,
    encrypt_media, verify_key_package,
};
use wasm_bindgen::prelude::{JsError, JsValue, wasm_bindgen};

use crate::dto::{
    BodyDto, Commit, DeviceInfo, EncryptedFile, HistoryEntryDto, IncomingDto, KeyPackageInfo,
    KeyPackages, OutboundDto, Queued, RemovalIn, SafetyDto, StoreChanges, StoredRecord,
    certificate, id16,
};
use crate::error::OrJs as _;

/// Core version.
#[wasm_bindgen]
pub fn version() -> String {
    tessel_core::version().to_owned()
}

/// IANA name of Tessel's MLS suite.
#[wasm_bindgen]
pub fn ciphersuite() -> String {
    tessel_core::CIPHERSUITE_NAME.to_owned()
}

/// Checks that the core can work in this browser (suite supported, randomness
/// available). Throws an error otherwise.
#[wasm_bindgen(js_name = selfCheck)]
pub fn self_check() -> Result<(), JsError> {
    tessel_core::self_check().map_err(|e| JsError::new(&e.to_string()))
}

/// Draws a new key for the local database (story 02-06). It is the one secret that
/// crosses this boundary: the page seals it at once with a non-extractable WebCrypto key
/// (`loadStoreKey`) and only hands it back to the core at startup.
#[wasm_bindgen(js_name = generateStoreKey)]
pub fn generate_store_key() -> Result<Vec<u8>, JsError> {
    let key = tessel_core::StoreKey::generate().map_err(|e| JsError::new(&e.to_string()))?;
    Ok(key.to_bytes().to_vec())
}

/// The partition of the state records, for the host to read them at startup.
#[wasm_bindgen(js_name = statePartition)]
pub fn state_partition(store_key: &[u8]) -> Result<Vec<u8>, JsValue> {
    let key = StoreKey::from_bytes(store_key).js()?;
    Ok(key.state_partition().js()?.as_bytes().to_vec())
}

/// Checks a *KeyPackage* fetched from the directory: what device it belongs to.
#[wasm_bindgen(js_name = verifyKeyPackage)]
pub fn verify(key_package: &[u8]) -> Result<JsValue, JsValue> {
    let verified = verify_key_package(key_package).js()?;
    to_js(&KeyPackageInfo {
        device: DeviceInfo::of(verified.certificate())?,
        reference: ByteBuf::from(verified.reference().to_vec()),
        last_resort: verified.is_last_resort(),
    })
}

fn to_js(value: &impl Serialize) -> Result<JsValue, JsValue> {
    value.serialize(&serde_wasm_bindgen::Serializer::new()).js()
}

fn from_js<T: DeserializeOwned>(value: JsValue) -> Result<T, JsValue> {
    serde_wasm_bindgen::from_value(value).js()
}

fn records(value: JsValue) -> Result<Vec<Record>, JsValue> {
    from_js::<Vec<StoredRecord>>(value)?
        .into_iter()
        .map(StoredRecord::into_record)
        .collect()
}

fn group(value: &[u8]) -> GroupId {
    GroupId::from_bytes(value)
}

/// A running device: the core's client, and the seal of the media secrets handed to
/// the page.
#[wasm_bindgen]
pub struct Core {
    client: Client,
    seal: MediaSeal,
}

#[wasm_bindgen]
impl Core {
    fn start(client: Client) -> Result<Self, JsValue> {
        Ok(Self {
            client,
            seal: MediaSeal::new().js()?,
        })
    }

    /// The first device of a new account `name@domain`, with a new database sealed
    /// with `store_key`. Its account key stays in that database.
    #[wasm_bindgen(js_name = createAccount)]
    pub fn create_account(account: &str, store_key: &[u8]) -> Result<Self, JsValue> {
        let account = AccountId::parse(account).js()?;
        let key = StoreKey::from_bytes(store_key).js()?;
        Self::start(Client::create_account(&account, &key).js()?)
    }

    /// Reopens a device from the records of its state partition.
    pub fn open(store_key: &[u8], state: JsValue) -> Result<Self, JsValue> {
        let key = StoreKey::from_bytes(store_key).js()?;
        Self::start(Client::open(&key, &records(state)?).js()?)
    }

    /// What to persist, in one transaction, before acting on the last calls' output.
    #[wasm_bindgen(js_name = takeChanges)]
    pub fn take_changes(&mut self) -> Result<JsValue, JsValue> {
        to_js(&StoreChanges::from(self.client.take_changes().js()?))
    }

    /// This device.
    pub fn device(&self) -> Result<JsValue, JsValue> {
        to_js(&DeviceInfo::of(self.client.device().certificate())?)
    }

    /// Whether this device holds its account's key.
    #[wasm_bindgen(js_name = holdsAccountKey)]
    pub fn holds_account_key(&self) -> bool {
        self.client.holds_account_key()
    }

    /// Renews the *KeyPackage* pool at `now` (Unix seconds).
    #[wasm_bindgen(js_name = refreshKeyPackages)]
    pub fn refresh_key_packages(&mut self, now: f64) -> Result<JsValue, JsValue> {
        let now = from_js::<u64>(JsValue::from_f64(now))?;
        to_js(&KeyPackages::from(
            self.client.refresh_key_packages(now).js()?,
        ))
    }

    #[wasm_bindgen(js_name = createGroup)]
    pub fn create_group(&mut self) -> Result<Vec<u8>, JsValue> {
        Ok(self.client.create_group().js()?.as_bytes().to_vec())
    }

    /// Adds devices through their *KeyPackages* (as fetched from the directory).
    #[wasm_bindgen(js_name = addDevices)]
    pub fn add_devices(
        &mut self,
        group_id: &[u8],
        key_packages: JsValue,
    ) -> Result<JsValue, JsValue> {
        let key_packages = from_js::<Vec<ByteBuf>>(key_packages)?
            .iter()
            .map(|bytes| verify_key_package(bytes).js())
            .collect::<Result<Vec<_>, _>>()?;
        to_js(&Commit::from(
            self.client
                .add_devices(&group(group_id), &key_packages)
                .js()?,
        ))
    }

    /// Removes accounts or devices: `[{ account, deviceId? }]`.
    #[wasm_bindgen(js_name = removeDevices)]
    pub fn remove_devices(
        &mut self,
        group_id: &[u8],
        removals: JsValue,
    ) -> Result<JsValue, JsValue> {
        let removals = from_js::<Vec<RemovalIn>>(removals)?
            .into_iter()
            .map(RemovalIn::into_removal)
            .collect::<Result<Vec<_>, _>>()?;
        to_js(&Commit::from(
            self.client
                .remove_devices(&group(group_id), &removals)
                .js()?,
        ))
    }

    #[wasm_bindgen(js_name = commitPendingProposals)]
    pub fn commit_pending_proposals(&mut self, group_id: &[u8]) -> Result<JsValue, JsValue> {
        to_js(&Commit::from(
            self.client
                .commit_pending_proposals(&group(group_id))
                .js()?,
        ))
    }

    #[wasm_bindgen(js_name = commitAccepted)]
    pub fn commit_accepted(&mut self, group_id: &[u8]) -> Result<(), JsValue> {
        self.client.commit_accepted(&group(group_id)).js()
    }

    #[wasm_bindgen(js_name = commitRejected)]
    pub fn commit_rejected(&mut self, group_id: &[u8]) -> Result<(), JsValue> {
        self.client.commit_rejected(&group(group_id)).js()
    }

    #[wasm_bindgen(js_name = retryCommit)]
    pub fn retry_commit(&mut self, group_id: &[u8]) -> Result<JsValue, JsValue> {
        to_js(
            &self
                .client
                .retry_commit(&group(group_id))
                .js()?
                .map(Commit::from),
        )
    }

    #[wasm_bindgen(js_name = leaveGroup)]
    pub fn leave_group(&mut self, group_id: &[u8]) -> Result<Vec<u8>, JsValue> {
        self.client.leave_group(&group(group_id)).js()
    }

    #[wasm_bindgen(js_name = joinGroup)]
    pub fn join_group(&mut self, welcome: &[u8]) -> Result<Vec<u8>, JsValue> {
        Ok(self.client.join_group(welcome).js()?.as_bytes().to_vec())
    }

    pub fn members(&self, group_id: &[u8]) -> Result<JsValue, JsValue> {
        to_js(&DeviceInfo::all(
            &self.client.members(&group(group_id)).js()?,
        )?)
    }

    pub fn epoch(&self, group_id: &[u8]) -> Result<f64, JsValue> {
        let epoch = self.client.epoch(&group(group_id)).js()?;
        // An epoch stays far below 2^53.
        Ok(epoch as f64)
    }

    /// Processes a message received from a group.
    pub fn process(&mut self, message: &[u8]) -> Result<JsValue, JsValue> {
        let incoming = self.client.process(message).js()?;
        to_js(&IncomingDto::of(incoming, &self.seal)?)
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
        let body = from_js::<BodyDto>(body)?.into_body(&self.seal)?;
        let sent_at = from_js::<u64>(JsValue::from_f64(sent_at))?;
        let message = Message::new(sent_at, body).js()?;
        let send_id = self.client.queue_message(&group(group_id), &message).js()?;
        to_js(&Queued::new(&message.id, &send_id))
    }

    /// The head of the group's send queue, encrypted; `undefined` when empty.
    #[wasm_bindgen(js_name = nextOutbound)]
    pub fn next_outbound(&mut self, group_id: &[u8]) -> Result<JsValue, JsValue> {
        to_js(
            &self
                .client
                .next_outbound(&group(group_id))
                .js()?
                .map(OutboundDto::from),
        )
    }

    #[wasm_bindgen(js_name = outboundSent)]
    pub fn outbound_sent(&mut self, group_id: &[u8], send_id: &[u8]) -> Result<(), JsValue> {
        let send_id = SendId::from_bytes(id16(send_id, "send id")?);
        self.client.outbound_sent(&group(group_id), &send_id).js()
    }

    #[wasm_bindgen(js_name = outboundGroups)]
    pub fn outbound_groups(&self) -> Result<JsValue, JsValue> {
        to_js(&dto::groups(&self.client.outbound_groups()))
    }

    #[wasm_bindgen(js_name = waitingMessages)]
    pub fn waiting_messages(&self, group_id: &[u8]) -> Result<JsValue, JsValue> {
        to_js(&dto::ids(&self.client.waiting_messages(&group(group_id))))
    }

    /// The partition of a group's history, for the host to read it.
    #[wasm_bindgen(js_name = historyPartition)]
    pub fn history_partition(&self, group_id: &[u8]) -> Result<Vec<u8>, JsValue> {
        Ok(self
            .client
            .history_partition(&group(group_id))
            .js()?
            .as_bytes()
            .to_vec())
    }

    /// The group's messages, from the records of its history partition.
    #[wasm_bindgen(js_name = readHistory)]
    pub fn read_history(&self, group_id: &[u8], history: JsValue) -> Result<JsValue, JsValue> {
        let entries = self
            .client
            .read_history(&group(group_id), &records(history)?)
            .js()?;
        let entries = entries
            .into_iter()
            .map(|entry| HistoryEntryDto::of(entry, &self.seal))
            .collect::<Result<Vec<_>, _>>()?;
        to_js(&entries)
    }

    /// Encrypts a file before upload; its secret comes back sealed.
    #[wasm_bindgen(js_name = encryptMedia)]
    pub fn encrypt_media(&self, file: &[u8]) -> Result<JsValue, JsValue> {
        let (encrypted, media) = encrypt_media(file).js()?;
        to_js(&EncryptedFile {
            size: media.size(),
            secret: ByteBuf::from(self.seal.seal(&media).js()?),
            encrypted: ByteBuf::from(encrypted),
        })
    }

    /// Decrypts a downloaded blob with the sealed secret of its media.
    #[wasm_bindgen(js_name = decryptMedia)]
    pub fn decrypt_media(&self, secret: &[u8], encrypted: &[u8]) -> Result<Vec<u8>, JsValue> {
        // Only the key, the digest and the size matter to decrypt.
        let media = self
            .seal
            .open(secret)
            .js()?
            .into_ref(Vec::new(), String::new(), String::new());
        decrypt_media(&media, encrypted).js()
    }

    #[wasm_bindgen(js_name = safetyNumber)]
    pub fn safety_number(&self, contact: &[u8]) -> Result<JsValue, JsValue> {
        to_js(&SafetyDto::from(
            self.client.safety_number(&certificate(contact)?),
        ))
    }

    #[wasm_bindgen(js_name = checkScanned)]
    pub fn check_scanned(&self, contact: &[u8], scanned: &[u8]) -> Result<(), JsValue> {
        self.client
            .check_scanned(&certificate(contact)?, scanned)
            .js()
    }

    #[wasm_bindgen(js_name = markVerified)]
    pub fn mark_verified(&mut self, contact: &[u8]) -> Result<(), JsValue> {
        self.client.mark_verified(&certificate(contact)?).js()
    }

    pub fn unverify(&mut self, account: &str) -> Result<(), JsValue> {
        let account = AccountId::parse(account).js()?;
        self.client.unverify(&account).js()
    }

    pub fn trust(&self, device: &[u8]) -> Result<String, JsValue> {
        Ok(dto::trust(self.client.trust(&certificate(device)?)).to_owned())
    }

    #[wasm_bindgen(js_name = acknowledgeDevice)]
    pub fn acknowledge_device(&mut self, device: &[u8]) -> Result<(), JsValue> {
        self.client.acknowledge_device(&certificate(device)?).js()
    }
}
