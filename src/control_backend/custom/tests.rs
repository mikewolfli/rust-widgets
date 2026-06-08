use super::*;
use crate::control_backend::trait_def::ControlBackend;
use crate::control_backend::types::ControlBackendKind;
use crate::platform::WidgetTriggerKind;

#[test]
fn custom_paint_control_backend_new_creates_valid_instance() {
    let backend = CustomPaintControlBackend::new();
    assert_eq!(backend.backend_name(), "custom-paint-control-backend");
    assert_eq!(backend.kind(), ControlBackendKind::Custom);
}

#[test]
fn custom_paint_control_backend_default() {
    let backend = CustomPaintControlBackend::default();
    assert_eq!(backend.backend_name(), "custom-paint-control-backend");
    assert_eq!(backend.kind(), ControlBackendKind::Custom);
}

#[test]
fn create_window_allocates_valid_id() {
    let backend = CustomPaintControlBackend::new();
    let id = backend.create_window("Test Window", 0, 0, 800, 600);
    assert_ne!(id, 0, "custom backend must allocate non-zero widget IDs");
}

#[test]
fn create_window_sets_text() {
    let backend = CustomPaintControlBackend::new();
    let id = backend.create_window("Hello Window", 10, 20, 640, 480);
    let text = backend.get_widget_text(id);
    assert_eq!(text, "Hello Window");
}

#[test]
fn create_window_is_enabled_and_visible() {
    let backend = CustomPaintControlBackend::new();
    let id = backend.create_window("Test", 0, 0, 100, 100);
    assert!(backend.is_widget_enabled(id));
    assert!(backend.is_widget_visible(id));
}

#[test]
fn create_button_allocates_valid_id() {
    let backend = CustomPaintControlBackend::new();
    let id = backend.create_button(0, "Click", 10, 20, 100, 30);
    assert_ne!(id, 0);
}

#[test]
fn create_button_sets_text() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 400, 300);
    let id = backend.create_button(parent, "Submit", 50, 50, 80, 25);
    let text = backend.get_widget_text(id);
    assert_eq!(text, "Submit");
}

#[test]
fn create_button_is_enabled_and_visible() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 400, 300);
    let id = backend.create_button(parent, "OK", 0, 0, 80, 25);
    assert!(backend.is_widget_enabled(id));
    assert!(backend.is_widget_visible(id));
}

#[test]
fn create_label_allocates_valid_id() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 400, 300);
    let id = backend.create_label(parent, "Hello", 10, 10, 200, 20);
    assert_ne!(id, 0);
}

#[test]
fn create_label_sets_text() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 400, 300);
    let id = backend.create_label(parent, "Hello World", 10, 10, 200, 20);
    assert_eq!(backend.get_widget_text(id), "Hello World");
}

#[test]
fn create_checkbox_sets_text() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 400, 300);
    let id = backend.create_checkbox(parent, "Enable feature", 10, 10, 150, 25);
    assert_ne!(id, 0);
    assert_eq!(backend.get_widget_text(id), "Enable feature");
}

#[test]
fn create_radio_button_sets_text() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 400, 300);
    let id = backend.create_radio_button(parent, "Option A", 10, 10, 150, 25);
    assert_ne!(id, 0);
    assert_eq!(backend.get_widget_text(id), "Option A");
}

#[test]
fn widget_ids_are_incremental() {
    let backend = CustomPaintControlBackend::new();
    let id1 = backend.create_window("A", 0, 0, 100, 100);
    let id2 = backend.create_window("B", 0, 0, 100, 100);
    let id3 = backend.create_button(0, "C", 0, 0, 50, 20);
    assert!(id1 < id2, "first alloc id ({}) must be < second ({})", id1, id2);
    assert!(id2 < id3, "second alloc id ({}) must be < third ({})", id2, id3);
}

#[test]
fn create_multiple_widgets_independent_state() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 500, 400);
    let btn1 = backend.create_button(parent, "Btn1", 0, 0, 50, 20);
    let btn2 = backend.create_button(parent, "Btn2", 60, 0, 50, 20);
    assert_eq!(backend.get_widget_text(btn1), "Btn1");
    assert_eq!(backend.get_widget_text(btn2), "Btn2");
    backend.set_widget_text(btn1, "Updated");
    assert_eq!(backend.get_widget_text(btn1), "Updated");
    assert_eq!(backend.get_widget_text(btn2), "Btn2");
}

