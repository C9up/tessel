use super::*;
use crate::groups::{GroupId, Incoming};
use crate::history::Effect;
use crate::link::LinkRequest;
use crate::store::{SqliteStore, StoreKey};
use crate::test_support::{Account, Peer, group, now};

fn alice() -> AccountId {
    AccountId::parse("alice@tessel.example").unwrap()
}

fn named(name: &str) -> Profile {
    Profile {
        name: name.into(),
        avatar: None,
    }
}

fn with_avatar() -> Profile {
    Profile {
        name: "Alice".into(),
        avatar: Some(Avatar {
            content_type: "image/webp".into(),
            image: vec![7; 2000],
        }),
    }
}

#[test]
fn a_profile_opens_with_its_key_for_its_account_only() {
    let key = ProfileKey::generate().unwrap();
    for profile in [named("Alice"), with_avatar()] {
        let blob = seal_profile(&key, &alice(), &profile).unwrap();
        assert_eq!(open_profile(&key, &alice(), &blob).unwrap(), profile);
    }
    let blob = seal_profile(&key, &alice(), &named("Alice")).unwrap();

    // Served as Bob's profile, under another key, altered, or of another version.
    let bob = AccountId::parse("bob@tessel.example").unwrap();
    assert_eq!(open_profile(&key, &bob, &blob), Err(ProfileError::Altered));
    let other = ProfileKey::generate().unwrap();
    assert_eq!(
        open_profile(&other, &alice(), &blob),
        Err(ProfileError::Altered)
    );
    let mut altered = blob.clone();
    let last = altered.len() - 1;
    altered[last] ^= 1;
    assert_eq!(
        open_profile(&key, &alice(), &altered),
        Err(ProfileError::Altered)
    );
    let mut newer = blob.clone();
    newer[0] = 2;
    assert_eq!(
        open_profile(&key, &alice(), &newer),
        Err(ProfileError::UnsupportedVersion(2))
    );
    assert_eq!(
        open_profile(&key, &alice(), &[1, 2]),
        Err(ProfileError::Altered)
    );
}

#[test]
fn the_blob_size_says_little_about_the_name() {
    let key = ProfileKey::generate().unwrap();
    let short = seal_profile(&key, &alice(), &named("Al")).unwrap();
    let longer = seal_profile(&key, &alice(), &named("Alice Liddell")).unwrap();
    assert_eq!(short.len(), longer.len());
}

#[test]
fn a_profile_breaking_a_limit_is_refused() {
    let key = ProfileKey::generate().unwrap();
    let too_big = Profile {
        name: "Alice".into(),
        avatar: Some(Avatar {
            content_type: "image/webp".into(),
            image: vec![0; MAX_AVATAR + 1],
        }),
    };
    for (profile, why) in [
        (named(""), "display name"),
        (named(&"a".repeat(MAX_DISPLAY_NAME + 1)), "display name"),
        (too_big, "avatar size"),
    ] {
        assert_eq!(
            seal_profile(&key, &alice(), &profile),
            Err(ProfileError::Invalid(why))
        );
    }
}

/// Sends the head of `from`'s queue in `group` to `to`; returns what `to` made of it.
fn relay(from: &mut Peer, to: &mut Peer, group: &GroupId) -> Effect {
    let outbound = from.client.next_outbound(group).unwrap().unwrap();
    from.client.outbound_sent(group, &outbound.send_id).unwrap();
    from.persist();
    let incoming = to.client.process(&outbound.encrypted).unwrap();
    to.persist();
    match incoming {
        Incoming::Message { effect, .. } => effect,
        other => panic!("expected a message, got {other:?}"),
    }
}

#[test]
fn contacts_open_the_profile_once_its_key_is_shared_and_hear_of_each_change() {
    let accounts = (Account::new("alice"), Account::new("bob"));
    let mut alice = accounts.0.device();
    let mut bob = accounts.1.device();
    let shared = group(&mut alice, &mut [&mut bob]);
    let account = alice.client.device().certificate().account().clone();

    let blob = alice.client.seal_profile(&with_avatar()).unwrap();
    assert_eq!(
        alice.client.open_profile(&account, &blob).unwrap(),
        with_avatar()
    );
    assert!(!bob.client.knows_profile(&account));
    assert_eq!(
        bob.client.open_profile(&account, &blob),
        Err(ProfileError::UnknownKey)
    );

    // Alice shares her key in their group: nothing in the history, a notice for Bob.
    alice.client.share_profile(&shared, now() * 1000).unwrap();
    alice.persist();
    assert_eq!(relay(&mut alice, &mut bob, &shared), Effect::ProfileUpdated);
    assert_eq!(
        bob.client.open_profile(&account, &blob).unwrap(),
        with_avatar()
    );
    assert!(bob.history(&shared).is_empty());
    assert!(alice.history(&shared).is_empty());

    // It survives a restart.
    bob.restart();
    assert_eq!(
        bob.client.open_profile(&account, &blob).unwrap(),
        with_avatar()
    );

    // Alice changes her name: Bob hears of it, and opens the new blob.
    let renamed = alice.client.seal_profile(&named("Alice L.")).unwrap();
    alice.client.share_profile(&shared, now() * 1000).unwrap();
    alice.persist();
    assert_eq!(relay(&mut alice, &mut bob, &shared), Effect::ProfileUpdated);
    assert_eq!(
        bob.client.open_profile(&account, &renamed).unwrap(),
        named("Alice L.")
    );
}

#[test]
fn a_linked_device_holds_its_account_profile_key() {
    let key = StoreKey::generate().unwrap();
    let mut phone = Peer {
        client: Client::create_account(&alice(), &key).unwrap(),
        store: SqliteStore::in_memory().unwrap(),
        key,
    };
    phone.persist();
    let blob = phone.client.seal_profile(&named("Alice")).unwrap();

    let request = LinkRequest::new().unwrap();
    let grant = phone.client.grant_link(&request.qr()).unwrap();
    let key = StoreKey::generate().unwrap();
    let mut tablet = Peer {
        client: request.accept(&grant.sealed, &key).unwrap(),
        store: SqliteStore::in_memory().unwrap(),
        key,
    };
    tablet.persist();
    assert_eq!(
        tablet.client.open_profile(&alice(), &blob).unwrap(),
        named("Alice")
    );
    tablet.restart();
    assert_eq!(
        tablet.client.open_profile(&alice(), &blob).unwrap(),
        named("Alice")
    );
}

#[test]
fn the_profile_key_appears_in_no_debug_output() {
    let key = ProfileKey::generate().unwrap();
    let bytes = format!("{:?}", key.as_bytes());
    assert!(!format!("{key:?}").contains(&bytes[1..20]));
}
