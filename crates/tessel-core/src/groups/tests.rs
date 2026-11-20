use openmls::prelude::{
    BasicCredential, Ciphersuite, CredentialWithKey, KeyPackage, MlsGroup, MlsGroupCreateConfig,
};
use openmls_rust_crypto::OpenMlsRustCrypto;
use tls_codec::Serialize as _;

use super::*;
use crate::identity::{AccountKey, DeviceKey};
use crate::key_packages::verify_key_package;
use crate::messages::{Body, MESSAGE_VERSION};

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Un compte et ses appareils.
struct Account {
    id: AccountId,
    key: AccountKey,
}

impl Account {
    fn new(name: &str) -> Self {
        Self {
            id: AccountId::parse(&format!("{name}@tessel.example")).unwrap(),
            key: AccountKey::generate().unwrap(),
        }
    }

    fn device(&self) -> Client {
        let key = DeviceKey::generate().unwrap();
        let certificate = self
            .key
            .certify(&self.id, DeviceId::random().unwrap(), key.public_key())
            .unwrap();
        Client::new(Device::new(key, certificate).unwrap())
    }
}

/// Le premier *KeyPackage* à usage unique que l'appareil publie.
fn key_package(client: &mut Client) -> VerifiedKeyPackage {
    let update = client.refresh_key_packages(now()).unwrap();
    verify_key_package(&update.publish[0]).unwrap()
}

/// Ce que fait le serveur d'un commit accepté : l'auteur reçoit l'accusé, les autres
/// membres le commit.
fn deliver_commit(
    group: &GroupId,
    author: &mut Client,
    others: &mut [&mut Client],
    out: &Outgoing,
) {
    author.commit_accepted(group).unwrap();
    for member in others.iter_mut() {
        assert!(matches!(
            member.process(&out.commit).unwrap(),
            Incoming::Commit { .. }
        ));
    }
}

fn text_message(text: &str) -> Message {
    Message::new(
        now() * 1000,
        Body::Text {
            text: text.into(),
            reply_to: None,
        },
    )
    .unwrap()
}

/// `sender` chiffre un texte ; chaque lecteur doit le lire, chaque exclu non.
fn say(
    group: &GroupId,
    sender: &mut Client,
    readers: &mut [&mut Client],
    excluded: &mut [&mut Client],
    text: &str,
) {
    let sent = text_message(text);
    let message = sender.send_message(group, &sent).unwrap();
    for reader in readers.iter_mut() {
        match reader.process(&message).unwrap() {
            Incoming::Message {
                message,
                sender: from,
                ..
            } => {
                assert_eq!(message, Ok(sent.clone()));
                assert_eq!(&from, sender.device().certificate());
            }
            other => panic!("expected a message, got {other:?}"),
        }
    }
    for outsider in excluded.iter_mut() {
        assert!(
            outsider.process(&message).is_err(),
            "an excluded device read {text:?}"
        );
    }
}

