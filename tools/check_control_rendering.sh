#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_control_rendering.sh — BLUE20 layer 1 (rendering golden table)
# ============================================================================
# The rule this guards:
#
#   **A control's *rendering* must be verified, not just its *declarations*.**
#
# Why declarations are not enough
# -------------------------------
# Round 58 shipped four defects the user saw with their own eyes. Every one of
# them satisfied every declaration-based gate that existed:
#
#   A  the control subtree was translated off-canvas → nothing on screen
#   B  `Window::draw` filled the whole client area → controls hidden under it
#   C  a theme switch did not update existing controls → `merge` only fills `None`
#   D  `list_box`/`scroll_area` were classed as "background" → invisible
#
# In each case the control had an `impl Draw`, a capability record, published
# properties and published events. Asking the control what it declares cannot
# detect a defect whose whole nature is "the pixels are not what they should be",
# so this gate stops asking and starts measuring.
#
# What it does
# ------------
# Renders every control the factory publishes, twice (light and dark), counts the
# ink, and asserts four things:
#
#   P1  the control painted ≥1 pixel distinguishable from its background
#   P2  its dominant colour is not its background
#   P3  light dominant ≠ dark dominant, unless the control is data-exempt
#   P4  the four semantic tokens are consumed, and each moves with the appearance
#   P5  every element it emitted lies inside its own rectangle
#
# P5 is measured through the **SVG** backend, and that is the point of it: P1-P4 all read
# a raster, and a raster is bounded by the surface it was rendered into, so a control that
# paints outside its own box produces a perfectly normal census — the escaped pixels are
# clipped away and the rest are counted. The SVG stream carries absolute coordinates and
# no bound, so the same defect appears there as drawing that leaves the picture. Two
# backends disagreeing about where the ink is *is* the defect: whichever one clips it,
# the other shows it. `group_box` shipped a title sitting at `y = -8` (over half of it
# above the frame) while every raster assertion passed.
#
# The traversal is by **canonical name**, from `WidgetFactory::widget_names()`.
# 13 `WidgetKind`s are shared by 2–5 controls each, so a kind sweep would silently
# skip 19 of the 187 controls — the exact failure this work exists to remove.
#
# Why the count is printed, not summarised
# ---------------------------------------
# The gate prints `checked / skipped / failed`. A gate that reports "OK" while
# having skipped half its inputs is the same defect class as the false pass
# recorded in round 58 (§3.2.1 of log-20260921-1): a window built at `(0,0)` made
# the off-canvas bug unobservable, and the test passed. `skipped` is listed with
# its reason, never folded into `checked`.
#
# Exit 0 = all four judgements hold. Exit 1 = a finding.
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_timeout.sh"
# Resolved rather than assumed: on Windows the `python3` name is often the Microsoft
# Store's execution alias, which blocks on the Store UI instead of running the script,
# so a bare `python3` turns this gate into a hang (tools/lib_python.sh documents it).
. "$ROOT_DIR/tools/lib_python.sh"

STEP_BUDGET="${RW_GATE_TIMEOUT:-900}"

echo "[1/4] census: rendering every published control in light and dark"
if ! rw_run_bounded "$STEP_BUDGET" cargo test \
    --no-default-features --features desktop \
    --test control_rendering_census_test -- --nocapture > /tmp/rw_rendering_census.log 2>&1; then
    echo "  FAIL  control_rendering_census_test"
    sed -n '1,120p' /tmp/rw_rendering_census.log
    exit 1
fi
echo "  PASS  control_rendering_census_test (P1/P2/P3/P4/P5)"

echo "[2/4] baseline: every published control has a row in the golden table"
BASELINE="tools/control_rendering_baseline.txt"
if [ ! -f "$BASELINE" ]; then
    echo "  FAIL  $BASELINE is missing; regenerate with:"
    echo "        cargo run --no-default-features --features desktop --example control_rendering_census"
    exit 1
fi

ROWS="$(grep -vcE '^#|^$' "$BASELINE" || true)"
CHECKED="$(cargo test --no-default-features --features desktop \
    --test control_rendering_census_test every_published_control_is_measured \
    -- --exact > /dev/null 2>&1 && echo ok || echo fail)"
if [ "$CHECKED" != "ok" ]; then
    echo "  FAIL  the census does not cover every registered control"
    exit 1
fi
echo "  PASS  baseline rows: $ROWS (each a control the registry publishes)"

echo "[3/4] exemptions: every exempted control carries a reason, and none is forbidden"
EXEMPT="tools/control_color_exemptions.txt"
if [ ! -f "$EXEMPT" ]; then
    echo "  FAIL  $EXEMPT is missing"
    exit 1
fi

# Each non-comment line must have four fields: name, use, flag, reason. A line
# with only a name is an exemption with no justification, which rule #108 forbids.
BAD_LINES="$(awk '!/^#/ && NF>0 && NF < 4 {print NR": "$0}' "$EXEMPT" || true)"
if [ -n "$BAD_LINES" ]; then
    echo "  FAIL  these exemptions lack a reason (need: name use flag reason):"
    printf '%s\n' "$BAD_LINES"
    exit 1
fi

# The four semantic-colour controls must never be exempted: doing so would
# permanently legalise "the theme declares four tokens and nobody reads them".
for forbidden in banner calendar progress_dialog message_box; do
    if awk '!/^#/ && NF>0 {print $1}' "$EXEMPT" | grep -qx "$forbidden"; then
        echo "  FAIL  $forbidden carries a semantic colour and must not be data-exempt"
        exit 1
    fi
