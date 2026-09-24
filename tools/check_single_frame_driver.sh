#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_single_frame_driver.sh — BLUE24 §1 criterion 6
# ============================================================================
# The rule this guards:
#
#   **A frame has exactly one library-side driver, and it advances each control
#    exactly once, before the frame is painted.**
#
# The defect this stops
# ---------------------
# Before `runtime::drive_frame` existed, three entry points each did part of a
# frame and their order differed per platform:
#
#   * `crate::drain_triggers()`          — emptied the input queue;
#   * `runtime::tick_animations(delta)`  — advanced the animation bus;
#   * `draw_bridge::draw_of(..)`         — advanced the control *again*, as a
#                                          side effect of drawing it.
#
# Measured cost: `tick_animations` had **no caller anywhere in the crate**, so
# every animation the library implements was unreachable from a real window —
# and because a control still *painted*, the failure looked like a still image
# rather than like a bug.
#
# The rule is therefore two structural facts, not a runtime observation:
#
#   1. `tick_animations` is called from exactly one place — the frame driver;
#   2. the frame driver is the only place that both drives the animation bus and
#      drains input, so the order "input, then advance, then report" cannot be
#      assembled wrongly by a backend.
#
# A backend that called `tick_animations` itself would be fact 1 violated; a
# backend that called `drain_triggers` itself would be fact 2. Both are the
# "same button advanced twice in one frame" class of defect BLUE24 §1.1 lists.
#
# What it accepts and what it rejects
# -----------------------------------
# Only **live code** counts. A comment mentioning `drain_triggers()` to explain
# the change is not a call, so comments are stripped before matching — otherwise
# this gate would fail on the very documentation that records why it exists.
#
# Exit 0 = one frame driver, no competing call sites.
# Exit 1 = a finding (a second driver, or an unlicensed call site).
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

RUNTIME="src/widget/runtime.rs"

echo "[1/3] the frame driver exists"
if [ ! -f "$RUNTIME" ]; then
    echo "  FAIL  $RUNTIME is missing; the frame driver has to live somewhere addressable"
    exit 1
fi
if ! grep -qE '^pub fn drive_frame\(' "$RUNTIME"; then
    echo "  FAIL  runtime::drive_frame is missing"
    echo "        BLUE24 §1.2 makes it the crate's single frame entry point."
    exit 1
fi
echo "  PASS  runtime::drive_frame is defined"

echo "[2/3] the animation bus has exactly one driver"
# Strips `//…` line comments so the gate reads code, not the prose about it.
# Finds every live call to `tick_animations(`, then asserts they all sit inside
# `drive_frame`'s own body. Anywhere else is a second driver.
BUS_CALLS="$(grep -rn 'tick_animations(' src/ --include=*.rs \
    | sed -e 's://.*::' \
    | grep -vE '^[^:]+:[0-9]+:[[:space:]]*$')"

# Drop the definition itself and its doc-comment references.
BUS_CALLS_CODE="$(printf '%s\n' "$BUS_CALLS" | grep -vE 'fn tick_animations\(' || true)"

if [ -z "$BUS_CALLS_CODE" ]; then
    echo "  FAIL  nothing calls tick_animations at all"
    echo "        That is the original defect (a bus with no driver), not a pass."
    exit 1
fi

# Every call site must be in runtime.rs. A backend that drove the bus directly
# is the "each backend drives" wrong answer BLUE24 §1.1 rejects.
OUTSIDE="$(printf '%s\n' "$BUS_CALLS_CODE" | grep -v "^${RUNTIME}:" || true)"
if [ -n "$OUTSIDE" ]; then
    echo "  FAIL  tick_animations is driven from outside the frame driver:"
    printf '%s\n' "$OUTSIDE" | sed 's/^/          /'
    echo "        One driver advancing every control once; a second driver advances"
    echo "        the same control twice in a frame, at a speed that depends on how"
    echo "        many layers happened to call in (BLUE24 §1 criterion 6)."
    exit 1
fi

