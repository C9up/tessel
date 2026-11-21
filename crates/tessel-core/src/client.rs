//! The client: a certified device, its MLS state, its groups, its pool of *KeyPackages*
//! and its history, kept in the encrypted local database (story 02-06).

use std::collections::HashMap;

use openmls::prelude::MlsGroup;
use openmls_rust_crypto::OpenMlsRustCrypto;
use openmls_traits::OpenMlsProvider;
use zeroize::Zeroizing;

use crate::codec::{Reader, Writer};
use crate::groups::{Group, GroupId, read_operation, write_operation};
use crate::history::{self, Effect, History, HistoryEntry, RECENT};
use crate::identity::{AccountId, AccountKey, Device, DeviceCertificate, IdentityError};
use crate::key_packages::{KeyPackageError, KeyPackagePool, KeyPackageUpdate};
use crate::messages::{Message, MessageId};
use crate::outbox::{OUTBOX, Outbox, SendId};
use crate::safety::{Contacts, VERIFIED};
use crate::store::{Changes, PartitionId, Record, Sealer, StoreError, StoreKey};

/// Version of the client record. (Version 1, without the account key, was never
/// released.)
const CLIENT_VERSION: u8 = 2;

/// Label of the client record: the device, the pool and the list of groups.
const CLIENT: &[u8] = b"client";

/// Label of an OpenMLS storage entry.
const MLS: &[u8] = b"mls";

/// What the database holds, as last handed to the host.
struct Persisted {
    sealer: Sealer,
    state: PartitionId,
    /// The client record.
    client: Zeroizing<Vec<u8>>,
    /// The OpenMLS storage entries.
    mls: HashMap<Vec<u8>, Vec<u8>>,
}

impl std::fmt::Debug for Persisted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Persisted")
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

