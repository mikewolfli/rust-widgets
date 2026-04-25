//! Integration tests for the Harmony desktop backend.
//!
//! These tests verify parity between the Harmony backend and the
//! existing platform backends.

use crate::platform::harmony::HarmonyPlatform;
use crate::platform::Platform;
use crate::WidgetTriggerKind;

#[test]
fn platform_creates_and_runs() {
    let backend = HarmonyPlatform::new();
    backend.init();
    assert_eq!(backend.backend_name(), "harmony-desktop");

    let window = backend.create_window("TestWindow", 100, 200, 640, 480);
    assert!(window > 0, "Window should be created");

    let button = backend.create_button(window, "btn", 10, 10, 80, 24);
    assert!(button > 0, "Button should be created");
    assert_eq!(backend.get_widget_text(button), "btn");

    let label = backend.create_label(window, "lbl", 10, 40, 80, 24);
    assert!(label > 0, "Label should be created");
    assert_eq!(backend.get_widget_text(label), "lbl");

    let checkbox = backend.create_checkbox(window, "chk", 10, 70, 80, 24);
    assert!(checkbox > 0, "CheckBox should be created");
    assert_eq!(backend.get_widget_text(checkbox), "chk");

    let radio = backend.create_radio_button(window, "radio", 10, 100, 80, 24);
    assert!(radio > 0, "RadioButton should be created");
    assert_eq!(backend.get_widget_text(radio), "radio");

    let edit = backend.create_line_edit(window, "edit", 10, 130, 100, 24);
    assert!(edit > 0, "LineEdit should be created");
    assert_eq!(backend.get_widget_text(edit), "edit");

    let slider = backend.create_slider(window, 10, 160, 100, 24);
    assert!(slider > 0, "Slider should be created");

    let progress = backend.create_progress_bar(window, 10, 190, 100, 24);
    assert!(progress > 0, "ProgressBar should be created");

    let combo = backend.create_combo_box(window, 10, 220, 100, 100);
    assert!(combo > 0, "ComboBox should be created");
    assert_eq!(backend.combo_box_item_count(combo), 0);

    let list_box = backend.create_list_box(window, 10, 330, 100, 100);
    assert!(list_box > 0, "ListBox should be created");

    let panel = backend.create_panel(window, 10, 440, 200, 100);
    assert!(panel > 0, "Panel should be created");
}

#[test]
fn widget_lifecycle() {
    let backend = HarmonyPlatform::new();
    backend.init();
    let window = backend.create_window("w", 0, 0, 200, 120);
    assert!(window > 0, "Window should be created");

    let button = backend.create_button(window, "btn", 10, 10, 80, 24);
    assert!(button > 0, "Button should be created");

    // Test show/hide.
    backend.show_widget(button);
    assert!(
        backend.is_widget_visible(button),
        "Widget should be visible after show"
    );

    backend.hide_widget(button);
    assert!(
        !backend.is_widget_visible(button),
        "Widget should be hidden after hide"
    );

    backend.show_widget(button);
    assert!(
        backend.is_widget_visible(button),
        "Widget should be visible after second show"
    );

    // Test enable/disable.
    backend.set_widget_enabled(button, false);
    assert!(
        !backend.is_widget_enabled(button),
        "Widget should be disabled"
    );

    backend.set_widget_enabled(button, true);
    assert!(
        backend.is_widget_enabled(button),
        "Widget should be enabled"
    );

    // Test text set/get roundtrip.
    backend.set_widget_text(button, "updated");
    assert_eq!(
        backend.get_widget_text(button),
        "updated",
        "Widget text should update"
    );

    // Test geometry update.
    backend.set_widget_geometry(button, 20, 20, 100, 30);

    // Test visibility roundtrip on window.
    backend.hide_widget(window);
    assert!(
        !backend.is_widget_visible(window),
        "Window should be hidden"
    );
    backend.show_widget(window);
    assert!(
        backend.is_widget_visible(window),
        "Window should be visible"
    );

    // Test IME (default: false).
    // IME defaults to true (BackendState::WidgetRecord default).
    assert!(
        backend.is_widget_ime_enabled(button),
        "IME should be enabled by default"
    );
    assert!(
        backend.set_widget_ime_enabled(button, false),
        "set_widget_ime_enabled should succeed"
    );
    assert!(
        !backend.is_widget_ime_enabled(button),
        "IME should be disabled after set_widget_ime_enabled(false)"
    );

    // Test accessibility name (default: copies widget text).
    assert!(
        backend.set_widget_accessibility_name(button, "acc"),
        "set_widget_accessibility_name should succeed for a valid widget"
    );
    assert_eq!(
        backend.get_widget_accessibility_name(button),
        "acc",
        "Accessibility name should match the value just set"
    );
}

