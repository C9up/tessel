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
        name: "beach.jpg".into(),
    }
}

/// One message of each kind.
fn samples() -> Vec<Message> {
    let bodies = [
        Body::Text {
            text: "hi 👋".into(),
            reply_to: None,
        },
        Body::Text {
            text: "yes!".into(),
            reply_to: Some(id(1)),
        },
        Body::Media {
            media: media(),
            caption: "look".into(),
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
            text: "hi 👋 (edited)".into(),
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
fn every_kind_round_trips() {
    for message in samples() {
        let bytes = message.encode().unwrap();
        assert_eq!(Message::decode(&bytes).unwrap(), message);
    }
}

#[test]
fn a_new_message_has_a_random_identifier() {
    let body = Body::Delete { target: id(1) };
    let (a, b) = (
        Message::new(0, body.clone()).unwrap(),
        Message::new(0, body).unwrap(),
    );
    assert_ne!(a.id, b.id);
}

#[test]
fn a_media_key_is_never_displayed() {
    let shown = format!("{:?}", media());
    assert!(shown.contains("MediaKey(..)"));
    assert!(!shown.contains("[9, 9"));
}

#[test]
fn padme_keeps_the_expected_buckets() {
    assert_eq!(padded_len(1), 256);
    assert_eq!(padded_len(256), 256);
    assert_eq!(padded_len(257), 272);
    assert_eq!(padded_len(1000), 1024);
    assert_eq!(padded_len(1025), 1088);
    for len in 1..70_000 {
        let padded = padded_len(len);
        assert!(padded >= len);
        // At most 12% overhead beyond the minimum.
        assert!(
            len < MIN_PADDED || padded * 100 <= len * 112,
            "{len} → {padded}"
        );
    }
}

#[test]
fn short_texts_all_have_the_same_size() {
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
        size("sure, I'll pick you up at 7 pm in front of the station 🚉")
    );
    assert_eq!(size(&"a".repeat(1000)), size(&"a".repeat(1010)));
}

#[test]
fn every_prefix_and_every_altered_byte_is_rejected_without_panicking() {
    for message in samples() {
        let bytes = message.encode().unwrap();
        for end in 0..bytes.len() {
            assert!(Message::decode(&bytes[..end]).is_err(), "prefix {end}");
        }
        let mut longer = bytes.clone();
        longer.push(0);
        assert!(Message::decode(&longer).is_err());
        // Altering a byte yields an error or another message, never a panic.
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
fn nonzero_padding_is_rejected() {
    let mut bytes = samples()[0].encode().unwrap();
    *bytes.last_mut().unwrap() = 1;
    assert_eq!(Message::decode(&bytes), Err(MessageError::Malformed));
}

#[test]
fn unknown_version_and_kind_are_named() {
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
fn the_format_rules_hold_on_sending_as_on_receiving() {
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

    // The longest allowed text goes through.
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
fn an_empty_receipt_or_invalid_utf8_is_rejected_on_receiving() {
    // A receipt of zero messages, padded properly.
    let mut empty = vec![MESSAGE_VERSION, KIND_RECEIPT];
    empty.extend_from_slice(&[0; 16 + 8]);
    empty.extend_from_slice(&[2, 0, 0]);
    empty.resize(padded_len(empty.len()), 0);
    assert_eq!(Message::decode(&empty), Err(MessageError::Malformed));

    // A text that is not UTF-8.
    let mut bad = vec![MESSAGE_VERSION, KIND_TEXT];
    bad.extend_from_slice(&[0; 16 + 8]);
    bad.extend_from_slice(&[0, 0, 2, 0xc3, 0x28]);
    bad.resize(padded_len(bad.len()), 0);
    assert_eq!(Message::decode(&bad), Err(MessageError::Malformed));
}

#[test]
fn random_bytes_never_cause_a_panic() {
    // A deterministic generator (xorshift): the test replays identically.
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
        // Half of the attempts start like a real message, to get further.
        if round % 2 == 0 && len >= 2 {
            bytes[0] = MESSAGE_VERSION;
            bytes[1] = (next() % 5) as u8 + 1;
        }
        let _ = Message::decode(&bytes);
    }
}
