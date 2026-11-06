#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_profile_builds.sh — every device profile builds and tests
# ============================================================================
# The crate is built in five device profiles, and each removes something the
# others rely on:
#
#   * `desktop` (the default) — the whole surface, with a native shell;
#   * `tablet` / `mobile` — device profiles without a native shell and (for
#     `tablet`) without the `chart` engine, but with the full widget set;
#   * `embedded` — `embedded_surface`: no OS runtime, so the process-default
#     platform is the in-memory stub;
#   * `mini` — `alloc_frugal` + `no_std`: no platform singleton, no
#     `widget::runtime`, and no std prelude (`Vec`/`Box`/`ToString` come from
#     `crate::compat`).
#
# A profile that stops compiling is the failure this gate exists for: the two
# reduced profiles were outside every matrix for a while, and by the time they
# were looked at, `mini` had 13 errors (missing `MiniToString`, an
# `input_companions` module gated on `widgets_unstripped` while its importers
# were not, and a `label` mnemonic that reached for `widget::runtime`).
#
# Each profile is checked **and** tested, because a test target that does not
# compile is the same silent gap one level up: the `mini` test harness had 18
# such errors (bare `Vec`/`Box` in test modules, tests that mount controls), and
# `tablet` had a chart import gated on a feature its own struct did not need.
#
# Usage: tools/check_profile_builds.sh
# Exit 0 = every profile checks and tests clean.
# Exit 1 = at least one profile failed (the failing command is echoed).
# rw-gate-budget: 1800
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_timeout.sh"
. "$ROOT_DIR/tools/lib_cargo_cache.sh"

# # Why every step is bounded and cached
#
# The steps used to run bare. Two consequences, both fixed here:
#
#   * **No bound.** Run through `run_all_gates.sh` this gate was protected by that
#     runner's per-gate timeout, but `bash tools/check_profile_builds.sh` — the way
#     a developer runs it — was unbounded: a `cargo` waiting on another process's
#     target-dir lock would hang forever with no output, exactly the failure mode
#     `tools/lib_timeout.sh` exists to prevent (principle #58).
#   * **Repeated work.** Several of these feature sets are compiled by other gates
#     too, and cargo treats each feature set as a separate compilation unit, so the
#     sweep paid for them more than once. `rw_cargo_cached` makes the second caller
#     reuse the first caller's successful result, keyed on argv + toolchain +
#     environment + source digest.
#
# The `CARGO` override still applies: `lib_cargo_cache.sh` resolves `${CARGO:-cargo}`
# once and drives that binary on both the cached and uncached paths.
PROFILE_BUDGET="${RW_GATE_TIMEOUT:-900}"

run() {
    local label="$1"
    shift
    echo "── $label"
    if ! rw_cargo_cached "$PROFILE_BUDGET" "$@"; then
        echo "❌ $label failed: cargo $*" >&2
        exit 1
    fi
}

# The device profile is the reference point: it must stay green first, so a
# failure below is attributable to the reduced profile rather than to the tree.
run "desktop: check" check --lib
run "desktop: test" test --lib

for profile in tablet mobile embedded mini; do
    run "$profile: check" \
        check --lib --no-default-features --features "$profile"
    run "$profile: test" \
        test --lib --no-default-features --features "$profile"
done

# The second macOS backend (`macos_objc2`) is an *add-on* feature, not a profile, so it is checked
# on top of the desktop profile and only on a host that can run it. It shipped broken for a while
# (`NSAppearanceNameAqua` read outside `unsafe`, and a chart test asserting engine-only geometry),
# which is what this arm exists to catch.
if [ "$(uname -s)" = "Darwin" ]; then
    run "macos (objc2 backend): check" check --lib --features macos
    run "macos (objc2 backend): test" test --lib --features macos
fi

echo "✅ every device profile builds and tests: desktop, tablet, mobile, embedded, mini"
