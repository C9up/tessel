use crate::groups::GroupId;
use crate::history::{Effect, Status};
use crate::messages::{MAX_RECEIPT, MessageId, ReceiptKind};
use crate::test_support::{Account, Peer, group, text};

/// Sends everything in the group's queue, as a host does; what the server relays.
fn flush(peer: &mut Peer, group: &GroupId) -> Vec<Vec<u8>> {
    let mut sent = Vec::new();
    while let Some(outbound) = peer.client.next_outbound(group).unwrap() {
        peer.client.outbound_sent(group, &outbound.send_id).unwrap();
        sent.push(outbound.encrypted);
    }
    peer.persist();
    sent
}

/// Queues what the device owes and sends it.
fn acknowledge(peer: &mut Peer, group: &GroupId) -> Vec<Vec<u8>> {
    let queued = peer.client.queue_receipts(1_000).unwrap();
    peer.persist();
    if queued.contains(group) {
        flush(peer, group)
    } else {
        Vec::new()
    }
}

/// The status and receipts of the device's message `id`.
fn status(
    peer: &Peer,
    group: &GroupId,
    id: MessageId,
) -> (Option<Status>, Vec<(String, ReceiptKind)>) {
    let entry = peer
        .history(group)
        .into_iter()
        .find(|entry| entry.id == id)
        .unwrap();
    (
        entry.status,
        entry
            .receipts
            .into_iter()
            .map(|receipt| (receipt.account.to_string(), receipt.kind))
            .collect(),
    )
}

#[test]
fn a_message_goes_from_pending_to_sent_delivered_and_read() {
    let (alice_account, bob_account) = (Account::new("alice"), Account::new("bob"));
    let (mut alice, mut bob) = (alice_account.device(), bob_account.device());
    let chat = group(&mut alice, &mut [&mut bob]);

    let hello = text("hello");
    alice.client.queue_message(&chat, &hello).unwrap();
    alice.persist();
    assert_eq!(status(&alice, &chat, hello.id).0, Some(Status::Pending));
    let sent = flush(&mut alice, &chat);
    assert_eq!(
        status(&alice, &chat, hello.id),
        (Some(Status::Sent), vec![])
    );

    // Bob's device gets it: it owes a delivery, which Alice records.
    assert_eq!(bob.receive(&sent[0]), Effect::Added);
    let delivered = acknowledge(&mut bob, &chat);
    assert_eq!(delivered.len(), 1);
    assert_eq!(alice.receive(&delivered[0]), Effect::Receipt);
    assert_eq!(
        status(&alice, &chat, hello.id),
        (
            Some(Status::Delivered),
            vec![("bob@tessel.example".into(), ReceiptKind::Delivered)]
        )
    );

    // Bob reads: Alice sees it read, after a restart too.
    bob.client.mark_read(&chat).unwrap();
    let read = acknowledge(&mut bob, &chat);
    assert_eq!(alice.receive(&read[0]), Effect::Receipt);
    alice.restart();
    assert_eq!(
        status(&alice, &chat, hello.id),
        (
            Some(Status::Read),
            vec![("bob@tessel.example".into(), ReceiptKind::Read)]
        )
    );
    // Nothing more is owed: reading again sends nothing.
    bob.client.mark_read(&chat).unwrap();
    assert!(acknowledge(&mut bob, &chat).is_empty());
    // Other accounts' messages carry no status.
    assert_eq!(status(&bob, &chat, hello.id), (None, vec![]));
}