#[test]
fn trois_comptes_deux_appareils_ajouts_retraits_et_depart() {
    let (alice, bob, carol) = (
        Account::new("alice"),
        Account::new("bob"),
        Account::new("carol"),
    );
    let (mut a1, mut a2) = (alice.device(), alice.device());
    let (mut b1, mut b2) = (bob.device(), bob.device());
    let (mut c1, mut c2) = (carol.device(), carol.device());

    // Alice crée le groupe et ajoute son autre appareil, Bob (deux appareils) et Carol (un).
    let group = a1.create_group().unwrap();
    let kps = [
        key_package(&mut a2),
        key_package(&mut b1),
        key_package(&mut b2),
        key_package(&mut c1),
    ];
    let out = a1.add_devices(&group, &kps).unwrap();
    a1.commit_accepted(&group).unwrap();
    let welcome = out.welcome.unwrap();
    for joiner in [&mut a2, &mut b1, &mut b2, &mut c1] {
        assert_eq!(joiner.join_group(&welcome).unwrap(), group);
    }
    assert_eq!(a1.members(&group).unwrap().len(), 5);
    say(
        &group,
        &mut b1,
        &mut [&mut a1, &mut a2, &mut b2, &mut c1],
        &mut [],
        "bonjour",
    );

    // Bob ajoute le second appareil de Carol ; tout le monde le voit arriver.
    let kp = key_package(&mut c2);
    let out = b2.add_devices(&group, &[kp]).unwrap();
    b2.commit_accepted(&group).unwrap();
    for member in [&mut a1, &mut a2, &mut b1, &mut c1] {
        match member.process(&out.commit).unwrap() {
            Incoming::Commit { added, .. } => {
                assert_eq!(added.len(), 1);
                assert_eq!(added[0].account(), &carol.id);
            }
            other => panic!("{other:?}"),
        }
    }
    c2.join_group(&out.welcome.unwrap()).unwrap();
    say(
        &group,
        &mut c2,
        &mut [&mut a1, &mut a2, &mut b1, &mut b2, &mut c1],
        &mut [],
        "me voilà",
    );

    // Alice retire un appareil de Bob ; il ne lit plus rien.
    let b2_id = b2.device().certificate().device_id();
    let out = a1
        .remove_devices(&group, &[Removal::Device(bob.id.clone(), b2_id)])
        .unwrap();
    deliver_commit(
        &group,
        &mut a1,
        &mut [&mut a2, &mut b1, &mut c1, &mut c2],
        &out,
    );
    match b2.process(&out.commit).unwrap() {
        Incoming::Commit {
            removed_self,
            removed,
            ..
        } => {
            assert!(removed_self);
            assert_eq!(removed[0].device_id(), b2_id);
        }
        other => panic!("{other:?}"),
    }
    say(
        &group,
        &mut c1,
        &mut [&mut a1, &mut a2, &mut b1, &mut c2],
        &mut [&mut b2],
        "sans b2",
    );

    // Alice (depuis son second appareil) retire tout le compte de Carol.
    let out = a2
        .remove_devices(&group, &[Removal::Account(carol.id.clone())])
        .unwrap();
    deliver_commit(&group, &mut a2, &mut [&mut a1, &mut b1], &out);
    for gone in [&mut c1, &mut c2] {
        assert!(matches!(
            gone.process(&out.commit).unwrap(),
            Incoming::Commit {
                removed_self: true,
                ..
            }
        ));
    }
    say(
        &group,
        &mut b1,
        &mut [&mut a1, &mut a2],
        &mut [&mut b2, &mut c1, &mut c2],
        "sans carol",
    );

    // Bob quitte : sa proposition entre par le commit d'Alice.
    let proposal = b1.leave_group(&group).unwrap();
    for member in [&mut a1, &mut a2] {
        assert!(matches!(
            member.process(&proposal).unwrap(),
            Incoming::Proposal { .. }
        ));
    }
    let out = a1.commit_pending_proposals(&group).unwrap();
    deliver_commit(&group, &mut a1, &mut [&mut a2], &out);
    assert!(matches!(
        b1.process(&out.commit).unwrap(),
        Incoming::Commit {
            removed_self: true,
            ..
        }
    ));
    say(
        &group,
        &mut a2,
        &mut [&mut a1],
        &mut [&mut b1, &mut b2, &mut c1, &mut c2],
        "entre nous",
    );
    assert_eq!(a1.members(&group).unwrap().len(), 2);
}

