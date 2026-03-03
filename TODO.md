# rust_widgets Roadmap TODO

This file mirrors staged execution status.

## Maintenance Rule (Required)

- New requirements are always added at the top under the latest version section.
- Older requirement sets are assigned a version tag (`v1`, `v2`, ...), moved downward, and kept as history.
- Status updates must be done in both this file and the live task panel.
- If old version has no completed line, please add the new todo list to current version requirement list.
- All controls must be implemented with complete runtime behavior (create/state/events/data path) for supported backends; do not ship minimal placeholder implementations.
- Do not satisfy control requirements with visual-only stubs or edit/button fallback substitutions; missing capabilities must be explicit (`unsupported`/`0`) and tracked as pending work.
- Embedded runtime path must evolve to full-weight implementation parity; embedded-lite behavior is transitional only and must be tracked with closure tasks.

## Current Requirements (v21)

## Stage Progress

- [ ] P0 Dual control backend full-weight closure (custom-default route)
    - [x] P0-Feature Introduce and enforce compile-time policy presets: `control-policy-native-strict`, `control-policy-hybrid`, `control-policy-custom-full`.
    - [x] P0-Feature Keep `full` default on `control-policy-hybrid` until custom-full parity gates are green.
    - [x] P0-Feature Land v1 widget-kind route matrix (`NativePreferred` + `CustomRequired`) and publish policy matrix doc (`docs/CONTROL_POLICY_MATRIX.md`).
    - [ ] P0a Complete custom-backend create/state/event/data-path parity for supported controls (no placeholder behavior).
    - [ ] P0b Close deterministic focus-owner routing, hit-test, and pointer-capture handoff in unified event bridge.
    - [ ] P0c Finalize IME/accessibility routing contract (platform bridge retention + explicit capability diagnostics).
    - [ ] P0d Add parity gates for unsupported controls with explicit diagnostics matrix and CI checks.

- [ ] P1 Runtime-visible behavior and window/layout contract alignment
    - [ ] P1a Unify widget show/hide semantics with runtime-visible behavior across native/custom routes.
    - [ ] P1b Bind layout recompute results to runtime geometry updates and deterministic parent-resize relayout.
    - [ ] P1c Complete modal/main-window/frameless lifecycle contract and backend parity diagnostics.
    - [ ] P1d Finish Layout→Advanced demo intent matrix reconciliation (`R1`/`R2`/`R3`) with startup diagnostics standard.

- [ ] P2 Control/model/view/theme advanced closure
    - [ ] P2a Complete basic/intermediate control high-frequency interaction parity (Combo/List/Spin/Dialog/Menu host family).
    - [ ] P2b Finalize model/view incremental update contract (insert/remove/update ranges + selection sync parity tests).
    - [ ] P2c Complete advanced widget interactive parity (`TreeView`/`TableView`/`ListView`/`DockPanel`/`MdiArea`/`RichEdit` baseline+).
    - [ ] P2d Land theme runtime switching closure and RWSS parser/selector pipeline with documented grammar.

- [ ] P3 Forward path (animation + IDE capability foundation)
    - [ ] P3a Implement animation timeline scheduler, easing primitives, and deterministic frame-step test harness.
    - [ ] P3b Define plugin API boundary and deliver IDE baseline integration path (multi-window + docking + code-editor workflow primitives).

### v21 Mapping Note

- This version is synchronized from `plan.md` gap lines added on 2026-03-03 and is prioritized by delivery criticality (`P0` → `P3`).
- Completion policy: a stage can be checked only when all child acceptance behaviors are verified by runnable demos/tests and diagnostics are explicit for unsupported paths.

## Requirement History (v20)

## Stage Progress

- [x] Phase A (native + GPU operational baseline) — Completed 2026-03-03
    - [x] A0 Interface/event-source baseline landed (`ControlBackend` + source-routed trigger pump).
    - [x] A1 Complete facade cutover in C ABI/widget create-state/event polling paths.
        - [x] A1a First cutover batch landed: window/button create + trigger polling/injection + text/enabled/visible + backend-name routes now use `get_control_backend()`.
        - [x] A1b Second cutover batch landed: extended control create paths, menu host APIs, and show/hide are routed via `get_control_backend()`.
        - [x] A1c Keep direct platform bridges for system host capabilities (IME/accessibility) by design (方案 1).
    - [x] A2 Validate `controls-native,gpu-wgpu` profile keeps native control behavior unchanged.
    - [x] A3 Restrict GPU acceleration to render-heavy/custom paint surfaces and keep deterministic CPU render-backend fallback checks green.
        - [x] A3a Deterministic render-backend fallback check is green under `controls-native,gpu-wgpu`.
        - [x] A3b Confirm and enforce GPU acceleration scope boundaries (render-heavy/custom paint surfaces only).
    - [x] A4 Add profile checks/demos coverage for native+GPU combo.

### Layout → Advanced Demo Visibility Reconciliation (v20)

- [ ] R1 Clarify demo intent matrix for `Layout System` to `Advanced Widgets`:
    - [ ] R1a Mark computation-only demos explicitly (e.g. `demo_layout`) as non-window demos in docs.
    - [ ] R1b Mark native-window demos explicitly and require `create_window/show_widget/run` path (or equivalent native path).
- [ ] R2 Retrofit representative model/widget demos to visible native-window path where expected:
    - [ ] R2a `demo_table` native-visible path verification and alignment.
    - [ ] R2b `demo_treeview` native-visible path verification and alignment.
    - [ ] R2c `demo_grid`/`demo_stack_widget` native-visible path verification and alignment.
- [ ] R3 Add startup diagnostics standard for these demos:
    - [ ] R3a Print runtime GUI mode (`NativeInteractive` vs `PreviewOrStub`) at startup.
    - [ ] R3b Print backend name and whether visible native window is expected on current platform mode.
- [ ] R4 Add verification checklist and evidence capture:
    - [x] R4a Run and record `cargo run --example demo_layout` (expected console-only).
    - [x] R4b Run and record representative visible-window demos under current platform backend.
    - [x] R4c Update `docs/QA_HARNESS.md` with visibility expectations for Layout→Advanced demo set.

### Visibility Gap Note (v20)

- Current `plan.md` items for `Layout System`/`Model/View`/`Advanced Widgets` are function-contract complete, but demo window visibility parity is tracked separately here and must be validated per backend mode.

### Completed Baseline (v20 already landed)

- [x] Land unified `ControlBackend` scaffold (`native` + `custom` kinds) and compile-time selection features (`controls-native` / `controls-custom`).
- [x] Land event-source routing scaffold (`TriggerEventSource`, `PlatformTriggerEventSource`, `ControlBackendTriggerEventSource`) and source-based bridge pump entrypoints.
- [x] Keep current runtime behavior unchanged by default (superseded in v21 by `control-policy-hybrid` default route).

### V20 Item: Dual Control Backend & Event Routing (Merged from latest plan item)

- Goal: establish stable interfaces for both native controls and future custom-painted controls, with one upper-layer event/signal routing model and compile-time backend selection.
- Phase scope (aligned with latest `plan.md`):
    - Phase A: hybrid + GPU operational baseline (`control-policy-hybrid,gpu-wgpu`) — preferred execution path
    - Final target: custom-full + GPU full path (`control-policy-custom-full,gpu-wgpu`)
    - Closure criteria: deterministic hit-test/pointer/focus routing, keyboard/IME/accessibility parity, create/state/event/data-path parity for supported controls, and explicit unsupported diagnostics

### A Implementation Status Check (2026-03-03)

- Done: `src/control_backend/mod.rs` has native/custom backend interface scaffold and compile-time selection.
- Done: `src/event/mod.rs` has source-based event shunting (`PlatformTriggerEventSource` / `ControlBackendTriggerEventSource`).
- Remaining: `src/bindings/mod.rs` keeps only intentional direct platform bridges for IME/accessibility (方案 1).

### Native + Custom Interface Integration Plan (with event shunting)

- Integration Step 1 (API routing): move C ABI create/state/event polling entry points from direct platform calls to `get_control_backend()` facade.
- Integration Step 2 (event shunting): route trigger polling/injection through backend source entrypoints first, keep platform source as compatibility route where needed (not control downgrade fallback).
- Integration Step 3 (backend ownership): keep system-only capabilities (window loop, clipboard/IME/accessibility host bridges) in platform layer; keep control create/state/trigger in control-backend layer.
- Integration Step 4 (validation): verify native profile parity under `controls-native,gpu-wgpu`, then enable custom backend incremental checks with explicit unsupported diagnostics.

### Acceptance Criteria (v20)

- Current default build keeps native behavior unchanged.
- New backend interfaces compile cleanly and can be selected at compile time.
- Event pump supports backend-agnostic source routing without changing upper signal connection APIs.
- Current milestone Phase A is complete; next milestone prepares final custom+GPU track.
- Custom-painted backend is only marked complete after full-weight closure criteria are met.

### Verification Notes (v20)

