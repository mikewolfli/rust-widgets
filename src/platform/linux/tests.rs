//! Integration tests for the Linux (GTK) backend.
//!
//! These tests verify platform creation, basic widget lifecycle,
//! and clipboard roundtrip for the Linux platform backend.

use crate::platform::linux::LinuxPlatform;
use crate::platform::Platform;

#[test]
#[cfg_attr(
    feature = "gtk-native",
    ignore = "gtk requires main-thread test execution"
)]
fn platform_creates_and_runs() {
    let backend = LinuxPlatform::new();
    backend.init();
    assert_eq!(backend.backend_name(), "gtk");

    // Create a window and a few basic widgets.
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
#[cfg_attr(
    feature = "gtk-native",
    ignore = "gtk requires main-thread test execution"
)]
fn widget_lifecycle() {
    let backend = LinuxPlatform::new();
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
#[cfg_attr(
    feature = "gtk-native",
    ignore = "gtk requires main-thread test execution"
)]
fn clipboard_roundtrip() {
    let backend = LinuxPlatform::new();
    backend.init();

    // Create a window so the backend is fully initialised.
    let _window = backend.create_window("ClipTest", 0, 0, 200, 120);

    // Clipboard set/get roundtrip.
    assert!(
        backend.set_clipboard_text("linux_clip_test"),
        "Should set clipboard text"
    );
    assert_eq!(
        backend.get_clipboard_text(),
        "linux_clip_test",
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
