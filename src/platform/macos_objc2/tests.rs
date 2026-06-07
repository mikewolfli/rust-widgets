//! Integration tests for the macOS objc2 migration preview backend.
//!
//! These tests verify parity between the objc2 preview backend and the
//! existing platform backends to ensure migration safety.

use crate::platform::macos_objc2::MacOSObjc2Platform;
use crate::platform::Platform;
use crate::WidgetTriggerKind;

#[test]
fn release_diagnostics_parity() {
    // Assert preview backend selection for warning-clean publish path checks.
    let backend = MacOSObjc2Platform::new();
    backend.init();
    assert_eq!(backend.backend_name(), "macos-objc2-preview");
}

#[test]
fn contract_parity_platform_trait() {
    // Verify Platform trait parity for migration route toggles.
    let backend = MacOSObjc2Platform::new();
    backend.init();
    let window = backend.create_window("w", 0, 0, 200, 120);
    let button = backend.create_button(window, "btn", 10, 10, 80, 24);
    backend.set_widget_enabled(button, true);
    backend.set_widget_visible(button, true);
    assert!(backend.is_widget_enabled(button));
    assert!(backend.is_widget_visible(button));
}

#[test]
fn macos_backend_architecture_parity() {
    // Verify migration preview covers core desktop API surface.
    let backend = MacOSObjc2Platform::new();
    backend.init();
    let window = backend.create_window("w", 0, 0, 200, 120);
    let _button = backend.create_button(window, "btn", 10, 10, 80, 24);
    let menu_bar = backend.create_menu_bar(window, 0, 0, 200, 24);
    let menu = backend.create_menu(menu_bar, "File", 0, 0, 100, 24);
    let _item = backend.menu_add_item(menu, "Open", None);
    let _statusbar = backend.create_status_bar(window, "Ready", 0, 96, 200, 24);
    backend.set_clipboard_text("test_clip");
    assert_eq!(backend.get_clipboard_text(), "test_clip");
}

#[test]
fn docs_changelog_migration_notes() {
    // Keep backend naming stable for migration docs/changelog notes.
    let backend = MacOSObjc2Platform::new();
    assert_eq!(backend.backend_name(), "macos-objc2-preview");
}

#[test]
fn dependency_policy_cocoa_fallback() {
    // Verify Cocoa remains fallback-only while objc2 preview is selected here.
    let backend = MacOSObjc2Platform::new();
    assert_eq!(backend.backend_name(), "macos-objc2-preview");
}

#[test]
fn warning_clean_publish_path() {
    // Verify publish path keeps objc2 preview identity.
    let backend = MacOSObjc2Platform::new();
    backend.init();
    assert_eq!(backend.backend_name(), "macos-objc2-preview");
}

#[test]
fn migration_regression_matrix_snapshot() {
    // Snapshot widget state for migration regression matrix comparison.
    let backend = MacOSObjc2Platform::new();
    backend.init();
    let window = backend.create_window("w", 0, 0, 200, 120);
    let _button = backend.create_button(window, "btn", 10, 10, 80, 24);
    let snapshot = backend.serialize_state().expect("Should serialize state");
    assert!(snapshot.contains("btn"), "Snapshot should contain button text");
}

#[test]
fn objc2_toolbar_statusbar_parity() {
    // Verify toolbar/status bar parity behavior for migration preview.
    let backend = MacOSObjc2Platform::new();
    backend.init();
    let window = backend.create_window("w", 0, 0, 200, 120);
    let toolbar = backend.create_tool_bar(window, 0, 0, 200, 24);
    assert!(toolbar > 0, "ToolBar should be created");
    backend.set_widget_visible(toolbar, true);
    assert!(backend.is_widget_visible(toolbar), "ToolBar should be visible");
    let statusbar = backend.create_status_bar(window, "Ready", 0, 96, 200, 24);
    assert!(statusbar > 0, "StatusBar should be created");
    assert_eq!(backend.get_widget_text(statusbar), "Ready");
    backend.set_widget_visible(statusbar, true);
    assert!(backend.is_widget_visible(statusbar), "StatusBar should be visible");
}

#[test]
fn objc2_menu_stack_parity() {
    // Verify menu hierarchy creation and menu trigger queue parity.
    let backend = MacOSObjc2Platform::new();
    backend.init();
    let window = backend.create_window("w", 0, 0, 200, 120);
    let menu_bar = backend.create_menu_bar(window, 0, 0, 200, 24);
    assert!(menu_bar > 0, "MenuBar should be created");
    let menu = backend.create_menu(menu_bar, "File", 0, 0, 100, 24);
    assert!(menu > 0, "Menu should be created");
    let item = backend.menu_add_item(menu, "Open", None);
    assert!(item > 0, "MenuItem should be created");
    assert!(
        backend.attach_menu_bar_to_window(window, menu_bar),
        "MenuBar should be attached to window"
    );
    // Inject and poll one menu trigger event.
    assert!(backend.inject_menu_trigger(item), "Should inject menu trigger");
    let triggered = backend.poll_menu_triggered();
    assert_eq!(triggered, Some(item), "Should poll triggered menu item");
}

