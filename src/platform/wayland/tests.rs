//! Integration tests for the Wayland backend.
//!
//! These tests verify platform creation, basic widget lifecycle,
//! clipboard roundtrip, list data methods, dialog creation, and
//! extended control creation for the Wayland platform backend.

use crate::platform::wayland::WaylandPlatform;
use crate::platform::Platform;

#[test]
fn platform_creates_and_runs() {
    let backend = WaylandPlatform::new();
    backend.init();
    assert_eq!(backend.backend_name(), "wayland");

    // Create a window and basic widgets.
    let window = backend.create_window("TestWindow", 50, 50, 400, 300);
    assert!(window > 0, "Window should be created");

    let button = backend.create_button(window, "Click", 10, 10, 80, 24);
    assert!(button > 0, "Button should be created");

    let label = backend.create_label(window, "Hello", 10, 40, 80, 24);
    assert!(label > 0, "Label should be created");

    let checkbox = backend.create_checkbox(window, "Check", 10, 70, 80, 24);
    assert!(checkbox > 0, "Checkbox should be created");

    let line_edit = backend.create_line_edit(window, "edit", 10, 100, 160, 24);
    assert!(line_edit > 0, "LineEdit should be created");

    let radio = backend.create_radio_button(window, "Radio", 10, 130, 80, 24);
    assert!(radio > 0, "RadioButton should be created");

    let slider = backend.create_slider(window, 10, 160, 200, 24);
    assert!(slider > 0, "Slider should be created");

    let progress = backend.create_progress_bar(window, 10, 190, 200, 24);
    assert!(progress > 0, "ProgressBar should be created");

    let combo = backend.create_combo_box(window, 10, 220, 140, 24);
    assert!(combo > 0, "ComboBox should be created");

    let list_box = backend.create_list_box(window, 10, 250, 140, 80);
    assert!(list_box > 0, "ListBox should be created");
}

#[test]
fn widget_lifecycle() {
    let backend = WaylandPlatform::new();
    backend.init();

    let window = backend.create_window("Lifecycle", 0, 0, 200, 120);
    assert!(window > 0, "Window should be created");

    let button = backend.create_button(window, "btn", 10, 10, 80, 24);
    assert!(button > 0, "Button should be created");

    // Text roundtrip.
    backend.set_widget_text(button, "updated");
    assert_eq!(backend.get_widget_text(button), "updated");

    // Show/hide roundtrip.
    backend.show_widget(button);
    assert!(
        backend.is_widget_visible(button),
        "Button should be visible after show"
    );

    backend.hide_widget(button);
    assert!(
        !backend.is_widget_visible(button),
        "Button should be hidden after hide"
    );

    // Enable/disable roundtrip.
    backend.set_widget_enabled(button, false);
    assert!(
        !backend.is_widget_enabled(button),
        "Button should be disabled"
    );

    backend.set_widget_enabled(button, true);
    assert!(
        backend.is_widget_enabled(button),
        "Button should be enabled"
    );

    // Geometry update.
    backend.set_widget_geometry(button, 20, 20, 120, 32);

    // Window-level lifecycle.
    backend.set_widget_text(window, "UpdatedTitle");
    assert_eq!(backend.get_widget_text(window), "UpdatedTitle");

    backend.show_widget(window);
    assert!(
        backend.is_widget_visible(window),
        "Window should be visible"
    );

    backend.hide_widget(window);
    assert!(
        !backend.is_widget_visible(window),
        "Window should be hidden"
    );
}

#[test]
fn clipboard_roundtrip() {
    let backend = WaylandPlatform::new();
    backend.init();

    // Create a window so the backend is fully initialised.
    let _window = backend.create_window("ClipTest", 0, 0, 200, 120);

    // Clipboard set/get roundtrip.
    assert!(
        backend.set_clipboard_text("wayland_clip_test"),
        "Should set clipboard text"
    );
    assert_eq!(
        backend.get_clipboard_text(),
        "wayland_clip_test",
        "Clipboard text should match"
    );

    // Overwrite with new content.
    assert!(
        backend.set_clipboard_text("updated_clip"),
        "Should overwrite clipboard"
    );
    assert_eq!(
        backend.get_clipboard_text(),
        "updated_clip",
        "Updated clipboard text should match"
    );

    // Empty string.
    assert!(backend.set_clipboard_text(""), "Should set empty clipboard");
    assert_eq!(
        backend.get_clipboard_text(),
        "",
        "Empty clipboard should match"
    );
}

