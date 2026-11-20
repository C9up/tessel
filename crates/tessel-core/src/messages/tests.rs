use super::*;

fn id(byte: u8) -> MessageId {
    MessageId::from_bytes([byte; 16])
}

fn media() -> MediaRef {
    MediaRef {
        blob: vec![7; 16],
        key: MediaKey::from_bytes([9; 32]),
        digest: [3; 32],
        size: 1_234_567,
        content_type: "image/jpeg".into(),
        name: "plage.jpg".into(),
    }
}

/// Un message de chaque genre.
fn samples() -> Vec<Message> {
    let bodies = [
        Body::Text {
            text: "salut 👋".into(),
            reply_to: None,
        },
        Body::Text {
            text: "oui !".into(),
            reply_to: Some(id(1)),
        },
        Body::Media {
            media: media(),
            caption: "regarde".into(),
            reply_to: Some(id(2)),
        },
        Body::Receipt {
            kind: ReceiptKind::Read,
            messages: vec![id(1), id(2), id(3)],
        },
        Body::Receipt {
            kind: ReceiptKind::Delivered,
            messages: vec![id(4)],
        },
        Body::Edit {
            target: id(1),
            text: "salut 👋 (édité)".into(),
        },
        Body::Delete { target: id(2) },
    ];
    bodies
        .into_iter()
        .enumerate()
        .map(|(i, body)| Message {
            id: id(100 + i as u8),
            sent_at: 1_795_000_000_000 + i as u64,
            body,
        })
        .collect()
}

#[test]
fn chaque_genre_fait_l_aller_retour() {
    for message in samples() {
        let bytes = message.encode().unwrap();
        assert_eq!(Message::decode(&bytes).unwrap(), message);
    }
}

#[test]
fn un_nouveau_message_a_un_identifiant_au_hasard() {
    let body = Body::Delete { target: id(1) };
    let (a, b) = (
        Message::new(0, body.clone()).unwrap(),
        Message::new(0, body).unwrap(),
    );
    assert_ne!(a.id, b.id);
}

#[test]
fn la_cle_d_un_media_ne_s_affiche_pas() {
    let shown = format!("{:?}", media());
    assert!(shown.contains("MediaKey(..)"));
    assert!(!shown.contains("[9, 9"));
}

#[test]
fn padme_garde_les_paliers_attendus() {
    assert_eq!(padded_len(1), 256);
    assert_eq!(padded_len(256), 256);
    assert_eq!(padded_len(257), 272);
    assert_eq!(padded_len(1000), 1024);
    assert_eq!(padded_len(1025), 1088);
    for len in 1..70_000 {
        let padded = padded_len(len);
        assert!(padded >= len);
        // Au plus 12 % de surcoût au-delà du minimum.
        assert!(
            len < MIN_PADDED || padded * 100 <= len * 112,
            "{len} → {padded}"
        );
    }
}

#[test]
fn les_textes_courts_ont_tous_la_meme_taille() {
    let size = |text: &str| {
        Message {
            id: id(0),
            sent_at: 0,
            body: Body::Text {
                text: text.into(),
                reply_to: None,
            },
        }
        .encode()
        .unwrap()
        .len()
    };
    assert_eq!(size("ok"), MIN_PADDED);
    assert_eq!(
        size("ok"),
        size("d'accord, je passe te prendre à 19 h devant la gare")
    );
    assert_eq!(size(&"a".repeat(1000)), size(&"a".repeat(1010)));
}

#[test]
fn chaque_prefixe_et_chaque_octet_change_sont_refuses_sans_planter() {
    for message in samples() {
        let bytes = message.encode().unwrap();
        for end in 0..bytes.len() {
            assert!(Message::decode(&bytes[..end]).is_err(), "prefix {end}");
        }
        let mut longer = bytes.clone();
        longer.push(0);
        assert!(Message::decode(&longer).is_err());
        // Changer un octet donne une erreur ou un autre message, jamais une panique.
        for i in 0..bytes.len() {
            for flip in [0x01, 0x80, 0xff] {
                let mut changed = bytes.clone();
                changed[i] ^= flip;
                let _ = Message::decode(&changed);
            }
        }
    }
}

