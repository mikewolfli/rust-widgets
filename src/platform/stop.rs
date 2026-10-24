// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! A stop request any thread may make, read by the loop's own frame tick.
//!
//! # The problem this solves
//!
//! A host that does not drive the loop itself — a smoke test, CI, a scripted demo — has to be
//! able to *end* a session, or its only clean exit is `std::process::exit`, which makes "the
//! session ended" and "the process was killed" indistinguishable in the exit code and the log.
//!
//! No toolkit lets an arbitrary thread stop its loop: `-[NSApplication stop:]` is
//! main-thread-only, GTK's `gtk_main_quit` walks a global main-loop stack that only the loop's
//! thread may touch, and a Win32 `PostQuitMessage` is thread-affine. So the request cannot be
//! *performed* from the watcher thread. What it can be is **recorded**: this module holds one
//! atomic flag, and the loop reads it.
//!
//! # Why the frame tick is the reader
//!
//! Every loop this library owns calls [`crate::drive_frame`] once per iteration **on the
//! thread that owns the loop**: GTK's `glib::timeout_add_local` closure, the polling loops of
//! the state/wayland/windows/mobile/harmony backends, and — the case that motivated this — the
//! repeating `NSTimer` AppKit installs with each window. That makes the frame tick the one
//! seam that exists in every backend *and* runs where the toolkit permits a stop, so the check
//! lives there rather than in each of the eleven loops (principles #36/#54: one implementation,
//! and no OS knowledge outside `src/platform/`).
//!
//! The recorded request therefore turns "the watcher thread cannot stop the loop" into "the
//! watcher thread asks, and the loop stops itself at its next frame" — without any cross-thread
//! toolkit call.
//!
//! # What `loop_has_ticked` answers, and what it does not
//!
//! A request made while no loop is running has no reader: nothing is ticking, so nothing will
//! read it. [`request`] reports that fact by returning whether a loop has driven a frame in
//! this process. It is deliberately the *observed* fact and not a per-backend claim: a backend
//! whose loop never runs (a state-only host, `mini`/`embedded`) has not ticked and answers
//! `false`, which is what a caller needs to decide whether to arrange its own ending.
//!
//! The request is **not** discarded in that case. It stays recorded, so a loop that starts
//! afterwards reads it at its first frame — the same "asked before it was running" case a host
//! hits when it calls this between `create_window` and `run`.

use crate::compat::atomic::{AtomicBool, Ordering};

/// A stop has been asked for and no loop has read it yet.
static STOP_REQUESTED: AtomicBool = AtomicBool::new(false);

/// Whether any loop in this process has driven a frame.
///
/// Set by [`honour_request_on_loop_thread`], which is the frame tick itself, so it is true
/// exactly when a loop has reached its frame body at least once. Read by [`request`] to answer
/// "is there a reader for this?".
static LOOP_TICKED: AtomicBool = AtomicBool::new(false);

/// Records a stop request and reports whether a loop is in a position to read it.
///
/// Returns `true` when a loop has driven a frame in this process: that loop reads the request
/// at its next frame and stops. Returns `false` when no loop has ticked, so nothing is reading
/// the request right now — the request stays recorded and a loop started later honours it at
/// its first frame.
///
/// This is the answer the public [`crate::quit`] reports (the entry a host calls), and the
/// reason it can be asked from a thread the toolkit would refuse: recording is one atomic
/// store, which every thread may do. Performing the toolkit's own stop is a *separate*,
/// thread-bound half — [`Platform::quit`](crate::platform::Platform::quit) — which the frame
/// tick above calls on the loop's own thread.
pub fn request() -> bool {
    STOP_REQUESTED.store(true, Ordering::SeqCst);
    // A loop blocked between frames must wake to read the request; the recorded flag is the
    // acknowledgement, the wake is what makes it observable before the idle backstop expires.
    crate::platform::frame_wait::wake();
    loop_has_ticked()
}

/// Returns whether a stop request is currently recorded and unread.
///
/// A read-only query: it does not consume the request. The loop's own path uses
/// [`take`] instead, because reading the request is what acknowledges it.
pub fn is_requested() -> bool {
    STOP_REQUESTED.load(Ordering::SeqCst)
}

