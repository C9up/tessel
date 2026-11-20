use openmls::prelude::{BasicCredential, MlsGroup, MlsGroupCreateConfig};
use openmls_rust_crypto::OpenMlsRustCrypto;

use super::*;
use crate::suite::CIPHERSUITE;

fn alice() -> AccountId {
    AccountId::parse("alice@tessel.example").unwrap()
}

#[test]
fn un_compte_valide_se_lit_et_se_reecrit() {
    let account = alice();
    assert_eq!(account.name(), "alice");
    assert_eq!(account.domain(), "tessel.example");
    assert_eq!(account.to_string(), "alice@tessel.example");
    for ok in ["bob.l_2-x@localhost", "abc@a-b.c0", "a12@x.y.z"] {
        assert!(AccountId::parse(ok).is_ok(), "{ok}");
    }
}

#[test]
fn les_comptes_invalides_sont_refuses() {
    for bad in [
        "alice",                       // pas de domaine
        "al@tessel.example",           // trop court
        "Alice@tessel.example",        // majuscule
        "1alice@tessel.example",       // ne commence pas par une lettre
        "\u{0430}lice@tessel.example", // « а » cyrillique : homoglyphe
        "ali ce@tessel.example",
        "alice@Tessel.example",
        "alice@-tessel.example",
        "alice@tessel..example",
        "alice@",
        "alice@tessel.example@x",
        "abcdefghijklmnopqrstuvwxyz0123456@x.y", // 33 caractères
    ] {
        assert!(AccountId::parse(bad).is_err(), "{bad}");
    }
}

#[test]
fn un_certificat_signe_par_le_compte_se_verifie() {
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
fn un_certificat_altere_est_refuse() {
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

    // Une autre clé de compte qui se substitue : la signature ne vérifie plus.
    let mut usurped = device.certificate().clone();
    usurped.account_key = public_key(AccountKey::generate().unwrap().public_key()).unwrap();
    assert_eq!(usurped.verify(), Err(IdentityError::BadSignature));
}

#[test]
fn des_octets_mal_formes_sont_refuses() {
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
fn un_nouvel_appareil_se_fait_certifier_par_un_appareil_du_compte() {
    // Story 03-04 : le nouvel appareil crée sa clé et n'envoie que la publique.
    let account_key = AccountKey::generate().unwrap();
    let new_key = DeviceKey::generate().unwrap();
    let certificate = account_key
        .certify(&alice(), DeviceId::random().unwrap(), new_key.public_key())
        .unwrap();
    let device = Device::new(new_key, certificate.clone()).unwrap();
    assert_eq!(device.certificate(), &certificate);

    // Un certificat fait pour une autre clé est refusé.
    assert_eq!(
        Device::new(DeviceKey::generate().unwrap(), certificate).unwrap_err(),
        IdentityError::KeyMismatch
    );
}

#[test]
fn un_membre_du_groupe_verifie_l_appareil_par_son_credential() {
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

    // La même feuille présentée avec une autre clé de signature est refusée.
    let other = DeviceKey::generate().unwrap();
    assert_eq!(
        DeviceCertificate::from_credential(&member.credential, other.public_key()),
        Err(IdentityError::KeyMismatch)
    );
    // Un credential qui ne porte pas de certificat est refusé.
    let bare: Credential = BasicCredential::new(b"alice@tessel.example".to_vec()).into();
    assert_eq!(
        DeviceCertificate::from_credential(&bare, &member.signature_key),
        Err(IdentityError::MalformedCertificate)
    );
}

#[test]
fn aucune_cle_privee_n_apparait_dans_le_debug() {
    let account_key = AccountKey::generate().unwrap();
    let device = Device::create(&alice(), &account_key).unwrap();
    let private = account_key.0.private.as_slice();
    let rendered = format!("{account_key:?} {device:?}");
    assert!(!rendered.contains(&format!("{private:?}")[1..20]));
    assert!(!rendered.contains("private"));
}

#[test]
fn aucun_prefixe_tronque_ne_fait_paniquer_la_lecture() {
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
fn une_cle_qui_n_est_pas_ed25519_est_refusee() {
    let account_key = AccountKey::generate().unwrap();
    assert_eq!(
        account_key
            .certify(&alice(), DeviceId::random().unwrap(), &[0; 31])
            .unwrap_err(),
        IdentityError::InvalidKey
    );
}
