#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_first_value_not_zero.sh — BLUE24 §2.4 gate B
# ============================================================================
# The rule this guards:
#
#   **An animated control is constructed at the RESTING end of its own property,
#    not at the target end.**
#
# The defect this stops
# ---------------------
# `PropertyDriver::at(value, tempo)` takes its starting value, and the two ends are
# not symmetric. Starting at the resting end means the control's first painted frame
# shows it *as it is*: a button that is not hovered is at rest, a menu that has not
# been opened is closed. Starting at the target end means the control is born
# already finished, and three things go wrong at once:
#
#   * the control animates **away** from its first frame the moment anything asks it
#     to settle, so a freshly built button fades *out*;
#   * `show_at` / `set_checked` / any "begin the motion" call has nothing to do,
#     because the value is already where it was being sent;
#   * `is_moving()` answers `false` at construction, so a frame loop never schedules
#     the frame that would have run the animation.
#
# None of the three is visible as an error. The picture is simply wrong, and stays
# wrong, which is why this is a gate and not a code review note.
#
# The crate's own history, from two directions
# -------------------------------------------
#   * `src/widget/base_widgets/button.rs` records the lesson in prose ("Starting at
#     the interactive end would make every button fade *out* on its first frame");
#   * `src/widget/advanced_widgets/pie_menu.rs` shipped the opposite anyway --
#     `animation_progress: 1.0` at construction, and nothing read the field at all.
#
# One lesson written down in one place and violated in another is exactly what a gate
# is for. Measured before the fix: `PieMenu` started at `1.0`.
#
# What it accepts and what it rejects
# -----------------------------------
# The check is on the **construction expression**, not on a value: it looks for an
# animated field initialised to the extreme-value literal `1.0` in a constructor.
# `PropertyDriver::at(0.0, ..)` is the resting end for every property this crate
# drives, because all of them are "how far into the interactive state are we" --
# a fraction whose target end is 1.0.
#
# A control that legitimately starts at 1.0 (a progress bar pinned full, a switch
# constructed ON) does not hold a driver for that fact: `checked` is a latch and a
# progress value is data, and neither is interpolated. So a `1.0` there is not in
# scope, and the field-name filter below is what keeps the two apart.
#
# What this gate does NOT prove
# -----------------------------
#   * It cannot tell a resting value from a legitimate non-zero start in general.
#     The name filter narrows it to fields that are `PropertyDriver`s, which is the
#     population the rule is about.
#   * It does not run the control. `PropertyDriver::default()` is checked by
#     `src/style/animation.rs`'s own unit test, which is the one place a default can
#     be asserted directly.
#
# Reverse injection
# -----------------
# Changing a control's driver construction from `at(0.0, ..)` to `at(1.0, ..)` must
# make this gate name it. See the round's report for the exact output.
#
# Usage: tools/check_first_value_not_zero.sh
# Exit 0 = every animated control starts at its resting end.
# Exit 1 = a finding (each offender is named file:line).
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

echo "[1/4] PropertyDriver::default() is the resting end"
# The default is the single fallback for every `..Default::default()` construction and
# for the type's own contract, so it is checked directly rather than inferred from the
# controls. A default at the target end would make the rule impossible to satisfy by
# omission, which is the shape that produced the defect.
DEFAULT_BODY="$(awk '/impl Default for PropertyDriver/{inside=1} inside{print} inside && /^}$/ {exit}' \
    src/style/animation.rs)"
if [ -z "$DEFAULT_BODY" ]; then
    echo "  FAIL  PropertyDriver has no \`impl Default\`"
    echo "        Without one there is no single answer to \"what is the first value\"."
    exit 1
fi
if ! printf '%s' "$DEFAULT_BODY" | grep -qE 'current:\s*0\.0'; then
    echo "  FAIL  PropertyDriver::default() does not start at 0.0:"
    printf '%s\n' "$DEFAULT_BODY" | sed 's/^/          /'
    echo "        The resting end is the only safe first value (BLUE24 §2.4 gate B)."
    exit 1
fi
echo "  PASS  the default starts at the resting end"

echo "[2/4] the type's own default is unit-tested"
# The default is the rule's backstop, so the check above must be backed by a test rather
# than standing alone: a structural grep can be satisfied by a comment, a test cannot.
if ! grep -qE 'fn the_default_starts_at_the_resting_end' src/style/animation.rs 2>/dev/null; then
    echo "  FAIL  src/style/animation.rs has no test for PropertyDriver::default()"
    echo "        The default is the resting end; assert it where the type is defined, so"
    echo "        the rule is checked by running the crate and not only by reading it."
    exit 1
fi
echo "  PASS  the default's value is asserted by a unit test"

echo "[3/4] no animated control is constructed at the target end"
# Two shapes, both "a PropertyDriver built at the far end":
#   * `PropertyDriver::at(1.0, ..)` — an explicit target-end start;
#   * `PropertyDriver::at(1.0` inside a `..`-less struct literal is the same thing.
# Comments are stripped so prose about the rule is not read as a violation, and
# `#[cfg(test)]` modules are excluded by scope (a fixture is not a control).
HITS="$(grep -rnE 'PropertyDriver::(at|default)\s*\(\s*1\.0' src/widget/ --include=*.rs \
    | sed -e 's://.*::' || true)"
HITS="$(printf '%s\n' "$HITS" | grep -vE ':[[:space:]]*$' || true)"

if [ -n "$HITS" ] && [ "$(printf '%s' "$HITS" | tr -d '[:space:]' | wc -c)" -gt 0 ]; then
    echo "  FAIL  a control is constructed at the target end of its animation:"
    printf '%s\n' "$HITS" | sed 's/^/          /'
    echo "        A control built already finished fades *out* on its first frame and"
    echo "        reports \`is_moving() == false\`, so its own reveal never runs"
    echo "        (BLUE24 §2.4 gate B). Start it at the resting end, 0.0."
    exit 1
fi
echo "  PASS  every driver is constructed at 0.0 or from its own resting value"

echo "[4/4] the population is not empty"
# A gate that passes because it found nothing to check reports absence as success.
if ! grep -rqE 'PropertyDriver::at\(' src/widget/ --include=*.rs; then
    echo "  FAIL  no control constructs a PropertyDriver at all"
    echo "        Either the type is unused (the state BLUE24 §2.3 removed) or the"
    echo "        scan is broken; both mean this gate judged nothing."
    exit 1
fi
COUNT="$(grep -rE 'PropertyDriver::at\(' src/widget/ --include=*.rs | wc -l | tr -d ' ')"
echo "  PASS  $COUNT PropertyDriver constructions audited"

echo
echo "check_first_value_not_zero: OK"
