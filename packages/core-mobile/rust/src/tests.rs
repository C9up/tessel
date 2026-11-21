use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;

use super::*;

/// A platform seal for the tests: a marker and the bytes flipped, as recognizable as a
/// Keystore's output is unreadable. `broken` refuses to unseal, as a wiped Keystore.
fn seal(broken: bool) -> Seal {
    Box::new(move |unseal, input| {
        if unseal {
            if broken || input.first() != Some(&0xEE) {
                return None;
            }
            Some(input[1..].iter().map(|b| b ^ 0x5A).collect())
        } else {
            let mut out = vec![0xEE];
            out.extend(input.iter().map(|b| b ^ 0x5A));
            Some(out)
        }
    })
}

/// A fresh directory for one test's database.
fn directory() -> PathBuf {
    let name: String = tessel_api::generate_store_key()
        .unwrap()
        .iter()
        .take(8)
        .map(|b| format!("{b:02x}"))
        .collect();
    let dir = std::env::temp_dir().join(format!("tessel-mobile-{name}"));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[derive(Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
struct Device {
    account: String,
    device_id: ByteBuf,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Outbound {
    send_id: ByteBuf,
    encrypted: ByteBuf,
}

#[derive(Serialize)]
struct Text<'a> {
    #[serde(rename = "type")]
    kind: &'a str,
    text: &'a str,
}

fn call<T: serde::de::DeserializeOwned>(
    host: &mut Host,
    method: &str,
    input: impl Serialize,
) -> Result<T, ApiError> {
    let answer = host.call(method, &rmp_serde::to_vec_named(&input).unwrap())?;
    Ok(rmp_serde::from_slice(&answer).unwrap())
}

#[test]
fn a_device_lives_in_its_sealed_database_and_comes_back_after_a_restart() {
    let dir = directory();
    let mut host = Host::new(&dir, seal(false));
    let none: Option<Device> = call(&mut host, "start", ()).unwrap();
    assert_eq!(none, None);
    let device: Device = call(&mut host, "createAccount", ("alice@tessel.example",)).unwrap();
    assert_eq!(device.account, "alice@tessel.example");

    // A group, a message queued and encrypted, its send acknowledged.
    let group: ByteBuf = call(&mut host, "createGroup", ()).unwrap();
    let _: serde::de::IgnoredAny = call(
        &mut host,
        "queueMessage",
        (
            &group,
            Text {
                kind: "text",
                text: "hello from the phone",
            },
            1_700_000_000_000_u64,
        ),
    )
    .unwrap();
    let outbound: Option<Outbound> = call(&mut host, "nextOutbound", (&group,)).unwrap();
    let outbound = outbound.unwrap();
    assert!(!outbound.encrypted.is_empty());
    let (): () = call(&mut host, "outboundSent", (&group, &outbound.send_id)).unwrap();
    let conversations: Vec<serde::de::IgnoredAny> = call(&mut host, "conversations", ()).unwrap();
    assert_eq!(conversations.len(), 1);

    // The database holds the store key sealed, never in clear.
    let (): () = call(&mut host, "close", ()).unwrap();
    let store = SqliteStore::open(&dir.join(DATABASE)).unwrap();
    let sealed = store.host_value(SEALED_KEY).unwrap().unwrap();
    assert_eq!(sealed[0], 0xEE);
    drop(store);

    // A restart: the same device, its group still there.
    let mut again = Host::new(&dir, seal(false));
    let reopened: Option<Device> = call(&mut again, "start", ()).unwrap();
    assert_eq!(reopened, Some(device));
    let groups: Vec<ByteBuf> = call(&mut again, "groups", ()).unwrap();
    assert_eq!(groups, vec![group]);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_store_key_the_platform_cannot_unseal_opens_nothing() {
    let dir = directory();
    let mut host = Host::new(&dir, seal(false));
    let _: Option<Device> = call(&mut host, "start", ()).unwrap();
    let _: Device = call(&mut host, "createAccount", ("alice@tessel.example",)).unwrap();
    let (): () = call(&mut host, "close", ()).unwrap();

    // Another phone, or a wiped Keystore.
    let mut elsewhere = Host::new(&dir, seal(true));
    let refused = call::<Option<Device>>(&mut elsewhere, "start", ()).unwrap_err();
    assert_eq!(refused.code, "host:keystore");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn calls_out_of_order_or_unknown_are_refused_with_a_code() {
    let dir = directory();
    let mut host = Host::new(&dir, seal(false));
    assert_eq!(
        call::<ByteBuf>(&mut host, "createGroup", ())
            .unwrap_err()
            .code,
        "host:not-started"
    );
    let _: Option<Device> = call(&mut host, "start", ()).unwrap();
    assert_eq!(
        call::<ByteBuf>(&mut host, "nothing", ()).unwrap_err().code,
        "host:not-started"
    );
    let _: Device = call(&mut host, "createAccount", ("alice@tessel.example",)).unwrap();
    assert_eq!(
        call::<ByteBuf>(&mut host, "nothing", ()).unwrap_err().code,
        "host:unknown-method"
    );
    assert_eq!(
        call::<Device>(&mut host, "createAccount", ("bob@tessel.example",))
            .unwrap_err()
            .code,
        "host:account-exists"
    );
    assert_eq!(
        call::<Device>(&mut host, "grantLink", (42,))
            .unwrap_err()
            .code,
        "input:malformed"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

/// The C ABI's seal, for the round trip below: the same flip as [`seal`].
#[allow(unsafe_code)]
unsafe extern "C" fn c_seal(
    unseal: i32,
    input: *const u8,
    len: usize,
    out: *mut u8,
    out_cap: usize,
) -> i64 {
    // SAFETY: the crate passes `len` readable bytes and `out_cap` writable ones.
    let (input, out) = unsafe {
        (
            std::slice::from_raw_parts(input, len),
            std::slice::from_raw_parts_mut(out, out_cap),
        )
    };
    let Some(result) = seal(false)(unseal != 0, input) else {
        return -1;
    };
    if result.len() > out.len() {
        return -1;
    }
    out[..result.len()].copy_from_slice(&result);
    i64::try_from(result.len()).unwrap_or(-1)
}

#[test]
#[allow(unsafe_code)]
fn the_c_abi_answers_in_messagepack_and_refuses_as_code_newline_message() {
    let dir = directory();
    let path = dir.to_str().unwrap().as_bytes();
    // SAFETY: valid pointers and lengths, a seal that lives for the test.
    unsafe {
        assert_eq!(ffi::tessel_configure(path.as_ptr(), path.len(), c_seal), 0);
        let call = |method: &str, input: &[u8]| {
            let mut out = ffi::TesselBuf {
                ptr: std::ptr::null_mut(),
                len: 0,
                cap: 0,
            };
            let code = ffi::tessel_call(
                method.as_ptr(),
                method.len(),
                input.as_ptr(),
                input.len(),
                &mut out,
            );
            let bytes = std::slice::from_raw_parts(out.ptr, out.len).to_vec();
            ffi::tessel_buf_free(out);
            (code, bytes)
        };
        let empty = rmp_serde::to_vec(&()).unwrap();
        let (code, bytes) = call("version", &empty);
        assert_eq!(code, 0);
        let version: String = rmp_serde::from_slice(&bytes).unwrap();
        assert_eq!(version, tessel_api::version());
        let (code, bytes) = call("createGroup", &empty);
        assert_eq!(code, 1);
        assert!(
            String::from_utf8(bytes)
                .unwrap()
                .starts_with("host:not-started\n")
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
}
