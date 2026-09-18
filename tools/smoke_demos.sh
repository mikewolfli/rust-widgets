#!/usr/bin/env bash
# ============================================================================
# smoke_demos.sh — R6 Smoke Test for Widget Creation & Rendering Lifecycle
# ============================================================================
# Verifies basic widget creation and rendering lifecycle for key widgets:
#   Button, Label, Window, ListView, CodeEditor, TerminalView,
#   MediaPlayer, MapView
#
# Each check compiles the widget test or example in both default and
# embedded profiles where applicable.
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

PASS=0
FAIL=0

pass() {
  echo "  ✅ $*"
  PASS=$((PASS + 1))
}

fail() {
  echo "  ❌ $*"
  FAIL=$((FAIL + 1))
}

run_smoke() {
  local name="$1"
  shift
  echo ""
  echo "═══ SMOKE: $name ═══"
  if "$@"; then
    pass "$name"
  else
    fail "$name"
  fi
}

run_example_smoke() {
  local name="$1"
  shift
  if [[ -f "examples/${name}.rs" ]]; then
    run_smoke "${name}" cargo check -q --example "${name}" "$@"
  else
    echo ""
    echo "═══ SMOKE: ${name} ═══"
    fail "${name} (missing examples/${name}.rs)"
  fi
}

# ---------------------------------------------------------------------------
# [1] Default profile demos
# ---------------------------------------------------------------------------
echo ""
echo "=============================================="
echo " Default Profile Widget Creation Smoke Tests "
echo "=============================================="

run_example_smoke "demo_button"
# `demo_button` is also the embedded-profile canary below; the other `demo_*.rs`
# SVG-length stubs that lived here were removed once the three real projects under
# `demo/` covered the same controls with actual applications and assertions. Keeping
# both meant two things claiming to be the example for one control.
# The declarative-retained loop (BLUE18 Phase F-1): state change -> one patch.
run_example_smoke "view_counter"

# ---------------------------------------------------------------------------
# [2] Embedded profile demos
# ---------------------------------------------------------------------------
# The `demo/` projects cannot stand in here: each declares `gtk-native` (a desktop
# backend), so they do not build on a stripped profile. This example is the only
# remaining coverage that a control is *constructible* on `embedded`.
echo ""
echo "=============================================="
echo " Embedded Profile Widget Creation Smoke Tests"
echo "=============================================="

FEAT="--no-default-features --features embedded"

run_example_smoke "demo_button" --no-default-features --features embedded

# ---------------------------------------------------------------------------
# [3] Runtime & integration test suite
# ---------------------------------------------------------------------------
echo ""
echo "=============================================="
echo " Runtime & Integration Smoke Tests"
echo "=============================================="

run_smoke "platform integration tests" cargo test -q --lib platform::tests::consistency_capability_contract_by_profile
run_smoke "widget kind smoke test" cargo test -q --test blue9_r6_platform_capability_test
run_smoke "widget structure tests" cargo test -q --test integration_test

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
echo ""
echo "=============================================="
echo " SMOKE TEST RESULTS"
echo "=============================================="
echo "  Passed: $PASS"
echo "  Failed: $FAIL"
echo ""

if [[ $FAIL -gt 0 ]]; then
  echo "❌ Some smoke tests failed."
  exit 1
else
  echo "✅ All smoke tests passed."
  exit 0
fi
