#!/usr/bin/env bash
# Builds tessel-wasm and writes its JS bindings into packages/core-web/generated.
# The wasm-bindgen CLI must match the crate version pinned in crates/tessel-wasm.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROFILE="${1:-release}"
BINDGEN="${WASM_BINDGEN:-$(command -v wasm-bindgen || echo "${CARGO_HOME:-$HOME/.cargo}/bin/wasm-bindgen")}"
[ -x "$BINDGEN" ] || { echo "wasm-bindgen not found (cargo install wasm-bindgen-cli --version 0.2.129)" >&2; exit 2; }

cd "$ROOT"
TARGET_DIR="$(scripts/cargo.sh metadata --format-version 1 --no-deps | python3 -I -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
scripts/cargo.sh build -p tessel-wasm --target wasm32-unknown-unknown --profile "$PROFILE"
DIR="$PROFILE"; [ "$PROFILE" = dev ] && DIR=debug
"$BINDGEN" --target web --out-dir packages/core-web/generated "$TARGET_DIR/wasm32-unknown-unknown/$DIR/tessel_wasm.wasm"
