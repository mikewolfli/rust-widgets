#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_setter_census.sh — RW-44 (BLUE30 §1.3 step 4)
# ============================================================================
# The rule this guards:
#
#   **A control's `pub fn set_*` is either a published command or a named exclusion.**
#
# `WidgetCapability::commands` is what a designer may bind to. A setter that is not there is
# invisible; if it is not in `PROGRAMMATIC_ONLY_SETTERS` either, nobody can tell an intentional
# omission from an accidental one. The census derives the diff from the registry itself, so the
# ledger cannot drift from the controls.
#
# # Reverse injection
#
# Step 2 removes one classified setter from the ledger (on paper) and requires a failure. Step 1
# alone would be satisfied by a comparison that never compares.
#
# Usage: tools/check_setter_census.sh
# Exit 0 = every setter is classified. Exit 1 = a finding or a failed injection.
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"
. "$ROOT_DIR/tools/lib_python.sh"

if ! "$PYTHON" tools/check_setter_census.py; then
    echo "FAIL: a setter is neither published as a command nor ledgered with a reason"
    exit 1
fi

# The injection names a setter that is genuinely classified today. If the control or setter is
# renamed, the script fails with a reason instead of injecting into nothing.
if "$PYTHON" tools/check_setter_census.py --inject=button.set_text >/dev/null 2>&1; then
    echo "FAIL: pretending a classified setter is unledgered did not make the check fail"
    exit 1
fi

# An injection naming something outside the registry must be reported as such, not silently pass.
injection_output="$("$PYTHON" tools/check_setter_census.py \
    --inject=no_such_control.no_such_setter 2>&1 || true)"
if ! printf '%s' "$injection_output" | grep -q "not a scanned control"; then
    echo "FAIL: an injection naming a control outside the registry was not reported"
    exit 1
fi

echo "setter census checks passed."
