#!/usr/bin/env bash
# ============================================================================
# check_demo_crates.sh — the demo crates must compile
# ============================================================================
# `demo/control`, `demo/finance` and `demo/code_editor` are separate crates that
# depend on this library by path. No root-workspace target compiles them, so a
# library change that breaks a demo is invisible to `cargo test` at the root:
# two of the three were found broken (missing `Layout::arrange*` /
# `Layout::update_with_context` implementations, which the trait requires with
# no default) only when they were built by hand.
#
# `--all-targets` is deliberate: each demo carries its own unit tests, and those
# are part of the same contract.
#
# Every `cargo` call is bounded through `rw_run_bounded` (principle #58), and the
# per-crate line is printed as each one finishes so a wedged build names itself.
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_timeout.sh"
DEMO_BUDGET="${RW_GATE_TIMEOUT:-900}"

PASS=0
FAIL=0

for crate in demo/control demo/finance demo/code_editor; do
    echo "═══ DEMO CRATE: $crate ═══"
    if [[ ! -f "$crate/Cargo.toml" ]]; then
        echo "  ❌ $crate has no Cargo.toml"
        FAIL=$((FAIL + 1))
        continue
    fi
    if rw_run_bounded "$DEMO_BUDGET" cargo check --all-targets --manifest-path "$crate/Cargo.toml"; then
        echo "  ✅ $crate compiles"
        PASS=$((PASS + 1))
    else
        echo "  ❌ $crate does not compile"
        FAIL=$((FAIL + 1))
    fi
done

echo ""
echo "check_demo_crates: passed=$PASS failed=$FAIL"
[[ "$FAIL" -eq 0 ]]
