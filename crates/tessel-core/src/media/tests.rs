use super::*;

const CHUNK: usize = MEDIA_CHUNK;
const SEALED: usize = MEDIA_CHUNK + TAG_LEN;

/// A file whose every byte depends on its position.
fn file(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i % 251) as u8).collect()
}

fn reference(media: EncryptedMedia) -> MediaRef {
    media.into_ref(b"blob-1".to_vec(), "image/jpeg".into(), "cat.jpg".into())
}

/// Encrypts `data` fed in pieces of `piece` bytes.
fn encrypt_in_pieces(data: &[u8], piece: usize) -> (Vec<u8>, MediaRef) {
    let mut encryptor = MediaEncryptor::new().unwrap();
    let mut blob = Vec::new();
    for part in data.chunks(piece.max(1)) {
        blob.extend(encryptor.update(part).unwrap());
    }
    let (end, media) = encryptor.finish().unwrap();
    blob.extend(end);
    (blob, reference(media))
}

/// Decrypts `blob` fed in pieces of `piece` bytes.
fn decrypt_in_pieces(media: &MediaRef, blob: &[u8], piece: usize) -> Result<Vec<u8>, MediaError> {
    let mut decryptor = MediaDecryptor::new(media);
    let mut out = Vec::new();
    for part in blob.chunks(piece.max(1)) {
        out.extend(decryptor.update(part)?);
    }
    out.extend(decryptor.finish()?);
    Ok(out)
}

#[test]
fn every_size_round_trips_whatever_the_pieces() {
    for len in [0, 1, CHUNK - 1, CHUNK, CHUNK + 1, 2 * CHUNK, 3 * CHUNK + 5] {
        let data = file(len);
        for piece in [1000, CHUNK, CHUNK + 7, len.max(1)] {
            let (blob, media) = encrypt_in_pieces(&data, piece);
            assert_eq!(blob.len() as u64, encrypted_len(len as u64), "size {len}");
            assert_eq!(media.size, len as u64);
            for back in [999, SEALED, len + 100] {
                assert_eq!(
                    decrypt_in_pieces(&media, &blob, back).as_deref(),
                    Ok(&data[..]),
                    "size {len}, pieces {piece}/{back}"
                );
            }
        }
    }
}

#[test]
fn a_chunk_is_handed_out_once_verified_and_never_the_last_one_early() {
    let data = file(2 * CHUNK + 10);
    let (blob, media) = encrypt_media(&data).unwrap();
    let media = reference(media);
    let mut decryptor = MediaDecryptor::new(&media);
    // Header and first chunk: it may be the last one, so nothing comes out yet.
    assert!(
        decryptor
            .update(&blob[..HEADER_LEN + SEALED])
            .unwrap()
            .is_empty()
    );
    let first = decryptor
        .update(&blob[HEADER_LEN + SEALED..HEADER_LEN + SEALED + 1])
        .unwrap();
    assert_eq!(first, data[..CHUNK]);
    let rest = decryptor.update(&blob[HEADER_LEN + SEALED + 1..]).unwrap();
    assert_eq!(rest, data[CHUNK..2 * CHUNK]);
    assert_eq!(decryptor.finish().unwrap(), data[2 * CHUNK..]);
}

#[test]
fn every_file_has_its_own_key() {
    let data = file(100);
    let (one, a) = encrypt_media(&data).unwrap();
    let (two, b) = encrypt_media(&data).unwrap();
    assert_ne!(one, two);
    assert_ne!(a.digest(), b.digest());
    let (a, b) = (reference(a), reference(b));
    assert_ne!(a.key, b.key);
    // One file's key does not open the other.
    let crossed = MediaRef {
        key: a.key.clone(),
        ..b.clone()
    };
    assert_eq!(decrypt_media(&crossed, &two), Err(MediaError::Altered));
}

#[test]
fn the_file_does_not_appear_in_the_blob() {
    let secret = b"the door code is 4512, the door code is 4512";
    let (blob, _) = encrypt_media(secret).unwrap();
    assert!(!blob.windows(16).any(|w| secret.windows(16).any(|s| s == w)));
}

#[test]
fn any_altered_byte_is_detected() {
    let data = file(2 * CHUNK + 300);
    let (blob, media) = encrypt_media(&data).unwrap();
    let media = reference(media);
    let positions = [
        1,                         // nonce prefix
        HEADER_LEN,                // first chunk
        HEADER_LEN + SEALED - 1,   // its tag
        HEADER_LEN + SEALED + 500, // second chunk
        blob.len() - 1,            // last tag
    ];
    for position in positions {
        let mut altered = blob.clone();
        altered[position] ^= 0x01;
        assert_eq!(
            decrypt_media(&media, &altered),
            Err(MediaError::Altered),
            "byte {position}"
        );
    }
    let mut format = blob.clone();
    format[0] = 2;
    assert_eq!(
        decrypt_media(&media, &format),
        Err(MediaError::UnsupportedFormat(2))
    );
}

