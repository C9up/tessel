//! The mobile binding of tessel-core (story 02-13), on Ream's mobile stack.
//!
//! The phone's JavaScript reaches the core through a JSI module (`@tessel/core-mobile`)
//! and the C ABI of [`ffi`]: one call per operation of `TesselCore`, its arguments and
//! its answer in MessagePack, which keeps bytes as bytes. The operations are those of
//! `tessel-api`, which the web binding wraps too: one API, two transports.
//!
//! What the web page does in TypeScript (`CoreHost`) this crate does in Rust, since the
//! database is the core's own SQLite store here: open it, read the store key sealed by
//! the platform (the Android Keystore), start the device, and persist what changed
//! after every call, even a failed one, before answering (ADR-007). The store key never
//! leaves this crate in clear: the platform only sees it to seal or unseal it.

pub mod ffi;

use std::path::PathBuf;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_bytes::ByteBuf;
use tessel_api::dto::{BodyDto, DeviceInfo, GroupChangeIn, ProfileDto, RemovalIn};
use tessel_api::error::{fail, malformed};
use tessel_api::{ApiError, Code as _, PendingLink, Session};
use tessel_core::store::SqliteStore;
use tessel_core::{PartitionId, Store as _};
use zeroize::Zeroizing;

/// The database file, in the directory the platform gives.
const DATABASE: &str = "tessel.db";

/// The host value holding the store key, sealed by the platform.
const SEALED_KEY: &str = "store-key";

/// The platform's seal (`false`) or unseal (`true`) of the store key; `None` when it
/// refuses (another phone, a wiped Keystore).
pub type Seal = Box<dyn Fn(bool, &[u8]) -> Option<Vec<u8>> + Send>;

/// What `requestLink` answers: the QR code to show, and the server's mailbox.
#[derive(Serialize)]
struct LinkRequestInfo {
    qr: ByteBuf,
    link: ByteBuf,
}

/// The core on a phone: its database, its device once there is one.
pub struct Host {
    dir: PathBuf,
    seal: Seal,
    store: Option<SqliteStore>,
    session: Option<Session>,
    /// The store key of a profile without a device yet, until `createAccount` or
    /// `completeLink`.
    pending_key: Option<Zeroizing<Vec<u8>>>,
    /// A new device's key waiting for its grant (story 03-04).
    pending_link: Option<PendingLink>,
}

fn store_error(error: tessel_core::StoreError) -> ApiError {
    error.into_api()
}

/// The arguments of a call: a MessagePack array, read as the method's tuple.
fn args<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, ApiError> {
    rmp_serde::from_slice(bytes).map_err(|_| malformed("arguments"))
}

/// An answer, in MessagePack; structs as maps, with their field names.
fn answer<T: Serialize>(value: T) -> Result<Vec<u8>, ApiError> {
    rmp_serde::to_vec_named(&value).map_err(|error| fail("host:encode", error))
}

fn not_started() -> ApiError {
    fail(
        "host:not-started",
        "no device: call start, then createAccount if needed",
    )
}

impl Host {
    /// The core whose database lives in `dir`, its key sealed by `seal`.
    pub fn new(dir: impl Into<PathBuf>, seal: Seal) -> Self {
        Self {
            dir: dir.into(),
            seal,
            store: None,
            session: None,
            pending_key: None,
            pending_link: None,
        }
    }

    /// Runs `method` with its MessagePack arguments; its MessagePack answer. What the
    /// call changed is persisted before it answers, even when it failed.
    pub fn call(&mut self, method: &str, input: &[u8]) -> Result<Vec<u8>, ApiError> {
        let result = self.dispatch(method, input);
        let persisted = self.persist();
        let value = result?;
        persisted?;
        Ok(value)
    }

    fn persist(&mut self) -> Result<(), ApiError> {
        if let (Some(session), Some(store)) = (self.session.as_mut(), self.store.as_mut()) {
            let changes = session.take_store_changes()?;
            if !changes.put.is_empty() || !changes.delete.is_empty() {
                store.apply(&changes).map_err(store_error)?;
            }
        }
        Ok(())
    }

