# group-b1-input — report (R087–R129)

CLOSED: R087, R088, R089, R090, R091, R092, R093, R094, R095, R096, R097, R098, R099, R100, R101, R102, R103, R104, R105, R106, R107, R108, R109, R110, R111, R112, R113, R114, R115, R116, R117, R118, R119, R120, R121, R122, R123, R124, R125, R126, R127, R128, R129
OPEN: none

> Note for the main thread: the properties below are published by `property_names()` but are not
> yet declared by the capability tables. The `SCHEMA` lines at the bottom of this file are the
> rows that must be inserted. The `line_edit` row also covers `password_edit`, `phone_input` and
> `email_input`: all four canonical names share `LINE_EDIT_PROPERTIES`. Until the rows land,
> `capability::properties_tests::schema_and_contract_publish_the_same_names` lists these pairs
> alongside the names other slices have already published (that test is red for several slices
> for the same reason). No other test is affected by this slice's additions.

## Row notes

- R087 — `lineedit.rs`: `modified` (`is_modified`/`set_modified`, test `the_modified_flag_tracks_user_edits_only`), `clear_button_enabled` + painted cross + press-to-erase (`the_optional_clear_button_erases_on_press`), `password_echo_on_edit` (`password_echo_on_edit_reveals_only_while_focused`). The fourth echo mode is a field-level modifier rather than a new `EchoMode` variant because `EchoMode` is the canonical `platform` enum; the rationale is recorded on the field.
- R088 — `lineedit.rs`: `MouseDoubleClick` selects the word, a third press in the same streak selects all (`a_double_click_selects_the_word`), plus `selection_changed` / `cursor_position_changed` (`selection_and_caret_signals_report_moves`).
- R089 — `lineedit.rs` `Draw::draw`: per-line selection band measured through the renderer and clipped to the value box (`a_selection_paints_a_band`).
- R090 — `lineedit.rs`: `scroll_x` + `max_scroll_x` + `ensure_caret_visible`; scrolled paint clipped, caret clamped to the value box (`a_long_value_scrolls_to_reveal_the_caret`).
- R091 — `lineedit.rs`: Primary+Left/Right step by word (`primary_arrows_step_by_word`); `DragSource`/`DropTarget` with `text/plain` and mask rules (`a_selection_is_a_drag_payload`).
- R092 — `lineedit.rs` `Draw::draw`: `FocusRing::for_control` + theme `outline` token via `metrics::focus_ring_color` (`keyboard_focus_draws_the_focus_ring`).
- R093 — `textarea.rs`: `soft_wrap` word-boundary rows (`soft_wrap_adds_word_boundary_rows`), `content_margin` (`a_content_margin_insets_the_value`), `max_block_count` (`a_block_budget_drops_the_oldest_lines`), and `scroll_x` driven by the caret (`a_long_unwrapped_line_scrolls_to_reveal_the_caret`).
- R094 — `textarea.rs` `handle_event`: `Wheel` arm (`textarea_wheel_scrolls_and_clamps`), Primary+C/X/V (`clipboard_shortcuts_copy_and_paste`), double-click word (`a_double_click_selects_a_word`), Page Up/Down (`page_keys_move_the_caret`), Tab inserts `\t` (`tab_inserts_a_tab_character`).
- R095 — `textarea.rs`: `band_width_cache` keyed by `revision`; `band_advance` memoises each selected run (`the_band_width_cache_is_scoped_to_the_revision`).
- R096 — `textarea.rs`: `TextDeltaCommand` records `(offset, removed, inserted)` instead of a whole snapshot (`an_edit_is_recorded_as_a_delta`).
- R097 — `textedit.rs`: `TextSpan { range, format }` + `CharFormat { bold, italic, underline, color }`, `set_spans`/`format_at`, painted by `flush_row_rich` (`spans_format_the_named_run`).
- R098 — `textedit.rs` `rows_for`: wrap breaks at the last space or hyphen, falling back to a character break only for an unbroken token (`soft_wrap_breaks_at_word_boundaries`).
- R099 — `textedit.rs` `handle_event`: `Wheel` (`textedit_wheel_scrolls_and_clamps`), Primary+C/X/V (`primary_v_pastes_at_the_caret`), double-click word (`a_double_click_selects_a_word`), Page Up/Down (`page_keys_move_by_a_window`).
- R100 — `textedit.rs`: `revision` counter identifies the document for `RowCache` (`the_wrap_cache_uses_the_revision_counter`).
- R101 — `combobox.rs`: `ComboBoxItem { icon, data, enabled }` kept beside the labels (`item_meta`/`set_item_meta`/`set_item_icon`/`set_item_enabled`), row icon painted, disabled rows skipped (`structured_rows_and_disabled_navigation`).
- R102 — `combobox.rs`: `ComboBoxInsertPolicy` (five variants, `insert_item`) and `ComboBoxSizeAdjustPolicy` honored by `size_hint`; both published as tokens (`the_insert_and_size_adjust_policies_are_published`).
- R103 — `combobox.rs`: wheel steps the closed value / open highlight, Page/Home/End, first-letter type-ahead, optional `Completer` (`the_wheel_steps_the_closed_value_and_the_open_highlight`, `the_dropdown_fades_and_the_completer_is_consulted`).
- R104 — `combobox.rs`: `reveal_progress` advanced by `tick` over 150 ms; `is_animating` true until complete; list fill and border use the alpha.
- R105 — `editable_combo_box.rs`: caret + `selection_anchor`, Left/Right/Home/End/Delete/Backspace step by character, mouse press places the caret (`the_field_has_a_caret_model`, `the_caret_and_row_cap_are_published`).
- R106 — `editable_combo_box.rs`: `max_visible_items` caps the popup, `page_size`/`scroll_offset`, auto-reveal of the selection (`the_open_list_filters_and_scrolls`); cap and offset published.
- R107 — `editable_combo_box.rs`: wheel scrolls the open list; the list filters live on the value (`filtered_items`).
- R108 — `font_combo_box.rs` `Draw::draw`: each popup row is drawn with `Font::simple(family)`, with an optional `sample_text` preview column (`font_combo_box_draw_produces_output`, `the_filter_properties_round_trip`).
- R109 — `font_combo_box.rs`: `FontFilters` (Monospaced/Scalable/Proportional) + `writing_system_filter`; both published by token (`font_filters_narrow_the_list`, `the_filter_properties_round_trip`).
- R110 — `font_combo_box.rs`: `popup_row_at` hit test; a press on a row commits that row; a press outside dismisses; wheel scrolls the capped popup; type-ahead and Page/Home/End (`the_popup_scrolls_and_dismisses_on_an_outside_press`, `font_combo_box_cycles_only_on_a_completed_press`).
- R111 — `auto_complete_edit.rs`: `CompletionMatchMode` Contains/StartsWith/EndsWith/FixedString/RegExp, `case_sensitive`, word-level `completion_prefix` (`match_modes_choose_the_filter`, `the_pattern_mode_matches_patterns`, `the_completion_contract_is_published`).
- R112 — `auto_complete_edit.rs`: caret with Left/Right/Home/End/Backspace/Delete, mid-value insert, Tab accepts the completion, `inline_completion` shows the remaining suffix (`the_caret_and_inline_completion_work`).
- R113 — `auto_complete_edit.rs`: `byte_index_at_x` places the caret on press; `CompletionClosePolicy` Always/Never/OnCommit (`the_close_policy_controls_the_popup`).
- R114 — `cascader.rs`: `CascaderOption::enabled` + `with_enabled`, disabled rows are skipped by pointer and keyboard (`disabled_rows_are_not_choosable`); Loading/Failed states were already present (`a_loading_branch_is_not_a_selectable_value`).
- R115 — `cascader.rs` `handle_event`: `Event::ImeCommit` takes the same filter branch as `TextInput` (`cascader_ime_commit_filters_the_browsed_level`).
- R116 — `cascader.rs` `handle_event`: Right descends, Left retreats, Enter commits a leaf, Home/End jump within the level (`keyboard_walks_levels_and_commits`).
- R117 — `cascader.rs`: wheel scrolls the deepest column, `max_visible` caps each column (now published), 120 ms reveal fade (`a_capped_level_scrolls_with_the_wheel`, `the_open_overlay_follows_the_appearance`).
- R118 — `command_link.rs`: opt-in shield slot — `shield_visible`/`set_shield_visible`, leading gutter that shifts both text lines, shield + check drawn from line segments (`the_shield_slot_is_opt_in_and_shifts_the_label`).
- R119 — `command_link.rs` `handle_event`: press arms via `set_grabbed`, release inside commits, release outside or `FocusLost` cancels; Enter/line feed/Space activate (`a_completed_press_activates_and_a_drag_off_cancels`, `enter_and_space_activate_the_link`).
- R120 — `dropdown.rs`: `max_visible` + `scroll_offset` with clamping, painted window (`a_capped_list_scrolls_and_pages`); both published (`the_popup_window_is_published`).
- R121 — `dropdown.rs` `handle_event`: `Wheel`, Page Up/Down, Home/End, first-letter type-ahead with wrap (`a_capped_list_scrolls_and_pages`).
- R122 — `ime_preedit.rs` `handle_event`: consumes real `Event::ImePreedit` (text + caret) and clears on `Event::ImeCommit` (`platform_composition_events_drive_the_overlay`).
- R123 — `ime_preedit.rs`: `PreeditSegment { text, color, underline, cursor }` + `set_segments`, painted per segment (`styled_segments_are_painted_and_readable_as_text`).
- R124 — `ime_preedit.rs`: `caret_rect`/`caret_screen_position` report the caret box for candidate-window anchoring (`the_caret_position_is_reported_for_candidate_window_anchoring`).
- R125 — `inplace_editor.rs`: `EditTriggers` bitmask (NONE/SINGLE_CLICK/DOUBLE_CLICK/F2), published as `edit_triggers` tokens (`edit_triggers_are_configurable`).
- R126 — `inplace_editor.rs`: press places the caret, drag selects, Home/End/Up/Down (`a_click_places_the_caret_and_a_drag_selects`, `home_end_and_arrows_move_the_caret`).
- R127 — `inplace_editor.rs`: public `delete_forward` deletes the selection or the character after the caret, no `\u{ffff}` sentinel (`delete_forward_removes_the_character_after_the_caret`).
- R128 — `input_validator.rs`: `Validator` trait with `state`/`message`/`fixup`/`filter`; `RangeValidator` (Int/Double), `PatternValidator` (anchors, classes, quantifiers), `InputValidator::fixup` (`a_range_rule_separates_incomplete_from_out_of_range`, `a_pattern_rule_supports_anchors_classes_and_quantifiers`).
- R129 — `lineedit.rs`: `LineEdit.validator` is `Option<Box<dyn Validator>>` with `set_dyn_validator`; `ValidationState` Invalid/Intermediate/Acceptable cached in `validation_state`, `validation_state()`/`is_valid()`, `fixup()`, published as `validation_state`; Intermediate shows the message but is not painted as an error (`a_host_rule_can_be_installed_behind_the_trait`, `a_repairable_value_can_be_fixed_up`).

