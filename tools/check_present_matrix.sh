#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_present_matrix.sh — which platforms can actually present, and which cannot
# ============================================================================
# The gate behind BLUE29 §7.5. It probes each platform *before* attempting anything and prints one
# verdict per platform:
#
#   RUN                     the platform can run the presentation probe on this host
#   SKIP(host-limited)      the platform is not runnable here; the reason names what is missing
#   FAIL                    the probe ran and an invariant did not hold
#
# Naming the reason is the point (principle #103): "Android: no adb device" is a fact a reader can
# act on, while a timeout after 15 minutes is indistinguishable from a broken library.
#
# # What runs today, on this host
#
#   macOS      RUN   — a real AppKit window; `tools/run_present_macos.sh` does the work
#   Android    probe `adb devices`; an emulator counts, a missing SDK does not
#   OpenHarmony probe `hdc list targets`; an empty list is the named exclusion
#   Windows    SKIP(host-limited) — cross-compiled only; no Windows/Wine GPU surface here
#   Linux/wasm/iOS  SKIP(host-limited) — named exclusions in the plan (§7.3)
#
# Exit status: 0 when nothing FAILed (skips are reported, never counted as passes), 1 on a FAIL,
# 2 when the host cannot run any of it.
#
# ============================================================================

set -euo pipefail

# rw-gate-budget: 2400

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

# shellcheck source=tools/lib_timeout.sh
. "$ROOT_DIR/tools/lib_timeout.sh"

BUDGET="${RW_GATE_TIMEOUT:-2400}"
FAILURES=0

report() { printf '  %-30s %s\n' "$1" "$2"; }

echo "--- [1/4] macOS (real window) ---"
if [[ "$(uname -s)" == "Darwin" ]]; then
    set +e
    rw_run_bounded "$BUDGET" bash tools/run_present_macos.sh
    STATUS=$?
    set -e
    case "$STATUS" in
        0) report "macOS" "RUN  PASS" ;;
        3) report "macOS" "SKIP(host-limited) no presentable window on this session" ;;
        *)
            report "macOS" "RUN  FAIL (exit $STATUS)"
            FAILURES=$((FAILURES + 1))
            ;;
    esac
else
    report "macOS" "SKIP(host-limited) this host is $(uname -s)"
fi

echo "--- [2/4] Android (arm64 emulator) ---"
ADB="$HOME/Library/Android/sdk/platform-tools/adb"
if [[ -x "$ADB" ]]; then
    DEVICES="$("$ADB" devices 2>/dev/null | awk 'NR>1 && $2=="device" {count++} END {print count+0}')"
    if [[ "$DEVICES" -gt 0 ]]; then
        report "Android" "RUN  available device(s): $DEVICES"
    else
        report "Android" "SKIP(host-limited) adb is installed but no device/emulator is attached"
    fi
else
    report "Android" "SKIP(host-limited) adb not found at $ADB"
fi

echo "--- [3/4] OpenHarmony (XComponent) ---"
HDC="$HOME/Library/OpenHarmony/Sdk/18/toolchains/hdc"
if [[ -x "$HDC" ]]; then
    TARGETS="$(rw_run_bounded 60 "$HDC" list targets 2>/dev/null | tail -n +2 | tr -d '\r' | grep -c . || true)"
    if [[ "${TARGETS:-0}" -gt 0 ]]; then
        report "OpenHarmony" "RUN  available target(s): $TARGETS"
    else
        report "OpenHarmony" "SKIP(host-limited) hdc reports no target"
    fi
else
    report "OpenHarmony" "SKIP(host-limited) hdc not found at $HDC"
fi

echo "--- [4/4] Windows and the named exclusions ---"
report "Windows" "SKIP(host-limited) cross-compiled only; no Windows/Wine GPU surface on this host"
report "Linux/Wayland" "SKIP(host-limited) named exclusion (plan §7.3): needs rwh + surface wiring"
report "wasm / iOS" "SKIP(host-limited) named exclusions (plan §7.3)"

echo
if [[ "$FAILURES" -gt 0 ]]; then
    echo "check_present_matrix: FAIL ($FAILURES platform(s))"
    exit 1
fi
echo "check_present_matrix: OK (no failures; skips are host limitations, not passes)"
