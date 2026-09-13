// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Guard: widget creation must go through the control-backend router.
//!
//! # What went wrong
//!
//! `src/control_backend/` carried a complete, documented native-vs-custom routing
//! design — `ControlBackend` (200+ methods), `NativeControlBackend`,
//! `CustomPaintControlBackend`, and `get_control_backend_for_widget(kind)` whose
//! doc comment calls itself "the canonical **create-time** selection entry" — yet
//! every `create_*` in `lib.rs` called `platform::get_platform()` directly. The
//! router had **zero callers**, so the documented policy could not take effect and
//! the two halves could silently drift.
//!
//! These tests pin the wiring from the consumer side: the resolver is reachable
//! and creation resolves through it.

#![cfg(all(
    any(feature = "desktop", feature = "tablet", feature = "mobile"),
    not(any(feature = "mini", feature = "embedded"))
))]

use rust_widgets::control_backend::{
    get_control_backend_for_widget, route_preference_for_widget_kind, ControlBackendKind,
    ControlRoutePreference,
};
use rust_widgets::widget::WidgetKind;

/// The per-kind resolver must be the single source of truth: whichever kind is
/// asked, the returned backend's `kind()` must agree with that kind's routing
/// preference. Drift here is exactly the defect this pins.
#[test]
fn per_kind_resolver_agrees_with_its_routing_preference_for_both_routes() {
    let cases = [
        (WidgetKind::Button, ControlRoutePreference::NativePreferred, ControlBackendKind::Native),
        (WidgetKind::GroupBox, ControlRoutePreference::CustomRequired, ControlBackendKind::Custom),
    ];

    for (kind, expected_pref, expected_backend) in cases {
        assert_eq!(
            route_preference_for_widget_kind(kind),
            expected_pref,
            "{kind:?} routing preference changed"
        );
        let backend = get_control_backend_for_widget(kind);
        assert_eq!(
            backend.kind(),
            expected_backend,
            "{kind:?} must resolve to the {expected_backend:?} backend, got {}",
            backend.backend_name()
        );
    }
}

/// A `CustomRequired` kind must reach a backend that owns real state for it —
/// proving the resolver is consulted for a case where native and custom genuinely
/// differ, rather than both paths accidentally behaving the same.
#[test]
fn custom_required_kind_reaches_the_custom_backend_state_model() {
    let backend = get_control_backend_for_widget(WidgetKind::GroupBox);
    assert_eq!(backend.kind(), ControlBackendKind::Custom);

    // The custom backend is a real state model, not a stub: it allocates ids and
    // stores text. This is the observable difference from the native path, whose
    // `create_group_box` would delegate to the platform.
    let id = backend.create_panel(0, 10, 10, 120, 80);
    assert_ne!(id, 0, "the custom backend must allocate a usable id");
    backend.set_widget_text(id, "hello");
    assert_eq!(backend.get_widget_text(id), "hello");
}

/// The router entry point must stay public: `lib.rs`'s creation functions call
/// it, and a refactor that made it private would break the wiring at the boundary
/// this test exercises.
#[test]
fn router_entry_point_is_public() {
    let backend: &'static dyn rust_widgets::control_backend::ControlBackend =
        get_control_backend_for_widget(WidgetKind::Label);
    assert!(!backend.backend_name().is_empty());
}

/// The unified entry point must create a widget for a primitive-mapped kind and
/// for a custom-required kind alike, so a caller never has to know which it was.
#[test]
fn unified_entry_point_creates_both_kinds_without_the_caller_branching() {
    rust_widgets::init();
    let win = rust_widgets::create_window("unified", 0, 0, 400, 300);

    // Primitive-mapped kind: no widget object needed.
    let button =
        rust_widgets::create_widget_of_kind(WidgetKind::Button, win, "ok", 4, 4, 80, 24, None);
    assert_ne!(button, 0, "a primitive-mapped kind must still create a widget");

    // Custom-required kind with nothing to host: must refuse honestly, not
    // fabricate an id.
    let refused =
        rust_widgets::create_widget_of_kind(WidgetKind::GroupBox, win, "box", 4, 40, 120, 80, None);
    assert_eq!(refused, 0, "a custom-required kind with no widget must not fabricate an id");
}
