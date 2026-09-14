// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Integration tests for the Wayland backend.
//!
//! These tests verify the host-facing capabilities that survived BLUE15: platform
//! creation and the runtime loop, the native session and its degrade path, window
//! lifecycle state, clipboard roundtrip, the in-process menu model, the injectable
//! widget-trigger queue, drag and drop, IME and accessibility names.
//!
//! Tests for the deleted `create_*` control creators are gone with their subject.
//! They asserted that a `create_button`-style call returned a non-zero id, which is
//! exactly the behaviour BLUE15 removed: the library paints every `WidgetKind`, and
//! those methods now fall through to the `Platform` defaults that return `0`.

use crate::platform::wayland::WaylandPlatform;
use crate::platform::Platform;

/// With a live compositor on `WAYLAND_DISPLAY`, creating a window must take the
/// *native* path: connect to the compositor, bind `wl_compositor` and
/// `xdg_wm_base`, and create a real `xdg_toplevel` surface.
///
/// This is the automated half of the "Wayland compositor interaction" gap: a
/// state-only backend never populates `native_session`, so asserting that it is
/// `Some` after `create_window` proves the protocol path actually ran rather
/// than silently falling back.
///
/// The test is opt-in — it skips (with a notice) when no compositor is present,
/// so a headless CI runner without weston does not fail. To run it against a
/// real compositor use `tools/run_wayland_compositor_tests.sh`, which fetches
/// and/or installs weston and drives both this test and its negative control.
#[cfg(all(feature = "wayland-native", target_os = "linux"))]
#[test]
fn native_session_binds_live_compositor() {
    use crate::core::PlatformFamily;
    use crate::platform::Platform as _;

    // No compositor advertised → nothing to verify here; skip rather than fail.
    if !compositor_advertised() {
        eprintln!("skipping: WAYLAND_DISPLAY is not set (no compositor to bind)");
        return;
    }

    let backend = WaylandPlatform::new();
    backend.init();
    assert_eq!(backend.family(), PlatformFamily::Desktop);

    let window = backend.create_window("NativeProbe", 0, 0, 320, 200);
    assert!(window > 0, "window should be created");

    // The state handle alone does not prove the native path; the session does.
    let guard = backend.native_session.lock().expect("native_session mutex poisoned");
    let session = guard.as_ref().expect(
        "native_session must be Some after create_window with a live compositor \
         (state-only fallback would leave it None)",
    );

    // Both globals must have been bound during the registry roundtrip.
    assert!(
        session.state.compositor.is_some(),
        "wl_compositor must be bound from the live compositor"
    );
    assert!(
        session.state.xdg_wm_base.is_some(),
        "xdg_wm_base must be bound from the live compositor"
    );

    eprintln!(
        "bound live compositor: wl_compositor=yes xdg_wm_base=yes dpi_scale={}",
        session.state.dpi_scale
    );
}

/// Whether the environment advertises a usable compositor.
///
/// An *empty* `WAYLAND_DISPLAY` does not name a socket, so it is treated the
/// same as an unset one — this lets the negative control run under
/// `WAYLAND_DISPLAY=` instead of being silently skipped.
#[cfg(all(feature = "wayland-native", target_os = "linux"))]
fn compositor_advertised() -> bool {
    matches!(std::env::var("WAYLAND_DISPLAY"), Ok(v) if !v.is_empty())
}

/// Without a compositor the backend must degrade to state-only and leave
/// `native_session` empty — this is the negative control for the test above.
#[cfg(all(feature = "wayland-native", target_os = "linux"))]
#[test]
fn native_session_stays_empty_without_compositor() {
    use crate::platform::Platform as _;

    if compositor_advertised() {
        eprintln!("skipping: a compositor is advertised, cannot test the fallback");
        return;
    }

    let backend = WaylandPlatform::new();
    backend.init();
    let window = backend.create_window("Fallback", 0, 0, 200, 120);
    assert!(window > 0, "state-only window should still be created");

    let guard = backend.native_session.lock().expect("native_session mutex poisoned");
    assert!(guard.is_none(), "native_session must stay empty when there is no compositor");
}

