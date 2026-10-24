# Group B2 — input widgets (R130–R162) report

Scope: `src/widget/input_widgets/{keyboard,listbox,masked_edit,mention,multi_select_combo_box,number_picker,otp_input,range_slider,rich_edit,search_bar,search_box,shortcut_editor,spinbox,tag_input}.rs`.
No files outside that set were modified. Three rows still had an open gap on inspection and were implemented
in this pass (R132, R142, R162); the other thirty were verified against the current source and their tests.

CLOSED: R130, R131, R132, R133, R134, R135, R136, R137, R138, R139, R140, R141, R142, R143, R144, R145, R146, R147, R148, R149, R150, R151, R152, R153, R154, R155, R156, R157, R158, R159, R160, R161, R162

OPEN: none

SCHEMA | keyboard | press_animation | Bool | true | true | false

## Per-row evidence

- R130 — `keyboard.rs:76-85` has `Qwerty`/`Qwertz`/`Numeric`/`Emoji`; `:510-599` builds each layout; `:295-307` Caps Lock; `:56-72`, `:649-668` dead keys; tests `keyboard_set_layout`, `the_emoji_layout_commits_its_glyph`.
- R131 — `keyboard.rs:418-448` `tick` auto-repeats a held key and opens the long-press accent popup at 500 ms; `:382-416` accent commits; `:882-909` swipe input; `:1140-1160` suggestion bar; tests `a_held_key_repeats_and_opens_the_accent_popup`, `swiping_across_keys_commits_the_crossed_text`, `a_pressed_suggestion_is_committed`.
- R132 — new opt-in `press_animation` (default off): `keyboard.rs:155-159` field, `:334-350` getter/setter, `painted_key_rect` at `:354-368` insets the pressed key and the draw adds a decaying ripple ring; test `a_press_animation_scales_the_pressed_key_only`. Default board output is unchanged.
- R133 — `listbox.rs:127-196` structured `ListBoxItem` (icon/check/enabled/data); `:771-791` `sort_items`; `:798-810` `columns`; `:813-826` `edit_triggers`; `:123-124` `rename_requested`; draw `:1539-1610` paints check, icon and columns; tests `rows_carry_icons_checks_and_data`, `columns_and_sorting_keep_rows_together`, `rename_gestures_emit_a_request`.
- R134 — `listbox.rs:1331-1345` builds `Modifiers` and routes Shift/Primary arrows through `select_with_modifiers` (`:533-570`); test `modified_arrow_keys_extend_and_toggle_the_selection`.
- R135 — `listbox.rs:1363-1398` Home/End/PageUp/PageDown; `:1417-1428` type-ahead via `find_prefix` (`:835-841`); `:1075-1160` `DragSource`/`DropTarget` reorder and transfer; tests `page_keys_type_ahead_and_disabled_rows_are_honoured`, `a_drag_within_the_list_reorders_rows_and_reports_move`.
- R136 — `masked_edit.rs:100-127` adds `D/d`, `#`, `H/h`, `B/b`; `:1387-1432` parses `\` escape, `>`/`<`/`!` case converters and `;c` placeholder; tests `the_mask_grammar_supports_case_and_a_custom_placeholder`, `a_backslash_escapes_a_mask_character`.
- R137 — `masked_edit.rs:578-588` `paste` fills the mask; `:496-529` press/drag and double-click word selection using `word_display_range` (`:594-604`); tests `paste_fills_the_mask`, `a_double_click_selects_an_input_run`.
- R138 — `mention.rs:1025-1031` handles `Event::TextInput | Event::ImeCommit` through the same insertion path; test `mention_ime_commit_inserts_text_and_opens_the_popup`.
- R139 — `mention.rs:984-1017` places the caret with `byte_index_at_x` (`:324-338`) and extends a drag selection; tests `a_click_places_the_caret_at_the_pointer`, `a_drag_selects_a_range`.
- R140 — `mention.rs:266-293` `max_visible`/`row_height` setters; `:706-707`, `:738-745` capability properties; test `the_popup_geometry_is_configurable`.
- R141 — `multi_select_combo_box.rs:838-885` keyboard arms: Escape closes, Up/Down/Page/Home/End move the active row, Space toggles, Enter toggles and closes, Primary+A selects all; test `keyboard_navigation_and_a_capped_list`.
- R142 — `multi_select_combo_box.rs:366-445` capped `max_visible`, scrolling window and `ensure_active_visible`; `:886-896` wheel scrolling; `:447-465` `select_all`; new Primary+Shift+A clears the selection (`:849-860`); tests `keyboard_navigation_and_a_capped_list`, `primary_shift_a_clears_the_open_list`.
- R143 — `number_picker.rs:195-247` prefix, decimals and special value text; `:250-279` display assembly; `:652-685` typed entry committed on Enter/Escape; tests `prefix_decimals_and_special_text_shape_the_label`, `a_typed_value_is_parsed_on_commit`.
- R144 — verified complete: `number_picker.rs:583-717` wheel, drag, double-click half, arrows, PageUp/Down, Home/End; tests `arrow_keys_step_the_value`, `a_drag_selects_the_row_under_the_pointer`, `the_value_at_a_row_offset_follows_the_wheel_direction`.
- R145 — `number_picker.rs:280-304` decaying settle offset and `is_settling`, started on release (`:627-635`); test `a_released_drag_settles_with_a_decaying_offset`.
- R146 — `otp_input.rs:573-582` `TextInput`/`ImeCommit` fill successive boxes; tests `text_input_fills_the_boxes`, `ime_commit_fills_the_next_boxes`.
- R147 — `otp_input.rs:516-571` Primary+V paste, Tab/Shift+Tab between boxes, Enter submits, Escape cancels; tests `primary_v_pastes_the_code`, `tab_steps_between_boxes`, `enter_and_escape_report_the_entry`.
- R148 — `range_slider.rs:1034-1097` Tab switches handle, arrows/PageUp/PageDown/Home/End step it, wheel steps it; test `keyboard_wheel_and_track_jump_move_the_active_handle`.
- R149 — `range_slider.rs:962-993` track press jumps the nearest handle; `:116-137` focus/active-handle state and `:896-953` focused/dragging drawing; tests `keyboard_wheel_and_track_jump_move_the_active_handle`, `focus_marks_the_active_handle`, `the_dragged_handle_is_drawn_larger`.
- R150 — `rich_edit.rs:28-49` `TextSpan`/`CharFormat`; `:185-217` `spans`/`set_spans`/`format_at`; `:943-1006` `draw_rich_line` paints per-run; test `spans_format_the_named_run`.
- R151 — `rich_edit.rs:1009-1265` drag selection, clipboard chords and wheel; `:276-308` clipboard helpers; `:220-274` `first_visible_line`, `scroll_lines`, caret-into-view; tests `a_drag_extends_the_selection`, `clipboard_shortcuts_copy_and_paste`, `the_wheel_scrolls_and_clamps`.
- R152 — `search_bar.rs:984-987` gates keys on `focused` (and `:1056-1060` gates committed text); test `keys_are_gated_on_focus`.
- R153 — `search_bar.rs:984-1054` caret movement/selection, Delete, Escape cancels; `:344-352` cancel width measured from the label; `:257-316` recent-search menu; `:336-338` `tokens`; tests `escape_cancels_the_search`, `submitted_queries_fill_the_recent_menu`, `the_caret_edits_inside_the_query_and_tokens_split_it`.
- R154 — `search_box.rs:50-55` caret/anchor/`CursorBlink`; draw paints the insertion point; tests `the_caret_drives_insertion_and_deletion`, `shift_selects_and_the_selection_is_painted`.
- R155 — `search_box.rs:236-301` caret movement, Backspace and forward Delete; test `search_box_delete_removes_forward_from_the_caret` (updated: Delete is no longer a global no-op).
- R156 — `shortcut_editor.rs:397-494` capture lifecycle (`begin_capture`/`cancel_capture`/`commit_capture`), multi-key sequence, `conflicting_shortcuts`, per-entry `clear_shortcut`/`reset_shortcut`; `:977-1044` recording event loop; tests `recording_captures_a_multi_key_sequence`, `conflicts_and_per_entry_clear_and_reset`.
- R157 — `shortcut_editor.rs:41-112` uses `Modifiers`/`primary_shortcut_matches` instead of numeric comparisons; `:496-517` `format_keys` renders ⌃ ⌥ ⇧ ⌘; tests `primary_shortcut_matches_ctrl_on_desktop_and_command_on_mac`, `key_sequences_are_rendered_with_modifier_symbols`.
- R158 — `spinbox.rs:152-158`, `:664-716` editing state, `begin_edit`/`commit_edit`/`cancel_edit`, `:1175-1235` numeric key entry; test `a_typed_value_is_committed_or_cancelled`.
- R159 — `spinbox.rs:722-754` held-button auto-repeat, `:1237-1245` wheel step, `:1205-1216` PageUp/PageDown; test `wheel_page_keys_and_auto_repeat_step_the_value`.
- R160 — `spinbox.rs:163-172` `SpinBoxStepType` (`SingleStep`/`AdaptiveDecimalStep`), `:641-651` setter, `:1337-1345` bound-aware dimmed arrows; test `adaptive_steps_and_bound_aware_buttons`.
- R161 — `tag_input.rs:54-56`, `:160-187` caret/selection model, `:101-125` Left/Right/Home/End and Delete replace the range; test `the_input_caret_edits_inside_the_buffer`.
- R162 — `tag_input.rs:229-258` batch paste, `:260-274` drag reorder, `:276-293` double-click edit, `:190-227` suggestions, `:143-157` `max_tags`; new `paste_from_clipboard`/`copy_input_to_clipboard` at `:263-296` bound to Primary+V/C at `:1079-1087`; tests `paste_creates_multiple_tags_and_respects_the_limit`, `tags_can_be_reordered_and_edited`, `tab_accepts_the_first_matching_suggestion`, `clipboard_chords_paste_a_list_and_copy_the_buffer`.

Note (R130, schema not in this slice): the capability `KEYBOARD_PROPERTIES` table still advertises only
`layout` tokens `qwerty` and `numeric`; the widget now also accepts `qwertz` and `emoji`. The main thread
should extend that enumeration (no widget change needed).

## Validation

- `cargo check --lib` — passes.
- `cargo test --lib -- <the eight new tests>` — 8 passed, 0 failed.
- `cargo test --lib -- input_widgets::{keyboard,listbox,masked_edit,mention,multi_select_combo_box,number_picker,otp_input}::tests` — 208 passed, 0 failed.
- `cargo test --lib -- input_widgets::{range_slider,rich_edit,search_bar,search_box,shortcut_editor,spinbox,tag_input}::tests` — 210 passed, 0 failed.
- `rustfmt --edition 2021 --check` on the five changed files — clean.
