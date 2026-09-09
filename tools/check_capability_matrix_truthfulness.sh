#!/usr/bin/env bash
# ============================================================================
# check_capability_matrix_truthfulness.sh — P1 capability matrix honesty gate
# ============================================================================
# Regenerates the capability matrix from its source table and cross-checks the
# generated document against the real per-OS platform_impl implementation grades
# (no ✅ usable-path claim for a desktop platform whose platform_impl lacks a
# create path).
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

MATRIX_FILE="docs/plans/platform_capability_matrix.md"

python3 tools/generate_platform_capability_matrix.py --output "$MATRIX_FILE"
python3 tools/check_capability_matrix_truthfulness.py

echo "✅ capability matrix regenerated and truthfulness-checked: $MATRIX_FILE"
