#!/usr/bin/env bash
# Vérifie sur un appareil ou un émulateur Android que l'app appelle bien le cœur Rust :
# construit tessel-ffi, l'APK release (JS embarqué, sans Metro), l'installe, la lance,
# et lit l'écran (uiautomator) jusqu'à y trouver « tessel-core <version du crate> ».
#
#   scripts/check-android.sh [numéro de série adb]
#
# Gradle tourne sans démon et dans un environnement nettoyé : le shell de la machine
# hérite de variables CARGO_* / TAURI_* d'autres projets.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SERIAL="${1:-$(adb devices | awk 'NR>1 && $2=="device" {print $1; exit}')}"
[ -n "$SERIAL" ] || { echo "aucun appareil Android (adb devices)" >&2; exit 2; }
ADB=(adb -s "$SERIAL")
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/crates/tessel-core/Cargo.toml" | head -1)"
APK="$ROOT/apps/mobile/android/app/build/outputs/apk/release/app-release.apk"

"$ROOT/scripts/build-android-core.sh"
(cd "$ROOT/apps/mobile" && CI=1 npx expo prebuild --platform android --no-install >/dev/null)
(cd "$ROOT/apps/mobile/android" && env -i HOME="$HOME" PATH="$PATH" JAVA_HOME="${JAVA_HOME:-}" \
  ANDROID_HOME="${ANDROID_HOME:-$HOME/Android/Sdk}" LANG="${LANG:-C.UTF-8}" TMPDIR="${TMPDIR:-/tmp}" \
  ./gradlew --no-daemon :app:assembleRelease --console=plain -q)

"${ADB[@]}" install -r "$APK" >/dev/null
"${ADB[@]}" shell am start -W -n app.tessel.mobile/.MainActivity >/dev/null
for _ in $(seq 1 30); do
  "${ADB[@]}" shell uiautomator dump /sdcard/tessel-ui.xml >/dev/null 2>&1 || true
  if "${ADB[@]}" exec-out cat /sdcard/tessel-ui.xml 2>/dev/null | grep -q "text=\"tessel-core $VERSION\""; then
    "${ADB[@]}" shell rm -f /sdcard/tessel-ui.xml
    echo "ok : l'app affiche tessel-core $VERSION sur $SERIAL"
    exit 0
  fi
  sleep 1
done
"${ADB[@]}" shell rm -f /sdcard/tessel-ui.xml
echo "échec : « tessel-core $VERSION » absent de l'écran sur $SERIAL" >&2
"${ADB[@]}" logcat -d -t 200 | grep -i -E "tessel|jna|UnsatisfiedLink|FATAL" >&2 || true
exit 1
