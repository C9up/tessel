//! Test helpers: accounts, devices with their own database, and messages.

use crate::client::Client;
use crate::groups::{GroupId, Incoming};
use crate::history::{Effect, HistoryEntry};
use crate::identity::{AccountId, AccountKey, Device, DeviceId, DeviceKey};
use crate::key_packages::{VerifiedKeyPackage, verify_key_package};
use crate::messages::{Body, Message, MessageId};
use crate::store::{Record, SqliteStore, Store as _, StoreKey};

/// Verifying a *KeyPackage* reads the real clock: the tests start from now.
pub(crate) fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// An account, able to certify devices.
pub(crate) struct Account {
    pub(crate) id: AccountId,
    key: AccountKey,
}

impl Account {
    pub(crate) fn new(name: &str) -> Self {
        Self {
            id: AccountId::parse(&format!("{name}@tessel.example")).unwrap(),
            key: AccountKey::generate().unwrap(),
        }
    }

    /// A new device of the account, with its own database.
    pub(crate) fn device(&self) -> Peer {
        let key = DeviceKey::generate().unwrap();
        let certificate = self
            .key
            .certify(&self.id, DeviceId::random().unwrap(), key.public_key())
            .unwrap();
        let store_key = StoreKey::generate().unwrap();
        let client = Client::new(Device::new(key, certificate).unwrap(), &store_key).unwrap();
        let mut peer = Peer {
            client,
            store: SqliteStore::in_memory().unwrap(),
            key: store_key,
        };
        peer.persist();
        peer
    }
}

/// A device and its database, persisted after every call as a host would.
pub(crate) struct Peer {
    pub(crate) client: Client,
    pub(crate) store: SqliteStore,
    pub(crate) key: StoreKey,
}

impl Peer {
    pub(crate) fn persist(&mut self) {
        let changes = self.client.take_changes().unwrap();
        self.store.apply(&changes).unwrap();
    }

    /// The records of the state partition.
    pub(crate) fn state(&self) -> Vec<Record> {
        self.store
            .list(&self.key.state_partition().unwrap())
            .unwrap()
    }

    /// Restarts the device: a new client, from the database alone.
    pub(crate) fn restart(&mut self) {
        self.client = Client::open(&self.key, &self.state()).unwrap();
    }

    /// Its first single-use *KeyPackage*.
    pub(crate) fn key_package(&mut self) -> VerifiedKeyPackage {
        let update = self.client.refresh_key_packages(now()).unwrap();
        self.persist();
        verify_key_package(&update.publish[0]).unwrap()
    }

    /// Sends a message; returns what the server relays.
    pub(crate) fn send(&mut self, group: &GroupId, message: &Message) -> Vec<u8> {
        let encrypted = self.client.send_message(group, message).unwrap();
        self.persist();
        encrypted
    }

    /// Processes a relayed message; returns its effect on the history.
    pub(crate) fn receive(&mut self, encrypted: &[u8]) -> Effect {
        let incoming = self.client.process(encrypted).unwrap();
        self.persist();
        match incoming {
            Incoming::Message { effect, .. } => effect,
            other => panic!("expected a message, got {other:?}"),
        }
    }

    /// The group's history, read back from the database.
    pub(crate) fn history(&self, group: &GroupId) -> Vec<HistoryEntry> {
        let partition = self.client.history_partition(group).unwrap();
        let records = self.store.list(&partition).unwrap();
        self.client.read_history(group, &records).unwrap()
    }
}

pub(crate) fn text(text: &str) -> Message {
    Message::new(
        now() * 1000,
        Body::Text {
            text: text.into(),
            reply_to: None,
        },
    )
    .unwrap()
}

pub(crate) fn edit(target: MessageId, text: &str) -> Message {
    Message::new(
        now() * 1000,
        Body::Edit {
            target,
            text: text.into(),
        },
    )
    .unwrap()
}

pub(crate) fn delete(target: MessageId) -> Message {
    Message::new(now() * 1000, Body::Delete { target }).unwrap()
}

/// A group created by `creator`, joined by every other device.
pub(crate) fn group(creator: &mut Peer, others: &mut [&mut Peer]) -> GroupId {
    let group = creator.client.create_group().unwrap();
    let key_packages: Vec<_> = others.iter_mut().map(|p| p.key_package()).collect();
    let out = creator.client.add_devices(&group, &key_packages).unwrap();
    creator.client.commit_accepted(&group).unwrap();
    creator.persist();
    for peer in others.iter_mut() {
        peer.client
            .join_group(out.welcome.as_ref().unwrap())
            .unwrap();
        peer.persist();
    }
    group
}