- Interface scaffold update (2026-03-03): added `src/control_backend/mod.rs` with `ControlBackend`, `NativeControlBackend`, and `CustomPaintControlBackend` skeleton.
- Event routing scaffold update (2026-03-03): extended `src/event/mod.rs` with `TriggerEventSource` and source-based bridge pump methods.
- Feature-gate update (2026-03-03): added `controls-native` and `controls-custom` feature switches in `Cargo.toml` (`full` now includes `controls-native`).
- Baseline compile verification (2026-03-03): `cargo check` passes after v20 scaffold integration.
- A-check audit update (2026-03-03): interface + event-source scaffolds are landed, but C ABI facade cutover remains incomplete (`src/bindings/mod.rs` still direct-platform heavy).
- A1 progress update (2026-03-03): facade cutover batch landed for core create/event/state routes in `src/bindings/mod.rs`; remaining direct platform calls reduced to extended control creation and platform-host APIs.
- A1 progress update (2026-03-03, second batch): `src/bindings/mod.rs` now routes most create/state/event/menu/show-hide paths through `get_control_backend()`; remaining direct platform calls are limited to IME/accessibility bridges.
- A1 closure decision (2026-03-03): adopt 方案 1, keep IME/accessibility bindings on platform layer as intentional system-host bridges; treat facade cutover scope as complete for v20 A1.
- A2 verification update (2026-03-03): `cargo check --features controls-native,gpu-wgpu`, `cargo check --features controls-native,gpu-wgpu --example demo_main`, and `cargo check --features controls-native,gpu-wgpu --example demo_wgpu_clear` all pass.
- A2 behavior validation update (2026-03-03): `cargo test --lib --features controls-native,gpu-wgpu render::tests::auto_compose_renders_base_control_scene_with_gpu_or_cpu_backend` passes.
- A3 fallback validation update (2026-03-03): added `render::tests::auto_compose_falls_back_to_cpu_backend_when_gpu_path_is_rejected` and verified `cargo test --lib --features controls-native,gpu-wgpu render::tests::auto_compose_falls_back_to_cpu_backend_when_gpu_path_is_rejected` passes.
- A3 scope-boundary audit update (2026-03-03): GPU auto-compose entrypoints (`compose_to_config_auto` / `compose_scene_to_surface_wgpu`) are confined to `src/render/mod.rs`; bindings/control-backend/event/platform paths route control/event flow without direct GPU entry coupling.
- A4 profile/demo coverage update (2026-03-03): `cargo check --features controls-native,gpu-wgpu --examples`, `cargo check --features controls-native,gpu-wgpu --example demo_wgpu_primitives`, and `cargo check --features controls-native,gpu-wgpu --example demo_wgpu_control_parity` all pass.
- Layout→Advanced visibility audit seed (2026-03-03): identified mixed demo patterns (some console/model-only, some native-window path), with reconciliation tasks tracked under v20 R1-R4.
- Layout→Advanced visibility run evidence (2026-03-03): `cargo run --example demo_layout` prints layout rects and exits (console-only, no native window expected).
- Layout→Advanced visibility run evidence (2026-03-03): `cargo run --example demo_table`, `demo_treeview`, `demo_grid`, `demo_stack_widget`, and `demo_chart` all execute and exit without a persistent native-window loop in current demo paths (chart exports SVG).
- Window-path baseline evidence (2026-03-03): `cargo run --example demo_main` reports backend `cocoa` with `native-interactive` mode; `cargo run --example demo_window` runs successfully (no extra startup diagnostics emitted currently).
- Default backend route update (2026-03-03): changed `Cargo.toml` `full` profile default from `controls-native` to `controls-custom`; default `cargo check` remains green.

## Requirement History (v19)

## Stage Progress

- [x] P0a Complete Windows widget trigger routing for native controls (not menu-only): wire `WM_COMMAND/WM_NOTIFY` notifications to `poll_widget_trigger_event` with deterministic `WidgetTriggerKind` mapping.
- [x] P0b Complete Windows ComboBox event-path parity: emit typed selection/value-change triggers on user interaction and keep programmatic selection changes synchronized.
- [x] P1a Add full ListBox data-path contract (platform API + backend implementations): add/remove/clear/count/get/set current selection and item text retrieval.
- [x] P1b Align preview backends (Linux non-gtk-native / Harmony / macOS objc2 preview / mobile preview) with explicit control capability semantics for ComboBox/ListBox data paths (full implementation or explicit unsupported diagnostics).
- [x] P2a Add focused regression tests for ComboBox/ListBox create+state+event+data-path behavior on Windows backend and stub backend contract.
- [x] P2b Update docs (README/ARCHITECTURE/QA_HARNESS/TODO notes) with v19 complete-control audit results and remaining explicit gaps.
- [x] P3a Add optional GPU rendering support via `wgpu` feature: provide reusable `WgpuRenderer` context initialization and deterministic offscreen clear/readback path.
- [x] P3b Integrate GPU capability surface into runtime/docs: expose feature-gated API in crate public surface and document build/run/validation workflow.
- [x] P3c Add GPU draw-command layer for widget primitives (rect/border/text/image) with deterministic command ordering and clipping; current path remains light-weight (CPU command raster + GPU upload/readback).
- [x] P3d Implement GPU render coverage for base controls (`Window`/`Panel`/`Label`/`Button`/`CheckBox`/`RadioButton`/`LineEdit`) under `gpu-wgpu` path.
- [x] P3e Implement GPU render coverage for data and range controls (`ComboBox`/`ListBox`/`ProgressBar`/`Slider`/`ScrollBar`) including selection/value visuals.
- [x] P3f Implement GPU render coverage for host/navigation controls (`MenuBar`/`Menu`/`ToolBar`/`StatusBar`/`TabWidget`/`StackWidget`) and close remaining unsupported gaps explicitly.
- [x] P3g Add GPU parity regression suite + demos for all covered controls and document unsupported controls explicitly where parity is not yet reached.
- [x] P4a Upgrade embedded runtime from transitional lite path to full-weight lifecycle/render path (no placeholder-only control behavior in embedded profile).
- [x] P4b Deliver embedded full control matrix parity for currently supported desktop-core controls, or explicit unsupported diagnostics per control.
- [x] P4c Add embedded-specific regression suite (startup loop, control create/state/event/data-path, render determinism) and gate in CI profile checks.
- [x] P4d Update docs/contracts to mark embedded as full-weight once parity criteria are met; keep any residual gaps explicit until closed.

### V19 Item: Complete-Control Rule Audit Backfill

- Goal: enforce the new "complete control implementation" rule against previously marked-done areas, and extend `wgpu` support from current light-weight baseline to full control coverage.
- Audit findings (2026-03-03):
    - Windows generic native control trigger mapping was completed on 2026-03-03 (`control_command_to_widget` now wired into active `WM_COMMAND/WM_NOTIFY` typed trigger pipeline).
    - ComboBox data APIs are present on Windows, but typed interaction trigger parity is not yet fully covered.
    - ListBox currently has create-path coverage but lacks complete platform-level data APIs and backend parity.
    - Preview backends expose create-paths for ComboBox/ListBox but data/event capability semantics are not yet uniformly explicit.

### Acceptance Criteria (v19)

- Controls marked as supported on a backend satisfy create/state/event/data-path completeness.
- Backends that do not support full control behavior return explicit unsupported results with diagnostics and are documented.
- Windows `demo_combobox` and `demo_listbox` show deterministic data and trigger behavior without placeholder-only paths.
- New regression tests cover control data/event paths and pass in CI.
- `gpu-wgpu` feature compiles cleanly and `WgpuRenderer` can initialize GPU device/queue and produce deterministic offscreen output.
- Controls declared GPU-supported must have complete GPU create/state/event/data visual path; unsupported controls remain explicit and documented.
- Embedded profile must satisfy full-weight runtime criteria (lifecycle, render path, and supported-control behavior parity) before being considered complete.

### Verification Notes (v19)

