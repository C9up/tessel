use super::*;
use crate::groups::Incoming;
use crate::identity::AccountId;
use crate::store::SqliteStore;
use crate::test_support::{Account, Peer, text};

/// Alice's first device: it holds the account key.
fn first_device() -> Peer {
    let key = StoreKey::generate().unwrap();
    let account = AccountId::parse("alice@tessel.example").unwrap();
    let mut peer = Peer {
        client: Client::create_account(&account, &key).unwrap(),
        store: SqliteStore::in_memory().unwrap(),
        key,
    };
    peer.persist();
    peer
}

/// The new device, once it accepted the grant.
fn linked(request: LinkRequest, grant: &[u8]) -> Peer {
    let key = StoreKey::generate().unwrap();
    let mut peer = Peer {
        client: request.accept(grant, &key).unwrap(),
        store: SqliteStore::in_memory().unwrap(),
        key,
    };
    peer.persist();
    peer
}

#[test]
fn a_scanned_device_becomes_a_device_of_the_account_and_joins_its_groups() {
    let mut alice = first_device();
    let mut bob = Account::new("bob").device();
    let with_bob = crate::test_support::group(&mut alice, &mut [&mut bob]);
    let alone = alice.client.create_group().unwrap();
    alice.persist();

    // The new device shows its QR code; the first device scans it.
    let request = LinkRequest::new().unwrap();
    let qr = request.qr();
    assert_eq!(qr.len(), LINK_QR_LEN);
    let grant = alice.client.grant_link(&qr).unwrap();
    assert_eq!(grant.link, request.link());
    let mut tablet = linked(request, &grant.sealed);
    assert_eq!(tablet.client.device().certificate(), &grant.certificate);
    assert_eq!(
        tablet.client.device().certificate().account(),
        alice.client.device().certificate().account()
    );
    assert_eq!(
        tablet.client.device().certificate().account_key(),
        alice.client.device().certificate().account_key()
    );
    // The account key stays where it was.
    assert!(!tablet.client.holds_account_key());

    // It survives a restart.
    tablet.restart();

    // The first device adds it to each of its groups, with one KeyPackage per group.
    assert_eq!(alice.client.groups().len(), 2);
    for group in alice.client.groups() {
        let key_package = tablet.key_package();
        let out = alice.client.add_devices(&group, &[key_package]).unwrap();
        alice.client.commit_accepted(&group).unwrap();
        alice.persist();
        if group == with_bob {
            assert!(matches!(
                bob.client.process(&out.commit).unwrap(),
                Incoming::Commit { .. }
            ));
            bob.persist();
        }
        tablet
            .client
            .join_group(out.welcome.as_ref().unwrap())
            .unwrap();
        tablet.persist();
    }
    assert_eq!(tablet.client.groups(), alice.client.groups());
    assert!(tablet.client.groups().contains(&alone));

    // Bob writes; the new device reads it.
    let sent = bob.send(&with_bob, &text("hello, new tablet"));
    tablet.receive(&sent);
    alice.receive(&sent);
    let members = tablet.client.members(&with_bob).unwrap();
    assert_eq!(members.len(), 3);
}

#[test]
fn a_grant_opens_for_its_own_link_only_and_unaltered() {
    let alice = first_device();
    let request = LinkRequest::new().unwrap();
    let other = LinkRequest::new().unwrap();
    let grant = alice.client.grant_link(&other.qr()).unwrap();
    let key = StoreKey::generate().unwrap();
    assert_eq!(
        request.accept(&grant.sealed, &key).unwrap_err(),
        LinkError::BadGrant
    );

    let request = LinkRequest::new().unwrap();
    let mut altered = alice.client.grant_link(&request.qr()).unwrap().sealed;
    let last = altered.len() - 1;
    altered[last] ^= 1;
    assert_eq!(
        request.accept(&altered, &key).unwrap_err(),
        LinkError::BadGrant
    );
    assert_eq!(
        LinkRequest::new()
            .unwrap()
            .accept(&[0; 10], &key)
            .unwrap_err(),
        LinkError::BadGrant
    );
}

#[test]
fn a_grant_certifying_another_key_is_refused() {
    let alice = first_device();
    let request = LinkRequest::new().unwrap();
    // Someone who saw the QR code swaps in their own key.
    let mut qr = request.qr().to_vec();
    let start = LINK_QR_LEN - 32;
    qr[start..].copy_from_slice(DeviceKey::generate().unwrap().public_key());
    let grant = alice.client.grant_link(&qr).unwrap();
    assert_eq!(
        request
            .accept(&grant.sealed, &StoreKey::generate().unwrap())
            .unwrap_err(),
        LinkError::WrongCertificate
    );
}

#[test]
fn only_the_device_holding_the_account_key_grants() {
    let alice = first_device();
    let request = LinkRequest::new().unwrap();
    let grant = alice.client.grant_link(&request.qr()).unwrap();
    let tablet = linked(request, &grant.sealed);
    assert_eq!(
        tablet
            .client
            .grant_link(&LinkRequest::new().unwrap().qr())
            .unwrap_err(),
        LinkError::NoAccountKey
    );
}

#[test]
fn a_qr_code_that_is_not_a_link_is_refused() {
    let alice = first_device();
    let qr = LinkRequest::new().unwrap().qr().to_vec();
    let mut other_version = qr.clone();
    other_version[0] = 2;
    for bad in [&qr[..LINK_QR_LEN - 1], &other_version[..], &[][..]] {
        assert_eq!(
            alice.client.grant_link(bad).unwrap_err(),
            LinkError::MalformedQr
        );
    }
}

#[test]
fn the_secret_appears_in_no_debug_output() {
    let request = LinkRequest::new().unwrap();
    let qr = request.qr();
    let secret = &qr[1 + LINK_ID_LEN..1 + LINK_ID_LEN + SECRET_LEN];
    assert!(!format!("{request:?}").contains(&format!("{secret:?}")[1..20]));
}