#[test]
fn un_remplissage_non_nul_est_refuse() {
    let mut bytes = samples()[0].encode().unwrap();
    *bytes.last_mut().unwrap() = 1;
    assert_eq!(Message::decode(&bytes), Err(MessageError::Malformed));
}

#[test]
fn version_et_genre_inconnus_sont_nommes() {
    let mut bytes = samples()[0].encode().unwrap();
    bytes[0] = 2;
    assert_eq!(
        Message::decode(&bytes),
        Err(MessageError::UnsupportedVersion(2))
    );
    bytes[0] = MESSAGE_VERSION;
    bytes[1] = 42;
    assert_eq!(Message::decode(&bytes), Err(MessageError::UnknownKind(42)));
}

#[test]
fn les_regles_du_format_tiennent_a_l_envoi_comme_a_la_reception() {
    let invalid = [
        Body::Text {
            text: String::new(),
            reply_to: None,
        },
        Body::Text {
            text: "a".repeat(MAX_TEXT + 1),
            reply_to: None,
        },
        Body::Edit {
            target: id(1),
            text: String::new(),
        },
        Body::Receipt {
            kind: ReceiptKind::Read,
            messages: vec![],
        },
        Body::Receipt {
            kind: ReceiptKind::Read,
            messages: vec![id(1); MAX_RECEIPT + 1],
        },
        Body::Media {
            media: MediaRef {
                blob: vec![],
                ..media()
            },
            caption: String::new(),
            reply_to: None,
        },
        Body::Media {
            media: MediaRef {
                content_type: String::new(),
                ..media()
            },
            caption: String::new(),
            reply_to: None,
        },
        Body::Media {
            media: MediaRef {
                name: "n".repeat(256),
                ..media()
            },
            caption: String::new(),
            reply_to: None,
        },
    ];
    for body in invalid {
        let message = Message {
            id: id(0),
            sent_at: 0,
            body,
        };
        assert!(
            matches!(message.encode(), Err(MessageError::Invalid(_))),
            "{message:?}"
        );
    }

    // Le plus long texte permis passe.
    let longest = Message {
        id: id(0),
        sent_at: 0,
        body: Body::Text {
            text: "a".repeat(MAX_TEXT),
            reply_to: None,
        },
    };
    assert_eq!(
        Message::decode(&longest.encode().unwrap()).unwrap(),
        longest
    );
}

#[test]
fn un_accuse_vide_ou_de_l_utf8_invalide_sont_refuses_a_la_reception() {
    // Un accusé de zéro message, rempli comme il faut.
    let mut empty = vec![MESSAGE_VERSION, KIND_RECEIPT];
    empty.extend_from_slice(&[0; 16 + 8]);
    empty.extend_from_slice(&[2, 0, 0]);
    empty.resize(padded_len(empty.len()), 0);
    assert_eq!(Message::decode(&empty), Err(MessageError::Malformed));

    // Un texte qui n'est pas de l'UTF-8.
    let mut bad = vec![MESSAGE_VERSION, KIND_TEXT];
    bad.extend_from_slice(&[0; 16 + 8]);
    bad.extend_from_slice(&[0, 0, 2, 0xc3, 0x28]);
    bad.resize(padded_len(bad.len()), 0);
    assert_eq!(Message::decode(&bad), Err(MessageError::Malformed));
}

#[test]
fn des_octets_au_hasard_ne_font_jamais_planter() {
    // Un générateur déterministe (xorshift) : le test se rejoue à l'identique.
    let mut state = 0x9e37_79b9_7f4a_7c15_u64;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for round in 0..20_000 {
        let len = (next() % 600) as usize;
        let mut bytes: Vec<u8> = (0..len).map(|_| next() as u8).collect();
        // La moitié des essais commence comme un vrai message, pour aller plus loin.
        if round % 2 == 0 && len >= 2 {
            bytes[0] = MESSAGE_VERSION;
            bytes[1] = (next() % 5) as u8 + 1;
        }
        let _ = Message::decode(&bytes);
    }
}
