#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_semantic_state_has_a_consumer.sh — BLUE25 A-3
# ============================================================================
# The rule this guards:
#
#   **A control has a `"<kind>:error"` preset key if and only if it implements
#   `Widget::semantic_state`.**
#
# The defect this stops
# ---------------------
# `preset_states.rs` declared `"<kind>:error"` for **ten** kinds. Only one of them
# (`line_edit`) both *reports* the state (`semantic_state()`) and *reads* it while
# drawing (`resolved_semantic_border`). The other nine keys were resolved and then
# dropped: a theme author who changed `"text_edit:error"` saw nothing move, because no
# control ever asked for it. A declared key with no consumer is the same class of
# defect as a declared event with no producer, and it is invisible without a matrix
# that puts "what the preset declares" beside "what the control reports".
#
# The rule, stated structurally
# -----------------------------
# Read the kinds named in `preset_states.rs`'s `":error"` table, then require that for
# each one a **single widget source file** carries both halves:
#   1. a `fn semantic_state(&self)` implementation (so the control reports the meaning), and
#   2. a `resolved_semantic_border("<kind>", ..)` call (so `draw` reads it).
# A kind whose two halves never meet in one file is reported as unconsumed.
#
# What this gate does NOT prove
# -----------------------------
#   * It does not prove the `semantic_state` implementation can ever return `Error` —
#     a control that always answers `None` passes. That is the control's own behavioural
#     test's job (see `lineedit.rs`'s semantic-channel tests).
#   * It is lexical. A kind that reads the border through a differently-spelled constant
#     would be reported as missing; the crate names the kind literally at the call site.
#
# Reverse injection
# -----------------
# Adding a kind to the `:error` table that has no consumer, or deleting the
# `resolved_semantic_border` read in `lineedit.rs`, must make this gate fail. See the
# round's report for the exact output.
#
# Usage: tools/check_semantic_state_has_a_consumer.sh
# Exit 0 = every declared `:error` key has one control that reports and reads it.
# Exit 1 = a finding (each offender is named with the half it lacks).
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

PRESETS="src/theme/preset_states.rs"

echo "[1/3] read the declared \`:error\` kinds from the preset"

# The kinds live in the array literal of the `for kind in [ … ]` loop whose body inserts the
# `"{kind}:error"` key. Reading them here, rather than restating them, is what makes the
# gate track the source rather than a copy that can drift.
error_line="$(grep -nE '\{kind\}:error' "$PRESETS" | head -1 | cut -d: -f1)"
if [ -z "$error_line" ]; then
    echo "  FAIL  $PRESETS no longer inserts a \`\"{kind}:error\"\` key"
    echo "        Either the table moved or it was removed; this gate is measuring nothing."
    exit 1
fi

# Take the source up to the error insert, keep the region after the **last** `for kind in [`
# that opens before it, and read the identifiers out of the array literal.
declared="$(
    head -n "$error_line" "$PRESETS" \
        | awk 'BEGIN { last = 0 } /for kind in \[/ { last = NR } { lines[NR] = $0 } END { for (i = last; i <= NR; i++) print lines[i] }' \
        | sed -n '1,/\]/p' \
        | sed -e 's/.*\[//' -e 's/\]/ /' -e 's/"//g' -e 's/,/ /g' \
        | tr ' ' '\n' \
        | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//' \
        | grep -E '^[a-z][a-z0-9_]*$'
)"""

if [ -z "$declared" ]; then
    echo "  FAIL  no \`:error\` kinds could be read from $PRESETS"
    echo "        Either the table moved or its shape changed; this gate is measuring nothing."
    exit 1
fi

echo "  found: $(printf '%s' "$declared" | tr '\n' ' ')"

echo "[2/3] one control reports and reads each declared kind"

missing_report=""
while IFS= read -r kind; do
    [ -z "$kind" ] && continue

    # Every file that reads the border for this kind.
    readers="$(grep -rlE "resolved_semantic_border\(\s*\"${kind}\"" src/widget --include=*.rs || true)"
    if [ -z "$readers" ]; then
        missing_report="${missing_report}${kind}: no control reads \`resolved_semantic_border(\"${kind}\", ..)\`
"
        continue
    fi

    # ...at least one of which also reports the meaning (the trait default does not count).
    met=false
    while IFS= read -r file; do
        [ -z "$file" ] && continue
        if grep -qE 'fn semantic_state\(&self\)' "$file"; then
            met=true
            break
        fi
    done <<< "$readers"

    if [ "$met" != true ]; then
        missing_report="${missing_report}${kind}: reads the border but no reader file implements \`semantic_state\`
"
    fi
done <<< "$declared"

if [ -n "$missing_report" ]; then
    echo "  FAIL  a declared \`:error\` kind has no consumer:"
    printf '%s' "$missing_report" | sed 's/^/          /'
    echo "        A key a control never reports is data no theme author can observe."
    echo "        Either give the control a \`semantic_state\` implementation that reads the"
    echo "        border, or remove the kind from the preset table (BLUE25 A-3)."
    exit 1
fi

echo "  PASS  every declared \`:error\` kind has a reporting, reading control"

echo "[3/3] the reader population is real"

if ! grep -rqE 'fn semantic_state\(&self\)' src/widget --include=*.rs --exclude=widget_trait.rs; then
    echo "  FAIL  no control implements \`semantic_state\` outside the trait default, so"
    echo "        step 2 could only pass vacuously"
    exit 1
fi

count="$(printf '%s\n' "$declared" | grep -c . || true)"
echo "  PASS  $count declared kind(s), each with a consumer"

echo
echo "check_semantic_state_has_a_consumer: OK"
