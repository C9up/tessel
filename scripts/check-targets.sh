#!/usr/bin/env bash
# Vérifie que le cœur compile sur les cibles de Tessel : WASM (web) et Android
# (mobile), en plus du natif que `cargo test` couvre déjà (ADR-001, ADR-003).
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
scripts/cargo.sh check -p tessel-wasm --target wasm32-unknown-unknown
for target in aarch64-linux-android x86_64-linux-android; do
  scripts/cargo.sh check -p tessel-core --target "$target"
done
