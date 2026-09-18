// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `create_*` must build the control it is named after.
//!
//! # The defect this closes
//!
//! `ControlBackend::create_web_view(..)` mounted `WidgetKind::WebEngineView`, and
//! that kind resolved through the capability registry to **`media_player`**: the
//! kind's capability entry was occupied by `MediaPlayer`, so `capability_by_kind`
//! handed back the media player's capability and this call — documented and
//! C-exported as "create a web view" — built a `MediaPlayer` and returned a valid,
//! non-zero id. Every existing check passed, including
//! `control_backend::custom::tests`'s `create_web_view_allocates_valid_id`, because
//! an id is exactly what the defect produced.
//!
//! # Why the assertion is about the *control*, not the id
//!
//! "Returned an id" cannot distinguish "built a web view" from "built a media
//! player". The discriminating question is what the id addresses, so this test
//! walks the backend's whole `create_*` surface and asks the **mounted control** for
//! its capability. `WidgetFactory::capability_for_kind_instance` resolves by
//! concrete type — the same resolution the property and event paths use — so a
//! control that was silently substituted reports its own name, not the one that was
//! requested.
//!
//! # What the expected name is
//!
//! The registered control name, because that is the only spelling the factory
//! guarantees. Where a name legitimately differs from the method (`create_wizard`
//! builds `wizard_dialog`; the whole `WebEngine*` family addresses one `web_view`),
//! the expected value is the alias target and the row is annotated. A method that
//! quietly built something *unrelated* would have no such annotation and would fail.

#![cfg(all(feature = "desktop", not(alloc_frugal)))]

use rust_widgets::control_backend::{ControlBackend, CustomPaintControlBackend};
use rust_widgets::core::ObjectId;
use rust_widgets::widget::capability::WidgetFactory;

/// Bounds for every control, large enough that no control is laid out to nothing.
const W: u32 = 400;
const H: u32 = 300;

/// The capability name of the control mounted at `id`, or `None` when the id
/// addresses nothing.
///
/// Read from the live control rather than from the request, so a control that was
/// silently substituted cannot answer with the name that was asked for.
fn mounted_control_name(id: ObjectId) -> Option<&'static str> {
    rust_widgets::widget::runtime::with_widget_mut(id, |widget| {
        let factory = WidgetFactory::new_with_defaults();
        factory.capability_for_kind_instance(widget).map(|capability| capability.canonical_name)
    })
    .flatten()
}

/// One row of the table: the method, the call, and the control it must build.
type CreationCase = (&'static str, ObjectId, &'static str);

