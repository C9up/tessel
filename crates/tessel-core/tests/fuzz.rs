//! Hostile input (story 02-12): every way bytes enter the core from outside — a message
//! from the server, a *Welcome*, a *KeyPackage* from the directory, a certificate, a
//! scanned QR, a media blob, a sealed media secret — gets thousands of damaged variants
//! of valid input, and pure noise.
//!
//! Two properties hold for each: the core never panics, and it never accepts a damaged
//! input where a signature or an authentication tag protects it.
//!
//! A short run (`FUZZ_RUNS` per target, deterministic) takes part in every `pnpm check`;
//! `TESSEL_FUZZ_RUNS=<n>` and `TESSEL_FUZZ_SEED=<n>` run longer or elsewhere.

use std::panic::{AssertUnwindSafe, catch_unwind};

use tessel_core::{
    AccountId, Body, Client, DeviceCertificate, EncryptedMedia, GroupId, MediaSeal, Message,
    MessageId, ReceiptKind, StoreKey, decrypt_media, encrypt_media, verify_key_package,
};

const FUZZ_RUNS: usize = 400;

fn runs() -> usize {
    std::env::var("TESSEL_FUZZ_RUNS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(FUZZ_RUNS)
}

/// xorshift64*: reproducible damage from a seed.
struct Damage(u64);

impl Damage {
    fn new(target: &str) -> Self {
        let seed = std::env::var("TESSEL_FUZZ_SEED")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(0x007e_55e1_u64);
        // One stream per target, so that adding a target does not change the others.
        let salt = target.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3)
        });
        Self((seed ^ salt) | 1)
    }

    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % u64::try_from(n.max(1)).unwrap_or(1)).unwrap_or(0)
    }

    fn byte(&mut self) -> u8 {
        self.next().to_le_bytes()[0]
    }

    /// A damaged copy of `valid`, never equal to it; one time in sixteen, pure noise.
    fn variant(&mut self, valid: &[u8]) -> Vec<u8> {
        loop {
            let mut bytes = if self.below(16) == 0 {
                (0..self.below(512)).map(|_| self.byte()).collect()
            } else {
                valid.to_vec()
            };
            for _ in 0..=self.below(4) {
                self.damage(&mut bytes);
            }
            if bytes != valid {
                return bytes;
            }
        }
    }

    fn damage(&mut self, bytes: &mut Vec<u8>) {
        let at = self.below(bytes.len() + 1);
        let i = at.min(bytes.len().saturating_sub(1));
        match self.below(7) {
            0 if !bytes.is_empty() => {
                let bit = 1 << self.below(8);
                bytes[i] ^= bit;
            }
            1 if !bytes.is_empty() => {
                let value = self.byte();
                bytes[i] = value;
            }
            // Lengths are where parsers break: their extreme values.
            2 if !bytes.is_empty() => {
                let value = [0x00, 0x7f, 0x80, 0xff][self.below(4)];
                bytes[i] = value;
            }
            3 => bytes.truncate(at),
            4 => {
                let end = (at + self.below(64)).min(bytes.len());
                bytes.drain(at..end);
            }
            5 => {
                let extra: Vec<u8> = (0..=self.below(32)).map(|_| self.byte()).collect();
                bytes.splice(at..at, extra);
            }
            _ => {
                let end = (at + self.below(64)).min(bytes.len());
                let copy = bytes[at..end].to_vec();
                bytes.splice(at..at, copy);
            }
        }
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Feeds `runs()` damaged variants of each seed to `target`, which returns whether the
/// core accepted the input. A panic, or an acceptance where `must_refuse`, fails with the
/// input, to replay.
fn fuzz(name: &str, seeds: &[Vec<u8>], must_refuse: bool, mut target: impl FnMut(&[u8]) -> bool) {
    let mut damage = Damage::new(name);
    for _ in 0..runs() {
        let seed = &seeds[damage.below(seeds.len())];
        let input = damage.variant(seed);
        match catch_unwind(AssertUnwindSafe(|| target(&input))) {
            Err(_) => panic!("{name}: the core panicked on {}", hex(&input)),
            Ok(true) if must_refuse => panic!("{name}: damaged input accepted: {}", hex(&input)),
            Ok(_) => {}
        }
    }
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn client(name: &str) -> Client {
    let account = AccountId::parse(&format!("{name}@tessel.example")).expect("account");
    Client::create_account(&account, &StoreKey::generate().expect("key")).expect("client")
}

fn key_package(client: &mut Client) -> Vec<u8> {
    let update = client.refresh_key_packages(now()).expect("key packages");
    update.publish.into_iter().next().expect("one key package")
}

fn text(text: &str) -> Message {
    Message::new(
        1_000,
        Body::Text {
            text: text.into(),
            reply_to: None,
        },
    )
    .expect("message")
}

/// Alice and Bob in a group; what Alice sent Bob, still unread by him.
struct Pair {
    alice: Client,
    bob: Client,
    group: GroupId,
}

impl Pair {
    fn new() -> (Self, Vec<u8>) {
        let (mut alice, mut bob) = (client("alice"), client("bob"));
        let package = verify_key_package(&key_package(&mut bob)).expect("valid");
        let group = alice.create_group().expect("group");
        let welcome = alice
            .add_devices(&group, &[package])
            .expect("add")
            .welcome
            .expect("welcome");
        alice.commit_accepted(&group).expect("accepted");
        bob.join_group(&welcome).expect("join");
        (Self { alice, bob, group }, welcome)
    }

    fn sent(&mut self, message: &Message) -> Vec<u8> {
        let send = self
            .alice
            .queue_message(&self.group, message)
            .expect("queued");
        let outbound = self
            .alice
            .next_outbound(&self.group)
            .expect("next")
            .expect("one");
        self.alice.outbound_sent(&self.group, &send).expect("sent");
        outbound.encrypted
    }
}

#[test]
fn a_damaged_mls_message_never_crashes_the_core_nor_gets_in() {
    let (mut pair, _) = Pair::new();
    let application = pair.sent(&text("hello"));
    // A commit, encrypted for Bob, adding a device of Carol.
    let mut carol = client("carol");
    let package = verify_key_package(&key_package(&mut carol)).expect("valid");
    let commit = pair
        .alice
        .add_devices(&pair.group, &[package])
        .expect("add")
        .commit;
    let proposal = pair.bob.leave_group(&pair.group).expect("leave");

    // Bob never processed the originals: only a damaged one could change his state, and
    // none may (the tag of a PrivateMessage covers it all).
    fuzz("process", &[application, commit, proposal], true, |input| {
        pair.bob.process(input).is_ok()
    });
    // And the group still works for him.
    let after = pair.sent(&text("still there"));
    assert!(pair.bob.process(&after).is_ok());
}

#[test]
fn a_damaged_welcome_never_crashes_the_core_nor_gets_in() {
    // Bob's KeyPackage is consumed only by a valid Welcome: he never joins.
    let (mut alice, mut bob) = (client("alice"), client("bob"));
    let package = verify_key_package(&key_package(&mut bob)).expect("valid");
    let group = alice.create_group().expect("group");
    let welcome = alice
        .add_devices(&group, &[package])
        .expect("add")
        .welcome
        .expect("welcome");
    fuzz("join_group", &[welcome], true, |input| {
        bob.join_group(input).is_ok()
    });
}

#[test]
fn a_damaged_key_package_or_certificate_never_crashes_the_core() {
    let mut bob = client("bob");
    let package = key_package(&mut bob);
    // A KeyPackage may in theory decode under another encoding of the same values: only
    // the absence of a crash is required.
    fuzz("verify_key_package", &[package], false, |input| {
        verify_key_package(input).is_ok()
    });

    // A certificate has one encoding: any change breaks its signature.
    let certificate = bob.device().certificate().to_bytes().expect("bytes");
    fuzz("certificate", &[certificate], true, |input| {
        DeviceCertificate::from_bytes(input).is_ok_and(|c| c.verify().is_ok())
    });
}

#[test]
fn a_damaged_application_message_never_crashes_the_decoder() {
    let id = MessageId::random().expect("id");
    let (blob, media) = encrypt_media(b"a small picture").expect("media");
    let media = media.into_ref(b"blob-1".to_vec(), "image/png".into(), "cat.png".into());
    drop(blob);
    let bodies = [
        Body::Text {
            text: "hello".into(),
            reply_to: Some(id),
        },
        Body::Media {
            media,
            caption: "a cat".into(),
            reply_to: None,
        },
        Body::Receipt {
            kind: ReceiptKind::Read,
            messages: vec![id, id],
        },
        Body::Edit {
            target: id,
            text: "hello!".into(),
        },
        Body::Delete { target: id },
    ];
    let seeds: Vec<Vec<u8>> = bodies
        .into_iter()
        .map(|body| {
            Message::new(1_000, body)
                .and_then(|m| m.encode())
                .expect("encoded")
        })
        .collect();
    // Inside MLS the bytes are authenticated: the decoder only has to never crash.
    fuzz("message_decode", &seeds, false, |input| {
        Message::decode(input).is_ok()
    });
}

#[test]
fn a_damaged_media_blob_never_crashes_the_core_nor_decrypts() {
    // Three chunks and a bit: damage can land in the header, a chunk, a tag, the end.
    let file: Vec<u8> = (0..150_000_u32).map(|i| (i % 251) as u8).collect();
    let (blob, media) = encrypt_media(&file).expect("media");
    let media = media.into_ref(b"blob-1".to_vec(), "video/mp4".into(), String::new());
    fuzz("decrypt_media", &[blob], true, |input| {
        decrypt_media(&media, input).is_ok()
    });

    let seal = MediaSeal::new().expect("seal");
    let secret = seal.seal(&EncryptedMedia::of(&media)).expect("sealed");
    fuzz("media_secret", &[secret], true, |input| {
        seal.open(input).is_ok()
    });
}

#[test]
fn a_damaged_safety_qr_is_never_taken_for_the_right_one() {
    let (alice, bob) = (client("alice"), client("bob"));
    let qr = bob.safety_number(alice.device().certificate()).qr;
    let contact = bob.device().certificate().clone();
    fuzz("check_scanned", &[qr], true, |input| {
        alice.check_scanned(&contact, input).is_ok()
    });
}

#[test]
#[should_panic(expected = "the core panicked")]
fn the_harness_reports_a_panic() {
    fuzz("harness_panic", &[vec![1, 2, 3]], false, |input| {
        assert!(input.len() < 2, "simulated crash");
        false
    });
}

#[test]
#[should_panic(expected = "damaged input accepted")]
fn the_harness_reports_an_acceptance() {
    fuzz("harness_accept", &[vec![1, 2, 3]], true, |_| true);
}
