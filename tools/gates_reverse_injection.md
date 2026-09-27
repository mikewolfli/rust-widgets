# Gates — reverse-injection evidence

> **What this file is.** One line per `tools/check_*.sh`, recording **what was injected** to
> make the gate fail and **what it reported**. A gate that has never been shown to go red is
> indistinguishable from a gate that always passes, so a missing line is treated the same as a
> `NOT-RUN` gate: **unverified**.
>
> **Why it is not in the plans.** It is the gate suite's own record, not a requirement. It lives
> beside `tools/run_all_gates.sh`, which is the runner that consumes the suite, and
> `tools/check_gates_are_worth_running.sh` is what insists every gate has a line here.
>
> **Format.** `| <gate> | <injected> | <reported> |`. The `reported` column quotes the gate's own
> output, because "it failed" is a claim and the quote is the evidence.
>
> **Status column.** `✔` = verified in this round or a recorded earlier round; `—` = not yet
> injected. A `—` is an honest gap, not a pass.

## BLUE25 — gates added this round (2026-09-27)

| gate | injected | reported |
|---|---|---|
| `check_semantic_state_has_a_consumer.sh` | added `"text_edit"` to the `:error` kind table in `preset_states.rs` (a kind with no `semantic_state`/`resolved_semantic_border` consumer) | `FAIL  a declared \`:error\` kind has no consumer:` / `text_edit: no control reads \`resolved_semantic_border("text_edit", ..)\`` |
| `check_alias_tables_agree.sh` | deleted the `"wizard" => "wizard_dialog"` row from `alias_for_name` in `capability.rs` | `FAIL  \`wizard\` -> \`wizard_dialog\` is in alias_factory_name but not alias_for_name` |

## BLUE25 ICON — gates added this round (2026-09-28)

| gate | injected | reported |
|---|---|---|
| `check_icon_data_is_opt_in.sh` | added `"icons"` to the `default` feature list | `FAIL  \`icons\` is in the \`default\` feature list` |
| `check_icon_licences.sh` | (a) removed the Material Symbols section from a copy of `NOTICE`; (b) ran `tools/gen_icon_data.py` with no `--license` and with `--license=not-a-real-licence`; (c) hand-edited one `d` in `src/widget/icon_data.rs` | (a) `finding: NOTICE: no section headed 'Material Symbols — SVG path subsets for \`icons\`'` / `checked=31 failed=1` (b) both exit 2 (c) `FAIL: src/widget/icon_data.rs is stale; regenerate with` |
| `check_implicit_size_uses_metrics.sh` | dropped the `src/widget/advanced_widgets/dial.rs Dial` row from `tools/implicit_size_exemptions.txt` | `src/widget/advanced_widgets/dial.rs:268: \`size_hint\` derives its answer without ControlMetrics, estimate_text_width/estimate_line_height or dimensions::` / `checked=188 failed=1` |

## BLUE24 — gates added or verified this round (2026-09-24)