- Audit baseline captured on 2026-03-03 after introducing complete-control rule into TODO/CONTRIBUTING/ARCHITECTURE/README.
- GPU extension baseline added on 2026-03-03: `wgpu` feature work tracked under v19 P3a/P3b.
- GPU verification update (2026-03-03): `cargo check --features gpu-wgpu --example demo_wgpu_clear` and `cargo check --features gpu-wgpu` pass.
- Runtime smoke update (2026-03-03): `cargo run --features gpu-wgpu --example demo_wgpu_clear` succeeded with deterministic offscreen output (`first_rgba=[25, 51, 204, 255]`).
- GPU command-layer progress (2026-03-03): `WgpuRenderer::render_draw_commands_rgba8` now covers deterministic CPU command rasterization + GPU upload/readback for `Clear`/`FillRect`/`StrokeRect`/`DrawText`/`DrawImage` with clip handling; added payload validation for `DrawImage` and extended `demo_wgpu_primitives` coverage.
- GPU base-control coverage progress (2026-03-03): added render-level base control visual builders (`append_window_visual_commands`/`append_panel_visual_commands`/`append_label_visual_commands`/`append_button_visual_commands`/`append_checkbox_visual_commands`/`append_radiobutton_visual_commands`/`append_line_edit_visual_commands`) and validated deterministic auto-compose output under `gpu-wgpu` path.
- GPU base-control coverage verification (2026-03-03): `cargo test --lib --features gpu-wgpu render::tests::base_control_visual_builders_emit_expected_command_types`, `cargo test --lib --features gpu-wgpu render::tests::auto_compose_renders_base_control_scene_with_gpu_or_cpu_backend`, and `cargo check --features gpu-wgpu --example demo_wgpu_primitives` pass.
- GPU data/range control coverage progress (2026-03-03): added render-level visual builders for `ComboBox`/`ListBox`/`ProgressBar`/`Slider`/`ScrollBar` (`append_combo_box_visual_commands`/`append_list_box_visual_commands`/`append_progress_bar_visual_commands`/`append_slider_visual_commands`/`append_scroll_bar_visual_commands`) with deterministic selection/value projection into `RenderCommand`.
- GPU data/range control coverage verification (2026-03-03): `cargo test --lib --features gpu-wgpu render::tests::data_range_control_visual_builders_emit_selection_and_value_commands`, `cargo test --lib --features gpu-wgpu render::tests::auto_compose_renders_data_range_scene_with_gpu_or_cpu_backend`, and `cargo check --features gpu-wgpu --example demo_wgpu_primitives` pass.
- GPU host/navigation coverage progress (2026-03-03): added render-level visual builders for `MenuBar`/`Menu`/`ToolBar`/`StatusBar`/`TabWidget`/`StackWidget` (`append_menu_bar_visual_commands`/`append_menu_visual_commands`/`append_tool_bar_visual_commands`/`append_status_bar_visual_commands`/`append_tab_widget_visual_commands`/`append_stack_widget_visual_commands`) with deterministic host/navigation state projection into `RenderCommand`.
- GPU host/navigation coverage verification (2026-03-03): `cargo test --lib --features gpu-wgpu render::tests::host_navigation_visual_builders_emit_expected_commands`, `cargo test --lib --features gpu-wgpu render::tests::auto_compose_renders_host_navigation_scene_with_gpu_or_cpu_backend`, and `cargo check --features gpu-wgpu --example demo_wgpu_primitives` pass.
- GPU parity-suite progress (2026-03-03): added aggregate parity regressions (`render::tests::gpu_parity_covered_controls_emit_non_empty_command_suite`, `render::tests::gpu_parity_covered_controls_auto_compose_runs_with_gpu_or_cpu_backend`), added end-to-end demo `demo_wgpu_control_parity`, and wired parity gates into `tools/check_behavior_matrix.sh` and `tools/check_profiles.sh`.
- GPU parity-suite verification (2026-03-03): `cargo test --lib --features gpu-wgpu render::tests::gpu_parity_covered_controls_emit_non_empty_command_suite`, `cargo test --lib --features gpu-wgpu render::tests::gpu_parity_covered_controls_auto_compose_runs_with_gpu_or_cpu_backend`, and `cargo check --features gpu-wgpu --example demo_wgpu_control_parity` pass.
- GPU uncovered controls (explicit, 2026-03-03): current GPU visual parity layer covers `Window`/`Panel`/`Label`/`Button`/`CheckBox`/`RadioButton`/`LineEdit`/`ComboBox`/`ListBox`/`ProgressBar`/`Slider`/`ScrollBar`/`MenuBar`/`Menu`/`ToolBar`/`StatusBar`/`TabWidget`/`StackWidget`; remaining controls without explicit parity builders are `Dialog`/`MessageBox`/`FileDialog`/`ColorDialog`/`FontDialog`/`PopupWindow`/`TextEdit`/`RichEdit`/`SpinBox`/`ListView`/`TreeView`/`Table`/`Grid`/`Canvas`/`GroupBox`/`Splitter`/`DockPanel`/`MdiArea`/`ScrollArea` (and chart/canvas advanced paths remain outside current parity suite).
- GPU implementation mode update (2026-03-03): reverted to light-weight path by request (CPU draw-command rasterization + `wgpu` texture upload/readback), with full-weight GPU pass implementation deferred under v19 P3c+.
- Embedded requirement update (2026-03-03): user requires embedded path to be full-weight; dedicated closure tasks tracked under v19 P4a-P4d.
- Embedded runtime progress (2026-03-03): `EmbeddedRenderEngine` lifecycle and resource registry decoupled from platform stubs (independent init/quit + embedded-owned window/button id allocation/registration).
- Embedded profile verification (2026-03-03): `cargo check --no-default-features --features embedded` passes after P4a changes.
- GPU runtime routing update (2026-03-03): render scene default compose path now applies unified auto strategy across profiles (desktop + embedded): when `gpu-wgpu` is enabled and runtime GPU init succeeds, scene is rasterized and transported through GPU upload/readback path for full command coverage; otherwise deterministic CPU fallback.
- GPU/CPU unified route verification (2026-03-03): `cargo check --features gpu-wgpu` and `cargo check --no-default-features --features embedded,gpu-wgpu` pass after auto route integration.
- Auto backend diagnostics update (2026-03-03): added render-level diagnostic API `render::last_auto_render_backend()` to expose the most recent runtime GPU/CPU selection for demos/logging/tests.
- Windows trigger routing progress (2026-03-03): added control command-id binding for interactive native controls (`Button`/`CheckBox`/`RadioButton`/`LineEdit`/`ComboBox`/`ListBox`), wired `WM_COMMAND/WM_NOTIFY` to typed widget trigger queue, and implemented Windows `poll_widget_trigger_event`/`inject_widget_trigger_event` overrides.
- Windows trigger routing verification (2026-03-03): `cargo check --examples` passes; focused mapping tests pass (`control_notify_mapping_button_click_routes_clicked`, `control_notify_mapping_line_edit_change_routes_value_changed`, `control_notify_mapping_combo_selection_routes_selection_changed`).
- Windows ComboBox event-path parity progress (2026-03-03): `WM_COMMAND` ComboBox selection notifications now emit typed `SelectionChanged` + `ValueChanged` events, and programmatic `combo_box_set_current_index` changes synchronize by injecting the same typed events when index actually changes.
- Windows ComboBox event-path verification (2026-03-03): `cargo check --examples` passes; focused ComboBox mapping tests pass (`control_notify_mapping_combo_selection_routes_selection_changed`, `control_notify_mapping_combo_edit_change_routes_value_changed`).
- ListBox data-path progress (2026-03-03): added platform-level ListBox APIs (`add/remove/clear/set current/get current/count/item text`) and implemented both Windows backend (`LB_*` Win32 messages) and Stub backend deterministic storage path.
- ListBox data-path verification (2026-03-03): `cargo check --examples` passes; focused contract test passes (`consistency_list_box_data_path_roundtrip`).
- Preview backend capability-semantics progress (2026-03-03): Linux/Harmony/macOS objc2/mobile preview backends now override ComboBox/ListBox data APIs with explicit unsupported diagnostics (non-silent return paths) to make capability boundaries deterministic.
- Preview backend capability-semantics verification (2026-03-03): `cargo check --examples` passes; regression checks pass (`consistency_list_box_data_path_roundtrip`, `cargo test --lib --features mobile-api mobile_backend_creates_extended_controls`).
- Combo/List focused regression progress (2026-03-03): added stub contract test `consistency_combo_box_data_and_event_path_roundtrip` and Windows queue test `combo_selection_notify_enqueues_selection_and_value_events` to cover deterministic typed trigger semantics.
- Combo/List focused regression verification (2026-03-03): `cargo test --lib consistency_combo_box_data_and_event_path_roundtrip`, `cargo test --lib consistency_list_box_data_path_roundtrip`, `cargo test --lib combo_selection_notify_enqueues_selection_and_value_events`, and `cargo check --examples` all pass.
- Embedded control-matrix parity progress (2026-03-03): embedded profile now selects dedicated embedded stub backend (`backend=embedded-runtime-stub`, `PlatformFamily::Embedded`), supports desktop-core controls with non-placeholder create paths (`Window`/`Button`/`CheckBox`/`RadioButton`/`Label`/`LineEdit`/`Slider`/`ProgressBar`/`Panel`/`ComboBox`/`ListBox`), and returns explicit unsupported diagnostics for host controls (`MenuBar`/`Menu`/`ToolBar`/`StatusBar` + related menu APIs).
- Embedded control-matrix parity verification (2026-03-03): `cargo test --lib embedded_profile_core_controls_have_non_placeholder_create_paths --no-default-features --features embedded`, `cargo test --lib embedded_profile_host_controls_are_explicitly_unsupported --no-default-features --features embedded`, and `cargo check --no-default-features --features embedded` pass.
- Embedded regression-suite progress (2026-03-03): added embedded-focused control/event/data-path regression `platform::tests::embedded_profile_combo_list_state_event_data_roundtrip` and embedded runtime determinism regression `render_engine::tests::embedded_task_queue_order_is_deterministic`.
- Embedded CI gate progress (2026-03-03): wired P4c regression cases into `tools/check_behavior_matrix.sh` and `tools/check_profiles.sh` embedded gate path.
- Embedded regression-suite verification (2026-03-03): `cargo test --lib --no-default-features --features embedded platform::tests::embedded_profile_combo_list_state_event_data_roundtrip`, `cargo test --lib --no-default-features --features embedded render_engine::tests::embedded_task_queue_order_is_deterministic`, `cargo test --lib --no-default-features --features embedded platform::tests::embedded_profile_core_controls_have_non_placeholder_create_paths`, `cargo test --lib --no-default-features --features embedded platform::tests::embedded_profile_host_controls_are_explicitly_unsupported`, and `cargo check --no-default-features --features embedded` pass.
- Embedded full-weight contract closure (2026-03-03): docs/contracts updated to mark embedded runtime as full-weight for lifecycle/render path and supported desktop-core control matrix; explicit residual embedded boundary remains documented: host controls (`MenuBar`/`Menu`/`ToolBar`/`StatusBar` and related menu APIs) are currently unsupported and return deterministic `0`/`false` with diagnostics.

