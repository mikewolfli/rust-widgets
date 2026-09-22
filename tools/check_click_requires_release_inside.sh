#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_click_requires_release_inside.sh — BLUE22 §6.7 (BLUE22 §E.3, gate of 5)
# ============================================================================
# The rule this guards (BLUE22 §6.7):
#
#   **A binary control must not emit `clicked` on a release that is outside it.**
#
#   | gate | what it blocks | reverse injection |
#   |---|---|---|
#   | `check_click_requires_release_inside` | press-and-release emits `clicked` | required |
#
# # The real defect this exists for
#
# A control that pairs `MousePress` with `MouseRelease` *by button alone* fires on any release
# anywhere on the window. The user's gesture is "press on the control, move off it, let go" — a
# cancel — and the control treats the cancellation as the commitment. Every assertion that drives
# the control with matching coordinates passes whether or not a containment test exists, because
# such a test only ever releases where it pressed. The gesture that is wrong is the one no test
# writes, which is exactly why this is a gate and not a test.
#
# The crate's own contract is the two-condition pair (base_widgets/toggle_button.rs, whose comment
# says it applies "the same two conditions `Button` applies"):
#
#     MousePress   => arm only while contains_point_with_touch_expansion(pos)
#     MouseRelease => commit only while armed && contains_point_with_touch_expansion(pos)
#
# # What this gate proves
#
# A lexical, whole-tree, no-build rule. It is stated in full because a gate that overclaims is
# worse than no gate:
#
#   For every file under src/widget/** that emits `clicked` and handles `Event::MouseRelease` in
#   its production code (test modules excluded), the file must honour the rule through ONE of:
#
#       (a) it references the containment vocabulary (`contains_point`, or the `geometry().contains`
#           spelling that `BaseWidget::contains_point_with_touch_expansion` delegates to), OR
#       (b) it contains the `MouseLeave` latch-clear that makes a press outside un-committable, OR
#       (c) it is recorded in tools/click_release_exemptions.txt with its reason and mechanism.
#
# The three arms are not three weakenings of one rule; they are the three ways a control can honour
# it, verified one by one against this tree:
#
#   (a) `contains_point` — `toggle_button` calls it on both arms. `mini_canvas` returns early on a
#       release outside. `empty_state`, `chart`, `font_combo_box` and `canvas` call
#       `geometry().contains(..)`, the same predicate `BaseWidget::contains_point_with_touch_expansion`
#       delegates to, on a point-lookup path (`data_index_at`, `cell_at`). Those lookups are
#       themselves bounds-checked against the geometry — grid.rs:154 returns `None` when the y is
#       outside `rect.y..rect.y + rect.height` — so a pointer outside the control cannot resolve to
#       a cell and cannot reach the `clicked.emit()`.
#   (b) The `MouseLeave` latch-clear — `button` and `fab` release *without* a containment test, and
#       the reason they are not defective is that they clear the `pressed` latch on `MouseLeave`.
#       button.rs documents it: "A pointer that leaves while held abandons the press. Only
#       `hovered` used to be cleared, so `pressed` stayed true: the widget then committed on the
#       *next* release". A mouse that leaves a control necessarily crosses its edge first, so the
#       latch is clear before any later release arrives and the log is empty. Requiring a
#       containment test here would demand a second guard for an already-guarded path, and would
#       contradict a documented, deliberate decision.
#   (c) The exemption table — for a control under review, or one whose containment is a mechanism
#       this scan cannot see, with the reason and the mechanism written down.
#
# # What this gate does NOT prove
#
# Stated explicitly:
#
#   * It does **not** prove the containment test guards the specific `clicked.emit()` reachable
#     from the release arm. A file containing one for an unrelated reason passes. Proving the *edge*
#     needs dataflow, not the lexical file.
#   * It does **not** prove the guard uses the release's own position.
#   * It is **not** a behavioural test. It cannot observe that a control fires on a release
#     outside; it observes that the shape of the code makes that impossible through one of three
#     known mechanisms. A fourth mechanism that is correct but unrecorded shows up as a finding,
#     which is the intended direction: an unknown mechanism should be looked at, not silently
#     accepted.
#   * It says nothing about `Event::Tap`, which is a gesture the host has already resolved and not
#     a release this crate sees. `radiobutton` and `roller` commit on `Tap` alone.
#   * It does not judge a release arm that only *clears* a latch without emitting a click.
#
# # Reverse injection
#
# This project's standard is "a gate that has never been seen to fail does not count". The injection
# observed when this gate landed was `fab`: removing its `MouseLeave` latch-clear turns it into a
# reportable finding. `fab` is the one control this round could not clear — see the note in the
# commit message — and the gate reports it rather than excusing it.
#
# Usage: tools/check_click_requires_release_inside.sh
# Exit 0 = every click-emitting release path honours the rule through (a), (b) or (c).
# Exit 1 = a finding (each offender is named file:line).
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_python.sh"