    fn session(&self) -> Result<&Session, ApiError> {
        self.session.as_ref().ok_or_else(not_started)
    }

    fn session_mut(&mut self) -> Result<&mut Session, ApiError> {
        self.session.as_mut().ok_or_else(not_started)
    }

    /// The store key of a started profile without a device; refuses otherwise.
    fn awaiting_device(&self, method: &str) -> Result<Zeroizing<Vec<u8>>, ApiError> {
        match (&self.pending_key, &self.session) {
            (Some(key), _) => Ok(key.clone()),
            (None, Some(_)) => Err(fail(
                "host:account-exists",
                format_args!("{method} needs a started profile without a device"),
            )),
            (None, None) => Err(fail(
                "host:not-started",
                format_args!("{method} needs a started profile without a device"),
            )),
        }
    }

    /// Opens the database and the device in it; `None` for a profile without one yet.
    fn start(&mut self) -> Result<Option<DeviceInfo>, ApiError> {
        if let Some(session) = &self.session {
            return session.device().map(Some);
        }
        if self.pending_key.is_some() {
            return Ok(None);
        }
        let mut store = SqliteStore::open(&self.dir.join(DATABASE)).map_err(store_error)?;
        let key = match store.host_value(SEALED_KEY).map_err(store_error)? {
            Some(sealed) => Zeroizing::new((self.seal)(true, &sealed).ok_or_else(|| {
                fail(
                    "host:keystore",
                    "the platform could not unseal the store key",
                )
            })?),
            None => {
                let key = Zeroizing::new(tessel_api::generate_store_key()?);
                let sealed = (self.seal)(false, &key).ok_or_else(|| {
                    fail("host:keystore", "the platform could not seal the store key")
                })?;
                store
                    .set_host_value(SEALED_KEY, &sealed)
                    .map_err(store_error)?;
                key
            }
        };
        let partition =
            PartitionId::from_bytes(&tessel_api::state_partition(&key)?).map_err(store_error)?;
        let state = store.list(&partition).map_err(store_error)?;
        self.store = Some(store);
        if state.is_empty() {
            self.pending_key = Some(key);
            return Ok(None);
        }
        let session = Session::open_records(&key, &state)?;
        let device = session.device()?;
        self.session = Some(session);
        Ok(Some(device))
    }

    fn create_account(&mut self, account: &str) -> Result<DeviceInfo, ApiError> {
        let key = self.awaiting_device("createAccount")?;
        let session = Session::create_account(account, &key)?;
        let device = session.device()?;
        self.session = Some(session);
        self.pending_key = None;
        Ok(device)
    }

    fn request_link(&mut self) -> Result<LinkRequestInfo, ApiError> {
        self.awaiting_device("requestLink")?;
        let link = PendingLink::new()?;
        let info = LinkRequestInfo {
            qr: ByteBuf::from(link.qr()?),
            link: ByteBuf::from(link.link()?),
        };
        self.pending_link = Some(link);
        Ok(info)
    }

    fn complete_link(&mut self, grant: &[u8]) -> Result<DeviceInfo, ApiError> {
        let key = self.awaiting_device("completeLink")?;
        // The link is spent whatever happens: a refused grant means a new QR code.
        let mut link = self
            .pending_link
            .take()
            .ok_or_else(|| fail("host:no-link", "completeLink needs requestLink first"))?;
        let session = link.accept(grant, &key)?;
        let device = session.device()?;
        self.session = Some(session);
        self.pending_key = None;
        Ok(device)
    }

    fn history(&self, group: &[u8]) -> Result<Vec<u8>, ApiError> {
        let session = self.session()?;
        let store = self.store.as_ref().ok_or_else(not_started)?;
        let partition =
            PartitionId::from_bytes(&session.history_partition(group)?).map_err(store_error)?;
        let records = store.list(&partition).map_err(store_error)?;
        answer(session.read_history_records(group, &records)?)
    }