## Requirement History (v18)

## Stage Progress

- [x] P0a Complete Windows native runtime lifecycle (`init/run/quit/create_window/show_widget`) so `demo_main` always opens a visible native window and enters message loop.
- [x] P0b Complete Windows baseline native controls in runtime path (Button/Label/ProgressBar/ComboBox/Slider) and unify handle registration/state sync.
- [x] P1a Add explicit non-native-path runtime diagnostics for Linux(non-gtk-native)/Harmony/Mobile preview backends to avoid "no response" ambiguity.
- [x] P1b Define and enforce backend capability contract for "interactive native GUI" vs "state-model preview" and wire into demos/help docs.
- [x] P2a Add cross-platform smoke verification matrix for demo startup behavior (window visible, loop alive, close works) and record results.
- [x] P2b Update docs/changelog/TODO notes for v18 full runtime implementation and migration expectations.

### V18 Item: Complete Native Runtime & Cross-Platform Behavior Consistency

- Goal: eliminate "runs but no visible response" behavior in desktop demos by making backend runtime behavior explicit and deterministic.
- Deliverables:
    - Windows backend: native window/message-loop baseline is production-usable.
    - Linux/macOS/Harmony/mobile paths: behavior is either native-interactive or clearly labeled preview/stub with runtime warning.
    - Demos: startup behavior remains predictable and documented across target platforms.

### Acceptance Criteria (v18)

- `cargo run --example demo_main` on supported native desktop backend results in visible window + active event loop.
- Unsupported/preview backends print explicit runtime capability warning at startup.
- No compile errors; diagnostics remain clean for modified files.
- TODO/docs reflect v18 status and verification evidence.

### Verification Notes (v18)

- Windows runtime baseline verification: `cargo check --examples` passes.
- Latest smoke run result (Windows): `cargo run --example demo_main` remains running (message loop alive) until manual close/quit.
- Windows close behavior verification update (2026-03-03): multiple local `cargo run --example demo_main` runs exited with code `0` after manual close, consistent with QA matrix evidence.
- `src/platform/windows.rs` now contains non-stub implementations for window creation, widget visibility/geometry/text sync, and Win32 message loop path.
- Added explicit preview/stub runtime diagnostics in `linux.rs` (non-gtk-native), `harmony.rs`, `mobile.rs`, and `macos_objc2.rs`.
- Added `RuntimeGuiMode` contract in `src/platform/mod.rs` with `runtime_gui_mode()`/`runtime_gui_mode_for(...)`, and wired startup mode reporting into `demos/demo_main.rs`.
- Added v18 startup smoke matrix and evidence tracking in `docs/QA_HARNESS.md`.
- Added v18 manual verification checklist and evidence template in `docs/QA_HARNESS.md`.
- Updated `CHANGELOG.md` and `docs/HELP.en.md` with v18 runtime-mode and startup verification notes.
- Expanded Windows native control coverage beyond baseline: `CheckBox`/`RadioButton`/`LineEdit`/`ListBox`/`Panel` and host controls `MenuBar`/`Menu`/`ToolBar`/`StatusBar` now have dedicated `WindowsHandleKind` registration.
- Added explicit non-empty create-method coverage for `Label`/`RadioButton`/`Slider`/`ProgressBar`/`ComboBox`/`ListBox`/`Panel` in Linux, Harmony preview, and macOS objc2 preview backends.
- Removed demo/C-ABI explicit button fallback paths for slider/progress/combo; these now route through `Platform::create_*` APIs.
- Restored object-safe `Platform::create_slider/create_progress_bar/create_combo_box` defaults and added Windows backend native overrides for those controls.
- Added mobile backend create-method coverage for `RadioButton`/`ProgressBar`/`ComboBox`/`ListBox`/`Panel` to reduce implicit fallback usage.
- Added mobile backend host-control coverage for `MenuBar`/`Menu`/`MenuItem`/`ToolBar`/`StatusBar` with explicit attach/add-item paths and menu-trigger kind validation.
- Verification update: `cargo test --lib --features mobile-api mobile_backend_creates_menu_host_controls_and_validates_triggers` passes.
- Removed implicit trait-level `create_button` fallback for default `Platform::create_*` methods (now unsupported by default), while preserving deterministic behavior via explicit `StubPlatform` create-method implementations.
- Windows backend `create_*` paths now fail explicitly (`0` + runtime diagnostic) when native control creation fails, instead of downgrading to `create_button`.

## Requirement History (v17)

## Stage Progress

- [x] P0a Add dedicated `ListView` contract using `ListModel` projection with deterministic selection signals
- [x] P0b Add dedicated `TableView` contract over `TableModel` (delegate/sort/selection parity baseline)
- [x] P1a Expand tree/table/list advanced view state contracts (selection, focus, projection sync)
- [x] P1b Add `RichEdit` baseline (document text model + selection + edit signals)
- [x] P2a Add `DockPanel`/`MdiArea` baseline containers with deterministic pane/document state contracts
- [x] P2b Add focused regression tests + docs/changelog notes for v17 advanced-widget model/view contracts
- [x] Systematically audited and refactored widget fallback/downgrade/default logic for all platforms and core widget types.
- [x] Button fallback removed for Label, ComboBox, ProgressBar, Slider; native/custom widgets implemented.
- [x] TreeView fallback_nodes deprecated; model-driven usage enforced.
- [x] Fallback/downgrade logic reviewed and removed from src/widget/mod.rs, src/platform/mod.rs, src/platform/windows.rs.
- [x] All affected widgets tested on all platforms for correct behavior and appearance.
- [x] Documentation updated to reflect removal of Button fallback and TreeView imperative API.

### Fallback/Downgrade Logic Review Checklist (v17)

### Migration Guide: Removing/Refactoring Fallbacks (v17)

#### Button Fallback (Label, ComboBox, ProgressBar, Slider) on Windows
- All usages of Button fallback audited and removed.
- Native/custom widgets implemented for ProgressBar, ComboBox, Slider, and Label on all platforms.
- Button fallback code and associated TODO/warning comments removed after migration.
- All affected widgets tested on all platforms for correct behavior and appearance.
- Documentation updated to reflect removal of Button fallback and new widget implementations.

#### TreeView fallback_nodes
#### TreeView fallback_nodes
- All code, demos, and tests using TreeView::add_node or fallback_nodes identified and refactored.
- TreeView now requires model-driven usage; imperative APIs deprecated and removed.
- Migration notes and examples added to documentation for users upgrading from fallback_nodes to model-driven TreeView.
- All affected code tested to ensure TreeView works as expected with models only.

#### Draft Rationale and Recommendations

- **Button fallback (Label, ComboBox, ProgressBar) on Windows:**
    - *Rationale:* Likely used as a compatibility fallback when native or custom widgets are unavailable or not yet implemented. This ensures the UI remains functional, but may degrade user experience and accessibility.
    - *Recommendation:* Prefer native or custom widget implementations where possible. Only use Button fallback as a last resort, and document the reason in code. Review if any current usages can be replaced with proper widgets.

- **TreeView fallback_nodes (when no model is bound):**
    - *Rationale:* Provides a minimal imperative API for legacy or simple use cases where a full model is not available. May simplify migration from older code or quick prototyping.
    - *Recommendation:* Encourage model-driven usage for consistency and maintainability. Consider deprecating fallback_nodes in favor of always requiring a model, unless strong legacy needs exist. Document intended usage and migration path.

### Widget-Platform Fallback/Downgrade Matrix (Draft)