| gate | injected | reported |
|---|---|---|
| `check_elevation_is_not_one_value.sh` | put a `Shadow { x: 0, y: 2, blur: 6, .. }` literal back into `role_base_style` | `FAIL  role_base_style builds a Shadow literal instead of reading a level:` |
| `check_single_frame_driver.sh` | (a) swapped `drive_frame`'s `drain_triggers` back to a bare call in the Linux loop; (b) drove the animation bus from the macOS tick | (a) `FAIL  drain_triggers is called outside the frame driver:` (b) `FAIL  tick_animations is driven from outside the frame driver:` |
| `check_animation_state_is_one_type.sh` | re-added `interaction_target: f32` to `Button` | `FAIL  a control keeps its own copy of an animation target: src/widget/base_widgets/button.rs:86` |
| `check_transition_durations_are_tokens.sh` | inserted `from_millis(250)` into `Button::tick` | `FAIL  ... button.rs:337: \`from_millis(250)\` is a hardcoded duration literal` |
| `check_animation_durations_are_tokens.sh` | three separate injections, each observed: added `PropertyDriver::at_ms(value, ms: u32)`; added `TransitionTempo::Millis(u32)`; replaced `duration_ms`'s body with per-arm literals `100/200/300` | `FAIL` on each, escalating: `animation.rs:1824: PropertyDriver::at_ms is a constructor whose tempo parameter is not MotionSlot` → `animation.rs:1574/1564: TransitionTempo::Millis(u32) carries a payload` + `variants are ['Fast','Normal','Slow','Millis']` → `animation.rs:1582/1587/1588/1589: duration_ms does not call motion_tokens() / returns the literal 100 for Fast` |
| `check_first_value_not_zero.sh` | changed a driver's initial value to the target end (`PropertyDriver::at(1.0, ..)`) | reported the offending construction site |
| `check_semantic_state_is_not_in_the_interaction_chain.sh` | returned `WidgetState::Error` from `LineEdit::widget_state` | `FAIL  a widget_state implementation returns a semantic variant: src/widget/input_widgets/lineedit.rs:590` |
| `check_a11y_has_a_producer.sh` | removed the `submit_mounted` / `submit_unmounted` calls from `runtime::{register,unregister}` | `FAIL  \`runtime::register\` no longer submits a mounted control` |
| `check_breakpoints_are_not_in_draw.sh` | inserted `if rect.width < 600 { return; }` into `Label::draw` | `FAIL  a widget branches on a width against a literal: src/widget/base_widgets/label.rs:152` |
| `check_single_open_plan.sh` | added a second plan whose title matches `blue24.md`'s subject, with an unchecked item, while `blue24.md` also had one | `FAIL  the same subject is open in two plans:` |
| `check_plan_archive_has_index.sh` | dropped an unindexed `.md` into `docs/plans/archive/` | `FAIL  these archived plans are not named in docs/plans/README.md:` |
| `check_environment_is_single_sourced.sh` | (earlier round) relabelled a device fact as a wall-clock read | `FAIL  wall-clock inference of an appearance` |
| `check_native_redraw_goes_through_the_runtime.sh` | removed the `note_native_redraw(id)` pairing from a macOS event handler (`forward_mouse`) | `FAIL  these files queue more platform redraws than they announce: src/platform/macos/canvas.rs:453 in forward_mouse()` |
| `check_no_anonymous_repaint.sh` | inserted a bare `crate::invalidate_surface(self.base().id());` into `progressbar.rs`'s `draw` | `FAIL  these repaints name no cause: src/widget/display_widgets/progressbar.rs:431 in draw()` |
| `check_conclusion_covers_the_gap_table.sh` | four injections, each observed: (a) removed the `§5 Breakpoint` cell from `blue24.md` §13's dimension table; (b) renamed every `§0B` mention inside §13 away; (c) added the count word `五件事` back to §13's opening sentence; (d) removed the `| 8 面的材质 |` mapping row from §14.2 step 4 | (a) `FAIL  §5 is not named in §13 (no \`Breakpoint\`)…` (b) `FAIL  §13 does not name §0B…` (c) `FAIL  §13 opens a sentence with a count word, which must be kept in sync by hand:` (d) `FAIL  §0B row 8 (…面…的材质没有声明通道…) is not in §14.2 step 4's merge table` |

## Earlier gates (BLUE12–BLUE23)

> The gates below predate this file. Their injections were recorded in the round logs under
> `docs/log/`; each is listed here so the coverage is visible, and the ones not yet re-verified
> in this file's own format are marked `—` rather than claimed. `check_gates_are_worth_running.sh`
> counts the `—` rows and prints them, so the backlog is a number rather than an impression.

| gate | injected | reported |
|---|---|---|
| `check_widget_kind_count.sh` | — | — |
| `check_view_platform_gate.sh` | — | — |
| `check_profiles.sh` | — | — |
| `check_behavior_matrix.sh` | — | — |
| `check_android_cross.sh` | — | — |
| `check_no_blanket_allow.sh` | — | — |
| `check_docs.sh` | — | — |
| `check_locking.sh` | — | — |
| `check_error_messages.py` | — | — |
| `check_theme_fixtures.sh` | — | — |
| `check_surface_style_is_declared_not_hand_rolled.sh` | appended `fn _probe() -> Color { Color::RED.blend(&Color::WHITE, 0.3) }` to `src/widget/base_widgets/label.rs` | `FAIL  29 files hand-derive a bevel; the ceiling is 28.` then `Offending files not already on the list: src/widget/base_widgets/label.rs` |

> **How to add a line.** Run the gate, inject something that must trip it, run it again, and paste
> the gate's own failure text into the `reported` column. If it does not fail, the gate is either
> too narrow or wrong — both are findings.