## SCHEMA requests

SCHEMA | command_link | shield | Bool | true | true | false
SCHEMA | line_edit | validation_state | String | true | false | "acceptable"
SCHEMA | combo_box | insert_policy | String | true | true | "insert_at_bottom"
SCHEMA | combo_box | size_adjust_policy | String | true | true | "adjust_to_contents"
SCHEMA | editable_combo_box | cursor_position | UInt | true | true | 0
SCHEMA | editable_combo_box | max_visible_items | UInt | true | true | ->
SCHEMA | editable_combo_box | scroll_offset | UInt | true | true | 0
SCHEMA | font_combo_box | font_filters | String | true | true | ""
SCHEMA | font_combo_box | writing_system_filter | String | true | true | ->
SCHEMA | font_combo_box | sample_text | String | true | true | ->
SCHEMA | auto_complete_edit | match_mode | String | true | true | "contains"
SCHEMA | auto_complete_edit | case_sensitive | Bool | true | true | false
SCHEMA | auto_complete_edit | inline_completion | Bool | true | true | false
SCHEMA | auto_complete_edit | close_policy | String | true | true | "always"
SCHEMA | auto_complete_edit | completion_prefix | String | true | false | ""
SCHEMA | dropdown | max_visible | UInt | true | true | ->
SCHEMA | dropdown | scroll_offset | UInt | true | true | 0
SCHEMA | cascader | max_visible | UInt | true | true | ->
