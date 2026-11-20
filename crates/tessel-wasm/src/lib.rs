//! WASM bindings of `tessel-core` for the web client.
//!
//! A thin layer: every exported function calls the core and decides nothing. No
//! private key crosses this boundary; the only secret that does is the store key, which
//! only the browser's key store can protect at rest (story 02-06).

use wasm_bindgen::prelude::{JsError, wasm_bindgen};

/// Core version.
#[wasm_bindgen]
pub fn version() -> String {
    tessel_core::version().to_owned()
}

/// IANA name of Tessel's MLS suite.
#[wasm_bindgen]
pub fn ciphersuite() -> String {
    tessel_core::CIPHERSUITE_NAME.to_owned()
}

/// Checks that the core can work in this browser (suite supported, randomness
/// available). Throws an error otherwise.
#[wasm_bindgen(js_name = selfCheck)]
pub fn self_check() -> Result<(), JsError> {
    tessel_core::self_check().map_err(|e| JsError::new(&e.to_string()))
}

/// Draws a new key for the local database (story 02-06). It is the one secret that
/// crosses this boundary: the page seals it at once with a non-extractable WebCrypto key
/// (`loadStoreKey`) and only hands it back to the core at startup.
#[wasm_bindgen(js_name = generateStoreKey)]
pub fn generate_store_key() -> Result<Vec<u8>, JsError> {
    let key = tessel_core::StoreKey::generate().map_err(|e| JsError::new(&e.to_string()))?;
    Ok(key.to_bytes().to_vec())
}
