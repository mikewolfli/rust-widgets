// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Integration tests for the Harmony desktop backend.
//!
//! Every `WidgetKind` is painted by `src/widget/`, so the backend is a pure state
//! model: windows carry geometry/text, trigger queues carry events, and there is
//! no native-control surface left to exercise.

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
    assert_eq!(backend.get_widget_text(window), "TestWindow");
}

#[test]
fn widget_lifecycle() {
    let backend = HarmonyPlatform::new();
    backend.init();
    let window = backend.create_window("w", 0, 0, 200, 120);
    assert!(window > 0, "Window should be created");

    // Test show/hide.
    backend.show_widget(window);
    assert!(backend.is_widget_visible(window), "Widget should be visible after show");

    backend.hide_widget(window);
    assert!(!backend.is_widget_visible(window), "Widget should be hidden after hide");

    backend.show_widget(window);
    assert!(backend.is_widget_visible(window), "Widget should be visible after second show");

    // Test enable/disable.
    backend.set_widget_enabled(window, false);
    assert!(!backend.is_widget_enabled(window), "Widget should be disabled");

    backend.set_widget_enabled(window, true);
    assert!(backend.is_widget_enabled(window), "Widget should be enabled");

    // Test text set/get roundtrip.
    backend.set_widget_text(window, "updated");
    assert_eq!(backend.get_widget_text(window), "updated", "Widget text should update");

    // Test geometry update.
    backend.set_widget_geometry(window, 20, 20, 100, 30);

    // Test IME (defaults to true in BackendState::WidgetRecord).
    assert!(backend.is_widget_ime_enabled(window), "IME should be enabled by default");
    assert!(backend.set_widget_ime_enabled(window, false), "set_widget_ime_enabled should succeed");
    assert!(
        !backend.is_widget_ime_enabled(window),
        "IME should be disabled after set_widget_ime_enabled(false)"
    );
}

#[test]
fn clipboard_roundtrip() {
    let backend = HarmonyPlatform::new();
    backend.init();

    // Clipboard should start empty.
    assert_eq!(backend.get_clipboard_text(), "", "Clipboard should be empty initially");

    // Set and get roundtrip.
    assert!(backend.set_clipboard_text("hello harmony"), "Should set clipboard text");
    assert_eq!(backend.get_clipboard_text(), "hello harmony", "Clipboard text should match");

    // Overwrite with new value.
    assert!(backend.set_clipboard_text("updated clipboard"), "Should overwrite clipboard text");
    assert_eq!(
        backend.get_clipboard_text(),
        "updated clipboard",
        "Clipboard text should reflect update"
    );

    // Set empty string.
    assert!(backend.set_clipboard_text(""), "Should set empty clipboard text");
    assert_eq!(
        backend.get_clipboard_text(),
        "",
        "Clipboard should be empty after setting empty string"
    );
}

#[test]
fn widget_trigger_events() {
    let backend = HarmonyPlatform::new();
    backend.init();
    let window = backend.create_window("w", 0, 0, 200, 120);

    // Queue should be empty initially.
    assert!(backend.poll_widget_trigger_event().is_none());

    // Inject a clicked event.
    assert!(backend.inject_widget_trigger_event(window, WidgetTriggerKind::Clicked));

    // Poll the injected event.
    let event = backend.poll_widget_trigger_event();
    assert!(event.is_some(), "Should poll a trigger event");
    let event = event.unwrap();
    assert_eq!(event.widget_id, window);
    assert_eq!(event.kind, WidgetTriggerKind::Clicked);

    // No more events.
    assert!(backend.poll_widget_trigger_event().is_none());

    // Inject a value-changed event.
    assert!(backend.inject_widget_trigger_event(window, WidgetTriggerKind::ValueChanged));
    let event = backend.poll_widget_trigger_event();
    assert!(event.is_some(), "Should poll value-changed event");
    let event = event.unwrap();
    assert_eq!(event.widget_id, window);
    assert_eq!(event.kind, WidgetTriggerKind::ValueChanged);
}

