#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_transition_durations_are_tokens.sh — BLUE22 §6.7 (BLUE22 §E.3, gate of 5)
# ============================================================================
# The rule this guards (BLUE22 §6.7):
#
#   **An interaction transition's duration must be a theme token, not a literal.**
#
#   | gate | what it blocks | reverse injection |
#   |---|---|---|
#   | `check_transition_durations_are_tokens` | hardcoded duration (outside the token definitions) | required |
#
# # Why a duration literal is a defect and not a style preference
#
# This crate already has the token system: `Theme::motion` carries `fast` (100 ms, a pointer
# reaction), `normal` (200 ms, a control's own state change) and `slow` (300 ms, a larger
# transition), and `Button::tick` reads `theme.motion.normal` rather than naming a number. The
# header of `src/theme/types.rs` says why motion is themed at all: "a platform that respects a
# user's reduced-motion preference shortens them, a game-like skin lengthens them, and a test
# harness sets them to zero to make an animated control reach its end state deterministically."
#
# A literal defeats all three at once. `ReducedMotionPreference` exists in `src/style/primitives.rs`
# and a control that hardcodes its transition cannot honour it. A test that wants a settled state in
# one frame cannot reach it. And — the reason this is a gate rather than a note — two controls that
# should move together drift apart the moment one is tuned: the number becomes a copy of a policy,
# with nothing linking the copy to the policy.
#
# # What this gate proves
#
# A lexical, whole-tree, no-build scan of `src/widget/**`. It flags two literal shapes:
#
#   (1) `from_millis(<numeric literal>)` — a duration constructed from a number written at the call
#       site. `from_millis(duration_ms)` (a theme-derived variable) and `from_millis(delta_ms)` (the
#       host's frame delta) are not literals and are not flagged; both are the *correct* shapes.
#   (2) `const <NAME>_MS | _DELAY | _DURATION : <numeric type> = <numeric literal>` — the second way
#       a hardcoded duration hides, and the way `floating_label` used it. The name pattern is narrow
#       so it does not fire on unrelated constants that merely happen to be numbers.
#
# # Exclusions and exemptions — each with its reason
#
# Excluded by *definition* (not exempted, because these are the mechanism itself):
#
#   * `src/style/animation.rs` and `src/theme/` — the token definition sites. `AnimationConfig::default`
#     naming `Duration::from_millis(300)` is a token being defined, not a control bypassing one.
#
# Excluded by *scope*:
#
#   * Test modules (`mod tests` and below). A duration in a test is the *fixture* for an assertion —
#     a test that advances an animation by 250 ms has to say 250 — and requiring those to be tokens
#     would force every test to import constants it deliberately is not exercising. The rule's
#     subject is the durations a control actually runs at.
#   * `//` comment lines, so documentation that mentions a duration is not read as code.
#
# Exempted by *table*, `tools/transition_duration_exemptions.txt`, each entry with its reason:
#
#   * `src/widget/dialog/tooltip.rs` — `DEFAULT_SHOW_DELAY_MS` / `DEFAULT_HIDE_DELAY_MS` are hover
#     **dwell** timings, not transitions. They are already caller-settable (`set_show_delay`,
#     `set_hide_delay`), and shortening them makes a tooltip appear sooner rather than making a
#     control move differently. Folding them into `theme.motion` would put two concepts behind one
#     token. This is the only entry, and it is the only literal the scan would otherwise report
#     beyond the one live defect below.
#
# # What this gate does NOT prove
#
# Stated explicitly:
#
#   * It reads two shapes, not the language. A duration can also arrive from a `Duration::from_secs`
#     literal or a string parsed at runtime; neither is flagged because neither appears in this tree.
#     If one appears, this gate will not see it and the rule should be extended.
#   * It cannot tell a *transition* duration from an unrelated duration that happens to be
#     constructed the same way. That is why the tooltip dwell timings need an exemption rather than a
#     smarter scan: the distinction is semantic and the scan is lexical.
#   * It does not check that a control which *animates* actually reads a token, only that it does not
#     name a number inline. A control could read a literal from a non-token constant elsewhere.
#
# # Reverse injection
#
# This project's standard is "a gate that has never been seen to fail does not count". The injection
# observed when this gate landed was a literal `from_millis(250)` added to a control's tick path,
# which must make this gate name it. See the round's report for the exact output.
#
# Usage: tools/check_transition_durations_are_tokens.sh
# Exit 0 = no interaction transition names a literal duration.
# Exit 1 = a finding (each offender is named file:line).
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_python.sh"

FINDINGS="$("$PYTHON" tools/check_transition_durations_are_tokens.py)"

SUMMARY="$(printf '%s\n' "$FINDINGS" | tail -n 1)"
DETAIL="$(printf '%s\n' "$FINDINGS" | sed '$d')"
SCANNED="$(printf '%s' "$SUMMARY" | sed -n 's/.*checked=\([0-9]*\) failed=[0-9]*.*/\1/p')"
FOUND="$(printf '%s' "$SUMMARY" | sed -n 's/.*checked=[0-9]* failed=\([0-9]*\).*/\1/p')"

# A population of zero would make the gate vacuously green: it would report success having judged
# nothing, which is the failure mode this project has recorded more than once. If the scan stops
# reading the widget tree, that must be said rather than passed.
if [[ -z "${SCANNED:-}" ]] || [[ -z "${FOUND:-}" ]]; then
    echo "FAIL: the rule could not be evaluated (no summary parsed)"
    printf '%s\n' "$FINDINGS"
    exit 1
fi

# The same source of truth `check_control_rendering.sh` and `check_svg_snapshots.sh` use, so the
# reported total is the registered-control count rather than a literal that could go stale. 264 is
# the current number of `.rs` files under `src/widget/**`; if the tree grows a whole directory the
# scan would silently cover less, which is what this pairing is here to catch.
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
    echo "FAIL: these interaction transitions name a duration literal instead of a theme token, so"
    echo "      they cannot follow a reduced-motion preference or a theme's tempo:"
    printf '%s\n' "$DETAIL"
    echo ""
    echo "  Read the duration from the motion tokens -- theme.motion.fast / .normal / .slow -- the"
    echo "  way base_widgets/button.rs:182 does:"
    echo "      let duration_ms = crate::style::theme_manager()"
    echo "          .current_theme()"
    echo "          .map(|theme| theme.motion.normal)"
    echo "          .unwrap_or(200)"
    echo "          .max(1);"
    echo "  The token definitions live in src/theme/types.rs (the Motion struct) and the engine in"
    echo "  src/style/animation.rs; both are excluded from this scan because that is where a"
    echo "  duration is supposed to be written down."
    echo "  If a literal is not an interaction transition (a dwell timing, a data duration), record"
    echo "  it in tools/transition_duration_exemptions.txt with its reason."
    echo ""
    echo "check_transition_durations_are_tokens: checked=$CONTROLS failed=$FOUND"
    exit 1
fi

echo "check_transition_durations_are_tokens: checked=$CONTROLS failed=0"
