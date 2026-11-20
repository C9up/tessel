//! Cœur client de Tessel.
//!
//! Tout ce qui touche aux clés et au chiffrement vit ici, une seule fois, pour le
//! web (WASM) comme pour le mobile (natif). Aucune clé privée ne sort de ce crate.

mod suite;

pub use suite::{CIPHERSUITE_ID, CIPHERSUITE_NAME, CoreError, self_check};

/// Version du cœur, telle que publiée par Cargo.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_est_celle_du_crate() {
        assert_eq!(version(), env!("CARGO_PKG_VERSION"));
    }
}
