#!/usr/bin/env bash
# Build the web version into dist/web (static files for GitHub Pages or any web server).
# Needs: nightly Rust with rust-src and wasm32-unknown-unknown, and wasm-bindgen-cli at the
# same version as the wasm-bindgen crate in Cargo.lock.
# The game files are never bundled (build.rs refuses for wasm); players pick their own copy.
set -euo pipefail
cd "$(dirname "$0")/.."

# wasm threads: the logic thread runs in a Web Worker sharing the module's memory.
export RUSTFLAGS="-C target-feature=+atomics,+bulk-memory,+mutable-globals \
  -C link-arg=--shared-memory -C link-arg=--max-memory=1073741824 -C link-arg=--import-memory \
  -C link-arg=--export=__wasm_init_tls -C link-arg=--export=__tls_size \
  -C link-arg=--export=__tls_align -C link-arg=--export=__tls_base"
cargo +nightly build --lib --release --target wasm32-unknown-unknown --no-default-features -Z build-std=std,panic_abort

out=dist/web
rm -rf "$out"
mkdir -p "$out"
wasm-bindgen --target web --no-typescript --out-dir "$out/pkg" target/wasm32-unknown-unknown/release/pokegame.wasm
if command -v wasm-opt >/dev/null; then
  wasm-opt -O2 --enable-threads --enable-bulk-memory --enable-mutable-globals "$out/pkg/pokegame_bg.wasm" -o "$out/pkg/pokegame_bg.wasm"
fi
cp web/index.html web/coi.js fonts/PixelOperator-Bold.ttf "$out/"
echo "built $out ($(du -sh "$out" | cut -f1))"
