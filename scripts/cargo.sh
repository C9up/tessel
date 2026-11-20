#!/usr/bin/env bash
# Cargo de rustup (qui lit rust-toolchain.toml), même si ~/.cargo/bin n'est pas dans le PATH.
# Sans rustup, le cargo du système, à la version qu'il a.
set -euo pipefail
RUSTUP_CARGO="${CARGO_HOME:-$HOME/.cargo}/bin/cargo"
if [ -x "$RUSTUP_CARGO" ]; then exec "$RUSTUP_CARGO" "$@"; fi
exec cargo "$@"
