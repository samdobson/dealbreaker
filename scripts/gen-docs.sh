#!/usr/bin/env bash
# Regenerate all auto-generated docs/reference/*.md pages from source of truth.
set -euo pipefail
cd "$(dirname "$0")/.."

node docs/scripts/gen-reference.mjs
python3 docs/scripts/gen-python-ref.py