#[test]
fn a_blob_cut_short_extended_or_reordered_is_refused() {
    let data = file(3 * CHUNK);
    let (blob, media) = encrypt_media(&data).unwrap();
    let media = reference(media);

    // Cut on a chunk boundary: the remaining chunks are intact, but none is the last.
    let cut = &blob[..HEADER_LEN + 2 * SEALED];
    assert_eq!(decrypt_media(&media, cut), Err(MediaError::Altered));
    assert_eq!(
        decrypt_media(&media, &blob[..HEADER_LEN + 3]),
        Err(MediaError::Truncated)
    );
    assert_eq!(
        decrypt_media(&media, &blob[..4]),
        Err(MediaError::Truncated)
    );
    assert_eq!(decrypt_media(&media, &[]), Err(MediaError::Truncated));

    let mut extended = blob.clone();
    extended.extend_from_slice(&[0; TAG_LEN]);
    assert_eq!(decrypt_media(&media, &extended), Err(MediaError::Altered));

    let mut swapped = blob[..HEADER_LEN].to_vec();
    swapped.extend_from_slice(&blob[HEADER_LEN + SEALED..HEADER_LEN + 2 * SEALED]);
    swapped.extend_from_slice(&blob[HEADER_LEN..HEADER_LEN + SEALED]);
    swapped.extend_from_slice(&blob[HEADER_LEN + 2 * SEALED..]);
    assert_eq!(decrypt_media(&media, &swapped), Err(MediaError::Altered));
}

#[test]
fn a_blob_that_does_not_match_the_message_is_refused() {
    let data = file(1000);
    let (blob, media) = encrypt_media(&data).unwrap();
    let media = reference(media);

    let mut digest = media.clone();
    digest.digest[0] ^= 0x01;
    assert_eq!(decrypt_media(&digest, &blob), Err(MediaError::Altered));

    for size in [999, 1001] {
        let wrong = MediaRef {
            size,
            ..media.clone()
        };
        assert_eq!(decrypt_media(&wrong, &blob), Err(MediaError::Altered));
    }

    // A larger file than announced stops as soon as it shows.
    let (big, big_media) = encrypt_media(&file(3 * CHUNK)).unwrap();
    let small = MediaRef {
        size: 10,
        ..reference(big_media)
    };
    let mut decryptor = MediaDecryptor::new(&small);
    assert_eq!(decryptor.update(&big), Err(MediaError::Altered));
}

#[test]
fn the_reference_survives_the_message() {
    use crate::messages::{Body, Message};
    let (blob, media) = encrypt_media(b"a photo").unwrap();
    let message = Message::new(
        1,
        Body::Media {
            media: reference(media),
            caption: String::new(),
            reply_to: None,
        },
    )
    .unwrap();
    let Body::Media { media, .. } = Message::decode(&message.encode().unwrap()).unwrap().body
    else {
        panic!("expected a media");
    };
    assert_eq!(decrypt_media(&media, &blob).unwrap(), b"a photo");
}

#[test]
fn no_key_appears_in_debug() {
    let encryptor = MediaEncryptor::new().unwrap();
    let key = format!("{:?}", encryptor.key.as_bytes());
    assert!(!format!("{encryptor:?}").contains(&key));
    let (_, media) = encryptor.finish().unwrap();
    assert!(format!("{media:?}").contains("MediaKey(..)"));
}

#[test]
fn a_sealed_media_secret_opens_only_in_the_seal_that_made_it() {
    let (blob, media) = encrypt_media(&file(1000)).unwrap();
    let seal = MediaSeal::new().unwrap();
    let sealed = seal.seal(&media).unwrap();
    // Neither the key nor the digest shows through.
    let reference = reference(seal.open(&sealed).unwrap());
    assert!(!sealed.windows(32).any(|w| w == reference.digest));
    assert_eq!(decrypt_media(&reference, &blob).unwrap(), file(1000));
    // Sealing again gives other bytes for the same secret.
    let again = seal.seal(&EncryptedMedia::of(&reference)).unwrap();
    assert_ne!(again, sealed);
    assert_eq!(
        seal.open(&again).unwrap().digest(),
        reference.digest.as_ref()
    );

    let other = MediaSeal::new().unwrap();
    assert_eq!(other.open(&sealed).err(), Some(MediaError::Altered));
    let mut altered = sealed.clone();
    altered[30] ^= 1;
    assert_eq!(seal.open(&altered).err(), Some(MediaError::Altered));
    assert_eq!(seal.open(&sealed[..10]).err(), Some(MediaError::Altered));
}
