use crate::groups::{GroupError, GroupId, Incoming};
use crate::history::{Content, Effect, Ignored};
use crate::messages::Message;
use crate::outbox::Outbound;
use crate::test_support::{Account, Peer, group, text};

fn pair() -> (GroupId, Peer, Peer) {
    let (mut a, mut b) = (Account::new("alice").device(), Account::new("bob").device());
    let group = group(&mut a, &mut [&mut b]);
    (group, a, b)
}

/// Writes a message, as the app does, and persists.
fn write(peer: &mut Peer, group: &GroupId, message: &Message) {
    peer.client.queue_message(group, message).unwrap();
    peer.persist();
}

/// The head of the queue, encrypted, persisted before it leaves.
fn next(peer: &mut Peer, group: &GroupId) -> Option<Outbound> {
    let outbound = peer.client.next_outbound(group).unwrap();
    peer.persist();
    outbound
}

fn ack(peer: &mut Peer, outbound: &Outbound) {
    peer.client
        .outbound_sent(&outbound.group, &outbound.send_id)
        .unwrap();
    peer.persist();
}

fn texts(peer: &Peer, group: &GroupId) -> Vec<String> {
    peer.history(group)
        .into_iter()
        .map(|e| match e.content {
            Content::Text { text, .. } => text,
            other => panic!("{other:?}"),
        })
        .collect()
}

#[test]
fn messages_written_offline_leave_after_a_restart_once_and_in_order() {
    let (group, mut a, mut b) = pair();
    let written = [
        text("are you there?"),
        text("the train is late"),
        text("20 min"),
    ];
    for message in &written {
        write(&mut a, &group, message);
    }
    // Shown at once, waiting to be sent.
    assert_eq!(texts(&a, &group).len(), 3);
    assert_eq!(
        a.client.waiting_messages(&group),
        written.iter().map(|m| m.id).collect::<Vec<_>>()
    );

    // The app is closed while offline; the queue survives in the database.
    a.restart();
    assert_eq!(a.client.outbound_groups(), std::slice::from_ref(&group));
    while let Some(outbound) = next(&mut a, &group) {
        assert_eq!(b.receive(&outbound.encrypted), Effect::Added);
        ack(&mut a, &outbound);
    }
    assert_eq!(
        texts(&b, &group),
        ["are you there?", "the train is late", "20 min"]
    );
    assert!(a.client.waiting_messages(&group).is_empty());
    assert!(a.client.outbound_groups().is_empty());

    a.restart();
    assert_eq!(next(&mut a, &group), None);
}

#[test]
fn the_head_comes_back_until_the_server_acknowledges_it() {
    let (group, mut a, mut b) = pair();
    write(&mut a, &group, &text("first"));
    write(&mut a, &group, &text("second"));

    let attempt = next(&mut a, &group).unwrap();
    // The acknowledgement is lost: the same message comes back, under the same send
    // identifier, encrypted again; the next one waits behind it.
    let retry = next(&mut a, &group).unwrap();
    assert_eq!(retry.send_id, attempt.send_id);
    assert_eq!(retry.message_id, attempt.message_id);
    assert_ne!(retry.encrypted, attempt.encrypted);

    // Should both reach Bob, the second is ignored.
    assert_eq!(b.receive(&attempt.encrypted), Effect::Added);
    assert_eq!(
        b.receive(&retry.encrypted),
        Effect::Ignored(Ignored::DuplicateId)
    );

    ack(&mut a, &retry);
    ack(&mut a, &attempt);
    let second = next(&mut a, &group).unwrap();
    assert_ne!(second.send_id, attempt.send_id);
    assert_eq!(b.receive(&second.encrypted), Effect::Added);
    assert_eq!(texts(&b, &group), ["first", "second"]);
}

