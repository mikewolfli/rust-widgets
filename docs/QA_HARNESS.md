# QA Harness

This document describes the cross-profile behavior matrix and visual regression harness.

## Behavior Matrix

Run:

```bash
tools/check_behavior_matrix.sh
```

What it validates:

- Capability contract consistency across `default`, `embedded`, and `full,mobile-api`.
- Menu trigger event parity.
- Typed widget trigger parity.
- Embedded C/Python/Java demo output schema parity (`KEY=VALUE` + key order).

Output report:

- `target/qa/behavior_matrix_report.md`

## Visual Regression

Run:

```bash
tools/check_visual_regression.sh
```

What it validates:

- Deterministic SVG snapshot hash for line chart rendering.
- Deterministic SVG snapshot hash for bar chart rendering.

Output report:

- `target/qa/visual_regression_report.md`

## Embedded Demo Schema

Run:

```bash
tools/check_embedded_demo_schema.sh
```

Note: this check is also included inside `tools/check_behavior_matrix.sh`.

What it validates:

- C/Python/Java embedded demo outputs use the same `KEY=VALUE` schema.
- Output key order is identical across all three language demos.

Scope:

- `examples/c_abi_embedded_engine_demo.c`
- `examples/python/demo_embedded_engine.py`
- `examples/java/RustWidgetsEmbeddedEngineDemo.java`

## Full QA Pass

```bash
tools/check_behavior_matrix.sh && tools/check_visual_regression.sh
```

Use this command in CI or release gates to detect behavior and rendering regressions.

## V19 Focused Regression Set (ComboBox/ListBox)

Run:

```bash
cargo test --lib consistency_combo_box_data_and_event_path_roundtrip
cargo test --lib consistency_list_box_data_path_roundtrip
cargo test --lib combo_selection_notify_enqueues_selection_and_value_events
```

What it validates:

- Stub backend ComboBox/ListBox create/state/event/data-path contract remains deterministic.
- Windows ComboBox selection notification enqueues typed `SelectionChanged` then `ValueChanged` events in deterministic order.
- Data-path coverage remains explicit across backends (implemented or explicit unsupported diagnostics).

## V19 Embedded Regression Set (P4c)

Run:

```bash
cargo test --lib --no-default-features --features embedded platform::tests::embedded_profile_core_controls_have_non_placeholder_create_paths
cargo test --lib --no-default-features --features embedded platform::tests::embedded_profile_host_controls_are_explicitly_unsupported
cargo test --lib --no-default-features --features embedded platform::tests::embedded_profile_combo_list_state_event_data_roundtrip
cargo test --lib --no-default-features --features embedded render_engine::tests::embedded_task_queue_order_is_deterministic
```

What it validates:

- Embedded startup/runtime loop path remains executable and deterministic for queued frame tasks.
- Embedded control matrix keeps complete create/state/event/data-path behavior for supported desktop-core controls.
- Embedded host controls stay explicit unsupported (`0`/`false`) with deterministic capability boundary semantics.

## V19 GPU Parity Regression Set (P3g)

Run:

```bash
cargo test --lib --features gpu-wgpu render::tests::gpu_parity_covered_controls_emit_non_empty_command_suite
cargo test --lib --features gpu-wgpu render::tests::gpu_parity_covered_controls_auto_compose_runs_with_gpu_or_cpu_backend
cargo run --features gpu-wgpu --example demo_wgpu_control_parity
```

What it validates:

- Covered-control GPU parity visual builders (base + data/range + host/navigation) produce deterministic non-empty command suites.
- Unified auto compose path remains deterministic with runtime GPU/CPU selection while preserving rendered frame output.
- End-to-end covered-control parity demo remains runnable and emits deterministic checksum diagnostics.

## V18 Startup Smoke Matrix

Scope: `cargo run --example demo_main`

