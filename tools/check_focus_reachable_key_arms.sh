#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_focus_reachable_key_arms.sh — W5-02, §8 extension direction 4
# ============================================================================
# The rule this guards:
#
#   **A keyboard arm gated on `focus_reason().is_some()` is only meaningful if the control
#   can become the focus owner.**
#
# # Why the gate exists
#
# Gating a key branch on the control holding focus is the correct shape: it stops a control
# embedded in a page from swallowing the arrows of whatever the user is editing. But the gate
# is a predicate over a fact -- "this control has focus" -- that the router can only make true
# by asking `Widget::is_focusable`. That default derives from
# `WidgetKind::wants_keyboard_focus`, and the pointer router focuses on a press only when it
# answers `true` (`src/widget/runtime/pointer.rs`). A control whose kind is not listed and that
# does not override `is_focusable` therefore has a keyboard branch that **cannot run**: a click
# never focuses it and Tab never reaches it. That was W4-06 for `TabView`, and W5-02 found nine
# more controls in the same state -- the exact "code exists, never triggers" defect a test that
# drives `Event::KeyPress` directly cannot see.
#
# # What this gate proves
#
# For every production file under `src/widget/**` that gates a `KeyPress` arm on
# `focus_reason().is_some()`, ONE of:
#
#   (a) the control's kind is in `WidgetKind::wants_keyboard_focus`; or
#   (b) the control overrides `fn is_focusable`;
#
# with the kind resolved through the control's sibling files when the arm lives in a split
# `events.rs`/`paint.rs` file.
#
# # What this gate does NOT prove
#
#   * It is lexical. A control could make itself focusable at runtime by another route (a host
#     calling `focus()` directly). That route exists and is legitimate, but it is not the
#     pointer/Tab path this rule is about, and it is not what the arms assume.
#   * It does not check that the arm's *body* does anything useful, only that the gate around it
#     can be satisfied.
#   * A kind listed in the table is assumed to be listed for a control that can actually use
#     it; the table's own comment explains the positive list's intent.
#
# # Reverse injection
#
# Performed when the gate landed: `src/widget/display_widgets/tag.rs` was caught by the first
# run of the scanner -- `WidgetKind::Tag` is not focusable and the tag had no override, so its
# Delete/Backspace arm was dead. The gate was red on that file until `is_focusable` was added
# (returning `self.closable`, because a tag with no close action has no keyboard job). Removing
# that override makes the gate red again; the observed output is the finding quoted above.
#
# Usage: tools/check_focus_reachable_key_arms.sh
# Exit 0 = every focus-gated keyboard arm belongs to a control that can hold focus.
# Exit 1 = a finding (each offender is named file:line).
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_python.sh"

FINDINGS="$("$PYTHON" tools/check_focus_reachable_key_arms.py)"

SUMMARY="$(printf '%s\n' "$FINDINGS" | tail -n 1)"
DETAIL="$(printf '%s\n' "$FINDINGS" | sed '$d')"
CHECKED="$(printf '%s' "$SUMMARY" | sed -n 's/.*checked=\([0-9]*\) failed=[0-9]*.*/\1/p')"
FAILED="$(printf '%s' "$SUMMARY" | sed -n 's/.*checked=[0-9]* failed=\([0-9]*\).*/\1/p')"

if [[ -z "${CHECKED:-}" ]] || [[ -z "${FAILED:-}" ]]; then
    echo "FAIL: the rule could not be evaluated (no summary parsed)"
    printf '%s\n' "$FINDINGS"
    exit 1
fi

# A scan that read no controls would pass while proving nothing. The tree has dozens of
# focus-gated arms today; require a floor well below that so the check is not brittle, but high
# enough that a broken walk is visible.
if [[ "$CHECKED" -lt 20 ]]; then
    echo "FAIL: only $CHECKED focus-gated keyboard arms were found; the scan is not reading the"
    echo "      widget tree."
    exit 1
fi

if [[ "$FAILED" -gt 0 ]]; then
    echo "FAIL: these keyboard arms are gated on focus, but the control can never become the"
    echo "      focus owner -- a click does not focus it and Tab skips it:"
    printf '%s\n' "$DETAIL"
    echo ""
    echo "  Either add the control's kind to WidgetKind::wants_keyboard_focus, or override"
    echo "  Widget::is_focusable on the control (see nav_widgets/tab_view.rs for the precedent)."
    echo ""
    echo "check_focus_reachable_key_arms: checked=$CHECKED failed=$FAILED"
    exit 1
fi

echo "check_focus_reachable_key_arms: checked=$CHECKED failed=0"
