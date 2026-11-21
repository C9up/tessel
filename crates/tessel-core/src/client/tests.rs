use super::*;
use crate::groups::Incoming;
use crate::store::{Store as _, StoreError};
use crate::test_support::{Account, Peer, group, now, text};

#[test]
fn a_restarted_device_carries_on_in_its_groups() {
    let (mut a, mut b) = (Account::new("alice").device(), Account::new("bob").device());
    let group = group(&mut a, &mut [&mut b]);
    let epoch = a.client.epoch(&group).unwrap();

    a.restart();
    assert_eq!(a.client.epoch(&group), Ok(epoch));
    assert_eq!(a.client.members(&group).unwrap().len(), 2);
    let question = text("still there?");
    let sent = b.send(&group, &question);
    assert!(matches!(
        a.client.process(&sent).unwrap(),
        Incoming::Message { message: Ok(m), .. } if m == question
    ));
    a.persist();
    let sent = a.send(&group, &text("yes"));
    assert!(matches!(
        b.client.process(&sent).unwrap(),
        Incoming::Message { message: Ok(_), .. }
    ));

    // The secrets used before the restart are gone for good: an old message does not
    // decrypt twice.
    a.restart();
    let again = b.send(&group, &text("one more"));
    assert!(matches!(
        a.client.process(&again).unwrap(),
        Incoming::Message { message: Ok(_), .. }
    ));
    a.persist();
    a.restart();
    assert!(a.client.process(&again).is_err());
}

#[test]
fn a_restarted_device_keeps_its_key_package_pool() {
    let mut a = Account::new("alice").device();
    let first = a.client.refresh_key_packages(now()).unwrap();
    assert!(!first.is_empty());
    a.persist();
    a.restart();
    assert!(a.client.refresh_key_packages(now()).unwrap().is_empty());
}

#[test]
fn a_pending_commit_survives_a_restart() {
    let (mut a, mut b) = (Account::new("alice").device(), Account::new("bob").device());
    let group = a.client.create_group().unwrap();
    let key_package = b.key_package();
    a.client.add_devices(&group, &[key_package]).unwrap();
    a.persist();

    // The server rejected it while the app was closed: the intent is replayed.
    a.restart();
    a.client.commit_rejected(&group).unwrap();
    let out = a
        .client
        .retry_commit(&group)
        .unwrap()
        .expect("the intent is kept");
    a.client.commit_accepted(&group).unwrap();
    a.persist();
    b.client.join_group(out.welcome.as_ref().unwrap()).unwrap();
    assert_eq!(a.client.members(&group).unwrap().len(), 2);
}

#[test]
fn nothing_changed_means_nothing_to_persist() {
    let mut a = Account::new("alice").device();
    a.client.create_group().unwrap();
    assert!(!a.client.take_changes().unwrap().is_empty());
    assert!(a.client.take_changes().unwrap().is_empty());
}

#[test]
fn the_state_does_not_open_with_another_key() {
    let a = Account::new("alice").device();
    let other = StoreKey::generate().unwrap();
    assert_eq!(
        Client::open(&other, &a.state()).err(),
        Some(StoreError::WrongPartition)
    );
    // The same records presented under the other key's partition.
    let moved: Vec<Record> = a
        .state()
        .into_iter()
        .map(|r| Record {
            partition: other.state_partition().unwrap(),
            ..r
        })
        .collect();
    assert_eq!(
        Client::open(&other, &moved).err(),
        Some(StoreError::Unreadable)
    );
    assert_eq!(
        Client::open(&a.key, &[]).err(),
        Some(StoreError::Missing("client"))
    );
}

#[test]
fn the_database_holds_nothing_in_clear() {
    let (mut a, mut b) = (Account::new("alice").device(), Account::new("bob").device());
    let group = group(&mut a, &mut [&mut b]);
    let sent = a.send(&group, &text("meet me at the old station"));
    b.receive(&sent);

    let peers: [&Peer; 2] = [&a, &b];
    for peer in peers {
        let history = peer.client.history_partition(&group).unwrap();
        let mut bytes = Vec::new();
        for partition in [peer.key.state_partition().unwrap(), history] {
            for record in peer.store.list(&partition).unwrap() {
                bytes.extend_from_slice(record.id.as_bytes());
                bytes.extend_from_slice(&record.value);
            }
        }
        assert!(!bytes.is_empty());
        // Needles long enough not to turn up by chance: some 300 KB of ciphertext hold
        // any given 3 bytes (`bob`) about once in sixty runs.
        let needles = [
            &b"old station"[..],
            b"alice@tessel.example",
            b"bob@tessel.example",
            b"tessel.example",
        ];
        for clear in needles {
            assert!(
                !bytes.windows(clear.len()).any(|w| w == clear),
                "{:?} in clear",
                String::from_utf8_lossy(clear)
            );
        }
    }
}

#[test]
fn the_first_device_keeps_its_account_key_sealed_across_restarts() {
    let account = AccountId::parse("alice@tessel.example").unwrap();
    let key = StoreKey::generate().unwrap();
    let client = Client::create_account(&account, &key).unwrap();
    assert!(client.holds_account_key());
    assert_eq!(client.device().certificate().account(), &account);
    let mut a = Peer {
        client,
        store: crate::store::SqliteStore::in_memory().unwrap(),
        key,
    };
    a.persist();
    a.restart();
    assert!(a.client.holds_account_key());

    // A device certified by an account key held elsewhere does not hold it.
    let b = Account::new("bob").device();
    assert!(!b.client.holds_account_key());
}
