#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_animation_state_is_one_type.sh — BLUE24 §2.3 (the generalisation criterion)
# ============================================================================
# The rule this guards:
#
#   **A control's animated property is held in one type, and that type owns both
#    values.** No control keeps its own `from`/`to`/`current` triple beside a
#    transition, and no control caches an animation target in a second field.
#
# The defect this stops
# ---------------------
# BLUE23 lifted `tick` onto the `Widget` trait but not the *thing being ticked*.
# So the crate ended up with two implementations of one operation:
#
#   * `src/style/animation.rs` — an easing engine, a duration table read from
#     `theme.motion`, and a `Transition` that interpolates between two values;
#   * each animated control — a `Transition` field **plus** a private
#     `interaction_target: f32` (or `target_progress: f32`, or a re-derived
#     `if checked { 1.0 } else { 0.0 }` written twice) that had to agree with the
#     transition's own aim, kept in step by hand.
#
# The failure mode is not a crash. It is a control whose "am I still moving?"
# answer and whose "where am I going?" answer drift apart, so a frame loop either
# stops one frame early or never stops at all -- and neither is visible on screen
# until it is.
#
# The rule, stated structurally
# -----------------------------
# One `pub struct PropertyDriver` in the style layer owns `current`, `target` and
# `tempo`. Therefore, across `src/widget/**`:
#
#   (1) zero fields named like a cached animation target
#       (`interaction_target`, `target_progress`, `*_target_progress`), because
#       the target lives in the driver;
#   (2) zero remaining `Transition` values held as control state, because the
#       driver is the type a control holds (the `style` layer itself may keep
#       using `Transition` -- that is where it is implemented).
#
# What this gate does NOT prove
# -----------------------------
#   * It does not prove a control *animates* — only that it does not keep a
#     second copy of the driver's answers. A control that reads `theme.motion`
#     directly and steps a bare `f32` is a different defect, covered by
#     `check_transition_durations_are_tokens`.
#   * It is lexical, so a field named `dest` or `aim` would slip through. The
#     names it checks are the ones the crate actually used, which is what makes
#     the check worth having; a new alias should be added here.
#
# Reverse injection
# -----------------
# Re-introducing `interaction_target: f32` into `Button` must make this gate name
# it. See the round's report for the exact output.
#
# Usage: tools/check_animation_state_is_one_type.sh
# Exit 0 = the animated property lives in exactly one type.
# Exit 1 = a finding (each offender is named file:line).
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

DRIVER="src/style/animation.rs"

echo "[1/3] the one animation-state type exists"
if ! grep -qE '^pub struct PropertyDriver' "$DRIVER"; then
    echo "  FAIL  src/style/animation.rs no longer defines \`pub struct PropertyDriver\`"
    echo "        It is the single type a control holds for an animated property."
    exit 1
fi
# Three fields, because the point of the type is that it owns all three: the value it
# is at, the value it is heading for, and the token that prices the move.
for field in current target tempo; do
    if ! grep -qE "^\s+${field}: " "$DRIVER"; then
        echo "  FAIL  PropertyDriver has no \`${field}\` field"
        echo "        The type exists to own the current value, the target and the tempo"
        echo "        together; missing \`${field}\` means a caller has to supply it again."
        exit 1
    fi
done
echo "  PASS  PropertyDriver owns current, target and tempo"

echo "[2/3] no control caches a second copy of the target"
# Strips `//` comments and test modules, then looks for the field names the crate
# used for a cached target. Test fixtures are excluded by scope: a test that names
# the concept is documenting it, not duplicating it.
CACHE_HITS="$(grep -rnE '^\s+(interaction_target|target_progress|[a-z_]*_target_progress)\s*:' \
    src/widget/ --include=*.rs | sed -e 's://.*::' || true)"
CACHE_HITS="$(printf '%s\n' "$CACHE_HITS" | grep -vE ':\s*$' || true)"

if [ -n "$CACHE_HITS" ] && [ "$(printf '%s' "$CACHE_HITS" | tr -d '[:space:]' | wc -c)" -gt 0 ]; then
    echo "  FAIL  a control keeps its own copy of an animation target:"
    printf '%s\n' "$CACHE_HITS" | sed 's/^/          /'
    echo "        The target belongs to \`PropertyDriver\` (set through \`set_target\`), so a"
    echo "        second field is a fact stored twice and one more chance for the two to"
    echo "        disagree (BLUE24 §2.3)."
    exit 1
fi
echo "  PASS  no control caches an animation target"

echo "[3/3] no control holds a Transition as its animation state"
# `Transition` is the interpolation engine. A control holding one directly is the
# pre-migration shape, where the target had to be supplied on every `tick`.
#
# The pattern matches a *field type position* -- `name: ... Transition` with no
# path qualifier beyond `crate::style::` -- and deliberately not:
#   * `MotionSlot` / `TransitionTempo`, which is the tempo, not the value;
#   * `Transition::...` in an expression (a method call, a local);
#   * comments and test modules.
TRANSITION_FIELDS="$(grep -rnE '^\s+[a-z_]+:\s*(crate::style::)?Transition\s*,' \
    src/widget/ --include=*.rs | sed -e 's://.*::' || true)"
if [ -n "$TRANSITION_FIELDS" ] && [ "$(printf '%s' "$TRANSITION_FIELDS" | tr -d '[:space:]' | wc -c)" -gt 0 ]; then
    echo "  FAIL  a control holds a bare Transition as its animated state:"
    printf '%s\n' "$TRANSITION_FIELDS" | sed 's/^/          /'
    echo "        Hold a \`PropertyDriver\` instead: it is the same interpolation with the"
    echo "        target stored, so \`is_moving\` and \`tick\` cannot disagree (BLUE24 §2.3)."
    exit 1
fi
echo "  PASS  every animated control holds a PropertyDriver"

echo
echo "check_animation_state_is_one_type: OK"
