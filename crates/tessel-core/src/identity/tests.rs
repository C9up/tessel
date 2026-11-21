use openmls::prelude::{BasicCredential, MlsGroup, MlsGroupCreateConfig};
use openmls_rust_crypto::OpenMlsRustCrypto;

use super::*;
use crate::suite::CIPHERSUITE;

fn alice() -> AccountId {
    AccountId::parse("alice@tessel.example").unwrap()
}

#[test]
fn a_valid_account_parses_and_round_trips() {
    let account = alice();
    assert_eq!(account.name(), "alice");
    assert_eq!(account.domain(), "tessel.example");
    assert_eq!(account.to_string(), "alice@tessel.example");
    for ok in ["bob.l_2-x@localhost", "abc@a-b.c0", "a12@x.y.z"] {
        assert!(AccountId::parse(ok).is_ok(), "{ok}");
    }
}

#[test]
fn invalid_accounts_are_rejected() {
    for bad in [
        "alice",                       // no domain
        "al@tessel.example",           // too short
        "Alice@tessel.example",        // uppercase
        "1alice@tessel.example",       // does not start with a letter
        "\u{0430}lice@tessel.example", // Cyrillic "а": homoglyph
        "ali ce@tessel.example",
        "alice@Tessel.example",
        "alice@-tessel.example",
        "alice@tessel..example",
        "alice@",
        "alice@tessel.example@x",
        "abcdefghijklmnopqrstuvwxyz0123456@x.y", // 33 characters
    ] {
        assert!(AccountId::parse(bad).is_err(), "{bad}");
    }
}

#[test]
fn a_certificate_signed_by_the_account_verifies() {
    let account_key = AccountKey::generate().unwrap();
    let device = Device::create(&alice(), &account_key).unwrap();
    let certificate = device.certificate();
    certificate.verify().unwrap();
    assert_eq!(certificate.account(), &alice());
    assert_eq!(certificate.account_key(), account_key.public_key());

    let read = DeviceCertificate::from_bytes(&certificate.to_bytes().unwrap()).unwrap();
    assert_eq!(&read, certificate);
}

#[test]
fn a_tampered_certificate_is_rejected() {
    let account_key = AccountKey::generate().unwrap();
    let device = Device::create(&alice(), &account_key).unwrap();

    let mut other_device = device.certificate().clone();
    other_device.device_id = DeviceId::random().unwrap();
    assert_eq!(other_device.verify(), Err(IdentityError::BadSignature));

    let mut other_account = device.certificate().clone();
    other_account.account = AccountId::parse("mallory@tessel.example").unwrap();
    assert_eq!(other_account.verify(), Err(IdentityError::BadSignature));

    let mut other_key = device.certificate().clone();
    other_key.device_key = public_key(DeviceKey::generate().unwrap().public_key()).unwrap();
    assert_eq!(other_key.verify(), Err(IdentityError::BadSignature));

    // Another account key substituting itself: the signature no longer verifies.
    let mut usurped = device.certificate().clone();
    usurped.account_key = public_key(AccountKey::generate().unwrap().public_key()).unwrap();
    assert_eq!(usurped.verify(), Err(IdentityError::BadSignature));
}

#[test]
fn malformed_bytes_are_rejected() {
    let account_key = AccountKey::generate().unwrap();
    let device = Device::create(&alice(), &account_key).unwrap();
    let bytes = device.certificate().to_bytes().unwrap();

    let mut trailing = bytes.clone();
    trailing.push(0);
    assert_eq!(
        DeviceCertificate::from_bytes(&trailing),
        Err(IdentityError::MalformedCertificate)
    );
    assert_eq!(
        DeviceCertificate::from_bytes(&bytes[..bytes.len() - 1]),
        Err(IdentityError::MalformedCertificate)
    );
    let mut future = bytes;
    future[0] = 2;
    assert_eq!(
        DeviceCertificate::from_bytes(&future),
        Err(IdentityError::MalformedCertificate)
    );
}

#[test]
fn a_new_device_is_certified_by_a_device_of_the_account() {
    // Story 03-04: the new device creates its key and only sends the public part.
    let account_key = AccountKey::generate().unwrap();
    let new_key = DeviceKey::generate().unwrap();
    let certificate = account_key
        .certify(&alice(), DeviceId::random().unwrap(), new_key.public_key())
        .unwrap();
    let device = Device::new(new_key, certificate.clone()).unwrap();
    assert_eq!(device.certificate(), &certificate);

    // A certificate made for another key is rejected.
    assert_eq!(
        Device::new(DeviceKey::generate().unwrap(), certificate).unwrap_err(),
        IdentityError::KeyMismatch
    );
}

