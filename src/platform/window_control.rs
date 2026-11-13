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

use crate::compat::{Box, HashMap, Rc, RefCell};
use crate::core::{ObjectId, Point};

/// A close veto handler: `true` allows the close, `false` refuses it.
pub(crate) type CloseHandler = Box<dyn Fn() -> bool>;

thread_local! {
    /// Close handlers installed by `WindowHandle::set_close_handler`, keyed by window id.
    ///
    /// The handler is **shared** rather than moved out while it runs: `request_window_close`
    /// clones it, ends the borrow, and only then calls it. That clone is what lets a handler
    /// re-enter this module (`set_close_handler`, `clear_close_handler`, a nested
    /// `request_window_close`) without panicking on the `RefCell`, while the map keeps the truth:
    /// a replacement installed from inside the callback wins, and one the callback clears stays
    /// cleared.
    static CLOSE_HANDLERS: RefCell<HashMap<ObjectId, Rc<dyn Fn() -> bool>>> =
        RefCell::new(HashMap::new());

    /// The last position the library knows for a window, used as the fallback when a backend
    /// has no position query (a state-only window). It is seeded at creation and updated by
    /// accepted moves and by platform move notifications, so a read never reports a position
    /// the window is known to have left.
    static WINDOW_POSITIONS: RefCell<HashMap<ObjectId, Point>> = RefCell::new(HashMap::new());

    /// The window that most recently gained keyboard focus, as reported by the platform's focus
    /// notification. `Platform` has no focus query, so this mirror is the one runtime source a
    /// host can ask (HB-12).
    static ACTIVE_WINDOW: core::cell::Cell<Option<ObjectId>> = const { core::cell::Cell::new(None) };
}