#[test]
fn combo_box_data_methods() {
    let backend = WaylandPlatform::new();
    backend.init();

    let window = backend.create_window("ComboTest", 0, 0, 200, 120);
    let combo = backend.create_combo_box(window, 10, 10, 140, 24);
    assert!(combo > 0, "ComboBox should be created");

    // Add items.
    assert!(
        backend.combo_box_add_item(combo, "Item A"),
        "Should add Item A"
    );
    assert!(
        backend.combo_box_add_item(combo, "Item B"),
        "Should add Item B"
    );
    assert!(
        backend.combo_box_add_item(combo, "Item C"),
        "Should add Item C"
    );

    assert_eq!(backend.combo_box_item_count(combo), 3);

    // Item text retrieval.
    assert_eq!(
        backend.combo_box_item_text(combo, 0),
        Some("Item A".to_string())
    );
    assert_eq!(
        backend.combo_box_item_text(combo, 1),
        Some("Item B".to_string())
    );
    assert_eq!(
        backend.combo_box_item_text(combo, 2),
        Some("Item C".to_string())
    );
    assert_eq!(backend.combo_box_item_text(combo, 5), None);

    // Set and get current index.
    assert!(
        backend.combo_box_set_current_index(combo, 1),
        "Should set index 1"
    );
    assert_eq!(backend.combo_box_current_index(combo), Some(1));

    // Clear items.
    assert!(backend.combo_box_clear_items(combo), "Should clear items");
    assert_eq!(backend.combo_box_item_count(combo), 0);
    assert_eq!(backend.combo_box_current_index(combo), None);
}

#[test]
fn list_box_data_methods() {
    let backend = WaylandPlatform::new();
    backend.init();

    let window = backend.create_window("ListBoxTest", 0, 0, 200, 200);
    let list_box = backend.create_list_box(window, 10, 10, 140, 80);
    assert!(list_box > 0, "ListBox should be created");

    // Add items.
    assert!(
        backend.list_box_add_item(list_box, "Item 1"),
        "Should add Item 1"
    );
    assert!(
        backend.list_box_add_item(list_box, "Item 2"),
        "Should add Item 2"
    );
    assert!(
        backend.list_box_add_item(list_box, "Item 3"),
        "Should add Item 3"
    );

    assert_eq!(backend.list_box_item_count(list_box), 3);

    // Item text retrieval.
    assert_eq!(
        backend.list_box_item_text(list_box, 0),
        Some("Item 1".to_string())
    );
    assert_eq!(
        backend.list_box_item_text(list_box, 1),
        Some("Item 2".to_string())
    );
    assert_eq!(
        backend.list_box_item_text(list_box, 2),
        Some("Item 3".to_string())
    );

    // Set and get current index.
    assert!(
        backend.list_box_set_current_index(list_box, 1),
        "Should set index 1"
    );
    assert_eq!(backend.list_box_current_index(list_box), Some(1));

    // Remove item at index 0 — remaining: [Item 2, Item 3], current should shift.
    assert!(
        backend.list_box_remove_item(list_box, 0),
        "Should remove Item 1"
    );
    assert_eq!(backend.list_box_item_count(list_box), 2);
    assert_eq!(
        backend.list_box_item_text(list_box, 0),
        Some("Item 2".to_string())
    );
    // Current index adjusts from 1 to 0 after removal.
    assert_eq!(
        backend.list_box_current_index(list_box),
        Some(0),
        "Current index should adjust after removal"
    );

    // Clear items.
    assert!(backend.list_box_clear_items(list_box), "Should clear items");
    assert_eq!(backend.list_box_item_count(list_box), 0);
    assert_eq!(backend.list_box_current_index(list_box), None);
}

#[test]
fn menu_system() {
    let backend = WaylandPlatform::new();
    backend.init();

    let window = backend.create_window("MenuTest", 0, 0, 400, 300);
    let menu_bar = backend.create_menu_bar(window, 0, 0, 400, 24);
    assert!(menu_bar > 0, "MenuBar should be created");

    let file_menu = backend.create_menu(window, "File", 0, 24, 100, 24);
    assert!(file_menu > 0, "File menu should be created");

    let new_item = backend.menu_add_item(file_menu, "New", Some("Ctrl+N"));
    assert!(new_item > 0, "New menu item should be created");

    let open_item = backend.menu_add_item(file_menu, "Open", Some("Ctrl+O"));
    assert!(open_item > 0, "Open menu item should be created");

    let quit_item = backend.menu_add_item(file_menu, "Quit", Some("Ctrl+Q"));
    assert!(quit_item > 0, "Quit menu item should be created");

    assert!(
        backend.attach_menu_bar_to_window(window, menu_bar),
        "Should attach menu bar to window"
    );

    // Inject and poll menu trigger.
    assert!(
        backend.inject_menu_trigger(open_item),
        "Should inject menu trigger"
    );
    assert_eq!(
        backend.poll_menu_triggered(),
        Some(open_item),
        "Should poll open item"
    );

    // Second trigger.
    assert!(
        backend.inject_menu_trigger(quit_item),
        "Should inject quit trigger"
    );
    assert_eq!(
        backend.poll_menu_triggered(),
        Some(quit_item),
        "Should poll quit item"
    );
}

