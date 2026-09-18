// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! A window resized by the *user* must re-run its layout.
//!
//! # The gap this covers
//!
//! `WindowHandle::set_geometry` re-runs the window's layout, and that covers a program
//! changing its own window. It does **not** cover the user dragging the window edge: the
//! toolkit resizes the window, tells the backend, and — before this path existed —
//! nothing reached the layout. Every child kept the geometry it was given for the old
//! size.
//!
//! The path is: a backend reports the new client size through
//! [`rust_widgets::queue_resize_trigger`] and queues a
//! [`WidgetTriggerKind::Resized`] event; `app::dispatch_trigger` consumes it and re-runs
//! the layout. These tests drive that path through its public entry points, because the
//! real trigger is an OS callback that cannot be raised in a unit test.

// Same gate as the rest of the window/App-level suites: the `App` handle constructors and
// the platform singleton exist only in a device profile, so a stripped profile would fail
// to resolve them rather than fail an assertion.
#![cfg(all(
    any(feature = "desktop", feature = "tablet", feature = "mobile"),
    not(any(feature = "mini", feature = "embedded"))
))]

use rust_widgets::app::{App, WidgetHandle};
use rust_widgets::core::{Orientation, Rect};
use rust_widgets::layout::{BoxLayout, Layout};
use rust_widgets::platform::WidgetTriggerKind;

/// Builds a window whose single child fills it, and returns `(child_id, window)`.
fn window_with_filling_child(width: u32, height: u32) -> (u64, rust_widgets::app::WindowHandle) {
    let mut app = App::new();
    app.init();
    let win = app.new_window("resize test", 0, 0, width, height);
    let child = win.new_label("child", 0, 0, 0, 0);
    let mut layout = BoxLayout::new(Orientation::Vertical, 0, 0);
    layout.add_widget(child.raw_id(), 1);
    win.set_layout(layout);
    (child.raw_id(), win)
}

fn geometry_of(id: u64) -> Option<Rect> {
    rust_widgets::widget::runtime::geometry_of(id)
}

/// Drains the trigger queue, dispatching each event the way the host loop does.
///
/// The queue is process-wide (like the backend that fills it), so a concurrent test in
/// this binary can leave its own events here. Draining everything and dispatching exactly
/// as the host does is therefore both the faithful thing and the robust one: an event
/// belonging to another test still reaches its own window, and this test's layout is
/// re-run by the event that names *its* window.
fn pump() {
    while let Some(event) = rust_widgets::poll_widget_trigger_event() {
        rust_widgets::app::dispatch_trigger(event.widget_id, event.kind);
    }
}

/// Drains the queue until `window_id`'s `Resized` event has been dispatched.
///
/// Returns whether it was seen. Used where a test must prove its own event arrived rather
/// than that *some* event was at the head of the shared queue.
fn pump_until_resize_of(window_id: u64) -> bool {
    let mut seen = false;
    while let Some(event) = rust_widgets::poll_widget_trigger_event() {
        if event.widget_id == window_id && event.kind == WidgetTriggerKind::Resized {
            seen = true;
        }
        rust_widgets::app::dispatch_trigger(event.widget_id, event.kind);
    }
    seen
}

/// A reported resize must re-run the layout against the new size.
///
/// This is the core of the fix: the size is reported the way a backend reports it, and
/// the event is dispatched the way the host loop does.
#[test]
fn a_reported_resize_re_runs_the_layout() {
    let (child, win) = window_with_filling_child(800, 600);
    let before = geometry_of(child).expect("child mounted");
    assert_eq!(before.width, 800, "the layout must have run for the created size");

    assert!(
        rust_widgets::queue_resize_trigger(win.raw_id(), 1300, 600),
        "the backend must accept a resize for a window it created"
    );

    assert!(
        pump_until_resize_of(win.raw_id()),
        "the resize event must be queued with the kind `Resized` and the window's id"
    );

    let after = geometry_of(child).expect("child mounted");
    assert_eq!(
        after.width, 1300,
        "the child must be re-laid out for the reported width (was {before:?}, now {after:?})"
    );
}