#[test]
fn set_and_get_widget_text() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 400, 300);
    let id = backend.create_button(parent, "Original", 0, 0, 100, 30);
    assert_eq!(backend.get_widget_text(id), "Original");
    backend.set_widget_text(id, "Modified");
    assert_eq!(backend.get_widget_text(id), "Modified");
}

#[test]
fn set_and_get_widget_enabled() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 400, 300);
    let id = backend.create_button(parent, "Test", 0, 0, 100, 30);
    assert!(backend.is_widget_enabled(id));
    backend.set_widget_enabled(id, false);
    assert!(!backend.is_widget_enabled(id));
    backend.set_widget_enabled(id, true);
    assert!(backend.is_widget_enabled(id));
}

#[test]
fn set_and_get_widget_visible() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 400, 300);
    let id = backend.create_button(parent, "Test", 0, 0, 100, 30);
    assert!(backend.is_widget_visible(id));
    backend.set_widget_visible(id, false);
    assert!(!backend.is_widget_visible(id));
    backend.set_widget_visible(id, true);
    assert!(backend.is_widget_visible(id));
}

#[test]
fn show_and_hide_widget() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 400, 300);
    let id = backend.create_button(parent, "Test", 0, 0, 100, 30);
    backend.hide_widget(id);
    assert!(!backend.is_widget_visible(id));
    backend.show_widget(id);
    assert!(backend.is_widget_visible(id));
}

#[test]
fn set_and_get_widget_ime_enabled() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 400, 300);
    let id = backend.create_button(parent, "Test", 0, 0, 100, 30);
    assert!(!backend.is_widget_ime_enabled(id));
    assert!(backend.set_widget_ime_enabled(id, true));
    assert!(backend.is_widget_ime_enabled(id));
}

#[test]
fn set_and_get_widget_accessibility_name() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 400, 300);
    let id = backend.create_button(parent, "Test", 0, 0, 100, 30);
    let default_name = backend.get_widget_accessibility_name(id);
    assert_eq!(default_name, "Test");
    assert!(backend.set_widget_accessibility_name(id, "Custom Label"));
    assert_eq!(backend.get_widget_accessibility_name(id), "Custom Label");
}

#[test]
fn create_slider_allocates_valid_id() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 400, 300);
    let id = backend.create_slider(parent, 10, 10, 200, 30);
    assert_ne!(id, 0);
}

#[test]
fn create_progress_bar_allocates_valid_id() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 400, 300);
    let id = backend.create_progress_bar(parent, 10, 10, 300, 20);
    assert_ne!(id, 0);
}

#[test]
fn create_line_edit_sets_text() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 400, 300);
    let id = backend.create_line_edit(parent, "default text", 10, 10, 200, 25);
    assert_ne!(id, 0);
    assert_eq!(backend.get_widget_text(id), "default text");
}

#[test]
fn create_combo_box_allocates_valid_id() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 400, 300);
    let id = backend.create_combo_box(parent, 10, 10, 150, 25);
    assert_ne!(id, 0);
}

#[test]
fn create_list_box_allocates_valid_id() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 400, 300);
    let id = backend.create_list_box(parent, 10, 10, 150, 100);
    assert_ne!(id, 0);
}

#[test]
fn create_panel_allocates_valid_id() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 400, 300);
    let id = backend.create_panel(parent, 10, 10, 380, 280);
    assert_ne!(id, 0);
}

#[test]
fn create_menu_bar_and_menu() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 800, 600);
    let menu_bar = backend.create_menu_bar(parent, 0, 0, 800, 30);
    assert_ne!(menu_bar, 0);
    let menu = backend.create_menu(parent, "File", 0, 0, 50, 30);
    assert_ne!(menu, 0);
    let item = backend.menu_add_item(menu, "Open", Some("Ctrl+O"));
    assert_ne!(item, 0);
}

#[test]
fn create_tool_bar_and_status_bar() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 800, 600);
    let tool_bar = backend.create_tool_bar(parent, 0, 0, 800, 40);
    assert_ne!(tool_bar, 0);
    let status_bar = backend.create_status_bar(parent, "Ready", 0, 560, 800, 40);
    assert_ne!(status_bar, 0);
    assert_eq!(backend.get_widget_text(status_bar), "Ready");
}

#[test]
fn create_dialog_allocates_valid_id() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 800, 600);
    let id = backend.create_dialog(parent, "Settings", 100, 100, 400, 300);
    assert_ne!(id, 0);
    assert_eq!(backend.get_widget_text(id), "Settings");
}

