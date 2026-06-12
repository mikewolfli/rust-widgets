//! Core widget registration — registers all 64 widget kinds in the factory.
//!
//! This module defines `register_core_widgets` which is called from
//! [`WidgetFactory::new_with_defaults`] to populate the factory with every
//! built-in widget kind.

// Bring constructors, capability functions, and WidgetFactory into scope.
use super::*;

impl WidgetFactory {
    /// Registers all 64 core widget kinds with their constructors and capabilities.
    pub(crate) fn register_core_widgets(&mut self) {
        // ── Core widgets (always available) ─────────────────────────
        self.register(button_capability(), create_button);
        self.register(label_capability(), create_label);
        self.register(check_box_capability(), create_check_box);
        self.register(radio_button_capability(), create_radio_button);
        self.register(slider_capability(), create_slider);
        self.register(progress_bar_capability(), create_progress_bar);
        self.register(scroll_bar_capability(), create_scroll_bar);
        self.register(list_box_capability(), create_list_box);
        self.register(spin_box_capability(), create_spin_box);
        self.register(combo_box_capability(), create_combo_box);
        self.register(window_capability(), create_window);
        self.register(group_box_capability(), create_group_box);
        self.register(line_edit_capability(), create_line_edit);
        self.register(arc_capability(), create_arc);
        self.register(spinner_capability(), create_spinner);
        self.register(roller_capability(), create_roller);
        self.register(dropdown_capability(), create_dropdown);
        self.register(text_area_capability(), create_textarea);
        self.register(keyboard_capability(), create_keyboard);
        self.register(switch_capability(), create_switch);
        self.register(line_capability(), create_line);
        self.register(meter_capability(), create_meter);
        self.register(mini_chart_capability(), create_mini_chart);
        self.register(image_view_capability(), create_image_view);
        self.register(mini_canvas_capability(), create_mini_canvas);
        self.register(tile_view_capability(), create_tile_view);

        // ── Non-core widgets (desktop only) ─────────────────────────
        #[cfg(not(feature = "mini"))]
        {
            self.register(toggle_button_capability(), create_toggle_button);
            self.register(menu_capability(), create_menu);
            self.register(freeform_shape_capability(), create_freeform_shape);
            self.register(dial_capability(), create_dial);
            self.register(splitter_capability(), create_splitter);
            self.register(lcd_number_capability(), create_lcd_number);
            self.register(command_link_capability(), create_command_link);
            self.register(font_combo_box_capability(), create_font_combo_box);
            self.register(action_capability(), create_action);
            self.register(tool_box_capability(), create_tool_box);
            self.register(tab_bar_capability(), create_tab_bar);
            self.register(calendar_capability(), create_calendar);
            self.register(date_edit_capability(), create_date_edit);
            self.register(time_edit_capability(), create_time_edit);
            self.register(list_view_capability(), create_list_view);
            self.register(tree_view_capability(), create_tree_view);
            self.register(table_widget_capability(), create_table_widget);
            self.register(data_grid_capability(), create_data_grid);
            self.register(tree_table_capability(), create_tree_table);
            self.register(virtual_table_capability(), create_virtual_table);
            self.register(virtual_list_capability(), create_virtual_list);
            self.register(menu_bar_capability(), create_menu_bar);
            self.register(tool_bar_capability(), create_tool_bar);
            self.register(ribbon_bar_capability(), create_ribbon_bar);
            self.register(color_picker_capability(), create_color_picker);
            self.register(code_editor_capability(), create_code_editor);
            self.register(gantt_widget_capability(), create_gantt_widget);
            self.register(terminal_view_capability(), create_terminal_view);
            self.register(snackbar_capability(), create_snackbar);
            self.register(map_view_capability(), create_map_view);
            self.register(media_player_capability(), create_media_player);
            self.register(breadcrumb_capability(), create_breadcrumb);
            self.register(split_button_capability(), create_split_button);
            self.register(segmented_control_capability(), create_segmented_control);
            self.register(chip_capability(), create_chip);
            self.register(grid_capability(), create_grid);

            // ── Dialog widgets ────────────────────────────────────────
            self.register(message_box_capability(), create_message_box);
            self.register(file_dialog_capability(), create_file_dialog);
            self.register(font_dialog_capability(), create_font_dialog);
            self.register(input_dialog_capability(), create_input_dialog);
            self.register(progress_dialog_capability(), create_progress_dialog);
            self.register(popup_window_capability(), create_popup_window);

            // ── Container widgets ─────────────────────────────────────
            self.register(scroll_area_capability(), create_scroll_area);
            self.register(tab_widget_capability(), create_tab_widget);
            self.register(stacked_widget_capability(), create_stacked_widget);
            self.register(collapsible_pane_capability(), create_collapsible_pane);
            self.register(dock_widget_capability(), create_dock_widget);
            self.register(mdi_area_capability(), create_mdi_area);

            // ── Text widget ───────────────────────────────────────────
            self.register(text_edit_capability(), create_text_edit);

            // ── Web widget ────────────────────────────────────────────
            self.register(web_view_capability(), create_web_view);

            // ── Advanced widgets ──────────────────────────────────────
            self.register(pie_menu_capability(), create_pie_menu);
            self.register(date_time_edit_capability(), create_date_time_edit);

            // ── Group A widgets (non-mini) ──────────────────────────────
            self.register(canvas_capability(), create_canvas);
            self.register(chart_capability(), create_chart);
            self.register(search_box_capability(), create_search_box);
            self.register(badge_capability(), create_badge);
            self.register(skeleton_loader_capability(), create_skeleton_loader);
            self.register(fab_capability(), create_fab);
            self.register(bottom_sheet_capability(), create_bottom_sheet);
            self.register(bottom_navigation_bar_capability(), create_bottom_navigation_bar);
            self.register(navigation_drawer_capability(), create_navigation_drawer);
            self.register(app_bar_capability(), create_app_bar);
            self.register(mobile_date_picker_capability(), create_mobile_date_picker);
            self.register(divider_capability(), create_divider);
            self.register(stepper_capability(), create_stepper);
            self.register(rating_capability(), create_rating);
            self.register(avatar_capability(), create_avatar);
            self.register(empty_state_capability(), create_empty_state);
            self.register(color_history_capability(), create_color_history);
            self.register(color_well_capability(), create_color_well);
            self.register(tag_input_capability(), create_tag_input);
            self.register(ime_preedit_capability(), create_ime_preedit);
            self.register(inplace_editor_capability(), create_inplace_editor);
            self.register(qr_code_capability(), create_qr_code);
            self.register(masonry_layout_capability(), create_masonry_layout);
            self.register(material_snackbar_capability(), create_material_snackbar);
            self.register(adaptive_scaffold_capability(), create_adaptive_scaffold);
            self.register(wizard_dialog_capability(), create_wizard_dialog);
            self.register(safe_area_capability(), create_safe_area);
            self.register(cupertino_alert_dialog_capability(), create_cupertino_alert_dialog);
            self.register(cupertino_slider_capability(), create_cupertino_slider);
            self.register(tooltip_capability(), create_tooltip);
            self.register(segmented_button_capability(), create_segmented_button);
            self.register(navigation_stack_capability(), create_navigation_stack);
            self.register(progress_circle_capability(), create_progress_circle);
            self.register(icon_capability(), create_icon);
            self.register(dropdown_menu_capability(), create_dropdown_menu);
            self.register(masked_edit_capability(), create_masked_edit);
            self.register(menu_button_capability(), create_menu_button);
            self.register(popover_capability(), create_popover);
            self.register(auto_complete_edit_capability(), create_auto_complete_edit);
            self.register(multi_select_combo_box_capability(), create_multi_select_combo_box);
            self.register(range_slider_capability(), create_range_slider);
            self.register(floating_label_capability(), create_floating_label);
            self.register(font_preview_capability(), create_font_preview);
            self.register(cupertino_navigation_bar_capability(), create_cupertino_navigation_bar);
            self.register(
                cupertino_segmented_control_capability(),
                create_cupertino_segmented_control,
            );
            self.register(refresh_control_capability(), create_refresh_control);
            self.register(modal_bottom_sheet_capability(), create_modal_bottom_sheet);
            self.register(find_replace_dialog_capability(), create_find_replace_dialog);
            self.register(properties_panel_capability(), create_properties_panel);
            self.register(cupertino_date_picker_capability(), create_cupertino_date_picker);
            self.register(editable_combo_box_capability(), create_editable_combo_box);
            self.register(date_range_picker_capability(), create_date_range_picker);

            // ── New widgets ────────────────────────────────────────────
            self.register(rich_edit_capability(), create_rich_edit);
            self.register(carousel_capability(), create_carousel);
            self.register(material_navigation_rail_capability(), create_material_navigation_rail);
            self.register(tab_view_capability(), create_tab_view);
            self.register(search_bar_capability(), create_search_bar);
            self.register(shortcut_editor_capability(), create_shortcut_editor);
            self.register(swipe_to_dismiss_capability(), create_swipe_to_dismiss);
            self.register(pager_page_view_capability(), create_pager_page_view);
            self.register(line_chart_capability(), create_line_chart);
            self.register(sparkline_capability(), create_sparkline);
            self.register(bar_chart_capability(), create_bar_chart);
            self.register(pie_chart_capability(), create_pie_chart);
            self.register(animated_image_capability(), create_animated_image);
            self.register(hero_animation_capability(), create_hero_animation);
            self.register(bezier_curve_editor_capability(), create_bezier_curve_editor);
            self.register(lottie_widget_capability(), create_lottie_widget);
            self.register(rive_widget_capability(), create_rive_widget);
            self.register(video_player_capability(), create_video_player);
            self.register(image_gallery_capability(), create_image_gallery);
            self.register(audio_visualizer_capability(), create_audio_visualizer);
            self.register(camera_preview_capability(), create_camera_preview);
            self.register(barcode_scanner_capability(), create_barcode_scanner);
            self.register(tool_button_capability(), create_tool_button);
            self.register(status_bar_capability(), create_status_bar);
            self.register(property_grid_capability(), create_property_grid);
        }
    }
}