#[test]
fn clipboard_roundtrip() {
    let backend = HarmonyPlatform::new();
    backend.init();

    // Clipboard should start empty.
    assert_eq!(
        backend.get_clipboard_text(),
        "",
        "Clipboard should be empty initially"
    );

    // Set and get roundtrip.
    assert!(
        backend.set_clipboard_text("hello harmony"),
        "Should set clipboard text"
    );
    assert_eq!(
        backend.get_clipboard_text(),
        "hello harmony",
        "Clipboard text should match"
    );

    // Overwrite with new value.
    assert!(
        backend.set_clipboard_text("updated clipboard"),
        "Should overwrite clipboard text"
    );
    assert_eq!(
        backend.get_clipboard_text(),
        "updated clipboard",
        "Clipboard text should reflect update"
    );

    // Set empty string.
    assert!(
        backend.set_clipboard_text(""),
        "Should set empty clipboard text"
    );
    assert_eq!(
        backend.get_clipboard_text(),
        "",
        "Clipboard should be empty after setting empty string"
    );
}

#[test]
fn combo_box_and_list_box_data() {
    let backend = HarmonyPlatform::new();
    backend.init();
    let window = backend.create_window("w", 0, 0, 200, 120);

    // ComboBox item management.
    let combo = backend.create_combo_box(window, 10, 10, 100, 100);
    assert!(combo > 0, "ComboBox should be created");
    assert_eq!(backend.combo_box_item_count(combo), 0);

    assert!(backend.combo_box_add_item(combo, "Item 1"));
    assert!(backend.combo_box_add_item(combo, "Item 2"));
    assert!(backend.combo_box_add_item(combo, "Item 3"));
    assert_eq!(backend.combo_box_item_count(combo), 3);

    assert_eq!(backend.combo_box_item_text(combo, 0), Some("Item 1".into()));
    assert_eq!(backend.combo_box_item_text(combo, 1), Some("Item 2".into()));
    assert_eq!(backend.combo_box_item_text(combo, 2), Some("Item 3".into()));

    assert!(backend.combo_box_set_current_index(combo, 1));
    assert_eq!(backend.combo_box_current_index(combo), Some(1));

    assert!(backend.combo_box_clear_items(combo));
    assert_eq!(backend.combo_box_item_count(combo), 0);

    // ListBox item management.
    let list_box = backend.create_list_box(window, 10, 120, 100, 100);
    assert!(list_box > 0, "ListBox should be created");
    assert_eq!(backend.list_box_item_count(list_box), 0);

    assert!(backend.list_box_add_item(list_box, "A"));
    assert!(backend.list_box_add_item(list_box, "B"));
    assert_eq!(backend.list_box_item_count(list_box), 2);

    assert_eq!(backend.list_box_item_text(list_box, 0), Some("A".into()));
    assert_eq!(backend.list_box_item_text(list_box, 1), Some("B".into()));

    assert!(backend.list_box_set_current_index(list_box, 0));
    assert_eq!(backend.list_box_current_index(list_box), Some(0));

    assert!(backend.list_box_remove_item(list_box, 0));
    assert_eq!(backend.list_box_item_count(list_box), 1);
    assert_eq!(backend.list_box_item_text(list_box, 0), Some("B".into()));

    assert!(backend.list_box_clear_items(list_box));
    assert_eq!(backend.list_box_item_count(list_box), 0);
}

#[test]
fn widget_trigger_events() {
    let backend = HarmonyPlatform::new();
    backend.init();
    let window = backend.create_window("w", 0, 0, 200, 120);
    let button = backend.create_button(window, "btn", 10, 10, 80, 24);

    // Queue should be empty initially.
    assert!(backend.poll_widget_trigger_event().is_none());

    // Inject a clicked event.
    assert!(backend.inject_widget_trigger_event(button, WidgetTriggerKind::Clicked));

    // Poll the injected event.
    let event = backend.poll_widget_trigger_event();
    assert!(event.is_some(), "Should poll a trigger event");
    let event = event.unwrap();
    assert_eq!(event.widget_id, button);
    assert_eq!(event.kind, WidgetTriggerKind::Clicked);

    // No more events.
    assert!(backend.poll_widget_trigger_event().is_none());

    // Inject a value-changed event.
    assert!(backend.inject_widget_trigger_event(button, WidgetTriggerKind::ValueChanged));
    let event = backend.poll_widget_trigger_event();
    assert!(event.is_some(), "Should poll value-changed event");
    let event = event.unwrap();
    assert_eq!(event.widget_id, button);
    assert_eq!(event.kind, WidgetTriggerKind::ValueChanged);
}

