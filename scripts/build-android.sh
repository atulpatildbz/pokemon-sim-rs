#!/usr/bin/env bash
# Build the Android APK (arm64) and install it on a connected device.
# Needs: Android SDK (platform android-35, build-tools), NDK, `rustup target add
# aarch64-linux-android`, `cargo install cargo-ndk`. The game files are compiled in
# (feature `bundled`, from $POKEGAME_DIR or ../downloads/pokegame/extracted).
# Usage: [PG_RELEASE=1] scripts/build-android.sh [--no-install]
set -euo pipefail
cd "$(dirname "$0")/.."

SDK="${ANDROID_HOME:-/opt/homebrew/share/android-commandlinetools}"
export ANDROID_HOME="$SDK"
export ANDROID_NDK_HOME="${ANDROID_NDK_HOME:-$(ls -d "$SDK"/ndk/* | sort -V | tail -1)}"
BT="$(ls -d "$SDK"/build-tools/* | sort -V | tail -1)"
PLATFORM="$(ls -d "$SDK"/platforms/android-* | sort -V | tail -1)/android.jar"
OUT=target/android
APK="$OUT/pokegame.apk"

cargo ndk -t arm64-v8a -P 26 build --release --lib

rm -rf "$OUT" && mkdir -p "$OUT/flat" "$OUT/stage/lib/arm64-v8a"
"$BT/aapt2" compile --dir android/res -o "$OUT/flat"
# Version from Cargo.toml: versionName "1.2.3", versionCode 10203. It must grow with every release,
# or Android refuses to install the update (and updates must keep the same signing key, which is
# what keeps players' saves).
VERSION=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
IFS=. read -r MAJ MIN PAT <<< "$VERSION"
CODE=$((MAJ * 10000 + MIN * 100 + PAT))
"$BT/aapt2" link -o "$OUT/unsigned.apk" -I "$PLATFORM" --manifest android/AndroidManifest.xml \
    --version-code "$CODE" --version-name "$VERSION" --replace-version "$OUT"/flat/*.flat
cp target/aarch64-linux-android/release/libpokegame.so "$OUT/stage/lib/arm64-v8a/"
(cd "$OUT/stage" && zip -q -r ../unsigned.apk lib)
"$BT/zipalign" -f -p 4 "$OUT/unsigned.apk" "$OUT/aligned.apk"
# PG_RELEASE=1: sign with the release key (~/.android/pokemon-sim-release.{jks,pass}, kept out
# of the repo; Android only accepts updates signed with the same key). Otherwise the debug key.
if [[ "${PG_RELEASE:-}" == 1 ]]; then
    PASS="$(tr -d '\n' < "$HOME/.android/pokemon-sim-release.pass")"
    "$BT/apksigner" sign --ks "$HOME/.android/pokemon-sim-release.jks" --ks-key-alias pokemon-sim \
        --ks-pass "pass:$PASS" --key-pass "pass:$PASS" --out "$APK" "$OUT/aligned.apk"
else
    "$BT/apksigner" sign --ks "$HOME/.android/debug.keystore" --ks-pass pass:android --key-pass pass:android \
        --ks-key-alias androiddebugkey --out "$APK" "$OUT/aligned.apk"
fi
echo "Built $APK ($(du -h "$APK" | cut -f1))"

if [[ "${1:-}" != "--no-install" ]] && adb get-state >/dev/null 2>&1; then
    adb install -r "$APK"
    adb shell am start -n com.atulpatildbz.pokegame/android.app.NativeActivity >/dev/null
    echo "Installed and started on $(adb shell getprop ro.product.model | tr -d '\r')"
fi
