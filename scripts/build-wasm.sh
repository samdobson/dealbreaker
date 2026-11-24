#!/usr/bin/env bash
# Build the WebAssembly package into pkg/<target> (default target: bundler; also web, nodejs).
# Needs: rustup target add wasm32-unknown-unknown; cargo install wasm-bindgen-cli --version 0.2.130
set -euo pipefail
cd "$(dirname "$0")/.."
TARGET="${1:-bundler}"
CARGO_PROFILE_RELEASE_DEBUG=0 CARGO_PROFILE_RELEASE_STRIP=true \
  cargo build --release -p dealbreaker-wasm --target wasm32-unknown-unknown
wasm-bindgen --target "$TARGET" --out-dir "pkg/$TARGET" --out-name dealbreaker \
  target/wasm32-unknown-unknown/release/dealbreaker_wasm.wasm
echo "built pkg/$TARGET"