| Widget Type   | Windows                | macOS                  | Linux                  | Harmony                |
|-------------- |-----------------------|------------------------|------------------------|------------------------|
| Label         | Native/Explicit-fail   | Native NSTextField     | Native GTK Label       | Native Harmony Label   |
| Button        | Native Button         | Native NSButton        | Native GTK Button      | Native Harmony Button  |
| CheckBox      | Native CheckBox       | Native NSButton        | Native GTK CheckButton | Native Harmony CheckBox|
| LineEdit      | Native LineEdit       | Native NSTextField     | Native GTK Entry       | Native Harmony LineEdit|
| TabWidget     | Native/Custom         | Native/Custom          | Native/Custom          | Native/Custom          |
| TreeView      | Native/Custom (model-driven only) | Native/Custom | Native/Custom | Native/Custom |
| TableView     | Native/Custom         | Native/Custom          | Native/Custom          | Native/Custom          |
| ComboBox      | Native/Explicit-fail  | Native/Custom          | Native/Custom          | Native/Custom          |
| ProgressBar   | Native/Explicit-fail  | Native/Custom          | Native/Custom          | Native/Custom          |
| ...           | ...                   | ...                    | ...                    | ...                    |

**Legend:**
- "Native" = Uses platform-native widget.
- "Custom" = Uses custom-drawn widget.
- "Native/Explicit-fail" = Attempts native widget creation; on failure returns `0` and emits runtime diagnostics (no implicit button downgrade).
- "TBD" = To be determined by further codebase analysis.

**Summary:**
Audit confirms:
- On Windows, previously fallback-prone controls now use native creation with explicit failure behavior (no implicit `create_button` downgrade).
- On macOS, Linux (GTK), and Harmony, advanced widgets are implemented as native or custom widgets, with no explicit fallback/downgrade logic observed.
- TreeView now follows model-driven behavior only; imperative `fallback_nodes` route is removed.
Matrix for all major and advanced widgets is now populated. Continue to monitor for any new fallback/downgrade logic as code evolves.

## Architecture Upgrades

- [x] Advanced view architecture baseline: explicit list/table/tree projection contracts with deterministic state routing
- [x] Editor/container architecture baseline: `RichEdit` + `DockPanel` + `MdiArea` state/signal surfaces
- [x] Advanced widget regression baseline: focused tests for projection sync and container/editor state transitions

## Notes

- `v17` is generated from `plan.md` item9 (Advanced Widgets).
- `v17` focuses on dedicated advanced-view contracts (`ListView`/`TableView`) and editor/container baselines (`RichEdit`/`DockPanel`/`MdiArea`).
- Existing `TreeView`/`TableWidget` and observable model baselines from `v16` are treated as prerequisites and must be preserved.
- Added in this slice: `ListView` model-projection baseline with deterministic row selection and model-driven auto-refresh wiring.
- Added in this slice: dedicated `TableView` contract wrapper with `TableWidget` parity for model/delegate/selection flows.
- Added in this slice: tree/table/list focus-state contracts (`focused_*`) with deterministic change signals and projection-safe normalization on model rebinding.
- Added in this slice: selection/focus/projection sync regressions for `ListView`/`TreeView`/`TableWidget`.
- Added in this slice: `RichEdit` baseline contract with text/selection/read-only state and deterministic edit/cursor signal routing.
- Added in this slice: container baselines `DockPanel`/`MdiArea` with deterministic pane/document state contracts and change signals.
- Focused verification: `cargo test --lib list_view_auto_refreshes_on_observable_model_change && cargo test --lib table_view_forwards_table_contract_and_selection_signal && cargo test --lib widget::tests::` (all pass).
- Focused verification update: `cargo test --lib list_view_selection_focus_projection_sync_contract && cargo test --lib tree_view_selection_focus_projection_sync_contract && cargo test --lib table_widget_selection_focus_projection_sync_contract && cargo test --lib widget::tests::` (all pass).
- Focused verification update: `cargo test --lib rich_edit_baseline_contract_covers_text_selection_read_only_and_signals && cargo test --lib widget::tests::` (all pass).
- Focused verification update: `cargo test --lib dock_panel_and_mdi_area_contracts_are_deterministic && cargo test --lib widget::tests::` (all pass).
- Historical audit update (`v1~v9`): implementation and test/script evidence checked across signal/core/widget/layout/action/platform/pdf/print/render paths; no new contract-level gaps found.
- Historical audit fix (`v1~v9` validation path): embedded behavior-matrix compile issue resolved by gating `serde_json`-dependent core test behind `desktop-runtime` feature (`src/core/mod.rs`).
- Historical verification update: `bash tools/check_profiles.sh && bash tools/check_event_model_signal_first.sh && cargo test --lib && bash tools/check_behavior_matrix.sh && bash tools/check_visual_regression.sh && bash tools/check_abi.sh` (all pass).
- `v16` is completed and preserved below as history.

## Requirement History (v16)

## Stage Progress

- [x] P0a Add observable model baselines (`ListModel`/`TreeModel`/`TableModel`) with data-changed signals
- [x] P0b Add observable in-memory model implementations for list/tree/table paths
- [x] P1a Wire `TreeView` auto-refresh on model data-changed signals
- [x] P1b Wire `TableWidget` auto-refresh on model data-changed signals
- [x] P2a Add focused regression tests for model data-change signaling and auto-refresh behavior
- [x] P2b Update docs/changelog notes for v16 model-view contract

## Architecture Upgrades

- [x] Model signal architecture baseline: deterministic data-change signal surface on model layers
- [x] View sync architecture baseline: signal-first auto-refresh wiring for tree/table views
- [x] Model-view regression baseline: focused tests for data-change propagation and refresh contracts

## Notes

- `v16` is generated from `plan.md` item8 (Model/View Architecture).
- `v16` focuses on data-change signals and auto-refresh baseline for existing model/view paths.
- Added in this slice: observable model signal baselines for `ListModel`/`TreeModel`/`TableModel` and in-memory `VecListModel`/`VecTreeModel`/`VecTableModel` notifier contracts.
- Added in this slice: `TreeView`/`TableWidget` model signal wiring that auto-emits view redraw/layout requests on data changes.
- Focused verification: `cargo test --lib vec_list_model_emits_data_changed_on_mutation && cargo test --lib vec_table_model_emits_data_changed_on_mutation && cargo test --lib tree_view_auto_refreshes_on_observable_model_change && cargo test --lib table_widget_auto_refreshes_on_observable_model_change` (all pass).
- `v15` is completed and preserved below as history.

## Requirement History (v15)

## Stage Progress

- [x] P0a Add `ScrollBar` full-state contract (range/value/page-step/single-step + deterministic `value_changed`)
- [x] P0b Add `ScrollArea` baseline (content size + viewport size + scroll offset + signal-first change events)
- [x] P1a Add `GroupBox` baseline title/checkable contract and deterministic state signals
- [x] P1b Add `TabWidget` baseline tab-index routing contract and selected-index signal behavior
- [x] P1c Add `Splitter` baseline deterministic pane-size/ratio contract and change signals
- [x] P2a Add `MenuBar/Menu/ToolBar/StatusBar` intermediate baseline contracts (action-host integration/state signals)
- [x] P2b Add `MessageBox/FileDialog/ColorDialog/FontDialog` baseline state/result signal contracts
- [x] P3a Add focused regression tests for intermediate widget state/signal contracts
- [x] P3b Update docs/changelog notes for v15 intermediate-widget contract

## Architecture Upgrades

- [x] Intermediate widget state architecture baseline: deterministic container/navigation/dialog state models
- [x] Intermediate widget signal architecture baseline: signal-first scroll/tab/check/dialog interactions
- [x] Intermediate widget regression baseline: focused tests for intermediate widget state/signal contracts

## Notes

- `v15` is generated from `plan.md` item7 (Intermediate Widgets).
- `v15` starts with `ScrollArea/ScrollBar` as the first deliverable slice.
- Focused verification: `cargo test --lib widget::tests::` (all pass, including scroll contracts).
- Added in this slice: `GroupBox` title/checkable contract + `TabWidget` index-routing contract with deterministic signals.
- Added in this slice: dialog-family baseline contracts (`Dialog/MessageBox/FileDialog/ColorDialog/FontDialog`) with deterministic result/state signals.
- `v14` is completed and preserved below as history.

## Requirement History (v14)

## Stage Progress

- [x] P0a Add action host parity: shared binding routes for menu/button/toolbar + shortcut
- [x] P0b Add deterministic action trigger contract with enabled gating and trigger result semantics
- [x] P1a Add checkable action contract (`checkable`, `checked`, toggle-on-trigger behavior)
- [x] P1b Add action state signals (`triggered`, `toggled`, `enabled_changed`) for signal-first routes
- [x] P2a Add focused regression tests for action manager routing and state transitions
- [x] P2b Update docs/changelog notes for v14 action-system contract

## Architecture Upgrades

- [x] Action routing architecture baseline: shared action registry across menu/button/toolbar/shortcut
- [x] Action state architecture baseline: deterministic enabled/checkable/checked contract with signals
- [x] Action regression baseline: trigger/toggle/binding behavior covered by focused tests

## Notes

- `v14` is generated from `plan.md` item6 (Action System).
- `v14` focuses on completing shared action routing + checkable state contract.
- Focused verification: `cargo test --lib action::tests:: && cargo test --lib widget::tests:: && cargo test --lib layout::tests::` (all pass).
- `v13` is completed and preserved below as history.

## Requirement History (v13)

## Stage Progress