#[test]
fn each_group_has_its_own_queue() {
    let (one, mut a, mut b) = pair();
    let two = group(&mut a, &mut [&mut b]);
    write(&mut a, &one, &text("in one"));
    write(&mut a, &two, &text("in two"));

    let mut groups = a.client.outbound_groups();
    groups.sort_by(|x, y| x.as_bytes().cmp(y.as_bytes()));
    let mut expected = [one.clone(), two.clone()];
    expected.sort_by(|x, y| x.as_bytes().cmp(y.as_bytes()));
    assert_eq!(groups, expected);

    // The second group does not wait for the first.
    let from_two = next(&mut a, &two).unwrap();
    assert_eq!(b.receive(&from_two.encrypted), Effect::Added);
    ack(&mut a, &from_two);
    assert_eq!(texts(&b, &two), ["in two"]);
    assert_eq!(a.client.outbound_groups(), [one]);
}

#[test]
fn a_queued_message_is_encrypted_for_the_members_present_when_it_leaves() {
    let (alice, bob, carol) = (
        Account::new("alice"),
        Account::new("bob"),
        Account::new("carol"),
    );
    let (mut a, mut b, mut c) = (alice.device(), bob.device(), carol.device());
    let group = group(&mut a, &mut [&mut b, &mut c]);

    // Bob writes while offline; meanwhile Carol leaves.
    write(&mut b, &group, &text("just between us"));
    let proposal = c.client.leave_group(&group).unwrap();
    for peer in [&mut a, &mut b] {
        assert!(matches!(
            peer.client.process(&proposal).unwrap(),
            Incoming::Proposal { .. }
        ));
        peer.persist();
    }
    // MLS forbids sending while the departure awaits its commit.
    assert_eq!(
        b.client.next_outbound(&group),
        Err(GroupError::PendingProposals)
    );

    let out = a.client.commit_pending_proposals(&group).unwrap();
    a.client.commit_accepted(&group).unwrap();
    a.persist();
    b.client.process(&out.commit).unwrap();
    b.persist();
    c.client.process(&out.commit).unwrap();

    let outbound = next(&mut b, &group).unwrap();
    assert_eq!(a.receive(&outbound.encrypted), Effect::Added);
    assert!(c.client.process(&outbound.encrypted).is_err());
}

#[test]
fn a_removed_device_learns_which_messages_will_never_leave() {
    let (group, mut a, mut b) = pair();
    let stranded = text("see you tomorrow");
    write(&mut b, &group, &stranded);

    let out = a
        .client
        .remove_devices(
            &group,
            &[crate::groups::Removal::Account(
                b.client.device().certificate().account().clone(),
            )],
        )
        .unwrap();
    a.client.commit_accepted(&group).unwrap();
    match b.client.process(&out.commit).unwrap() {
        Incoming::Commit {
            removed_self: true,
            unsent,
            ..
        } => assert_eq!(unsent, [stranded.id]),
        other => panic!("expected the removal, got {other:?}"),
    }
    b.persist();
    b.restart();
    assert!(b.client.outbound_groups().is_empty());
    assert_eq!(
        b.client.next_outbound(&group),
        Err(GroupError::UnknownGroup)
    );
}

#[test]
fn acknowledging_again_or_an_unknown_message_changes_nothing() {
    let (group, mut a, _b) = pair();
    write(&mut a, &group, &text("hello"));
    let outbound = next(&mut a, &group).unwrap();
    ack(&mut a, &outbound);
    ack(&mut a, &outbound);
    let unknown = GroupId::from_bytes(b"no such group");
    a.client.outbound_sent(&unknown, &outbound.send_id).unwrap();
    assert!(a.client.take_changes().unwrap().is_empty());
}

#[test]
fn the_queue_is_sealed_in_the_database() {
    let (group, mut a, _b) = pair();
    let secret = "the door code is 4512";
    write(&mut a, &group, &text(secret));
    for record in a.state() {
        assert!(
            !record
                .value
                .windows(secret.len())
                .any(|w| w == secret.as_bytes())
        );
    }
}
