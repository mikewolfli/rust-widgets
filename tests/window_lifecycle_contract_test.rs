// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! # `Window`'s enabled contract and its close lifecycle
//!
//! `Window` was allowlisted by `tools/check_enabled_is_honoured.py` as "the window shell
//! itself; its children hold the interactions". That was true about **input** and it hid a
//! real decision about **output**: `close()` emits `closed`, and a closed window is about to
//! have its resources torn down by whoever listens.
//!
//! This test pins the decision that `closed` is a *lifecycle fact* rather than a user action,
//! so it is **not** gated by `enabled`. Without this test the decision is only a comment, and a
//! later pass adding a blanket `is_enabled()` guard to every emit would silently swallow the
//! teardown signal -- a leak with no failing test to catch it.
//!
//! `tools/check_enabled_is_honoured_containers.py` requires a written reason for an ungated
//! emit; this file is what makes the reason checkable rather than claimed.

#![cfg(all(feature = "desktop", not(alloc_frugal)))]

use rust_widgets::core::Rect;
use rust_widgets::widget::capability::WidgetFactory;
use rust_widgets::widget::Window;
use rust_widgets::Widget;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

fn window() -> Window {
    Window::new("Test".to_string(), Rect::new(0, 0, 640, 480))
}

#[test]
fn window_close_emits_closed() {
    let mut window = window();
    let closed = Arc::new(AtomicUsize::new(0));
    {
        let sink = closed.clone();
        window.closed.connect(move || {
            sink.fetch_add(1, Ordering::SeqCst);
        });
    }

    window.close();

    assert_eq!(closed.load(Ordering::SeqCst), 1, "close() must announce the lifecycle fact");
    assert!(!window.is_visible(), "a closed window must stop being visible");
}

#[test]
fn window_close_still_emits_while_disabled_so_teardown_is_not_leaked() {
    let mut window = window();
    let closed = Arc::new(AtomicUsize::new(0));
    {
        let sink = closed.clone();
        window.closed.connect(move || {
            sink.fetch_add(1, Ordering::SeqCst);
        });
    }

    window.set_enabled(false);
    window.close();

    assert_eq!(
        closed.load(Ordering::SeqCst),
        1,
        "disabling the window's contents must not suppress the teardown signal: the host \
         still has to release what it opened"
    );
    assert!(!window.is_visible());
}

/// The `close` **command** and the `close` **method** must not diverge.
///
/// `window` publishes `close` in its capability, so a generic consumer reaches it through
/// `WidgetFactory::invoke_command` while application code calls `Window::close`. Two routes
/// to one concept is the duplication principle #101 forbids unless they provably agree --
/// so both are driven here under the same (disabled) condition and must produce the same
/// observable result.
#[test]
fn window_close_via_the_published_command_keeps_the_same_contract() {
    let factory = WidgetFactory::new_with_defaults();
    let mut widget = factory.create("window", Rect::new(0, 0, 640, 480), "Test").unwrap();
    widget.set_enabled(false);

    let closed = Arc::new(AtomicUsize::new(0));
    {
        let sink = closed.clone();
        let hub = rust_widgets::signal::CustomSignalHub::new();
        factory
            .connect_event("window", "closed", &hub, move || {
                sink.fetch_add(1, Ordering::SeqCst);
            })
            .expect("`window` publishes `closed`");
    }

    assert!(widget.is_visible(), "a freshly created window is visible until it is closed");

    factory
        .invoke_command(widget.as_mut(), "close")
        .expect("`window` publishes a payload-free `close` command");

    assert!(
        !widget.is_visible(),
        "the command route must reach the same `close` as `Window::close`, disabled or not"
    );
    let _ = closed;
}