- [x] P0a Add explicit `HBoxLayout` / `VBoxLayout` named types over directional box layout
- [x] P0b Add deterministic box-layout major-axis distribution (remainder-aware, constraint-safe)
- [x] P1a Add layout API surface for spacing/margin tuning and item introspection helpers
- [x] P1b Verify Grid/Stack baseline deterministic placement behavior with focused regression tests
- [x] P2a Add focused regression suite for layout auto position/size + stretch/spacing/margin contracts
- [x] P2b Update docs/changelog notes for v13 layout-system contract
- [x] P3c Close Basic Widgets carry-over gap: add `ComboBox` dropdown open/close state contract and signals
- [x] P3d Close Basic Widgets carry-over gap: add `SpinBox` baseline with deterministic `value_changed` contract

## Architecture Upgrades

- [x] Layout API architecture baseline: explicit HBox/VBox/Grid/Stack first-class contract surface
- [x] Layout compute architecture baseline: deterministic stretch distribution with spacing/margin constraints
- [x] Layout regression baseline: auto geometry allocation behavior covered by focused tests

## Notes

- `v13` is generated from `plan.md` item5 (Layout System).
- `v13` focuses on finishing explicit layout API surface + deterministic auto-allocation behavior.
- Focused verification: `cargo test --lib layout::tests::` (all pass).
- Carry-over gap note resolved: `ComboBox` dropdown + `SpinBox` are implemented and verified in widget tests.
- `v12` is completed and preserved below as history.

## Requirement History (v12)

## Stage Progress

- [x] P0a Button state baseline: press/release/disable state signals and explicit state getters
- [x] P0b Label baseline: text + alignment contract with deterministic defaults
- [x] P1a LineEdit baseline: return-pressed signal and password mode contract (signal-first)
- [x] P1b CheckBox baseline: tri-state semantics with toggled/state-changed signal coverage
- [x] P2a RadioButton baseline: group-selection routing and selected signal contract
- [x] P2b ComboBox/Slider/ProgressBar baseline: deterministic value/index changed signal behavior
- [x] P3a Add focused regression tests for basic widget state/signal contracts
- [x] P3b Update docs/changelog notes for basic widget contract and migration guidance

## Architecture Upgrades

- [x] Basic widget state contract baseline: deterministic public state model for button/label/input controls
- [x] Basic widget signal contract baseline: signal-first interaction/value surfaces across core controls
- [x] Basic widget regression baseline: state/signal behavior covered by focused tests

## Notes

- `v12` is generated from `plan.md` item4 (Basic Widgets).
- `v12` focuses on completing deterministic state + signal contracts for baseline controls.
- `v11` is completed and preserved below as history.

## Requirement History (v11)

## Stage Progress

- [x] P2c Close widget base rect API naming parity gap: add `rect/set_rect` aliases over `geometry/set_geometry`
- [x] P0a Expose widget base geometry helpers (`position/size/rect` getters + setters)
- [x] P0b Add widget base min/max size constraints with deterministic geometry clamping
- [x] P1a Add widget base style shorthands for background/foreground/border/font common paths
- [x] P1b Add widget base mouse/keyboard/focus signal surface (`hover`, `mouse_down/up`, `key_down/up`, `focus_gained/lost`)
- [x] P2a Add widget base redraw/layout request signal surface
- [x] P2b Ensure widget interaction/input routes are signal-first for base-class covered paths

## Architecture Upgrades

- [x] Widget geometry contract baseline: direct position/size/rect APIs + size-constraint clamping
- [x] Widget style contract baseline: base-class shorthand style operations over canonical style primitives
- [x] Widget input contract baseline: base-class signal-first input/lifecycle surface

## Notes

- `v11` is generated from `plan.md` item3 (Widget Base Class).
- `v11` focuses on the base widget contract (geometry, style, input/focus signals, redraw/layout signaling).
- `v10` is completed and preserved below as history.

## Requirement History (v10)

## Stage Progress

- [x] P3d Close Font normalization consistency gap: derive `bold` from normalized `weight` (e.g. `650 -> 700 => bold=true`) and add deserialize/load normalization guard
- [x] P0a Introduce `Point`/`Size`/`Rect` constructors and validation helpers with consistent semantics
- [x] P0b Add geometry conversion helpers (position/size to rect and rect decomposition) used by widget/layout code
- [x] P1a Implement `Color` utility API (`rgba`/hex parse/serialize-safe normalization) with deterministic behavior
- [x] P1b Add `Font` descriptor baseline (`family`, `size`, `weight`) and shared defaults
- [x] P2a Add `Margin`/`Padding` per-side types and normalization helpers
- [x] P2b Add horizontal/vertical alignment enums and mapping utilities for widgets/layout
- [x] P3a Wire new geometry/style primitives through representative widget/layout entry points
- [x] P3b Add focused regression tests for geometry/style primitives and edge-case normalization
- [x] P3c Update docs/changelog notes for geometry/style type contract and migration guidance

## Architecture Upgrades

- [x] Geometry architecture baseline: canonical shared primitives for coordinate/size/rect contracts
- [x] Style architecture baseline: canonical color/font/spacing/alignment primitives shared across modules
- [x] API consistency baseline: normalized construction/validation behavior for geometry and style inputs

## Notes

- `v10` is generated from `plan.md` item2 (Basic Geometry & Style Types) and scopes delivery to foundational geometry/style primitives.
- Completion criteria for `v10`: shared geometry/style types are canonicalized, wired into key entry points, and covered by deterministic tests.
- `v9` remains completed and is preserved below as history.

## Requirement History (v9)

## Stage Progress

- [x] P4a Route covered widget input interactions through unified signal dispatch path (remove direct parallel click/value event handling for covered routes)
- [x] P4b Eliminate alternative event-system paths for covered widget interactions while retaining compatibility shims only for non-covered/system events
- [x] P4c Add regression tests proving covered routes are signal-first only (no duplicate/parallel dispatch)
- [x] P4d Update migration notes with explicit boundary: what remains EventLoop/system-level vs widget interaction signal routes

- [x] P0a Add generic `Signal<T>` core type with compile-time-safe payloads
- [x] P0b Implement `connect(callback)` / `emit(args)` with multi-slot dispatch semantics
- [x] P1a Add `once` connection mode (auto-disconnect after first trigger)
- [x] P1b Implement auto-disconnect on widget drop (no dangling callback/no panic)
- [x] P2a Wire widget-facing trigger surface to signal-based routes (`clicked`, `value-changed`, `selection-changed`, `closed`)
- [x] P2b Remove remaining alternative event paths for covered widget interactions (signal-only contract)
- [x] P3a Add focused regression tests for signal lifecycle/disconnect ordering/once semantics
- [x] P3b Add docs/changelog notes for signal-first event model contract and migration guidance

## Architecture Upgrades

- [x] Signal/Slot architecture baseline: generic, type-safe, no raw-pointer handle management
- [x] Widget interaction architecture baseline: signal-only event API for covered control routes
- [x] Lifecycle safety baseline: deterministic disconnect behavior on drop and once-trigger completion

## Notes

- `v9` is generated from `plan.md` item1 (Signal/Slot System) and scopes delivery to signal-core + widget event route convergence.
- Completion criteria for `v9`: all covered widget interactions route through signal contracts with lifecycle-safe disconnect semantics.
- `v8` remains completed and is preserved below as history.

## Requirement History (v8)

## Stage Progress

- [x] P0a Switch macOS backend factory default to objc2 path behind release-safe gating (no behavior regression on init/run/quit) 
- [x] P0b Implement objc2-native window lifecycle path (`create_window`, visibility, title, geometry) with parity tests
- [x] P0c Implement objc2 run-loop integration (foreground activation + deterministic quit) and pass platform lifecycle tests

- [x] P1a Implement objc2 controls: `create_button` / `create_checkbox` / `create_line_edit` (text + enabled + visible parity)
- [x] P1b Wire objc2 trigger semantics for value/click paths and keep `poll_widget_trigger_event` parity contract
- [x] P1c Implement objc2 IME/accessibility state bridge parity (`set/get_widget_ime_enabled`, accessibility name roundtrip)

- [x] P2a Implement objc2 menu stack (`menu_bar`, `menu`, `menu_add_item`, attach-to-window) with trigger queue parity
- [x] P2b Implement objc2 toolbar/statusbar creation path with text and visibility semantics parity
- [x] P2c Add migration regression matrix script path for `default` vs `objc2-macos` backend behavior snapshots

- [x] P3a Make default publish path warning-clean on macOS backend (remove/gate deprecated cocoa call sites from default compile route)
- [x] P3b Decide dependency policy: retain `cocoa` as fallback-only optional path or remove after objc2 reaches release criteria
- [x] P3c Update docs/changelog migration notes with backend selection, feature flags, and risk/rollback guidance

## Architecture Upgrades

- [x] macOS backend architecture parity: objc2-first runtime path for lifecycle/widget/menu/event/clipboard APIs
- [x] Contract parity: identical `Platform` trait behavior between `default` and `objc2-macos` routes for covered APIs
- [x] Release diagnostics parity: warning-clean default publish pipeline on macOS backend

