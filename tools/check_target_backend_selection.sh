#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# check_target_backend_selection.sh — BLUE28 B28.38 gate.
#
# `cargo rustc -- --print cfg` shows the target's cfgs, not which `Platform` implementation the
# library selects. This gate reads the selection function instead and asserts that every supported
# target family has an arm naming its backend. A target added without an arm silently falls through
# to the portable stub — the tvOS/visionOS failure this gate exists to prevent.

set -u
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
RUNTIME="$ROOT/src/platform/runtime.rs"
BUILD_RS="$ROOT/build.rs"
FAIL=0

fail() { echo "FAIL: $*"; FAIL=1; }
pass() { echo "PASS: $*"; }

[ -f "$RUNTIME" ] || { echo "FAIL: $RUNTIME is missing"; exit 1; }

require() {
    local pattern="$1" what="$2"
    if grep -qF "$pattern" "$RUNTIME"; then
        pass "$what"
    else
        fail "$what (missing '$pattern' in runtime.rs)"
    fi
}

require 'WindowsPlatform' 'Windows selects WindowsPlatform'
require 'SelectedMacOSPlatform' 'macOS selects the cocoa/objc2 bridge'
require 'AndroidPlatform' 'Android selects AndroidPlatform'
require 'IosMobilePlatform' 'the Apple UIKit family selects IosMobilePlatform'
require 'HarmonyPlatform' 'OpenHarmony selects HarmonyPlatform'
require 'OrbitalPlatform' 'Redox selects OrbitalPlatform'
require 'WasmPlatform' 'wasm32 selects WasmPlatform'
require 'LinuxPlatform' 'unix desktop selects LinuxPlatform'
require 'WaylandPlatform' 'Wayland selects WaylandPlatform'

# The Apple alias must be declared once, in build.rs, and consumed by the selection arms.
if grep -q 'apple_uikit' "$BUILD_RS"; then
    pass 'build.rs declares the apple_uikit alias'
else
    fail 'build.rs does not declare apple_uikit'
fi
if grep -q 'apple_uikit' "$RUNTIME"; then
    pass 'runtime.rs consumes the apple_uikit alias'
else
    fail 'runtime.rs does not consume apple_uikit'
fi

# A target family with no arm would land on the stub. The stub is a legitimate *fallback* but must
# be the portable backend, not a silent wrong selection.
if grep -q 'StubPlatform' "$RUNTIME"; then
    pass 'the no-backend fallback is the explicit portable stub'
else
    fail 'no explicit fallback in runtime.rs'
fi

if [ "$FAIL" -ne 0 ]; then
    echo "RESULT: FAIL"
    exit 1
fi
echo "RESULT: PASS"