/// Every `create_*` on the custom backend, and the control each must build.
fn creation_cases(backend: &CustomPaintControlBackend, parent: ObjectId) -> Vec<CreationCase> {
    vec![
        ("create_action", backend.create_action(parent, "t", 0, 0, W, H), "action"),
        (
            "create_activity_indicator",
            backend.create_activity_indicator(parent, 0, 0, W, H),
            "progress_bar",
        ),
        (
            "create_adaptive_scaffold",
            backend.create_adaptive_scaffold(parent, 0, 0, W, H),
            "adaptive_scaffold",
        ),
        (
            "create_animated_image",
            backend.create_animated_image(parent, 0, 0, W, H),
            "animated_image",
        ),
        ("create_app_bar", backend.create_app_bar(parent, 0, 0, W, H), "app_bar"),
        ("create_arc", backend.create_arc(parent, 0, 0, W, H), "arc"),
        (
            "create_audio_visualizer",
            backend.create_audio_visualizer(parent, 0, 0, W, H),
            "audio_visualizer",
        ),
        (
            "create_auto_complete_edit",
            backend.create_auto_complete_edit(parent, 0, 0, W, H),
            "auto_complete_edit",
        ),
        ("create_avatar", backend.create_avatar(parent, 0, 0, W, H), "avatar"),
        ("create_badge", backend.create_badge(parent, 0, 0, W, H), "badge"),
        ("create_banner", backend.create_banner(parent, 0, 0, W, H), "banner"),
        ("create_bar_chart", backend.create_bar_chart(parent, 0, 0, W, H), "bar_chart"),
        (
            "create_barcode_scanner",
            backend.create_barcode_scanner(parent, 0, 0, W, H),
            "barcode_scanner",
        ),
        (
            "create_bezier_curve_editor",
            backend.create_bezier_curve_editor(parent, 0, 0, W, H),
            "bezier_curve_editor",
        ),
        (
            "create_bottom_navigation_bar",
            backend.create_bottom_navigation_bar(parent, 0, 0, W, H),
            "bottom_navigation_bar",
        ),
        ("create_bottom_sheet", backend.create_bottom_sheet(parent, 0, 0, W, H), "bottom_sheet"),
        ("create_button", backend.create_button(parent, "t", 0, 0, W, H), "button"),
        ("create_calendar", backend.create_calendar(parent, 0, 0, W, H), "calendar"),
        (
            "create_camera_preview",
            backend.create_camera_preview(parent, 0, 0, W, H),
            "camera_preview",
        ),
        (
            "create_candlestick_chart",
            backend.create_candlestick_chart(parent, 0, 0, W, H),
            "candlestick_chart",
        ),
        ("create_canvas", backend.create_canvas(parent, 0, 0, W, H), "canvas"),
        ("create_carousel", backend.create_carousel(parent, 0, 0, W, H), "carousel"),
        ("create_cascader", backend.create_cascader(parent, 0, 0, W, H), "cascader"),
        ("create_chart", backend.create_chart(parent, 0, 0, W, H), "chart"),
        ("create_check_list_box", backend.create_check_list_box(parent, 0, 0, W, H), "list_box"),
        ("create_checkbox", backend.create_checkbox(parent, "t", 0, 0, W, H), "check_box"),
        ("create_chip", backend.create_chip(parent, 0, 0, W, H), "chip"),
        (
            "create_collapsible_pane",
            backend.create_collapsible_pane(parent, "t", 0, 0, W, H),
            "collapsible_pane",
        ),
        (
            "create_color_dialog",
            backend.create_color_dialog(parent, "t", 0, 0, W, H),
            "color_dialog",
        ),
        ("create_color_history", backend.create_color_history(parent, 0, 0, W, H), "color_history"),
        ("create_color_picker", backend.create_color_picker(parent, 0, 0, W, H), "color_picker"),
        ("create_color_well", backend.create_color_well(parent, 0, 0, W, H), "color_well"),
        ("create_column_view", backend.create_column_view(parent, 0, 0, W, H), "tree_view"),
        ("create_combo_box", backend.create_combo_box(parent, 0, 0, W, H), "combo_box"),
        (
            "create_command_link",
            backend.create_command_link(parent, "t", 0, 0, W, H),
            "command_link",
        ),
        ("create_context_menu", backend.create_context_menu(parent, "t", 0, 0, W, H), "menu"),
        (
            "create_cupertino_alert_dialog",
            backend.create_cupertino_alert_dialog(parent, 0, 0, W, H),
            "cupertino_alert_dialog",
        ),
        (
            "create_cupertino_date_picker",
            backend.create_cupertino_date_picker(parent, 0, 0, W, H),
            "cupertino_date_picker",
        ),
        (
            "create_cupertino_navigation_bar",
            backend.create_cupertino_navigation_bar(parent, 0, 0, W, H),
            "cupertino_navigation_bar",
        ),
        (
            "create_cupertino_segmented_control",
            backend.create_cupertino_segmented_control(parent, 0, 0, W, H),
            "cupertino_segmented_control",
        ),
        (
            "create_cupertino_slider",
            backend.create_cupertino_slider(parent, 0, 0, W, H),
            "cupertino_slider",
        ),
        (
            "create_cupertino_switch",
            backend.create_cupertino_switch(parent, 0, 0, W, H),
            "cupertino_switch",
        ),
        ("create_data_view", backend.create_data_view(parent, 0, 0, W, H), "virtual_list"),
        ("create_date_picker", backend.create_date_picker(parent, 0, 0, W, H), "date_edit"),
        (
            "create_date_range_picker",
            backend.create_date_range_picker(parent, 0, 0, W, H),
            "date_range_picker",
        ),
        (
            "create_date_time_picker",
            backend.create_date_time_picker(parent, 0, 0, W, H),
            "date_time_edit",
        ),
        ("create_depth_chart", backend.create_depth_chart(parent, 0, 0, W, H), "depth_chart"),
        ("create_dial", backend.create_dial(parent, 0, 0, W, H), "dial"),
        ("create_dialog", backend.create_dialog(parent, "t", 0, 0, W, H), "popup_window"),
        (
            "create_directory_dialog",
            backend.create_directory_dialog(parent, "t", 0, 0, W, H),
            "file_dialog",
        ),
        ("create_divider", backend.create_divider(parent, 0, 0, W, H), "divider"),
        ("create_dock_panel", backend.create_dock_panel(parent, 0, 0, W, H), "dock_widget"),
        ("create_dock_widget", backend.create_dock_widget(parent, "t", 0, 0, W, H), "dock_widget"),
        ("create_double_spin_box", backend.create_double_spin_box(parent, 0, 0, W, H), "spin_box"),
        ("create_dropdown", backend.create_dropdown(parent, 0, 0, W, H), "dropdown"),
        ("create_dropdown_menu", backend.create_dropdown_menu(parent, 0, 0, W, H), "dropdown_menu"),
        (
            "create_editable_combo_box",
            backend.create_editable_combo_box(parent, 0, 0, W, H),
            "editable_combo_box",
        ),
        ("create_emoji_picker", backend.create_emoji_picker(parent, 0, 0, W, H), "emoji_picker"),
        ("create_empty_state", backend.create_empty_state(parent, 0, 0, W, H), "empty_state"),
        ("create_fab", backend.create_fab(parent, 0, 0, W, H), "fab"),
        ("create_file_dialog", backend.create_file_dialog(parent, "t", 0, 0, W, H), "file_dialog"),
        (
            "create_find_replace_dialog",
            backend.create_find_replace_dialog(parent, 0, 0, W, H),
            "find_replace_dialog",
        ),
        (
            "create_floating_label",
            backend.create_floating_label(parent, 0, 0, W, H),
            "floating_label",
        ),
        (
            "create_font_combo_box",
            backend.create_font_combo_box(parent, 0, 0, W, H),
            "font_combo_box",
        ),
        ("create_font_dialog", backend.create_font_dialog(parent, "t", 0, 0, W, H), "font_dialog"),
        ("create_font_preview", backend.create_font_preview(parent, 0, 0, W, H), "font_preview"),
        ("create_frame", backend.create_frame(parent, 0, 0, W, H), "frame"),
        (
            "create_freeform_shape",
            backend.create_freeform_shape(parent, 0, 0, W, H),
            "freeform_shape",
        ),
        ("create_grid", backend.create_grid(parent, 0, 0, W, H), "grid"),
        ("create_grid_table", backend.create_grid_table(parent, 0, 0, W, H), "grid_table"),
        ("create_group_box", backend.create_group_box(parent, "t", 0, 0, W, H), "group_box"),
        (
            "create_hero_animation",
            backend.create_hero_animation(parent, 0, 0, W, H),
            "hero_animation",
        ),
        ("create_icon", backend.create_icon(parent, 0, 0, W, H), "icon"),
        ("create_image_gallery", backend.create_image_gallery(parent, 0, 0, W, H), "image_gallery"),
        ("create_image_view", backend.create_image_view(parent, 0, 0, W, H), "image_view"),
        ("create_ime_preedit", backend.create_ime_preedit(parent, 0, 0, W, H), "ime_preedit"),
        (
            "create_indicator_chart",
            backend.create_indicator_chart(parent, 0, 0, W, H),
            "indicator_chart",
        ),
        (
            "create_inplace_editor",
            backend.create_inplace_editor(parent, 0, 0, W, H),
            "inplace_editor",
        ),
        ("create_kanban_board", backend.create_kanban_board(parent, 0, 0, W, H), "kanban_board"),
        ("create_keyboard", backend.create_keyboard(parent, 0, 0, W, H), "keyboard"),
        ("create_label", backend.create_label(parent, "t", 0, 0, W, H), "label"),
        ("create_lcd_number", backend.create_lcd_number(parent, 0, 0, W, H), "lcd_number"),
        ("create_line", backend.create_line(parent, 0, 0, W, H), "line"),
        ("create_line_chart", backend.create_line_chart(parent, 0, 0, W, H), "line_chart"),
        ("create_line_edit", backend.create_line_edit(parent, "t", 0, 0, W, H), "line_edit"),
        ("create_list_box", backend.create_list_box(parent, 0, 0, W, H), "list_box"),
        ("create_list_view", backend.create_list_view(parent, 0, 0, W, H), "list_view"),
        ("create_lottie_widget", backend.create_lottie_widget(parent, 0, 0, W, H), "lottie_widget"),
        ("create_masked_edit", backend.create_masked_edit(parent, 0, 0, W, H), "masked_edit"),
        (
            "create_masonry_layout",
            backend.create_masonry_layout(parent, 0, 0, W, H),
            "masonry_layout",
        ),
        (
            "create_material_navigation_rail",
            backend.create_material_navigation_rail(parent, 0, 0, W, H),
            "material_navigation_rail",
        ),
        (
            "create_material_snackbar",
            backend.create_material_snackbar(parent, 0, 0, W, H),
            "material_snackbar",
        ),
        ("create_mdi_area", backend.create_mdi_area(parent, 0, 0, W, H), "mdi_area"),
        ("create_mention", backend.create_mention(parent, 0, 0, W, H), "mention"),
        ("create_menu", backend.create_menu(parent, "t", 0, 0, W, H), "menu"),
        ("create_menu_bar", backend.create_menu_bar(parent, 0, 0, W, H), "menu_bar"),
        ("create_menu_button", backend.create_menu_button(parent, 0, 0, W, H), "menu_button"),
        (
            "create_message_box",
            backend.create_message_box(parent, "t", "t", 0, 0, W, H),
            "message_box",
        ),
        ("create_meter", backend.create_meter(parent, 0, 0, W, H), "meter"),
        ("create_mini_canvas", backend.create_mini_canvas(parent, 0, 0, W, H), "mini_canvas"),
        ("create_mini_chart", backend.create_mini_chart(parent, 0, 0, W, H), "mini_chart"),
        (
            "create_mobile_date_picker",
            backend.create_mobile_date_picker(parent, 0, 0, W, H),
            "mobile_date_picker",
        ),
        (
            "create_modal_bottom_sheet",
            backend.create_modal_bottom_sheet(parent, 0, 0, W, H),
            "modal_bottom_sheet",
        ),
        (
            "create_multi_select_combo_box",
            backend.create_multi_select_combo_box(parent, 0, 0, W, H),
            "multi_select_combo_box",
        ),
        (
            "create_navigation_drawer",
            backend.create_navigation_drawer(parent, 0, 0, W, H),
            "navigation_drawer",
        ),
        (
            "create_navigation_stack",
            backend.create_navigation_stack(parent, 0, 0, W, H),
            "navigation_stack",
        ),
        ("create_number_picker", backend.create_number_picker(parent, 0, 0, W, H), "number_picker"),
        ("create_order_book", backend.create_order_book(parent, 0, 0, W, H), "order_book"),
        ("create_otp_input", backend.create_otp_input(parent, 0, 0, W, H), "otp_input"),
        ("create_pagination", backend.create_pagination(parent, 0, 0, W, H), "pagination"),
        ("create_panel", backend.create_panel(parent, 0, 0, W, H), "group_box"),
        ("create_pie_chart", backend.create_pie_chart(parent, 0, 0, W, H), "pie_chart"),
        ("create_pie_menu", backend.create_pie_menu(parent, 0, 0, W, H), "pie_menu"),
        ("create_popover", backend.create_popover(parent, 0, 0, W, H), "popover"),
        (
            "create_popup_window",
            backend.create_popup_window(parent, "t", 0, 0, W, H),
            "popup_window",
        ),
        ("create_progress_bar", backend.create_progress_bar(parent, 0, 0, W, H), "progress_bar"),
        (
            "create_progress_circle",
            backend.create_progress_circle(parent, 0, 0, W, H),
            "progress_circle",
        ),
        (
            "create_properties_panel",
            backend.create_properties_panel(parent, 0, 0, W, H),
            "properties_panel",
        ),
        ("create_property_grid", backend.create_property_grid(parent, 0, 0, W, H), "property_grid"),
        ("create_qr_code", backend.create_qr_code(parent, 0, 0, W, H), "qr_code"),
        ("create_query_builder", backend.create_query_builder(parent, 0, 0, W, H), "query_builder"),
        ("create_quote_board", backend.create_quote_board(parent, 0, 0, W, H), "quote_board"),
        ("create_radar_chart", backend.create_radar_chart(parent, 0, 0, W, H), "radar_chart"),
        (
            "create_radio_button",
            backend.create_radio_button(parent, "t", 0, 0, W, H),
            "radio_button",
        ),
        ("create_range_slider", backend.create_range_slider(parent, 0, 0, W, H), "range_slider"),
        ("create_rating", backend.create_rating(parent, 0, 0, W, H), "rating"),
        (
            "create_refresh_control",
            backend.create_refresh_control(parent, 0, 0, W, H),
            "refresh_control",
        ),
        ("create_ribbon_bar", backend.create_ribbon_bar(parent, 0, 0, W, H), "ribbon_bar"),
        ("create_rich_edit", backend.create_rich_edit(parent, "t", 0, 0, W, H), "rich_edit"),
        ("create_rive_widget", backend.create_rive_widget(parent, 0, 0, W, H), "rive_widget"),
        ("create_roller", backend.create_roller(parent, 0, 0, W, H), "roller"),
        ("create_safe_area", backend.create_safe_area(parent, 0, 0, W, H), "safe_area"),
        ("create_scroll_area", backend.create_scroll_area(parent, 0, 0, W, H), "scroll_area"),
        ("create_scroll_bar", backend.create_scroll_bar(parent, 0, 0, W, H), "scroll_bar"),
        ("create_search_bar", backend.create_search_bar(parent, 0, 0, W, H), "search_bar"),
        ("create_search_box", backend.create_search_box(parent, 0, 0, W, H), "search_box"),
        (
            "create_segmented_button",
            backend.create_segmented_button(parent, 0, 0, W, H),
            "segmented_button",
        ),
        (
            "create_shortcut_editor",
            backend.create_shortcut_editor(parent, 0, 0, W, H),
            "shortcut_editor",
        ),
        (
            "create_skeleton_loader",
            backend.create_skeleton_loader(parent, 0, 0, W, H),
            "skeleton_loader",
        ),
        ("create_slider", backend.create_slider(parent, 0, 0, W, H), "slider"),
        ("create_sparkline", backend.create_sparkline(parent, 0, 0, W, H), "sparkline"),
        ("create_spin_box", backend.create_spin_box(parent, 0, 0, W, H), "spin_box"),
        ("create_spinner", backend.create_spinner(parent, 0, 0, W, H), "spinner"),
        ("create_splash_screen", backend.create_splash_screen(parent, 0, 0, W, H), "splash_screen"),
        ("create_splitter", backend.create_splitter(parent, 0, 0, W, H), "splitter"),
        ("create_stack_widget", backend.create_stack_widget(parent, 0, 0, W, H), "stacked_widget"),
        ("create_status_bar", backend.create_status_bar(parent, "t", 0, 0, W, H), "status_bar"),
        ("create_stepper", backend.create_stepper(parent, 0, 0, W, H), "stepper"),
        (
            "create_swipe_to_dismiss",
            backend.create_swipe_to_dismiss(parent, 0, 0, W, H),
            "swipe_to_dismiss",
        ),
        ("create_switch", backend.create_switch(parent, 0, 0, W, H), "switch"),
        ("create_tab_bar", backend.create_tab_bar(parent, 0, 0, W, H), "tab_bar"),
        ("create_tab_view", backend.create_tab_view(parent, 0, 0, W, H), "tab_view"),
        ("create_tab_widget", backend.create_tab_widget(parent, 0, 0, W, H), "tab_widget"),
        ("create_table", backend.create_table(parent, 0, 0, W, H), "table"),
        ("create_tag_input", backend.create_tag_input(parent, 0, 0, W, H), "tag_input"),
        ("create_text_area", backend.create_text_area(parent, 0, 0, W, H), "text_area"),
        ("create_text_edit", backend.create_text_edit(parent, "t", 0, 0, W, H), "text_edit"),
        ("create_time_picker", backend.create_time_picker(parent, 0, 0, W, H), "time_edit"),
        ("create_toast", backend.create_toast(parent, 0, 0, W, H), "toast"),
        (
            "create_toggle_button",
            backend.create_toggle_button(parent, "t", 0, 0, W, H),
            "toggle_button",
        ),
        ("create_tool_bar", backend.create_tool_bar(parent, 0, 0, W, H), "tool_bar"),
        ("create_tool_box", backend.create_tool_box(parent, 0, 0, W, H), "tool_box"),
        ("create_tool_button", backend.create_tool_button(parent, "t", 0, 0, W, H), "tool_button"),
        ("create_toolbox", backend.create_toolbox(parent, 0, 0, W, H), "tool_box"),
        ("create_tooltip", backend.create_tooltip(parent, 0, 0, W, H), "tooltip"),
        ("create_tree_view", backend.create_tree_view(parent, 0, 0, W, H), "tree_view"),
        ("create_undo_view", backend.create_undo_view(parent, 0, 0, W, H), "list_view"),
        ("create_video_player", backend.create_video_player(parent, 0, 0, W, H), "video_player"),
        ("create_volume_chart", backend.create_volume_chart(parent, 0, 0, W, H), "volume_chart"),
        (
            "create_web_engine_context_menu_request",
            backend.create_web_engine_context_menu_request(parent, 0, 0, W, H),
            "web_engine_view",
        ),
        (
            "create_web_engine_cookie_store",
            backend.create_web_engine_cookie_store(parent, 0, 0, W, H),
            "web_engine_view",
        ),
        (
            "create_web_engine_download_item",
            backend.create_web_engine_download_item(parent, 0, 0, W, H),
            "web_engine_view",
        ),
        (
            "create_web_engine_find_text_result",
            backend.create_web_engine_find_text_result(parent, 0, 0, W, H),
            "web_engine_view",
        ),
        (
            "create_web_engine_notification",
            backend.create_web_engine_notification(parent, 0, 0, W, H),
            "web_engine_view",
        ),
        (
            "create_web_engine_page",
            backend.create_web_engine_page(parent, 0, 0, W, H),
            "web_engine_view",
        ),
        (
            "create_web_engine_script_dialog",
            backend.create_web_engine_script_dialog(parent, 0, 0, W, H),
            "web_engine_view",
        ),
        (
            "create_web_engine_settings",
            backend.create_web_engine_settings(parent, 0, 0, W, H),
            "web_engine_view",
        ),
        (
            "create_web_engine_view",
            backend.create_web_engine_view(parent, 0, 0, W, H),
            "web_engine_view",
        ),
        (
            "create_web_engine_web_channel",
            backend.create_web_engine_web_channel(parent, 0, 0, W, H),
            "web_engine_view",
        ),
        ("create_web_view", backend.create_web_view(parent, 0, 0, W, H), "web_engine_view"),
        ("create_wizard", backend.create_wizard(parent, "t", 0, 0, W, H), "wizard_dialog"),
        ("create_wizard_dialog", backend.create_wizard_dialog(parent, 0, 0, W, H), "wizard_dialog"),
    ]
}

