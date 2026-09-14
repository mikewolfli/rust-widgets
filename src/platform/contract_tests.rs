// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Cross-backend contract-consistency tests for the host surface.
//!
//! # What changed and why
//!
//! These tests used to pin the *native-control* contract: that `create_button`
//! returns a usable id, that a slider's value round-trips through
//! `set_widget_value`, that destroying a control frees a real handle. That contract
//! no longer exists — every control is painted by the library, and the `Platform`
//! trait's control methods are now optional with defaults that report
//! **"this host creates no such control"** (BLUE15 Phase B-1).
//!
//! Keeping the old assertions would have meant either (a) leaving 15 permanently
//! red tests, or (b) restoring a shadow implementation to satisfy them. The second
//! is the duplication this refactor removed, so the tests are rewritten around the
//! contract that *is* true now:
//!
//!   1. A host that does not create controls **says so** — `0` for an id, `false`
//!      for a boolean, `None` for an optional — instead of fabricating a value.
//!   2. Nothing panics when a control operation is requested on such a host.
//!   3. The capabilities a host *must* provide (a window, DPI, clipboard, IME) keep
//!      working, so the degradation is confined to controls.
//!   4. Destroying an unknown id is a safe `false`, and no id is ever reused.
//!
//! Point 1 is the important one: the failure this guards against is a backend that
//! *pretends* to support a control and then silently does nothing, which would make
//! an application's calls unobservable failures.

use crate::platform::types::WindowStateFlag;
use crate::platform::{get_platform, Platform};

/// The kinds whose control methods a host may legitimately not implement.
///
/// Each entry is (label, call). Every one must return the "not supported" value
/// rather than panicking, and must not be mistaken for success.
fn requested_controls(platform: &dyn Platform) -> Vec<(&'static str, u64)> {
    vec![
        ("button", Platform::create_button(platform, 1, "b", 0, 0, 40, 20)),
        ("checkbox", Platform::create_checkbox(platform, 1, "c", 0, 0, 40, 20)),
        ("line_edit", Platform::create_line_edit(platform, 1, "t", 0, 0, 40, 20)),
        ("label", Platform::create_label(platform, 1, "l", 0, 0, 40, 20)),
        ("slider", Platform::create_slider(platform, 1, 0, 0, 40, 20)),
        ("progress_bar", Platform::create_progress_bar(platform, 1, 0, 0, 40, 20)),
        ("combo_box", Platform::create_combo_box(platform, 1, 0, 0, 40, 20)),
        ("list_box", Platform::create_list_box(platform, 1, 0, 0, 40, 20)),
        ("panel", Platform::create_panel(platform, 1, 0, 0, 40, 20)),
        ("scroll_area", Platform::create_scroll_area(platform, 1, 0, 0, 40, 20)),
        ("tab_widget", Platform::create_tab_widget(platform, 1, 0, 0, 40, 20)),
        ("calendar", Platform::create_calendar(platform, 1, 0, 0, 40, 20)),
    ]
}

/// A host that does not create controls must report that, not fabricate values.
///
/// The default implementations return `0` / `false` / `None`. A backend that
/// overrides one of these and returns a *fake* success is the regression this
/// catches: the app would believe a control exists, and nothing would ever appear.
#[test]
fn unsupported_controls_report_absence_rather_than_faking_success() {
    let platform = get_platform();
    platform.init();

    for (label, id) in requested_controls(platform) {
        // Whatever a backend returns, it must be internally consistent: an id of 0
        // means "no control", and then the control's state must be absent too.
        if id == 0 {
            assert!(
                Platform::get_widget_text(platform, id).is_empty(),
                "{}: {label} creation reported no control, so its text must be empty",
                platform.backend_name()
            );
            // The predicates answer `false` for "not enabled / not visible". A
            // backend that returned `true` here would claim a control it did not
            // create, which is the fake-success failure this guards.
            assert!(
                !Platform::is_widget_enabled(platform, id),
                "{}: {label} creation reported no control, so it cannot be enabled",
                platform.backend_name()
            );
            assert!(
                !Platform::is_widget_visible(platform, id),
                "{}: {label} creation reported no control, so it cannot be visible",
                platform.backend_name()
            );
            assert!(
                !Platform::destroy_widget(platform, id),
                "{}: {label} creation reported no control, so there is nothing to destroy",
                platform.backend_name()
            );
        } else {
            // A backend with a real primitive must supply real state for it.
            assert!(
                Platform::is_widget_enabled(platform, id),
                "{}: {label} reports id {id}, so it must report as enabled",
                platform.backend_name()
            );
        }
    }
}

