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

        // ── Non-core widgets (desktop only) ─────────────────────────
        #[cfg(not(feature = "mini"))]
        {
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
        }
    }
}