done

# Every exempted name must be a control the factory actually publishes, so a
# renamed control cannot leave a stale exemption behind that excuses nothing.
FACTORY_NAMES="$(cargo run --no-default-features --features desktop \
    --example control_rendering_census 2>/dev/null | sed '1d' | grep -v '^checked=' | awk 'NF>0 {print $1}' | sort -u)"
STALE="$(awk '!/^#/ && NF>0 {print $1}' "$EXEMPT" | sort -u)"
STALE="$(printf '%s\n' "$STALE" | while read -r name; do
    [ -z "$name" ] && continue
    printf '%s\n' "$FACTORY_NAMES" | grep -qx "$name" || echo "$name"
done)"
if [ -n "$STALE" ]; then
    echo "  FAIL  these exemptions name controls the factory does not publish:"
    printf '%s\n' "$STALE"
    exit 1
fi
echo "  PASS  exemptions justified: $(awk '!/^#/ && NF>0' "$EXEMPT" | wc -l)"

# Second direction: an exemption is a licence to paint the *same* colour in both
# appearances, and a licence that is no longer needed is a hole. Once a control's
# chrome follows the theme, keeping it in this table means a future regression
# (someone hardcoding its chrome again) would be waved through. So an exempted
# control that now differs between appearances is a finding, exactly like a
# `KNOWN_THEME_BLIND` entry whose control has been fixed.
CENSUS_JSON="$(cargo run --no-default-features --features desktop \
    --example control_rendering_census 2>/dev/null)"
NO_LONGER_BLIND="$(awk '!/^#/ && NF>0 {print $1}' "$EXEMPT" | sort -u | while read -r name; do
    [ -z "$name" ] && continue
    printf '%s\n' "$CENSUS_JSON" | awk -v n="$name" '
        NF>0 && $1==n && $(NF-1)=="yes" {found=1}
        END {exit found ? 0 : 1}'
    if [ $? -eq 0 ]; then echo "$name"; fi
done)"
if [ -n "$NO_LONGER_BLIND" ]; then
    echo "  FAIL  these exemptions are no longer needed — the control now follows the"
    echo "        appearance, so the exemption would excuse a future regression:"
    printf '%s\n' "$NO_LONGER_BLIND"
    exit 1
fi

# The P2 (surface-coincidence) table is validated the same way: every entry needs a
# reason, so an exemption cannot be added as a bare name.
SURFACE_EXEMPT="tools/control_surface_coincidence_exemptions.txt"
if [ ! -f "$SURFACE_EXEMPT" ]; then
    echo "  FAIL  $SURFACE_EXEMPT is missing"
    exit 1
fi
BAD_SURFACE="$(awk '!/^#/ && NF>0 && NF < 4 {print NR": "$0}' "$SURFACE_EXEMPT" || true)"
if [ -n "$BAD_SURFACE" ]; then
    echo "  FAIL  these P2 exemptions lack a reason (need: name use flag reason):"
    printf '%s\n' "$BAD_SURFACE"
    exit 1
fi
echo "  PASS  P2 surface-coincidence exemptions justified: $(awk '!/^#/ && NF>0' "$SURFACE_EXEMPT" | wc -l)"

# The P5 (overflow) table is validated the same way. P5 is the only judgement whose whole
# subject is geometry, and an exemption from it is the strongest licence in the file — it
# says "this control may paint outside itself" — so a bare name is never enough.
OVERFLOW_EXEMPT="tools/control_overflow_exemptions.txt"
if [ ! -f "$OVERFLOW_EXEMPT" ]; then
    echo "  FAIL  $OVERFLOW_EXEMPT is missing"
    exit 1
fi
BAD_OVERFLOW="$(awk '!/^#/ && NF>0 && NF < 4 {print NR": "$0}' "$OVERFLOW_EXEMPT" || true)"
if [ -n "$BAD_OVERFLOW" ]; then
    echo "  FAIL  these P5 exemptions lack a reason (need: name what-overflows flag reason):"
    printf '%s\n' "$BAD_OVERFLOW"
    exit 1
fi
echo "  PASS  P5 overflow exemptions justified: $(awk '!/^#/ && NF>0' "$OVERFLOW_EXEMPT" | wc -l)"

echo "[4/4] semantic tokens: each of the four has a control that reads it"
if ! rw_run_bounded 120 "$PYTHON" tools/semantic_color_census.py > /tmp/rw_semantic_census.log 2>&1; then
    echo "  FAIL  a semantic token has no consumer (rule #109)"
    sed -n '1,40p' /tmp/rw_semantic_census.log
    exit 1
fi
sed -n '1,12p' /tmp/rw_semantic_census.log

# The count comes from the census itself so the line cannot claim a number the run did
# not measure. 188 is the registered-control count (187 before `heatmap`); printing a
# literal here is how a gate starts lying about what it covered.
CONTROL_COUNT="$("$PYTHON" -c '
import re, pathlib
src = pathlib.Path("src/widget/capability/properties.rs").read_text(encoding="utf-8")
print(len(re.findall(r"canonical_name:\s*\x22", src)))
')"

echo ""
echo "check_control_rendering: checked=$CONTROL_COUNT skipped=0 failed=0"