#[test]
fn drag_and_drop() {
    let backend = HarmonyPlatform::new();
    backend.init();
    let window = backend.create_window("w", 0, 0, 200, 120);

    // Drag and drop: begin_drag succeeds because the window is a valid widget.
    assert!(
        backend.begin_drag(window, "text/plain", b"payload"),
        "begin_drag should succeed for a valid widget"
    );
    assert!(
        backend.poll_drop_event().is_some(),
        "poll_drop_event should return Some after begin_drag"
    );
    assert!(
        backend.inject_drop_event(crate::platform::DropEvent {
            source_widget_id: window,
            target_widget_id: window,
            mime: "text/plain".into(),
            payload: vec![]
        }),
        "inject_drop_event should succeed for valid target widget"
    );
}

/// Teardown must report whether the widget existed, and a second call must not
/// claim success for an id that is already gone.
#[test]
fn destroy_widget_reports_existence() {
    let backend = HarmonyPlatform::new();
    backend.init();
    let window = backend.create_window("w", 0, 0, 200, 120);
    assert!(backend.destroy_widget(window));
    assert!(!backend.destroy_widget(window));
    assert!(!backend.destroy_widget(4242));
}

/// The Harmony backend is state-only, so it must not advertise a native menu or
/// inherit desktop defaults that would overstate its capabilities.
#[test]
fn capabilities_are_explicit_and_honest() {
    let backend = HarmonyPlatform::new();
    let caps = backend.capabilities();

    assert_eq!(backend.family(), crate::core::PlatformFamily::Desktop);
    assert!(caps.dpi_scaling, "DPI scaling is tracked from the host");
    assert!(caps.ime, "IME state is modelled");
    assert!(caps.accessibility, "a11y metadata is modelled");
    assert!(!caps.native_menu, "the Harmony menu is an in-process tree, not an OS menu");
    assert!(caps.typed_widget_trigger, "typed trigger events are supported");
}

/// The surface contract: the backend hosts library-painted widgets and reports it.
///
/// This test used to assert the *opposite* (`supports_surfaces()` was `false` because
/// `mount_surface` was unimplemented). The surface is now a record plus a repaint
/// queue the ArkTS host drains, so the contract is the one below — and each step is
/// asserted, so a regression to "claims support but does nothing" is caught rather
/// than passing on the strength of a boolean.
#[test]
fn widget_surfaces_are_advertised_and_round_trip() {
    let backend = HarmonyPlatform::new();
    assert!(
        backend.supports_surfaces(),
        "the backend records surfaces and queues repaints, so it can host widgets"
    );

    let window = backend.create_window("w", 0, 0, 640, 480);
    let rect = crate::core::Rect::new(0, 0, 100, 40);
    assert!(backend.mount_surface(window, window, rect));
    assert_eq!(backend.state.surface_rect(window), Some(rect));
    assert_eq!(backend.state.mounted_surface_count(), 1);

    // Invalidating queues exactly one repaint, which the host then drains.
    assert!(backend.invalidate_surface(window));
    assert!(backend.invalidate_surface(window), "a second invalidate still reports the mount");
    assert_eq!(backend.state.pending_repaint_count(), 1, "repaints are coalesced");
    assert_eq!(backend.state.take_pending_repaint(), Some(window));
    assert_eq!(backend.state.pending_repaint_count(), 0);

    // Resizing moves the recorded rect.
    let moved = crate::core::Rect::new(10, 10, 200, 80);
    assert!(backend.resize_surface(window, moved));
    assert_eq!(backend.state.surface_rect(window), Some(moved));

    // Unmounting forgets it, and a stale repaint request cannot survive the unmount.
    assert!(backend.invalidate_surface(window));
    assert!(backend.unmount_surface(window));
    assert_eq!(backend.state.surface_rect(window), None);
    assert_eq!(backend.state.pending_repaint_count(), 0, "a gone widget must not be repainted");
    assert!(!backend.unmount_surface(window), "unmounting twice must report no-op");
}

/// A surface for a widget the backend never made must be refused, not silently
/// recorded: a frame nobody can produce is not a display.
#[test]
fn mounting_a_surface_for_an_unknown_widget_is_refused() {
    let backend = HarmonyPlatform::new();
    assert!(!backend.mount_surface(1, 9_999, crate::core::Rect::new(0, 0, 10, 10)));
    assert!(!backend.invalidate_surface(9_999));
    assert!(!backend.resize_surface(9_999, crate::core::Rect::new(0, 0, 10, 10)));
    assert_eq!(backend.state.mounted_surface_count(), 0);
}
