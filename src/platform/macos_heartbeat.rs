// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The frame heartbeat both macOS backends install, kept alive inside AppKit's modal loops.
//!
//! # The defect this exists to remove
//!
//! Dragging a window's edge (or its title bar) puts AppKit in a **modal tracking loop**: the
//! resize/move loop pumps the run loop itself, in `NSEventTrackingRunLoopMode`, and the process does
//! not return to the backend's own loop until the user releases the mouse. Both macOS backends
//! drive [`crate::drive_frame`] from that loop, so for the whole drag nothing drains the trigger
//! queue: the `Resized` trigger `windowDidResize:` queues is never dispatched, the client area keeps
//! the layout it had when the drag began, and no animation advances.
//!
//! Measured on the cocoa backend, whose heartbeat was an `NSTimer` that
//! `scheduledTimerWithTimeInterval:…` had put in `NSDefaultRunLoopMode` — a mode the tracking loop
//! does not run. The fix is not a longer timeout or a second drain: it is enrolling the heartbeat
//! in the modes the modal loop *does* run.
//!
//! # Why this is a CoreFoundation timer
//!
//! `CFRunLoopAddTimer` with `kCFRunLoopCommonModes` enrols the timer in every mode AppKit's own
//! modal loops run — the common-mode set is exactly {default, modal panel, event tracking} — so the
//! heartbeat keeps its cadence inside the drag instead of resuming after it. CoreFoundation is also
//! the one dependency both macOS backends already share: `NSTimer` would have forced
//! `objc2-foundation`'s `NSTimer`/`NSRunLoop` features onto the objc2 backend, and `NSRunLoopModes`
//! is not exposed by the `cocoa` crate the other one uses. One timer, one cadence rule, both
//! backends.
//!
//! # [`HeartbeatScope`], and why the two backends differ
//!
//! * The **cocoa backend has no other frame driver**: `-[NSApplication run]` only pumps AppKit, and
//!   this timer *is* its frame loop. It therefore drives in every mode.
//! * The **objc2 backend drives frames from its own `run()` loop**, which cannot see a modal loop
//!   (`-[NSApplication nextEventMatchingMask:…inMode:NSDefaultRunLoopMode]` returns nothing while
//!   AppKit tracks, and `sendEvent:` does not return at all). Its heartbeat therefore fires only
//!   **outside** `kCFRunLoopDefaultMode`, i.e. exactly while a modal or tracking loop has taken the
//!   process, so it cannot tick twice per interval when the loop is running.
//!
//! In default mode that modal-only timer retargets to the idle backstop instead of driving, which
//! is what keeps a still window at the same wake rate it had before this heartbeat existed
//! (PL-04) while capping the delay before a drag is noticed at one idle interval.
//!
//! # The cadence
//!
//! One frame per tick, paced by the display link: the tick asks
//! [`crate::platform::display_link::next_deadline`] for the instant the next frame is presented,
//! advances the animation bus by the measured delta, and moves its own next fire to that instant.
//! A frame that owes nothing retargets to [`IDLE_FRAME_INTERVAL_MS`] instead, so an idle window is
//! not woken at the display rate. The timer repeats and is never invalidated: invalidating it would
//! also stop the drain this exists for.

#![cfg(all(
    target_os = "macos",
    any(all(feature = "cocoa-legacy", widgets_unstripped), feature = "macos")
))]

use core::ffi::c_void;
use core::ptr;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::time::{Duration, Instant};

use objc2_core_foundation::{
    kCFRunLoopCommonModes, kCFRunLoopDefaultMode, CFAbsoluteTimeGetCurrent, CFRunLoop,
    CFRunLoopTimer, CFRunLoopTimerContext,
};

use crate::platform::frame_wait::{drive_once, heartbeat_may_drive};

/// Which run-loop modes the heartbeat may drive a frame in. See the module docs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HeartbeatScope {
    /// Every mode, `kCFRunLoopDefaultMode` included. The cocoa backend, whose frame loop this is.
    ///
    /// Gated to the backend that asks for it: a build with only the objc2 backend has no frame
    /// loop to serve, and a variant nothing constructs is a name that lies about being reachable.
    #[cfg(feature = "cocoa-legacy")]
    EveryMode,
    /// Only outside `kCFRunLoopDefaultMode`. The objc2 backend, whose own loop is the driver.
    ModalLoopsOnly,
}

impl HeartbeatScope {
    fn encoded(self) -> u8 {
        match self {
            #[cfg(feature = "cocoa-legacy")]
            Self::EveryMode => 0,
            Self::ModalLoopsOnly => 1,
        }
    }
}

/// 0 = [`HeartbeatScope::EveryMode`], 1 = [`HeartbeatScope::ModalLoopsOnly`].
static SCOPE: AtomicU8 = AtomicU8::new(0);
/// Whether the process-wide timer is on the main run loop.
static INSTALLED: AtomicBool = AtomicBool::new(false);

