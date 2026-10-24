# Group C — Layout (`R163`–`R191`) report

CLOSED: R163, R164, R165, R166, R167, R168, R169, R170, R171, R172, R174, R175, R176, R177, R178, R179, R180, R181, R182, R183, R184, R185, R186, R187, R188, R189, R190, R191

OPEN: R173 — `Widget::minimum_size_hint()` lives on the base `Widget` trait (`src/widget/widget_trait.rs`), which is outside this slice's file scope; next step for the base slice: add `fn minimum_size_hint(&self) -> Size { self.size_hint() }` to `Widget` and let controls override it.

SCHEMA: none — no new capability properties were added (all new behaviour is opt-in Rust API, so the published property-name set is unchanged).

## Per-row evidence

- R163 — `src/layout/absolute.rs`: `AbsolutePosition::{fill, center_in, with_margins, with_baseline}`, `Anchor::{BaselineLeft, BaselineCenter, BaselineRight}`, `AbsoluteTransition`/`TransitionEasing`, and `AbsolutePosition::resolve` wired into `AbsoluteLayout::layout`/`calculate_positions`; tests `fill_margins_and_baseline_express_declarative_anchors`, `the_layout_applies_fill_and_margins`, `a_transition_animates_an_anchored_child_over_frames`.
- R164 — `src/layout/aspect_ratio.rs`: `AspectMode::{Contain, Cover, Stretch}` via `set_mode`, and symmetric centring rounding; test `cover_mode_fills_and_centring_rounds_symmetrically`.
- R165 — `src/layout/border.rs`: an edge extent of `0` is now "auto" and takes the largest preferred extent from `ChildInfo`; test `an_auto_edge_takes_its_childrens_preferred_extent`.
- R166 — `src/layout/box_layout.rs`: `SizePolicy` now has all seven values; `insert_widget_at`, `insert_stretch`, `set_orientation`; shortfall is truncated proportionally in one pass instead of a per-pixel loop; tests `inserting_and_reorienting_keeps_the_same_items`, `the_extended_size_policies_are_honoured`.
- R167 — `src/layout/constraint.rs`: `ConstraintRef::priority`, `add_constraint_with_priority`, `has_constraint_cycle`, dependency-ordered resolution with logged cycle fallback; test `cycles_are_detected_and_priority_arbitrates`.
- R168 — `src/layout/declarative.rs`: `SharedLayoutRegistry` (cross-thread) and `invalidate_layout`/`is_layout_dirty`/`apply_layout_if_dirty`; tests `dirty_layouts_apply_once_and_then_stay_clean`, `the_shared_registry_is_usable_from_another_thread`.
- R169 — `src/layout/flex.rs`: `FlexItem::{basis, order, baseline}` and `set_item_order` (sorts items and their positional sizes together); tests `baseline_basis_and_order_are_real_inputs`.
- R170 — `src/layout/flow.rs`, `src/layout/wrap.rs`, `src/widget/container_widgets/linear_layouts.rs`: opt-in `TextDirection` mirroring in both engines and both controls; tests `right_to_left_mirrors_the_arrangement`, `right_to_left_mirrors_each_row`, `flow_and_wrap_controls_mirror_for_right_to_left`.
- R171 — `src/layout/form.rs`: label column measured from `Hints`, `FormLabelAlignment`, `field_for_label`/`label_for_field`, `item_at(FormItemRole)`, and full-width standalone (cross-cell) items; test `the_label_column_uses_the_measured_width`.
- R172 — `src/layout/grid.rs`: `GridOriginCorner` + `set_origin_corner`, `GridItemPosition` + `item_positions()`; test `a_bottom_right_origin_mirrors_placement_and_the_map_is_exported`.
- R173 — OPEN, see above. In-scope layout channel exists (`Hints.min`, `SplitterLayout::set_min_size`, measured form labels, per-section accordion heights), so the three named downstream symptoms are closed; only the `Widget` trait hook is missing.
- R174 — `src/layout/splitter.rs`: per-pane `set_min_size`, `set_collapsible`, versioned `save_state`/`restore_state`; test `minimums_collapse_and_state_round_trip`.
- R175 — `src/layout/stack.rs`: `StackMode::All` and `update_slide`; test `stack_all_and_slide_emit_more_than_the_current_page`.
- R176 — `src/layout/types.rs`: `Layout::{minimum_size, size_hint, expanding_directions, invalidate, height_for_width}` with seven `SizePolicy` values; `BoxLayout`/`FlexLayout` implement `minimum_size`, `FlowLayout`/`WrapLayout` implement `size_hint`, `minimum_size`, and a real `height_for_width`; tests `the_flow_aggregates_its_size_and_answers_height_for_width`, `the_wrap_layout_answers_the_upward_channel`.
- R177 — `src/widget/container_widgets/accordion.rs`: per-section `set_body_height`/`body_height`, opt-in `tick` open/close animation, and Up/Down/Space/Enter navigation; test `body_height_animation_and_keyboard_are_wired`.
- R178 — `src/widget/container_widgets/carousel.rs`: opt-in `set_rubber_band` with damped edge drag, `settle_offset`, and a ~200 ms spring-back in `tick`; test `rubber_band_damps_the_edge_and_springs_back`.
- R179 — `src/widget/container_widgets/collapsible_pane.rs`: versioned `save_state`/`restore_state`; test `the_collapsed_state_round_trips_through_a_saved_blob`.
- R180 — `src/widget/container_widgets/dockwidget.rs`: `set_dock_area`/`dock_hint`/`snap_preview_rect` edge docking, `set_title_bar_widget`, `save_state`/`restore_state`, and a new `DockArea` host with `tabify`/`untabify`, edge bands, centre widget and state round-trip; test `dock_area_arranges_edges_tabs_and_round_trips_state`.
- R181 — `src/widget/container_widgets/groupbox.rs`: `set_flat` and `set_content_size_hint` folded into `size_hint`; test `flat_mode_and_content_size_are_wired`.
- R182 — `src/widget/container_widgets/linear_layouts.rs`: `FlexItemSpec::with_widget`/`widget_id`, cached `item_rects`, `item_at`, and `item_clicked`; test `items_carry_child_ids_and_clicks_name_them`.
- R183 — `src/widget/container_widgets/masonry_layout.rs`: versioned arrangement cache, `item_at`, `item_clicked`, and off-screen cards skipped; test `the_arrangement_is_cached_and_cards_are_clickable`.
- R184 — `src/widget/container_widgets/mdiarea.rs`: activation order now drives next/previous, edge-drag resize, title-bar double-click maximize/restore, and a viewport pan (`scroll_offset`/`scroll_by`) applied to painting and hit testing; test `the_viewport_pan_translates_frames_and_hit_tests`.
- R185 — `src/widget/container_widgets/safe_area.rs`: `set_content` and idempotent `apply` place the child in the safe rect; test `a_content_child_is_placed_inside_the_safe_rect`.
- R186 — `src/widget/container_widgets/scrollarea.rs`: opt-in `set_inertia` consuming swipe velocity in `tick`, draggable scroll-bar thumbs, and keyboard (arrows/PageUp/PageDown/Home/End); test `thumb_drag_keyboard_and_inertia_are_connected`.
- R187 — `src/widget/container_widgets/splitter.rs`: `set_min_pane_size`, `set_pane_collapsible`, double-click `equalize_panes`, `save_state`/`restore_state`; test `minimums_state_and_double_click_equalize_are_wired`.
- R188 — `src/widget/container_widgets/stackedwidget.rs`: opt-in `set_transition_duration` slide in `tick`; test `a_page_transition_slides_instead_of_snapping`.
- R189 — `src/widget/container_widgets/stepper.rs`: verified complete (value/min/max/step, clamping, `value_changed`, keyboard path); existing module tests pass.
- R190 — `src/widget/container_widgets/tabwidget.rs`: overflow scroll arrows, `TabElideMode` + `elide_to_width`, Ctrl+Tab cycling, `set_corner_widget`, and hovered-tab tooltip query; test `overflow_elide_corner_and_ctrl_tab_are_wired`.
- R191 — `src/widget/container_widgets/toolbox.rs`: `set_item_height` (used by geometry and hints) and opt-in `set_smooth_scroll` easing in `tick`; test `item_height_and_smooth_scroll_are_wired`.

## Validation

- `cargo check --lib` — passes (after the shared worktree's in-flight edits compiled).
- `cargo test --lib layout::` — 392 passed, 0 failed.
- `cargo test --lib widget::container_widgets` — 465 passed, 0 failed.
- Targeted run of the ten tests added in this pass — 10 passed, 0 failed.
- Note: `widget::capability::properties_tests::schema_and_contract_publish_the_same_names` currently fails on names published by other slices (button, label, frame, combo boxes); no `container_widgets` or `layout` name is implicated and no capability file was touched here.
