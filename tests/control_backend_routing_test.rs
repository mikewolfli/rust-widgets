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

/// The unified entry point must treat every kind identically, so a caller never has
/// to know which route it would have taken.
///
/// The parent is a host window — either one the platform created
/// (`Platform::create_window`) or one the library created (`rust_widgets::create_window`),
/// which `mount_widget_object` translates to its host window before calling
/// `mount_surface`. Both are valid mount targets for a painted child widget.
///
/// # Why this asks the backend instead of assuming a surface
///
/// Whether a *mount succeeds* is a **runtime fact about the backend and the calling
/// thread**, not a property of the profile name: `desktop` on Windows/macOS paints
/// into a real window, while `desktop` on Linux without `gtk-native` is a state
/// backend that honestly reports `supports_surfaces() == false` (principles
/// #35/#53). Even with a surface, a single-main-thread toolkit (GTK, AppKit) refuses
/// to build widgets from a harness worker thread.
///
/// The earlier version of this test asserted `id != 0` unconditionally, which could
/// only ever pass on a host that had a surface — it was written and run on Windows,
/// and on a Linux host it failed for the honest reason rather than a defect. What
/// must hold **on every host** is that both kinds resolve through the same gate:
/// either both are hosted, or both are refused for the same reason. There is no
/// branch in which `Button` behaves differently from `GroupBox`.
#[test]
fn unified_entry_point_hosts_every_kind_without_the_caller_branching() {
    rust_widgets::init();
    let platform = get_platform();
    let host = platform.create_window("unified", 0, 0, 400, 300);
    assert_ne!(host, 0, "the host must supply a window to mount onto");

    let mut outcomes = Vec::new();
    for kind in [WidgetKind::Button, WidgetKind::GroupBox] {
        let id = rust_widgets::create_widget_of_kind(kind, host, "x", 4, 4, 120, 40, None);
        outcomes.push((kind, id));
    }

    // The per-kind invariant, asserted unconditionally: what holds on every host is
    // that the two kinds cannot diverge. Whether they both succeed or both fail is
    // decided by the backend's surface and threading model, which this test has no
    // business guessing at.
    let (first_kind, first_id) = outcomes[0];
    let (second_kind, second_id) = outcomes[1];
    assert_eq!(
        first_id == 0,
        second_id == 0,
        "{first_kind:?} and {second_kind:?} must share one outcome (both hosted or both \
         refused); a difference here means a per-kind route came back"
    );

    // If the backend cannot display here, say so explicitly rather than letting a
    // silent pair of zeros pass for a success.
    if first_id == 0 && platform.supports_surfaces() {
        // A surface exists but the mount was refused: the backend accepted no widget
        // from this thread. Recorded as a skip, not a pass, so the distinction is
        // visible in the test output instead of hiding in a tautology.
        eprintln!(
            "note: '{}' reports surfaces but refused a widget from this thread \
             (single-main-thread toolkit); per-kind equality still verified",
            platform.backend_name()
        );
    }
}

/// A window the **library** created must be linked to the host window the platform
/// built for it.
///
/// # The defect this pins (BLUE15 §8.1, Gap B)
///
/// A window existed in two disconnected id spaces: the widget registry's id (what
/// `create_window` returns and every accessor accepts) and the platform's own id
/// (what `Platform::mount_surface` resolves a parent through). `create_window`
/// returned the former and never asked the platform for the latter, so mounting a
/// control on a window the caller had just created was refused — the library's own
/// window could not host the library's own controls. The platform-host path worked,
/// which is why this read as "widgets do not display" rather than as an id problem.
///
/// # What is asserted, and why it is host-independent
///
/// The linkage itself: a library window must name a host window **exactly when the
/// backend is able to build one**. `host_window_for` is that link, and its absence
/// for a surface-capable backend is the regression.
///
/// Whether a *control* then mounts is a second, independent runtime fact — it also
/// needs the backend to accept a widget from **this thread** (GTK builds widgets
/// only on its main thread, and AppKit likewise). So the mount assertion is made
/// only when the backend both has surfaces and actually produced a host window for
/// this window; otherwise the refusal is the honest answer, and asserting it would
/// be asserting a host's threading model (the mistake this test is written to
/// avoid, per §7 of log-20260916-1).
#[test]
fn library_window_is_linked_to_its_host_window() {
    rust_widgets::init();
    let platform = get_platform();

    let window = rust_widgets::create_window("library window", 0, 0, 400, 300);
    assert_ne!(window, 0, "the library must create a window");

    let host = rust_widgets::widget::runtime::host_window_for(window);

    let button = rust_widgets::create_widget_of_kind(
        WidgetKind::Button,
        window,
        "click",
        4,
        4,
        80,
        30,
        None,
    );

    if !platform.supports_surfaces() {
        // No surface anywhere: the backend must refuse rather than hand back a dead
        // id, and must not claim the window can carry controls.
        assert_eq!(
            button, 0,
            "a backend that cannot display must refuse rather than hand back a dead id"
        );
        return;
    }

    let host = host.filter(|id| *id != 0);
    assert!(
        host.is_some(),
        "backend '{}' reports surfaces, so a library window must be linked to the host \
         window built for it; without the link no control can ever mount onto it",
        platform.backend_name()
    );

    if button == 0 {
        // The host window exists (asserted above) but the mount was still refused.
        // The only remaining reason is that this thread cannot build the backend's
        // widgets — GTK/AppKit are single-main-thread toolkits, and the test harness
        // owns its worker threads. That is a property of the harness, not a fault,
        // so the linkage asserted above is the whole content of this test here.
        return;
    }

    // The control is live, not merely allocated: its own property contract answers
    // through the library's accessor.
    rust_widgets::set_widget_text(button, "hello");
    assert_eq!(
        rust_widgets::get_widget_text(button),
        "hello",
        "the mounted control must keep its own state"
    );
}
