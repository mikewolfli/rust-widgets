#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_spacing_is_not_sibling_layout.sh — BLUE22 §6.7 (BLUE22 §E.3, gate of 5)
# ============================================================================
# The rule this guards (BLUE22 §6.7):
#
#   **`spacing` must not be used for sibling layout.**
#
#   | gate | what it blocks | reverse injection |
#   |---|---|---|
#   | `check_spacing_is_not_sibling_layout` | `spacing` used as the gap between two siblings | required |
#
# # Why one field with two meanings is a defect
#
# `Style::spacing` (src/style/primitives.rs) carries this contract, verbatim:
#
#   > QML's `CheckBox.qml:61` and `ComboBox.qml:21` both set `spacing`, and in both it means
#   > exactly one thing: the distance from the control's own indicator to its own text. It is a
#   > fact about *this control's* contents, so a checkbox, a radio and a menu item with the same
#   > `spacing` look like one family.
#   >
#   > The gap between two *siblings* is a different fact and belongs to the layout that places them
#   > (`FlexLayout::gap`), because the sibling pair is the layout's knowledge, not either control's.
#
# The moment one control reads `spacing` as the gap to the control *next to* it, the field no longer
# has one meaning: a theme that tightens a checkbox's label gap silently also moves its neighbours,
# and no caller can change one without the other. The two roles have to be separable, which means
# the field has to be confined to the first.
#
# # What this gate proves
#
# A lexical, whole-tree, no-build scan of `src/widget/**`. For every file that reads the style field
# — spelled `style().spacing`, the only form that reaches `Style::spacing` — the file must also
# declare or call the one accessor that names the legitimate role:
#
#   * `fn label_gap(..)` / `label_gap()` — the method whose documented contract is "the distance
#     from the control's own indicator to its own text".
#
# The accessor is the claim, made in place next to the derivation it protects, rather than an entry
# in a side table. That is deliberate: the rule is narrow enough that a file either has a
# `label_gap` or it does not, and an exemption list would let the second reading back in silently.
#
# # What this gate does NOT prove
#
# Stated explicitly:
#
#   * It is **lexical**. A file with a `label_gap` accessor could read `style().spacing` somewhere
#     else for a different purpose and pass. The gate confines the *spelling*; the contract is
#     still the reviewer's.
#   * It does not flag a layout's own `gap` field, which is the correct way to state a sibling gap.
#     That is not a miss: `gap` is what the rule asks for.
#   * It matches the field read `style().spacing` and nothing else. A control that copies the value
#     into a `let` on one line and uses it two lines later is judged at the read site, which is
#     where the role is decided.
#   * The bare word `spacing` is deliberately **not** matched. `let spacing = TAB_SPACING`, the
#     capability key `"spacing"` and `fn set_bar_spacing(.., spacing: f32)` are three unrelated
#     things that share the word; matching them would make the gate so noisy it would be ignored.
#
# # Reverse injection
#
# This project's standard is "a gate that has never been seen to fail does not count". The injection
# observed when this gate landed was a control reading `self.style().spacing.unwrap_or(0)` **without**
# a `label_gap` accessor — the exact shape the rule names — which must make this gate name the file.
# See the round's report for the exact output.
#
# Usage: tools/check_spacing_is_not_sibling_layout.sh
# Exit 0 = every `Style::spacing` reader confines it to the indicator-to-text role.
# Exit 1 = a finding (each offender is named file:line).
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_python.sh"

FINDINGS="$("$PYTHON" tools/spacing_scan.py)"

SUMMARY="$(printf '%s\n' "$FINDINGS" | tail -n 1)"
DETAIL="$(printf '%s\n' "$FINDINGS" | sed '$d')"
SCANNED="$(printf '%s' "$SUMMARY" | sed -n 's/.*checked=\([0-9]*\) failed=[0-9]*.*/\1/p')"
FOUND="$(printf '%s' "$SUMMARY" | sed -n 's/.*checked=[0-9]* failed=\([0-9]*\).*/\1/p')"

# A population of zero would make the gate vacuously green. If the scan stops reading the widget
# tree, that must be said rather than passed.
if [[ -z "${SCANNED:-}" ]] || [[ -z "${FOUND:-}" ]]; then
    echo "FAIL: the rule could not be evaluated (no summary parsed)"
    printf '%s\n' "$FINDINGS"
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
    echo "FAIL: these controls read the style's \`spacing\` without confining it to the"
    echo "      indicator-to-text role, so a theme cannot change one gap without the other:"
    printf '%s\n' "$DETAIL"
    echo ""
    echo "  \`Style::spacing\` means ONE thing -- the distance from a control's own indicator to its"
    echo "  own text (src/style/primitives.rs, the field's own contract). A gap between two siblings"
    echo "  is the *layout's* fact and belongs in a layout's own \`gap\`, not in a control."
    echo "  Route the read through the accessor that names the role, the way"
    echo "  base_widgets/checkbox.rs:109 does:"
    echo "      fn label_gap(&self) -> i32 {"
    echo "          self.style().spacing.unwrap_or(dimensions::INDICATOR_TEXT_SPACING) as i32"
    echo "      }"
    echo ""
    echo "check_spacing_is_not_sibling_layout: checked=$CONTROLS failed=$FOUND"
    exit 1
fi

echo "check_spacing_is_not_sibling_layout: checked=$CONTROLS failed=0"