> **Full suite coverage.** The rows below list every `check_*.sh` that has not yet been
> injected in this file's own format. Each is an honest `—`, and
> `check_gates_are_worth_running.sh` counts them, so the backlog is a number rather than
> an impression. Replace a `—` by running the gate, injecting something that must trip it,
> and pasting the gate's own failure text.

| gate | injected | reported |
|---|---|---|
| `check_abi.sh` | — | — |
| `check_animation_has_a_driver.sh` | — | — |
| `check_apple_native.sh` | — | — |
| `check_apple_thread_safety.sh` | — | — |
| `check_binding_symbol_coverage.sh` | — | — |
| `check_capability_events_are_emitted.sh` | — | — |
| `check_capability_feature_gates.sh` | — | — |
| `check_capability_matrix_truthfulness.sh` | — | — |
| `check_changelog_sync.sh` | — | — |
| `check_click_requires_release_inside.sh` | — | — |
| `check_control_feature_visible_in_own_snapshot.sh` | — | — |
| `check_control_has_tests.sh` | — | — |
| `check_control_rendering.sh` | — | — |
| `check_control_route_matrix.sh` | — | — |
| `check_declaration_implementation_alignment.sh` | — | — |
| `check_declarative_path_repaints.sh` | — | — |
| `check_declared_targets_ship.sh` | — | — |
| `check_declared_tokens_have_consumers.sh` | — | — |
| `check_designer_feature_gate.sh` | — | — |
| `check_designer_manifest_roundtrip.sh` | — | — |
| `check_embedded_demo_schema.sh` | — | — |
| `check_enabled_is_honoured.sh` | — | — |
| `check_enabled_is_honoured_containers.sh` | — | — |
| `check_event_model_signal_first.sh` | — | — |
| `check_event_payload_types.sh` | — | — |
| `check_event_producers.sh` | — | — |
| `check_event_signal_dyn.sh` | — | — |
| `check_feature_completeness_matrix.sh` | — | — |
| `check_focus_ring_respects_reason.sh` | — | — |
| `check_font_data_is_opt_in.sh` | — | — |
| `check_font_licenses.sh` | — | — |
| `check_generated_font_table_integrity.sh` | — | — |
| `check_generated_sources.sh` | — | — |
| `check_generator_output_compiles.sh` | — | — |
| `check_generator_reuses_wire_rules.sh` | — | — |
| `check_glyph_source_is_the_only_glyph_path.sh` | — | — |
| `check_harmony_cross.sh` | — | — |
| `check_implicit_size_uses_metrics.sh` | — | — |
| `check_ios_cross.sh` | — | — |
| `check_jni_signatures.sh` | — | — |
| `check_json_event_route.sh` | — | — |
| `check_lifecycle_hooks_are_not_build_time.sh` | — | — |
| `check_mechanism_has_a_consumer.sh` | — | — |
| `check_mode_consistency.sh` | — | — |
| `check_module_reachability.sh` | — | — |
| `check_platform_capability_matrix.sh` | — | — |
| `check_platform_create_coverage.sh` | — | — |
| `check_platform_impl_matrix.sh` | — | — |
| `check_readable_flag_is_true_when_get_answers.sh` | — | — |
| `check_routing_match_is_exhaustive.sh` | — | — |
| `check_rw_prefix_is_abi_only.sh` | — | — |
| `check_single_creation_mechanism.sh` | — | — |
| `check_spacing_is_not_sibling_layout.sh` | — | — |
| `check_state_source_is_the_base.sh` | — | — |
| `check_svg_snapshots.sh` | — | — |
| `check_test_guard_uniqueness.sh` | — | — |
| `check_text_coverage_claim_matches_features.sh` | — | — |
| `check_text_model_is_single_sourced.sh` | — | — |
| `check_text_origin_is_a_top_edge.sh` | — | — |
| `check_text_vertically_centred.sh` | — | — |
| `check_view_failures_are_local.sh` | — | — |
| `check_view_keys_are_unique.sh` | — | — |
| `check_visual_regression.sh` | — | — |
| `check_web_engine_honest.sh` | — | — |
| `check_widget_registration_fidelity.sh` | — | — |
| `check_wire_rules_are_data.sh` | — | — |
