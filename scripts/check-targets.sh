#!/usr/bin/env bash
# Checks that the core compiles for Tessel's targets: WASM (web) and Android
# (mobile), on top of native, which `cargo test` already covers (ADR-001, ADR-003).
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
scripts/cargo.sh check -p tessel-wasm --target wasm32-unknown-unknown
for target in aarch64-linux-android x86_64-linux-android; do
  scripts/cargo.sh check -p tessel-core --target "$target"
done
