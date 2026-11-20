use openmls::prelude::{Ciphersuite, KeyPackage, KeyPackageRef, MlsMessageOut};
use openmls_traits::OpenMlsProvider;
use openmls_traits::storage::StorageProvider;
use tls_codec::Serialize as _;

use super::*;
use crate::client::Client;
use crate::identity::{AccountId, AccountKey};
use crate::store::StoreKey;

/// Verifying a *KeyPackage* reads the real clock: the tests start from now.
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn client() -> Client {
    let account = AccountId::parse("alice@tessel.example").unwrap();
    Client::new(
        Device::create(&account, &AccountKey::generate().unwrap()).unwrap(),
        &StoreKey::generate().unwrap(),
    )
    .unwrap()
}

fn consume(client: &Client, key_package: &VerifiedKeyPackage) {
    // What OpenMLS does when a *Welcome* uses the *KeyPackage*.
    client
        .provider()
        .storage()
        .delete_key_package(key_package.key_package_ref())
        .unwrap();
}

#[test]
fn the_first_renewal_fills_the_pool_and_the_last_resort() {
    let mut client = client();
    let update = client.refresh_key_packages(now()).unwrap();
    assert_eq!(update.publish.len(), KEY_PACKAGE_POOL);
    assert!(update.withdraw.is_empty());

    let last_resort = verify_key_package(update.last_resort.as_ref().unwrap()).unwrap();
    assert!(last_resort.is_last_resort());
    let one_time = verify_key_package(&update.publish[0]).unwrap();
    assert!(!one_time.is_last_resort());
    assert_eq!(
        one_time.certificate(),
        client.device().certificate(),
        "the KeyPackage carries the device certificate"
    );

    // Nothing changes as long as nothing is consumed or close to its end.
    assert!(
        client
            .refresh_key_packages(now() + 3600)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn a_consumed_keypackage_is_replaced() {
    let mut client = client();
    let first = client.refresh_key_packages(now()).unwrap();
    let used = verify_key_package(&first.publish[3]).unwrap();
    consume(&client, &used);

    let update = client.refresh_key_packages(now() + 60).unwrap();
    assert_eq!(update.publish.len(), 1);
    assert!(update.last_resort.is_none());
    assert!(
        update.withdraw.is_empty(),
        "the server already handed it out"
    );
}

#[test]
fn a_keypackage_near_its_end_is_withdrawn_replaced_then_wiped() {
    let mut client = client();
    let first = client.refresh_key_packages(now()).unwrap();
    let published: Vec<VerifiedKeyPackage> = first
        .publish
        .iter()
        .chain(first.last_resort.iter())
        .map(|b| verify_key_package(b).unwrap())
        .collect();

    let renew_at = now() + KEY_PACKAGE_LIFETIME - KEY_PACKAGE_RENEW_BEFORE + 1;
    let update = client.refresh_key_packages(renew_at).unwrap();
    assert_eq!(update.publish.len(), KEY_PACKAGE_POOL);
    assert!(update.last_resort.is_some());
    assert_eq!(update.withdraw.len(), KEY_PACKAGE_POOL + 1);
    for key_package in &published {
        assert!(update.withdraw.contains(&key_package.reference().to_vec()));
    }

    // Withdrawn but not yet expired: a Welcome in flight may still use them.
    let stored = |client: &Client, k: &VerifiedKeyPackage| {
        client
            .provider()
            .storage()
            .key_package::<KeyPackageRef, KeyPackageBundle>(k.key_package_ref())
            .unwrap()
            .is_some()
    };
    assert!(published.iter().all(|k| stored(&client, k)));

    // After their validity ends, their secrets are wiped, without sending anything to the server.
    let update = client
        .refresh_key_packages(now() + KEY_PACKAGE_LIFETIME)
        .unwrap();
    assert!(update.is_empty());
    assert!(!published.iter().any(|k| stored(&client, k)));
}

#[test]
fn a_keypackage_of_another_suite_is_rejected() {
    let client = client();
    let device = client.device();
    let bundle = KeyPackage::builder()
        .build(
            Ciphersuite::MLS_128_DHKEMX25519_CHACHA20POLY1305_SHA256_Ed25519,
            client.provider(),
            device,
            device.credential_with_key().unwrap(),
        )
        .unwrap();
    let bytes = MlsMessageOut::from(bundle.key_package().clone())
        .tls_serialize_detached()
        .unwrap();
    assert_eq!(
        verify_key_package(&bytes).unwrap_err(),
        KeyPackageError::WrongCiphersuite
    );
}

#[test]
fn a_tampered_keypackage_is_rejected() {
    let mut client = client();
    let update = client.refresh_key_packages(now()).unwrap();
    let mut bytes = update.publish[0].clone();
    let last = bytes.len() - 1;
    bytes[last] ^= 1; // inside the signature
    assert!(matches!(
        verify_key_package(&bytes),
        Err(KeyPackageError::Invalid(_))
    ));
    assert_eq!(
        verify_key_package(b"not a key package").unwrap_err(),
        KeyPackageError::Malformed
    );
}