/// The event loop must observe `quit()` promptly even with no compositor: the
/// fd-based loop falls back to a bounded idle wait when there is no native
/// session, so `run()` cannot hang. This guards the quit-latency contract.
#[test]
fn run_exits_promptly_on_quit_without_compositor() {
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    let backend = Arc::new(WaylandPlatform::new());
    backend.init();

    let runner = Arc::clone(&backend);
    let handle = std::thread::spawn(move || runner.run());

    // Let the loop enter its wait state, then quit from this thread.
    std::thread::sleep(Duration::from_millis(50));
    let started = Instant::now();
    backend.quit();

    // The loop re-checks the flag at least once per idle timeout, so joining
    // must complete well within a generous bound.
    let mut finished = false;
    for _ in 0..100 {
        if handle.is_finished() {
            finished = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let elapsed = started.elapsed();
    if !finished {
        // Avoid a hung test process if the contract regressed.
        panic!("run() did not exit within 2s of quit() (elapsed {elapsed:?})");
    }
    handle.join().expect("event loop thread panicked");
    assert!(elapsed < Duration::from_millis(500), "quit() took too long: {elapsed:?}");
}

#[test]
fn platform_creates_and_runs() {
    let backend = WaylandPlatform::new();
    backend.init();
    assert_eq!(backend.backend_name(), "wayland");

    // The window is the one host capability this backend owns.
    let window = backend.create_window("TestWindow", 50, 50, 400, 300);
    assert!(window > 0, "Window should be created");
}

/// The three controls the library paints are *not* created by the host, so the
/// `Platform` defaults answer for them. This pins the post-BLUE15 contract for the
/// Wayland backend: the advertised capabilities and the control creators must agree.
#[test]
fn host_creates_no_controls() {
    let backend = WaylandPlatform::new();
    backend.init();
    assert!(
        !backend.capabilities().native_menu,
        "the Wayland menu is an in-process tree, not a compositor menu"
    );

    let window = backend.create_window("NoControls", 0, 0, 400, 300);
    assert!(window > 0, "Window should be created");

    // A valid parent is supplied, so only the removed override can explain a 0.
    assert_eq!(backend.create_button(window, "Click", 10, 10, 80, 24), 0);
    assert_eq!(backend.create_label(window, "Hello", 10, 40, 80, 24), 0);
    assert_eq!(backend.create_checkbox(window, "Check", 10, 70, 80, 24), 0);
    assert_eq!(backend.create_line_edit(window, "edit", 10, 100, 160, 24), 0);
    assert_eq!(backend.create_combo_box(window, 10, 220, 140, 24), 0);
    assert_eq!(backend.create_list_box(window, 10, 250, 140, 80), 0);
    assert_eq!(backend.create_spin_box(window, 10, 10, 80, 24), 0);
    assert_eq!(backend.create_message_box(window, "Title", "Body", 10, 10, 300, 150), 0);
}

/// The window is a real state record, so the ordinary widget operations still
/// round-trip on it.
#[test]
fn window_lifecycle() {
    let backend = WaylandPlatform::new();
    backend.init();

    let window = backend.create_window("Lifecycle", 0, 0, 200, 120);
    assert!(window > 0, "Window should be created");

    backend.set_widget_text(window, "UpdatedTitle");
    assert_eq!(backend.get_widget_text(window), "UpdatedTitle");

    backend.show_widget(window);
    assert!(backend.is_widget_visible(window), "Window should be visible");

    backend.hide_widget(window);
    assert!(!backend.is_widget_visible(window), "Window should be hidden");

    backend.set_widget_enabled(window, false);
    assert!(!backend.is_widget_enabled(window), "Window should be disabled");

    backend.set_widget_geometry(window, 20, 20, 320, 240);

    backend.set_widget_accessibility_name(window, "MainWindow");
    assert_eq!(backend.get_widget_accessibility_name(window), "MainWindow");

    assert!(backend.destroy_widget(window), "Window should be destroyed");
    assert!(!backend.destroy_widget(window), "A destroyed window cannot be destroyed twice");
}

#[test]
fn clipboard_roundtrip() {
    let backend = WaylandPlatform::new();
    backend.init();

    // Create a window so the backend is fully initialised.
    let _window = backend.create_window("ClipTest", 0, 0, 200, 120);

    // Clipboard set/get roundtrip.
    assert!(backend.set_clipboard_text("wayland_clip_test"), "Should set clipboard text");
    assert_eq!(backend.get_clipboard_text(), "wayland_clip_test", "Clipboard text should match");

    // Overwrite with new content.
    assert!(backend.set_clipboard_text("updated_clip"), "Should overwrite clipboard");
    assert_eq!(backend.get_clipboard_text(), "updated_clip", "Updated clipboard text should match");

    // Empty string.
    assert!(backend.set_clipboard_text(""), "Should set empty clipboard");
    assert_eq!(backend.get_clipboard_text(), "", "Empty clipboard should match");
}

#[test]
fn menu_system() {
    let backend = WaylandPlatform::new();
    backend.init();

    let window = backend.create_window("MenuTest", 0, 0, 400, 300);
    let menu_bar = backend.create_menu_bar(window, 0, 0, 400, 24);
    assert!(menu_bar > 0, "MenuBar should be created");

    let file_menu = backend.create_menu(menu_bar, "File", 0, 24, 100, 24);
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
    assert!(backend.inject_menu_trigger(open_item), "Should inject menu trigger");
    assert_eq!(backend.poll_menu_triggered(), Some(open_item), "Should poll open item");

    // Second trigger.
    assert!(backend.inject_menu_trigger(quit_item), "Should inject quit trigger");
    assert_eq!(backend.poll_menu_triggered(), Some(quit_item), "Should poll quit item");
}

#[test]
fn widget_trigger_events() {
    use crate::platform::WidgetTriggerKind;

    let backend = WaylandPlatform::new();
    backend.init();

    let window = backend.create_window("TriggerTest", 0, 0, 200, 120);

    assert!(
        backend.inject_widget_trigger_event(window, WidgetTriggerKind::Clicked),
        "Should inject click event"
    );

    let event = backend.poll_widget_trigger_event();
    assert!(event.is_some(), "Should poll trigger event");
    assert_eq!(event.unwrap().widget_id, window);
    assert_eq!(event.unwrap().kind, WidgetTriggerKind::Clicked, "Should match Clicked kind");
    assert_eq!(backend.poll_widget_triggered(), None, "The queue must be drained");
}

#[test]
fn invalid_parent_and_kind_validation() {
    let backend = WaylandPlatform::new();
    backend.init();

    let window = backend.create_window("Validation", 0, 0, 400, 300);
    assert!(window > 0);

    let bogus = 9999;

    // A menu must hang off a menu bar (or another menu), not a window.
    assert_eq!(backend.create_menu(window, "File", 0, 0, 10, 10), 0);

    let menu_bar = backend.create_menu_bar(window, 0, 0, 400, 24);
    assert!(menu_bar > 0);
    let menu = backend.create_menu(menu_bar, "File", 0, 0, 10, 10);
    assert!(menu > 0, "menu under a menu bar must succeed");

    // Menu item requires a menu parent.
    assert_eq!(backend.menu_add_item(window, "Bad", None), 0);
    let item = backend.menu_add_item(menu, "Open", None);
    assert!(item > 0);

    // Only a menu item may be injected as a menu trigger.
    assert!(!backend.inject_menu_trigger(window));
    assert!(!backend.inject_menu_trigger(menu_bar));
    assert!(backend.inject_menu_trigger(item));

    // attach_menu_bar_to_window validates both ids and their kinds.
    assert!(!backend.attach_menu_bar_to_window(bogus, menu_bar));
    assert!(!backend.attach_menu_bar_to_window(window, bogus));
    assert!(!backend.attach_menu_bar_to_window(window, item));
    assert!(backend.attach_menu_bar_to_window(window, menu_bar));

    // inject_widget_trigger_event rejects unknown ids.
    use crate::platform::WidgetTriggerKind;
    assert!(!backend.inject_widget_trigger_event(bogus, WidgetTriggerKind::Clicked));
    assert!(backend.inject_widget_trigger_event(window, WidgetTriggerKind::Clicked));

    // Destroying a menu bar detaches it from its window.
    assert!(backend.destroy_widget(menu_bar));
    assert_eq!(backend.create_menu(window, "File", 0, 0, 10, 10), 0);
}

#[test]
fn drag_and_drop() {
    let backend = WaylandPlatform::new();
    backend.init();

    let window = backend.create_window("DragTest", 0, 0, 400, 300);

    // Begin drag from the window; the library paints the drag source's feedback.
    assert!(backend.begin_drag(window, "text/plain", b"drag payload"), "Should begin drag");

    // Poll drop event.
    let drop = backend.poll_drop_event();
    assert!(drop.is_some(), "Should poll drop event");
    let drop = drop.unwrap();
    assert_eq!(drop.source_widget_id, window);
    assert_eq!(drop.mime, "text/plain");
    assert_eq!(drop.payload, b"drag payload");
}

#[test]
fn ime_and_accessibility() {
    let backend = WaylandPlatform::new();
    backend.init();

    let window = backend.create_window("IMETest", 0, 0, 400, 300);

    // The IME flag is a per-widget flag the input path consults, so it round-trips
    // on the window even though the host creates no text field for it.
    assert!(backend.set_widget_ime_enabled(window, true), "Should enable IME");
    assert!(backend.is_widget_ime_enabled(window), "IME should be enabled");

    assert!(backend.set_widget_ime_enabled(window, false), "Should disable IME");
    assert!(!backend.is_widget_ime_enabled(window), "IME should be disabled");

    // Accessibility name roundtrip.
    assert!(
        backend.set_widget_accessibility_name(window, "InputField"),
        "Should set accessibility name"
    );
    assert_eq!(
        backend.get_widget_accessibility_name(window),
        "InputField",
        "Accessibility name should match"
    );
}
