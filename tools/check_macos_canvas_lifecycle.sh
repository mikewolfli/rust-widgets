#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_macos_canvas_lifecycle.sh — PF-X03 / PF-X06 evidence on a real AppKit canvas
# ============================================================================
# The rules this guards:
#
#   * PF-X03 — mounting a surface and unmounting it must leave **nothing** behind: the backend
#     stops answering for the id (a second unmount is refused), and the next mount of the same id
#     still works. That per-id record was the half of the defect that leaked one bookkeeping entry
#     per mount, forever.
#   * PF-X06 — a same-size repaint must reuse the cached `CGBitmapContext`, a size change must
#     rebuild it, and the blit must premultiply translucent pixels while copying opaque rows
#     verbatim.
#
# # Why this is a gate and not a unit test
#
# The lifecycle half needs a real AppKit window, and `cargo test` runs its tests on worker threads
# while AppKit is main-thread-only. The cache half *is* a unit test (no window needed); both are
# driven from here so one command answers "is the macOS canvas still correct".
#
# # Why resident memory is not asserted
#
# Measured on macOS 15 with this crate's bookkeeping entirely disabled (no association, no redraw
# note, no platform state, no handle table): a bare `alloc`/`addSubview:`/`removeFromSuperview`/
# `release` cycle over the same window still grows peak RSS by ~4 KB per cycle. So an RSS
# assertion would fail a *correct* build (principle #103's "a host limitation must be reported,
# not turned into a false verdict"). The probe prints the number as `note …`; the gate asserts the
# object-level invariants instead.
#
# # Host gating
#
# macOS only. Off it this exits 2 with `unsupported host (…)`, so a Linux CI run cannot look like
# Apple coverage.
#
# Exit 0 = every invariant held. Exit 1 = a finding. Exit 2 = unsupported host.
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

# shellcheck source=tools/lib_timeout.sh
. "$ROOT_DIR/tools/lib_timeout.sh"

if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "check_macos_canvas_lifecycle: unsupported host '$(uname -s)' (a real AppKit session is required)" >&2
    exit 2
fi

# Three bounded cargo invocations: a lib-test build plus two example builds+runs. 900 s is the
# floor the runner uses; a cold tree is still well inside it.
BUDGET="${RW_GATE_TIMEOUT:-900}"

echo "[1/3] bitmap-context reuse, resize rebuild and blit arithmetic (no window needed)"
rw_run_bounded "$BUDGET" cargo test -q --lib --features macos -- canvas_bitmap_cache_tests

echo "[2/3] cocoa-legacy surface lifecycle (2000 distinct + 200 reused ids)"
rw_run_bounded "$BUDGET" cargo run -q --no-default-features --features desktop \
    --example macos_canvas_lifecycle_probe

echo "[3/3] objc2 surface lifecycle (2000 distinct + 200 reused ids)"
rw_run_bounded "$BUDGET" cargo run -q --no-default-features --features desktop,macos \
    --example macos_canvas_lifecycle_probe

echo "check_macos_canvas_lifecycle: OK"
