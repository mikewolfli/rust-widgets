// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Integration tests for the Linux (GTK) backend.
//!
//! Scope note (BLUE15 #55/#56): this backend creates **no** native controls. The
//! library paints every `WidgetKind` itself, so the control-construction and
//! control-state methods keep the `Platform` trait defaults, which report "this
//! host holds no such control". What remains here is what the backend still owns:
//! the window, the GTK lifecycle, clipboard, accessibility forwarding, and the
//! honest reporting of absent controls.
//!
//! # GTK threading constraint
//!
//! GTK 3 allows `gtk::init()` to succeed on exactly one thread per process and
//! aborts the process if a second thread tries to initialize it. The Rust test
//! harness runs each `#[test]` on its own worker thread, so running one GTK test
//! per function would either fail or segfault. All GTK-backed scenarios are
//! therefore exercised from a single `#[test]` function so GTK is initialized
//! once; the state-only assertions remain in separate tests because they never
//! touch GTK.
//!
//! When no display is available `gtk::init()` fails, and the combined test
//! falls back to asserting the honest state-backend contract instead of
//! pretending the native GTK path ran.

use crate::platform::linux::LinuxPlatform;
use crate::platform::Platform;

/// Initialize GTK on the current thread.
///
/// Returns `false` when no display is reachable, which selects the state-only
/// fallback path in the combined GTK test.
#[cfg(all(target_os = "linux", feature = "gtk-native"))]
fn ensure_gtk() -> bool {
    gtk::init().is_ok()
}

/// The backend must expose the honest identity for the build it was compiled in.
///
/// Without `gtk-native` it never opens a native window, so reporting "gtk" would
/// be a lie; with it, the real toolkit is what drives the window.
#[cfg(not(all(target_os = "linux", feature = "gtk-native")))]
#[test]
fn state_backend_reports_honest_name() {
    let backend = LinuxPlatform::new();
    backend.init();
    assert_eq!(backend.backend_name(), "linux-state-backend");
}

/// A window is the one thing this backend still constructs, and it must come back
/// with an id that survives the lifecycle calls.
#[cfg(not(all(target_os = "linux", feature = "gtk-native")))]
#[test]
fn state_backend_window_lifecycle() {
    let backend = LinuxPlatform::new();
    backend.init();

    let window = backend.create_window("TestWindow", 50, 50, 400, 300);
    assert!(window > 0, "Window should be created");
    assert!(backend.destroy_widget(window), "an existing window must be destroyable");
    assert!(!backend.destroy_widget(window), "a destroyed window must not exist twice");
}

/// This host creates no native controls, so every `create_*` must fall back to the
/// trait default and report `0` rather than inventing a usable id.
///
/// Returning a non-zero id while creating nothing is the dishonesty the trait
/// default removes; this pins the honest answer for the whole constructor family.
#[cfg(not(all(target_os = "linux", feature = "gtk-native")))]
#[test]
fn state_backend_reports_no_native_controls() {
    let backend = LinuxPlatform::new();
    backend.init();
    let window = backend.create_window("w", 0, 0, 400, 300);
    assert!(window > 0);

    // Round-2 containers.
    assert_eq!(backend.create_group_box(window, "g", 0, 0, 10, 10), 0);
    assert_eq!(backend.create_frame(window, 0, 0, 10, 10), 0);
    assert_eq!(backend.create_tab_widget(window, 0, 0, 10, 10), 0);
    assert_eq!(backend.create_splitter(window, 0, 0, 10, 10), 0);
    // Round-3 controls.
    assert_eq!(backend.create_toggle_button(window, "t", 0, 0, 10, 10), 0);
    assert_eq!(backend.create_calendar(window, 0, 0, 10, 10), 0);
    assert_eq!(backend.create_scroll_bar(window, 0, 0, 10, 10), 0);
    assert_eq!(backend.create_double_spin_box(window, 0, 0, 10, 10), 0);
    assert_eq!(backend.create_font_combo_box(window, 0, 0, 10, 10), 0);
    // Round-4 dialogs / popups / context menu.
    assert_eq!(backend.create_context_menu(window, 0, 0, 10, 10), 0);
    assert_eq!(backend.create_popup_window(window, "p", 0, 0, 10, 10), 0);
    assert_eq!(backend.create_dialog(window, "d", 0, 0, 10, 10), 0);
    assert_eq!(backend.create_input_dialog(window, 0, 0, 10, 10), 0);
    assert_eq!(backend.create_progress_dialog(window, 0, 0, 10, 10), 0);
    assert_eq!(backend.create_directory_dialog(window, "t", 0, 0, 10, 10), 0);
    // Round-5 pickers / busy indicator.
    assert_eq!(backend.create_date_picker(window, 0, 0, 10, 10), 0);
    assert_eq!(backend.create_time_picker(window, 0, 0, 10, 10), 0);
    assert_eq!(backend.create_date_time_picker(window, 0, 0, 10, 10), 0);
    assert_eq!(backend.create_activity_indicator(window, 0, 0, 10, 10), 0);
}

