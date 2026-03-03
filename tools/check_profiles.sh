#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

echo "[1/5] cargo check (default)"
cargo check

echo "[2/5] cargo check --examples"
cargo check --examples

echo "[3/5] cargo check --no-default-features --features embedded"
cargo check --no-default-features --features embedded

echo "[4/5] embedded P4c regression gate"
cargo test --lib --no-default-features --features embedded platform::tests::embedded_profile_combo_list_state_event_data_roundtrip
cargo test --lib --no-default-features --features embedded render_engine::tests::embedded_task_queue_order_is_deterministic

echo "[5/5] gpu P3g parity regression gate"
cargo test --lib --features gpu-wgpu render::tests::gpu_parity_covered_controls_emit_non_empty_command_suite
cargo test --lib --features gpu-wgpu render::tests::gpu_parity_covered_controls_auto_compose_runs_with_gpu_or_cpu_backend
cargo check --features gpu-wgpu --example demo_wgpu_control_parity

echo "All profile checks passed."
