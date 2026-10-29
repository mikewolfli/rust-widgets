#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# check_platform_probe.sh — BLUE28 B28.01 gate.
#
# The unified probe has three declarations that must stay in step:
#
#   1. the bit table in `src/platform/diagnostics.rs` (the authority),
#   2. the C ABI exports in `include/rw_generated.h`, and
#   3. each host harness that reads the result.
#
# This script is deliberately textual, but it anchors every check to a shape that only the
# production declaration has: the ordered `DIAGNOSTIC_NAMES` array, the `DIAGNOSTIC_BIT_COUNT`
# constant, the generated header's function signatures, and the Android/iOS/desktop call sites.
# A test-only mention cannot satisfy those anchors.

set -u

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DIAG="$ROOT/src/platform/diagnostics.rs"
HEADER="$ROOT/include/rw_generated.h"
EXAMPLE="$ROOT/examples/platform_diagnostics.rs"
FAIL=0

fail() { echo "FAIL: $*"; FAIL=1; }
pass() { echo "PASS: $*"; }

[ -f "$DIAG" ] || { echo "FAIL: $DIAG is missing"; exit 1; }

# ── 1. The stable bit table ────────────────────────────────────────────────
EXPECTED_NAMES=(
    clipboard_mime displays display_of_window display_generation dpi system_theme
    text_scale safe_area ime accessibility file_dialog open_external
)
EXPECTED_COUNT=${#EXPECTED_NAMES[@]}

if grep -q "pub const DIAGNOSTIC_BIT_COUNT: usize = ${EXPECTED_COUNT};" "$DIAG"; then
    pass "DIAGNOSTIC_BIT_COUNT == $EXPECTED_COUNT"
else
    fail "DIAGNOSTIC_BIT_COUNT is not the published $EXPECTED_COUNT"
fi

# Extract the array body and compare the names in order.
MAP=$(sed -n '/pub const DIAGNOSTIC_NAMES/,/];/p' "$DIAG" | grep -o '"[a-z_]*"' | tr -d '"' | tr '\n' ' ')
EXPECTED=$(printf '%s ' "${EXPECTED_NAMES[@]}")
if [ "$MAP" = "$EXPECTED" ]; then
    pass "bit names and order match the published table"
else
    fail "bit names/order drifted: got [$MAP], expected [$EXPECTED]"
fi

# Every published bit must have a row in the module's doc table. The table uses numeric
# rows `| N |`, so a missing bit is visible as a missing number.
for ((i = 0; i < EXPECTED_COUNT; i++)); do
    if ! grep -qE "^//! *\| *${i} *\|" "$DIAG"; then
        fail "bit $i has no row in the module documentation table"
    fi
done
[ "$FAIL" -eq 0 ] && pass "all $EXPECTED_COUNT bits are documented"

# ── 2. C ABI exports ───────────────────────────────────────────────────────
for symbol in rw_platform_diagnostics rw_platform_diagnostics_json; do
    if grep -qE "^[a-z].* ${symbol}\(" "$HEADER"; then
        pass "header exports ${symbol}"
    else
        fail "header is missing ${symbol} (regenerate with tools/generate_c_header.py)"
    fi
done

if grep -q "pub extern \"C\" fn rw_platform_diagnostics()" "$ROOT/src/bindings/binding_impl/c_abi_platform.rs"; then
    pass "Rust defines the C ABI mask export"
else
    fail "Rust C ABI mask export is missing"
fi
if grep -q "pub extern \"C\" fn rw_platform_diagnostics_json()" "$ROOT/src/bindings/binding_impl/c_abi_platform.rs"; then
    pass "Rust defines the C ABI JSON export"
else
    fail "Rust C ABI JSON export is missing"
fi

# ── 3. Host harnesses ──────────────────────────────────────────────────────
if [ -f "$EXAMPLE" ] && grep -q "platform::diagnostics::run" "$EXAMPLE"; then
    pass "desktop harness runs the shared probe"
else
    fail "examples/platform_diagnostics.rs does not run the shared probe"
fi

if grep -q "crate::platform::diagnostics::run" "$ROOT/src/platform/android_jni.rs"; then
    pass "Android probe delegates to the shared probe"
else
    fail "Android nativePlatformProbe still has its own bit logic"
fi

if grep -q "rw_platform_diagnostics" "$ROOT/bindings/ios/main.m"; then
    pass "iOS host declares the C ABI probe"
else
    fail "iOS host does not declare rw_platform_diagnostics"
fi

if grep -q "diagnostics::run" "$ROOT/examples/platform_diagnostics.rs"; then
    pass "a runnable desktop harness exists"
else
    fail "no runnable desktop harness"
fi

# ── 4. The probe must not be a capability-flag summary ────────────────────
# A fact is claimed only after the real method was called. Anchor to calls, not to names.
for call in \
    "platform.displays()" \
    "platform.display_of_window(" \
    "platform.display_generation()" \
    "platform.dpi_scale_factor()" \
    "platform.system_theme()" \
    "platform.text_scale()" \
    "platform.safe_area_insets(" \
    "platform.ime_bridge()" \
    "platform.accessibility_bridge()" \
    "platform.clipboard_backend()"; do
    if grep -qF "$call" "$DIAG"; then
        pass "probe calls $call"
    else
        fail "probe does not call $call"
    fi
done

if [ "$FAIL" -ne 0 ]; then
    echo "RESULT: FAIL"
    exit 1
fi
echo "RESULT: PASS"
