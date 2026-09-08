#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

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
cargo test --lib --no-default-features --features embedded platform::tests::embedded_profile_combo_list_state_event_data_roundtrip
cargo test --lib --no-default-features --features embedded render_engine::embedded_engine::tests::embedded_task_queue_order_is_deterministic

echo "[8/8] gpu P3g parity regression gate"
cargo test --lib --features gpu-wgpu render::tests::auto_compose_renders_mixed_commands_scene_with_gpu_or_cpu_backend
cargo test --lib --features gpu-wgpu render::tests::auto_compose_falls_back_to_cpu_backend_when_gpu_path_is_rejected
cargo check --features gpu-wgpu --example demo_wgpu_control_parity

echo "All profile checks passed."