/// Returns whether any loop in this process has driven a frame.
///
/// See [`request`] for why this is an observation rather than a per-backend claim. It is true
/// from the first frame of the first loop and stays true for the rest of the process: a loop
/// that has run is evidence that this profile's loops do run, which is the fact a caller needs
/// before relying on a cross-thread stop.
pub fn loop_has_ticked() -> bool {
    LOOP_TICKED.load(Ordering::SeqCst)
}

/// Consumes a recorded request, reporting whether there was one.
///
/// Taking rather than reading is what makes a request stop exactly one loop: a host that ends
/// a session and then opens another one in the same process does not have the second session
/// stopped by the first session's request.
pub fn take() -> bool {
    STOP_REQUESTED.swap(false, Ordering::SeqCst)
}

/// Marks this thread as the frame tick and stops the loop if a request is pending.
///
/// Called by [`crate::drive_frame`] on the loop's own thread, which is the only thread a
/// toolkit accepts a stop from. The order inside is deliberate: the tick is recorded *before*
/// the request is taken, so a host that asks from another thread while this frame is running
/// is answered by the next frame rather than by a frame that has already passed the check.
///
/// # Why the stop is delegated to the backend
///
/// Stopping a loop is per-toolkit work (`gtk_main_quit`, `NSApplication stop:`,
/// `PostQuitMessage`), and each backend already answers that in its own
/// [`Platform::quit`](crate::platform::Platform::quit). Calling it from here — where we are
/// provably on the loop's thread — is what keeps the toolkit rules inside the backends: this
/// module knows about one atomic flag and nothing about any toolkit.
#[cfg(not(alloc_frugal))]
pub(crate) fn honour_request_on_loop_thread() {
    LOOP_TICKED.store(true, Ordering::SeqCst);
    if !is_requested() {
        return;
    }
    // # Why the request is taken *after* the backend agrees to stop
    //
    // Taking first and stopping second loses the request whenever the backend refuses — and a
    // backend refuses for a reason that has nothing to do with the request being unwanted: a
    // toolkit's stop call is main-thread-only, so a frame driven on any other thread answers
    // `false`. The loop that *can* honour it is still ticking somewhere, so the flag has to stay
    // readable for it. Measured: a helper loop ticking frames on a worker thread swallowed the
    // stop a platform test had just recorded, and that test then waited out its ten-second join.
    //
    // Peeking rather than polling also keeps the common case free: with no request recorded this
    // is one atomic load, which is what the still-frame guarantee costs per frame.
    if crate::platform::get_platform().quit() {
        let _ = take();
    }
}

/// Serializes every test that touches the process-global stop flag or the loop-turn seam.
///
/// [`STOP_REQUESTED`] and [`LOOP_TICKED`] are process-wide by design, so a request left pending
/// by one test ends another test's loop at its first frame — measured as the scheduling test
/// failing only in the full suite and passing alone. The guard lives here rather than in
/// `platform::tests` because the flag's own unit tests are a sibling module and must take the
/// same lock; a per-module guard cannot express that.
#[cfg(test)]
pub(crate) static TEST_SEAM_SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_request_is_recorded_and_consumed_once() {
        let _serial = TEST_SEAM_SERIAL.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        // The flag is process-wide, so this test states its own end state rather than assuming
        // one: take() first, then assert the record, the read and the single consumption.
        let _ = take();
        assert!(!is_requested(), "a taken request must not still read as pending");

        let _ = request();
        assert!(is_requested(), "request() must record the request");
        assert!(take(), "the recorded request must be consumable");
        assert!(!take(), "a request stops one loop, not every loop after it");
    }

    #[test]
    fn a_stop_request_does_not_depend_on_which_thread_asked() {
        let _serial = TEST_SEAM_SERIAL.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        // The whole point of the flag: asking from a thread that does not own any loop must not
        // panic, must not call into a toolkit, and must leave the request where a loop reads it.
        let _ = take();
        let asked = std::thread::spawn(request).join().expect("the asking thread must not panic");
        // On a host where a loop has already ticked (any test binary that ran one), the asker is
        // told a reader exists; where none has, it is told the truth about that too. Both are
        // the same answer as this thread gets, which is what "independent of the asker" means.
        assert_eq!(
            asked,
            loop_has_ticked(),
            "the answer must describe the loop, not the thread that asked"
        );
        assert!(take());
        assert!(!is_requested());
    }
}
