---
type: Leçon
title: "OpenMLS en WASM et avis RustSec"
description: "- **WASM** : la fonctionnalité `js` d'OpenMLS couvre getrandom 0"
tags: [openmls, wasm, cargo-deny]
status: draft
created: 2026-11-20
---

# OpenMLS en WASM et avis RustSec

- **WASM** : la fonctionnalité `js` d'OpenMLS couvre getrandom 0.4, mais `rand_core` 0.6 (RustCrypto, via hpke-rs) tire encore getrandom 0.2, qui refuse de compiler sur `wasm32-unknown-unknown` sans sa propre fonctionnalité `js`. `tessel-core` la déclare (`getrandom_02`) sur cette seule cible.
- **cargo audit ≠ cargo deny** : audit lit tout `Cargo.lock`, y compris les dépendances facultatives jamais compilées (ex. `libcrux-kem` via `hpke-rs-libcrux`) ; deny suit le vrai graphe. Pour savoir si un avis touche vraiment Tessel : `cargo tree -i <crate> --target all`. Si la sortie est vide, le crate n'est pas compilé.
- **Exceptions** : justifiées dans `deny.toml` (et reprises dans `.cargo/audit.toml`), à revoir à chaque montée d'OpenMLS.
- `openmls_libcrux_crypto` 0.3.1 suit `openmls_traits` 0.5 : incompatible avec OpenMLS 0.9.2 (0.6).
