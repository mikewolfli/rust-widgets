#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# check_native_control_paths.sh — BLUE28 B28.29 gate.
#
# The 50 `Platform::create_*` methods are a historical/native-mode contract, not the production
# path. Production goes through `CustomPaintControlBackend` (`control_backend/dispatcher.rs`), so
# a call from `lib.rs`/`app`/`widget` to the platform's `create_*` would be a second creation
# mechanism (principle #101). This gate keeps that boundary true.

set -u
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
FAIL=0
fail() { echo "FAIL: $*"; FAIL=1; }
pass() { echo "PASS: $*"; }

# Production files that must never reach the platform's native control constructors.
FILES=(
    "src/lib.rs"
    "src/app/handle.rs"
    "src/widget/runtime.rs"
)

for file in "${FILES[@]}"; do
    if [ ! -f "$ROOT/$file" ]; then
        continue
    fi
    # `get_platform().create_*` is the shape this gate forbids. A bare `create_button` call is
    # fine: it goes through the control backend. `create_window` is the required lifecycle method,
    # not a native control constructor, so it is the one allowed `create_*`.
    if grep -nE 'get_platform\(\)\.create_[a-z_]+' "$ROOT/$file" | grep -v 'create_window'; then
        fail "$file calls a Platform::create_* directly"
    else
        pass "$file uses the control-backend path"
    fi
done

# The documented native-mode implementation is allowed exactly one home.
if [ -f "$ROOT/src/control_backend/native.rs" ]; then
    pass "control_backend/native.rs remains the native-mode home"
else
    fail "control_backend/native.rs is missing (native-mode contract has no home)"
fi

if [ "$FAIL" -ne 0 ]; then
    echo "RESULT: FAIL"
    exit 1
fi
echo "RESULT: PASS"