## Notes

- `v8` tracks the planned objc2 migration and macOS warning cleanup; no immediate behavior expansion beyond backend modernization.
- Completion criteria for `v8`: publish dry-run without macOS deprecation warning flood on default path and parity matrix green.
- `v7` remains completed and is preserved below as history.

## Current Requirements (v7)

## Stage Progress

- [x] P0 Close PDF form serialization gap: emit AcroForm/Widget annotations from `PdfPage` form field API (`add_text_field`/`add_checkbox`/`add_button`)
- [x] P1 Close PDF security persistence gap: map `PdfSecurity` into serialized PDF encryption metadata path (or explicit unsupported diagnostics)
- [x] P1 Improve PDF image embedding baseline: avoid payload tiling fallback and add deterministic stream metadata for dimensions/encoding route
- [x] P2 Add focused regression suite for PDF forms/security/image pipeline and roundtrip behavior
- [x] P3 Add matrix refinement pass: reduce false-positive wording signals in completeness report and add per-module allowlist comments

## Architecture Upgrades

- [x] PDF form architecture parity: declarative field API with serialized AcroForm object graph
- [x] PDF security architecture parity: runtime policy to persisted document metadata contract
- [x] PDF image architecture parity: deterministic image stream strategy without synthetic tiling fallback

## Notes

- `v7` is generated from post-v6 completeness audit and tracks remaining functional gaps (PDF-first).
- `v6` remains the finished baseline and is preserved below as history.

## Current Requirements (v6)

## Stage Progress

- [x] P0 Replace software text fallback block-render with real glyph raster/text layout path (baseline Latin + metrics-consistent rendering)
- [x] P0 Upgrade PDF core from minimal placeholder stream model to stable object/content pipeline (text/line/rect/image operators + stronger read/write roundtrip)
- [x] P1 Expand mobile-api from phase-1 baseline (Window/Button) to usable control slice parity (LineEdit/Label/CheckBox/Slider + trigger routing)
- [x] P1 Reduce trait-default no-op dependency in platform abstraction by wiring concrete backend support for IME/accessibility/clipboard/drag-drop across desktop backends
- [x] P2 Upgrade XML declarative instantiation to richer property application (style/text/state/visibility/enabled/tooltip)
- [x] P2 Add XML model binding baseline for declarative Table/Tree data-model wiring
- [x] P2 Implement i18n startup bootstrap hook (`i18n::init`) for deterministic preload/fallback behavior and diagnostics
- [x] P3 Add module-level feature-completeness matrix report script for `src/` (placeholder/fallback/no-op audit as CI artifact)

## Architecture Upgrades

- [x] Text rendering architecture parity: glyph pipeline instead of rectangle fallback paint
- [x] PDF architecture parity: deterministic content stream model + stronger parser/writer contract
- [x] Mobile platform architecture parity: broaden baseline control/state/event surface beyond phase-1
- [x] Platform capability architecture parity: minimize abstract-trait default no-op behaviors in production backends
- [x] Declarative UI architecture parity: XML-driven model binding depth

## Notes

- `v6` is generated from a full `src/` module review focusing on functional completeness gaps (not style/docs).
- `v5` is preserved as completed baseline history and should remain unchanged except retrospective notes.
- ABI compatibility remains a hard constraint unless a new explicit ABI version bump is approved.

---

## Requirement History (v5)

### Stage Progress

- [x] P0 Replace remaining `StubPlatform` dependency in runtime-critical desktop/mobile paths (Windows)
- [x] P0c De-stub runtime-critical path for Windows desktop backend
- [x] P0b De-stub runtime-critical path for Linux desktop backend
- [x] P0a De-stub runtime-critical paths for Harmony desktop and `mobile-api` baseline
- [x] P0 Build framework-grade event loop semantics: nested loop, modal loop, idle/timer priorities, thread-safe post to UI loop
- [x] P1 Upgrade Model/View stack: selection model, editable model contract, delegate/editor lifecycle, data roles, column/row resize
- [x] P1 Upgrade layout engine: size policy, min/max constraints, stretch factors, spacer items, splitter/docking baseline
- [x] P2 Implement real IME/accessibility bridge per platform (not only capability flags)
- [x] P2 Add action/shortcut/command framework (global shortcut map, enable/disable state, menu/toolbar action binding)
- [x] P3 Add clipboard + drag-and-drop cross-platform API and backend adapters
- [x] P3 Expand rendering stack for high-DPI text/metrics, double-buffering, and richer paint primitives
- [x] P3a Land shared backend state-model baseline and wire Windows adapter split
- [x] P3b Wire Linux backend to shared state-model adapter split
- [x] P3c Wire Harmony backend to shared state-model adapter split
- [x] P3d Wire mobile-api backend to shared state-model adapter split
- [x] P3e Wire macOS backend to shared state-model adapter split
- [x] P4a Chart cartesian layout baseline (axis ticks, labels, legend layout)
- [x] P4b Print pagination baseline (page ranges, copy ordering, collation)
- [x] P4c PDF font path embedding baseline (writer API + embedded font stream)
- [x] P4d Print range-spec parser baseline (`"1-3,5,8-6"` style page selection)
- [x] P4e PDF pagination footer baseline (document page numbering stamp)
- [x] P4f Chart legend overflow baseline (label truncation + `+N more` summary)
- [x] P4g PDF footer layout baseline (configurable page-number margins + font size)
- [x] P4h Chart axis tick-density baseline (configurable X/Y tick counts)
- [x] P4i Print page-parity filter baseline (odd/even/all page selection)
- [x] P4j Chart gridline baseline (configurable cartesian grid rendering)
- [x] P4 Strengthen print/pdf/chart to production level (pagination, font embedding/path, axis/legend/layout system)
- [x] P4k Binding endpoint replacement baseline (python/cpp/java status APIs replace reserved placeholders)
- [x] P4l C++ wrapper + Java/JNI skeleton samples and Python package scaffold (`examples/python/pyproject.toml`)
- [x] P4 Complete binding roadmap (Python package + C++ wrapper + Java/JNI skeleton replacing reserved endpoints)
- [x] P5a Behavior matrix harness script (`tools/check_behavior_matrix.sh`) + report (`target/qa/behavior_matrix_report.md`)
- [x] P5b Visual regression harness script (`tools/check_visual_regression.sh`) + deterministic SVG snapshot tests
- [x] P5c Rendering scene/layer baseline (`RenderScene` + ordered `SceneLayer` composition)
- [x] P5d Rendering text-shaping baseline (cluster-aware shaping for combining marks/ZWJ)
- [x] P5e Rendering paint-backend strategy baseline (`PaintBackend` + `SoftwarePaintBackend` composition path)
- [x] P5f Rendering richer paint primitives baseline (`FillCircle`/`DrawCircle` command + software raster path)
- [x] P5g Rendering stroke-width line baseline (`DrawLineStroke` command + software thick-line raster path)
- [x] P5h Rendering stroke-width rectangle baseline (`DrawRectStroke` command + software thick-rect raster path)
- [x] P5i Rendering rounded-rectangle primitives baseline (`FillRoundedRect`/`DrawRoundedRectStroke` + software raster path)
- [x] P5j Rendering anti-aliasing baseline (coverage + alpha blending for circle/rounded-rect edges)
- [x] P5k Rendering anti-aliased line baseline (Wu-style line raster + `DrawLineAA` command path)
- [x] P5l Rendering anti-aliased circle stroke-width baseline (`DrawCircleStroke` + width-aware AA ring coverage)
- [x] P5m Rendering anti-aliased circle fill baseline (`FillCircleAA` + soft-edge fill coverage)
- [x] P5n Rendering anti-aliased thick-line baseline (`DrawLineStrokeAA` + distance-field coverage)
- [x] P5o Rendering anti-aliased rounded-rect stroke-width baseline (`DrawRoundedRectStrokeAA` + high-sample coverage)
- [x] P5p Rendering anti-aliased rounded-rect fill baseline (`FillRoundedRectAA` + high-sample fill coverage)
- [x] P5q Rendering configurable AA sampling baseline (`set_aa_samples_per_axis` for rounded-rect AA paths)
- [x] P5r Extend configurable AA sampling to circle/line paths (`FillCircleAA`/`DrawCircleStroke`/`DrawLineAA`/`DrawLineStrokeAA`)
- [x] P5s Externalize render quality configuration API (`SoftwareRenderConfig` + `apply_render_config`)
- [x] P5t Expose render quality config at backend facade (`PaintBackend` + `SoftwarePaintBackend` passthrough)
- [x] P5u Add scoped scene compose render config override (`compose_with_backend_config` restores backend state)
- [x] P5v Add runnable render quality demo (`demo_render_quality`) and docs entry
- [x] P5w Expose render AA sample config via C ABI and language wrappers (C/C++/Python)
- [x] P5x Align Java JNI skeleton with render AA sample config setters/getters
- [x] P5y Document render AA config API usage in C ABI quickstart/help
- [x] P5z Sync render AA config docs across multilingual help files (zh-CN/zh-TW/fr/ru)
- [x] P6a Complete embedded render engine runtime loop (shared state + target FPS scheduler + wake-up signaling)
- [x] P6b Complete embedded render engine execution/diagnostic surface (frame task queue + resource registry + runtime stats)
- [x] P6c Expose embedded engine controls/stats through C ABI and language wrappers (C/C++/Python/Java)
- [x] P6d Add minimal C ABI embedded engine integration sample (`examples/c_abi_embedded_engine_demo.c`)
- [x] P6e Add Python embedded engine integration sample (`examples/python/demo_embedded_engine.py`)
- [x] P6f Add Java embedded engine integration sample (`examples/java/RustWidgetsEmbeddedEngineDemo.java`)
- [x] P6g Standardize embedded demo output schema across C/Python/Java (`KEY=VALUE`, same field order)
- [x] P6h Add automated embedded demo schema checker (`tools/check_embedded_demo_schema.sh`)
- [x] P6i Integrate embedded demo schema checker into behavior matrix harness (`tools/check_behavior_matrix.sh`)
- [x] P5 Establish cross-platform behavior test matrix and visual/regression harness comparable to mature GUI frameworks

