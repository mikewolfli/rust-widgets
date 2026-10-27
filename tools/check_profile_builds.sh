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
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

CARGO="${CARGO:-cargo}"

run() {
    local label="$1"
    shift
    echo "── $label"
    if ! "$@"; then
        echo "❌ $label failed: $*" >&2
        exit 1
    fi
}

# The device profile is the reference point: it must stay green first, so a
# failure below is attributable to the reduced profile rather than to the tree.
run "desktop: check" "$CARGO" check --lib
run "desktop: test" "$CARGO" test --lib

for profile in tablet mobile embedded mini; do
    run "$profile: check" \
        "$CARGO" check --lib --no-default-features --features "$profile"
    run "$profile: test" \
        "$CARGO" test --lib --no-default-features --features "$profile"
done

# The second macOS backend (`macos_objc2`) is an *add-on* feature, not a profile, so it is checked
# on top of the desktop profile and only on a host that can run it. It shipped broken for a while
# (`NSAppearanceNameAqua` read outside `unsafe`, and a chart test asserting engine-only geometry),
# which is what this arm exists to catch.
if [ "$(uname -s)" = "Darwin" ]; then
    run "macos (objc2 backend): check" "$CARGO" check --lib --features macos
    run "macos (objc2 backend): test" "$CARGO" test --lib --features macos
fi

echo "✅ every device profile builds and tests: desktop, tablet, mobile, embedded, mini"