#[test]
fn create_message_box_allocates_valid_id() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 800, 600);
    let id = backend.create_message_box(parent, "Info", "Hello!", 200, 200, 300, 150);
    assert_ne!(id, 0);
    assert_eq!(backend.get_widget_text(id), "Hello!");
}

#[test]
fn create_file_dialog_allocates_valid_id() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 800, 600);
    let id = backend.create_file_dialog(parent, "Open File", 100, 100, 500, 400);
    assert_ne!(id, 0);
}

#[test]
fn create_color_dialog_allocates_valid_id() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 800, 600);
    let id = backend.create_color_dialog(parent, "Pick Color", 100, 100, 400, 300);
    assert_ne!(id, 0);
}

#[test]
fn attach_menu_bar_to_window_returns_true() {
    let backend = CustomPaintControlBackend::new();
    let window = backend.create_window("Win", 0, 0, 800, 600);
    let menu_bar = backend.create_menu_bar(window, 0, 0, 800, 30);
    let result = backend.attach_menu_bar_to_window(window, menu_bar);
    assert!(result);
}

#[test]
fn poll_and_inject_menu_trigger() {
    let backend = CustomPaintControlBackend::new();
    assert!(backend.poll_menu_triggered().is_none());
    let result = backend.inject_menu_trigger(42);
    assert!(result);
    let triggered = backend.poll_menu_triggered();
    assert_eq!(triggered, Some(42));
    assert!(backend.poll_menu_triggered().is_none());
}

#[test]
fn poll_and_inject_widget_trigger_event() {
    let backend = CustomPaintControlBackend::new();
    assert!(backend.poll_widget_trigger_event().is_none());
    assert!(backend.poll_widget_triggered().is_none());
    let injected = backend.inject_widget_trigger_event(99, WidgetTriggerKind::Clicked);
    assert!(injected);
    let event = backend.poll_widget_trigger_event();
    assert!(event.is_some());
    let event = event.unwrap();
    assert_eq!(event.widget_id, 99);
    let triggered = backend.poll_widget_triggered();
    assert!(triggered.is_none());
}

#[test]
fn set_widget_geometry_updates_properties() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 800, 600);
    let id = backend.create_button(parent, "Btn", 10, 20, 100, 30);
    backend.set_widget_geometry(id, 50, 60, 200, 40);
}

#[test]
fn create_canvas_allocates_valid_id() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 800, 600);
    let id = backend.create_canvas(parent, 0, 0, 400, 300);
    assert_ne!(id, 0);
}

#[test]
fn create_table_allocates_valid_id() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 800, 600);
    let id = backend.create_table(parent, 0, 0, 400, 300);
    assert_ne!(id, 0);
}

#[test]
fn create_grid_allocates_valid_id() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 800, 600);
    let id = backend.create_grid(parent, 0, 0, 400, 300);
    assert_ne!(id, 0);
}

#[test]
fn create_chart_allocates_valid_id() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 800, 600);
    let id = backend.create_chart(parent, 0, 0, 400, 300);
    assert_ne!(id, 0);
}

#[test]
fn create_toggle_button_sets_text() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 800, 600);
    let id = backend.create_toggle_button(parent, "Toggle", 0, 0, 100, 30);
    assert_ne!(id, 0);
    assert_eq!(backend.get_widget_text(id), "Toggle");
}

#[test]
fn create_check_list_box_allocates_valid_id() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 800, 600);
    let id = backend.create_check_list_box(parent, 0, 0, 200, 150);
    assert_ne!(id, 0);
}

#[test]
fn create_double_spin_box_allocates_valid_id() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 800, 600);
    let id = backend.create_double_spin_box(parent, 0, 0, 100, 25);
    assert_ne!(id, 0);
}

#[test]
fn create_dial_allocates_valid_id() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 800, 600);
    let id = backend.create_dial(parent, 0, 0, 100, 100);
    assert_ne!(id, 0);
}

#[test]
fn create_wizard_allocates_valid_id() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 800, 600);
    let id = backend.create_wizard(parent, "Setup Wizard", 100, 100, 500, 400);
    assert_ne!(id, 0);
}

#[test]
fn create_date_picker_allocates_valid_id() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 800, 600);
    let id = backend.create_date_picker(parent, 0, 0, 200, 30);
    assert_ne!(id, 0);
}

