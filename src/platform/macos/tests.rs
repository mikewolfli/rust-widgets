// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! macOS platform tests.
//!
//! Scope note (BLUE15 #55/#56): this backend creates **no** native controls and no
//! native menus, so there is nothing to assert about control construction. The
//! library paints every `WidgetKind` itself, so the `Platform` trait's defaults
//! honestly report "this host holds no such control" and the widget-property
//! methods answer `false` / `None`. What remains here is what this backend still
//! owns: the window contract, the platform facts it publishes, and the macOS
//! config-directory convention.

#![allow(deprecated)] // Cocoa 0.24 fallback; remove when objc2 backend fully replaces cocoa

use crate::platform::macos::MacOSPlatform;
use crate::platform::Platform;

/// A window must be creatable off the AppKit main thread.
///
/// This is the load-bearing contract of the legacy backend's state-only fallback:
/// constructing an `NSWindow` off-main raises a foreign exception that aborts the
/// process, so `create_window` must instead register a state-only handle and still
/// report a usable id. Unit tests run on worker threads, which is exactly the
/// condition that exercises this path.
#[test]
fn macos_create_window_succeeds_off_main_thread() {
    let platform = MacOSPlatform::new();
    assert!(!crate::platform::macos::types::is_main_thread(), "test harness runs off-main");

    let window = platform.create_window("Off-main", 10, 20, 320, 240);
    assert_ne!(window, 0, "create_window must return a usable id off-main");

    let handle = platform.get_handle(window).expect("handle must be registered");
    assert_eq!(handle.ptr, 0, "no native NSWindow may be constructed off the main thread");
}

/// The backend must not claim native controls it no longer creates.
///
/// This pins the honest-reporting contract: with every control `create_*`
/// override gone, a control constructor falls back to the trait default and
/// returns `0`, and the state query that would accompany it reports absence.
/// A future change that resurrects a native control must come with an override
/// and update this expectation deliberately.
#[test]
fn macos_reports_no_native_controls() {
    let platform = MacOSPlatform::new();
    let window = platform.create_window("W", 0, 0, 320, 240);

    assert_eq!(
        platform.create_button(window, "ok", 0, 0, 80, 24),
        0,
        "this host creates no native button"
    );
    assert_eq!(platform.create_checkbox(window, "on", 0, 0, 80, 24), 0);
    assert_eq!(platform.create_label(window, "text", 0, 0, 80, 24), 0);
    assert_eq!(platform.widget_value(window), None, "a window has no numeric value to report");
}

/// Window records must answer the same window queries on every host backend.
///
/// The window is the one primitive a backend still owns, so a regression here
/// would silently break minimise/maximise handling for the whole library. The
/// off-main path registers a state-only handle; the window state must still
/// round-trip through it.
#[test]
fn macos_window_state_round_trips_without_a_native_handle() {
    use crate::platform::WindowStateFlag;

    let platform = MacOSPlatform::new();
    let window = platform.create_window("Stateful", 0, 0, 320, 240);

    assert!(platform.set_window_state(window, WindowStateFlag::Maximized, true));
    assert_eq!(platform.is_window_in_state(window, WindowStateFlag::Maximized), Some(true));
    assert!(platform.set_window_state(window, WindowStateFlag::Maximized, false));
    assert_eq!(platform.is_window_in_state(window, WindowStateFlag::Maximized), Some(false));

    // A non-window must refuse window state rather than invent a record.
    assert!(!platform.set_window_state(window + 1000, WindowStateFlag::Maximized, true));
    assert_eq!(platform.is_window_in_state(window + 1000, WindowStateFlag::Maximized), None);
}

/// Text and geometry recorded at creation must be readable straight back.
///
/// Both creation paths (host-allocated window, library-painted widget adopted via
/// `register_widget`) share one record, so the accessors have to serve both.
#[test]
fn macos_widget_text_and_geometry_round_trip() {
    let platform = MacOSPlatform::new();
    let window = platform.create_window("Original", 10, 20, 320, 240);
    assert_eq!(platform.get_widget_text(window), "Original");

    platform.set_widget_text(window, "Renamed");
    assert_eq!(platform.get_widget_text(window), "Renamed");

    platform.set_widget_geometry(window, 5, 6, 100, 50);
    assert!(platform.is_widget_visible(window));
    assert!(platform.is_widget_enabled(window));
    platform.set_widget_enabled(window, false);
    assert!(!platform.is_widget_enabled(window));
}

/// Destruction must release the record it created.
///
/// A backend registry that only grows leaks one entry per discarded widget in a
/// UI-rebuild loop. The macOS objc2 backend measured +49 MB RSS over 8000
/// create/destroy cycles before `destroy_widget` existed.
#[test]
fn macos_destroy_widget_releases_the_record() {
    let platform = MacOSPlatform::new();
    let window = platform.create_window("Temp", 0, 0, 100, 100);
    assert!(platform.destroy_widget(window));
    assert!(!platform.destroy_widget(window), "second destroy must report absence");
    assert_eq!(platform.get_widget_text(window), "");
}

/// macOS must place the config directory under `Library/Application Support`.
///
/// This assertion used to live in `menu_config::tests`, which put a `cfg(target_os = "macos")`
/// inside a UI-config module (principle #68). The expectation is an OS
/// convention, so it belongs to the backend that owns the OS knowledge.
///
/// The regression it guards is real: an earlier revision hand-built
/// `home.join(".config")`, which on macOS wrote settings to the wrong place.
///
/// Gated on `advanced-widgets`: the assertion calls `dirs::config_dir()`, and
/// `dirs` is an optional dependency enabled only by that feature. `macos-legacy`
/// alone does not pull it in, so without this gate the test compiled against an
/// absent crate. The check is worth keeping where it can run rather than being
/// rewritten to avoid the very API the production path uses.
#[cfg(feature = "advanced-widgets")]
#[test]
fn config_dir_uses_application_support_not_xdg() {
    let Some(base) = dirs::config_dir() else {
        return; // No home directory (unusual CI): nothing to assert.
    };
    let text = base.to_string_lossy();

    assert!(
        text.contains("Library/Application Support"),
        "macOS config dir must use Application Support, got {text}"
    );
    assert!(!text.contains("/.config/"), "macOS must not use the Linux XDG path: {text}");
}
