//! Liaisons natives de `tessel-core` pour l'app mobile.
//!
//! Une couche mince : chaque fonction exportée appelle le cœur et ne décide rien.
//! Aucune clé privée ne traverse cette frontière.

uniffi::setup_scaffolding!();

/// Version du cœur.
#[uniffi::export]
pub fn version() -> String {
    tessel_core::version().to_owned()
}
