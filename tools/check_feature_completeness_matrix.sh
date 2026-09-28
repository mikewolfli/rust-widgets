#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# check_feature_completeness_matrix.sh — the feature-completeness allowlist must be live.
#
# ============================================================================
# The rule this guards
# ============================================================================
#
#   Every row in `tools/feature_completeness_allowlist.toml` must still suppress a real
#   finding in the file or module it names.
#
# # Why this is a gate and not only a report
#
# This script used to be a generator wrapper: it produced
# `target/qa/feature_completeness_matrix.md` and exited 0 unconditionally. It was
# registered in `tools/check_*.sh`, so `check_gates_are_worth_running.sh` counted it as a
# gate — but **no input could make it fail**, which is the "gate that cannot fail"
# defect class (BLUE24 §9.2 criterion 3). See `tools/gates_reverse_injection.md`,
# "BLUE25 C-wave 1".
#
# The report it generates is still the artifact CI uploads. What this gate adds is an
# **assertion** over the data that report is built from: a suppression whose file or
# category no longer contains the finding is a dead exemption. It silences nothing, yet a
# reader of the allowlist believes a real finding is still being excused — the same defect
# the rendering and colour exemption tables already guard against ("an exemption that is
# no longer needed is a defect").
#
# # What this checks
#
#   [1] the matrix generates (the report artifact CI uploads)
#   [2] every allowlist row still suppresses at least the finding it claims
#
# Exit 0 = the matrix generated and the allowlist has no stale rows.
# Exit 1 = at least one allowlist row is stale, or generation failed.
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"
. "$ROOT_DIR/tools/lib_python.sh"

echo "[1/2] generating the feature-completeness matrix"
"$PYTHON" tools/generate_feature_completeness_matrix.py \
  --src src \
  --output target/qa/feature_completeness_matrix.md \
  --threshold 1

echo "[2/2] the allowlist has no stale rows"
if ! "$PYTHON" tools/generate_feature_completeness_matrix.py \
    --src src \
    --output target/qa/feature_completeness_matrix.md \
    --threshold 1 \
    --assert-allowlist-is-live; then
    echo "  FAIL  the feature-completeness allowlist excuses findings that are no longer present"
    exit 1
fi

echo
echo "check_feature_completeness_matrix: OK"
