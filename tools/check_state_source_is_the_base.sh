#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_state_source_is_the_base.sh -- BLUE23 §2.2 (gate of 5)
# ============================================================================
# The rule this guards (BLUE23 §2.2, judgement 4):
#
#   **Hover, press, grab and the focus reason each have exactly one source: BaseWidget.**
#
# # Why this is a gate and not a test
#
# The state channel is only real if "am I hovered?" has one answer. The runtime already
# commits to that answer -- `widget::runtime::dispatch_hover_transition` synthesises the
# `MouseEnter`/`MouseLeave` pair from the pointer position -- so a control that keeps its
# own `hovered: bool` maintains a *second* opinion of the same fact. Those two opinions
# drift: the runtime delivers the pair to everyone, while only the control that wired its
# private field notices. That is exactly how a `"button:hover"` theme entry stayed
# unreachable while `Button` alone looked correct and every test passed.
#
# The same shape applies to `focus_reason`. A private copy can disagree with the base's,
# and then `draws_focus_ring()` answers differently at two call sites -- the ring appears
# under the cursor in one control and not another, with no test able to see it.
#
# # What this gate proves
#
# A lexical, whole-tree, no-build scan of `src/widget/**` (excluding `base.rs`, the one
# file allowed to declare the fields). No field of the widget-level state shape --
# `hovered: bool`, `pressed: bool`, `grabbed: bool`, `focus_reason: FocusReason` -- may be
# declared anywhere else. Fields that merely *contain* those words but answer a different
# question (`hovered_tab`, `hovered_item`, `minimize_hovered`) are deliberately not
# matched; a data hover is not the widget's own hover.
#
# # What this gate does NOT prove
#
#   * It is **lexical**. A control could cache hover in a field named something else
#     (`pointer_inside: bool`) and drift just the same. The vocabulary the crate uses is
#     what makes the guard meaningful, and that vocabulary is uniform today -- the scan
#     matched seven fields and all seven were the widget-level shape.
#   * It does not read draw paths. A control can paint hover without a field, by reading
#     `base.is_hovered()` in `draw`; that is the *desired* form and the gate does not --
#     and should not -- require it.
#
# # Reverse injection
#
# This project's standard is "a gate that has never been seen to fail does not count".
# Step 2 appends a private `hovered: bool` to a named widget and requires the scan to
# report it, so a search that matched nothing cannot pass.
#
# Usage: tools/check_state_source_is_the_base.sh
# Exit 0 = the four facts are declared only in BaseWidget.
# Exit 1 = a finding (each offender is named file:line).
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_python.sh"

if ! "$PYTHON" tools/state_source_scan.py; then
    echo ""
    echo "  Fix: delete the private field and read the base accessor instead --"
    echo "      if self.base.is_pressed() { ... } else if self.base.is_hovered() { ... }"
    echo "  The base records all four facts from the primitive input events for every"
    echo "  control, so a private copy is never needed."
    exit 1
fi

# Reverse injection: adding a second source must fail the scan, or the search is not
# reading the tree it reports on.
if "$PYTHON" tools/state_source_scan.py --inject=src/widget/display_widgets/spinner.rs >/dev/null 2>&1; then
    echo "FAIL: injecting a private hover field did not change the result, so the check"
    echo "      is not comparing against the widget tree"
    exit 1
fi

echo "state-source checks passed."
