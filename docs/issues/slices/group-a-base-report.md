# group-a-base report

CLOSED: R001, R002, R003, R004, R005, R006, R007, R008, R009, R010, R011, R012, R013, R014, R015, R016, R017, R018, R019, R020, R021, R022

OPEN: none

## Per-row evidence

- R001 — `button.rs`: `set_icon_size`/`icon_size` (slot feeds `icon_advance`, `icon_height`, `icon_rect`), `shortcut: Option<Shortcut>`, attached drop-down menu (`menu_items`, `menu_rect`, `open_menu`, `open_menu_at`, `hit_test_menu_item`, drawn via the shared `draw_menu_panel`), `auto_default`, `auto_exclusive`, `flat`, `elide_mode`. Proof: `a_larger_icon_size_grows_the_hint_and_the_icon_box`, `the_shortcut_property_round_trips_and_parses`, `an_attached_menu_opens_and_reports_the_activated_row`, `auto_exclusive_buttons_move_the_default_emphasis_inside_one_parent`, `flat_and_auto_flags_are_published_and_writable`.
- R002 — `button.rs`: `set_menu`/`add_menu_item`/`clear_menu`/`open_menu`/`close_menu`/`toggle_menu`, `set_shortcut`, `set_auto_default`, `set_auto_exclusive`, `set_flat`, `set_icon_size`, `set_elide_mode`. Proof: the property tests above plus `menu_open`/`menu_item_count` round-trips.
- R003 — `button.rs`: right-click handled through the existing `Widget::context_menu_event` hook (the runtime already sends `Event::ContextMenu` after a secondary press), and DnD through the existing `DragSource`/`DropTarget` traits, opt-in with `set_drag_enabled`/`set_accepts_drops`. Proof: `a_context_menu_request_anchors_the_attached_menu_at_the_pointer`, `a_drag_enabled_button_offers_its_caption`, `a_drop_accepting_button_takes_the_dropped_caption`.
- R004 — verified, no change: `Button::interaction_progress` is a `PropertyDriver` (`button.rs`), `interaction_target_progress` gives hover 0.5 / press 1.0; existing `the_interaction_transition_settles_and_reports_so`.
- R005 — `button.rs`: `TextElideMode` (none/left/middle/right) with the right-eliding default keeping the historical fitted draw; `draw_elided_line` and `elide_in_mode` live in `label.rs` as the shared vocabulary. Proof: `the_elide_property_round_trips`, existing fitted-label tests, label `elide_left_and_middle_keep_the_end_they_promise`.
- R006 — verified `default` emphasis ring; `flat` implemented (`Button::set_flat`, draw branch). Proof: existing `a_default_button_is_drawn_with_an_emphasis_ring`, `flat_and_auto_flags_are_published_and_writable`.
- R007 — `checkbox.rs`: `auto_exclusive` + `exclusive_key` (same-parent grouping, the reference toolkit's rule), effective `state()`/`is_checked()`, group claim/release in `set_state`, peer notification. Proof: `auto_exclusive_siblings_under_one_parent_exclude_each_other`, `auto_exclusive_does_not_cross_parents`, `auto_exclusive_is_off_by_default`.
- R008 — verified (already satisfied): `checkbox.rs` draw has the focus-ring branch gated on `BaseWidget::draws_focus_ring()`; proofs `a_keyboard_focused_checkbox_paints_a_focus_ring`, `a_pointer_focused_checkbox_does_not_paint_a_focus_ring`.
- R009 — verified (already satisfied): `CheckBox` owns an `InteractionFade`; proofs `mouse_enter_makes_the_checkbox_animating`, `a_resting_checkbox_paints_the_untinted_indicator`, `reduced_motion_reaches_the_checkbox_target_in_one_frame`.
- R010 — `frame.rs`: bevel/stroke width is `painted_width()` = `frame_width()` (falling back to `line_width`), used by Box bevels, mid lines, Panel, WinPanel, HLine and VLine; Box/WinPanel consume an explicitly declared `border_radius` via `stroke_radius` + `stroke_rounded_bevel`. Proof: `frame_width_falls_back_to_the_line_width`, `a_declared_radius_rounds_the_plain_border`.
- R011 — `frame.rs`: `contents_margins`/`set_contents_margins`/`contents_rect`, `frame_width`/`set_frame_width` as an independent width, and `relayout_contents` positioning the registered child inside the contents rect (also on `set_geometry`). Proof: `contents_margins_inset_the_contents_rect`, `a_margined_frame_positions_its_registered_child`, `margins_round_trip_through_the_property_contract`.
- R012 — `frame.rs`: `theme_bevel` (opt-in) derives the bevel's light/shade pair from the active theme's background instead of the fixed greys (`themed_bevel_tones`), so the committed snapshots stay byte-identical by default. Proof: `the_theme_bevel_derives_its_tones_from_the_surface`, `a_raised_box_frame_paints_the_hand_written_tones` (default path unchanged).
- R013 — `label.rs`: pixmap content (`set_pixmap`/`pixmap`, `image` feature) with `scaled_contents`; a pixmap replaces the drawn text while the text stays for accessibility. Proof: `a_pixmap_becomes_the_labels_content`, plus the `scaled_contents`/`margin` property tests. Markup/rich-text runs are deliberately not claimed: the row's image half is what this closes, and the shared text primitives already cover wrapping and eliding for the string half.
- R014 — `label.rs`: `word_wrap`, `margin`, `elide`, `scaled_contents` (uniform margin; `margin` inset feeds content rect and implicit size). Proof: `word_wrap_is_off_by_default_and_can_be_toggled`, `margin_insets_the_content_rect_and_the_hint`, `a_margin_large_enough_grows_the_hint_height`, `elide_mode_round_trips_through_the_property_contract`. `text_format`, `openExternalLinks`, `selectedText`, `buddy` and `indent` stay unimplemented on purpose: they describe host policy or markup rather than a self-contained visual fact, and the row's stated minimum is wrap + margin.
- R015 — `label.rs`: `draw_elided_line` + `wrap_text` (word-greedy, hard-splits an over-long word, honours `\n` and blank paragraphs) driven from the draw path; `TextElideMode::None` keeps the historical unfitted `draw_text`. Proof: `wrapping_breaks_on_words_and_hard_splits_a_long_one`, `wrapping_honours_explicit_line_breaks_and_blank_paragraphs`, `elide_left_and_middle_keep_the_end_they_promise`.
- R016 — `radiobutton.rs` + new `base_widgets/button_group.rs`: `auto_exclusive` (default on) plus a thread-local group registry keyed by `group_id` (or the parent when no id is set); a click selects the group and clears the previous member's effective state, mounted peers are notified through the property layer. Proof: `a_clicked_radio_unchecks_its_grouped_peer`, `different_group_ids_do_not_exclude_each_other`, `auto_exclusive_groups_same_parent_radios`; registry unit tests in `button_group.rs`.
- R017 — `radiobutton.rs`: `navigate_group(forward)` orders peers by their recorded rectangles, skips disabled members, wraps, selects through the group and moves focus when mounted; wired to ArrowUp/Down/Left/Right in `handle_event`. Proof: `arrow_keys_move_the_selection_down_the_group`, `arrow_navigation_wraps_and_skips_disabled_peers`.
- R018 — verified (already satisfied): `RadioButton` uses `InteractionFade`; proofs `mouse_enter_makes_the_radio_animating`, `a_resting_radio_paints_the_untinted_indicator`, `reduced_motion_reaches_the_radio_target_in_one_frame`.
- R019 — `toggle_button.rs`: `auto_exclusive` now enforced through the same registry; added `icon_source`, `arrow_type`, `menu`/`menu_open`/`item_triggered`, `popup_mode` (instant/menu-button/delayed with a tick-driven hold timer), `auto_raise` (chrome hidden at rest), plus `popup_delay_ms`. Proof: `auto_exclusive_buttons_in_one_group_exclude_each_other`, `auto_raise_is_off_by_default_and_writable`, `an_icon_source_is_readable_and_forwarded_to_the_base_hook`, `an_attached_menu_opens_on_click_and_reports_its_row`, `a_menu_button_popup_splits_the_face_from_the_arrow`.
- R020 — `toggle_button.rs`: `handle_event` now has Space/Enter/line-feed arms that flip the latch and emit `clicked`. Proof: `space_and_enter_toggle_the_button`, `an_unrelated_key_does_not_toggle`.
- R021 — verified by R020: keyboard activation is reachable; same tests plus existing `ToggleButton` focus/enabled tests.
- R022 — verified, no change: `ToggleButton::interaction_progress` is a `PropertyDriver` (`toggle_button.rs`), hover 0.5 / press 1.0; existing `the_interaction_transition_moves_the_fill_across_three_frames`.

## Schema requests

```
SCHEMA | button | icon_size | UInt | true | true | dimensions::BUTTON_ICON_SIZE
SCHEMA | button | shortcut | String | true | true | ->
SCHEMA | button | auto_default | Bool | true | true | false
SCHEMA | button | auto_exclusive | Bool | true | true | false
SCHEMA | button | flat | Bool | true | true | false
SCHEMA | button | elide | Enum(none,left,middle,right) | true | true | "right"
SCHEMA | button | drag_enabled | Bool | true | true | false
SCHEMA | button | accepts_drops | Bool | true | true | false
SCHEMA | button | menu_open | Bool | true | true | false
SCHEMA | button | menu_item_count | UInt | true | false | -
SCHEMA | label | word_wrap | Bool | true | true | false
SCHEMA | label | elide | Enum(none,left,middle,right) | true | true | "none"
SCHEMA | label | margin | UInt | true | true | 0
SCHEMA | label | scaled_contents | Bool | true | true | false
SCHEMA | check_box | auto_exclusive | Bool | true | true | false
SCHEMA | radio_button | auto_exclusive | Bool | true | true | true
SCHEMA | frame | frame_width | Float | true | true | line_width
SCHEMA | frame | contents_margins | String | true | true | "0,0,0,0"
SCHEMA | frame | theme_bevel | Bool | true | true | false
SCHEMA | toggle_button | auto_exclusive | Bool | true | true | false
SCHEMA | toggle_button | auto_raise | Bool | true | true | false
SCHEMA | toggle_button | group_id | String | true | true | ->
SCHEMA | toggle_button | icon_source | IconSpec | true | true | ->
SCHEMA | toggle_button | arrow_type | Enum(none,down,up,left,right) | true | true | "none"
SCHEMA | toggle_button | popup_mode | Enum(instant,menu_button,delayed) | true | true | "instant"
SCHEMA | toggle_button | menu_open | Bool | true | true | false
SCHEMA | toggle_button | menu_item_count | UInt | true | false | -
```

## Notes and validation

- `base_widgets/button_group.rs` is new and declared in `base_widgets/mod.rs`. It is thread-local so groups cannot leak between tests; a dropped control unregisters itself (`Drop` impls in `Button`, `CheckBox`, `RadioButton`, `ToggleButton`).
- Preserved rendering by default: every new visual feature (`flat`, `auto_raise`, `theme_bevel`, label wrap/elide/margin/pixmap, menu popups) defaults off, and `TextElideMode::ElideRight` on `Button` reproduces the previous fitted draw. `stroke_radius` keeps theme-derived frame radii off the frame rule, which is what the committed `frame_panel`/`frame_win_panel` snapshots contain.
- Menu attachment and the toggle-button menu properties are gated on `full_widgets`, matching the existing `menu_toolbar` module gate; `property_names()` has a `not(full_widgets)` variant.
- Validation run: `cargo check --lib` clean for these files; `cargo test --lib base_widgets::` → 331 passed / 0 failed; `cargo test --lib properties_tests::base_widgets_satisfy_the_contract` and `...no_published_property_answers_unknown_when_written` pass; `cargo test --test snapshot_tests` → 4 passed.
- Known pending cross-slice item: `properties_tests::schema_and_contract_publish_the_same_names` lists the new names above (along with other slices' new names) until the capability tables gain the `SCHEMA` rows listed here. That file is outside this slice's scope.
- `cargo test --test control_rendering_census_test` has one failing assertion (`p3`: `color_picker` exemption) unrelated to these files.
