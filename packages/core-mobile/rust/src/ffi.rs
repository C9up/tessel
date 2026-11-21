//! The C ABI (story 02-13), the shape of orbit-host's: what the JSI module
//! (`packages/core-mobile/android/src/main/cpp`) calls. Every function is synchronous;
//! a buffer handed out is freed with [`tessel_buf_free`]. A panic never crosses: it
//! comes back as a `host:panic` refusal.

// The one place the crate touches raw pointers: the boundary with C++.
#![allow(unsafe_code)]

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Mutex;

use tessel_api::ApiError;

use crate::Host;

/// A buffer the crate allocated, for C++ to read, then free with [`tessel_buf_free`].
#[repr(C)]
pub struct TesselBuf {
    pub ptr: *mut u8,
    pub len: usize,
    pub cap: usize,
}

impl TesselBuf {
    fn from_vec(bytes: Vec<u8>) -> Self {
        let mut bytes = std::mem::ManuallyDrop::new(bytes);
        Self {
            ptr: bytes.as_mut_ptr(),
            len: bytes.len(),
            cap: bytes.capacity(),
        }
    }
}

/// The platform's seal (`unseal == 0`) or unseal (`unseal == 1`): writes into `out`
/// (at most `out_cap` bytes) and returns the length written, or -1.
pub type TesselSealFn = unsafe extern "C" fn(
    unseal: i32,
    input: *const u8,
    len: usize,
    out: *mut u8,
    out_cap: usize,
) -> i64;

/// The one core of the app.
static HOST: Mutex<Option<Host>> = Mutex::new(None);

/// What sealing adds at most: a nonce and a tag (AES-GCM: 12 + 16), with room to spare.
const SEAL_OVERHEAD: usize = 64;

/// Reads `len` bytes at `ptr`; an empty slice for a null pointer.
///
/// # Safety
/// `ptr` is null or points to `len` readable bytes that outlive the call.
unsafe fn slice<'a>(ptr: *const u8, len: usize) -> &'a [u8] {
    if ptr.is_null() || len == 0 {
        &[]
    } else {
        // SAFETY: the caller guarantees `len` readable bytes at `ptr`.
        unsafe { std::slice::from_raw_parts(ptr, len) }
    }
}

/// Where the database lives, and how its key is sealed. Once, before any call; 0 on
/// success.
///
/// # Safety
/// `dir` points to `len` bytes of UTF-8; `seal` stays callable for the life of the app.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tessel_configure(dir: *const u8, len: usize, seal: TesselSealFn) -> i32 {
    // SAFETY: the caller guarantees `len` bytes at `dir`.
    let Ok(dir) = std::str::from_utf8(unsafe { slice(dir, len) }) else {
        return 1;
    };
    let platform = move |unseal: bool, input: &[u8]| -> Option<Vec<u8>> {
        let mut out = vec![0_u8; input.len() + SEAL_OVERHEAD];
        // SAFETY: `input` and `out` are live for the call, `out` holds `out.len()` bytes.
        let written = unsafe {
            seal(
                i32::from(unseal),
                input.as_ptr(),
                input.len(),
                out.as_mut_ptr(),
                out.len(),
            )
        };
        let written = usize::try_from(written).ok()?;
        out.truncate(written);
        Some(out)
    };
    let Ok(mut host) = HOST.lock() else {
        return 1;
    };
    *host = Some(Host::new(dir, Box::new(platform)));
    0
}

/// Runs `method` with its MessagePack arguments. 0: `out` holds the MessagePack answer;
/// otherwise `out` holds "<code>\n<message>", the TesselError to throw.
///
/// # Safety
/// `method` and `args` point to `method_len` and `args_len` readable bytes; `out` points
/// to a writable `TesselBuf`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tessel_call(
    method: *const u8,
    method_len: usize,
    args: *const u8,
    args_len: usize,
    out: *mut TesselBuf,
) -> i32 {
    // SAFETY: the caller guarantees the lengths.
    let (method, args) = unsafe { (slice(method, method_len), slice(args, args_len)) };
    let result = catch_unwind(AssertUnwindSafe(|| {
        let method =
            std::str::from_utf8(method).map_err(|_| tessel_api::error::malformed("method"))?;
        let mut host = HOST
            .lock()
            .map_err(|_| tessel_api::error::fail("host:poisoned", "an earlier call panicked"))?;
        let host = host.as_mut().ok_or_else(|| {
            tessel_api::error::fail("host:not-configured", "tessel_configure was not called")
        })?;
        host.call(method, args)
    }))
    .unwrap_or_else(|_| Err(tessel_api::error::fail("host:panic", "the core panicked")));
    let (code, bytes) = match result {
        Ok(answer) => (0, answer),
        Err(ApiError { code, message }) => (1, format!("{code}\n{message}").into_bytes()),
    };
    if !out.is_null() {
        // SAFETY: the caller guarantees `out` is writable.
        unsafe { out.write(TesselBuf::from_vec(bytes)) };
    }
    code
}

/// Frees a buffer this crate handed out.
///
/// # Safety
/// `buf` came from this crate, and is freed once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tessel_buf_free(buf: TesselBuf) {
    if !buf.ptr.is_null() {
        // SAFETY: `buf` is a Vec this crate leaked in `TesselBuf::from_vec`.
        drop(unsafe { Vec::from_raw_parts(buf.ptr, buf.len, buf.cap) });
    }
}
