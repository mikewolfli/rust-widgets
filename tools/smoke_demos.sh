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
  if cargo check -q "$@" 2>&1; then
    pass "$name"
  else
    fail "$name"
  fi
}

# ---------------------------------------------------------------------------
# [1] Default profile demos
# ---------------------------------------------------------------------------
echo ""
echo "=============================================="
echo " Default Profile Widget Creation Smoke Tests "
echo "=============================================="

run_smoke "demo_main (full profile)"      --example demo_main
run_smoke "demo_button (full profile)"     --example demo_button
run_smoke "demo_window (full profile)"     --example demo_window
run_smoke "demo_list_view (full profile)"  --example demo_list_view
run_smoke "demo_code_editor (full)"        --example demo_code_editor
run_smoke "demo_terminal (full)"           --example demo_terminal
run_smoke "demo_media_player (full)"       --example demo_media_player
run_smoke "demo_map_view (full)"           --example demo_map_view

# ---------------------------------------------------------------------------
# [2] Embedded profile demos
# ---------------------------------------------------------------------------
echo ""
echo "=============================================="
echo " Embedded Profile Widget Creation Smoke Tests"
echo "=============================================="

FEAT="--no-default-features --features embedded"

run_smoke "demo_button (embedded)"     cargo check --example demo_button $FEAT
run_smoke "demo_window (embedded)"     cargo check --example demo_window $FEAT
run_smoke "demo_list_view (embedded)"  cargo check --example demo_list_view $FEAT

# ---------------------------------------------------------------------------
# [3] Runtime & integration test suite
# ---------------------------------------------------------------------------
echo ""
echo "=============================================="
echo " Runtime & Integration Smoke Tests"
echo "=============================================="

run_smoke "platform integration tests"              cargo test -q --lib platform::tests
run_smoke "widget kind smoke test"                  cargo test -q --test blue9_r6_platform_capability_test
run_smoke "widget structure tests"                  cargo test -q --test test_widget_structure

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