    fn close(&mut self) -> Result<(), ApiError> {
        self.persist()?;
        self.session = None;
        self.store = None;
        self.pending_key = None;
        self.pending_link = None;
        Ok(())
    }

    fn dispatch(&mut self, method: &str, input: &[u8]) -> Result<Vec<u8>, ApiError> {
        match method {
            "version" => answer(tessel_api::version()),
            "ciphersuite" => answer(tessel_api::ciphersuite()),
            "start" => answer(self.start()?),
            "createAccount" => {
                let (account,): (String,) = args(input)?;
                answer(self.create_account(&account)?)
            }
            "requestLink" => answer(self.request_link()?),
            "completeLink" => {
                let (grant,): (ByteBuf,) = args(input)?;
                answer(self.complete_link(&grant)?)
            }
            "close" => answer(self.close()?),
            "history" => {
                let (group,): (ByteBuf,) = args(input)?;
                self.history(&group)
            }
            "verifyKeyPackage" => {
                let (key_package,): (ByteBuf,) = args(input)?;
                answer(tessel_api::verify_key_package(&key_package)?)
            }
            _ => self.session_call(method, input),
        }
    }

    /// The operations of a started device: each one of [`Session`]'s.
    fn session_call(&mut self, method: &str, input: &[u8]) -> Result<Vec<u8>, ApiError> {
        let session = self.session_mut()?;
        match method {
            "device" => answer(session.device()?),
            "holdsAccountKey" => answer(session.holds_account_key()),
            "signIn" => {
                let (challenge,): (ByteBuf,) = args(input)?;
                answer(ByteBuf::from(session.sign_in(&challenge)?))
            }
            "grantLink" => {
                let (qr,): (ByteBuf,) = args(input)?;
                answer(session.grant_link(&qr)?)
            }
            "groups" => answer(session.groups()),
            "revokeDevice" => {
                let (device,): (ByteBuf,) = args(input)?;
                answer(ByteBuf::from(session.revoke_device(&device)?))
            }
            "groupsWith" => {
                let (device,): (ByteBuf,) = args(input)?;
                answer(session.groups_with(&device)?)
            }
            "recipients" => {
                let (group,): (ByteBuf,) = args(input)?;
                answer(session.recipients(&group)?)
            }
            "conversations" => answer(session.conversations()),
            "markRead" => {
                let (group,): (ByteBuf,) = args(input)?;
                answer(session.mark_read(&group)?)
            }
            "queueReceipts" => {
                let (sent_at,): (u64,) = args(input)?;
                answer(session.queue_receipts(sent_at)?)
            }
            "readReceipts" => answer(session.read_receipts()),
            "setReadReceipts" => {
                let (on,): (bool,) = args(input)?;
                answer(session.set_read_receipts(on)?)
            }
            "signAccountDeletion" => answer(ByteBuf::from(session.sign_account_deletion()?)),
            "sealProfile" => {
                let (profile,): (ProfileDto,) = args(input)?;
                answer(ByteBuf::from(session.seal_profile(profile)?))
            }
            "openProfile" => {
                let (account, blob): (String, ByteBuf) = args(input)?;
                answer(session.open_profile(&account, &blob)?)
            }
            "knowsProfile" => {
                let (account,): (String,) = args(input)?;
                answer(session.knows_profile(&account)?)
            }
            "shareProfile" => {
                let (group, sent_at): (ByteBuf, u64) = args(input)?;
                answer(ByteBuf::from(session.share_profile(&group, sent_at)?))
            }
            "refreshKeyPackages" => {
                let (now,): (u64,) = args(input)?;
                answer(session.refresh_key_packages(now)?)
            }
            "createGroup" => answer(ByteBuf::from(session.create_group()?)),
            "addDevices" => {
                let (group, key_packages): (ByteBuf, Vec<ByteBuf>) = args(input)?;
                answer(session.add_devices(&group, &key_packages)?)
            }
            "removeDevices" => {
                let (group, removals): (ByteBuf, Vec<RemovalIn>) = args(input)?;
                answer(session.remove_devices(&group, removals)?)
            }
            "configureGroup" => {
                let (group, change): (ByteBuf, GroupChangeIn) = args(input)?;
                answer(session.configure_group(&group, change)?)
            }
            "groupSettings" => {
                let (group,): (ByteBuf,) = args(input)?;
                answer(session.group_settings(&group)?)
            }
            "commitPendingProposals" => {
                let (group,): (ByteBuf,) = args(input)?;
                answer(session.commit_pending_proposals(&group)?)
            }
            "commitAccepted" => {
                let (group,): (ByteBuf,) = args(input)?;
                answer(session.commit_accepted(&group)?)
            }
            "commitRejected" => {
                let (group,): (ByteBuf,) = args(input)?;
                answer(session.commit_rejected(&group)?)
            }
            "retryCommit" => {
                let (group,): (ByteBuf,) = args(input)?;
                answer(session.retry_commit(&group)?)
            }
            "leaveGroup" => {
                let (group,): (ByteBuf,) = args(input)?;
                answer(ByteBuf::from(session.leave_group(&group)?))
            }
            "joinGroup" => {
                let (welcome,): (ByteBuf,) = args(input)?;
                answer(ByteBuf::from(session.join_group(&welcome)?))
            }
            "members" => {
                let (group,): (ByteBuf,) = args(input)?;
                answer(session.members(&group)?)
            }
            "epoch" => {
                let (group,): (ByteBuf,) = args(input)?;
                answer(session.epoch(&group)?)
            }
            "process" => {
                let (message,): (ByteBuf,) = args(input)?;
                answer(session.process(&message)?)
            }
            "queueMessage" => {
                let (group, body, sent_at): (ByteBuf, BodyDto, u64) = args(input)?;
                answer(session.queue_message(&group, body, sent_at)?)
            }
            "nextOutbound" => {
                let (group,): (ByteBuf,) = args(input)?;
                answer(session.next_outbound(&group)?)
            }
            "outboundSent" => {
                let (group, send_id): (ByteBuf, ByteBuf) = args(input)?;
                answer(session.outbound_sent(&group, &send_id)?)
            }
            "outboundGroups" => answer(session.outbound_groups()),
            "waitingMessages" => {
                let (group,): (ByteBuf,) = args(input)?;
                answer(session.waiting_messages(&group))
            }
            "encryptMedia" => {
                let (file,): (ByteBuf,) = args(input)?;
                answer(session.encrypt_media(&file)?)
            }
            "decryptMedia" => {
                let (secret, encrypted): (ByteBuf, ByteBuf) = args(input)?;
                answer(ByteBuf::from(session.decrypt_media(&secret, &encrypted)?))
            }
            "safetyNumber" => {
                let (contact,): (ByteBuf,) = args(input)?;
                answer(session.safety_number(&contact)?)
            }
            "checkScanned" => {
                let (contact, scanned): (ByteBuf, ByteBuf) = args(input)?;
                answer(session.check_scanned(&contact, &scanned)?)
            }
            "markVerified" => {
                let (contact,): (ByteBuf,) = args(input)?;
                answer(session.mark_verified(&contact)?)
            }
            "unverify" => {
                let (account,): (String,) = args(input)?;
                answer(session.unverify(&account)?)
            }
            "trust" => {
                let (device,): (ByteBuf,) = args(input)?;
                answer(session.trust(&device)?)
            }
            "acknowledgeDevice" => {
                let (device,): (ByteBuf,) = args(input)?;
                answer(session.acknowledge_device(&device)?)
            }
            other => Err(fail(
                "host:unknown-method",
                format_args!("no method {other}"),
            )),
        }
    }
}

#[cfg(test)]
mod tests;
