#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_breakpoints_are_not_in_draw.sh — BLUE24 §5 criterion 4
# ============================================================================
# The rule this guards:
#
#   **A size tier selects a subtree, not a coordinate.** A control's `draw` may use
#   its rectangle to compute *sizes*; it may not branch on width to choose
#   *structure* (which controls exist / which layout mode the whole view is in).
#
# The defect this stops
# ---------------------
# The tempting spelling of "a phone layout" is `if rect.width < 600 { drawer() } else
# { rail() }` inside a `draw`. It fails in two ways that are both invisible in a unit
# test of that one control:
#
#   1. It cannot be reviewed. "What does this application look like narrow?" then has
#      one answer per control, spread over the paint code.
#   2. It has lost the context. `Hints` propagate *upward*, so a control's intrinsic
#      size depends on where it sits; a branch taken in `draw` can no longer see
#      whether it is in a drawer or the main region.
#
# The crate has `Breakpoint` (`src/view/breakpoint.rs`) for exactly this, applied while
# *building* the tree. This gate keeps the drawn-`if` spelling from coming back.
#
# The rule, stated structurally
# -----------------------------
# Across `src/widget/**`:
#   (1) no literal numeric width threshold inside a `draw` body — the pattern
#       `rect.width < N` / `width <= N` / `width > N` with a numeric literal, which is
#       the shape a hand-rolled breakpoint takes;
#   (2) the `Breakpoint` type is never named in a widget file: a control asks about
#       tiers by *not being built at all*, so a reference to it in a control is itself
#       the smell.
#
# What this gate does NOT prove
# -----------------------------
#   * It does not forbid `if rect.width < metrics::SOME_CONST`: a *named* threshold
#     that also appears in the metric table is a size decision, and this rule is about
#     structure. Only bare numeric literals are findings.
#   * It is lexical. A threshold behind a helper function is not seen; the value of
#     this gate is that the hand-rolled spelling is recognisable and common.
#
# Reverse injection
# -----------------
# Adding `if rect.width < 600 { .. }` to any `draw` must make this gate name it. See
# the round's report for the exact output.
#
# Usage: tools/check_breakpoints_are_not_in_draw.sh
# Exit 0 = no draw path hand-rolls a size tier.
# Exit 1 = a finding (each offender is named file:line).
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

BREAKPOINT="src/view/breakpoint.rs"

echo "[1/3] the tier type exists in the view layer"
if ! grep -qE '^pub enum Breakpoint' "$BREAKPOINT"; then
    echo "  FAIL  $BREAKPOINT no longer defines \`pub enum Breakpoint\`"
    echo "        It is the declared way to select a subtree by size."
    exit 1
fi
echo "  PASS  Breakpoint is declared in the view layer"

echo "[2/3] no widget's draw body hand-rolls a width tier from a literal"
# The pattern is a comparison of a width against a bare **tier-sized** number. Three
# deliberate restrictions, each of which the measurement showed is necessary:
#
#   * Only widths >= 200. A tier boundary is a window size; a guard like
#     `bar_row.width > 20` asks "is there room for this element's own inset", which is
#     size arithmetic and is exactly what a draw body is *supposed* to do. Including
#     small numbers would flag every degenerate-shape guard in the crate.
#   * Comments are stripped, so a doc-comment naming the old spelling is documentation.
#   * `assert`/`debug_assert` lines are excluded: a test that asserts a rectangle's
#     width is a measurement, not a structure decision.
#
# What remains is the shape a hand-rolled breakpoint takes: `if rect.width < 600`.
WIDTH_TIERS="$(grep -rnE '\.width\s*(<=|>=|<|>)\s*[2-9][0-9]{2,}' src/widget/ --include=*.rs \
    | sed -e 's://.*::' \
    | grep -vE 'assert' || true)"

if [ -n "$WIDTH_TIERS" ] && [ "$(printf '%s' "$WIDTH_TIERS" | tr -d '[:space:]' | wc -c)" -gt 0 ]; then
    echo "  FAIL  a widget branches on a width against a literal:"
    printf '%s\n' "$WIDTH_TIERS" | sed 's/^/          /'
    echo "        A size tier is chosen while *building* the tree (\`Node::breakpoint\`),"
    echo "        not while painting one control: a branch here is unreviewable and has"
    echo "        lost the upward \`Hints\` context (BLUE24 §5.1)."
    exit 1
fi
echo "  PASS  no widget hand-rolls a width tier"

echo "[3/3] the tier type is not named inside a control"
# A control that names `Breakpoint` is asking a question it should not have: whether a
# subtree exists was already decided before the control was built.
#
# Comments are stripped first, so a doc-link that points at the type (the metric table's
# own rustdoc does, because that is where the thresholds live) is not a finding. The
# rule is about a control *reading* the tier, not about mentioning it.
CONTROL_HITS="$(grep -rn 'Breakpoint' src/widget/ --include=*.rs \
    | sed -e 's://.*::' \
    | grep -vE '[: ]\s*$' || true)"
if [ -n "$CONTROL_HITS" ] && [ "$(printf '%s' "$CONTROL_HITS" | tr -d '[:space:]' | wc -c)" -gt 0 ]; then
    echo "  FAIL  a control names \`Breakpoint\`:"
    printf '%s\n' "$CONTROL_HITS" | sed 's/^/          /'
    echo "        Whether a subtree exists is settled during the build; a control that"
    echo "        asks again has two places deciding the same thing (BLUE24 §5.1)."
    exit 1
fi
echo "  PASS  no control consults the breakpoint"

echo
echo "check_breakpoints_are_not_in_draw: OK"
