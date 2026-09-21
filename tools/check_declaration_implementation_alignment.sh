#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_declaration_implementation_alignment.sh — BLUE20 layer 2 (三向对齐)
# ============================================================================
# The rule this guards:
#
#   **A control's declarations must describe what the control actually does.**
#
# Why "it compiles" is not enough
# ------------------------------
# A capability record is a promise written in a *table*: `PropertySchema { name,
# value_kind, readable, writable }` and `EventSchema { name, payload }`. Nothing in the
# type system connects a table entry to the `match` arm in `WidgetProperties::get` that
# has to answer it, nor to the signal field an event name refers to, nor to a `draw`
# body that paints. Each of those pairs can drift with no compile error and no visibly
# broken control — the failure only appears when a designer offers a property that
# silently returns its default, or subscribes to an event that never fires.
#
# What it asserts
# ---------------
#   Q1  every declared property is answered by its control, with the declared writability
#   Q2  every control's `draw` paints (an empty `Draw` is forbidden by principle #5)
#   Q3  every published event list is a set, and every name carries a payload shape
#
# Two real defects this found on its first run (both fixed):
#   * `message_box` declared `modal` readable:false/writable:false while the control had a
#     working `is_modal`/`set_modal` — a property the designer hid even though it worked.
#   * `order_book::show_spread` answered `TypeMismatch` for a non-bool write and
#     `ReadOnlyProperty` for a bool one, so a caller was told "wrong type" about a name
#     that can never accept a write at all. It now answers `ReadOnlyProperty` always.
#
# Relationship to `check_event_payload_types` (principle #101)
# ------------------------------------------------------------
# That gate asks "is the declared *payload type* the signal's real type" — it locates the
# signal first and then checks the type. Q3 asks the weaker question "does the published
# name correspond to a real declared signal list at all", which that gate cannot see
# because a payload derivation starts from a name that has already been located. The two
# are complementary and do not overlap.
#
# The traversal unit is the **canonical name** (188 controls), never `WidgetKind` (179
# variants): 13 kinds are shared by 2–5 controls, so a kind sweep silently skips 19.
#
# Exit 0 = aligned. Exit 1 = a finding.
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_timeout.sh"
. "$ROOT_DIR/tools/lib_python.sh"

STEP_BUDGET="${RW_GATE_TIMEOUT:-900}"
CENSUS="tools/declaration_alignment_census.txt"

echo "[1/2] alignment: Q1/Q2/Q3 asserted against every registered control"
if ! rw_run_bounded "$STEP_BUDGET" cargo test \
    --no-default-features --features desktop \
    --test declaration_alignment_test > /tmp/rw_alignment.log 2>&1; then
    echo "  FAIL  declaration_alignment_test"
    sed -n '1,120p' /tmp/rw_alignment.log
    exit 1
fi
grep -E '^test (q1|q2|q3)' /tmp/rw_alignment.log || true
echo "  PASS  declaration_alignment_test (Q1/Q2/Q3)"

echo "[2/2] census: the counted table is present and consistent with the registry"
if [[ ! -f "$CENSUS" ]]; then
    echo "  FAIL  $CENSUS is missing; regenerate with:"
    echo "        python3 tools/check_declaration_implementation_alignment.py --update"
    exit 1
fi

if ! rw_run_bounded 300 "$PYTHON" tools/check_declaration_implementation_alignment.py; then
    echo "  FAIL  a declaration does not match its implementation"
    exit 1
fi

# Print the counted table's summary (rule #100 / #107): `checked / skipped / failed`, with
# the skipped entries listed by reason. A gate that reports "OK" while having skipped half
# its inputs is the defect class rule #107 exists to remove. The numbers are read back
# from the census rather than recomputed here, so the line cannot disagree with the
# artifact a reader can open.
CONTROLS="$(awk '/^controls /{print $2}' "$CENSUS")"
SKIPPED="$(awk '/^skipped /{print $2}' "$CENSUS")"
FAILED="$(awk '/^failed /{print $2}' "$CENSUS")"

echo ""
echo "check_declaration_implementation_alignment: checked=$CONTROLS skipped=$SKIPPED failed=$FAILED"

# A non-zero skip count is fine (a placeholder property promises nothing), but it must be
# *explicable*: the census lists every one with its reason, so an unexplained skip cannot
# hide inside the number.
if [[ "$SKIPPED" -gt 0 ]]; then
    echo "  skipped entries are listed with reasons in $CENSUS"
fi