/// Installs the process-wide heartbeat on the main run loop, once.
///
/// Returns `true` when a heartbeat is installed (now or from an earlier call) and `false` when the
/// main run loop or the timer could not be created. The scope is updated on every call, so a second
/// backend in the same process changes the policy rather than installing a second timer — the
/// timer is process-wide because a frame is process-wide.
///
/// # Safety
///
/// Must be called on the AppKit main thread, because the timer is added to that thread's main run
/// loop. Both call sites are backend start-up, which AppKit already requires to be the main thread.
pub(crate) unsafe fn install(scope: HeartbeatScope) -> bool {
    SCOPE.store(scope.encoded(), Ordering::SeqCst);
    if INSTALLED.load(Ordering::SeqCst) {
        return true;
    }

    let Some(run_loop) = CFRunLoop::main() else {
        log::warn!("[macos] no main run loop; the frame heartbeat cannot be installed");
        return false;
    };
    let interval = Duration::from_millis(crate::platform::types::FRAME_INTERVAL_MS);
    let mut context = CFRunLoopTimerContext {
        version: 0,
        info: ptr::null_mut(),
        retain: None,
        release: None,
        copyDescription: None,
    };
    // SAFETY: the callout has the `CFRunLoopTimerCallBack` signature, the context is a valid
    // `CFRunLoopTimerContext` that outlives the create call (CoreFoundation copies it), and a null
    // allocator asks for the default one.
    let Some(timer) = (unsafe {
        CFRunLoopTimer::new(
            None,
            CFAbsoluteTimeGetCurrent(),
            interval.as_secs_f64(),
            0,
            0,
            Some(tick),
            &mut context,
        )
    }) else {
        log::warn!("[macos] CoreFoundation refused to create the frame heartbeat timer");
        return false;
    };

    // SAFETY: `kCFRunLoopCommonModes` is an immutable CoreFoundation mode constant, and the timer is
    // live for the duration of the call. This is the whole point of the module: the default mode
    // alone is not run by AppKit's tracking loops.
    run_loop.add_timer(Some(&timer), unsafe { kCFRunLoopCommonModes });

    // The run loop retains the timer; dropping this handle releases only this reference. The
    // timer itself runs for the life of the process, because a heartbeat that stopped could not
    // restart the drain it exists for.
    drop(timer);
    INSTALLED.store(true, Ordering::SeqCst);
    true
}

/// `CFRunLoopTimerCallBack`: one heartbeat tick.
extern "C-unwind" fn tick(timer: *mut CFRunLoopTimer, _info: *mut c_void) {
    let Some(timer) = (unsafe { timer.as_ref() }) else {
        return;
    };
    let modal_loops_only = SCOPE.load(Ordering::SeqCst) == HeartbeatScope::ModalLoopsOnly.encoded();

    if !heartbeat_may_drive(modal_loops_only, current_mode_is_default()) {
        // The backend's own loop is running and is the frame driver. Backing off to the idle
        // interval here is what keeps a still window at its old wake rate; the cost is that a drag
        // is noticed at most one idle interval late, which is a tenth of a second.
        retarget_in(timer, Duration::from_millis(crate::platform::types::IDLE_FRAME_INTERVAL_MS));
        return;
    }

    // A trigger dispatched by this frame may run a host's modal dialog, whose own pump would
    // deliver the next tick straight back into this function. `drive_once` makes the heartbeat mean
    // "at most one frame in flight".
    let driven = drive_once(|| {
        let deadline = crate::platform::display_link::next_deadline();
        let delta = crate::platform::frame_wait::presented_delta_ms(deadline);
        let outcome = crate::drive_frame(delta);
        // Tell the display link whether this frame owed another one; an idle window's callback
        // then skips its present-time bookkeeping and its wake (PL-04). The flag lives in the
        // shared module both macOS backends use.
        crate::platform::display_link::note_frame_demand(outcome.needs_another_frame);
        (outcome.needs_another_frame, deadline)
    });
    let Some((needs_another_frame, deadline)) = driven else {
        return;
    };

    if needs_another_frame {
        // Follow the display when there is one. With no display link (a headless or CoreVideo-less
        // host) the timer's own interval *is* the cadence, so it is deliberately left alone --
        // retargeting to the idle backstop there would drop every animation to 10 fps.
        if let Some(deadline) = deadline {
            retarget_at(timer, deadline);
        }
    } else {
        // Nothing is animating and nothing is queued (G-503): a still window does not need 60
        // wakeups a second. The tick is moved out to the idle backstop, which still drains the
        // resize triggers and notices a host that changed state without a wake; the next animated
        // frame moves it back to the display deadline.
        retarget_in(timer, Duration::from_millis(crate::platform::types::IDLE_FRAME_INTERVAL_MS));
    }
}

/// Whether the main run loop is currently running in `kCFRunLoopDefaultMode`.
///
/// `false` when the run loop is not in a mode at all (which cannot happen from inside a timer
/// callback, but must not be read as "modal"): the caller then treats the tick as drivable, which
/// is the harmless answer — a frame runs and the app is no worse for it.
fn current_mode_is_default() -> bool {
    let Some(run_loop) = CFRunLoop::current() else {
        return false;
    };
    let Some(current) = run_loop.current_mode() else {
        return false;
    };
    // SAFETY: `kCFRunLoopDefaultMode` is an immutable CoreFoundation mode constant.
    let Some(default_mode) = (unsafe { kCFRunLoopDefaultMode }) else {
        return false;
    };
    ptr::eq(&*current, default_mode)
}

/// Moves the next fire to `deadline`, or leaves it alone when the instant has already passed.
fn retarget_at(timer: &CFRunLoopTimer, deadline: Instant) {
    let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
        return;
    };
    retarget_in(timer, remaining);
}

/// Moves the next fire to `delay` from now.
///
/// A delay below one millisecond is left alone so a missed presentation cannot turn the timer into
/// a busy loop.
fn retarget_in(timer: &CFRunLoopTimer, delay: Duration) {
    if delay < Duration::from_millis(1) {
        return;
    }
    timer.set_next_fire_date(CFAbsoluteTimeGetCurrent() + delay.as_secs_f64());
}
