#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# check_profile_target_matrix.sh — BLUE28 B28.37.
#
# `default = ["desktop"]` means `cargo check --features embedded` mixes two mutually exclusive
# profiles and reports a configuration no host can run (principle #48). This gate always passes
# `--no-default-features`, and it records each cell's expectation instead of treating an
# intentionally unsupported combination as a failure.
#
# Usage:
#   bash tools/check_profile_target_matrix.sh            # host target, all five profiles
#   bash tools/check_profile_target_matrix.sh --cross    # plus installed cross targets
#
# rw-gate-budget: 2400

set -u
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
. "$ROOT/tools/lib_timeout.sh"
cd "$ROOT" || exit 1

HOST_TARGET="$(rustc -vV | sed -n 's/^host: //p')"
PROFILES=(desktop tablet mobile mini embedded)
FAIL=0
RUN=0
SKIP=0

run_cell() {
    local label="$1" target="$2" features="$3" expectation="$4"
    if ! rustup target list --installed | grep -qx "$target"; then
        echo "SKIP $label: target $target is not installed"
        SKIP=$((SKIP + 1))
        return
    fi
    RUN=$((RUN + 1))
    echo "RUN  $label ($expectation)"
    if rw_run_bounded 900 cargo check --target "$target" --no-default-features --features "$features"; then
        echo "PASS $label"
    else
        echo "FAIL $label (exit $?)"
        FAIL=$((FAIL + 1))
    fi
}

for profile in "${PROFILES[@]}"; do
    run_cell "$profile@$HOST_TARGET" "$HOST_TARGET" "$profile" "compiles"
done

if [ "${1:-}" = "--cross" ]; then
    # Representative cross cells. Each one is a target the plan names; a missing target is a
    # host limitation, reported as SKIP rather than a failure (principle #103).
    run_cell "desktop@windows" "x86_64-pc-windows-msvc" "windows,controls-custom,controls-native" "compiles (no audio C dep)"
    # Upstream feedback `feedback-20261029.md` §2: the sized profiles use the **core** prelude, which
    # has no `ToOwned`; a `&'static str` that leaned on std's prelude compiled everywhere except a
    # Windows target, because no gate exercised `embedded` on Windows. Both sized profiles are cells
    # here so the class, not the one line, is covered.
    run_cell "embedded@windows" "x86_64-pc-windows-msvc" "embedded" "compiles the sized profile on Windows (core prelude)"
    run_cell "mini@windows" "x86_64-pc-windows-msvc" "mini" "compiles the smallest profile on Windows (core prelude)"
    run_cell "desktop@linux-gnu" "x86_64-unknown-linux-gnu" "wayland-native" "compiles the Wayland path"
    run_cell "ios@ios" "aarch64-apple-ios" "ios-uikit-ffi,mobile,controls-custom,controls-native,serde,serde_json" "compiles the UIKit path"
    run_cell "android@android" "x86_64-linux-android" "android-jni,jni,mobile,controls-custom,controls-native,serde,serde_json" "compiles the JNI path"
    run_cell "harmony@ohos" "aarch64-unknown-linux-ohos" "harmony" "compiles the OhOS path (link-level)"
    run_cell "wasm@wasm" "wasm32-unknown-unknown" "wasm" "compiles the browser path"
fi

echo "matrix: ran $RUN cell(s), skipped $SKIP"
if [ "$FAIL" -ne 0 ]; then
    echo "RESULT: FAIL ($FAIL cell(s))"
    exit 1
fi
echo "RESULT: PASS"
