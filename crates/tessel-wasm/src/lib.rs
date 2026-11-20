//! WASM bindings of `tessel-core` for the web client.
//!
//! A thin layer: every exported function calls the core and decides nothing. No
//! private key crosses this boundary.

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
