#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

echo "[1/3] cargo check (default)"
cargo check

echo "[2/3] cargo check --examples"
cargo check --examples

echo "[3/3] cargo check --no-default-features --features embedded"
cargo check --no-default-features --features embedded

echo "All profile checks passed."
