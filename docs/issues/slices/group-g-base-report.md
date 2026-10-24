# group-g-base — report (R483–R498, excluding R490–R492)

CLOSED: R483, R484, R485, R486, R487, R488, R489, R493, R494, R495, R497, R498

OPEN: R496 — the shared elide/wrap primitives are delivered, but the 22 consumer
controls that still carry private copies are outside this slice's file set. Next step:
migrate `container_widgets/tabwidget.rs` (private `elide_to_width`),
`display_widgets/empty_state.rs` (private `wrap_text`), `chart_widgets/charts.rs`
(`truncate_legend_label` already delegates) and the `ElideMode`-carrying tab controls
onto `text_utils::{elide, elide_start, elide_middle_to_width, wrap_to_width}`.

SCHEMA: none. Every row is base-class, registry, icon-system or text-helper
behaviour; no new writable widget property was introduced, so no capability row is
needed.

## Per-row evidence

- **R483** — `src/widget/base.rs:268` `set_geometry` clamps to `min_size`/`max_size`,
  early-returns when the clamped rect equals the stored one, and otherwise emits
  `request_layout()` + `request_redraw()`. Tests:
  `set_geometry_requests_repaint_only_when_the_rectangle_changes`,
  `set_geometry_enforces_the_declared_bounds`.
- **R484** — already consumed by layout: `Widget::hints()` (`widget_trait.rs`)
  normalises `min_size`/`max_size` into `AxisHints`, `minimum_size_hint()` exists and
  is folded into `hints()`; `BaseWidget::set_geometry` enforces the same bounds.
  Added early-return + `request_layout()` on bound changes (`base.rs:353`, `:366`);
  test `size_bound_setters_request_layout_only_on_change`.
