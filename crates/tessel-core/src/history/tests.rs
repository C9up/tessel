use super::*;
use crate::groups::GroupError;
use crate::identity::{AccountKey, DeviceId};
use crate::store::StoreKey;
use crate::test_support::{Account, Peer, delete, edit, group, text};

/// Alice, Bob on two devices, and Carol, in one group.
fn four() -> (GroupId, Peer, Peer, Peer, Peer) {
    let (alice, bob, carol) = (
        Account::new("alice"),
        Account::new("bob"),
        Account::new("carol"),
    );
    let (mut a, mut b1, mut b2, mut c) =
        (alice.device(), bob.device(), bob.device(), carol.device());
    let group = group(&mut a, &mut [&mut b1, &mut b2, &mut c]);
    (group, a, b1, b2, c)
}

/// What a client that ignores the rules would send: the message, encrypted as is.
fn forge(peer: &mut Peer, group: &GroupId, message: &Message) -> Vec<u8> {
    peer.client
        .encrypt(group, &message.encode().unwrap())
        .unwrap()
}

fn content(peer: &Peer, group: &GroupId, id: MessageId) -> Content {
    peer.history(group)
        .into_iter()
        .find(|e| e.id == id)
        .expect("message in the history")
        .content
}

#[test]
fn an_edit_or_a_deletion_applies_only_from_the_authors_account() {
    let (group, mut a, mut b1, mut b2, mut c) = four();
    let original = text("lunch at noon?");
    let sent = b1.send(&group, &original);
    for peer in [&mut a, &mut b2, &mut c] {
        assert_eq!(peer.receive(&sent), Effect::Added);
    }

    // Carol cannot change Bob's message: her client refuses to send it, and when a
    // client sends it anyway, every member ignores it.
    let hijack = edit(original.id, "lunch is cancelled");
    assert_eq!(
        c.client.send_message(&group, &hijack),
        Err(GroupError::Ignored(Ignored::NotAuthor))
    );
    let forged = forge(&mut c, &group, &hijack);
    for peer in [&mut a, &mut b1, &mut b2] {
        assert_eq!(peer.receive(&forged), Effect::Ignored(Ignored::NotAuthor));
    }
    let forged = forge(&mut c, &group, &delete(original.id));
    for peer in [&mut a, &mut b1, &mut b2] {
        assert_eq!(peer.receive(&forged), Effect::Ignored(Ignored::NotAuthor));
    }
    assert!(matches!(
        content(&a, &group, original.id),
        Content::Text { text, edited_at: None, .. } if text == "lunch at noon?"
    ));

    // Bob's other device is the same account: its edit applies.
    let fix = edit(original.id, "lunch at 12:30?");
    let sent = b2.send(&group, &fix);
    for peer in [&mut a, &mut b1, &mut c] {
        assert_eq!(peer.receive(&sent), Effect::Edited);
    }
    for peer in [&a, &b1, &b2, &c] {
        assert!(matches!(
            content(peer, &group, original.id),
            Content::Text { text, edited_at: Some(at), .. } if text == "lunch at 12:30?" && at == fix.sent_at
        ));
    }

    // Bob deletes it: the text leaves every database, a tombstone stays.
    let sent = b1.send(&group, &delete(original.id));
    for peer in [&mut a, &mut b2, &mut c] {
        assert_eq!(peer.receive(&sent), Effect::Deleted);
    }
    for peer in [&a, &b1, &b2, &c] {
        let entry = peer
            .history(&group)
            .into_iter()
            .find(|e| e.id == original.id)
            .unwrap();
        assert_eq!(entry.content, Content::Deleted);
        assert_eq!(
            entry.author,
            b1.client.device().certificate().account().clone()
        );
    }
    assert_eq!(
        b2.client
            .send_message(&group, &edit(original.id, "too late")),
        Err(GroupError::Ignored(Ignored::AlreadyDeleted))
    );
}

