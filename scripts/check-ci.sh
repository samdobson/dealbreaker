#!/usr/bin/env bash
# Fast local mirror of .github/workflows/ci.yml (used by .git/hooks/pre-push).
set -euo pipefail
cd "$(dirname "$0")/.."

echo "[check-ci] rustfmt..."
cargo fmt --all --check

echo "[check-ci] clippy (workspace)..."
cargo clippy --workspace --all-targets -- -D warnings

echo "[check-ci] clippy (dealbreaker-py)..."
cargo clippy --manifest-path crates/dealbreaker-py/Cargo.toml -- -D warnings

echo "[check-ci] test --lib --no-default-features..."
cargo test -p dealbreaker --lib --no-default-features

echo "[check-ci] test --lib --features schema..."
cargo test -p dealbreaker --lib --features schema

echo "[check-ci] test --lib --features parallel..."
cargo test -p dealbreaker --lib --features parallel

if cargo semver-checks --version >/dev/null 2>&1; then
  echo "[check-ci] cargo semver-checks (dealbreaker)..."
  cargo semver-checks --package dealbreaker
else
  echo "[check-ci] skipping semver check (install with: cargo install cargo-semver-checks --locked)"
fi

echo "[check-ci] rustdoc (-D warnings)..."
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps

if command -v cc >/dev/null 2>&1; then
  echo "[check-ci] C ABI smoke test..."
  cargo build -p dealbreaker-ffi
  cc -Wall -Wextra -Werror \
    -I crates/dealbreaker-ffi/include \
    crates/dealbreaker-ffi/tests/smoke.c \
    -L target/debug -ldealbreaker_ffi \
    -Wl,-rpath,"$PWD/target/debug" \
    -o target/debug/ffi-smoke
  ./target/debug/ffi-smoke
fi

if rustup target list --installed | grep -qx wasm32-unknown-unknown; then
  echo "[check-ci] clippy (wasm32)..."
  cargo clippy -p dealbreaker-wasm --target wasm32-unknown-unknown -- -D warnings
fi

echo "[check-ci] version parity (Cargo.toml vs js/package.json vs pyproject.toml)..."
CARGO_VERSION="$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n1)"
JS_VERSION="$(node -p 'require("./js/package.json").version')"
PY_VERSION="$(sed -n 's/^version = "\([^"]*\)"/\1/p' crates/dealbreaker-py/pyproject.toml | head -n1)"
if [ "$CARGO_VERSION" != "$JS_VERSION" ] || [ "$CARGO_VERSION" != "$PY_VERSION" ]; then
  echo "error: version mismatch: Cargo.toml=$CARGO_VERSION js=$JS_VERSION py=$PY_VERSION" >&2
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

if [ -d docs/node_modules ]; then
  echo "[check-ci] reference docs freshness & VitePress build..."
  ./scripts/gen-docs.sh
  git diff --exit-code docs/reference/
  npm --prefix docs run build
fi

if command -v maturin >/dev/null 2>&1 && command -v pytest >/dev/null 2>&1; then
  PY_BIN=""
  if python3 -m pytest --version >/dev/null 2>&1; then
    PY_BIN="$(command -v python3)"
  elif command -v pyenv >/dev/null 2>&1 && pyenv which pytest >/dev/null 2>&1; then
    PY_BIN="$(sed -n '1s/^#!//p' "$(pyenv which pytest)" | awk '{print $1}')"
  else
    SHEBANG="$(sed -n '1s/^#!//p' "$(command -v pytest)" | awk '{print $1}')"
    if [ -n "$SHEBANG" ] && [ -x "$SHEBANG" ] && "$SHEBANG" -m pytest --version >/dev/null 2>&1; then
      PY_BIN="$SHEBANG"
    elif [ -d "${HOME}/.pyenv/versions" ]; then
      for cand in "${HOME}"/.pyenv/versions/*/bin/python; do
        if [ -x "$cand" ] && "$cand" -m pytest --version >/dev/null 2>&1; then
          PY_BIN="$cand"
          break
        fi
      done
    fi
  fi
  if [ -n "$PY_BIN" ]; then
    echo "[check-ci] python (maturin + pytest)..."
    PY_VENV="$(mktemp -d)"
    trap 'rm -rf "$PY_VENV"' EXIT
    "$PY_BIN" -m venv --system-site-packages "$PY_VENV"
    VIRTUAL_ENV="$PY_VENV" maturin develop --release --manifest-path crates/dealbreaker-py/Cargo.toml
    "$PY_VENV/bin/python" -m pytest -v crates/dealbreaker-py/tests
    rm -rf "$PY_VENV"
    trap - EXIT
  else
    echo "[check-ci] skipping python tests (no python interpreter with pytest found)"
  fi
fi

if cargo audit --version >/dev/null 2>&1; then
  echo "[check-ci] cargo audit (workspace + dealbreaker-py)..."
  cargo audit
  cargo audit --file crates/dealbreaker-py/Cargo.lock
else
  echo "[check-ci] skipping cargo audit (install with: cargo install cargo-audit)"
fi

echo "[check-ci] all checks passed!"
