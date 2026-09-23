#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_implicit_size_uses_metrics.sh — BLUE22 §6.7 (BLUE22 §E.3, gate of 5)
# ============================================================================
# The rule this guards (BLUE22 §6.7):
#
#   **A control's size hint must come from the shared metric system, not from literals.**
#
#   | gate | what it blocks | reverse injection |
#   |---|---|---|
#   | `check_implicit_size_uses_metrics` | a `size_hint` that computes `Size::new(w, h)` from numbers | required |
#
# # Why a hand-rolled size hint is a defect and not a style preference
#
# `ControlMetrics` exists so the answer to "how big is this control?" is derived once. A control
# that writes its own arithmetic has made a **second copy of that answer**, and the copy has no
# link to the original: the two drift the moment either is tuned. This crate has already paid for
# exactly that: `checkbox.rs`'s hint was `text.len() * 8 + 24`, with the comment "16px checkbox +
# 4px padding + text" beside it — naming `INDICATOR_SIZE`, a constant that lives elsewhere, as a
# literal. Changing `INDICATOR_SIZE` would have left the hint wrong and the comment lying.
#
# The same shape appeared at three sites (checkbox, radio button, label), each with its own
# character-advance estimate. That is the iceberg: one literal in one hint means a class of them.
#
# # What this gate proves
#
# A lexical, whole-tree, no-build scan of `src/widget/**`. For every production `fn size_hint`
# (test modules excluded), the body must reference at least one of:
#
#   * `ControlMetrics` — the shared derivation;
#   * `estimate_text_width` / `estimate_line_height` — the pure measurement primitives it reads,
#     which exist precisely so a hint with no `RenderContext` can measure honestly;
#   * `dimensions::` — the named-constant table.
#
# A hint that uses none of them is deriving its answer from bare numbers.
#
# # What this gate does NOT prove
#
# Stated explicitly, because a gate that overclaims is worse than no gate:
#
#   * It is **lexical**, not semantic. A hint that mentions `ControlMetrics` and then overrides the
#     result with a literal passes. The gate raises the floor; it is not a proof of correctness.
#     The companion evidence is the round's `implicit_size` tests, which pin the numbers.
#   * It does not check that the *value* the hint returns is right, only where it came from.
#   * It cannot tell a control whose size genuinely is a constant (an icon, a swatch) from one that
#     has content and ignored it. Both are exempt; see the table below.
#
# # Exclusions and exemptions — each with its reason
#
# Excluded by *definition*:
#
#   * `src/widget/widget_trait.rs` — `Widget::size_hint`'s default implementation. That is the
#     trait's own fallback, i.e. the mechanism, not a control bypassing it.
#
# Excluded by *scope*:
#
#   * Test modules (`mod tests` and below). A test that pins a hint's value must name the value it
#     expects; requiring a token there would force the test to assert the implementation against
#     itself.
#   * `//` comment lines are not excluded (the scan reads the raw body), because a hint's comment is
#     part of the site a reviewer reads.
#
# Exempted by *table*, `tools/implicit_size_exemptions.txt`, keyed on `path:line` so an entry
# cannot cover a second hint in the same file. The table has two sections and the header says so:
#
#   * ~158 **backlog** entries — hints that are hand-rolled today and that a later round should
#     route through `ControlMetrics`. This is a debt listing, not a claim of correctness. It
#     shrinks: fixing a hint moves its line, which turns the entry red until it is removed.
#   * a small number of **permanent** entries — controls whose size is a fixed piece of their own
#     chrome (an icon, a swatch), which have no content to measure.
#
# # Reverse injection
#
# This project's standard is "a gate that has never been seen to fail does not count". The injection
# observed when this gate landed was a `fn size_hint` whose body was
# `Size::new(self.text.len() as u32 * 8 + 24, 24)` — the exact defect the rule names — added to a
# control, which must make this gate name it. See the round's report for the exact output.
#
# Usage: tools/check_implicit_size_uses_metrics.sh
# Exit 0 = every production size hint reads the metric system.
# Exit 1 = a finding (each offender is named file:line).
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_python.sh"

FINDINGS="$("$PYTHON" tools/implicit_size_scan.py)"

SUMMARY="$(printf '%s\n' "$FINDINGS" | tail -n 1)"
DETAIL="$(printf '%s\n' "$FINDINGS" | sed '$d')"
SCANNED="$(printf '%s' "$SUMMARY" | sed -n 's/.*checked=\([0-9]*\) failed=[0-9]*.*/\1/p')"
FOUND="$(printf '%s' "$SUMMARY" | sed -n 's/.*checked=[0-9]* failed=\([0-9]*\).*/\1/p')"

# A population of zero would make the gate vacuously green: it would report success having judged
# nothing. If the scan stops reading the widget tree, that must be said rather than passed.
if [[ -z "${SCANNED:-}" ]] || [[ -z "${FOUND:-}" ]]; then
    echo "FAIL: the rule could not be evaluated (no summary parsed)"
    printf '%s\n' "$FINDINGS"
    exit 1
fi

# The same source of truth `check_control_rendering.sh` and `check_svg_snapshots.sh` use, so the
# reported total is the registered-control count rather than a literal that could go stale.
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
    echo "FAIL: these size hints derive an answer from literals instead of the shared metric system,"
    echo "      so the number they claim has no link to the one every other control uses:"
    printf '%s\n' "$DETAIL"
    echo ""
    echo "  Route the hint through the metric system -- the way base_widgets/button.rs:442 and"
    echo "  base_widgets/checkbox.rs do:"
    echo "      ControlMetrics::implicit_size(Size::new(content_w, 0), my_padding, my_floor)"
    echo "  A hint that has no RenderContext to measure text with should read"
    echo "  src/widget/metrics.rs's pure primitives, estimate_text_width / estimate_line_height,"
    echo "  which use the same advance model the renderer draws with."
    echo "  If the control's size genuinely is a constant (an icon, a swatch) or is delegated to a"
    echo "  real child, record it in tools/implicit_size_exemptions.txt with its reason."
    echo ""
    echo "check_implicit_size_uses_metrics: checked=$CONTROLS failed=$FOUND"
    exit 1
fi

echo "check_implicit_size_uses_metrics: checked=$CONTROLS failed=0"