| Backend | Runtime GUI Mode | Window Visible | Loop Alive | Close Works | Evidence | Status |
|---|---|---|---|---|---|---|
| Windows (`WindowsPlatform`) | `NativeInteractive` | Pending manual verification | ✅ (interactive run keeps loop active) | ✅ (latest local runs exited with code 0 after close) | 2026-03-03: multiple `cargo run --example demo_main` runs in this workspace completed with exit code 0 after close; prior timeout probe also confirmed loop stays active while open | In progress |
| macOS (`cocoa`) | `NativeInteractive` | Pending | Pending | Pending | Not executed in current Windows workspace | Pending |
| Linux + `gtk-native` (`gtk`) | `NativeInteractive` | Pending | Pending | Pending | Not executed in current Windows workspace | Pending |
| Linux (non `gtk-native`) | `PreviewOrStub` | N/A | Expected poll loop | N/A | Runtime warning path implemented in `src/platform/linux.rs` | Pending validation |
| Harmony desktop (`harmony-desktop`) | `PreviewOrStub` | N/A | Expected poll loop | N/A | Runtime warning path implemented in `src/platform/harmony.rs` | Pending validation |
| macOS objc2 preview (`macos-objc2-preview`) | `PreviewOrStub` | N/A | Expected poll loop | N/A | Runtime warning path implemented in `src/platform/macos_objc2.rs` | Pending validation |
| Android mobile (`android-mobile`) | `PreviewOrStub` | N/A | Stub | N/A | Runtime warning path implemented in `src/platform/mobile.rs` | Pending validation |

Notes:
- `Window Visible` and `Close Works` require interactive manual verification on target OS.
- Matrix should be updated after each platform smoke run.
- Unsupported widget `create_*` calls should return `0` (invalid object id) explicitly and emit backend diagnostics where applicable.

## V20 Layout → Advanced Demo Visibility Audit

Scope: `Layout System` to `Advanced Widgets` representative demos.

| Demo | Current intent | Expected native window | Evidence (2026-03-03) | Status |
|---|---|---|---|---|
| `demo_layout` | Layout computation/rect output | No | `cargo run --example demo_layout` prints rect list and exits | Verified |
| `demo_table` | Model/view contract smoke | No (current path) | `cargo run --example demo_table` prints column/selection/shape and exits | Verified |
| `demo_treeview` | Model/view contract smoke | No (current path) | `cargo run --example demo_treeview` prints visible nodes/selection and exits | Verified |
| `demo_grid` | Layout/container smoke | No (current path) | `cargo run --example demo_grid` exits without persistent window loop | Verified |
| `demo_stack_widget` | Layout/container smoke | No (current path) | `cargo run --example demo_stack_widget` exits without persistent window loop | Verified |
| `demo_chart` | Rendering/export smoke | No | `cargo run --example demo_chart` prints draw count + SVG export and exits | Verified |
| `demo_main` | Runtime window-loop baseline | Yes | startup log reports `backend 'cocoa'` + `native-interactive` | Verified |

Notes:
- This matrix tracks runtime visibility behavior separately from feature-contract completion in `plan.md`.
- Demos marked `No (current path)` are candidates for reconciliation under TODO v20 R2/R3 when native-visible behavior is desired.

### Manual Verification Checklist (per backend)

1. Run `cargo run --example demo_main` on the target backend.
2. Confirm startup log includes backend name and runtime GUI mode.
3. Confirm a native window is visible (only for `NativeInteractive` backends).
4. Keep app open for at least 5 seconds and verify process remains alive.
5. Close the window (or trigger quit) and verify process exits cleanly.
6. Record result in the matrix row (`Window Visible`, `Loop Alive`, `Close Works`, `Evidence`, `Status`).

### Evidence Template

- Backend:
- Runtime GUI Mode:
- Command:
- Startup log:
- Window visible:
- Loop alive (duration observed):
- Close/quit behavior:
- Exit code:
- Notes:
