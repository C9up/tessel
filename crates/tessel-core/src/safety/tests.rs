use super::*;
use crate::test_support::{Account, Peer, group};

fn certificate(peer: &Peer) -> DeviceCertificate {
    peer.client.device().certificate().clone()
}

#[test]
fn both_sides_see_the_same_code_and_each_accepts_the_others_qr() {
    let (alice, bob) = (Account::new("alice"), Account::new("bob"));
    let (a, b) = (alice.device(), bob.device());
    let seen_by_a = a.client.safety_number(&certificate(&b));
    let seen_by_b = b.client.safety_number(&certificate(&a));

    assert_eq!(seen_by_a.digits, seen_by_b.digits);
    assert_eq!(seen_by_a.digits.len(), SAFETY_NUMBER_DIGITS);
    assert!(seen_by_a.digits.bytes().all(|c| c.is_ascii_digit()));
    assert_ne!(seen_by_a.qr, seen_by_b.qr);

    a.client
        .check_scanned(&certificate(&b), &seen_by_b.qr)
        .unwrap();
    b.client
        .check_scanned(&certificate(&a), &seen_by_a.qr)
        .unwrap();
    // One's own QR, scanned back, is not the other's.
    assert_eq!(
        a.client.check_scanned(&certificate(&b), &seen_by_a.qr),
        Err(SafetyError::Mismatch)
    );
}

#[test]
fn the_code_follows_the_account_key_not_the_devices() {
    let (alice, bob) = (Account::new("alice"), Account::new("bob"));
    let a = alice.device();
    let (b1, b2) = (bob.device(), bob.device());
    let code = a.client.safety_number(&certificate(&b1));
    assert_eq!(a.client.safety_number(&certificate(&b2)), code);
    assert_eq!(alice.device().client.safety_number(&certificate(&b1)), code);

    // Same account name, another account key: what a server putting itself in between
    // would hand out.
    let impostor = Account::new("bob").device();
    let other = a.client.safety_number(&certificate(&impostor));
    assert_ne!(other.digits, code.digits);
    assert_eq!(
        a.client.check_scanned(
            &certificate(&impostor),
            &b1.client.safety_number(&certificate(&a)).qr
        ),
        Err(SafetyError::Mismatch)
    );
}

#[test]
fn a_code_from_another_conversation_or_garbage_is_refused() {
    let (alice, bob, carol) = (
        Account::new("alice"),
        Account::new("bob"),
        Account::new("carol"),
    );
    let (a, b, c) = (alice.device(), bob.device(), carol.device());
    let carol_to_alice = c.client.safety_number(&certificate(&a)).qr;
    assert_eq!(
        a.client.check_scanned(&certificate(&b), &carol_to_alice),
        Err(SafetyError::Mismatch)
    );
    assert_eq!(
        a.client.check_scanned(&certificate(&b), &[]),
        Err(SafetyError::Malformed)
    );
    let mut truncated = b.client.safety_number(&certificate(&a)).qr;
    truncated.pop();
    assert_eq!(
        a.client.check_scanned(&certificate(&b), &truncated),
        Err(SafetyError::Malformed)
    );
    let mut newer = b.client.safety_number(&certificate(&a)).qr;
    newer[0] = 2;
    assert_eq!(
        a.client.check_scanned(&certificate(&b), &newer),
        Err(SafetyError::UnsupportedVersion(2))
    );
}

#[test]
fn a_new_device_of_a_verified_contact_is_reported_until_acknowledged() {
    let (alice, bob) = (Account::new("alice"), Account::new("bob"));
    let (mut a, mut b1, mut b2) = (alice.device(), bob.device(), bob.device());
    let group = group(&mut a, &mut [&mut b1, &mut b2]);
    assert_eq!(a.client.trust(&certificate(&b1)), Trust::Unverified);

    // Verified through one device: the account's devices already in the group are too.
    a.client.mark_verified(&certificate(&b1)).unwrap();
    a.persist();
    assert_eq!(a.client.trust(&certificate(&b1)), Trust::Verified);
    assert_eq!(a.client.trust(&certificate(&b2)), Trust::Verified);

    let mut b3 = bob.device();
    let key_package = b3.key_package();
    a.client.add_devices(&group, &[key_package]).unwrap();
    a.client.commit_accepted(&group).unwrap();
    a.persist();
    assert_eq!(a.client.trust(&certificate(&b3)), Trust::NewDevice);

    // Still reported after a restart; no longer once acknowledged.
    a.restart();
    assert_eq!(a.client.trust(&certificate(&b3)), Trust::NewDevice);
    a.client.acknowledge_device(&certificate(&b3)).unwrap();
    a.persist();
    a.restart();
    assert_eq!(a.client.trust(&certificate(&b3)), Trust::Verified);
    assert_eq!(a.client.trust(&certificate(&b1)), Trust::Verified);
}

#[test]
fn a_changed_account_key_breaks_the_verification() {
    let (alice, bob) = (Account::new("alice"), Account::new("bob"));
    let (mut a, b) = (alice.device(), bob.device());
    a.client.mark_verified(&certificate(&b)).unwrap();
    a.persist();

    let impostor = Account::new("bob").device();
    assert_eq!(a.client.trust(&certificate(&impostor)), Trust::KeyChanged);
    assert_eq!(
        a.client.acknowledge_device(&certificate(&impostor)),
        Err(SafetyError::KeyChanged)
    );
    a.restart();
    assert_eq!(a.client.trust(&certificate(&impostor)), Trust::KeyChanged);

    // Verifying again, with the new key, makes the old one the suspicious one.
    a.client.mark_verified(&certificate(&impostor)).unwrap();
    assert_eq!(a.client.trust(&certificate(&impostor)), Trust::Verified);
    assert_eq!(a.client.trust(&certificate(&b)), Trust::KeyChanged);
}

#[test]
fn a_contact_can_be_unverified() {
    let (alice, bob) = (Account::new("alice"), Account::new("bob"));
    let (mut a, b) = (alice.device(), bob.device());
    assert_eq!(
        a.client.acknowledge_device(&certificate(&b)),
        Err(SafetyError::NotVerified)
    );
    a.client.mark_verified(&certificate(&b)).unwrap();
    a.persist();
    a.client.unverify(certificate(&b).account()).unwrap();
    a.persist();
    a.restart();
    assert_eq!(a.client.trust(&certificate(&b)), Trust::Unverified);
}

#[test]
fn the_digits_are_five_by_five_from_the_fingerprint() {
    let mut fingerprint = [0u8; 64];
    // 5 bytes 0x00..0x04 = 0x0001020304 = 16_909_060 → 09060.
    fingerprint[..5].copy_from_slice(&[0, 1, 2, 3, 4]);
    fingerprint[5..10].copy_from_slice(&[0xff; 5]);
    let digits = digits(&fingerprint);
    assert_eq!(digits.len(), 30);
    assert_eq!(&digits[..5], "09060");
    // 0xffffffffff = 1_099_511_627_775 → 27775.
    assert_eq!(&digits[5..10], "27775");
    assert_eq!(&digits[10..], "00000".repeat(4));
}
