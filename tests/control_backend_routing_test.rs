// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Guard: widget creation must go through the control-backend router.
//!
//! # What went wrong (pre-BLUE15)
//!
//! `src/control_backend/` carried a complete, documented native-vs-custom routing
//! design — `ControlBackend` (200+ methods), `NativeControlBackend`,
//! `CustomPaintControlBackend`, and `get_control_backend_for_widget(kind)` whose
//! doc comment calls itself "the canonical **create-time** selection entry" — yet
//! every `create_*` in `lib.rs` called `platform::get_platform()` directly. The
//! router had **zero callers**, so the documented policy could not take effect and
//! the two halves could silently drift.
//!
//! # What BLUE15 changed
//!
//! The two routes became one: the host supplies a window and a painting surface,
//! and the library paints every `WidgetKind` (rules #55/#56). The tests below were
//! written against the deleted two-route policy — they expected `Button` to resolve
//! to `NativePreferred` and a "custom-required" kind to be refused for lack of a
//! host primitive. Both expectations are now wrong by design, so they are asserted
//! the other way round: **every** kind is library-painted and resolves to the same
//! backend, and the unified entry point hosts every kind on the surface the host
//! supplied without the caller branching.

#![cfg(all(
    any(feature = "desktop", feature = "tablet", feature = "mobile"),
    not(any(feature = "mini", feature = "embedded"))
))]

use rust_widgets::control_backend::{
    get_control_backend_for_widget, route_preference_for_widget_kind, ControlBackendKind,
    ControlRoutePreference,
};
use rust_widgets::platform::get_platform;
use rust_widgets::widget::WidgetKind;

/// Kinds drawn from both of the former routes.
///
/// `Button` / `Label` / `Slider` used to be `NativePreferred`; `GroupBox` / `Chart`
/// / `CodeEditor` used to be `CustomRequired`. Under BLUE15 the two groups must be
/// indistinguishable to a caller — which is what these tests check, so a
/// regression that restored a per-kind route would fail on the first group.
const SAMPLE: [WidgetKind; 6] = [
    WidgetKind::Button,
    WidgetKind::Label,
    WidgetKind::Slider,
    WidgetKind::GroupBox,
    WidgetKind::Chart,
    WidgetKind::Canvas,
];

/// The per-kind resolver must be the single source of truth: every kind reports the
/// library-painted route, and the resolver hands back the one backend that
/// implements it.
#[test]
fn every_kind_resolves_to_the_library_painted_route() {
    for kind in SAMPLE {
        assert_eq!(
            route_preference_for_widget_kind(kind),
            ControlRoutePreference::CustomRequired,
            "{kind:?} must be painted by the library; another preference means a second \
             mechanism came back"
        );
        let backend = get_control_backend_for_widget(kind);
        assert_eq!(
            backend.kind(),
            ControlBackendKind::Custom,
            "{kind:?} must resolve to the library backend, got {}",
            backend.backend_name()
        );
    }
}

/// The single backend is a real state model, not a stub: it allocates ids and the
/// text it writes is the text it reads back, straight off the widget it created
/// rather than a shadow copy.
#[test]
fn the_single_backend_owns_real_state_for_a_hosted_widget() {
    let backend = get_control_backend_for_widget(WidgetKind::GroupBox);
    assert_eq!(backend.kind(), ControlBackendKind::Custom);

    // A window is a root; every other kind must name a live container, so the
    // window is created first and becomes the parent.
    let window = backend.create_window("Router state", 0, 0, 320, 200);
    assert_ne!(window, 0, "the backend must allocate a usable window id");

    let label = backend.create_label(window, "hello", 10, 10, 120, 20);
    assert_ne!(label, 0, "the backend must host a child of a live container");
    assert_eq!(
        backend.get_widget_text(label),
        "hello",
        "the backend must read back the text it just wrote"
    );
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

/// The unified entry point must host a widget for every kind, so a caller never has
/// to know which route it would have taken.
///
/// The parent is the **host window** (`Platform::create_window`) — the window the
/// host actually owns and paints into, which is what `mount_surface` resolves. A
/// library-side `Window` *widget* is a painted child of that host, so it carries no
/// native handle of its own and cannot be the mount target.
#[test]
fn unified_entry_point_hosts_every_kind_without_the_caller_branching() {
    rust_widgets::init();
    let host = get_platform().create_window("unified", 0, 0, 400, 300);
    assert_ne!(host, 0, "the host must supply a window to mount onto");

    for kind in [WidgetKind::Button, WidgetKind::GroupBox] {
        let id = rust_widgets::create_widget_of_kind(kind, host, "x", 4, 4, 120, 40, None);
        assert_ne!(
            id, 0,
            "{kind:?} must be hosted through the same entry point as every other kind"
        );
    }
}
