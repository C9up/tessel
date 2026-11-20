---
type: Story
title: "Squelette de tessel-core et chaîne de compilation"
description: "Le crate Rust existe et une fonction triviale est appelée depuis le web (WASM) et depuis Expo (natif)."
tags: [product]
status: draft
state: review
parent: epics/01-fondations-du-depot.md
depends_on: [stories/01-01-depot-git-et-monorepo.md]
priority: high
created: 2026-11-20
---

# Squelette de tessel-core et chaîne de compilation

Le crate Rust existe et une fonction triviale est appelée depuis le web (WASM) et depuis Expo (natif).

## Critères d'acceptation

- `crates/tessel-core` compile en natif, en `wasm32-unknown-unknown` et pour Android (`aarch64`, `x86_64`).
- Une fonction `version()` est appelée depuis `apps/web` (WASM) et depuis `apps/mobile` (module natif), testée.
- `cargo clippy -D warnings`, `cargo deny` et `cargo audit` passent.

## Reviews

```workstream
reviews
```