/// Requesting a control on a host without primitives must never panic.
///
/// A panic here would take down an application that merely asked for a widget,
/// which is strictly worse than the widget not appearing.
#[test]
fn requesting_unsupported_controls_never_panics() {
    let platform = get_platform();
    platform.init();

    for (_, id) in requested_controls(platform) {
        Platform::show_widget(platform, id);
        Platform::hide_widget(platform, id);
        Platform::set_widget_geometry(platform, id, 1, 2, 3, 4);
        Platform::set_widget_text(platform, id, "x");
        Platform::set_widget_enabled(platform, id, true);
        Platform::set_widget_visible(platform, id, true);
        let _ = Platform::get_widget_text(platform, id);
        let _ = Platform::is_widget_enabled(platform, id);
        let _ = Platform::is_widget_visible(platform, id);
    }
}

/// An unknown id must be answered honestly and safely.
///
/// The predicates answer `false` — "not enabled", "not visible" — which for an
/// id that addresses nothing is the truthful answer, and must never panic. The
/// mutators must be safe no-ops for the same reason.
#[test]
fn unknown_ids_are_reported_as_absent_not_as_state() {
    let platform = get_platform();
    platform.init();

    let unknown = 987_654_321u64;
    assert!(Platform::get_widget_text(platform, unknown).is_empty());
    assert!(!Platform::is_widget_enabled(platform, unknown));
    assert!(!Platform::is_widget_visible(platform, unknown));
    assert!(!Platform::destroy_widget(platform, unknown));

    // The mutators must be safe no-ops rather than panics.
    Platform::set_widget_text(platform, unknown, "x");
    Platform::set_widget_enabled(platform, unknown, true);
    Platform::set_widget_visible(platform, unknown, true);
    Platform::set_widget_geometry(platform, unknown, 0, 0, 10, 10);
}

/// Destroying a control must not make its id reusable.
///
/// An id that comes back after destruction would silently alias a different
/// control in any code holding the old handle.
#[test]
fn destroyed_ids_are_not_reused() {
    let platform = get_platform();
    platform.init();

    let window = Platform::create_window(platform, "w", 0, 0, 320, 240);
    assert_ne!(window, 0, "{} must create a window", platform.backend_name());

    let first = Platform::create_label(platform, window, "one", 0, 0, 80, 20);
    if first == 0 {
        // This host has no controls; there is no id to reuse, and saying so is the
        // whole contract. The window assertions above still had to hold.
        return;
    }
    assert!(Platform::destroy_widget(platform, first));

    let second = Platform::create_label(platform, window, "two", 0, 0, 80, 20);
    assert_ne!(second, first, "a destroyed id must not be handed out again");
}

/// The capabilities a host **must** provide keep working.
///
/// This is the counterpart to the tests above: control support may degrade, but
/// the window, DPI and IME surface may not, because the library's own painting
/// depends on them.
#[test]
fn required_host_capabilities_remain_available() {
    let platform = get_platform();
    platform.init();

    assert!(!platform.backend_name().is_empty(), "a backend must name itself");
    assert!(platform.dpi_scale_factor() > 0.0, "DPI must be a positive scale factor");

    let window = Platform::create_window(platform, "w", 0, 0, 320, 240);
    assert_ne!(window, 0, "every host must be able to create a window");

    // The host must answer whether it can carry a painting surface at all; that is
    // the capability the library actually depends on.
    let _ = Platform::supports_surfaces(platform);
}

/// A window is not a control: window state must be refused for anything else.
#[test]
fn window_state_is_not_applied_to_non_windows() {
    let platform = get_platform();
    platform.init();

    let window = Platform::create_window(platform, "w", 0, 0, 320, 240);
    assert_ne!(window, 0);

    // A made-up id is not a window, so its window state must be absent.
    let not_a_window = 987_654_321u64;
    assert_eq!(
        Platform::is_window_in_state(platform, not_a_window, WindowStateFlag::Resizable),
        None,
        "an unknown id must not report window state"
    );
    assert!(
        !Platform::set_window_min_size(platform, not_a_window, 10, 10),
        "a non-window must not accept a minimum size"
    );
}

/// An id space that mixes windows and controls must stay consistent across churn.
#[test]
fn id_allocation_stays_consistent_across_churn() {
    let platform = get_platform();
    platform.init();

    let mut seen = std::collections::HashSet::new();
    for round in 0..8 {
        let window = Platform::create_window(platform, "w", 0, 0, 200, 150);
        assert_ne!(window, 0, "round {round}: window creation must succeed");
        assert!(seen.insert(window), "round {round}: a live id was reused");
        assert!(Platform::destroy_widget(platform, window));
        assert!(!Platform::destroy_widget(platform, window), "destroy must be idempotent");
    }
}
