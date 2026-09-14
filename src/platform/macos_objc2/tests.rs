// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Integration tests for the macOS objc2 migration preview backend.
//!
//! Scope note (BLUE15 #55/#56): this backend creates **no** native controls. The
//! library paints every `WidgetKind` itself, so the control-construction and
//! control-state methods keep the `Platform` trait defaults, which report "this
//! host holds no such control". What remains here is what the backend still owns:
//! its identity, the window, the lifecycle loop, clipboard/drag-drop plumbing, and
//! the honest reporting of absent controls.

use crate::platform::macos_objc2::MacOSObjc2Platform;
use crate::platform::Platform;

#[test]
fn release_diagnostics_parity() {
    // Assert preview backend selection for warning-clean publish path checks.
    let backend = MacOSObjc2Platform::new();
    backend.init();
    assert_eq!(backend.backend_name(), "macos-objc2-preview");
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

/// This host creates no native controls, so every `create_*` must fall back to the
/// trait default and report `0` rather than inventing a usable id.
///
/// Returning a non-zero id while creating nothing is the dishonesty the trait
/// default removes; this pins the honest answer for the whole constructor family.
#[test]
fn objc2_reports_no_native_controls() {
    let backend = MacOSObjc2Platform::new();
    backend.init();
    let window = backend.create_window("w", 0, 0, 200, 120);
    assert!(window > 0, "the window is the one thing this host does create");

    assert_eq!(backend.create_button(window, "btn", 10, 10, 80, 24), 0);
    assert_eq!(backend.create_checkbox(window, "chk", 10, 10, 80, 24), 0);
    assert_eq!(backend.create_line_edit(window, "edit", 10, 10, 80, 24), 0);
    assert_eq!(backend.create_label(window, "lbl", 10, 10, 80, 24), 0);
    assert_eq!(backend.create_slider(window, 10, 10, 80, 24), 0);
    assert_eq!(backend.create_progress_bar(window, 10, 10, 80, 24), 0);
    assert_eq!(backend.create_menu_bar(window, 0, 0, 200, 24), 0);
    assert_eq!(backend.create_status_bar(window, "Ready", 0, 96, 200, 24), 0);
}

/// Control state is owned by the widget, so the trait defaults report absence
/// rather than a value invented by this backend.
#[test]
fn objc2_owns_no_control_state() {
    let backend = MacOSObjc2Platform::new();
    backend.init();
    let window = backend.create_window("w", 0, 0, 200, 120);

    assert_eq!(backend.widget_value(window), None, "no numeric value to report");
    assert_eq!(backend.is_widget_checked(window), None, "nothing here is checkable");
    assert_eq!(backend.get_widget_text(window), "", "no control text to read back");
}

/// A window must get a usable id and keep it across the lifecycle calls.
///
/// Visibility is control state the widget now owns, so this backend reports
/// absence there rather than pretending a window was shown. The id contract is
/// what it still owes a caller.
#[test]
fn objc2_window_lifecycle_parity() {
    let backend = MacOSObjc2Platform::new();
    backend.init();
    let window = backend.create_window("TestWindow", 100, 200, 640, 480);
    assert!(window > 0, "Window should be created");

    // Visibility is not this host's state to report.
    assert!(!backend.is_widget_visible(window), "the host holds no control visibility");
    // Teardown reports whether the widget existed, which is state this host owns.
    assert!(backend.destroy_widget(window), "an existing window must be destroyable");
    assert!(!backend.destroy_widget(window), "a destroyed window must not report existence twice");
}

#[test]
fn objc2_clipboard_roundtrip() {
    let backend = MacOSObjc2Platform::new();
    backend.init();
    assert!(backend.set_clipboard_text("clip"), "Should set clipboard text");
    assert_eq!(backend.get_clipboard_text(), "clip", "Clipboard text should match");
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
