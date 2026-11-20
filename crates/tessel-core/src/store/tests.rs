use super::*;

fn new_sealer() -> (StoreKey, Sealer) {
    let key = StoreKey::generate().unwrap();
    let sealer = Sealer::new(&key).unwrap();
    (key, sealer)
}

#[test]
fn a_sealed_record_opens_with_its_label_and_body() {
    let (key, sealer) = new_sealer();
    let partition = key.state_partition().unwrap();
    let record = sealer
        .seal(partition, b"label", b"name", b"secret body")
        .unwrap();
    assert_eq!(record.value[0], STORE_FORMAT);
    assert_eq!(
        record.id,
        sealer.record_id(&partition, b"label", b"name").unwrap()
    );

    let opened = sealer.open(&record).unwrap();
    assert_eq!(opened.label, b"label");
    assert_eq!(opened.body.as_slice(), b"secret body");
}

#[test]
fn identifiers_are_stable_for_a_key_and_unrelated_across_keys() {
    let (key, sealer) = new_sealer();
    let (other_key, other) = new_sealer();
    assert_eq!(key.state_partition(), key.state_partition());
    assert_ne!(key.state_partition(), other_key.state_partition());

    let partition = sealer.partition(b"history", b"group").unwrap();
    assert_eq!(partition, sealer.partition(b"history", b"group").unwrap());
    assert_ne!(partition, other.partition(b"history", b"group").unwrap());
    assert_ne!(partition, sealer.partition(b"histor", b"ygroup").unwrap());
    assert_ne!(
        sealer.record_id(&partition, b"a", b"b").unwrap(),
        sealer.record_id(&partition, b"ab", b"").unwrap()
    );
}

#[test]
fn the_host_sees_no_plaintext() {
    let (key, sealer) = new_sealer();
    let record = sealer
        .seal(
            key.state_partition().unwrap(),
            b"message",
            b"alice@tessel.example",
            b"see you at noon",
        )
        .unwrap();
    let bytes = [
        record.partition.as_bytes().as_slice(),
        record.id.as_bytes(),
        &record.value,
    ]
    .concat();
    for clear in [&b"see you at noon"[..], b"alice", b"message"] {
        assert!(!bytes.windows(clear.len()).any(|w| w == clear));
    }
}

#[test]
fn sizes_only_betray_an_order_of_magnitude() {
    let (key, sealer) = new_sealer();
    let partition = key.state_partition().unwrap();
    let a = sealer.seal(partition, b"m", b"1", &[7; 1000]).unwrap();
    let b = sealer.seal(partition, b"m", b"2", &[7; 1010]).unwrap();
    assert_eq!(a.value.len(), b.value.len());
}

#[test]
fn a_record_does_not_open_with_another_key() {
    let (key, sealer) = new_sealer();
    let (_, other) = new_sealer();
    let record = sealer
        .seal(key.state_partition().unwrap(), b"l", b"n", b"body")
        .unwrap();
    assert_eq!(other.open(&record).err(), Some(StoreError::Unreadable));
}

#[test]
fn a_moved_or_altered_record_does_not_open() {
    let (key, sealer) = new_sealer();
    let partition = key.state_partition().unwrap();
    let record = sealer.seal(partition, b"l", b"n", b"body").unwrap();

    let moved = Record {
        id: sealer.record_id(&partition, b"l", b"other").unwrap(),
        ..record.clone()
    };
    assert_eq!(sealer.open(&moved).err(), Some(StoreError::Unreadable));
    let elsewhere = Record {
        partition: sealer.partition(b"history", b"g").unwrap(),
        ..record.clone()
    };
    assert_eq!(sealer.open(&elsewhere).err(), Some(StoreError::Unreadable));

    for i in 1..record.value.len() {
        let mut altered = record.clone();
        altered.value[i] ^= 0x01;
        assert_eq!(
            sealer.open(&altered).err(),
            Some(StoreError::Unreadable),
            "byte {i}"
        );
    }
    for len in 0..record.value.len() {
        let mut truncated = record.clone();
        truncated.value.truncate(len);
        assert!(sealer.open(&truncated).is_err());
    }
}

#[test]
fn a_record_from_a_newer_core_is_named() {
    let (key, sealer) = new_sealer();
    let mut record = sealer
        .seal(key.state_partition().unwrap(), b"l", b"n", b"b")
        .unwrap();
    record.value[0] = STORE_FORMAT + 1;
    assert_eq!(
        sealer.open(&record).err(),
        Some(StoreError::UnsupportedFormat(STORE_FORMAT + 1))
    );
}

#[test]
fn the_store_key_is_never_displayed_and_has_32_bytes() {
    let key = StoreKey::generate().unwrap();
    assert_eq!(format!("{key:?}"), "StoreKey(..)");
    assert_eq!(
        StoreKey::from_bytes(&[0; 31]).err(),
        Some(StoreError::InvalidKey)
    );
    let again = StoreKey::from_bytes(key.to_bytes().as_slice()).unwrap();
    assert_eq!(key.state_partition(), again.state_partition());
}
