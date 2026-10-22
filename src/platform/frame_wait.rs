// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Bounded wait between backend frame iterations.
//!
//! # Why this exists
//!
//! Every backend loop used to end an iteration with `thread::sleep(FRAME_INTERVAL_MS)`: 62.5
//! wakeups per second even when nothing was animating and no input had arrived. The frame driver
//! already answers whether a frame is owed
//! ([`FrameOutcome::needs_another_frame`](crate::widget::runtime::FrameOutcome)); the missing
//! piece was a way to *block* when it is not.
//!
//! # Wake sources and the backstop
//!
//! [`wake`] is called when something a still loop must react to happens: an event is posted, a
//! stop is requested, or cross-thread work is scheduled. [`pause_between_frames`] blocks until
//! such a wake or a bounded backstop, so state that has no wake channel (a device fact changing
//! on disk, a host mutating state directly) is still observed within
//! [`IDLE_FRAME_INTERVAL_MS`](crate::platform::types::IDLE_FRAME_INTERVAL_MS).
//!
//! # Platform note
//!
//! Win32 owns a native message queue: blocking on a condition variable there would delay input.
//! On Windows [`pause_between_frames`] blocks in `MsgWaitForMultipleObjectsEx` instead, which
//! returns as soon as a message arrives. AppKit delivers through its own run loop; that backend
//! waits with `nextEventMatchingMask:untilDate:` rather than using this module's condvar.

#[cfg(not(all(windows, not(alloc_frugal))))]
use core::time::Duration;

use crate::platform::types::{FRAME_INTERVAL_MS, IDLE_FRAME_INTERVAL_MS};

#[cfg(not(alloc_frugal))]
mod inner {
    use core::sync::atomic::{AtomicU64, Ordering};
    #[cfg(not(windows))]
    use core::time::Duration;
    use std::sync::{Condvar, Mutex, OnceLock};

    struct WakeState {
        /// Incremented by every [`wake`]. The waiter compares it before and after taking the
        /// mutex, which closes the window in which a notify could arrive between the check and
        /// the wait.
        generation: AtomicU64,
        mutex: Mutex<()>,
        ready: Condvar,
    }

    static WAKE: OnceLock<WakeState> = OnceLock::new();

    fn state() -> &'static WakeState {
        WAKE.get_or_init(|| WakeState {
            generation: AtomicU64::new(0),
            mutex: Mutex::new(()),
            ready: Condvar::new(),
        })
    }

    /// Wakes any backend loop blocked in the frame wait.
    pub(crate) fn wake() {
        let state = state();
        state.generation.fetch_add(1, Ordering::SeqCst);
        // Notify while holding the mutex: a waiter that observed the previous generation must
        // not be able to enter `wait_timeout` after this notify has already passed.
        let _guard = state.mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        state.ready.notify_all();
    }

    /// Blocks until [`wake`] or `timeout`, whichever happens first.
    ///
    /// Win32 has no caller for this: [`pause_between_frames`](super::pause_between_frames)
    /// blocks in its own message queue there, which is what keeps input latency at zero.
    #[cfg(not(windows))]
    pub(crate) fn wait_timeout(timeout: Duration) {
        let state = state();
        let observed = state.generation.load(Ordering::SeqCst);
        let guard = state.mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.generation.load(Ordering::SeqCst) != observed {
            return;
        }
        let _ = state.ready.wait_timeout(guard, timeout);
    }
}

#[cfg(alloc_frugal)]
mod inner {
    use core::time::Duration;

    /// No loop in this profile blocks on a wake channel; keeping the entry point means shared
    /// backend loops need no `cfg` of their own (principle #53).
    pub(crate) fn wake() {}

    /// The allocation-frugal profile has no animation bus, so the loop simply paces itself.
    pub(crate) fn wait_timeout(timeout: Duration) {
        std::thread::sleep(timeout);
    }
}

pub(crate) use inner::wake;

/// Waits for the next iteration of a backend loop.
///
/// A frame that is owed is paced at the display interval. A still loop blocks for a wake with a
/// bounded backstop, which is what removes the idle busy-poll.
pub(crate) fn pause_between_frames(needs_another_frame: bool) {
    #[cfg(all(windows, not(alloc_frugal)))]
    {
        // Win32 has a real message queue. Blocking in it is what makes input latency zero while
        // idle CPU stays at zero; a condition-variable wait would delay `WM_*` messages.
        let timeout = if needs_another_frame { FRAME_INTERVAL_MS } else { IDLE_FRAME_INTERVAL_MS };
        // SAFETY: no handles are passed (`nCount == 0`, null pointer), so the call cannot read
        // or write any memory we own. `QS_ALLINPUT` waits for any input message, and
        // `MWMO_INPUTAVAILABLE` also returns for input already queued before the call.
        unsafe {
            winapi::um::winuser::MsgWaitForMultipleObjectsEx(
                0,
                core::ptr::null(),
                timeout as u32,
                winapi::um::winuser::QS_ALLINPUT,
                winapi::um::winuser::MWMO_INPUTAVAILABLE,
            );
        }
    }
    #[cfg(not(all(windows, not(alloc_frugal))))]
    {
        let timeout = if needs_another_frame { FRAME_INTERVAL_MS } else { IDLE_FRAME_INTERVAL_MS };
        inner::wait_timeout(Duration::from_millis(timeout));
    }
}
