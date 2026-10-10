#!/usr/bin/env bash
# Build the release binaries into dist/:
#   macOS   universal .app (Apple Silicon + Intel), zipped
#   Windows x86_64 .exe, zipped (cross-compiled with mingw-w64)
#   Android arm64 .apk signed with the release key
# Each build has the game files compiled in: personal use only, never publish these.
set -euo pipefail
cd "$(dirname "$0")/.."
VERSION=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
DIST=dist
rm -rf "$DIST" && mkdir -p "$DIST"
NAME="POKéMON Simulator"

GEN_ICONS=1 cargo test --release --test icons -q

# ---- macOS
for t in aarch64-apple-darwin x86_64-apple-darwin; do cargo build --release --bin pokegame --target $t; done
APP="$DIST/$NAME.app"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
lipo -create -output "$APP/Contents/MacOS/pokegame" \
    target/aarch64-apple-darwin/release/pokegame target/x86_64-apple-darwin/release/pokegame
iconutil -c icns packaging/icon.iconset -o "$APP/Contents/Resources/AppIcon.icns"
sed "s/@VERSION@/$VERSION/g" packaging/Info.plist > "$APP/Contents/Info.plist"
codesign --force --deep --sign - "$APP"
(cd "$DIST" && ditto -c -k --keepParent "$NAME.app" "PokemonSimulator-$VERSION-macos-universal.zip")
rm -rf "$APP"

# ---- Windows
cargo build --release --bin pokegame --target x86_64-pc-windows-gnu
mkdir -p "$DIST/win"
cp target/x86_64-pc-windows-gnu/release/pokegame.exe "$DIST/win/PokemonSimulator.exe"
(cd "$DIST/win" && zip -q "../PokemonSimulator-$VERSION-windows-x64.zip" PokemonSimulator.exe)
rm -rf "$DIST/win"

# ---- Android
PG_RELEASE=1 scripts/build-android.sh --no-install
cp target/android/pokegame.apk "$DIST/PokemonSimulator-$VERSION-android-arm64.apk"

(cd "$DIST" && for f in *; do shasum -a 256 "$f" > "$f.sha256"; done)
ls -la "$DIST"
