use crate::messages::{Body, Message};
use crate::test_support::{Account, group};

/// A text sent at `at` (milliseconds): the list orders by the time a message was sent.
fn text_at(at: u64, words: &str) -> Message {
    Message::new(
        at,
        Body::Text {
            text: words.into(),
            reply_to: None,
        },
    )
    .unwrap()
}

#[test]
fn the_list_puts_the_latest_conversation_first_and_counts_what_is_unread() {
    let accounts = (
        Account::new("alice"),
        Account::new("bob"),
        Account::new("carol"),
    );
    let mut alice = accounts.0.device();
    let mut bob = accounts.1.device();
    let mut carol = accounts.2.device();
    let with_bob = group(&mut alice, &mut [&mut bob]);
    let with_carol = group(&mut alice, &mut [&mut carol]);

    // No message yet: nothing unread, no time.
    assert!(
        alice
            .client
            .conversations()
            .iter()
            .all(|c| c.unread == 0 && c.last_sent_at.is_none())
    );

    // Bob writes twice, then Carol once: Carol's conversation comes first.
    for (at, words) in [(1_000, "hi"), (2_000, "are you there?")] {
        let sent = bob.send(&with_bob, &text_at(at, words));
        alice.receive(&sent);
    }
    let sent = carol.send(&with_carol, &text_at(3_000, "hello"));
    alice.receive(&sent);
    let list = alice.client.conversations();
    assert_eq!(
        list.iter()
            .map(|c| (c.group.clone(), c.unread, c.last_sent_at))
            .collect::<Vec<_>>(),
        vec![
            (with_carol.clone(), 1, Some(3_000)),
            (with_bob.clone(), 2, Some(2_000))
        ]
    );

    // Alice answers Bob: her own message is not unread, and his conversation is the
    // latest again.
    alice.send(&with_bob, &text_at(4_000, "yes"));
    let list = alice.client.conversations();
    assert_eq!(
        (list[0].group.clone(), list[0].unread),
        (with_bob.clone(), 2)
    );

    // She reads it: nothing unread there any more, and it holds after a restart.
    alice.client.mark_read(&with_bob).unwrap();
    alice.persist();
    alice.restart();
    let unread: Vec<_> = alice
        .client
        .conversations()
        .iter()
        .map(|c| (c.group.clone(), c.unread))
        .collect();
    assert_eq!(unread, vec![(with_bob.clone(), 0), (with_carol.clone(), 1)]);

    // A new message after the marker is unread again.
    let sent = bob.send(&with_bob, &text_at(5_000, "great"));
    alice.receive(&sent);
    let list = alice.client.conversations();
    assert_eq!((list[0].group.clone(), list[0].unread), (with_bob, 1));
}