/// The window that most recently gained keyboard focus, if the platform reported one.
/// The window that most recently gained keyboard focus, as reported by the platform's focus
/// notification.
///
/// Not every build compiles the handle layer that reads this (a cross cell may enable a platform
/// backend without `desktop-runtime`), so the accessor is allowed to be unused rather than
/// producing a warning in those cells.
#[allow(dead_code)]
pub(crate) fn active_window_id() -> Option<ObjectId> {
    ACTIVE_WINDOW.try_with(|active| active.get()).unwrap_or(None)
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

/// Removes the library-side position mirror for `window_id`.
///
/// A window create/destroy loop used to leak one entry per window: the mirror had a writer and a
/// reader but no removal, so `remove_callbacks` could not make good on "remove every callback and
/// mirror for this id" (HB-02).
/// Removes the library-side position mirror for `window_id`.
///
/// A window create/destroy loop used to leak one entry per window: the mirror had a writer and a
/// reader but no removal, so `remove_callbacks` could not make good on "remove every callback and
/// mirror for this id" (HB-02). Like [`active_window_id`], the handle layer that calls this is not
/// compiled by every cross cell.
#[allow(dead_code)]
pub(crate) fn remove_window_position(window_id: ObjectId) -> bool {
    WINDOW_POSITIONS.with(|positions| positions.borrow_mut().remove(&window_id).is_some())
}

/// Clears the active-window mirror when `window_id` is the one it names.
///
/// Focus is reported by `notify_window_focus`; teardown has no focus notification, so a
/// destroyed focused window otherwise stays visible through `App::active_window()`.
#[allow(dead_code)]
pub(crate) fn clear_active_window(window_id: ObjectId) -> bool {
    let mut cleared = false;
    let _ = ACTIVE_WINDOW.try_with(|active| {
        if active.get() == Some(window_id) {
            active.set(None);
            cleared = true;
        }
    });
    cleared
}

/// Installs (or replaces) the close handler for `window_id`.
///
/// The `Box` is turned into an `Rc` here so a dispatch can hold a clone of the handler while the
/// map keeps owning the entry; see [`CLOSE_HANDLERS`].
#[allow(dead_code)]
pub(crate) fn set_close_handler(window_id: ObjectId, handler: CloseHandler) {
    CLOSE_HANDLERS.with(|handlers| {
        handlers.borrow_mut().insert(window_id, Rc::from(handler));
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
    // Clone the handler out and end the borrow before invoking it. The handler is user code, and
    // user code re-enters this module for real: a "save before closing" veto installs a follow-up
    // handler and may ask whether the window can close now. Calling it inside the `borrow()`
    // panicked with `already borrowed` on the first of those calls.
    //
    // The entry stays in the map for the duration, so the clone running here is never written
    // back: a handler installed by the callback answers the next close, and a handler that clears
    // itself is not resurrected. A nested request does reach this same handler again — the veto
    // decides the proposal — so a handler that re-asks must not recurse unconditionally.
    let handler = CLOSE_HANDLERS.with(|handlers| handlers.borrow().get(&window_id).cloned());
    let handler_allows = handler.map(|handler| handler()).unwrap_or(true);
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
    // Track the active window beside the signal: the platform trait has no focus query, so a
    // host that asks "which window is active?" has only this notification (HB-12).
    let _ = ACTIVE_WINDOW.try_with(|active| {
        if gained {
            active.set(Some(window));
        } else if active.get() == Some(window) {
            active.set(None);
        }
    });
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
    // Queue the same `Moved` trigger the GTK and Win32 backends queue, so the screen-anchored
    // layout branch and the DPI/scale re-layout are reachable on macOS too (PL4-02). A
    // moved-only tick is a no-op through the LY3-01 guard, so this costs nothing at rest.
    let _ = crate::inject_widget_trigger_event(window, crate::platform::WidgetTriggerKind::Moved);
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::cell::Cell;

    /// Ids no other test or mounted window uses, so the thread-local handler table cannot collide.
    const REENTRANT_WINDOW: ObjectId = 0x5746_0001;
    const SELF_CLEARING_WINDOW: ObjectId = 0x5746_0002;

    /// A veto handler may re-enter the close API, and the handler it installs answers next time.
    ///
    /// # The defect this closes
    ///
    /// The handler used to be called inside `CLOSE_HANDLERS.borrow()`. `set_close_handler` and
    /// `request_window_close` are exactly the calls a "save before closing" screen makes from
    /// inside its own veto, and both took the same `RefCell` borrow again — `already borrowed`
    /// instead of a decision. The handler is now cloned out and the borrow released before it
    /// runs, and because the map still owns the entry, the replacement wins for the next close.
    #[test]
    fn a_close_veto_handler_can_reenter_the_close_api() {
        let window = REENTRANT_WINDOW;
        clear_close_handler(window);

        let calls = Rc::new(Cell::new(0u32));
        let nested = Rc::new(Cell::new(None));
        let handler_calls = Rc::clone(&calls);
        let nested_result = Rc::clone(&nested);

        set_close_handler(
            window,
            Box::new(move || {
                handler_calls.set(handler_calls.get() + 1);
                if handler_calls.get() == 1 {
                    // Re-enter the module twice: a nested close request (which still sees this
                    // handler installed) and a replacement (which must win for the next close).
                    nested_result.set(Some(request_window_close(window)));
                    set_close_handler(window, Box::new(|| false));
                }
                false
            }),
        );

        assert!(!request_window_close(window), "the installed handler refuses the close");
        assert_eq!(
            calls.get(),
            2,
            "the nested request re-entered the handler instead of panicking on the borrow"
        );
        assert_eq!(
            nested.get(),
            Some(false),
            "the nested request is answered by the handler that was installed then"
        );

        // The replacement installed from inside the callback owns the next close. Had the clone
        // that just ran been written back over it, `calls` would rise again here.
        assert!(!request_window_close(window), "the replacement handler vetoes the next close");
        assert_eq!(calls.get(), 2, "the replaced handler must not answer again");

        clear_close_handler(window);
    }

    /// A handler that clears itself during its run stays cleared.
    ///
    /// `clear_close_handler` from inside the veto was the other re-entrant call that panicked on
    /// the `RefCell` borrow. The handler is cloned rather than moved out of the map, so the
    /// removal the callback performs is the last word: nothing re-inserts the clone afterwards.
    #[test]
    fn a_close_handler_that_clears_itself_is_not_resurrected() {
        let window = SELF_CLEARING_WINDOW;
        clear_close_handler(window);

        set_close_handler(
            window,
            Box::new(move || {
                clear_close_handler(window);
                false
            }),
        );

        assert!(!request_window_close(window), "the installed handler vetoes the first close");
        assert!(request_window_close(window), "a cleared handler must not answer the next close");
        assert!(!clear_close_handler(window), "the callback already removed it");
    }
}
