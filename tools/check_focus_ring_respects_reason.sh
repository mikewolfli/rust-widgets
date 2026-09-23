#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_focus_ring_respects_reason.sh — BLUE22 §6.7 (BLUE22 §E.3, gate of 5)
# ============================================================================
# The rule this guards (BLUE22 §6.7):
#
#   **A focus ring must not be painted for a focus reason that does not warrant one.**
#
#   | gate | what it blocks | reverse injection |
#   |---|---|---|
#   | `check_focus_ring_respects_reason` | a ring drawn without consulting the reason | required |
#
# # Why the reason matters
#
# Qt Quick's rule, and this crate's: a **pointer press must not leave a focus ring**. The pointer
# already tells the user where they are; a ring drawn under the cursor reads as a stuck highlight.
# `FocusReason` (src/event/types.rs) is the type that carries this, and its own documentation says
# why it exists as a method rather than a predicate at each call site:
#
#   > This is a method rather than a call site predicate so the one place that knows the answer is
#   > the one place that names the reasons -- a new variant must be classified here, not at each
#   > draw site.
#
# A draw site that paints a ring whenever it holds `focused: bool` has thrown that away: the user
# clicks a control, the control takes focus, and the ring appears under the pointer. Every test that
# drives focus programmatically still passes, because such a test never uses the pointer — which is
# exactly why this is a gate and not a test.
#
# # What this gate proves
#
# A lexical, whole-tree, no-build scan of `src/widget/**`. For every production construction of a
# focus ring — the shared `FocusRing::for_control(..)` — the file must honour the rule through ONE
# of:
#
#   (a) the site sits inside an `if` on the predicate: `visual_focus()` (which is
#       `focused && focus_reason.draws_focus_ring()`) or `draws_focus_ring()` directly; or
#   (b) the file carries both halves of the pair itself — it references `draws_focus_ring`, i.e.
#       it owns the classification of every reason.
#
# The two arms are the two ways a control can honour the rule. Arm (b) is not a weakening: a file
# that classifies the reasons has the rule *inside* it, and flagging it would demand a control
# import a predicate instead of owning one.
#
# # What this gate does NOT prove
#
# Stated explicitly, because a gate that overclaims is worse than no gate:
#
#   * It is **lexical**. It reads the ring *constructor*, not the resulting pixels. A control could
#     guard the constructor and then paint a ring some other way — a manual `draw_rect_stroke` in
#     the focus colour — and this gate would not see it. The vocabulary it matches is the shared
#     one, which is what makes the guard meaningful, but a control that bypasses the shared
#     constructor is outside this rule's reach.
#   * It does not check *which* reasons are classified, only that a classification is consulted. A
#     control that got `draws_focus_ring` wrong is a defect this gate cannot see; `FocusReason`'s own
#     test suite is where that is pinned.
#   * The guard search looks backwards a bounded window (12 lines) from the construction. A guard
#     written far above its use would not be recognised, which would report a **false positive**
#     rather than a false negative. That direction is deliberate: a false red is visible and gets
#     fixed, a false green is what this project has recorded as the more expensive failure.
#   * It matches the two predicate spellings. A control that inlines
#     `self.focused && self.focus_reason != FocusReason::Pointer` passes by arm (b) only if it also
#     names `draws_focus_ring`; inlining the classification instead is a defect the gate names.
#
# # Reverse injection
#
# This project's standard is "a gate that has never been seen to fail does not count". The injection
# observed when this gate landed was a ring construction guarded by a bare `if self.focused {` with
# no reason consulted — the exact defect the rule names — added to a control, which must make this
# gate name it. See the round's report for the exact output.
#
# Usage: tools/check_focus_ring_respects_reason.sh
# Exit 0 = every focus ring construction consults the focus reason.
# Exit 1 = a finding (each offender is named file:line).
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_python.sh"

FINDINGS="$("$PYTHON" tools/focus_ring_scan.py)"

SUMMARY="$(printf '%s\n' "$FINDINGS" | tail -n 1)"
DETAIL="$(printf '%s\n' "$FINDINGS" | sed '$d')"
SCANNED="$(printf '%s' "$SUMMARY" | sed -n 's/.*checked=\([0-9]*\) failed=[0-9]*.*/\1/p')"
FOUND="$(printf '%s' "$SUMMARY" | sed -n 's/.*checked=[0-9]* failed=\([0-9]*\).*/\1/p')"
SITES="$(printf '%s' "$SUMMARY" | sed -n 's/.*sites=\([0-9]*\).*/\1/p')"

# A population of zero would make the gate vacuously green. If the scan stops reading the widget
# tree, that must be said rather than passed.
if [[ -z "${SCANNED:-}" ]] || [[ -z "${FOUND:-}" ]]; then
    echo "FAIL: the rule could not be evaluated (no summary parsed)"
    printf '%s\n' "$FINDINGS"
    exit 1
fi

# The rule has a subject only if rings are actually painted. A tree where the scan finds no
# construction at all would pass while proving nothing, so the site count is asserted rather than
# assumed: three controls draw a ring today (switch, radio button, button).
if [[ -z "${SITES:-}" ]] || [[ "$SITES" -lt 1 ]]; then
    echo "FAIL: no focus-ring construction was found anywhere in the widget tree, so the rule"
    echo "      judged nothing. Either every ring draws through a different path (in which case"
    echo "      this scan must be widened to match it), or the tree lost its focus rings."
    exit 1
fi

CONTROLS="$("$PYTHON" -c '
import re, pathlib
src = pathlib.Path("src/widget/capability/properties.rs").read_text(encoding="utf-8")
print(len(re.findall(r"canonical_name:\s*\x22", src)))
')"
if [[ "$SCANNED" -lt "$CONTROLS" ]]; then
    echo "FAIL: the scan walked $SCANNED files but the registry publishes $CONTROLS controls;"
    echo "      the scan is not reading the whole widget tree"
    exit 1
fi

if [[ "$FOUND" -gt 0 ]]; then
    echo "FAIL: these controls paint a focus ring without asking the focus reason, so a pointer"
    echo "      click would leave a ring under the cursor:"
    printf '%s\n' "$DETAIL"
    echo ""
    echo "  Ask the shared predicate before constructing the ring -- the way"
    echo "  base_widgets/button.rs:789 does:"
    echo "      pub fn visual_focus(&self) -> bool {"
    echo "          self.focused && self.focus_reason.draws_focus_ring()"
    echo "      }"
    echo "      // ... and in draw():"
    echo "      if self.visual_focus() {"
    echo "          let ring = FocusRing::for_control(rect, radius);"
    echo "      }"
    echo "  The classification lives in src/event/types.rs :: FocusReason::draws_focus_ring, which is"
    echo "  the one place that knows that Pointer must not draw a ring."
    echo ""
    echo "check_focus_ring_respects_reason: checked=$CONTROLS failed=$FOUND sites=$SITES"
    exit 1
fi

echo "check_focus_ring_respects_reason: checked=$CONTROLS failed=0 sites=$SITES"