/// Every `create_*` on the custom backend builds the control its name promises.
///
/// Fails once, listing **every** mismatch, because the fixes differ: an id of `0`
/// means "built nothing" (a name or a profile problem) while a mismatched name means
/// "built the wrong control" (a kind→capability problem). Reporting one at a time
/// would hide the second behind the first.
#[test]
fn every_create_method_mounts_the_control_it_is_named_after() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 1000, 800);
    assert_ne!(parent, 0, "the fixture window must exist before controls are mounted");

    let cases = creation_cases(&backend, parent);
    let mut mismatches: Vec<String> = Vec::new();
    for (method, id, expected) in cases {
        if id == 0 {
            mismatches.push(format!("{method}: returned id 0 (created nothing)"));
            continue;
        }
        match mounted_control_name(id) {
            Some(actual) if actual == expected => {}
            Some(actual) => mismatches.push(format!(
                "{method}: mounted {actual:?} but the method is named for {expected:?}"
            )),
            None => mismatches.push(format!("{method}: id {id} addresses no mounted widget")),
        }
    }

    assert!(
        mismatches.is_empty(),
        "the custom backend built a control other than the one its method names — the id is \
         real, so nothing else reports this:\n  {}",
        mismatches.join("\n  ")
    );
}

/// The table above must cover **every** `create_*` the trait declares.
///
/// The previous version of this protection was a hand-picked sample, which is how
/// `create_web_view` stayed wrong while a test named `create_web_view_allocates_valid_id`
/// passed. A sample cannot be trusted to grow with the surface, so the count of the
/// table is pinned here and the *membership* is checked against the trait in
/// `tools/check_control_create_names.py`, which parses both sides.
#[test]
fn creation_table_covers_the_whole_create_surface() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 1000, 800);
    let mut names: Vec<&'static str> =
        creation_cases(&backend, parent).into_iter().map(|(method, _, _)| method).collect();
    names.sort_unstable();
    let total = names.len();
    names.dedup();
    assert_eq!(names.len(), total, "the table must not list a method twice");
    assert_eq!(
        total, 182,
        "the create_* surface changed: add the new methods to the table and to \
         tools/check_control_create_names.py's expectation"
    );
}
