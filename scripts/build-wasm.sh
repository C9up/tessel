#!/usr/bin/env bash
# Construit tessel-wasm et écrit ses liaisons JS dans packages/core-web/generated.
# Le CLI wasm-bindgen doit avoir la version du crate épinglée dans crates/tessel-wasm.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROFILE="${1:-release}"
BINDGEN="${WASM_BINDGEN:-$(command -v wasm-bindgen || echo "${CARGO_HOME:-$HOME/.cargo}/bin/wasm-bindgen")}"
[ -x "$BINDGEN" ] || { echo "wasm-bindgen introuvable (cargo install wasm-bindgen-cli --version 0.2.129)" >&2; exit 2; }

cd "$ROOT"
TARGET_DIR="$(scripts/cargo.sh metadata --format-version 1 --no-deps | python3 -I -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
scripts/cargo.sh build -p tessel-wasm --target wasm32-unknown-unknown --profile "$PROFILE"
DIR="$PROFILE"; [ "$PROFILE" = dev ] && DIR=debug
"$BINDGEN" --target web --out-dir packages/core-web/generated "$TARGET_DIR/wasm32-unknown-unknown/$DIR/tessel_wasm.wasm"