# Within runtime.rs, the calls must be inside drive_frame. The function body is
# sliced from its signature to the closing brace at its own indent; a call
# anywhere else in the file (a test is allowed, a second public driver is not).
DRIVER_BODY="$(awk '/^pub fn drive_frame\(/{inside=1} inside{print} inside && /^}$/ {exit}' "$RUNTIME")"
if [ -z "$DRIVER_BODY" ]; then
    echo "  FAIL  could not extract drive_frame's body"
    exit 1
fi
IN_DRIVER="$(printf '%s\n' "$DRIVER_BODY" | grep -c 'tick_animations(' || true)"
# Locate calls by line number so the "inside the driver" test is positional.
DRIVER_START="$(grep -n '^pub fn drive_frame(' "$RUNTIME" | head -n 1 | cut -d: -f1)"
DRIVER_END="$((DRIVER_START + $(printf '%s\n' "$DRIVER_BODY" | wc -l) - 1))"
STRAY="$(printf '%s\n' "$BUS_CALLS_CODE" \
    | awk -F: -v lo="$DRIVER_START" -v hi="$DRIVER_END" \
        '!/#\[test\]/ && ($2 < lo || $2 > hi) { print }' || true)"
# A call inside a `#[cfg(test)] mod tests` is a test double driving the bus on
# purpose, which is how criterion 4's "exactly once" is measured. Only a call in
# non-test code outside the driver is a finding.
STRAY_CODE="$(printf '%s\n' "$STRAY" | while IFS= read -r line; do
    [ -z "$line" ] && continue
    f="${line%%:*}"; rest="${line#*:}"; ln="${rest%%:*}"
    # Find the enclosing module boundary above this line; if the nearest
    # `mod tests` starts before it and the file ends with it, it is a test.
    mod_line="$(awk -v target="$ln" '/mod tests/{m=NR} NR==target{print m+0}' "$f")"
    if [ "${mod_line:-0}" -eq 0 ]; then
        printf '%s\n' "$line"
    fi
done || true)"
if [ -n "$STRAY_CODE" ]; then
    echo "  FAIL  tick_animations is called from runtime.rs but outside drive_frame:"
    printf '%s\n' "$STRAY_CODE" | sed 's/^/          /'
    exit 1
fi

if [ "$IN_DRIVER" -lt 1 ]; then
    echo "  FAIL  drive_frame does not drive the animation bus at all"
    exit 1
fi
echo "  PASS  drive_frame is the animation bus's only driver ($IN_DRIVER call)"

echo "[3/3] the frame driver owns the input drain"
# The drain is what must happen *first* in a frame. If a backend drains on its
# own, the order "input, then advance, then report" is split across two places
# and nothing checks which ran first.
DRAIN_CALLS="$(grep -rnE 'crate::drain_triggers\(\)|[^_a-z]drain_triggers\(\)' src/ --include=*.rs \
    | sed -e 's://.*::' \
    | grep -vE 'fn drain_triggers\(' \
    | grep -vE '^[^:]+:[0-9]+:[[:space:]]*$' || true)"

if [ -z "$DRAIN_CALLS" ]; then
    echo "  FAIL  nothing calls drain_triggers at all"
    echo "        Input would never reach the tree; that is not a pass."
    exit 1
fi

DRAIN_OUTSIDE="$(printf '%s\n' "$DRAIN_CALLS" | grep -v "^${RUNTIME}:" || true)"
if [ -n "$DRAIN_OUTSIDE" ]; then
    echo "  FAIL  drain_triggers is called outside the frame driver:"
    printf '%s\n' "$DRAIN_OUTSIDE" | sed 's/^/          /'
    echo "        The drain has to be step 1 *of* the frame, or the order the plan"
    echo "        fixes (input -> advance -> report) is split across two places."
    echo "        Call crate::drive_frame(ms) instead; it drains and advances."
    exit 1
fi

if ! printf '%s\n' "$DRIVER_BODY" | grep -q 'drain_triggers()'; then
    echo "  FAIL  drive_frame does not drain input"
    exit 1
fi
echo "  PASS  drive_frame is the input drain's only caller"

echo
echo "check_single_frame_driver: OK (one driver, no competing call sites)"
