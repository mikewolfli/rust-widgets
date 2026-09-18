#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_timeout.sh"

# Each case is a `cargo test` (or a nested gate). Bounding per case keeps a wedge
# localised: the report names which case hung instead of the whole matrix going
# quiet. The case budget covers a cold compile of its feature set.
CASE_TIMEOUT=900

REPORT_DIR="target/qa"
REPORT_FILE="$REPORT_DIR/behavior_matrix_report.md"
mkdir -p "$REPORT_DIR"

run_case() {
  local title="$1"
  shift
  local output_file
  output_file="$(mktemp)"
  echo "- running: $title"
  if ! rw_run_bounded "$CASE_TIMEOUT" "$@" >"$output_file" 2>&1; then
    cat "$output_file"
    rm -f "$output_file"
    echo "❌ case failed or exceeded ${CASE_TIMEOUT}s: $title" >&2
    return 1
  fi
  cat "$output_file"
  if [[ "${1:-}" == "cargo" && "${2:-}" == "test" ]] \
    && ! grep -Eq 'running [1-9][0-9]* tests?' "$output_file"; then
    echo "QA case ran zero tests: $title" >&2
    rm -f "$output_file"
    return 1
  fi
  rm -f "$output_file"
  echo "- ✅ $title" >> "$REPORT_FILE"
}

# Records a case that this host cannot build, with the reason.
#
# The `full` meta-feature enables `macos-legacy`, `ios`, `android`, `wasm`,
# `harmony`, `linux-gtk`, `webkit-engine` and `video-codecs`. Those dependencies
# cannot be built on Windows at all, and the GTK/WebKit/FFmpeg half also needs
# system dev packages on Linux — so the two `full` cases are only meaningful on
# macOS. Failing on every other host would report a defect that is not one, and
# skipping silently would be the gate-weakening the project rules forbid: the case
# is named in the report, and it keeps its full authority on a host that can build
# the set.
skip_case() {
  local title="$1"
  local reason="$2"
  echo "- ⏭ not applicable: $title ($reason)"
  echo "- ⏭ $title — NOT APPLICABLE on this host: $reason" >> "$REPORT_FILE"
}

CAN_BUILD_FULL=0
if [[ "$(uname -s)" == "Darwin" ]]; then
  CAN_BUILD_FULL=1
fi

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

echo "[5/14] embedded control registration parity"
run_case "embedded control matrix parity" cargo test --lib --no-default-features --features embedded platform::tests::embedded_profile_self_drawn_controls_are_registered_with_the_host

echo "[6/14] embedded host unsupported semantics"
run_case "embedded host capability contract" cargo test --lib --no-default-features --features embedded platform::tests::consistency_capability_contract_by_profile

echo "[7/14] embedded combo/list state-event-data parity"
run_case "embedded selection-state parity" cargo test --lib --no-default-features --features embedded platform::tests::embedded_profile_selection_state_roundtrip

echo "[8/14] embedded runtime deterministic task order"
run_case "embedded runtime deterministic order" cargo test --lib --no-default-features --features embedded render_engine::embedded_engine::tests::embedded_task_queue_order_is_deterministic

echo "[9/14] full+mobile-api capability contract"
if [[ "$CAN_BUILD_FULL" == "1" ]]; then
  run_case "full+mobile-api capability contract" cargo test --features "full,mobile-api" platform::tests::consistency_capability_contract_by_profile
else
  skip_case "full+mobile-api capability contract" "the 'full' feature set needs macOS-only and system-library dependencies"
fi

echo "[10/14] full+mobile-api typed trigger parity"
if [[ "$CAN_BUILD_FULL" == "1" ]]; then
  run_case "full+mobile-api typed trigger parity" cargo test --features "full,mobile-api" platform::tests::consistency_typed_widget_trigger_roundtrip
else
  skip_case "full+mobile-api typed trigger parity" "the 'full' feature set needs macOS-only and system-library dependencies"
fi

echo "[11/14] gpu covered-controls parity command suite"
run_case "gpu covered-controls parity command suite" cargo test --lib --features gpu-wgpu render::tests::auto_compose_renders_mixed_commands_scene_with_gpu_or_cpu_backend

echo "[12/14] gpu covered-controls parity auto compose"
run_case "gpu covered-controls parity auto compose" cargo test --lib --features gpu-wgpu render::tests::auto_compose_falls_back_to_cpu_backend_when_gpu_path_is_rejected

echo "[13/14] embedded demo schema parity"
run_case "embedded demo schema parity" bash tools/check_embedded_demo_schema.sh

echo "[14/14] signal-first event model gate"
run_case "signal-first event model gate" bash tools/check_event_model_signal_first.sh

echo >> "$REPORT_FILE"
echo "All behavior matrix checks passed." >> "$REPORT_FILE"

echo "Behavior matrix report written to $REPORT_FILE"
