# Group D — view widgets report (R192–R224)

CLOSED: R192, R193, R194, R195, R196, R197, R198, R199, R200, R201, R202, R203, R204,
R205, R206, R207, R208, R209, R210, R211, R212, R213, R214, R215, R216, R217, R218,
R219, R220, R221, R222, R223, R224

OPEN: none

SCHEMA | none requested — no new capability property was added; every new knob is a typed
Rust API whose default preserves the historical pixels.

## Row notes

- R192 — `src/widget/view_widgets/data_source.rs`: `ModelIndex` (`root`/`child`), `DataRole`,
  `data_at`, `parent_index`; proven by `roles_and_model_indices_are_addressable`.
- R193 — `data_source.rs`: `cell_changed_signal`, `rows_inserted_signal`,
  `rows_removed_signal`, `rows_reset_signal` (each defaults to `None` for old sources).
- R194 — `data_source.rs`: `set_data`, `flags`/`CellFlags`, `mime_data`; writable models emit a
  fine-grained signal after a write.
- R195 — `filter_expr.rs`: `accepts_with` (case policy), `build_index`/`build_index_with`,
  `FilterIndex` bitmap; `data_grid.rs::filter_changed` broadcasts filter edits. Test
  `case_sensitivity_and_the_bitmap_index_are_real_inputs`.
- R196 — `data_grid.rs`: `set_full_model_projection`, `fetch_full_projected_window` (fetch whole
  model once, project, then slice the visible window, cached by revision/generation). Test
  `full_model_projection_header_sort_and_keyboard_are_wired`.
- R197 — `data_grid.rs`: `ColumnWidthMode` (`Fixed`/`Stretch`/`Interactive`), `EditTriggers`,
  `set_alternating_rows`; `grid_table.rs` mirrors alternating rows.
- R198 — `data_grid.rs`: the reserve header is now painted from `column_name`, hit-tested by
  `header_column_at` and clicks emit `header_clicked`/`sort_changed`; frozen columns are pinned
  in `cell_rect`/`cell_at`/draw (tests `frozen_columns_ignore_horizontal_scroll`,
  `a_frozen_column_is_painted_at_the_leading_edge_after_scrolling`).
- R199 — `data_grid.rs::handle_event`: wheel (vertical + horizontal/shift), Up/Down/Left/Right,
  PageUp/PageDown, Home/End, Enter-to-edit, right-click; opt-in scrollbar thumb.
- R200 — `grid_table.rs`: `cell_selected`, `cell_double_clicked`, `sort_changed`,
  `header_clicked` already published and wired; verified in `handle_event`/`event_signal_dyn`.
- R201 — `grid_table.rs`: `GridSectionResizeMode` (Interactive/Stretch/Fixed/ResizeToContents)
  via `set_section_resize_mode`; `set_multi_select` with `selected_rows`/`selected_cells`;
  `set_alternating_rows`. Test `multi_select_type_ahead_and_scrollbar_are_wired`.
- R202 — `grid_table.rs`: Home/End/PageUp/PageDown/Enter, printable-key type-ahead
  (`type_ahead`), ctrl/shift extended selection, context menu, edit request.
- R203 — `grid_table.rs`: `show_scrollbar` thumb and `set_header_height` (heavier header strip).
  The whole-widget repaint remains the `Draw` contract; no widget-local partial invalidation
  exists in the framework.
- R204 — `table_widget.rs::ItemDelegate`: `paint`, `size_hint`, `update_editor_geometry`,
  `close_editor`; used by `draw`, `resolved_row_height`, `begin_edit`, `commit_edit`.
- R205 — `table_widget.rs`: `TableEditTriggers` (`NONE`/`DOUBLE_CLICKED`/`EDIT_KEY`/`ALL`) and
  `edit_requested`; test `scrolling_keyboard_and_edit_triggers_are_wired`.
- R206 — `table_widget.rs::handle_event`: wheel, Up/Down/Home/End/PageUp/PageDown, double-click
  activation/edit, context menu; rows respect `scroll_row`.
- R207 — `virtual_table.rs`: `selected_cell`/`selection_changed`, `show_header`,
  `show_scrollbar`, PageUp/PageDown grid paging, `VirtualCellDelegate` paint hook. Tests
  `selection_header_paging_and_scrollbar_are_wired`, `a_cell_delegate_can_paint_a_cell`.