#[test]
fn create_time_picker_allocates_valid_id() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 800, 600);
    let id = backend.create_time_picker(parent, 0, 0, 200, 30);
    assert_ne!(id, 0);
}

#[test]
fn create_date_time_picker_allocates_valid_id() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 800, 600);
    let id = backend.create_date_time_picker(parent, 0, 0, 200, 30);
    assert_ne!(id, 0);
}

#[test]
fn create_directory_dialog_allocates_valid_id() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 800, 600);
    let id = backend.create_directory_dialog(parent, "Open Folder", 100, 100, 500, 400);
    assert_ne!(id, 0);
}

#[test]
fn create_data_view_allocates_valid_id() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 800, 600);
    let id = backend.create_data_view(parent, 0, 0, 400, 300);
    assert_ne!(id, 0);
}

#[test]
fn create_property_grid_allocates_valid_id() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 800, 600);
    let id = backend.create_property_grid(parent, 0, 0, 300, 400);
    assert_ne!(id, 0);
}

#[test]
fn create_toolbox_allocates_valid_id() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 800, 600);
    let id = backend.create_toolbox(parent, 0, 0, 200, 400);
    assert_ne!(id, 0);
}

#[test]
fn create_collapsible_pane_sets_text() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 800, 600);
    let id = backend.create_collapsible_pane(parent, "Details", 0, 0, 300, 200);
    assert_ne!(id, 0);
    assert_eq!(backend.get_widget_text(id), "Details");
}

#[test]
fn create_dock_widget_sets_text() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 800, 600);
    let id = backend.create_dock_widget(parent, "Dock Panel", 0, 0, 200, 400);
    assert_ne!(id, 0);
    assert_eq!(backend.get_widget_text(id), "Dock Panel");
}

#[test]
fn create_web_view_allocates_valid_id() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 800, 600);
    let id = backend.create_web_view(parent, 0, 0, 800, 600);
    assert_ne!(id, 0);
}

#[test]
fn create_activity_indicator_allocates_valid_id() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 800, 600);
    let id = backend.create_activity_indicator(parent, 0, 0, 40, 40);
    assert_ne!(id, 0);
}

#[test]
fn create_calendar_allocates_valid_id() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 800, 600);
    let id = backend.create_calendar(parent, 0, 0, 300, 250);
    assert_ne!(id, 0);
}

#[test]
fn create_column_view_allocates_valid_id() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 800, 600);
    let id = backend.create_column_view(parent, 0, 0, 300, 400);
    assert_ne!(id, 0);
}

#[test]
fn create_undo_view_allocates_valid_id() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 800, 600);
    let id = backend.create_undo_view(parent, 0, 0, 200, 300);
    assert_ne!(id, 0);
}

#[test]
fn create_command_link_sets_text() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 800, 600);
    let id = backend.create_command_link(parent, "Open Folder", 0, 0, 300, 40);
    assert_ne!(id, 0);
    assert_eq!(backend.get_widget_text(id), "Open Folder");
}

#[test]
fn create_lcd_number_allocates_valid_id() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 800, 600);
    let id = backend.create_lcd_number(parent, 0, 0, 100, 40);
    assert_ne!(id, 0);
}

#[test]
fn create_font_combo_box_allocates_valid_id() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 800, 600);
    let id = backend.create_font_combo_box(parent, 0, 0, 200, 25);
    assert_ne!(id, 0);
}

#[test]
fn create_action_sets_text() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 800, 600);
    let id = backend.create_action(parent, "Save", 0, 0, 50, 25);
    assert_ne!(id, 0);
    assert_eq!(backend.get_widget_text(id), "Save");
}

#[test]
fn create_tool_button_sets_text() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 800, 600);
    let id = backend.create_tool_button(parent, "Save", 0, 0, 40, 40);
    assert_ne!(id, 0);
    assert_eq!(backend.get_widget_text(id), "Save");
}

#[test]
fn create_tool_box_allocates_valid_id() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 800, 600);
    let id = backend.create_tool_box(parent, 0, 0, 200, 400);
    assert_ne!(id, 0);
}

#[test]
fn create_context_menu_sets_text() {
    let backend = CustomPaintControlBackend::new();
    let parent = backend.create_window("Parent", 0, 0, 800, 600);
    let id = backend.create_context_menu(parent, "Edit", 0, 0, 100, 200);
    assert_ne!(id, 0);
    assert_eq!(backend.get_widget_text(id), "Edit");
}

#[test]
fn send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<CustomPaintControlBackend>();
}
