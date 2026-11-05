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
#
# # Why the patterns include `Box::new(...)`
#
# The first version matched the bare type name. A doc comment that mentions `WasmPlatform`
# therefore satisfied the check, and renaming the real construction to `WasmBackend` left the gate
# green — the textual-gate failure principle #106 is about. The pattern is now the construction
# shape, so only an arm that actually returns that backend can satisfy it.

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

require 'Box::new(WindowsPlatform::new())' 'Windows selects WindowsPlatform'
require 'Box::new(SelectedMacOSPlatform::new())' 'macOS selects the cocoa/objc2 bridge'
require 'Box::new(AndroidPlatform::new())' 'Android selects AndroidPlatform'
require 'Box::new(IosMobilePlatform::new())' 'the Apple UIKit family selects IosMobilePlatform'
require 'Box::new(HarmonyPlatform::new())' 'OpenHarmony selects HarmonyPlatform'
require 'Box::new(crate::platform::redox::OrbitalPlatform::new())' 'Redox selects OrbitalPlatform'
require 'Box::new(crate::platform::wasm::WasmPlatform::default())' 'wasm32 selects WasmPlatform'
require 'Box::new(LinuxPlatform::new())' 'unix desktop selects LinuxPlatform'
require 'Box::new(WaylandPlatform::new())' 'Wayland selects WaylandPlatform'

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
if grep -qF 'Box::new(crate::platform::stub::StubPlatform::new(' "$RUNTIME"; then
    pass 'the no-backend fallback is the explicit portable stub'
else
    fail 'no explicit fallback in runtime.rs'
fi

if [ "$FAIL" -ne 0 ]; then
    echo "RESULT: FAIL"
    exit 1
fi
echo "RESULT: PASS"