#[test]
fn reusing_someone_elses_identifier_does_not_overwrite_their_message() {
    let (group, mut a, mut b1, _b2, mut c) = four();
    let original = text("the code is 4512");
    let sent = b1.send(&group, &original);
    a.receive(&sent);

    let mut copy = text("the code is 0000");
    copy.id = original.id;
    let sent = c.send(&group, &copy);
    assert_eq!(a.receive(&sent), Effect::Added);

    let history = a.history(&group);
    assert_eq!(history.len(), 2);
    let texts: Vec<_> = history
        .iter()
        .map(|e| match &e.content {
            Content::Text { text, .. } => (e.author.to_string(), text.clone()),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(
        texts,
        [
            (
                "bob@tessel.example".to_owned(),
                "the code is 4512".to_owned()
            ),
            (
                "carol@tessel.example".to_owned(),
                "the code is 0000".to_owned()
            ),
        ]
    );

    // Its own duplicate is refused.
    assert_eq!(
        c.client.send_message(&group, &copy),
        Err(GroupError::Ignored(Ignored::DuplicateId))
    );
}

#[test]
fn an_unknown_target_is_ignored() {
    let (group, mut a, mut b1, _b2, _c) = four();
    let ghost = MessageId::from_bytes([9; 16]);
    let forged = forge(&mut b1, &group, &edit(ghost, "boo"));
    assert_eq!(a.receive(&forged), Effect::Ignored(Ignored::UnknownTarget));
    let forged = forge(&mut b1, &group, &delete(ghost));
    assert_eq!(a.receive(&forged), Effect::Ignored(Ignored::UnknownTarget));
    assert!(a.history(&group).is_empty());
}

#[test]
fn the_history_comes_back_in_delivery_order_after_a_restart() {
    let (group, mut a, mut b1, _b2, mut c) = four();
    let mut sent = Vec::new();
    for i in 0..3 {
        let peer = if i == 1 { &mut c } else { &mut b1 };
        let message = text(&format!("message {i}"));
        let encrypted = peer.send(&group, &message);
        a.receive(&encrypted);
        sent.push(message.id);
    }
    let own = text("and mine");
    a.send(&group, &own);
    sent.push(own.id);

    a.restart();
    let ids: Vec<_> = a.history(&group).iter().map(|e| e.id).collect();
    assert_eq!(ids, sent);

    // The index came back too: Bob can still edit his first message.
    let sent = b1.send(&group, &edit(sent[0], "message zero"));
    assert_eq!(a.receive(&sent), Effect::Edited);
}

#[test]
fn only_the_last_messages_of_a_group_can_be_changed() {
    let key = StoreKey::generate().unwrap();
    let sealer = Sealer::new(&key).unwrap();
    let state = key.state_partition().unwrap();
    let account = AccountId::parse("bob@tessel.example").unwrap();
    let account_key = AccountKey::generate().unwrap();
    let certificate = account_key
        .certify(&account, DeviceId::random().unwrap(), &[0x42; 32])
        .unwrap();
    let group = GroupId::from_bytes(b"group");

    let mut history = History::default();
    let mut ids = Vec::new();
    for i in 0..=EDITABLE_MESSAGES {
        let message = text(&format!("{i}"));
        ids.push(message.id);
        assert_eq!(
            history.apply(&sealer, state, &group, &certificate, &message),
            Ok(Effect::Added)
        );
    }
    assert_eq!(history.recent_len(&group), EDITABLE_MESSAGES);
    // The oldest one left the index, and its index record is deleted.
    let changes = history.take();
    assert_eq!(changes.delete.len(), 1);
    assert_eq!(
        history.verdict(&group, &account, &edit(ids[0], "late")),
        Some(Ignored::UnknownTarget)
    );
    assert_eq!(
        history.verdict(&group, &account, &edit(ids[1], "in time")),
        None
    );
}