#[test]
fn widget_trigger_events() {
    use crate::platform::WidgetTriggerKind;

    let backend = WaylandPlatform::new();
    backend.init();

    let window = backend.create_window("TriggerTest", 0, 0, 200, 120);
    let button = backend.create_button(window, "Click", 10, 10, 80, 24);

    assert!(
        backend.inject_widget_trigger_event(button, WidgetTriggerKind::Clicked),
        "Should inject click event"
    );

    let event = backend.poll_widget_trigger_event();
    assert!(event.is_some(), "Should poll trigger event");
    assert_eq!(event.unwrap().widget_id, button);
    assert_eq!(
        event.unwrap().kind,
        WidgetTriggerKind::Clicked,
        "Should match Clicked kind"
    );
}

#[test]
fn dialog_and_extended_controls() {
    let backend = WaylandPlatform::new();
    backend.init();

    let window = backend.create_window("DialogTest", 0, 0, 500, 400);

    // Dialogs.
    let msg_box = backend.create_message_box(window, "Title", "Hello", 10, 10, 300, 150);
    assert!(msg_box > 0, "MessageBox should be created");

    let file_dlg = backend.create_file_dialog(window, 10, 170, 400, 300);
    assert!(file_dlg > 0, "FileDialog should be created");

    let color_dlg = backend.create_color_dialog(window, 10, 10, 300, 300);
    assert!(color_dlg > 0, "ColorDialog should be created");

    let font_dlg = backend.create_font_dialog(window, 10, 10, 300, 300);
    assert!(font_dlg > 0, "FontDialog should be created");

    // Extended controls.
    let spin = backend.create_spin_box(window, 10, 10, 80, 24);
    assert!(spin > 0, "SpinBox should be created");

    let list_view = backend.create_list_view(window, 10, 40, 200, 150);
    assert!(list_view > 0, "ListView should be created");

    let scroll = backend.create_scroll_area(window, 10, 200, 200, 150);
    assert!(scroll > 0, "ScrollArea should be created");
}

#[test]
fn drag_and_drop() {
    let backend = WaylandPlatform::new();
    backend.init();

    let window = backend.create_window("DragTest", 0, 0, 400, 300);
    let source = backend.create_button(window, "Drag", 10, 10, 80, 24);

    // Begin drag from source.
    assert!(
        backend.begin_drag(source, "text/plain", b"drag payload"),
        "Should begin drag"
    );

    // Poll drop event.
    let drop = backend.poll_drop_event();
    assert!(drop.is_some(), "Should poll drop event");
    let drop = drop.unwrap();
    assert_eq!(drop.source_widget_id, source);
    assert_eq!(drop.mime, "text/plain");
    assert_eq!(drop.payload, b"drag payload");
}

#[test]
fn ime_and_accessibility() {
    let backend = WaylandPlatform::new();
    backend.init();

    let window = backend.create_window("IMETest", 0, 0, 400, 300);
    let line_edit = backend.create_line_edit(window, "input", 10, 10, 160, 24);

    // IME roundtrip.
    assert!(
        backend.set_widget_ime_enabled(line_edit, true),
        "Should enable IME"
    );
    assert!(
        backend.is_widget_ime_enabled(line_edit),
        "IME should be enabled"
    );

    assert!(
        backend.set_widget_ime_enabled(line_edit, false),
        "Should disable IME"
    );
    assert!(
        !backend.is_widget_ime_enabled(line_edit),
        "IME should be disabled"
    );

    // Accessibility name roundtrip.
    assert!(
        backend.set_widget_accessibility_name(line_edit, "InputField"),
        "Should set accessibility name"
    );
    assert_eq!(
        backend.get_widget_accessibility_name(line_edit),
        "InputField",
        "Accessibility name should match"
    );
}