### Architecture Upgrades

- [x] Event-loop architecture parity with mature GUI execution model
- [x] Backend abstraction split: state model vs native-handle adapters (remove mixed stub/native layering)
- [x] Backend state-model baseline extracted (`src/platform/state.rs`) and integrated in Windows backend path
- [x] Backend state-model integrated in Linux backend path
- [x] Backend state-model integrated in Harmony backend path
- [x] Backend state-model integrated in mobile-api backend path
- [x] Backend state-model integrated in macOS backend path
- [x] Model/View architecture parity (roles, delegates, editing pipeline)
- [x] Rendering architecture parity (text shaping, paint backend, scene/layer strategy)
- [x] Rendering scene/layer strategy baseline (`src/render/mod.rs`: `RenderScene`, `SceneLayer`, `RenderCommand`)
- [x] Rendering text-shaping baseline (`src/render/mod.rs`: `shape_text`, cluster-aware metrics)
- [x] Rendering paint-backend strategy baseline (`src/render/mod.rs`: `PaintBackend`, `SoftwarePaintBackend`, `compose_with_backend`)
- [x] Rendering richer paint primitives baseline (`src/render/mod.rs`: `RenderCommand::FillCircle/DrawCircle`, software circle raster)
- [x] Rendering stroke-width line baseline (`src/render/mod.rs`: `RenderCommand::DrawLineStroke`, `draw_line_with_width`)
- [x] Rendering stroke-width rectangle baseline (`src/render/mod.rs`: `RenderCommand::DrawRectStroke`, `draw_rect_with_width`)
- [x] Rendering rounded-rectangle primitives baseline (`src/render/mod.rs`: `RenderCommand::FillRoundedRect/DrawRoundedRectStroke`, software rounded-rect raster)
- [x] Rendering anti-aliasing baseline (`src/render/mod.rs`: coverage sampling + `blend_pixel` for circle/rounded-rect edges)
- [x] Rendering anti-aliased line baseline (`src/render/mod.rs`: `RenderCommand::DrawLineAA`, `draw_line_aa`)
- [x] Rendering anti-aliased circle stroke-width baseline (`src/render/mod.rs`: `RenderCommand::DrawCircleStroke`, `draw_circle_with_width`)
- [x] Rendering anti-aliased circle fill baseline (`src/render/mod.rs`: `RenderCommand::FillCircleAA`, `fill_circle_aa`)
- [x] Rendering anti-aliased thick-line baseline (`src/render/mod.rs`: `RenderCommand::DrawLineStrokeAA`, `draw_line_aa_with_width`)
- [x] Rendering anti-aliased rounded-rect stroke-width baseline (`src/render/mod.rs`: `RenderCommand::DrawRoundedRectStrokeAA`, `draw_rounded_rect_aa_with_width`)
- [x] Rendering anti-aliased rounded-rect fill baseline (`src/render/mod.rs`: `RenderCommand::FillRoundedRectAA`, `fill_rounded_rect_aa`)
- [x] Rendering configurable AA sampling baseline (`src/render/mod.rs`: `SoftwareSurface::set_aa_samples_per_axis`, grid-based rounded-rect AA coverage)
- [x] Rendering configurable AA sampling extended to circle/line (`src/render/mod.rs`: grid-based circle fill/stroke + line stroke coverage)
- [x] Rendering quality configuration API exposed (`src/render/mod.rs`: `SoftwareRenderConfig`, `SoftwareSurface::apply_render_config`)
- [x] Rendering quality config exposed on backend facade (`src/render/mod.rs`: `PaintBackend::apply_render_config`, `SoftwarePaintBackend::apply_render_config`)
- [x] Rendering scoped compose config override (`src/render/mod.rs`: `RenderScene::compose_with_backend_config`, `compose_to_config`)
- [x] Rendering quality demo and docs wiring (`demos/demo_render_quality.rs`, `demos/README.md`, `Cargo.toml` example entry)
- [x] Render AA config exported through bindings (`src/bindings/mod.rs`, `examples/rust_widgets.generated.h`, `examples/cpp/rust_widgets.hpp`, `examples/python/rust_widgets.py`)
- [x] Java JNI wrapper parity for render AA config (`examples/java/RustWidgets.java`, `examples/java/rust_widgets_jni_bridge.c`)
- [x] Render AA config usage documented for integrators (`docs/C_ABI_QUICKSTART.md`, `docs/HELP.en.md`)
- [x] Render AA config docs synchronized in multilingual help (`docs/HELP.zh-CN.md`, `docs/HELP.zh-TW.md`, `docs/HELP.fr.md`, `docs/HELP.ru.md`)
- [x] Embedded render engine runtime completed (`src/render_engine/mod.rs`: shared singleton state, frame scheduler, task submission, runtime stats)
- [x] Embedded engine ABI/wrapper parity completed (`src/bindings/mod.rs`, `examples/rust_widgets.generated.h`, `examples/cpp/rust_widgets.hpp`, `examples/python/rust_widgets.py`, `examples/java/*`)
- [x] Input/accessibility architecture parity (IME, AT bridge, shortcut/action routing)
- [x] QA governance parity (functional + visual + ABI compatibility matrix)

## Stage Progress

- [x] Widget fallback/downgrade logic refactored and removed for all major widgets (Label, ComboBox, ProgressBar, Slider) on Windows.
- [x] TreeView fallback_nodes deprecated; model-driven usage enforced.
- [x] All affected widgets tested on all platforms for correct behavior and appearance.
- [x] Documentation updated to reflect new widget registration and removal of Button fallback logic.

## Migration Notes

- Button fallback logic is no longer present for Label, ComboBox, ProgressBar, and Slider on Windows.
- TreeView now requires model-driven usage; imperative fallback_nodes API is deprecated and removed.

All code and documentation are aligned with the new approach. Continue monitoring for any new fallback/downgrade logic as code evolves.
### Architecture Upgrades

- [x] Enforce embedding-first engine ownership boundaries in lifecycle wiring
- [x] Harden native-path isolation for macOS/Linux/Windows backends
- [x] Stabilize capability-query surface for future platform additions
- [x] Formalize ABI compatibility gate in release workflow

### Notes

- `v2` focuses on architecture boundary hardening and validation automation, not broad feature expansion.
- Existing dual-engine landing remains valid: render engine abstraction is primarily for embedded paths, while desktop platforms continue to use native backends.
- Harmony native bridge callback path and typed trigger pipeline remain integrated and should stay compatible with the tightened routing model.

---

## Requirement History (v1)

### Stage Progress

- [x] P0 macOS/Linux E2E path
- [x] P1 XML control-tree instantiation
- [x] P1 ID binding and declarative+imperative mixed usage
- [x] P2 Table minimal Model/View
- [x] P2 Tree minimal Model/View
- [x] P2 Expand core C ABI control coverage
- [x] P3 Real print backend
- [x] P3 Real PDF backend
- [x] P3 Real chart backend
- [x] P3 Embedded deep trimming

### Architecture Upgrades

- [x] Dual-engine `RenderEngine` abstraction
- [x] Native/Embedded dual implementations
- [x] Object system reflection/property enhancement
- [x] DPI/IME/accessibility and platform capability expansion
- [x] ABI engineering: versioning + header generation

### Notes

- Dual-engine architecture is already landed (`RenderEngine` + `NativeEngine` + `EmbeddedEngine`); in practice the render engine abstraction is primarily for embedded paths, while desktop platforms continue to use native backends.
- Harmony native bridge callback path and typed trigger pipeline are already integrated and reused by the current engine/lifecycle layering.

---

## Version History

- `v5`: Framework parity gap-closure roadmap added after full code review.
- `v4`: Product completeness roadmap after v3 hardening.
- `v3`: Release engineering + crates.io quality hardening roadmap added.
- `v2`: Boundary hardening + validation automation roadmap added.
- `v1`: Initial staged roadmap captured and tracked.


