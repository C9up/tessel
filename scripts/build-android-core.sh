#!/usr/bin/env bash
# Construit tessel-ffi pour Android (cargo-ndk) et génère ses liaisons Kotlin (uniffi),
# dans le module Expo local apps/mobile/modules/tessel-core.
#
#   scripts/build-android-core.sh [release|dev]
#
# Prérequis : rustup (rust-toolchain.toml), `cargo install cargo-ndk`, le NDK Android
# (ANDROID_NDK_HOME, ou le plus récent de $ANDROID_HOME/ndk).
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROFILE="${1:-release}"
MODULE="$ROOT/apps/mobile/modules/tessel-core/android/src/main"
# arm64 pour les téléphones, x86_64 pour l'émulateur.
ABIS=(-t arm64-v8a -t x86_64)
# L'API minimale d'Android que l'app vise.
API=24

if [ -z "${ANDROID_NDK_HOME:-}" ]; then
  SDK="${ANDROID_HOME:-$HOME/Android/Sdk}"
  ANDROID_NDK_HOME="$(ls -d "$SDK"/ndk/* 2>/dev/null | sort -V | tail -1)"
  export ANDROID_NDK_HOME
fi
[ -d "$ANDROID_NDK_HOME" ] || { echo "NDK Android introuvable (ANDROID_NDK_HOME)" >&2; exit 2; }

cd "$ROOT"
TARGET_DIR="$(scripts/cargo.sh metadata --format-version 1 --no-deps | python3 -I -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
scripts/cargo.sh ndk "${ABIS[@]}" -P "$API" -o "$MODULE/jniLibs" build -p tessel-ffi --profile "$PROFILE"
DIR="$PROFILE"; [ "$PROFILE" = dev ] && DIR=debug
scripts/cargo.sh run -q -p uniffi-bindgen -- generate \
  --library "$TARGET_DIR/aarch64-linux-android/$DIR/libtessel_ffi.so" \
  --language kotlin --no-format --out-dir "$MODULE/java"
echo "libtessel_ffi.so et liaisons Kotlin écrites dans $MODULE"