/// What creating an account's first device can fail on.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ClientError {
    /// The keys or the certificate could not be made.
    #[error(transparent)]
    Identity(#[from] IdentityError),
    /// The local database failed.
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// A running Tessel device.
#[derive(Debug)]
pub struct Client {
    pub(crate) provider: OpenMlsRustCrypto,
    pub(crate) device: Device,
    /// The account key, on the device that holds it: it certifies the account's other
    /// devices (story 03-04).
    account_key: Option<AccountKey>,
    key_packages: KeyPackagePool,
    pub(crate) groups: HashMap<GroupId, Group>,
    pub(crate) history: History,
    pub(crate) outbox: Outbox,
    pub(crate) contacts: Contacts,
    persisted: Persisted,
}

impl Client {
    /// Starts the client of a newly certified device, with a new store key
    /// ([`StoreKey::generate`]). Its first [`Client::take_changes`] holds the whole
    /// initial state.
    pub fn new(device: Device, key: &StoreKey) -> Result<Self, StoreError> {
        let sealer = Sealer::new(key)?;
        Ok(Self {
            provider: OpenMlsRustCrypto::default(),
            device,
            account_key: None,
            key_packages: KeyPackagePool::default(),
            groups: HashMap::new(),
            history: History::default(),
            outbox: Outbox::default(),
            contacts: Contacts::default(),
            persisted: Persisted {
                state: key.state_partition()?,
                sealer,
                client: Zeroizing::new(Vec::new()),
                mls: HashMap::new(),
            },
        })
    }

    /// Reopens the client from the records of its state partition
    /// ([`StoreKey::state_partition`]). Fails on a record sealed with another key or
    /// altered on disk.
    pub fn open(key: &StoreKey, state: &[Record]) -> Result<Self, StoreError> {
        let sealer = Sealer::new(key)?;
        let partition = key.state_partition()?;
        let provider = OpenMlsRustCrypto::default();
        let mut client_record = None;
        let mut mls = HashMap::new();
        let mut history = History::default();
        let mut outbox = Outbox::default();
        let mut contacts = Contacts::default();

        for record in state {
            if record.partition != partition {
                return Err(StoreError::WrongPartition);
            }
            let opened = sealer.open(record)?;
            match opened.label.as_slice() {
                CLIENT => client_record = Some(opened.body),
                MLS => {
                    let mut input = Reader(&opened.body);
                    let key = input.vec32()?.to_vec();
                    let value = input.vec32()?.to_vec();
                    input.finish()?;
                    mls.insert(key, value);
                }
                RECENT => history.load(&opened.body)?,
                OUTBOX => outbox.load(&opened.body)?,
                VERIFIED => contacts.load(&opened.body)?,
                _ => return Err(StoreError::Malformed),
            }
        }
        history.sort();
        outbox.sort();
        provider
            .storage()
            .values
            .write()
            .map_err(|_| StoreError::Crypto)?
            .clone_from(&mls);

        let client_record = client_record.ok_or(StoreError::Missing("client"))?;
        let mut input = Reader(&client_record);
        match input.u8()? {
            CLIENT_VERSION => {}
            newer if newer > CLIENT_VERSION => return Err(StoreError::UnsupportedFormat(newer)),
            _ => return Err(StoreError::Malformed),
        }
        let device = Device::read_from(&mut input)?;
        let account_key = match input.u8()? {
            0 => None,
            1 => Some(AccountKey::read_from(&mut input)?),
            _ => return Err(StoreError::Malformed),
        };
        let key_packages = KeyPackagePool::read_from(&mut input)?;
        let mut groups = HashMap::new();
        for _ in 0..input.u32()? {
            let id = GroupId::from_bytes(input.vec8()?);
            let pending = read_operation(&mut input)?;
            let mls = MlsGroup::load(provider.storage(), &id.to_openmls())
                .map_err(|_| StoreError::Malformed)?
                .ok_or(StoreError::Missing("group"))?;
            groups.insert(id, Group { mls, pending });
        }
        input.finish()?;

        Ok(Self {
            provider,
            device,
            account_key,
            key_packages,
            groups,
            history,
            outbox,
            contacts,
            persisted: Persisted {
                sealer,
                state: partition,
                client: client_record,
                mls,
            },
        })
    }

    /// Starts the client of an account's first device: a new account key, a device it
    /// certifies, and a new store key's database. The account key stays in the
    /// encrypted database (story 02-02); no function hands it out.
    pub fn create_account(account: &AccountId, key: &StoreKey) -> Result<Self, ClientError> {
        let account_key = AccountKey::generate()?;
        let device = Device::create(account, &account_key)?;
        let mut client = Self::new(device, key)?;
        client.account_key = Some(account_key);
        Ok(client)
    }

    /// The device.
    pub fn device(&self) -> &Device {
        &self.device
    }

    /// Whether this device holds its account's key.
    pub fn holds_account_key(&self) -> bool {
        self.account_key.is_some()
    }

    /// The account key, on the device that holds it: only to certify a new device.
    pub(crate) fn account_key(&self) -> Option<&AccountKey> {
        self.account_key.as_ref()
    }

    /// The groups this device is a member of: those a newly linked device of the
    /// account is added to (story 03-04).
    pub fn groups(&self) -> Vec<GroupId> {
        let mut groups: Vec<GroupId> = self.groups.keys().cloned().collect();
        groups.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
        groups
    }

    /// Renews the *KeyPackage* pool at time `now` (Unix seconds) and returns what must
    /// be published and withdrawn on the server. To call at startup, then
    /// periodically.
    pub fn refresh_key_packages(&mut self, now: u64) -> Result<KeyPackageUpdate, KeyPackageError> {
        self.key_packages.refresh(&self.provider, &self.device, now)
    }

    /// What changed since the last call, sealed, for the host to persist in one
    /// transaction before acting on the output of the calls that changed it (see
    /// [`crate::store`]).
    pub fn take_changes(&mut self) -> Result<Changes, StoreError> {
        let record = self.encode()?;
        let Persisted {
            sealer,
            state,
            client,
            mls,
        } = &mut self.persisted;
        let mut changes = Changes::default();

        let client_changed = *record != **client;
        if client_changed {
            changes.put.push(sealer.seal(*state, CLIENT, &[], &record)?);
        }

        // OpenMLS writes its state into its in-memory storage; what differs from the
        // last persisted copy is sealed. Nothing is updated before everything is sealed.
        let values = self
            .provider
            .storage()
            .values
            .read()
            .map_err(|_| StoreError::Crypto)?;
        let mut written = Vec::new();
        for (key, value) in values.iter() {
            if mls.get(key) != Some(value) {
                let mut body = Writer::default();
                body.vec32(key)?;
                body.vec32(value)?;
                changes.put.push(sealer.seal(*state, MLS, key, &body.0)?);
                written.push((key.clone(), value.clone()));
            }
        }
        let removed: Vec<Vec<u8>> = mls
            .keys()
            .filter(|key| !values.contains_key(*key))
            .cloned()
            .collect();
        for key in &removed {
            changes
                .delete
                .push((*state, sealer.record_id(state, MLS, key)?));
        }
        drop(values);

        changes.append(self.history.take());
        changes.append(self.outbox.take());
        changes.append(self.contacts.take());
        if client_changed {
            *client = record;
        }
        for key in removed {
            mls.remove(&key);
        }
        mls.extend(written);
        Ok(changes)
    }

    /// The partition of a group's history, for the host to read it.
    pub fn history_partition(&self, group: &GroupId) -> Result<PartitionId, StoreError> {
        history::partition(&self.persisted.sealer, group)
    }

    /// Opens the records of a group's history partition and returns its messages, in
    /// the order the server delivered them, their last edit applied.
    pub fn read_history(
        &self,
        group: &GroupId,
        records: &[Record],
    ) -> Result<Vec<HistoryEntry>, StoreError> {
        let partition = self.history_partition(group)?;
        history::read(&self.persisted.sealer, partition, records)
    }

    /// The client record: device, pool, groups and their pending intents.
    fn encode(&self) -> Result<Zeroizing<Vec<u8>>, StoreError> {
        let mut out = Writer(Vec::with_capacity(16 * 1024));
        out.u8(CLIENT_VERSION);
        self.device.write_to(&mut out)?;
        match &self.account_key {
            None => out.u8(0),
            Some(key) => {
                out.u8(1);
                key.write_to(&mut out)?;
            }
        }
        self.key_packages.write_to(&mut out)?;
        let mut groups: Vec<_> = self.groups.iter().collect();
        groups.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
        out.u32(u32::try_from(groups.len()).map_err(|_| StoreError::Malformed)?);
        for (id, group) in groups {
            out.vec8(id.as_bytes())?;
            write_operation(&mut out, group.pending.as_ref())?;
        }
        Ok(Zeroizing::new(out.0))
    }

    /// Records a received or sent message in the history.
    pub(crate) fn record(
        &mut self,
        group: &GroupId,
        sender: &DeviceCertificate,
        message: &Message,
    ) -> Result<Effect, StoreError> {
        let Persisted { sealer, state, .. } = &self.persisted;
        self.history.apply(sealer, *state, group, sender, message)
    }

    /// Drops the history index and the send queue of a group the device left; returns
    /// the messages that will never leave.
    pub(crate) fn forget_local(&mut self, group: &GroupId) -> Result<Vec<MessageId>, StoreError> {
        let Persisted { sealer, state, .. } = &self.persisted;
        self.history.forget(sealer, *state, group)?;
        self.outbox.forget(sealer, *state, group)
    }

    /// Puts a message at the end of the group's send queue.
    pub(crate) fn enqueue(
        &mut self,
        group: &GroupId,
        message: &Message,
    ) -> Result<SendId, StoreError> {
        let Persisted { sealer, state, .. } = &self.persisted;
        self.outbox.push(sealer, *state, group, message)
    }

    /// The verified contacts, with what they need to seal their records.
    pub(crate) fn contacts_mut(&mut self) -> (&mut Contacts, &Sealer, PartitionId) {
        let Persisted { sealer, state, .. } = &self.persisted;
        (&mut self.contacts, sealer, *state)
    }

    /// Removes an acknowledged message from the group's send queue.
    pub(crate) fn dequeue(&mut self, group: &GroupId, send_id: &SendId) -> Result<(), StoreError> {
        let Persisted { sealer, state, .. } = &self.persisted;
        self.outbox.remove(sealer, *state, group, send_id)
    }

    #[cfg(test)]
    pub(crate) fn provider(&self) -> &OpenMlsRustCrypto {
        &self.provider
    }
}

#[cfg(test)]
mod tests;