#[test]
fn menu_lifecycle() {
    let backend = HarmonyPlatform::new();
    backend.init();
    let window = backend.create_window("w", 0, 0, 200, 120);

    let menu_bar = backend.create_menu_bar(window, 0, 0, 200, 24);
    assert!(menu_bar > 0, "MenuBar should be created");

    let menu = backend.create_menu(menu_bar, "File", 0, 0, 100, 24);
    assert!(menu > 0, "Menu should be created");

    let item = backend.menu_add_item(menu, "Open", None);
    assert!(item > 0, "MenuItem should be created");

    // Attach menu bar to window.
    assert!(
        backend.attach_menu_bar_to_window(window, menu_bar),
        "MenuBar should be attached to window"
    );

    // Inject and poll a menu trigger.
    assert!(
        backend.inject_menu_trigger(item),
        "Should inject menu trigger"
    );
    let triggered = backend.poll_menu_triggered();
    assert_eq!(triggered, Some(item), "Should poll triggered menu item");

    // Second poll should be empty.
    assert!(
        backend.poll_menu_triggered().is_none(),
        "No more menu triggers expected"
    );
}

#[test]
fn toolbar_and_statusbar() {
    let backend = HarmonyPlatform::new();
    backend.init();
    let window = backend.create_window("w", 0, 0, 200, 120);

    let toolbar = backend.create_tool_bar(window, 0, 0, 200, 24);
    assert!(toolbar > 0, "ToolBar should be created");
    backend.set_widget_visible(toolbar, true);
    assert!(
        backend.is_widget_visible(toolbar),
        "ToolBar should be visible"
    );

    let statusbar = backend.create_status_bar(window, "Ready", 0, 96, 200, 24);
    assert!(statusbar > 0, "StatusBar should be created");
    assert_eq!(backend.get_widget_text(statusbar), "Ready");
    backend.set_widget_text(statusbar, "Updated");
    assert_eq!(backend.get_widget_text(statusbar), "Updated");
    backend.set_widget_visible(statusbar, true);
    assert!(
        backend.is_widget_visible(statusbar),
        "StatusBar should be visible"
    );
}

#[test]
fn dialog_creation() {
    let backend = HarmonyPlatform::new();
    backend.init();
    let window = backend.create_window("w", 0, 0, 200, 120);

    let msg_box = backend.create_message_box(window, "Title", "Message", 0, 0, 200, 100);
    assert!(msg_box > 0, "MessageBox should be created");

    let file_dlg = backend.create_file_dialog(window, 0, 0, 400, 300);
    assert!(file_dlg > 0, "FileDialog should be created");

    let color_dlg = backend.create_color_dialog(window, 0, 0, 300, 200);
    assert!(color_dlg > 0, "ColorDialog should be created");

    let font_dlg = backend.create_font_dialog(window, 0, 0, 300, 200);
    assert!(font_dlg > 0, "FontDialog should be created");
}

#[test]
fn spin_box_list_view_scroll_area() {
    let backend = HarmonyPlatform::new();
    backend.init();
    let window = backend.create_window("w", 0, 0, 200, 120);

    let spin = backend.create_spin_box(window, 10, 10, 80, 24);
    assert!(spin > 0, "SpinBox should be created");

    let list_view = backend.create_list_view(window, 10, 40, 100, 80);
    assert!(list_view > 0, "ListView should be created");

    let scroll = backend.create_scroll_area(window, 10, 130, 100, 80);
    assert!(scroll > 0, "ScrollArea should be created");
}

#[test]
fn drag_and_drop() {
    let backend = HarmonyPlatform::new();
    backend.init();
    let window = backend.create_window("w", 0, 0, 200, 120);
    let button = backend.create_button(window, "btn", 10, 10, 80, 24);

    // Drag and drop: begin_drag succeeds because button is a valid widget.
    assert!(
        backend.begin_drag(button, "text/plain", b"payload"),
        "begin_drag should succeed for a valid widget"
    );
    assert!(
        backend.poll_drop_event().is_some(),
        "poll_drop_event should return Some after begin_drag"
    );
    assert!(
        backend.inject_drop_event(crate::platform::DropEvent {
            source_widget_id: button,
            target_widget_id: button,
            mime: "text/plain".into(),
            payload: vec![]
        }),
        "inject_drop_event should succeed for valid target widget"
    );
}