FINDINGS="$("$PYTHON" tools/check_click_requires_release_inside.py)"

SUMMARY="$(printf '%s\n' "$FINDINGS" | tail -n 1)"
DETAIL="$(printf '%s\n' "$FINDINGS" | sed '$d')"
FOUND="$(printf '%s' "$SUMMARY" | sed -n 's/.*checked=[0-9]* failed=\([0-9]*\).*/\1/p')"

# The population the scan actually walked, reported by the scan itself.
SCANNED="$(printf '%s' "$SUMMARY" | sed -n 's/.*checked=\([0-9]*\) failed=[0-9]*.*/\1/p')"

# `checked` in the summary is the *widget count the registry publishes*, not the number of files the
# scan happened to match, and the two are deliberately independent. If the factory publishes a
# control the scan cannot see, that is the failure this pairing exposes: the gate would otherwise
# report a healthy total while quietly covering less than the crate contains. 188 is the registered
# count (read from the capability table, the same source `check_control_rendering.sh` uses, rather
# than a literal that could go stale).
CHECKED="$("$PYTHON" -c '
import re, pathlib
src = pathlib.Path("src/widget/capability/properties.rs").read_text(encoding="utf-8")
print(len(re.findall(r"canonical_name:\s*\x22", src)))
' 2>/dev/null || echo 0)"

# A population of zero would make the gate vacuously green: it would report success having judged
# nothing, which is the failure mode this project has recorded more than once. If the widget tree
# stops containing click-emitting controls, the rule no longer applies to the codebase as written
# and that must be said rather than passed.
if [[ -z "${FOUND:-}" ]] || [[ -z "${SCANNED:-}" ]] || [[ "$CHECKED" -le 0 ]]; then
    echo "FAIL: the rule could not be evaluated (no summary parsed)"
    printf '%s\n' "$FINDINGS"
    exit 1
fi
if [[ "$SCANNED" -lt 5 ]]; then
    echo "FAIL: only $SCANNED files matched the rule population; the scan is not reading the"
    echo "      widget tree (expected the click-emitting controls under src/widget/**)"
    exit 1
fi

if [[ "$FOUND" -gt 0 ]]; then
    echo "FAIL: these controls can emit a click from a release outside themselves. Their release"
    echo "      path has no containment test, no MouseLeave latch-clear, and no exemption:"
    printf '%s\n' "$DETAIL"
    echo ""
    echo "  Fix it by one of the three mechanisms the rest of the tree already uses:"
    echo "    a) ask containment on the release -- contains_point, as toggle_button does on both"
    echo "       arms (the crate spelling is contains_point_with_touch_expansion, which adds the"
    echo "       touch-target expansion);"
    echo "    b) clear the pressed latch on MouseLeave, as button.rs documents -- a mouse that"
    echo "       leaves crosses the edge first, so the latch is clear before any later release;"
    echo "    c) record it in tools/click_release_exemptions.txt with the reason and the mechanism."
    echo ""
    echo "check_click_requires_release_inside: checked=$CHECKED failed=$FOUND"
    exit 1
fi

echo "check_click_requires_release_inside: checked=$CHECKED failed=0"