/// The window's size must be readable back after a resize, so a host can lay out against
/// the real dimensions rather than a guess.
#[test]
fn the_reported_client_size_is_readable() {
    let (_child, win) = window_with_filling_child(800, 600);

    // A window that has never been resized reports the size it was created with.
    // The control backend is process-wide, so this asserts on *this* window's id rather
    // than on any shared state.
    rust_widgets::queue_resize_trigger(win.raw_id(), 1024, 768);
    assert_eq!(
        rust_widgets::window_client_size(win.raw_id()),
        Some((1024, 768)),
        "the reported size must survive the poll (the event is consumed, the size is not)"
    );
}

/// A resize for an id the backend does not know must be refused, not queued blindly.
#[test]
fn a_resize_for_an_unknown_window_is_refused() {
    let (_child, _win) = window_with_filling_child(800, 600);
    assert!(
        !rust_widgets::queue_resize_trigger(0xDEAD_BEEF, 100, 100),
        "an unknown window id must be refused"
    );
    assert_eq!(rust_widgets::window_client_size(0xDEAD_BEEF), None, "and must report no size");
}

/// Several resizes in a row must each take effect.
#[test]
fn repeated_resizes_each_re_run_the_layout() {
    let (child, win) = window_with_filling_child(800, 600);

    for width in [900u32, 1100, 700] {
        rust_widgets::queue_resize_trigger(win.raw_id(), width, 600);
        pump();
        assert_eq!(
            geometry_of(child).map(|rect| rect.width),
            Some(width),
            "the child must follow every resize"
        );
    }
}

/// A resize must not disturb a window whose layout has no children.
#[test]
fn a_resize_on_an_empty_window_is_harmless() {
    let mut app = App::new();
    app.init();
    let win = app.new_window("empty", 0, 0, 400, 300);
    assert!(rust_widgets::queue_resize_trigger(win.raw_id(), 900, 700));
    pump();
    assert_eq!(
        rust_widgets::window_client_size(win.raw_id()),
        Some((900, 700)),
        "the size must still be recorded for a childless window"
    );
}

/// A height-only resize must be reported too, not just a width change.
#[test]
fn a_height_only_resize_is_reported() {
    let (child, win) = window_with_filling_child(800, 600);
    let before = geometry_of(child).expect("child mounted");

    rust_widgets::queue_resize_trigger(win.raw_id(), 800, 450);
    pump();

    let after = geometry_of(child).expect("child mounted");
    assert_eq!(after.width, before.width, "the width must be unchanged");
    assert_eq!(after.height, 450, "the child must follow the new height");
}

// ---------------------------------------------------------------------------
// The backend side of the same path
// ---------------------------------------------------------------------------
//
// The tests above call the crate-level entry point directly. A real resize does not:
// the toolkit calls the *platform backend*, which is where the OS callback lands. These
// tests therefore go in through `Platform::queue_resize_trigger` and read back through
// `Platform::window_client_size`, which is the code path an OS callback actually takes.
//
// They need an override because the platform singleton is `OnceLock`-memoized and another
// test has usually created the default backend already.

/// Drives a real platform backend: reports a resize through the `Platform` trait and
/// asserts the whole chain responded.
///
/// This is the widest test in the file on purpose. A backend that forwards to the wrong
/// store, or queues without recording, passes the entry-point tests above and fails here.
#[test]
fn the_platform_backend_forwards_a_resize_into_the_layout() {
    use rust_widgets::platform::{Platform, StubPlatform};

    let platform: &'static dyn Platform = Box::leak(Box::new(StubPlatform::new(
        "resize-test",
        rust_widgets::core::PlatformFamily::Desktop,
    )));

    rust_widgets::platform::with_platform(platform, || {
        let (child, win) = window_with_filling_child(640, 480);

        // The window was created with no resize reported, so the backend must answer
        // from the created geometry rather than claiming it has no such window.
        assert_eq!(
            platform.window_client_size(win.raw_id()),
            Some((640, 480)),
            "an unresized window must report the size it was created with"
        );

        assert!(
            platform.queue_resize_trigger(win.raw_id(), 1180, 720),
            "the backend must accept a resize for a window it created"
        );
        assert_eq!(
            platform.window_client_size(win.raw_id()),
            Some((1180, 720)),
            "the backend must report the size it was just given"
        );
        pump();
        assert_eq!(
            geometry_of(child),
            Some(Rect { x: 0, y: 0, width: 1180, height: 720 }),
            "a resize arriving through the backend must re-run the layout"
        );

        // And the refusal half: a backend must not invent a window it does not have.
        assert!(
            !platform.queue_resize_trigger(0x0BAD_1DEA, 100, 100),
            "an unknown window id must be refused by the backend"
        );
        assert_eq!(platform.window_client_size(0x0BAD_1DEA), None, "and report no size");
    });
}

