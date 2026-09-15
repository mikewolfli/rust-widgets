#!/usr/bin/env bash
# ============================================================================
# check_platform_impl_matrix.sh — P1 per-OS platform_impl implementation gate
# ============================================================================
# Validates that every control `create_*` method implemented by each OS
# `platform_impl` is classified as Native / StateBacked / Placeholder (never
# Unclassifiable), so QA reports never mistake an unverified body for native.
#
# Produces `target/qa/platform_impl_matrix.md` (per-OS implementation grades).
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"
. "$ROOT_DIR/tools/lib_python.sh"

"$PYTHON" tools/platform_impl_scan.py \
  --output target/qa/platform_impl_matrix.md \
  --fail-on-unclassifiable

echo "✅ per-OS platform_impl matrix generated: target/qa/platform_impl_matrix.md"
