#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# run_present_macos.sh — the real-window presentation probe (BLUE29 §7 / B29.13)
# ============================================================================
# Runs `examples/present_smoke` on **both** macOS canvas backends. The probe presents a known
# frame into a real AppKit window, reads the presented pixels back and asserts byte equality, then
# exercises the damaged-band path, a resize, 300 steady frames, the per-window capability question,
# and — through `WindowHandle::set_frame_presentation` — whether the canvas itself consumes the
# intent.
#
# The probe's own exit codes are the verdict:
#   0 = every invariant held                     -> PASS
#   1 = an invariant did not hold                -> FAIL
#   3 = this host/window cannot present          -> SKIP(host-limited)
#
# The canvas half is reported separately by the probe: a backend whose canvas does not consume the
# intent yet prints `SKIP(wiring)` and still exits 0 when the presenter half passed. That is the
# distinction the plan asks for — a missing wiring is a named absence, not a failure of the target.
#
# # Why this is a script and not a unit test
#
# The window is AppKit's, and `cargo test` runs tests on worker threads while AppKit is
# main-thread-only. The probe therefore runs as a binary with a real run loop and drives the same
# public API a host would.
#
# # Bounds
#
# Every cargo invocation goes through `rw_run_bounded` (principle #58) and the budget is declared
# for the runner (principle #104). No second, looser `timeout` is wrapped around them.
#
# Exit status: 0 when both backends were not FAIL, 1 on a failure, 2 when this host cannot run it.
# ============================================================================

set -euo pipefail

# rw-gate-budget: 2400

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

# shellcheck source=tools/lib_timeout.sh
. "$ROOT_DIR/tools/lib_timeout.sh"

if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "run_present_macos: unsupported host '$(uname -s)' (a real AppKit session is required)" >&2
    exit 2
fi

BUDGET="${RW_GATE_TIMEOUT:-2400}"
FEATURES="desktop,gpu-wgpu,raw-window-handle"
FEATURES_OBJC2="desktop,macos,gpu-wgpu,raw-window-handle"
STATUS=0

# A display that is asleep makes the window server report every window as occluded, and no frame
# can be acquired. Waking it is a *host* action, not a library one: `caffeinate -u` asserts user
# activity for a bounded window. The window is 180 s because the probe deliberately idles for 60 s
# and a display that sleeps during that wait makes the *next* acquire legitimately `occluded`. Best effort — when it is unavailable, or the window stays
# occluded, the probe reports `SKIP(host-limited)` with that exact reason instead of a failure.
wake_display() {
    if command -v caffeinate >/dev/null 2>&1; then
        caffeinate -u -t 180 || true
    fi
}

run_probe() {
    local label="$1"
    local features="$2"
    local status
    echo "--- $label"
    set +e
    rw_run_bounded "$BUDGET" cargo run -q --no-default-features --features "$features" \
        --example present_smoke
    status=$?
    set -e
    case "$status" in
        0) echo "    $label: PASS" ;;
        3) echo "    $label: SKIP(host-limited)" ;;
        *)
            echo "    $label: FAIL (exit $status)" >&2
            STATUS=1
            ;;
    esac
}

echo "[1/3] building the presentation probe (cocoa canvas)"
rw_run_bounded "$BUDGET" cargo build -q --no-default-features --features "$FEATURES" \
    --example present_smoke

echo "[2/3] cocoa canvas: presenter + pixel equality + wired intent"
wake_display
run_probe "cocoa" "$FEATURES"

echo "[3/3] objc2 canvas: presenter + pixel equality (wiring reported by the probe)"
rw_run_bounded "$BUDGET" cargo build -q --no-default-features --features "$FEATURES_OBJC2" \
    --example present_smoke
wake_display
run_probe "objc2" "$FEATURES_OBJC2"

if [[ "$STATUS" -ne 0 ]]; then
    echo "run_present_macos: FAIL" >&2
    exit 1
fi
echo "run_present_macos: PASS (failures: none; per-backend skips are named above)"
exit 0
