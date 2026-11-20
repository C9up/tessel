use openmls::prelude::{Ciphersuite, KeyPackage, KeyPackageRef, MlsMessageOut};
use openmls_traits::OpenMlsProvider;
use openmls_traits::storage::StorageProvider;
use tls_codec::Serialize as _;

use super::*;
use crate::client::Client;
use crate::identity::{AccountId, AccountKey};

/// La vérification d'un *KeyPackage* lit l'horloge réelle : les tests partent de maintenant.
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn client() -> Client {
    let account = AccountId::parse("alice@tessel.example").unwrap();
    Client::new(Device::create(&account, &AccountKey::generate().unwrap()).unwrap())
}

fn consume(client: &Client, key_package: &VerifiedKeyPackage) {
    // Ce que fait OpenMLS quand un *Welcome* utilise le *KeyPackage*.
    client
        .provider()
        .storage()
        .delete_key_package(key_package.key_package_ref())
        .unwrap();
}

#[test]
fn le_premier_renouvellement_remplit_la_reserve_et_le_dernier_recours() {
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
        "le KeyPackage porte le certificat de l'appareil"
    );

    // Rien ne change tant que rien n'est consommé ni proche de sa fin.
    assert!(
        client
            .refresh_key_packages(now() + 3600)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn un_keypackage_consomme_est_remplace() {
    let mut client = client();
    let first = client.refresh_key_packages(now()).unwrap();
    let used = verify_key_package(&first.publish[3]).unwrap();
    consume(&client, &used);

    let update = client.refresh_key_packages(now() + 60).unwrap();
    assert_eq!(update.publish.len(), 1);
    assert!(update.last_resort.is_none());
    assert!(update.withdraw.is_empty(), "le serveur l'a déjà donné");
}

#[test]
fn un_keypackage_proche_de_sa_fin_est_retire_remplace_puis_efface() {
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

    // Retirés mais pas encore expirés : un Welcome en route peut encore les utiliser.
    let stored = |client: &Client, k: &VerifiedKeyPackage| {
        client
            .provider()
            .storage()
            .key_package::<KeyPackageRef, KeyPackageBundle>(k.key_package_ref())
            .unwrap()
            .is_some()
    };
    assert!(published.iter().all(|k| stored(&client, k)));

    // Après leur fin de validité, leurs secrets sont effacés, sans rien renvoyer au serveur.
    let update = client
        .refresh_key_packages(now() + KEY_PACKAGE_LIFETIME)
        .unwrap();
    assert!(update.is_empty());
    assert!(!published.iter().any(|k| stored(&client, k)));
}

#[test]
fn un_keypackage_d_une_autre_suite_est_refuse() {
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
fn un_keypackage_altere_est_refuse() {
    let mut client = client();
    let update = client.refresh_key_packages(now()).unwrap();
    let mut bytes = update.publish[0].clone();
    let last = bytes.len() - 1;
    bytes[last] ^= 1; // dans la signature
    assert!(matches!(
        verify_key_package(&bytes),
        Err(KeyPackageError::Invalid(_))
    ));
    assert_eq!(
        verify_key_package(b"not a key package").unwrap_err(),
        KeyPackageError::Malformed
    );
}
