//! Liaisons WASM de `tessel-core` pour le client web.
//!
//! Une couche mince : chaque fonction exportée appelle le cœur et ne décide rien.
//! Aucune clé privée ne traverse cette frontière.

use wasm_bindgen::prelude::{JsError, wasm_bindgen};

/// Version du cœur.
#[wasm_bindgen]
pub fn version() -> String {
    tessel_core::version().to_owned()
}

/// Nom IANA de la suite MLS de Tessel.
#[wasm_bindgen]
pub fn ciphersuite() -> String {
    tessel_core::CIPHERSUITE_NAME.to_owned()
}

/// Vérifie que le cœur peut travailler dans ce navigateur (suite prise en charge,
/// aléa disponible). Lève une erreur sinon.
#[wasm_bindgen(js_name = selfCheck)]
pub fn self_check() -> Result<(), JsError> {
    tessel_core::self_check().map_err(|e| JsError::new(&e.to_string()))
}