#[test]
fn in_a_group_a_message_is_delivered_and_read_once_every_other_account_is() {
    let accounts = [
        Account::new("alice"),
        Account::new("bob"),
        Account::new("carol"),
    ];
    let [mut alice, mut bob, mut carol] = accounts.each_ref().map(Account::device);
    let family = group(&mut alice, &mut [&mut bob, &mut carol]);
    let hello = text("hello all");
    let sent = alice.send(&family, &hello);

    bob.receive(&sent);
    bob.client.mark_read(&family).unwrap();
    for receipt in acknowledge(&mut bob, &family) {
        alice.receive(&receipt);
        carol.receive(&receipt);
    }
    // Bob read it, Carol has not got it: only sent.
    assert_eq!(status(&alice, &family, hello.id).0, Some(Status::Sent));

    carol.receive(&sent);
    for receipt in acknowledge(&mut carol, &family) {
        alice.receive(&receipt);
    }
    assert_eq!(status(&alice, &family, hello.id).0, Some(Status::Delivered));
    carol.client.mark_read(&family).unwrap();
    for receipt in acknowledge(&mut carol, &family) {
        alice.receive(&receipt);
    }
    assert_eq!(status(&alice, &family, hello.id).0, Some(Status::Read));
}

#[test]
fn read_receipts_off_send_none_and_record_none() {
    let (alice_account, bob_account) = (Account::new("alice"), Account::new("bob"));
    let (mut alice, mut bob) = (alice_account.device(), bob_account.device());
    let chat = group(&mut alice, &mut [&mut bob]);
    assert!(bob.client.read_receipts());

    // Bob turns them off: his delivery still goes, his reading does not.
    bob.client.set_read_receipts(false).unwrap();
    bob.persist();
    bob.restart();
    assert!(!bob.client.read_receipts());
    let hello = text("hello");
    let sent = alice.send(&chat, &hello);
    bob.receive(&sent);
    bob.client.mark_read(&chat).unwrap();
    let receipts = acknowledge(&mut bob, &chat);
    assert_eq!(receipts.len(), 1);
    alice.receive(&receipts[0]);
    assert_eq!(status(&alice, &chat, hello.id).0, Some(Status::Delivered));

    // Reciprocal: with them off, Alice does not record Bob's reading either.
    bob.client.set_read_receipts(true).unwrap();
    alice.client.set_read_receipts(false).unwrap();
    let again = text("again");
    let sent = alice.send(&chat, &again);
    bob.receive(&sent);
    bob.client.mark_read(&chat).unwrap();
    for receipt in acknowledge(&mut bob, &chat) {
        alice.receive(&receipt);
    }
    assert_eq!(status(&alice, &chat, again.id).0, Some(Status::Delivered));
}

#[test]
fn what_one_device_reads_is_read_on_the_account_s_other_ones() {
    let (alice_account, bob_account) = (Account::new("alice"), Account::new("bob"));
    let (mut phone, mut tablet, mut bob) = (
        alice_account.device(),
        alice_account.device(),
        bob_account.device(),
    );
    let chat = group(&mut bob, &mut [&mut phone, &mut tablet]);
    let sent = bob.send(&chat, &text("hi Alice"));
    phone.receive(&sent);
    tablet.receive(&sent);
    assert_eq!(phone.client.conversations()[0].unread, 1);

    // Alice reads on her tablet: her phone counts nothing unread any more.
    tablet.client.mark_read(&chat).unwrap();
    for receipt in acknowledge(&mut tablet, &chat) {
        assert_eq!(phone.receive(&receipt), Effect::Nothing);
    }
    assert_eq!(phone.client.conversations()[0].unread, 0);
}

#[test]
fn receipts_owed_survive_a_restart_and_leave_by_batches() {
    let (alice_account, bob_account) = (Account::new("alice"), Account::new("bob"));
    let (mut alice, mut bob) = (alice_account.device(), bob_account.device());
    let chat = group(&mut alice, &mut [&mut bob]);
    let count = MAX_RECEIPT + 10;
    for i in 0..count {
        let sent = alice.send(&chat, &text(&format!("message {i}")));
        bob.receive(&sent);
    }
    bob.restart();

    // One delivery for 266 messages would be too long: two receipts.
    let receipts = acknowledge(&mut bob, &chat);
    assert_eq!(receipts.len(), 2);
    for receipt in &receipts {
        alice.receive(receipt);
    }
    let delivered = alice
        .history(&chat)
        .iter()
        .filter(|entry| entry.status == Some(Status::Delivered))
        .count();
    assert_eq!(delivered, count);
    assert!(bob.client.queue_receipts(2_000).unwrap().is_empty());
}
