# `tools/`

Scripts that verify or generate the repository's artifacts. Nothing here is part of
the shipped library.

## `check_*.sh` / `check_*.py` — the gates

Each `check_*.sh` is one gate with a single PASS/FAIL verdict; the paired `check_*.py`
holds the logic when the check needs real parsing. `tools/lib_python.sh` and
`tools/lib_check_locking.py` are shared helpers, not gates.

Run them all:

```bash
bash tools/run_all_gates.sh                # every gate, one PASS/FAIL/NOT-RUN line each
bash tools/run_all_gates.sh --summary      # add the per-gate output
bash tools/run_all_gates.sh --summary --filter check_abi.sh   # one gate
```

`run_all_gates.sh` is the **single** maintenance entry point. It bounds each gate with
`tools/lib_timeout.sh`'s `rw_run_bounded` and stops when the whole-run budget is spent,
reporting the gates it did not reach as `NOT-RUN`.

**Do not wrap the runner in an outer `timeout`** and do not hand-roll a
`for s in tools/check_*.sh; do timeout 400 bash "$s"; done` loop. Both were recommended
here once; both are wrong. The tools' own `rw_run_bounded` already sets an internal bound,
so an outer `timeout` stacks a second timer on top and, as measured in round 52, makes
**every** gate report failure (PASS:0/FAIL:30) while each script run alone exits 0. The
hand-rolled loop also has no `rw_run_bounded` on a host without GNU `timeout`, where it
simply reports `timeout: command not found`. (D08-C-01.)

### What the runner's exit status means

The runner exits non-zero for **any** of:

* one or more gates `FAIL`;
* one or more gates `TIMEOUT` (an inconclusive result must not read as success);
* one or more gates `NOT-RUN` because the whole-run budget was exhausted (that is
  verification which did not happen);
* a `--filter` that matched **no** gate (an empty selection verified nothing).

`exit 0` therefore means "every selected gate ran and passed", not merely "nothing
failed". `SKIP` (a gate that reports `unsupported host`) is the one non-pass that does not
fail the run, because the gate itself ran and answered honestly. (D08-G-01.)

`check_apple_native.sh` reports "unsupported host" and fails on any non-macOS host by
design: it verifies AppKit/UIKit entry points, which cannot be checked elsewhere. Every
other gate must pass on Linux.

**Adding a gate.** A gate that can only report PASS is worse than no gate, because it
looks like a defence. Prove a new gate fails before trusting it: break the thing it
checks, watch it fail, restore. `tools/check_control_has_tests.py` and
`tools/check_event_producers.py` each documented two false-passing versions in their
docstrings for exactly this reason.

## `generate_*.py` — generators for published artifacts

`include/rw_generated.h`, `include/rw_errors.h`, the capability/route matrices and the
feature-completeness matrix are **generated**, never hand-edited. `tools/check_abi.sh`
regenerates them and `cmp`s the result against the published copies, so a hand edit to a
generated file fails the gate rather than shipping.

Regenerate after changing an exported signature:

```bash
python3 tools/generate_c_header.py
bash tools/check_abi.sh
```

The C ABI function count appears in `README.md` and `README.zh-CN.md`; `check_abi.sh`
compares all three and names the file to update when they disagree.

## Other scripts

| Script | Purpose |
|---|---|
| `add_spdx_headers.py` | Adds the SPDX header to source files that lack one. |
| `build_android_testapp.sh`, `run_android_testapp.sh` | Builds and runs the Android test app. |
| `build_ios_testapp.sh`, `run_ios_testapp.sh` | Builds and runs the iOS Simulator app. |
| `run_harmony_testapp.sh` | Builds the `embedded_host` example for `aarch64-unknown-linux-ohos`, copies it to a **connected** OpenHarmony target over `hdc`, runs it there and asserts its printed facts. A bare ELF, not a `.hap`: it covers the portable surface, not ArkUI. With no target it reports `unsupported host` and exits 2. See `src/platform/harmony/status.md`. |
| `run_wayland_compositor_tests.sh` | Runs the Wayland tests against a real compositor. |
| `smoke_demos.sh` | Smoke-runs every demo. |
| `gtk_source_snapshot.py` | Writes the Linux/GTK canvas source, with crate-internal imports and platform-dependent bodies replaced, into a scratch directory for **offline reading**. This is a *source snapshot generator, not a compile check* — the real GTK compile check is the `linux-gtk` CI job (`cargo check --features desktop,gtk-native` on a host with GTK 3). Renamed from `gtk_check.py`, whose name and README line overstated it (D08-G-04). |
| `audit_text_contrast.py` | Reads the committed `snapshots/svg/` files and reports the WCAG contrast ratio of every `<text>` against the element painted under it. **Deliberately not a gate**: a disabled label and a watermark are *supposed* to be faint, so a low ratio is not by itself a defect. It is the evidence generator that says which of 231 controls deserve a look. |
| `audit_appearance.py`, `audit_theme_tokens.py`, `audit_kind_sharing.py`, `audit_control_gaps.py`, `audit_platform_create_coverage.py`, `audit_text_y.py` | Audits, not gates — they quantify a class of defect so the fixes can be prioritised. `audit_platform_create_coverage.sh` (the gate) is what asserts `UNRESOLVED (0)`. |
| `platform_impl_scan.py` | Feeds the platform implementation matrix. |
| `rename_prefix.py` | Renames the crate prefix across bindings. |
| `verify_window_pixels.py` | Reads back pixels from a live window. |
| `test_check_locking.py` | Unit tests for `lib_check_locking.py`. |
| `widget_gallery.rs` | Generates the SVG widget gallery (`cargo run --example widget_gallery`). |
| `win32_accel_probe/` | Host-side test harness for the Windows accelerator parser, whose logic is platform-independent but whose crate is Windows-gated. |
| `feature_completeness_allowlist.toml` | Data for `check_feature_completeness_matrix.sh`. |