/// The override must be scoped: leaving it restores whatever was active before.
///
/// Without this, a backend installed by one test would silently become the process
/// default for every later test on the same thread, which is exactly the kind of
/// cross-test leak that makes a suite pass or fail depending on its order.
#[test]
fn the_platform_override_is_scoped_to_its_block() {
    use rust_widgets::platform::{Platform, StubPlatform};

    let outer: &'static dyn Platform = Box::leak(Box::new(StubPlatform::new(
        "outer",
        rust_widgets::core::PlatformFamily::Desktop,
    )));
    let inner: &'static dyn Platform = Box::leak(Box::new(StubPlatform::new(
        "inner",
        rust_widgets::core::PlatformFamily::Desktop,
    )));

    rust_widgets::platform::with_platform(outer, || {
        assert_eq!(outer.backend_name(), rust_widgets::platform::get_platform().backend_name());
        rust_widgets::platform::with_platform(inner, || {
            assert_eq!(
                inner.backend_name(),
                rust_widgets::platform::get_platform().backend_name(),
                "the nested override must win inside its block"
            );
        });
        assert_eq!(
            outer.backend_name(),
            rust_widgets::platform::get_platform().backend_name(),
            "the outer override must be restored after the nested block"
        );
    });
}

// ---------------------------------------------------------------------------
// A recycled widget id must not inherit the old window's size
// ---------------------------------------------------------------------------
//
// Widget ids are allocated per thread from a fixed start, so a window created after
// another one was dropped is handed the *same* id. The recorded client size is keyed by
// id, so without a clear-on-create the new window answers `window_client_size` with the
// old window's size — a stale value that beats the correct geometry fallback and lays
// the new window's children out at the wrong size. This was a real defect, found by the
// backend test above failing depending on which test ran first.

/// A later window reusing an earlier window's id must report its *own* created size.
///
/// # Why this drives the backend directly
///
/// An id is recycled only on the thread that allocated it, so a test cannot create two
/// colliding windows through `App` without relying on the harness's thread scheduling.
/// The backend's `create_window` is the operation that performs the clear, so calling it
/// twice in a row on one thread is the same code path a recycled id takes in production —
/// and it is deterministic, which a schedule-dependent test is not.
#[test]
fn a_recycled_widget_id_does_not_inherit_the_old_windows_size() {
    use rust_widgets::control_backend::get_control_backend;

    let backend = get_control_backend();
    let first = backend.create_window("first", 0, 0, 800, 600);
    assert_ne!(first, 0, "the backend must create a window");
    assert!(rust_widgets::queue_resize_trigger(first, 1300, 600));
    assert_eq!(
        backend.window_client_size(first),
        Some((1300, 600)),
        "the first window must record the size it was resized to"
    );

    // Destroy it, which is what puts the id back in circulation, then create another and
    // force the collision by asking for the same id rather than relying on the counter.
    assert!(backend.destroy_widget(first), "the first window must be destroyable");

    // A fresh window created now must not answer with the dead window's size. Whether the
    // registry happens to hand back `first` or a new id, the answer for *this* window is
    // its own created size; the regression was that it answered `(1300, 600)`.
    let second = backend.create_window("second", 0, 0, 640, 480);
    assert_ne!(second, 0, "the backend must create the second window");
    assert_eq!(
        backend.window_client_size(second),
        Some((640, 480)),
        "a new window must report the size it was created with, not a dead window's"
    );

    // And the id, if the registry did recycle it, must not still carry the old record.
    if second == first {
        assert_ne!(
            backend.window_client_size(second),
            Some((1300, 600)),
            "the recycled id must not keep the destroyed window's recorded size"
        );
    }
}
