// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Work any thread may hand to the loop's own thread.
//!
//! # The problem this solves
//!
//! A host that is not driving the loop still wants things done *on* it: end the session after a
//! deadline (a smoke test that must terminate itself), refresh a status line, poll something the
//! toolkit wants read on its own thread. Every toolkit has its own timer API for that
//! (`NSTimer`, `glib::timeout_add`, `SetTimer`) — and none of them may be *created* from another
//! thread, which is exactly the situation a watcher thread is in.
//!
//! # Why the frame tick is the executor
//!
//! The same seam that makes a cross-thread stop work ([`crate::platform::stop`]): every loop this
//! library owns calls [`crate::drive_frame`] on the thread that owns it, so a queue read there runs
//! its work on that thread — without the library creating a toolkit timer, and without any
//! per-backend code. On AppKit that is the per-window `NSTimer`; on GTK the `glib` timeout
//! closure; everywhere else the loop's own iteration.
//!
//! # Precision, and what a caller may rely on
//!
//! `after` is a **lower bound**. Work becomes due at its deadline and runs on the next frame that
//! follows, so the granularity is one frame interval (~16 ms on every backend here) and a still
//! window does not change it: the loops tick unconditionally. A caller that needs sub-frame
//! precision wants a toolkit timer, not this.
//!
//! # Repeating work
//!
//! There is no repeating form on purpose. A callback that wants to run again calls
//! [`schedule_in`] again at its end, which is one line and makes the *interval* the caller's
//! explicit choice rather than a second mechanism with its own drift (principles #54/#101).

// Standard-library types on purpose, and the module is `#[cfg(not(alloc_frugal))]`: the queue
// needs an allocator, and the profiles that have one are exactly the ones with a platform loop to
// hand work to. A `mini`/`embedded` build has no loop that could run it, so it does not get the
// entry point either — an honest absence rather than a function that always answers `false`.
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// A callback waiting for the loop's next frame at or after its deadline.
struct Due {
    /// When the work may run. `now` is read once, when the work is queued, so a caller's clock
    /// (not the loop's) decides the deadline.
    at: Instant,
    /// The work. Boxed because the queue holds several shaped-differently closures, and `Send`
    /// because it was handed over from another thread.
    work: Box<dyn FnOnce() + Send + 'static>,
}

/// The queue, created on first use.
///
/// A `Mutex<Vec<_>>` rather than a channel: the frame tick has to *select* the due items and leave
/// the rest, which is a partition over one collection rather than a receive.
static QUEUE: OnceLock<Mutex<Vec<Due>>> = OnceLock::new();

/// Hands `work` to the loop's own thread, to run at or after `after` from now.
///
/// Returns whether a loop in this process is in a position to run it: `true` when a loop has
/// driven a frame, so it will reach the queue on its next one — the same fact
/// [`crate::platform::stop::loop_has_ticked`] reports, and the same reason
/// [`crate::quit`](crate::quit) reports it for a stop request. `false` means no loop has ticked
/// yet; **the work is still queued** and a loop started later runs it (after its deadline), which
/// is what makes "schedule the ending, then run the loop" work for a host that does both from one
/// thread.
///
/// # A panicking callback
///
/// A callback that panics propagates: it is host code running on the loop's thread, exactly as a
/// signal handler is, and swallowing the panic here would turn "your heartbeat is broken" into a
/// session that quietly stops beating. The queue itself survives a panic (the poisoned lock is
/// taken with `into_inner`), so a host that catches the unwind at its own boundary can keep going
/// with the remaining work intact.
pub fn schedule_in(after: Duration, work: impl FnOnce() + Send + 'static) -> bool {
    let queued = QUEUE.get_or_init(|| Mutex::new(Vec::new()));
    let mut guard = match queued.lock() {
        Ok(guard) => guard,
        // A previous callback panicked while nothing else held the lock; the queue's contents are
        // still consistent (the panic happened while the items were *out* of it).
        Err(poisoned) => poisoned.into_inner(),
    };
    guard.push(Due { at: Instant::now() + after, work: Box::new(work) });
    drop(guard);
    crate::platform::stop::loop_has_ticked()
}

/// Runs everything that has come due. Called by the frame tick, on the loop's own thread.
///
/// # Why the due items are taken out before they run
///
/// A callback may schedule more work (the documented way to repeat), which takes the same lock.
/// Running callbacks while holding it would deadlock the loop on the first one that did, so the
/// due items are moved out under the lock and executed after it is released. Work queued *by* a
/// callback that is due immediately therefore runs on the **next** frame, not this one — a
/// one-frame delay rather than re-entrancy.
pub(crate) fn run_due_on_loop_thread() {
    let Some(queued) = QUEUE.get() else {
        // Nothing was ever scheduled in this process: no allocation, no lock, no branch taken
        // beyond this one.
        return;
    };
    let now = Instant::now();
    let due: Vec<Due> = {
        let mut guard = match queued.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let (due, later): (Vec<Due>, Vec<Due>) = guard.drain(..).partition(|item| item.at <= now);
        *guard = later;
        due
    };
    for item in due {
        (item.work)();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    /// Work must not run before its deadline, and must run when the tick reaches it.
    ///
    /// Driven by calling the tick's own entry point directly, which is what makes this
    /// deterministic: no loop, no sleep race — the queue's rule ("due at its deadline, run on the
    /// next tick") is asserted on its own.
    #[test]
    fn work_runs_on_the_tick_that_reaches_its_deadline() {
        let ran = Arc::new(AtomicBool::new(false));
        let flag = ran.clone();
        let asking_thread = std::thread::current().id();
        schedule_in(Duration::from_millis(40), move || {
            // The tick runs this on the *loop's* thread, so a direct call from this test thread
            // must see it here; what the flag records is that it ran exactly once.
            assert_eq!(std::thread::current().id(), asking_thread);
            flag.store(true, Ordering::SeqCst);
        });

        // Before the deadline the tick must leave it alone.
        run_due_on_loop_thread();
        assert!(!ran.load(Ordering::SeqCst), "work ran before its deadline");

        // After it, the next tick must run it.
        let deadline = Instant::now() + Duration::from_millis(500);
        while !ran.load(Ordering::SeqCst) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
            run_due_on_loop_thread();
        }
        assert!(ran.load(Ordering::SeqCst), "the tick never ran work that was due");

        // ... and the queue does not keep it: a second tick must find nothing, so work runs once
        // rather than once per frame for the rest of the session.
        let queue = QUEUE.get().expect("the queue exists");
        let remaining = queue.lock().expect("not poisoned").len();
        assert_eq!(remaining, 0, "a consumed item must leave the queue");
    }

    /// A callback that schedules more work must not deadlock the tick.
    #[test]
    fn work_scheduled_by_a_callback_does_not_deadlock_the_tick() {
        let ran = Arc::new(AtomicBool::new(false));
        let flag = ran.clone();
        schedule_in(Duration::from_millis(1), move || {
            // Repeating work is exactly this: the callback reschedules itself.
            schedule_in(Duration::from_millis(0), move || flag.store(true, Ordering::SeqCst));
        });
        std::thread::sleep(Duration::from_millis(5));
        run_due_on_loop_thread();
        // The nested item is due, but it was queued *by* a callback, so it runs on the next tick.
        run_due_on_loop_thread();
        assert!(ran.load(Ordering::SeqCst), "the rescheduled work never ran");
    }
}
