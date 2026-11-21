use super::*;
use crate::test_support::{Account, group, text};

#[test]
fn the_header_says_group_epoch_and_kind_and_nothing_else() {
    let mut alice = Account::new("alice").device();
    let mut bob = Account::new("bob").device();
    let shared = group(&mut alice, &mut [&mut bob]);
    let epoch = alice.client.epoch(&shared).unwrap();

    let message = alice.send(&shared, &text("a secret"));
    let header = inspect_envelope(&message).unwrap();
    assert_eq!(
        header,
        EnvelopeHeader {
            kind: EnvelopeKind::Application,
            group: Some(shared.clone()),
            epoch: Some(epoch),
        }
    );
    // The content is not in the clear.
    assert!(!message.windows(8).any(|w| w == b"a secret"));

    let mut carol = Account::new("carol").device();
    let key_package = carol.key_package();
    let out = alice.client.add_devices(&shared, &[key_package]).unwrap();
    let commit = inspect_envelope(&out.commit).unwrap();
    assert_eq!(commit.kind, EnvelopeKind::Commit);
    assert_eq!(commit.epoch, Some(epoch));
    let welcome = inspect_envelope(out.welcome.as_ref().unwrap()).unwrap();
    assert_eq!(
        welcome,
        EnvelopeHeader {
            kind: EnvelopeKind::Welcome,
            group: None,
            epoch: None,
        }
    );

    let proposal = bob.client.leave_group(&shared).unwrap();
    assert_eq!(
        inspect_envelope(&proposal).unwrap().kind,
        EnvelopeKind::Proposal
    );
}

#[test]
fn what_is_not_a_group_message_is_refused() {
    let mut carol = Account::new("carol").device();
    let update = carol
        .client
        .refresh_key_packages(crate::test_support::now())
        .unwrap();
    // A KeyPackage goes to the directory, not to a mailbox.
    assert_eq!(
        inspect_envelope(&update.publish[0]),
        Err(EnvelopeError::Malformed)
    );
    assert_eq!(inspect_envelope(&[1, 2, 3]), Err(EnvelopeError::Malformed));
    assert_eq!(inspect_envelope(&[]), Err(EnvelopeError::Malformed));
}

#[test]
fn recipients_are_every_member_but_this_device() {
    let mut alice = Account::new("alice").device();
    let mut bob = Account::new("bob").device();
    let mut carol = Account::new("carol").device();
    let shared = group(&mut alice, &mut [&mut bob, &mut carol]);
    let mut recipients = alice.client.recipients(&shared).unwrap();
    recipients.sort_by_key(|d| *d.as_bytes());
    let mut expected = vec![
        bob.client.device().certificate().device_id(),
        carol.client.device().certificate().device_id(),
    ];
    expected.sort_by_key(|d| *d.as_bytes());
    assert_eq!(recipients, expected);
}
