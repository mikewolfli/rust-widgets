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

/// The self-drawn contract must stay honest before the OpenHarmony SDK lands.
///
/// `mount_custom_widget` has no ArkUI Canvas mapping yet, so the backend must keep
/// reporting `false`. A host that checks `supports_custom_widgets()` then refuses to
/// build a UI it cannot display, instead of opening an empty window.
#[test]
fn custom_widget_support_is_refused_until_the_arkui_bridge_exists() {
    let backend = HarmonyPlatform::new();
    assert!(
        !backend.supports_custom_widgets(),
        "HarmonyOS cannot display self-drawn widgets until mount_custom_widget is \
         implemented against an ArkUI Canvas"
    );
    // The trait defaults must also refuse, rather than silently succeeding.
    assert!(!backend.mount_custom_widget(1, 2, crate::core::Rect::new(0, 0, 10, 10)));
    assert!(!backend.repaint_custom_widget(2));
    assert!(!backend.unmount_custom_widget(2));
}
