#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

REPORT_DIR="target/qa"
REPORT_FILE="$REPORT_DIR/behavior_matrix_report.md"
mkdir -p "$REPORT_DIR"

run_case() {
  local title="$1"
  shift
  echo "- running: $title"
  "$@"
  echo "- ✅ $title" >> "$REPORT_FILE"
}

{
  echo "# rust_widgets behavior matrix report"
  echo
  echo "Generated at: $(date -u +"%Y-%m-%dT%H:%M:%SZ")"
  echo
  echo "## Cases"
} > "$REPORT_FILE"

echo "[1/8] default profile capability contract"
run_case "default capability contract" cargo test platform::tests::consistency_capability_contract_by_profile

echo "[2/8] default menu trigger parity"
run_case "default menu trigger parity" cargo test platform::tests::consistency_menu_trigger_roundtrip

echo "[3/8] default typed trigger parity"
run_case "default typed trigger parity" cargo test platform::tests::consistency_typed_widget_trigger_roundtrip

echo "[4/8] embedded capability contract"
run_case "embedded capability contract" cargo test --no-default-features --features embedded platform::tests::consistency_capability_contract_by_profile

echo "[5/8] full+mobile-api capability contract"
run_case "full+mobile-api capability contract" cargo test --features "full,mobile-api" platform::tests::consistency_capability_contract_by_profile

echo "[6/8] full+mobile-api typed trigger parity"
run_case "full+mobile-api typed trigger parity" cargo test --features "full,mobile-api" platform::tests::consistency_typed_widget_trigger_roundtrip

echo "[7/8] embedded demo schema parity"
run_case "embedded demo schema parity" bash tools/check_embedded_demo_schema.sh

echo "[8/8] signal-first event model gate"
run_case "signal-first event model gate" bash tools/check_event_model_signal_first.sh

echo >> "$REPORT_FILE"
echo "All behavior matrix checks passed." >> "$REPORT_FILE"

echo "Behavior matrix report written to $REPORT_FILE"