- **R485** — subtree hiding is a render-time skip, not a flag cascade: the draw walk
  skips a child that is not visible (`runtime.rs` `render_frame_tree`, "What is
  skipped, and why silently"), and hit-testing / focus discovery consult
  `is_visible()` before descending (`runtime.rs` `hit_test`, focus candidate read).
  `show()`/`hide()` (`base.rs:603`, `:619`) now early-return when unchanged and
  request redraw + layout on a real change, and deliberately do not rewrite child
  flags (a child keeps the visibility it chose). Test
  `visibility_toggles_repaint_and_leave_child_flags_alone`.
- **R486** — `BaseWidget::try_set_dpi_scale` (`base.rs:805`) returns
  `Err(DpiScaleError::NonFinite)` for `NaN`/infinity and keeps the last good value;
  `set_dpi_scale` is the infallible form used by property setters. Values below `0.1`
  still clamp up, an unchanged value is a no-op. Test
  `dpi_scale_refuses_non_finite_values`.
- **R487** — verified in `src/widget/runtime.rs`: `RepaintMode` (Full/Dirty/Adaptive)
  at `:3993`, `AUTO_REPAINT_MIN_PIXELS = 250_000` at `:4185` with its decision used in
  the adaptive selection at `:4166`, and `report_state(id, StateFact)` at `:3400`
  (the native-control state sync). No change made; this is a "healthy" row.
- **R488** — `src/widget/registry.rs` module and `SimpleRegistry` docs now state that
  this is a **closure forwarding table**, not the ownership registry; the table names
  the three similarly named concepts (`SimpleRegistry`, `WidgetFactory`,
  `widget::runtime`) and says ownership lives in the runtime. Name kept (renaming is a
  breaking API change), per the row's "document it" option. Also added a manual
  `Debug` impl (size only) because a container that derives `Debug` cannot derive it
  through a boxed `FnMut`.
- **R489** — `src/widget/census.rs` now documents the canonical-name enumeration as
  the counting basis and that the number is **profile-dependent** (capabilities are
  feature-gated), so no constant pins it. Test
  `the_census_measures_every_canonical_control_exactly_once` asserts the set is
  non-empty, contains no duplicate canonical name, and that every published name has
  a constructor. Note: the row's "188" is a stale snapshot of the baseline file
  (`tools/control_rendering_baseline.txt` has 188 rows); the current default build
  publishes **213** canonical names, which is why the count is derived rather than
  asserted against 188.
- **R493** — bypass scan re-run on the current tree: 221 `impl Widget for …`
  declarations, of which **7 are test-only** (`TestWidget` in `layout/absolute.rs` and
  `layout/flow.rs`, `Unmigrated` in `capability/properties_tests.rs`, `LayerProbe` in
  `runtime.rs`, `TestPage` in `navigation_stack.rs`, `TestContent` in `popover.rs`,
  `TestChild` in `swipe_to_dismiss.rs`); **4 private `pressed` fields** remain
  (`empty_state.rs` bool, `switch.rs` bool, `app_bar.rs` `Option<TapTarget>`,
  `message_box.rs` `Option<usize>`), each documented as a different fact from
  `BaseWidget::is_pressed` (a local latch, or *which item/button* is pressed). No
  production control bypasses the factory: every user-facing control is registered
  and built through `WidgetFactory`, and direct `Control::new(..)` calls are the
  normal public constructor path, not a second registry.
- **R494** — `src/widget/icon_source.rs` adds `IconMode` (`Normal`, `Hovered`,
  `Pressed`, `Checked`, `Unchecked`, `Disabled`) and `IconModeSet` with a documented
  fallback chain (hover→normal, pressed→hover→normal, checked/unchecked/disabled→
  normal) — the on/off/disabled **icon groups** the row asked for. `@2x` sources are
  covered end-to-end: `src/widget/icon_registry.rs:118` `register_icon_at_density`
  (plus `try_register_icon_at_density` / `try_register_icon_on_grid_at_density`)
  validates through the same path as an ordinary registration and stores density
  variants under the same name; selection at scale factor ≥ 2 already exists in
  `display_widgets/icon_data_set.rs::lookup_registered_scaled`, exercised by the
  extended `registration_refuses_junk_and_honours_capacity_and_replacement` test and
  by `icon_source.rs` tests `a_partial_mode_set_resolves_through_the_documented_fallback`
  and `pressed_prefers_its_own_glyph_then_hover_then_normal`. The new types are public
  at `widget::icon_source::{IconMode, IconModeSet}`; a `widget::`-level re-export would
  require `src/widget/mod.rs`, which is outside this slice's file set.
- **R495** — verified: `src/widget/svg.rs` is an export channel
  (`render_widget_to_svg*` driving `SvgPaintBackend` plus the SVG geometry readers),
  with no widget-rendering ownership; `src/widget/draw_bridge.rs` keeps the
  `impl_draw_bridge!` macro, `draw_of` and the coverage test
  `every_factory_widget_can_be_painted`. No change made.
- **R496** — `src/widget/text_utils.rs` gains the shared primitives:
  `ElideMode` (`End`/`Middle`/`Start`/`None`) + `elide()` dispatcher, `elide_start()`,
  `elide_middle_to_width()`, and `wrap_to_width()` (width-budget word wrap returning
  byte ranges, same contract as `wrap_char_ranges`, including whole-word breaks,
  character-boundary splitting of an over-long word, and trailing-space trimming).
  Tests: `start_elision_keeps_the_tail`,
  `the_elide_mode_dispatches_to_distinct_endings`,
  `middle_width_elision_keeps_the_head_and_the_tail`,
  `width_wrapping_breaks_at_word_boundaries`,
  `an_over_long_word_is_split_on_character_boundaries`. Consumer migration: see OPEN.
- **R497** — `src/widget/metrics.rs:595` adds
  `ControlMetrics::sub_control_rect(rect, SubControl, SubControlMetrics)` with
  `SubControl` (`Contents`, `Leading`, `Trailing`, `Increment`, `Decrement`, `Close`)
  and `SubControlMetrics` (leading/trailing column widths, close side, gap) — the
  part-level metric the reference toolkit derives per sub-control. Tests
  `sub_control_rects_tile_the_control_without_overlap` and
  `a_close_affordance_is_a_centred_square`. The focus-ring colour already reads the
  `outline` token (`focus_ring_color_from_theme`, `ring_color_from`), present on both
  palettes (`style::theme::Colors::outline` / `theme::types::Colors::outline`), and is
  covered by `focus_ring_color_uses_outline_when_the_theme_provides_one_and_falls_back_otherwise`.
- **R498** — verified: `sample_fill` is gated (`#[cfg(full_widgets)] pub mod
  sample_fill;` and the same gate on the `lib.rs` re-export), so no production path
  reaches it; `create` stays empty by design. The downcast lock already exists and is
  full-coverage by construction: `every_filled_name_downcasts_to_the_type_the_registry_builds`
  parses every arm out of `apply`'s own source (`declared_arm_names`) and asserts each
  one builds through the real factory and fills the type that factory constructs, so a
  new arm cannot be added untested and a renamed control cannot silently stop filling.
  No change made.

## Validation

`cargo check --lib` clean (no errors, no warnings for this slice's files). Targeted
tests, all passing:

- `widget::base::tests` — 40 passed
- `widget::text_utils::tests` — 17 passed
- `widget::metrics::tests` — 41 passed
- `widget::icon_source::tests` — 3 passed
- `widget::icon_registry::tests` — 1 passed
- `widget::census::tests` — 1 passed
- regression spot-checks on untouched neighbours: `widget::sample_fill::tests` 5,
  `widget::draw_bridge::tests` 4, `widget::svg::tests` 8 — all passed

(103 tests in the touched modules, 120 including the neighbours.)

Files changed: `src/widget/base.rs`, `src/widget/registry.rs`, `src/widget/census.rs`,
`src/widget/text_utils.rs`, `src/widget/metrics.rs`, `src/widget/icon_source.rs`,
`src/widget/icon_registry.rs`. No capability schema, `Cargo.toml`, or snapshot file
was touched; the new behaviour is opt-in (an unset `IconModeSet` resolves to nothing,
geometry/visibility setters only act on a real change).
