#!/usr/bin/env bash
# Fast local mirror of .github/workflows/ci.yml (used by .git/hooks/pre-push).
set -euo pipefail
cd "$(dirname "$0")/.."

echo "[check-ci] rustfmt..."
cargo fmt --all --check

echo "[check-ci] clippy (workspace)..."
cargo clippy --workspace --all-targets -- -D warnings

echo "[check-ci] test --lib --no-default-features..."
cargo test -p dealbreaker --lib --no-default-features

echo "[check-ci] test --lib --features schema..."
cargo test -p dealbreaker --lib --features schema

echo "[check-ci] rustdoc (-D warnings)..."
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps

if rustup target list --installed | grep -qx wasm32-unknown-unknown; then
  echo "[check-ci] clippy (wasm32)..."
  cargo clippy -p dealbreaker-wasm --target wasm32-unknown-unknown -- -D warnings
fi

echo "[check-ci] version parity (Cargo.toml vs js/package.json)..."
CARGO_VERSION="$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n1)"
JS_VERSION="$(node -p 'require("./js/package.json").version')"
if [ "$CARGO_VERSION" != "$JS_VERSION" ]; then
  echo "error: Cargo.toml ($CARGO_VERSION) != js/package.json ($JS_VERSION)" >&2
  exit 1
fi

if command -v tsc >/dev/null 2>&1; then
  echo "[check-ci] TypeScript declarations (tsc)..."
  tsc --noEmit --strict --module nodenext --moduleResolution nodenext --target es2022 \
    js/src/index.d.ts js/src/worker.d.ts
elif command -v npx >/dev/null 2>&1; then
  echo "[check-ci] TypeScript declarations (npx tsc)..."
  npx --yes -p typescript tsc --noEmit --strict --module nodenext --moduleResolution nodenext --target es2022 \
    js/src/index.d.ts js/src/worker.d.ts
fi

echo "[check-ci] all checks passed!"
