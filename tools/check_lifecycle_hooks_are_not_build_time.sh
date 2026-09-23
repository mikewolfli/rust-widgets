#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_lifecycle_hooks_are_not_build_time.sh -- BLUE23 §5A.3 (judgement 7)
# ============================================================================
# The rule this guards (BLUE23 §5A.3):
#
#   **`build` stays a pure function: a lifecycle hook is never invoked while a tree is
#   being described or diffed.**
#
# # Why this is a gate and not a test
#
# `View::build` returns a `Node` tree, and `diff` compares two of them. Both are pure
# functions, and that is the premise the whole retained-tree architecture rests on: if
# describing a tree could fire a side effect, then the same state would produce different
# effects on every rebuild, and an update could never settle. Lifecycle hooks
# (`on_mount`/`on_unmount`) are **side effects** — start a poll, register a subscription —
# so they must be *carried* on the node and run by the engine after the patch batch lands.
#
# A one-line mistake — calling the hook inside `Node`'s builder, or inside `diff` — would
# still compile, still pass every behaviour test (the hook does fire, just at the wrong
# time), and quietly break the purity the diff depends on. Nothing else would notice. That
# is exactly the class of defect this project gates rather than tests.
#
# # What this gate proves
#
# A lexical, no-build scan. In the files that *describe* or *compare* a tree — `node.rs`
# (the builders) and `diff.rs` (the comparator) — the callable hook fields (`on_mount`,
# `on_unmount`) must never appear in a call position. The engine (`engine.rs`) is the one
# file allowed to invoke them, and it is where the invocation is asserted to exist.
#
# # What this gate does NOT prove
#
#   * It is lexical: it matches the call spelling, not the call graph. A hook could be
#     invoked indirectly through a helper. The engine tests
#     (`view::engine::tests::on_mount_*`) cover the behavioural half.
#
# # Reverse injection
#
# Step 2 appends a hook invocation to `node.rs` and requires the scan to report it, so a
# search that matched nothing cannot pass.
#
# Usage: tools/check_lifecycle_hooks_are_not_build_time.sh
# Exit 0 = hooks are carried in describe/compare and invoked only by the engine.
# Exit 1 = a hook is invoked at build/diff time.
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_python.sh"

if ! "$PYTHON" tools/lifecycle_hook_scan.py; then
    echo ""
    echo "  A lifecycle hook is a side effect and must not run while a tree is described or"
    echo "  diffed: carry it on the node and let the engine run it after the patches land."
    echo "  See BLUE23 §5A.3."
    exit 1
fi

if "$PYTHON" tools/lifecycle_hook_scan.py --inject >/dev/null 2>&1; then
    echo "FAIL: injecting a build-time hook call did not change the result, so the check is"
    echo "      not reading the describe/compare files"
    exit 1
fi

echo "lifecycle-hook checks passed."