/// Control state belongs to the widget, so the trait defaults report absence
/// rather than a value invented by this backend.
#[cfg(not(all(target_os = "linux", feature = "gtk-native")))]
#[test]
fn state_backend_owns_no_control_state() {
    let backend = LinuxPlatform::new();
    backend.init();
    let window = backend.create_window("w", 0, 0, 400, 300);

    assert_eq!(backend.widget_value(window), None, "no numeric value to report");
    assert_eq!(backend.is_widget_checked(window), None, "nothing here is checkable");
    assert!(!backend.is_widget_visible(window), "the host holds no control visibility");
}

/// The AT-SPI, clipboard and drop plumbing stay behind this backend even without
/// GTK, because those are Linux subsystems rather than GTK widgets.
#[cfg(not(all(target_os = "linux", feature = "gtk-native")))]
#[test]
fn state_backend_linux_subsystems_still_forward() {
    let backend = LinuxPlatform::new();
    backend.init();

    // The clipboard keeps an in-process mirror when no display is reachable.
    assert!(backend.set_clipboard_text("linux_clip_test"));
    assert_eq!(backend.get_clipboard_text(), "linux_clip_test");

    // Accessibility forwarding works without GTK: the AT-SPI bridge is a
    // Linux-only (not gtk-gated) component.
    let window = backend.create_window("w", 0, 0, 400, 300);
    if let Some(bridge) = <LinuxPlatform as Platform>::accessibility_bridge(&backend) {
        let _ = bridge.accessibility_name(window);
    }
    assert_eq!(
        <LinuxPlatform as Platform>::accessibility_bridge(&backend).is_some(),
        cfg!(target_os = "linux"),
        "the AT-SPI bridge is available exactly on Linux"
    );
}

/// Exercise the full GTK-backed Linux backend from one thread.
///
/// Covers: window creation and teardown, the honest control contract, clipboard
/// round-trip through the real GDK display clipboard, and accessibility
/// forwarding.
#[cfg(all(target_os = "linux", feature = "gtk-native"))]
#[test]
fn gtk_native_backend_lifecycle() {
    if !ensure_gtk() {
        // Honest fallback: without a display the native backend cannot run.
        let backend = LinuxPlatform::new();
        backend.init();
        assert_eq!(backend.backend_name(), "linux-state-backend");
        return;
    }

    let backend = LinuxPlatform::new();
    backend.init();
    assert_eq!(backend.backend_name(), "gtk");

    // ── Window ──
    let window = backend.create_window("TestWindow", 50, 50, 400, 300);
    assert!(window > 0, "Window should be created");

    // ── Honest control contract ──
    // The library paints every control itself, so this host must report that it
    // creates none rather than handing back a state-only id that nothing backs.
    assert_eq!(backend.create_button(window, "Click", 10, 10, 80, 24), 0);
    assert_eq!(backend.create_label(window, "Hello", 10, 40, 80, 24), 0);
    assert_eq!(backend.create_checkbox(window, "Check", 10, 70, 80, 24), 0);
    assert_eq!(backend.create_line_edit(window, "edit", 10, 100, 160, 24), 0);
    assert_eq!(backend.create_slider(window, 10, 160, 200, 24), 0);

    // ── Clipboard (native GTK/GDK path) ──
    assert!(backend.set_clipboard_text("linux_clip_test"));
    assert_eq!(backend.get_clipboard_text(), "linux_clip_test");
    assert!(backend.set_clipboard_text(""));
    assert_eq!(backend.get_clipboard_text(), "");

    // Prove the value reached the real GDK display clipboard and not only the
    // in-process mirror: read it back through an independent Clipboard handle.
    backend.set_clipboard_text("native-clipboard-proof");
    if let Some(display) = gtk::gdk::Display::default() {
        if let Some(clipboard) = gtk::Clipboard::default(&display) {
            let native = clipboard.wait_for_text().map(|s| s.to_string());
            assert_eq!(
                native.as_deref(),
                Some("native-clipboard-proof"),
                "clipboard text must be visible through the native GDK clipboard"
            );
        }
    }

    // ── Accessibility (AT-SPI bridge is authoritative) ──
    if let Some(bridge) =
        <crate::platform::linux::LinuxPlatform as Platform>::accessibility_bridge(&backend)
    {
        let _ = bridge.accessibility_name(window);
    }

    // ── Window teardown ──
    assert!(backend.destroy_widget(window), "the window must be destroyable");
    assert!(!backend.destroy_widget(window), "a destroyed window must not exist twice");
}