## What is deliberately not here

The one-off migration scripts that split large modules (`split_*.py`) and the batch source
rewrites (`fix_*.sh`, `fix_*.py`) were deleted once their targets no longer existed. They
were not reusable tooling: each carried hardcoded paths and `sed -i`/`write` calls that
would silently rewrite current source if re-run, and several had already been superseded
by a `_v2`/`_v3` successor. Keeping them was a hazard rather than a reference — the split
history lives in `docs/` and `docs/log/`, which is where it belongs.

## Appendix: every other file under `tools/`

The sections above describe the tools that need explaining. This appendix is the
**completeness** list: every remaining file, with the first line of its own header.
It exists so a reader can answer "what is `probe_quote_board_columns.py`?" without
opening it, and so a file that is neither a gate nor described cannot hide.

Refresh it after adding or removing a tool (the command prints the same table):

```sh
python3 - <<'PY'
import pathlib, ast
readme = pathlib.Path('tools/README.md').read_text()
for p in sorted(pathlib.Path('tools').iterdir()):
    if p.is_file() and p.name not in readme.split('## Appendix')[0]:
        print(p.name)
PY
```

| file | role |
|---|---|
| `_audit_scan.py` | — |
| `_inject_probe.sh` | _inject_probe.sh — run one gate against one injected edit, then restore. |
| `_inject_unresolvable_alias.py` | Reverse-injection helper for `check_generator_agrees_with_registry.sh`. |
| `_scan_draw_alloc.py` | Scratch scanner: allocation shapes inside `impl Draw for` bodies. |
| `animation_driver_scan.py` | BLUE23 §3.3 gate: an animation state machine must be drivable by the bus. |
| `apply_cargo_cache.py` | Point the heavy gates at the shared cargo cache. |
| `apply_cargo_cache_extra.py` | Cache the remaining *build* steps that the uniform rewrite could not reach. |
| `apply_round90_verdicts.py` | Writes the round-90/91 audit verdicts into the appendix-A rows that still lack one. |
| `audit_animation_wiring.py` | Which controls already implement `Widget::tick` / `is_animating`? |
| `audit_control_theme_reach.py` | Measure each canonical control's implementation against the appendix-A defects. |
| `audit_list_family_conventions.py` | How far the list/table family has got on the two judgements appendix A asks of it. |
| `bench_baseline.txt` | Steady-state benchmark baseline (GV4-04) |
| `check_a11y_has_a_producer.sh` | check_a11y_has_a_producer.sh — BLUE24 §6.3 criterion 6 |
| `check_alias_tables_agree.py` | Gate: the two `WidgetKind` alias tables must name the same pairs (BLUE25 C-7). |
| `check_alias_tables_agree.sh` | check_alias_tables_agree.sh — BLUE25 C-7 |
| `check_android_cross.sh` | Every Android ABI the build script ships must compile (BLUE19 T-11). |
| `check_android_runtime.sh` | The Android probe must run on a real device or emulator, not merely compile. |
| `check_animation_durations_are_tokens.sh` | check_animation_durations_are_tokens.sh — BLUE24 §2.4 gate A (BLUE23 §3.3 判据 5) |
| `check_animation_has_a_driver.sh` | check_animation_has_a_driver.sh -- BLUE23 §3.3 (gate of 6) |
| `check_animation_state_is_one_type.sh` | check_animation_state_is_one_type.sh — BLUE24 §2.3 (the generalisation criterion) |
| `check_apple_thread_safety.sh` | check_apple_thread_safety.sh — Apple native FFI thread-safety gate |
| `check_behavior_matrix.sh` | Each case is bounded by its own timeout so a wedge is reported as "this case hung" rather |
| `check_binding_symbol_coverage.py` | Verify that every language binding reaches the whole published C ABI. |
| `check_binding_symbol_coverage.sh` | check_binding_symbol_coverage.sh — binding surface gate |
| `check_breakpoints_are_not_in_draw.sh` | check_breakpoints_are_not_in_draw.sh — BLUE24 §5 criterion 4 |
| `check_bsd_cross.sh` | check_bsd_cross.sh — the BSD half of `unix_desktop` |
| `check_capability_events_are_emitted.py` | Every published capability event must resolve to a signal its own widget really emits. |
| `check_capability_events_are_emitted.sh` | Every published capability event must resolve to a signal its own widget emits. |
| `check_capability_feature_gates.sh` | check_capability_feature_gates.sh — principle #41 / #53 |
| `check_capability_flags_match_their_methods.py` | Every `PlatformCapabilities` flag a backend declares must be backed by the method it promises. |
| `check_capability_flags_match_their_methods.sh` | check_capability_flags_match_their_methods.sh |
| `check_capability_matrix_truthfulness.py` | check_capability_matrix_truthfulness.py — P1 capability-matrix honesty gate. |
| `check_capability_matrix_truthfulness.sh` | check_capability_matrix_truthfulness.sh — P1 capability matrix honesty gate |
| `check_changelog_sync.sh` | check_changelog_sync.sh — the two changelog copies must agree |
| `check_cjk_shards_cover_the_face.sh` | check_cjk_shards_cover_the_face — the CJK shards must partition the `fonts-cjk` face. |
| `check_click_requires_release_inside.py` | The rule this implements is stated in full — what it proves and, importantly, what it does not — |
| `check_click_requires_release_inside.sh` | check_click_requires_release_inside.sh — BLUE22 §6.7 (BLUE22 §E.3, gate of 5) |
| `check_composite_assembly_rules.py` | Every composite control must assemble its children the way BLUE22 §B.6 requires. |
| `check_composite_assembly_rules.sh` | check_composite_assembly_rules.sh — BLUE22 §B.6 (composite assembly) |
| `check_conclusion_covers_the_gap_table.sh` | check_conclusion_covers_the_gap_table.sh — BLUE24 §14.5 / §14.3 |
| `check_control_feature_visible_in_own_snapshot.py` | Gate: a control that exists to demonstrate a feature must show that feature in its own snapshot. |
| `check_control_feature_visible_in_own_snapshot.sh` | A control that exists to demonstrate a feature must show that feature in its own snapshot |
| `check_control_has_tests.sh` | Every control must have at least one test that builds it. |
| `check_control_rendering.sh` | check_control_rendering.sh — BLUE20 layer 1 (rendering golden table) |
| `check_control_route_matrix.sh` | Generates the control-route matrix into `target/qa/control_route_matrix.md` and fails on a |
| `check_cookbook.sh` | check_cookbook.sh — the three mdBook cookbooks build, and their API names exist |
| `check_declaration_implementation_alignment.py` | BLUE20 layer 2 — the counted declaration/implementation alignment table. |
| `check_declaration_implementation_alignment.sh` | check_declaration_implementation_alignment.sh — BLUE20 layer 2 (三向对齐) |
| `check_declarative_path_repaints.py` | Gate: the declarative property-write path must ask for a repaint. |
| `check_declarative_path_repaints.sh` | The declarative property-write path must ask for a repaint (BLUE21 E1 / P3-1f). |
| `check_declared_targets_ship.sh` | check_declared_targets_ship.sh — every declared Cargo target must be packaged |
| `check_declared_tokens_have_consumers.sh` | check_declared_tokens_have_consumers.sh -- BLUE23 §5.5 (P1-11) |
| `check_demo_crates.sh` | check_demo_crates.sh — the demo crates must compile |
| `check_designer_feature_gate.sh` | The designer tooling must be present on `desktop` and **absent** everywhere else by default |
| `check_designer_manifest_roundtrip.sh` | The designer's capability document must survive export -> load -> export unchanged. |
| `check_drawn_types_are_covered.py` | Gate: every `impl Draw` type must be covered by some existing gate, or be acknowledged. |
| `check_drawn_types_are_covered.sh` | check_drawn_types_are_covered.sh — 每个 `impl Draw` 都要有门禁覆盖 |
| `check_elevation_is_not_one_value.sh` | check_elevation_is_not_one_value.sh — BLUE24 §10A.6 criterion 4 |
| `check_embedded_demo_schema.sh` | Reads the demo sources and asserts each declared key is present, so a demo that stops reporting |
| `check_enabled_is_honoured.py` | Every control that handles user input must consult its `enabled` state. |
| `check_enabled_is_honoured.sh` | Every input-handling control must consult its `enabled` state (principle #37, round 52 carry-over). |
| `check_enabled_is_honoured_containers.py` | A container that owns programmatic mutators must honour `enabled` in the *emitter*. |
| `check_enabled_is_honoured_containers.sh` | A container that owns programmatic mutators must honour `enabled` in the emitter. |
| `check_environment_is_single_sourced.sh` | check_environment_is_single_sourced.sh — BLUE24 §4 criterion 6 |
| `check_error_messages.py` | Sweeps the crate for error messages and reports the ones that are not actionable. |
| `check_event_model_signal_first.sh` | Scans `src/`, `demo/` and `examples/` for event-table patterns; this crate is signal-first, and a |
| `check_event_names_follow_the_convention.py` | A published event name describes the event, not a listener method: no `on_` prefix. Reads the census |
| `check_capability_schema_is_current.sh` | `docs/capability_schema.json` must equal a fresh export of the live capability table (AG-13); the exporter pins `profile`/`triple` so the comparison is host-independent. |
| `check_event_names_follow_the_convention.sh` | A published event name must not carry the `on_` listener prefix; the suites crate's remaining names |
| `check_event_payload_types.py` | Every declared event payload must equal the real Rust type of the signal behind it. |
| `check_event_payload_types.sh` | Every declared event payload must equal the real Rust type of the signal behind it, and the |
| `check_event_producers.sh` | Every gesture event must have a production construction site (principle #75). |
| `check_event_signal_dyn.py` | A control that wires its events dynamically must resolve every name its capability publishes. |
| `check_event_signal_dyn.sh` | A control that wires its events dynamically must resolve every name its capability publishes. |
| `check_event_variants_have_a_producer.py` | Every `Event` variant must have either a real producer or a declared host producer. |
| `check_event_variants_have_a_producer.sh` | check_event_variants_have_a_producer.sh — principle #75, for the whole enum |
| `check_first_value_not_zero.sh` | check_first_value_not_zero.sh — BLUE24 §2.4 gate B |
| `check_focus_ring_respects_reason.sh` | check_focus_ring_respects_reason.sh — BLUE22 §6.7 (BLUE22 §E.3, gate of 5) |
| `check_font_data_is_opt_in.sh` | check_font_data_is_opt_in.sh — BLUE23 §0A.4 (附录 G, 约束 1) |
| `check_font_licenses.sh` | check_font_licenses.sh — BLUE23 §0A.4 (附录 G-4a) |
| `check_gates_are_worth_running.sh` | check_gates_are_worth_running.sh — BLUE24 §9.2 (the meta-gate) |
| `check_generated_files_have_a_runnable_producer.sh` | check_generated_files_have_a_runnable_producer.sh |
| `check_generated_font_table_integrity.sh` | check_generated_font_table_integrity.sh — BLUE23 §0A.4 (附录 G, G-4b/G-4a) |
| `check_generated_sources.sh` | Committed generated sources must match their project documents **and** compile clean |
| `check_generator_agrees_with_registry.sh` | A name the generator can construct must be a name the registry can construct — and vice versa. |
| `check_generator_output_compiles.sh` | T-23: the generator's output must COMPILE on every target profile it claims to support. |
| `check_generator_reuses_wire_rules.py` | The generator must reuse the runtime's wire rules, not restate them (BLUE19 T-23 DoD). |
| `check_generator_reuses_wire_rules.sh` | The generator must reuse the runtime's wire-compatibility rules, not restate them (T-23 DoD). |
| `check_glyph_source_is_the_only_glyph_path.sh` | check_glyph_source_is_the_only_glyph_path.sh — BLUE23 §0A.4 (附录 G, G-2b) |
| `check_harmony_cross.sh` | HarmonyOS (OpenHarmony) cross-target gate. |
| `check_icon_data_is_opt_in.sh` | check_icon_data_is_opt_in.sh — BLUE25 ICON-8 (the feature gate) |
| `check_icon_entry_contract.sh` | check_icon_entry_contract.sh — the U0 entry inventory, as an assertion |
| `check_icon_licenses.sh` | check_icon_licenses.sh — BLUE25 ICON-8 (the licence chain) |
| `check_icons_not_symbol_glyphs.sh` | check_icons_not_symbol_glyphs.sh — the "an icon is an icon" gate |
| `check_implicit_size_uses_metrics.sh` | check_implicit_size_uses_metrics.sh — BLUE22 §6.7 (BLUE22 §E.3, gate of 5) |
| `check_ios_cross.sh` | The iOS build surface must type-check, as far as this host can take it (BLUE19 T-12). |
| `check_jni_signatures.py` | check_jni_signatures.py — Java ↔ Rust JNI signature contract gate. |
| `check_jni_signatures.sh` | check_jni_signatures.sh — Java ↔ Rust JNI signature + symbol gate |
| `check_json_event_route.py` | The JSON event route and the capability event table must not drift apart (BLUE19 T-8/T-10). |
| `check_json_event_route.sh` | The JSON event route must resolve published names against the capability table (BLUE19 T-8/T-10). |
| `check_lifecycle_hooks_are_not_build_time.sh` | check_lifecycle_hooks_are_not_build_time.sh -- BLUE23 §5A.3 (judgement 7) |
| `check_linux_gtk_check.sh` | Linux/GTK type-check closure (GV4-02). |
| `check_locking.sh` | Locking audit gate. |
| `check_log_archive_has_index.sh` | check_log_archive_has_index.sh — AG6-13 / A3-01 root cause |
| `check_macos_backends_clippy.sh` | check_macos_backends_clippy.sh — both macOS backends are linted, not just compiled |
| `check_macos_canvas_lifecycle.sh` | check_macos_canvas_lifecycle.sh — PF-X03 / PF-X06 evidence on a real AppKit canvas |
| `check_macos_cross.sh` | check_macos_cross.sh — the macOS backend must type-check, as far as this host can take it |
| `check_mechanism_has_a_consumer.py` | Gate: an abstraction must have at least one production consumer (BLUE21 P3-1g). |
| `check_mechanism_has_a_consumer.sh` | An abstraction must have at least one production consumer (BLUE21 E1 / P3-1g). |
| `check_modal_loop_frame_heartbeat.sh` | check_modal_loop_frame_heartbeat.sh — the drag/resize drain stays closed |
| `check_mode_consistency.sh` | T-24: mode 1 and mode 2 must describe the same UI, and each must stay in its own profile's API. |
| `check_module_reachability.py` | Module reachability gate (principle #72). |
| `check_module_reachability.sh` | Module reachability gate (principle #72). Runs `tools/check_module_reachability.py` |
| `check_mousepress_carries_modifiers.py` | Gate: every `Event::MousePress` *struct literal* must carry the `modifiers` field. |
| `check_mousepress_carries_modifiers.sh` | Gate: `Event::MousePress` must carry the modifier bitmask end to end, so that a |
| `check_native_control_paths.sh` | check_native_control_paths.sh — BLUE28 B28.29 gate. |
| `check_native_redraw_goes_through_the_runtime.sh` | check_native_redraw_goes_through_the_runtime.sh — BLUE24 §7 criterion 4 |
| `check_no_anonymous_repaint.sh` | check_no_anonymous_repaint.sh — BLUE24 §8 criterion 5 |
| `check_no_native_control_creation.py` | The `src/` tree must contain no OS control-creation call — only window/surface creation. |
| `check_no_native_control_creation.sh` | check_no_native_control_creation.sh |
| `check_no_write_only_fields.py` | Gate: no struct field in the widget/style/layout layers may be written and never read. |
| `check_no_write_only_fields.sh` | Gate: no struct field in the widget/style/layout layers may be written and never read |
| `check_node_icon_contract.sh` | check_node_icon_contract.sh - the icon contract as the **Node** binding sees it |
| `check_plan_archive_has_index.sh` | check_plan_archive_has_index.sh — BLUE24 §9.1 criterion 3 |
| `check_plan_tables.py` | Reports table rows whose cell count disagrees with their own header. |
| `check_platform_capability_matrix.sh` | check_platform_capability_matrix.sh — R6 Platform Capability Matrix Validator |
| `check_platform_create_coverage.sh` | check_platform_create_coverage.sh — 平台控件工厂必须能查到控件（跨层一致性） |
| `check_platform_docs_truthfulness.sh` | check_platform_docs_truthfulness.sh — BLUE28 B28.31 gate. |
| `check_platform_facts_matrix.sh` | check_platform_facts_matrix.sh — BLUE28 B28.30 gate. |
| `check_platform_impl_matrix.sh` | check_platform_impl_matrix.sh — P1 per-OS platform_impl implementation gate |
| `check_platform_probe.sh` | check_platform_probe.sh — BLUE28 B28.01 gate. |
| `check_present_matrix.sh` | check_present_matrix.sh — which platforms can actually present, and which cannot |
| `check_profile_builds.sh` | check_profile_builds.sh — every device profile builds and tests |
| `check_profile_target_matrix.sh` | check_profile_target_matrix.sh — BLUE28 B28.37. |
| `check_profiles.sh` | Runs the profile matrix CI uses (`mini`, `embedded`, `tablet`, `desktop+gpu-wgpu`, ...) with one |
| `check_readable_flag_is_true_when_get_answers.py` | Gate: a schema row flagged `readable: false` must not be answered by its control's `get`. |
| `check_readable_flag_is_true_when_get_answers.sh` | A schema row flagged `readable: false` must not be answered by its control's `get` |
| `check_redox_cross.sh` | Redox OS (Orbital) cross-target check. |
| `check_routing_match_is_exhaustive.sh` | check_routing_match_is_exhaustive.sh — BLUE23 §0A.6 ITEM 0（编译期路由闭合的后半条） |
| `check_runtime_fonts_is_opt_in.sh` | check_runtime_fonts_is_opt_in — the runtime face registry ships no data and stays removable. |
| `check_rw_prefix_is_abi_only.sh` | check_rw_prefix_is_abi_only.sh — the `rw_` prefix belongs to the ABI boundary |
| `check_scrollable_census.py` | Every control that holds a scroll position must declare its **unit** (RW-58). |
| `check_scrollable_census.sh` | check_scrollable_census.sh — RW-58 (BLUE30 §2.2 step 5) |
| `check_semantic_state_has_a_consumer.sh` | check_semantic_state_has_a_consumer.sh — BLUE25 A-3 |
| `check_semantic_state_is_not_in_the_interaction_chain.sh` | check_semantic_state_is_not_in_the_interaction_chain.sh — BLUE24 §3.3 criterion 4 |
| `check_setter_census.py` | Gate: every `pub fn set_*` on a registered control is published or ledgered (RW-44). |
| `check_setter_census.sh` | check_setter_census.sh — RW-44 (BLUE30 §1.3 step 4) |
| `check_single_creation_mechanism.sh` | check_single_creation_mechanism.sh — BLUE15 #55/#56 gate |
| `check_single_frame_driver.sh` | check_single_frame_driver.sh — BLUE24 §1 criterion 6 |
| `check_source_structure.sh` | check_source_structure.sh — principles #8 (mod.rs is a façade) and #9 (5000 lines) |
| `check_spacing_is_not_sibling_layout.sh` | check_spacing_is_not_sibling_layout.sh — BLUE22 §6.7 (BLUE22 §E.3, gate of 5) |
| `check_state_source_is_the_base.sh` | check_state_source_is_the_base.sh -- BLUE23 §2.2 (gate of 5) |
| `check_status_docs_name_real_types.py` | A backend's `status.md` may not claim native objects the backend's source does not name. |
| `check_status_docs_name_real_types.sh` | check_status_docs_name_real_types.sh |
| `check_surface_style_is_declared_not_hand_rolled.sh` | check_surface_style_is_declared_not_hand_rolled.sh — BLUE24 §10A.6 criterion 3 |
| `check_svg_snapshots.sh` | check_svg_snapshots.sh — BLUE20 layer 3 (SVG 全量快照再生门禁) |
| `check_target_backend_selection.sh` | check_target_backend_selection.sh — BLUE28 B28.38 gate. |
| `check_test_guard_uniqueness.py` | A test guard must delegate to the shared lock, not declare its own. |
| `check_test_guard_uniqueness.sh` | check_test_guard_uniqueness.sh — one test guard per process-wide singleton |
| `check_text_coverage_claim_matches_features.sh` | check_text_coverage_claim_matches_features.sh — BLUE23 §0A.2 / §0A.4 (附录 G, 判据 15) |
| `check_text_model_is_single_sourced.sh` | check_text_model_is_single_sourced.sh — BLUE23 §0A.4 (附录 G, G-3/G-2b) |
| `check_text_origin_is_a_top_edge.py` | Gate: a text origin must never carry an `ascent` term. |
| `check_text_origin_is_a_top_edge.sh` | A text origin is the glyph box's TOP edge, never a baseline (principle #18, BLUE20 rules |
| `check_text_vertically_centred.py` | Gate: a single-line label must not be positioned at `band.y + band.height / 2`. |
| `check_text_vertically_centred.sh` | A single-line label must not be positioned at `band.y + band.height / 2` (BLUE21 / P0-1). |
| `check_theme_fixtures.sh` | Theme fixture gate. |
| `check_transition_durations_are_tokens.py` | The rule this implements is stated in full — what it proves and what it does not, with the |
| `check_transition_durations_are_tokens.sh` | check_transition_durations_are_tokens.sh — BLUE22 §6.7 (BLUE22 §E.3, gate of 5) |
| `check_version_consistency.sh` | check_version_consistency.sh — every published version string must match Cargo.toml |
| `check_view_failures_are_local.sh` | check_view_failures_are_local.sh — BLUE23 §5A.5（P1-15 错误边界，判据 14/31） |
| `check_view_keys_are_unique.py` | Declarative key-uniqueness gate (BLUE18 rule #88). |
| `check_view_keys_are_unique.sh` | Shell wrapper for `check_view_keys_are_unique.py` (BLUE18 rule #88). |
| `check_view_platform_gate.sh` | check_view_platform_gate.sh — BLUE18 rules #92 / #94 |
| `check_visual_regression.sh` | Bounded per test, with the results written to `target/qa/visual_regression_report.md` so a |
| `check_web_engine_honest.sh` | check_web_engine_honest.sh — BLUE20 layer 5 (WebEngine 诚实降级，裁定 W1) |
| `check_widget_kind_count.sh` | check_widget_kind_count.sh — documentation/code count consistency gate |
| `check_widget_registration_fidelity.py` | Registration fidelity gate: every `WidgetKind` must be reachable, aliased or |
| `check_widget_registration_fidelity.sh` | Delegates to `tools/check_widget_registration_fidelity.py`, which is the documentation of record |
| `check_wildcard_exports.sh` | check_wildcard_exports.sh — the glob-re-export surface has an owner |
| `check_wire_rules_are_data.py` | The wire-compatibility rules must be data both consumers can read (BLUE19 T-3). |
| `check_wire_rules_are_data.sh` | The wire-compatibility rules must be shared data, not a `match` (BLUE19 T-3). |
| `cjk_vector_codepoints.txt` | The CJK codepoints the `fonts-cjk` vector face carries (BLUE23 §0A.4, G-4c). |
| `cjk_vector_shards.txt` | The unicode-range shards of the CJK vector face (`fonts-cjk-shards`). |
| `classify_setter_candidates.py` | Classify the 30 candidates by *why* they might not need a direct `request_redraw`. |
| `click_release_exemptions.txt` | Exemptions for tools/check_click_requires_release_inside.sh (BLUE22 §6.7). |
| `control_color_exemptions.txt` | BLUE20 layer 1 — data-colour exemption table (P3 whitelist) |
| `control_overflow_exemptions.txt` | BLUE20 layer 3 — P5 overflow exemptions (drawing that legitimately leaves the box) |
| `control_rendering_baseline.txt` | BLUE20 layer 1 — control rendering baseline (generated by examples/control_rendering_census.rs) |
| `control_surface_coincidence_exemptions.txt` | BLUE20 layer 1 — P2 surface-coincidence exemptions |
| `cookbook_api_scan.py` | The rule this implements is stated in full — what it proves, what it does not, and the reverse |
| `declaration_alignment_census.txt` | BLUE20 layer 2 — declaration/implementation alignment census |
| `declared_tokens_scan.py` | BLUE23 §5.5 gate: every declared colour role has a production consumer. |
| `derive_event_payloads.py` | Derive the real Rust payload type of every *published* capability event, from source. |
| `designer_generate.rs` | data / fixture |
| `emoji_font_probe.py` | Inspect a colour-emoji face: table directory, `CBLC`/`CBDT` sizes, and the cmap. |
| `emoji_list_codepoints.py` | Dump the codepoints the full NotoColorEmoji face covers, as a starting point for a subset list. |
| `emoji_subset_codepoints.txt` | The colour-emoji codepoints the shipped subset carries (BLUE23 G-6). |
| `emoji_subset_scan.py` | Scan the full NotoColorEmoji face and report what a given subset would cover. |
| `event_published_census.txt` | Published capability events, one control per line: control: name, name, ... |
| `fix_double_cargo.py` | Fix the double-`cargo` the cache rewrite introduced. |
| `focus_ring_scan.py` | The rule this implements is stated in full — what it proves, what it does not, and why the two |
| `font_data_opt_in_scan.py` | The rule this implements is stated in full — what it proves, what it does not, and the |
| `font_license_scan.py` | The rule this implements is stated in full — what it proves, what it does not, and the |
| `font_table_integrity_scan.py` | The rule this implements is stated in full — what it proves, what it does not, and the |
| `gates_reverse_injection.md` | Gates — reverse-injection evidence |
| `gen_cjk_bitmap.py` | Generate the 16x16 bitmap CJK subset `src/render/text/cjk_bitmap_data.rs`. |
| `gen_cjk_shards.py` | Generate the unicode-range shards of the CJK vector face (BLUE23 §0A.4 follow-up). |
| `gen_emoji_subset.py` | Generate the opt-in colour-emoji subset this crate ships (BLUE23 §0A.4, G-6). |
| `gen_font_subset.py` | Generate the opt-in vector font subsets this crate ships (BLUE23 §0A.4, G-4c). |
| `gen_icon_entry_inventory.py` | Emit the BLUE27 icon-entry inventory as a committed artifact. |
| `generate_control_index.py` | Generates `control.md` — every control's SVG snapshots, grouped by family. |
| `generate_control_route_matrix.py` | Generate control-route and implementation-grade matrix from source code. |
| `generate_error_header.py` | Generate C header for rust_widgets error codes. |
| `generate_feature_completeness_matrix.py` | Generate feature-completeness matrix report for src modules. |
| `generate_platform_capability_matrix.py` | generate_platform_capability_matrix.py — R6 Platform Capability Matrix Generator |
| `generate_platform_facts_matrix.py` | Generate the platform-facts matrix (BLUE28 B28.30). |
| `generator_agreement_probe.rs` | [test] |
| `glyph_path_scan.py` | The rule this implements is stated in full — what it proves, what it does not, and the |
| `icon_allocations_baseline.txt` | Icon paint allocations — the blue27 §19.13 I17 baseline |
| `icon_census.txt` | data / fixture |
| `icon_count_claim_scan.py` | Check that every documented icon count matches `crates/rust-widgets-icons/tools/icon_tokens.txt`. |
| `icon_entry_inventory.txt` | BLUE27 icon entry inventory - GENERATED by tools/gen_icon_entry_inventory.py |
| `icon_license_scan.py` | The rule this implements is stated in full — what it proves, what it does not, and the reverse |
| `implicit_size_exemptions.txt` | check_implicit_size_uses_metrics — exemption table. |
| `implicit_size_scan.py` | The rule this implements is stated in full — what it proves and what it does not, with the |
| `lib_cargo_cache.sh` | lib_cargo_cache.sh — run a `cargo` verification step once per build identity |
| `lib_jdk.sh` | Shared JDK discovery for the Android tooling. |
| `lib_python_selftest.sh` | Self-test for `tools/lib_python.sh`'s interpreter probe (N-G-03, N-G-04). |
| `lifecycle_hook_scan.py` | BLUE23 §5A.3 gate: lifecycle hooks are carried at build time, never invoked there. |
| `list_controls.py` | List every registered control, grouped, so a gap can be argued from a list rather than recalled. |
| `list_event_payload_shapes.py` | List the Rust payload type of every signal in the crate, grouped by shape. |
| `make_sample_photos.py` | Generate the four sample photographs the image gallery's snapshot shows. |
| `migrate_capability_events.py` | Rewrite every capability's `events:` list into a generated-schema lookup. |
| `migrate_own_face_to_surface.py` | BLUE24 §10A A1: route every control's own face through `RenderContext::face`. |
| `move_chart_chrome_onto_axis_chrome.py` | Moves the remaining chart-engine chrome literals onto `axis_chrome`/`axis_chrome_color`. |
| `normalise_plan_tables.py` | Normalises every appendix table row to its own header's cell count. |
| `pkgconfig_linux_cross_shim.sh` | Lets a macOS host type-check the GTK backend for a Linux target. |
| `polish_debranded_prose.py` | Fourth pass: make the de-branded prose read naturally. |
| `probe_asymmetric_sibling_setters.py` | Iceberg scan: setters that are *siblings* of a repainting setter but do not repaint themselves. |
| `probe_quote_board_columns.py` | Check that each text run in a `quote_board` snapshot sits inside a column. |
| `probe_setters_without_repaint.py` | Locates setter methods that assign to a field without asking for a redraw. |
| `probe_snapshot_ink_left.py` | Measure where a snapshot's text ink actually starts, in device pixels. |
| `publish_emitted_events.py` | Add the published event names a capability's control emits but does not publish. |
| `regenerate_census_lists.py` | Regenerate the KNOWN_* allowlists inside the census test from live output. |
| `repair_appendix_rows.py` | Repairs the appendix rows that `apply_round90_verdicts.py` left one cell short. |
| `report_appendix_a_discharge.py` | Discharge appendix-A rows that a machine can decide, so the rest are the real work. |
| `report_appendix_coverage.py` | Reports which appendix-A control rows still lack a round-90/91 verdict. |
| `routing_match_scan.py` | The rule this implements is stated in full — what it proves, what it does not, and the |
| `rule_setter_candidates.py` | Ruling aid: for each candidate setter, is the assigned field *read by a draw path*? |
| `run_local_gates.sh` | run_local_gates.sh — run the `check_*.sh` gates that no workflow or gate invokes |
| `run_present_macos.sh` | run_present_macos.sh — the real-window presentation probe (BLUE29 §7 / B29.13) |
| `run_wayland_clipboard_paste.sh` | rw-gate-budget: 600 |
| `scan_own_face_blocks.py` | Dry-run tool: find the "the control's own face" block in a widget's `draw`. |
| `semantic_color_census.py` | Semantic-colour census (BLUE20 layer 1, rule #109). |
| `semantic_color_census.txt` | data / fixture |
| `setter_census.py` | Ledger the setters each registered control exposes but does not publish as commands (RW-44). |
| `spacing_scan.py` | The rule this implements is stated in full — what it proves, what it does not, and the two |
| `state_source_scan.py` | BLUE23 §2.2 gate: hover/press/focus state has exactly one source, `BaseWidget`. |
| `strip_qml_filenames.py` | Second pass: neutralize `.qml` filename and checkout-path citations. |
| `strip_reference_paths.py` | Third pass: neutralize the read-only *checkout paths* used as citation addresses. |
| `strip_third_party_names.py` | Strip third-party toolkit *names* from the plan and log documents. |
| `surface_style_allowlist.txt` | Allowlist for tools/check_surface_style_is_declared_not_hand_rolled.sh |
| `svg_snapshot_check.py` | The Python steps of `tools/check_svg_snapshots.sh`, one subcommand each. |
| `symbol_glyph_scan.py` | The rule this implements is stated in full — what it proves, what it does not, and the reverse |
| `test_cargo_cache.sh` | Exercises `tools/lib_cargo_cache.sh`: a cold run compiles, a warm run replays successful output, |
| `text_coverage_claim_scan.py` | The rule this implements is stated in full — what it proves, what it does not, and the |
| `text_model_single_source_scan.py` | The rule this implements is stated in full — what it proves, what it does not, and the |
| `transition_duration_exemptions.txt` | Exemptions for tools/check_transition_durations_are_tokens.sh (BLUE22 §6.7). |
| `vector_failure_summary.py` | Prints the exact assertion each remaining vector-build failure makes. |
| `view_failure_locality_scan.py` | The rule this implements is stated in full — what it proves, what it does not, and the |
| `view_platform_gate_probe.rs` | [cfg(declarative_view)] |
| `word_spacing_probe.py` | Shows what the flat 0.6 em estimate does to a real word. |
| `x11_inject_keys.py` | Type a path into a focused window using the X server's XTEST extension. |