#[test]
fn a_group_member_verifies_the_device_through_its_credential() {
    let account_key = AccountKey::generate().unwrap();
    let device = Device::create(&alice(), &account_key).unwrap();
    let provider = OpenMlsRustCrypto::default();
    let config = MlsGroupCreateConfig::builder()
        .ciphersuite(CIPHERSUITE)
        .use_ratchet_tree_extension(true)
        .build();
    let group = MlsGroup::new(
        &provider,
        &device,
        &config,
        device.credential_with_key().unwrap(),
    )
    .unwrap();

    let member = group.members().next().unwrap();
    let verified =
        DeviceCertificate::from_credential(&member.credential, &member.signature_key).unwrap();
    assert_eq!(verified.account(), &alice());
    assert_eq!(verified.device_id(), device.certificate().device_id());
    assert_eq!(verified.account_key(), account_key.public_key());

    // The same leaf presented with another signing key is rejected.
    let other = DeviceKey::generate().unwrap();
    assert_eq!(
        DeviceCertificate::from_credential(&member.credential, other.public_key()),
        Err(IdentityError::KeyMismatch)
    );
    // A credential that carries no certificate is rejected.
    let bare: Credential = BasicCredential::new(b"alice@tessel.example".to_vec()).into();
    assert_eq!(
        DeviceCertificate::from_credential(&bare, &member.signature_key),
        Err(IdentityError::MalformedCertificate)
    );
}

#[test]
fn no_private_key_appears_in_debug() {
    let account_key = AccountKey::generate().unwrap();
    let device = Device::create(&alice(), &account_key).unwrap();
    let private = account_key.0.private.as_slice();
    let rendered = format!("{account_key:?} {device:?}");
    assert!(!rendered.contains(&format!("{private:?}")[1..20]));
    assert!(!rendered.contains("private"));
}

#[test]
fn no_truncated_prefix_makes_parsing_panic() {
    let account_key = AccountKey::generate().unwrap();
    let device = Device::create(&alice(), &account_key).unwrap();
    let bytes = device.certificate().to_bytes().unwrap();
    for len in 0..bytes.len() {
        assert_eq!(
            DeviceCertificate::from_bytes(&bytes[..len]),
            Err(IdentityError::MalformedCertificate),
            "{len}"
        );
    }
}

#[test]
fn a_key_that_is_not_ed25519_is_rejected() {
    let account_key = AccountKey::generate().unwrap();
    assert_eq!(
        account_key
            .certify(&alice(), DeviceId::random().unwrap(), &[0; 31])
            .unwrap_err(),
        IdentityError::InvalidKey
    );
}

#[test]
fn a_sign_in_proof_verifies_once_for_its_challenge_and_device_only() {
    let account_key = AccountKey::generate().unwrap();
    let device = Device::create(&alice(), &account_key).unwrap();
    let other = Device::create(&alice(), &account_key).unwrap();
    let challenge = [7; CHALLENGE_LEN];
    let proof = device.sign_in(&challenge).unwrap();

    device
        .certificate()
        .verify_sign_in(&challenge, &proof)
        .unwrap();
    // Another challenge, another device of the same account: the proof says nothing.
    assert_eq!(
        device
            .certificate()
            .verify_sign_in(&[8; CHALLENGE_LEN], &proof),
        Err(IdentityError::BadSignature)
    );
    assert_eq!(
        other.certificate().verify_sign_in(&challenge, &proof),
        Err(IdentityError::BadSignature)
    );
    let mut tampered = proof.clone();
    tampered[0] ^= 1;
    assert_eq!(
        device.certificate().verify_sign_in(&challenge, &tampered),
        Err(IdentityError::BadSignature)
    );
}

#[test]
fn a_sign_in_proof_is_not_an_mls_signature() {
    let account_key = AccountKey::generate().unwrap();
    let device = Device::create(&alice(), &account_key).unwrap();
    let challenge = [7; CHALLENGE_LEN];
    // The bare challenge, signed as the device signs MLS content, is no proof.
    let raw = Signer::sign(&device, &challenge).unwrap();
    assert_eq!(
        device.certificate().verify_sign_in(&challenge, &raw),
        Err(IdentityError::BadSignature)
    );
}

#[test]
fn a_challenge_of_the_wrong_size_is_refused() {
    let account_key = AccountKey::generate().unwrap();
    let device = Device::create(&alice(), &account_key).unwrap();
    assert_eq!(
        device.sign_in(&[0; 31]).unwrap_err(),
        IdentityError::MalformedChallenge
    );
    assert_eq!(
        device.certificate().verify_sign_in(&[0; 33], &[0; 64]),
        Err(IdentityError::MalformedChallenge)
    );
}
