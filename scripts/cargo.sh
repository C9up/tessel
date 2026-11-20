#!/usr/bin/env bash
# rustup's cargo (which reads rust-toolchain.toml), even when ~/.cargo/bin is not in the PATH.
# Without rustup, the system's cargo, at whatever version it has.
set -euo pipefail
RUSTUP_CARGO="${CARGO_HOME:-$HOME/.cargo}/bin/cargo"
if [ -x "$RUSTUP_CARGO" ]; then exec "$RUSTUP_CARGO" "$@"; fi
exec cargo "$@"