#[test]
fn un_commit_refuse_est_rejoue_sur_l_etat_a_jour() {
    let (alice, bob, carol, dave) = (
        Account::new("alice"),
        Account::new("bob"),
        Account::new("carol"),
        Account::new("dave"),
    );
    let (mut a, mut b, mut c, mut d) =
        (alice.device(), bob.device(), carol.device(), dave.device());
    let group = a.create_group().unwrap();
    let out = a.add_devices(&group, &[key_package(&mut b)]).unwrap();
    a.commit_accepted(&group).unwrap();
    b.join_group(&out.welcome.unwrap()).unwrap();

    // Alice et Bob commitent dans la même epoch ; le serveur prend celui d'Alice.
    let kp_c = key_package(&mut c);
    let kp_d = key_package(&mut d);
    let winner = a.add_devices(&group, &[kp_c]).unwrap();
    let loser = b.add_devices(&group, std::slice::from_ref(&kp_d)).unwrap();
    assert_ne!(winner.commit, loser.commit);
    a.commit_accepted(&group).unwrap();
    b.commit_rejected(&group).unwrap();
    b.process(&winner.commit).unwrap();
    c.join_group(&winner.welcome.unwrap()).unwrap();

    // Bob rejoue son ajout de Dave sur l'epoch suivante, sans rien perdre.
    let replay = b.retry_commit(&group).unwrap().unwrap();
    deliver_commit(&group, &mut b, &mut [&mut a, &mut c], &replay);
    d.join_group(&replay.welcome.unwrap()).unwrap();
    assert_eq!(a.epoch(&group).unwrap(), 3);
    say(
        &group,
        &mut d,
        &mut [&mut a, &mut b, &mut c],
        &mut [],
        "tous là",
    );

    // Une intention déjà accomplie par un autre n'a plus d'objet.
    let mut e = Account::new("eve").device();
    let kp_e = key_package(&mut e);
    let first = a.add_devices(&group, std::slice::from_ref(&kp_e)).unwrap();
    let _second = c.add_devices(&group, &[kp_e]).unwrap();
    deliver_commit(&group, &mut a, &mut [&mut b, &mut d], &first);
    c.commit_rejected(&group).unwrap();
    c.process(&first.commit).unwrap();
    assert_eq!(c.retry_commit(&group).unwrap(), None);
}

#[test]
fn un_commit_recu_annule_le_notre_sans_perdre_l_intention() {
    // Le refus du serveur peut arriver après le commit gagnant : le traiter suffit.
    let (alice, bob, carol) = (
        Account::new("alice"),
        Account::new("bob"),
        Account::new("carol"),
    );
    let (mut a, mut b, mut c) = (alice.device(), bob.device(), carol.device());
    let group = a.create_group().unwrap();
    let out = a.add_devices(&group, &[key_package(&mut b)]).unwrap();
    a.commit_accepted(&group).unwrap();
    b.join_group(&out.welcome.unwrap()).unwrap();

    let winner = a
        .remove_devices(&group, &[Removal::Account(bob.id.clone())])
        .unwrap();
    let kp_c = key_package(&mut c);
    let _loser = b.add_devices(&group, &[kp_c]).unwrap();
    a.commit_accepted(&group).unwrap();
    assert!(matches!(
        b.process(&winner.commit).unwrap(),
        Incoming::Commit {
            removed_self: true,
            ..
        }
    ));
    assert_eq!(b.retry_commit(&group), Err(GroupError::UnknownGroup));
}

#[test]
fn un_welcome_d_une_autre_suite_est_refuse() {
    let (alice, bob) = (Account::new("alice"), Account::new("bob"));
    let (a, mut b) = (alice.device(), bob.device());
    let other = Ciphersuite::MLS_128_DHKEMX25519_CHACHA20POLY1305_SHA256_Ed25519;
    // Bob a un KeyPackage de l'autre suite (fabriqué à la main : Tessel n'en crée pas).
    let bob_kp = KeyPackage::builder()
        .build(
            other,
            &b.provider,
            &b.device,
            b.device.credential_with_key().unwrap(),
        )
        .unwrap();
    let provider = OpenMlsRustCrypto::default();
    let config = MlsGroupCreateConfig::builder()
        .ciphersuite(other)
        .use_ratchet_tree_extension(true)
        .build();
    let mut group = MlsGroup::new(
        &provider,
        &a.device,
        &config,
        a.device.credential_with_key().unwrap(),
    )
    .unwrap();
    let (_, welcome, _) = group
        .add_members(&provider, &a.device, &[bob_kp.key_package().clone()])
        .unwrap();
    let welcome = welcome.tls_serialize_detached().unwrap();
    assert_eq!(b.join_group(&welcome), Err(GroupError::WrongCiphersuite));
}

