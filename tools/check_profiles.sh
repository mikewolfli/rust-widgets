#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

# Runs one named test and fails if the filter matched nothing.
#
# `cargo test <filter>` exits **0** when the filter matches no test at all, so a
# stale test name turns a step into a silent no-op: `set -e` sees success, the
# script prints "All profile checks passed", and zero tests ran. That is exactly
# what happened to step [7] — it kept naming
# `platform::tests::embedded_profile_combo_list_state_event_data_roundtrip` after
# commit 45ea40b renamed the test to `embedded_profile_selection_state_roundtrip`
# (the sibling `check_behavior_matrix.sh` had its `run_case` reference updated in
# the same commit; this file's plain `cargo test` line was missed), so the
# "embedded P4c regression gate" verified nothing for that whole period.
#
# `check_behavior_matrix.sh` already refuses to pass a case that ran zero tests;
# this is the same guard, for the same reason, in the gate that lacked it.
run_test_case() {
  local title="$1"
  shift
  local output_file
  output_file="$(mktemp)"
  echo "  - running: $title"
  if ! "$@" >"$output_file" 2>&1; then
    cat "$output_file"
    rm -f "$output_file"
    echo "❌ $title FAILED" >&2
    return 1
  fi
  if ! grep -Eq 'running [1-9][0-9]* tests?' "$output_file"; then
    echo "❌ QA case ran zero tests: $title" >&2
    echo "   (cargo test exits 0 on a filter that matches nothing — fix the filter)" >&2
    rm -f "$output_file"
    return 1
  fi
  rm -f "$output_file"
  echo "  - ✅ $title"
}

echo "[1/8] cargo check (default)"
cargo check

echo "[2/8] cargo check --examples"
cargo check --examples

echo "[3/8] cargo check --no-default-features --features tablet --all-targets"
cargo check --no-default-features --features tablet --all-targets

echo "[4/8] cargo check --no-default-features --features mobile --all-targets"
cargo check --no-default-features --features mobile --all-targets

echo "[5/8] cargo check --no-default-features --features mini --all-targets"
cargo check --no-default-features --features mini --all-targets

echo "[6/8] cargo check --no-default-features --features embedded --all-targets"
cargo check --no-default-features --features embedded --all-targets

echo "[7/8] embedded P4c regression gate"
run_test_case "embedded selection-state roundtrip" \
  cargo test --lib --no-default-features --features embedded platform::tests::embedded_profile_selection_state_roundtrip
run_test_case "embedded task queue determinism" \
  cargo test --lib --no-default-features --features embedded render_engine::embedded_engine::tests::embedded_task_queue_order_is_deterministic

echo "[8/8] gpu P3g parity regression gate"
run_test_case "gpu auto-compose mixed scene" \
  cargo test --lib --features gpu-wgpu render::tests::auto_compose_renders_mixed_commands_scene_with_gpu_or_cpu_backend
run_test_case "gpu auto-compose cpu fallback" \
  cargo test --lib --features gpu-wgpu render::tests::auto_compose_falls_back_to_cpu_backend_when_gpu_path_is_rejected
cargo check --features gpu-wgpu --example demo_wgpu_control_parity

echo "All profile checks passed."
