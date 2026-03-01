#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

echo "[1/2] default profile demo smoke: demo_main"
cargo check --example demo_main

echo "[2/2] embedded profile demo smoke: demo_button"
cargo check --example demo_button --no-default-features --features embedded

echo "Demo smoke checks passed."
