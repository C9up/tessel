//! Liaisons WASM de `tessel-core` pour le client web.
//!
//! Une couche mince : chaque fonction exportée appelle le cœur et ne décide rien.
//! Aucune clé privée ne traverse cette frontière.

use wasm_bindgen::prelude::wasm_bindgen;

/// Version du cœur.
#[wasm_bindgen]
pub fn version() -> String {
    tessel_core::version().to_owned()
}
