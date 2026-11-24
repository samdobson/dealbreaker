#!/usr/bin/env bash
# Build the npm package in js/ (WASM target: web, used by both browsers/bundlers and Node).
# Needs: rustup target add wasm32-unknown-unknown; cargo install wasm-bindgen-cli --version 0.2.130
set -euo pipefail
cd "$(dirname "$0")/.."

CARGO_PROFILE_RELEASE_DEBUG=0 CARGO_PROFILE_RELEASE_STRIP=true \
  cargo build --release -p dealbreaker-wasm --target wasm32-unknown-unknown

rm -rf js/wasm
wasm-bindgen --target web --out-dir js/wasm --out-name dealbreaker \
  target/wasm32-unknown-unknown/release/dealbreaker_wasm.wasm

# Keep only the files needed at runtime; TypeScript types live in js/src/*.d.ts.
rm -f js/wasm/package.json js/wasm/.gitignore js/wasm/dealbreaker.d.ts js/wasm/dealbreaker_bg.wasm.d.ts

cp README.md LICENSE-MIT LICENSE-APACHE js/
echo "built npm package in js/"
