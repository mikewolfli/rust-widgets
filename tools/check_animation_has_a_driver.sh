#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_animation_has_a_driver.sh -- BLUE23 §3.3 (gate of 6)
# ============================================================================
# The rule this guards (BLUE23 §3.3, judgement 4):
#
#   **Every control that implements an animation `tick` must be drivable by the bus.**
#
# # Why this is a gate and not a test
#
# Eleven controls implemented `pub fn tick(delta_ms) -> bool` and **not one production
# caller existed** anywhere in `src/app`, `src/render` or `src/platform`. From the source
# each control read as animated; on screen the hover faded nowhere and the caret never
# blinked. Every unit test passed, because a test calls the method directly -- a test
# proves a method works, not that anything drives it.
#
# The fix is one bus (`widget::runtime::tick_animations`) that reaches controls through the
# `Widget` trait. This gate is what keeps the door shut: a control that adds an animation
# `tick` but no trait bridge is an island again, and the scan says so by name.
#
# # What this gate proves
#
# A lexical, whole-tree, no-build scan of `src/widget/**` (excluding the bus itself). Any
# file declaring `fn tick(&mut self, <delta>: ...)` must also contain the trait bridge
# `fn tick(&mut self, delta_ms: u32) -> bool`.
#
# # What this gate does NOT prove
#
#   * It is **lexical**: it matches the shape, not the call graph. A control could bridge
#     `tick` and then lie about `is_animating()`. The runtime tests
#     (`widget::runtime::tests::the_bus_*`) cover the behavioural half.
#   * `tick_count` / `tick_label_height` (a count and a label height) are not animations;
#     they are excluded by requiring a `delta`-named parameter.
#
# # Reverse injection
#
# Step 2 appends an undispatched animation `tick` to a named widget and requires the scan
# to report it, so a search that matched nothing cannot pass.
#
# Usage: tools/check_animation_has_a_driver.sh
# Exit 0 = every animation tick is reachable from the bus.
# Exit 1 = an island (each offender is named by file).
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_python.sh"

if ! "$PYTHON" tools/animation_driver_scan.py; then
    echo ""
    echo "  The bus is the ONLY frame driver (BLUE23 §3.3): a per-control timer would not"
    echo "  be frame-aligned, and driving from each backend would advance a control twice a"
    echo "  frame. Bridge the control instead."
    exit 1
fi

if "$PYTHON" tools/animation_driver_scan.py --inject=src/widget/display_widgets/skeleton_loader.rs >/dev/null 2>&1; then
    echo "FAIL: injecting an undispatched tick did not change the result, so the check is"
    echo "      not comparing against the widget tree"
    exit 1
fi

echo "animation-driver checks passed."