- R208 — `list_view.rs`: the single SelectionModel (`SelectionMode` + anchor/shift/ctrl) is
  verified in `extended_selection_replaces_on_a_plain_click_and_extends_with_shift`.
- R209 — `list_view.rs`: `scroll_row`/`set_scroll_row`/`scroll_by_lines`, wheel, keyboard, and
  `row_rect`/`row_at_point` apply the scroll offset.
- R210 — `list_view.rs`: keyboard navigation, `ListModel::icon` tile glyphs, `row_activated`,
  `context_menu_requested`, and press-drag-release `row_reordered`. Tests
  `scrolling_keyboard_and_icons_are_wired`, `a_drag_between_rows_emits_a_reorder_request`.
- R211 — `virtual_list.rs`: `selection_changed` verified (pre-existing `Signal1`), plus keyboard
  navigation and `show_scrollbar`; test `keyboard_and_scrollbar_are_wired`.
- R212 — `tree_view.rs::TreeModel`: `can_fetch_more`/`fetch_more`, exposed as
  `TreeView::can_fetch_more`/`fetch_more`.
- R213 — `tree_view.rs`: `expanded`, `collapsed`, `activated`, `node_double_clicked` signals.
- R214 — `tree_view.rs`: Up/Down/Home/End/PageUp/PageDown move focus and scroll; double-click
  toggles expansion and emits `activated`.
- R215 — `tree_view.rs`: `scroll_row`, `show_scrollbar`, `set_animated` + `expansion_progress`
  (PropertyDriver fade, the mechanism adopted from ImageGallery), `set_source_list` tint.
  Test `the_expansion_fade_advances_and_settles`.
- R216 — `tree_table.rs`: `set_windowed_projection` + `projection_budget`/`flatten_path` stop at
  the viewport, `projection_truncated`, and `set_scroll_row` extends on demand. Test
  `a_windowed_projection_stops_at_the_viewport_and_extends`.
- R217 — `tree_table.rs`: keyboard paging, wheel, `context_menu_requested`, clickable header
  (`header_column_at`), sibling sort (`set_sort_column`/`toggle_sort_column` + `sort_changed`),
  `set_indent`, `set_row_height`.
- R218 — `tree_table.rs`: `expansion_changed` emitted by `expand_row`/`collapse_row`.
- R219 — `image_gallery.rs`: PropertyDriver/MotionSlot reveal verified; the mechanism is now
  also used by `tree_view` (R215).
- R220 — `image_gallery.rs`: wheel paging, `fullscreen_requested` on double-click,
  `set_multi_select` + ctrl/shift selection, `set_zoom`; test
  `zoom_wheel_double_click_and_multi_select_are_wired`.
- R221 — `image_gallery.rs`: `GalleryImage::load_from_path`/`load_from_bytes` decode into
  `GalleryImage::image` (feature `image`) and `draw` blits the decoded RGBA via
  `fit_image_into`/`draw_image`; the placeholder remains the default for undecoded entries.
- R222 — `properties_panel.rs`: Bool/Choice/Text/Number editors (`begin_edit`/`commit_edit`/
  `cancel_edit`), row-semantic wheel, keyboard focus/navigation/typing.
- R223 — `property_grid.rs`: zebra rows and Home/End verified; added PageUp/PageDown,
  double-click editing, context menu, `cell_changed`.
- R224 — `query_builder.rs`: `QueryBuilderGroup` nesting (`add_group`, `add_row_in_group`,
  `set_row_group`, `set_group_conjunction`/`set_group_negated`, `remove_group`), recursive
  `build_scope`/`load_scope` so `(a AND b) OR c` round-trips, group guide/indent painting,
  row scrolling, drag reorder, `rows_changed`. Tests
  `query_builder_set_query_round_trips_a_nested_group`,
  `nested_groups_build_a_parenthesised_expression`.

## Validation

- `cargo check --lib` — clean (no warnings in `src/widget/view_widgets/**`).
- `cargo test --lib widget::view_widgets` — `329 passed; 0 failed`.
- `rustfmt` run only on the files changed in this slice.