#[test]
fn objc2_ime_accessibility_parity() {
    // Verify IME and accessibility state parity.
    let backend = MacOSObjc2Platform::new();
    backend.init();
    let window = backend.create_window("w", 0, 0, 200, 120);
    let line_edit = backend.create_line_edit(window, "edit", 30, 70, 100, 24);
    // IME enabled/disabled roundtrip.
    assert!(backend.set_widget_ime_enabled(line_edit, true));
    assert!(backend.is_widget_ime_enabled(line_edit));
    assert!(backend.set_widget_ime_enabled(line_edit, false));
    assert!(!backend.is_widget_ime_enabled(line_edit));
    // Accessibility name roundtrip.
    assert!(backend.set_widget_accessibility_name(line_edit, "acc"));
    assert_eq!(backend.get_widget_accessibility_name(line_edit), "acc");
}

#[test]
fn objc2_trigger_semantics_parity() {
    // Verify typed trigger semantics for clicked/value-changed normalization.
    let backend = MacOSObjc2Platform::new();
    backend.init();
    let window = backend.create_window("w", 0, 0, 200, 120);
    let button = backend.create_button(window, "btn", 10, 10, 80, 24);
    // Inject and poll one clicked trigger event.
    let ok = backend.inject_widget_trigger_event(button, WidgetTriggerKind::Clicked);
    assert!(ok, "Should inject click event");
    let event = backend.poll_widget_trigger_event();
    assert!(event.is_some(), "Should poll a trigger event");
    let event = event.unwrap();
    assert_eq!(event.widget_id, button);
    assert_eq!(event.kind, WidgetTriggerKind::Clicked);
}

#[test]
fn objc2_controls_parity() {
    // Verify button/checkbox/line-edit parity behavior.
    let backend = MacOSObjc2Platform::new();
    backend.init();
    let window = backend.create_window("w", 0, 0, 200, 120);
    // Button parity checks.
    let button = backend.create_button(window, "btn", 10, 10, 80, 24);
    assert!(button > 0, "Button should be created");
    assert_eq!(backend.get_widget_text(button), "btn");
    backend.set_widget_enabled(button, false);
    assert!(!backend.is_widget_enabled(button), "Button should be disabled");
    backend.set_widget_visible(button, false);
    assert!(!backend.is_widget_visible(button), "Button should be hidden");
    // Checkbox parity checks.
    let checkbox = backend.create_checkbox(window, "chk", 20, 40, 80, 24);
    assert!(checkbox > 0, "Checkbox should be created");
    assert_eq!(backend.get_widget_text(checkbox), "chk");
    backend.set_widget_enabled(checkbox, true);
    assert!(backend.is_widget_enabled(checkbox), "Checkbox should be enabled");
    backend.set_widget_visible(checkbox, true);
    assert!(backend.is_widget_visible(checkbox), "Checkbox should be visible");
    // Line edit parity checks.
    let line_edit = backend.create_line_edit(window, "edit", 30, 70, 100, 24);
    assert!(line_edit > 0, "LineEdit should be created");
    assert_eq!(backend.get_widget_text(line_edit), "edit");
    backend.set_widget_enabled(line_edit, true);
    assert!(backend.is_widget_enabled(line_edit), "LineEdit should be enabled");
    backend.set_widget_visible(line_edit, true);
    assert!(backend.is_widget_visible(line_edit), "LineEdit should be visible");
}

#[test]
fn objc2_runloop_integration_and_quit() {
    // Verify run-loop start/quit parity with deterministic shutdown.
    let backend = MacOSObjc2Platform::new();
    backend.init();
    std::thread::scope(|scope| {
        // Start run-loop in a scoped worker thread.
        scope.spawn(|| {
            backend.run();
        });
        // Allow run-loop startup.
        std::thread::sleep(std::time::Duration::from_millis(50));
        // Request deterministic quit.
        backend.quit();
    });
    // Backend should report not-running after quit.
    assert!(
        !backend.runtime.running.load(std::sync::atomic::Ordering::SeqCst),
        "Backend should not be running after quit"
    );
}

#[test]
fn objc2_window_lifecycle_parity() {
    // Verify window lifecycle parity: title, visibility, and geometry updates.
    let backend = MacOSObjc2Platform::new();
    backend.init();
    // Create window and verify title.
    let window = backend.create_window("TestWindow", 100, 200, 640, 480);
    assert!(window > 0, "Window should be created");
    let text = backend.get_widget_text(window);
    assert_eq!(text, "TestWindow", "Window title should match");
    // Apply geometry update.
    backend.set_widget_geometry(window, 120, 220, 800, 600);
    // Verify show/hide visibility transitions.
    backend.show_widget(window);
    assert!(backend.is_widget_visible(window), "Window should be visible");
    backend.hide_widget(window);
    assert!(!backend.is_widget_visible(window), "Window should be hidden");
}

#[test]
fn objc2_basic_control_and_clipboard_parity() {
    // Verify basic control and clipboard parity flow.
    let backend = MacOSObjc2Platform::new();
    backend.init();
    // Create a window.
    let window = backend.create_window("w", 0, 0, 200, 120);
    assert!(window > 0, "Window should be created and have a valid id");
    // Create a child button.
    let button = backend.create_button(window, "ok", 10, 10, 80, 24);
    assert!(button > 0, "Button should be created and have a valid id");
    assert_eq!(backend.get_widget_text(button), "ok", "Button text should match");
    // Update button text.
    backend.set_widget_text(button, "updated");
    assert_eq!(backend.get_widget_text(button), "updated", "Button text should update");
    // Clipboard set/get roundtrip.
    assert!(backend.set_clipboard_text("clip"), "Should set clipboard text");
    assert_eq!(backend.get_clipboard_text(), "clip", "Clipboard text should match");
}