#[test]
fn un_appareil_sans_certificat_ne_peut_pas_entrer() {
    let (alice, bob) = (Account::new("alice"), Account::new("bob"));
    let (mut a, mut b) = (alice.device(), bob.device());
    let group = a.create_group().unwrap();
    let out = a.add_devices(&group, &[key_package(&mut b)]).unwrap();
    a.commit_accepted(&group).unwrap();
    b.join_group(&out.welcome.unwrap()).unwrap();

    // Un membre (ici Alice, en passant sous Tessel) ajoute un appareil qui ne porte
    // qu'un nom, sans certificat.
    let signer = Device::create(&alice.id, &alice.key).unwrap();
    let bare = CredentialWithKey {
        credential: BasicCredential::new(b"mallory@tessel.example".to_vec()).into(),
        signature_key: signer.credential_with_key().unwrap().signature_key,
    };
    let provider = OpenMlsRustCrypto::default();
    let kp = KeyPackage::builder()
        .leaf_node_capabilities(capabilities())
        .build(CIPHERSUITE, &provider, &signer, bare)
        .unwrap();
    let commit = {
        let provider = &a.provider;
        let device = &a.device;
        let mls = &mut a.groups.get_mut(&group).unwrap().mls;
        let (commit, _, _) = mls
            .add_members(provider, device, &[kp.key_package().clone()])
            .unwrap();
        commit.tls_serialize_detached().unwrap()
    };
    assert_eq!(
        b.process(&commit),
        Err(GroupError::Identity(IdentityError::MalformedCertificate))
    );
}

#[test]
fn un_message_mal_forme_est_refuse_sans_planter() {
    let mut a = Account::new("alice").device();
    let group = a.create_group().unwrap();
    assert_eq!(a.process(b"\x00\x01garbage"), Err(GroupError::Malformed));
    assert_eq!(a.join_group(b""), Err(GroupError::Malformed));
    let message = a.send_message(&group, &text_message("echo")).unwrap();
    assert_eq!(a.process(&message), Err(GroupError::OwnMessage));
}

/// Alice et Bob, un groupe à deux.
fn pair() -> (GroupId, Client, Client) {
    let mut a = Account::new("alice").device();
    let mut b = Account::new("bob").device();
    let group = a.create_group().unwrap();
    let out = a.add_devices(&group, &[key_package(&mut b)]).unwrap();
    a.commit_accepted(&group).unwrap();
    b.join_group(out.welcome.as_ref().unwrap()).unwrap();
    (group, a, b)
}

#[test]
fn un_contenu_illisible_est_rendu_avec_son_expediteur_et_le_groupe_continue() {
    let (group, mut a, mut b) = pair();
    for (data, error) in [
        (
            &b"pas un message"[..],
            MessageError::UnsupportedVersion(b'p'),
        ),
        (&[MESSAGE_VERSION, 99][..], MessageError::UnknownKind(99)),
        (&[MESSAGE_VERSION][..], MessageError::Malformed),
    ] {
        let message = a.encrypt(&group, data).unwrap();
        match b.process(&message).unwrap() {
            Incoming::Message {
                sender, message, ..
            } => {
                assert_eq!(message, Err(error));
                assert_eq!(&sender, a.device().certificate());
            }
            other => panic!("expected a message, got {other:?}"),
        }
    }
    say(&group, &mut a, &mut [&mut b], &mut [], "toujours là");
}

#[test]
fn la_taille_chiffree_ne_trahit_pas_la_longueur_d_un_texte_court() {
    let (group, mut a, _b) = pair();
    let short = a.send_message(&group, &text_message("ok")).unwrap();
    let long = a
        .send_message(
            &group,
            &text_message("d'accord, je passe te prendre à 19 h devant la gare"),
        )
        .unwrap();
    assert_eq!(short.len(), long.len());

    let invalid = text_message("");
    assert!(matches!(
        a.send_message(&group, &invalid),
        Err(GroupError::Message(MessageError::Invalid(_)))
    ));
}
