// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Window lifecycle notifications shared by the desktop backends.
//!
//! # Why this is not an OS module
//!
//! Closing a window, gaining keyboard focus and moving are window-manager facts, but the
//! library's *response* to them is platform-independent: consult a veto handler, announce the
//! event, and update the window mirror. Keeping that response here means the Win32, GTK and
//! both AppKit backends each translate their own notification into one call instead of
//! reproducing the policy (and its ordering) per OS.
//!
//! # The close veto
//!
//! [`request_window_close`] is the single decision point a backend consults before it lets a
//! window disappear. Two things can refuse the request:
//!
//! * a handler installed with `WindowHandle::set_close_handler` returning `false`; and
//! * a control consuming [`Event::WindowCloseRequest`] (the event is the documented
//!   "proposal the application may refuse", and consuming it is how a handler says no).
//!
//! The handler is asked first, then the event is dispatched, and only a request that both
//! allow reaches the backend. A build with no widget runtime has neither mechanism and the
//! function reports "allowed", which is what the backend's own lifecycle did before this
//! module existed.

use crate::compat::{Box, HashMap, RefCell};
use crate::core::{ObjectId, Point};

/// A close veto handler: `true` allows the close, `false` refuses it.
pub(crate) type CloseHandler = Box<dyn Fn() -> bool>;

thread_local! {
    /// Close handlers installed by `WindowHandle::set_close_handler`, keyed by window id.
    static CLOSE_HANDLERS: RefCell<HashMap<ObjectId, CloseHandler>> =
        RefCell::new(HashMap::new());

    /// The last position the library knows for a window, used as the fallback when a backend
    /// has no position query (a state-only window). It is seeded at creation and updated by
    /// accepted moves and by platform move notifications, so a read never reports a position
    /// the window is known to have left.
    static WINDOW_POSITIONS: RefCell<HashMap<ObjectId, Point>> = RefCell::new(HashMap::new());
}

/// Records the library-side position mirror for `window_id`.
pub(crate) fn record_window_position(window_id: ObjectId, position: Point) {
    WINDOW_POSITIONS.with(|positions| {
        positions.borrow_mut().insert(window_id, position);
    });
}

/// Reads the library-side position mirror for `window_id`.
#[allow(dead_code)]
pub(crate) fn window_position_mirror(window_id: ObjectId) -> Option<Point> {
    WINDOW_POSITIONS.with(|positions| positions.borrow().get(&window_id).copied())
}

/// Installs (or replaces) the close handler for `window_id`.
#[allow(dead_code)]
pub(crate) fn set_close_handler(window_id: ObjectId, handler: CloseHandler) {
    CLOSE_HANDLERS.with(|handlers| {
        handlers.borrow_mut().insert(window_id, handler);
    });
}

/// Removes the close handler for `window_id`, returning whether one was installed.
#[allow(dead_code)]
pub(crate) fn clear_close_handler(window_id: ObjectId) -> bool {
    CLOSE_HANDLERS.with(|handlers| handlers.borrow_mut().remove(&window_id).is_some())
}

/// Decides whether a window may close, asking the veto handler and the `WindowCloseRequest`
/// event.
///
/// Returns `true` when the close may proceed. The answer is deliberately conservative: a
/// handler returning `false` refuses even if the event route would allow it, and a consumed
/// event refuses even when no handler was installed. A build with no widget runtime has no
/// route to consult, so the historical "allow" answer stands.
///
/// `allow(dead_code)` on a profile with no desktop backend: only an OS close notification calls
/// this, and such a profile never produces one. The function is still compiled so the backend
/// matrix needs no per-OS arms in this shared module.
#[allow(dead_code)]
pub(crate) fn request_window_close(window_id: ObjectId) -> bool {
    let handler_allows = CLOSE_HANDLERS
        .with(|handlers| handlers.borrow().get(&window_id).map(|h| h()).unwrap_or(true));
    // Clear any stale flag left by unrelated delivery on this thread: without this, a control
    // that consumed an earlier event could veto a close it never saw.
    let _ = crate::event::propagation::take_consumed();
    let delivered =
        crate::widget::runtime::dispatch_event(window_id, &crate::event::Event::WindowCloseRequest);
    let event_vetoed = delivered && crate::event::propagation::take_consumed();
    handler_allows && !event_vetoed
}

/// Announces that `window_id`'s top-level window gained or lost keyboard focus.
///
/// The window widget emits its `focus_gained`/`focus_lost` signal; a handle-level callback
/// connected to that signal runs on the same thread, which is where the platform notification
/// arrived.
#[allow(dead_code)]
pub(crate) fn notify_window_focus(window_id: ObjectId, gained: bool) {
    let Some(window) = crate::widget::runtime::top_level_window_of(window_id) else {
        return;
    };
    let _ = crate::widget::runtime::with_widget_mut(window, |widget| {
        if let Some(window) =
            crate::widget::capability::widget_as_mut::<crate::widget::Window>(widget)
        {
            if gained {
                window.notify_focus_gained();
            } else {
                window.notify_focus_lost();
            }
        }
    });
}

/// Announces that `window_id`'s top-level window moved to `position`.
///
/// The mirror is updated before the signal is emitted, so a `position()` read made from a
/// handler sees the same point the signal carries.
#[allow(dead_code)]
pub(crate) fn notify_window_moved(window_id: ObjectId, position: Point) {
    let Some(window) = crate::widget::runtime::top_level_window_of(window_id) else {
        return;
    };
    // Update the mirror before emitting, so a `position()` read made from a handler sees the
    // same point the signal carries.
    record_window_position(window, position);
    let _ = crate::widget::runtime::with_widget_mut(window, |widget| {
        if let Some(window) =
            crate::widget::capability::widget_as_mut::<crate::widget::Window>(widget)
        {
            window.notify_moved(position);
        }
    });
}
