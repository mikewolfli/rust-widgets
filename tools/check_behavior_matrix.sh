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

echo "[1/14] default profile capability contract"
run_case "default capability contract" cargo test platform::tests::consistency_capability_contract_by_profile

echo "[2/14] default menu trigger parity"
run_case "default menu trigger parity" cargo test platform::tests::consistency_menu_trigger_roundtrip

echo "[3/14] default typed trigger parity"
run_case "default typed trigger parity" cargo test platform::tests::consistency_typed_widget_trigger_roundtrip

echo "[4/14] embedded capability contract"
run_case "embedded capability contract" cargo test --no-default-features --features embedded platform::tests::consistency_capability_contract_by_profile

echo "[5/14] embedded control matrix parity"
run_case "embedded control matrix parity" cargo test --lib --no-default-features --features embedded platform::tests::embedded_profile_core_controls_have_non_placeholder_create_paths

echo "[6/14] embedded host unsupported semantics"
run_case "embedded host unsupported semantics" cargo test --lib --no-default-features --features embedded platform::tests::embedded_profile_host_controls_are_explicitly_unsupported

echo "[7/14] embedded combo/list state-event-data parity"
run_case "embedded combo/list parity" cargo test --lib --no-default-features --features embedded platform::tests::embedded_profile_combo_list_state_event_data_roundtrip

echo "[8/14] embedded runtime deterministic task order"
run_case "embedded runtime deterministic order" cargo test --lib --no-default-features --features embedded render_engine::tests::embedded_task_queue_order_is_deterministic

echo "[9/14] full+mobile-api capability contract"
run_case "full+mobile-api capability contract" cargo test --features "full,mobile-api" platform::tests::consistency_capability_contract_by_profile

echo "[10/14] full+mobile-api typed trigger parity"
run_case "full+mobile-api typed trigger parity" cargo test --features "full,mobile-api" platform::tests::consistency_typed_widget_trigger_roundtrip

echo "[11/14] gpu covered-controls parity command suite"
run_case "gpu covered-controls parity command suite" cargo test --lib --features gpu-wgpu render::tests::gpu_parity_covered_controls_emit_non_empty_command_suite

echo "[12/14] gpu covered-controls parity auto compose"
run_case "gpu covered-controls parity auto compose" cargo test --lib --features gpu-wgpu render::tests::gpu_parity_covered_controls_auto_compose_runs_with_gpu_or_cpu_backend

echo "[13/14] embedded demo schema parity"
run_case "embedded demo schema parity" bash tools/check_embedded_demo_schema.sh

echo "[14/14] signal-first event model gate"
run_case "signal-first event model gate" bash tools/check_event_model_signal_first.sh

echo >> "$REPORT_FILE"
echo "All behavior matrix checks passed." >> "$REPORT_FILE"

echo "Behavior matrix report written to $REPORT_FILE"
