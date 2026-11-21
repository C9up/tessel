//! The official MLS test vectors (story 02-12), through public APIs only: the crypto
//! provider Tessel uses, on Tessel's suite, gives RFC 9420's results; OpenMLS decodes the
//! official messages and encodes them back byte for byte. See `tests/vectors/README.md`.

use openmls::prelude::MlsMessageIn;
use openmls_rust_crypto::RustCrypto;
use openmls_traits::crypto::OpenMlsCrypto;
use openmls_traits::types::{Ciphersuite, HpkeCiphertext};
use serde_json::Value;
use tls_codec::{DeserializeBytes as _, Serialize as _, VLBytes};

const CRYPTO_BASICS: &str = include_str!("vectors/crypto-basics.json");
const MESSAGES: &str = include_str!("vectors/messages.json");

fn hex(value: &Value) -> Vec<u8> {
    let text = value.as_str().expect("a hex string");
    assert!(text.len().is_multiple_of(2), "odd hex length");
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).expect("hex digits"))
        .collect()
}

fn text(value: &Value) -> &str {
    value.as_str().expect("a string")
}

fn number(value: &Value) -> u64 {
    value.as_u64().expect("a number")
}

/// `opaque<V>`: a QUIC variable-length length, then the bytes (RFC 9420, §2.1.2).
fn opaque(bytes: &[u8]) -> Vec<u8> {
    VLBytes::new(bytes.to_vec())
        .tls_serialize_detached()
        .expect("encodable")
}

/// `"MLS 1.0 " + label`, as an `opaque<V>`.
fn mls_label(label: &str) -> Vec<u8> {
    opaque(format!("MLS 1.0 {label}").as_bytes())
}

/// `ExpandWithLabel` (RFC 9420, §5.1).
fn expand_with_label(
    crypto: &RustCrypto,
    suite: Ciphersuite,
    secret: &[u8],
    label: &str,
    context: &[u8],
    length: u16,
) -> Vec<u8> {
    let mut info = length.to_be_bytes().to_vec();
    info.extend(mls_label(label));
    info.extend(opaque(context));
    crypto
        .hkdf_expand(suite.hash_algorithm(), secret, &info, usize::from(length))
        .expect("expand")
        .as_slice()
        .to_vec()
}

#[test]
fn the_crypto_basics_of_tessels_suite_match_the_official_vectors() {
    let vectors: Value = serde_json::from_str(CRYPTO_BASICS).expect("json");
    let [vector] = vectors.as_array().expect("an array").as_slice() else {
        panic!("one vector, for ciphersuite 1");
    };
    assert_eq!(
        number(&vector["cipher_suite"]),
        u64::from(tessel_core::CIPHERSUITE_ID)
    );
    let suite = Ciphersuite::try_from(tessel_core::CIPHERSUITE_ID).expect("a known suite");
    let crypto = RustCrypto::default();

    // RefHash: Hash(RefHashInput { label<V>, value<V> }), the label as given.
    let v = &vector["ref_hash"];
    let mut input = opaque(text(&v["label"]).as_bytes());
    input.extend(opaque(&hex(&v["value"])));
    assert_eq!(
        crypto.hash(suite.hash_algorithm(), &input).expect("hash"),
        hex(&v["out"])
    );

    let v = &vector["expand_with_label"];
    let length = u16::try_from(number(&v["length"])).expect("u16");
    assert_eq!(
        expand_with_label(
            &crypto,
            suite,
            &hex(&v["secret"]),
            text(&v["label"]),
            &hex(&v["context"]),
            length
        ),
        hex(&v["out"])
    );

    // DeriveSecret: ExpandWithLabel(secret, label, "", Nh).
    let v = &vector["derive_secret"];
    let nh = u16::try_from(suite.hash_length()).expect("u16");
    assert_eq!(
        expand_with_label(
            &crypto,
            suite,
            &hex(&v["secret"]),
            text(&v["label"]),
            &[],
            nh
        ),
        hex(&v["out"])
    );

    // DeriveTreeSecret: ExpandWithLabel(secret, label, generation as uint32, length).
    let v = &vector["derive_tree_secret"];
    let generation = u32::try_from(number(&v["generation"])).expect("u32");
    let length = u16::try_from(number(&v["length"])).expect("u16");
    assert_eq!(
        expand_with_label(
            &crypto,
            suite,
            &hex(&v["secret"]),
            text(&v["label"]),
            &generation.to_be_bytes(),
            length
        ),
        hex(&v["out"])
    );

    // SignWithLabel: Ed25519 is deterministic, so signing gives the vector's signature,
    // and the vector's signature verifies.
    let v = &vector["sign_with_label"];
    let mut content = mls_label(text(&v["label"]));
    content.extend(opaque(&hex(&v["content"])));
    let scheme = suite.signature_algorithm();
    assert_eq!(
        crypto
            .sign(scheme, &content, &hex(&v["priv"]))
            .expect("sign"),
        hex(&v["signature"])
    );
    crypto
        .verify_signature(scheme, &content, &hex(&v["pub"]), &hex(&v["signature"]))
        .expect("the official signature verifies");

    // EncryptWithLabel: HPKE base mode, info = EncryptContext { label<V>, context<V> }.
    let v = &vector["encrypt_with_label"];
    let mut info = mls_label(text(&v["label"]));
    info.extend(opaque(&hex(&v["context"])));
    let sealed = HpkeCiphertext {
        kem_output: hex(&v["kem_output"]).into(),
        ciphertext: hex(&v["ciphertext"]).into(),
    };
    assert_eq!(
        crypto
            .hpke_open(suite.hpke_config(), &sealed, &hex(&v["priv"]), &info, &[])
            .expect("open"),
        hex(&v["plaintext"])
    );
}

#[test]
fn the_official_mls_messages_decode_and_encode_back_unchanged() {
    let vectors: Value = serde_json::from_str(MESSAGES).expect("json");
    let vectors = vectors.as_array().expect("an array");
    assert!(!vectors.is_empty());
    let fields = [
        "mls_welcome",
        "mls_group_info",
        "mls_key_package",
        "public_message_application",
        "public_message_proposal",
        "public_message_commit",
        "private_message",
    ];
    let mut checked = 0;
    for (index, vector) in vectors.iter().enumerate() {
        for field in fields {
            let bytes = hex(&vector[field]);
            let (message, rest) = MlsMessageIn::tls_deserialize_bytes(&bytes)
                .unwrap_or_else(|e| panic!("vector {index}, {field}: {e:?}"));
            assert!(rest.is_empty(), "vector {index}, {field}: trailing bytes");
            assert_eq!(
                message.tls_serialize_detached().expect("encodable"),
                bytes,
                "vector {index}, {field}: encoded differently"
            );
            checked += 1;
        }
    }
    assert_eq!(checked, vectors.len() * fields.len());
}
