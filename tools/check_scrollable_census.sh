#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_scrollable_census.sh — RW-58 (BLUE30 §2.2 step 5)
# ============================================================================
# The rule this guards:
#
#   **A control that stores a scroll position declares its unit.**
#
# `ScrollableSurface` is pixels, `ScrollableRows` is rows. A container with neither (and no
# exclusion) makes a host branch on its concrete type to restore a scroll position, which is the
# per-control branching RW-58 removes.
#
# # Reverse injection
#
# Step 2 removes one exclusion on paper and requires the check to fail. Step 1 alone would be
# satisfied by a census that found no candidates.
#
# Usage: tools/check_scrollable_census.sh
# Exit 0 = every candidate is covered or excluded with a reason. Exit 1 = a finding.
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"
. "$ROOT_DIR/tools/lib_python.sh"

if ! "$PYTHON" tools/check_scrollable_census.py; then
    echo "FAIL: a control holds scroll state but implements neither scroll trait and has no reason"
    exit 1
fi

if "$PYTHON" tools/check_scrollable_census.py --inject=tab_bar >/dev/null 2>&1; then
    echo "FAIL: removing an exclusion did not make the check fail, so it is not checking"
    exit 1
fi

injection_output="$("$PYTHON" tools/check_scrollable_census.py \
    --inject=no_such_control 2>&1 || true)"
if ! printf '%s' "$injection_output" | grep -q "not a candidate control"; then
    echo "FAIL: an injection naming a non-candidate was not reported"
    exit 1
fi

echo "scrollable census checks passed."
