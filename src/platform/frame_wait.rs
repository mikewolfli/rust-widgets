// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Bounded, deadline-paced wait between backend frame iterations.
//!
//! # Why this exists
//!
//! Every backend loop used to end an iteration with `thread::sleep(FRAME_INTERVAL_MS)`: 62.5
//! wakeups per second even when nothing was animating and no input had arrived. The frame driver
//! already answers whether a frame is owed
//! ([`FrameOutcome::needs_another_frame`](crate::widget::runtime::FrameOutcome)); the missing
//! piece was a way to *block* when it is not.
//!
//! # Deadline pacing
//!
//! Sleeping for a fixed interval measures from the end of the frame's work, so the period became
//! `work + 16 ms` and the error accumulated frame after frame. The wait now schedules against a
//! deadline grid: the next frame is due at the previous target plus [`FRAME_INTERVAL_MS`], which
//! keeps work that fits inside the interval from stretching it. A frame that misses its target is
//! caught up exactly once -- the next iteration runs immediately -- and the grid resumes a full
//! interval later. Missed deadlines are never replayed as a burst.
//!
//! # Wake sources and the backstop
//!
//! [`wake`] is called when something a still loop must react to happens: an event is posted, a
//! stop is requested, or cross-thread work is scheduled. [`pause_between_frames`] blocks until
//! such a wake or a bounded backstop, so state that has no wake channel (a device fact changing
//! on disk, a host mutating state directly) is still observed within
//! [`IDLE_FRAME_INTERVAL_MS`](crate::platform::types::IDLE_FRAME_INTERVAL_MS). A wake that
//! arrives while the frame is still in the future ends the wait without moving the deadline, so
//! input is handled at once and the cadence keeps its phase.
//!
//! # Presented-time pacing
//!
//! A backend that answers `true` from
//! [`Platform::supports_vsync_frame`](crate::platform::Platform::supports_vsync_frame) also
//! publishes an absolute presentation instant through
//! [`Platform::next_vsync_deadline`](crate::platform::Platform::next_vsync_deadline). The vsync
//! entry points below schedule against that instant instead of the fixed grid and report the
//! measured delta for `tick(delta_ms)`. A backend that answers `false` never reaches them, so its
//! behaviour is unchanged.
//!
//! # Platform note
//!
//! Win32 owns a native message queue: blocking on a condition variable there would delay input.
//! On Windows [`pause_between_frames`] blocks in `MsgWaitForMultipleObjectsEx` instead, which
//! returns as soon as a message arrives. A backend whose host owns the event queue uses
//! [`pause_between_frames_with`], which applies the same deadline grid to the timeout it hands to
//! that wait.

use core::cell::Cell;
use core::time::Duration;
use std::time::Instant;

use crate::platform::types::{FRAME_INTERVAL_MS, IDLE_FRAME_INTERVAL_MS};

#[cfg(not(alloc_frugal))]
mod inner {
    use core::sync::atomic::{AtomicU64, Ordering};
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
    /// Returns `true` when a [`wake`] arrived before `timeout` (the wait was cut short) and
    /// `false` when the whole timeout elapsed.
    ///
    /// Win32 is included: the backend loop pumps its own message queue in
    /// [`pause_between_frames`](super::pause_between_frames), but the event loop runs on its own
    /// thread and may block here just like any other waiter.
    ///
    /// Consumed by each backend's `pause_between_frames`. A build with no platform backend (the
    /// `windows desktop-runtime` cell in `check_profiles`) compiles this module without a caller,
    /// so the allow is scoped to this function rather than to the module: `wake`/`generation`/
    /// `wait_after` still warn if their wiring disappears.
    #[allow(dead_code)]
    pub(crate) fn wait_timeout(timeout: Duration) -> bool {
        let state = state();
        let observed = state.generation.load(Ordering::SeqCst);
        let guard = state.mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.generation.load(Ordering::SeqCst) != observed {
            return true;
        }
        let (_guard, result) = state
            .ready
            .wait_timeout(guard, timeout)
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        !result.timed_out()
    }

    /// The current wake generation. Pair with [`wait_after`].
    pub(crate) fn generation() -> u64 {
        state().generation.load(Ordering::SeqCst)
    }

    /// Blocks until a [`wake`] happens **after** `observed`, or `timeout` elapses.
    ///
    /// # Why the generation is passed in
    ///
    /// A caller that checks its own queue and then waits has a window between the check and the
    /// wait. Taking the generation *before* the check closes it: a post that lands in the window
    /// has already bumped the generation, so this returns at once instead of sleeping until the
    /// timeout (the lost-wakeup delay the plain [`wait_timeout`] would pay).
    pub(crate) fn wait_after(observed: u64, timeout: Duration) {
        let state = state();
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

    /// The allocation-frugal profile has no animation bus and no wake channel, so the loop
    /// simply paces itself and every wait runs to its full timeout. `false` says "not woken
    /// early", which is the honest answer for this profile.
    pub(crate) fn wait_timeout(timeout: Duration) -> bool {
        std::thread::sleep(timeout);
        false
    }
}

pub(crate) use inner::wake;

/// The wake generation and the generation-guarded wait are only meaningful where a wake
/// channel exists; the allocation-frugal profile's host loop has none and does not call them.
#[cfg(not(alloc_frugal))]
pub(crate) use inner::{generation, wait_after};

// The frame grid of the backend loop running on the current thread.
//
// The state is kept per thread because pacing belongs to the thread that owns the loop: a
// second loop, or a unit test, must not inherit another loop's phase.
thread_local! {
    static PACING: Cell<Pacing> = const { Cell::new(Pacing { next_deadline: None }) };
    /// Presented-time phase for the same loop. Kept apart from [`PACING`] so the fixed grid
    /// stays untouched on a backend whose vsync signal disappears mid-session.
    ///
    /// Only the macOS backend consumes this today; every other build compiles the vsync path so
    /// the trait contract and its tests stay platform-independent.
    #[allow(dead_code)]
    static VSYNC_PACING: Cell<VsyncPacing> =
        const { Cell::new(VsyncPacing { last_presented: None }) };
}

/// The deadline grid shared by the pause calls of one backend loop.
#[derive(Debug, Clone, Copy, Default)]
struct Pacing {
    /// The instant at which the next owed frame should run.
    next_deadline: Option<Instant>,
}

impl Pacing {
    /// Returns the deadline of the next owed frame and how long to wait for it.
    ///
    /// The deadline is carried over from the previous call rather than derived from `now`, so
    /// the wait absorbs the frame's work time instead of adding it to the period.
    fn plan(&self, now: Instant, interval: Duration) -> (Instant, Duration) {
        let deadline = match self.next_deadline {
            Some(deadline) => deadline,
            // No grid yet: the frame that just ran is followed one interval later, never by an
            // immediate second frame.
            None => now.checked_add(interval).unwrap_or(now),
        };
        (deadline, deadline.saturating_duration_since(now))
    }

    /// Records where the wait for `deadline` ended.
    ///
    /// `behind` is true when the frame was already due before the wait started. That is the
    /// single catch-up path: the following frame is scheduled one interval after it starts, so a
    /// stall cannot shed frames back to back.
    fn settle(&mut self, deadline: Instant, ran_at: Instant, behind: bool, interval: Duration) {
        self.next_deadline = Some(if behind {
            ran_at.checked_add(interval).unwrap_or(ran_at)
        } else {
            // Anchored to the target, not to the clock reading after the wake: scheduler jitter
            // then stays local to one frame instead of accumulating.
            deadline.checked_add(interval).unwrap_or(ran_at)
        });
    }

    /// Keeps the existing deadline: a wake that arrived early does not consume the slot.
    fn resume(&mut self, deadline: Instant) {
        self.next_deadline = Some(deadline);
    }

    /// Clears the grid while no frame is owed; the next active pause seeds a fresh phase.
    fn idle(&mut self) {
        self.next_deadline = None;
    }
}

/// The presented-time phase shared by the vsync calls of one backend loop.
///
/// Only the macOS backend drives this today; the dead-code allowance keeps the shared module
/// warning-free on hosts whose frame loop still uses the fixed grid, while the tests below cover
/// every branch on every host.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, Default)]
struct VsyncPacing {
    /// The presented time of the newest frame the loop accounted for.
    last_presented: Option<Instant>,
}

#[allow(dead_code)]
impl VsyncPacing {
    /// How long to wait for `deadline` from `now`.
    ///
    /// The deadline is the backend's absolute presentation instant, so the wait shrinks with the
    /// frame's work time instead of adding it to the period.
    fn wait_for(&self, now: Instant, deadline: Instant) -> Duration {
        deadline.saturating_duration_since(now)
    }

    /// Records a presented frame and returns the measured delta for `tick`, in milliseconds.
    ///
    /// The first frame has no predecessor, so it reports one nominal interval rather than a
    /// fabricated zero. A presented time that moved backwards saturates at zero: the measurement
    /// is honest about the delta and the next deadline remains the backend's absolute value, so a
    /// reordered reading cannot accumulate into a growing offset.
    fn observe(&mut self, presented: Instant) -> u32 {
        let delta = match self.last_presented {
            Some(previous) => presented.saturating_duration_since(previous),
            None => Duration::from_millis(FRAME_INTERVAL_MS),
        };
        self.last_presented = Some(presented);
        u32::try_from(delta.as_millis()).unwrap_or(u32::MAX)
    }

    /// Clears the phase while no frame is owed; the next active frame starts a fresh measurement.
    fn idle(&mut self) {
        self.last_presented = None;
    }
}

/// Reads the measured presented-time delta for `deadline`, or the nominal interval when the
/// backend has no live deadline.
///
/// This is the non-blocking half of the vsync path: the host's own loop (an AppKit timer, for
/// example) calls it once per iteration and hands the answer to `tick(delta_ms)`. Passing `None`
/// clears the phase, so a backend that loses its presentation signal and later recovers starts
/// from one nominal interval instead of a delta spanning the outage.
#[allow(dead_code)] // Consumed by the macOS frame heartbeat; other hosts have no deadline yet.
pub(crate) fn presented_delta_ms(deadline: Option<Instant>) -> u32 {
    VSYNC_PACING.with(|cell| {
        let mut pacing = cell.get();
        let delta = match deadline {
            // A timer that fired late is presenting now, not at the deadline it missed, so the
            // overrun belongs to this delta; an early fire keeps the future deadline as the
            // presentation instant.
            Some(deadline) => pacing.observe(deadline.max(Instant::now())),
            None => {
                pacing.idle();
                FRAME_INTERVAL_MS as u32
            }
        };
        cell.set(pacing);
        delta
    })
}

/// Reads the current thread's frame grid.
fn load_pacing() -> Pacing {
    PACING.with(|pacing| pacing.get())
}

/// Stores the current thread's frame grid.
fn store_pacing(pacing: Pacing) {
    PACING.with(|cell| cell.set(pacing));
}

/// Waits for the next iteration of a backend loop.
///
/// A frame that is owed is paced to the next deadline: the previous target plus
/// [`FRAME_INTERVAL_MS`]. Drift therefore does not accumulate, and a frame that is already late
/// is caught up exactly once -- the next iteration runs immediately, the one after it waits a
/// full interval. Missed deadlines are never replayed as a burst.
///
/// A still loop blocks for a wake with a bounded backstop, which is what removes the idle
/// busy-poll. An early wake ends the wait without moving the deadline.
pub(crate) fn pause_between_frames(needs_another_frame: bool) {
    pause_between_frames_with(needs_another_frame, block_for);
}

/// [`pause_between_frames`] with the blocking primitive supplied by the caller.
///
/// A backend whose host owns the event queue must wait in that queue rather than on this
/// module's condition variable, or input would be delayed. `block` is called once per pause;
/// its timeout is zero when the frame is already due, which lets a host-owned queue still be
/// polled without waiting. It must return `true` when it returned before a positive timeout
/// elapsed (an event or a stop request woke it) and `false` when the timeout expired; the
/// answer for a zero timeout is ignored.
pub(crate) fn pause_between_frames_with<F>(needs_another_frame: bool, block: F)
where
    F: FnOnce(Duration) -> bool,
{
    let interval = Duration::from_millis(FRAME_INTERVAL_MS);
    let mut pacing = load_pacing();
    if !needs_another_frame {
        // Nothing is owed. Clear the grid so the first animated frame after this idle period
        // starts a fresh phase, and block only for the bounded backstop.
        pacing.idle();
        store_pacing(pacing);
        let _ = block(Duration::from_millis(IDLE_FRAME_INTERVAL_MS));
        return;
    }

    let now = Instant::now();
    let (deadline, wait) = pacing.plan(now, interval);
    if wait.is_zero() {
        // Already at or past the deadline: this iteration is the catch-up. Poll the host's
        // queue with a zero timeout rather than skipping the call, so a backend that owns its
        // event pump still drains it while frames are running long. Do not wait, and take the
        // following frame a full interval from here so there is only one catch-up.
        let _ = block(Duration::ZERO);
        pacing.settle(deadline, now, true, interval);
        store_pacing(pacing);
        return;
    }

    if block(wait) {
        // The frame is due later, but an event arrived first. Run the frame now and keep the
        // deadline where it was: waking early must not shift the cadence.
        pacing.resume(deadline);
        store_pacing(pacing);
        return;
    }
    // The wait ran to the deadline. The next target is anchored to that deadline rather than
    // to the slightly later clock reading after the wake, so jitter cannot accumulate frame
    // after frame.
    pacing.settle(deadline, Instant::now(), false, interval);
    store_pacing(pacing);
}

/// [`pause_between_frames`] against a backend's presented-time deadline.
///
/// `next_deadline` is called once per iteration. When it answers `None` the backend has no live
/// display signal, so this falls back to the fixed grid and returns `None`; the caller then keeps
/// its existing [`FRAME_INTERVAL_MS`] delta. When it answers `Some`, the wait targets that absolute
/// instant and the returned value is the measured presented-time delta for `tick`.
///
/// A late frame runs at once and is caught up exactly once: the following wait targets the
/// backend's next absolute deadline, so missed deadlines are never replayed as a burst. A wake
/// that arrives before the deadline runs the frame with the clock-based delta and leaves the
/// deadline in place, so input stays immediate and the cadence keeps its phase.
// The blocking backend loops call the fixed-grid entry today; this is the entry a vsync-capable
// loop uses, and the tests below exercise every branch of it directly.
#[allow(dead_code)]
pub(crate) fn pause_between_frames_with_vsync<D, F>(
    needs_another_frame: bool,
    next_deadline: D,
    block: F,
) -> Option<u32>
where
    D: FnOnce() -> Option<Instant>,
    F: FnOnce(Duration) -> bool,
{
    if !needs_another_frame {
        // Nothing is owed. Drop the phase so the first animated frame after this idle period
        // measures from a fresh start, and block only for the bounded backstop.
        VSYNC_PACING.with(|cell| cell.set(VsyncPacing::default()));
        let _ = block(Duration::from_millis(IDLE_FRAME_INTERVAL_MS));
        return None;
    }

    let Some(deadline) = next_deadline() else {
        // No live deadline: the fixed grid is in charge, and the next vsync frame starts a new
        // presented-time phase.
        VSYNC_PACING.with(|cell| cell.set(VsyncPacing::default()));
        pause_between_frames_with(true, block);
        return None;
    };

    let now = Instant::now();
    let mut pacing = VSYNC_PACING.with(Cell::get);
    let wait = pacing.wait_for(now, deadline);
    if wait.is_zero() {
        // Already at or past the presentation instant: this iteration is the single catch-up.
        // Poll the host's queue with a zero timeout, then record the clock reading so the overrun
        // lands in this frame's delta exactly once. The next wait still targets the backend's
        // absolute deadline, so the overrun cannot push later frames off the grid.
        let _ = block(Duration::ZERO);
        let delta = pacing.observe(now);
        VSYNC_PACING.with(|cell| cell.set(pacing));
        return Some(delta);
    }

    if block(wait) {
        // An event woke the loop before the deadline. The frame runs now, so measure the delta
        // against the clock; the deadline itself is not consumed and the next wait takes only
        // what is left of it.
        let delta = pacing.observe(Instant::now());
        VSYNC_PACING.with(|cell| cell.set(pacing));
        return Some(delta);
    }

    let delta = pacing.observe(deadline);
    VSYNC_PACING.with(|cell| cell.set(pacing));
    Some(delta)
}

/// Blocks for at most `timeout`, reporting whether a wake cut the wait short.
#[cfg(all(windows, not(alloc_frugal)))]
fn block_for(timeout: Duration) -> bool {
    // Win32 has a real message queue. Blocking in it is what makes input latency zero while
    // idle CPU stays at zero; a condition-variable wait would delay `WM_*` messages.
    let millis = timeout_millis_ceil(timeout);
    // SAFETY: no handles are passed (`nCount == 0`, null pointer), so the call cannot read or
    // write any memory we own. `QS_ALLINPUT` waits for any input message, and
    // `MWMO_INPUTAVAILABLE` also returns for input already queued before the call.
    let wait_result = unsafe {
        winapi::um::winuser::MsgWaitForMultipleObjectsEx(
            0,
            core::ptr::null(),
            millis,
            winapi::um::winuser::QS_ALLINPUT,
            winapi::um::winuser::MWMO_INPUTAVAILABLE,
        )
    };
    // Anything other than a timeout means input arrived before the deadline, which is exactly
    // the early wake the pacing grid must preserve.
    wait_result != winapi::um::winbase::WAIT_TIMEOUT
}

/// Blocks for at most `timeout`, reporting whether a wake cut the wait short.
#[cfg(not(all(windows, not(alloc_frugal))))]
fn block_for(timeout: Duration) -> bool {
    if timeout.is_zero() {
        // Nothing to wait for; skip the condition-variable round trip.
        return false;
    }
    inner::wait_timeout(timeout)
}

/// Rounds a wait up to whole milliseconds for the Win32 message wait.
///
/// Truncating would wake a fraction of a millisecond early on every frame; the remainder would
/// then become a zero-length second wait, which is the spin this module exists to remove.
#[cfg(all(windows, not(alloc_frugal)))]
fn timeout_millis_ceil(timeout: Duration) -> u32 {
    let millis = timeout.as_micros().div_ceil(1_000);
    u32::try_from(millis).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::Vec;

    /// The pacing interval the tests schedule against.
    fn interval() -> Duration {
        Duration::from_millis(FRAME_INTERVAL_MS)
    }

    /// Advances a simulated clock from `base` by `millis`.
    fn at(base: Instant, millis: u64) -> Instant {
        base + Duration::from_millis(millis)
    }

    /// Runs a deterministic frame loop against the pacing grid.
    ///
    /// `work` returns how long the frame at that index takes. `waits` holds the wait each pause
    /// asked for and `starts` the instant the following frame started. No clock is read and
    /// nothing sleeps, so every expectation on the returned values is exact.
    fn simulate(
        frames: usize,
        mut work: impl FnMut(usize) -> u64,
    ) -> (Vec<Duration>, Vec<Instant>) {
        let base = Instant::now();
        let mut now = base;
        let mut pacing = Pacing::default();
        let mut waits = Vec::with_capacity(frames);
        let mut starts = Vec::with_capacity(frames + 1);
        starts.push(now);
        for index in 0..frames {
            now += Duration::from_millis(work(index));
            let (deadline, wait) = pacing.plan(now, interval());
            waits.push(wait);
            if !wait.is_zero() {
                // A simulated wait that runs to its deadline lands exactly on it.
                now = deadline;
            }
            pacing.settle(deadline, now, wait.is_zero(), interval());
            starts.push(now);
        }
        (waits, starts)
    }

    #[test]
    fn the_frame_interval_is_unchanged() {
        // The deadline grid is built from this value; changing it would change the cadence
        // even if the scheduler itself were correct.
        assert_eq!(FRAME_INTERVAL_MS, 16);
    }

    #[test]
    fn a_steady_loop_waits_exactly_one_interval_per_frame() {
        // Work that fits inside the interval must not stretch it: starts stay on the grid and
        // every later wait is the unused remainder of the interval.
        let (waits, starts) = simulate(8, |_| 4);
        // The first frame has no previous target, so its pause is a full interval measured from
        // the moment the pause begins. Every later frame is anchored to the grid, so the work
        // time is absorbed by a shorter wait and the start-to-start period is exactly one interval.
        for pair in starts.windows(2).skip(1) {
            assert_eq!(pair[1] - pair[0], interval(), "frame starts drifted off the grid");
        }
        assert_eq!(waits[0], interval(), "the first frame must get a full interval");
        for wait in &waits[1..] {
            assert_eq!(*wait, interval() - Duration::from_millis(4));
        }
    }

    #[test]
    fn a_late_frame_does_not_accumulate_drift() {
        // One frame overruns the interval. After the single catch-up the grid must be back on
        // 16 ms spacing: the overrun is absorbed once, not added to every later frame.
        let (waits, starts) = simulate(12, |index| if index == 4 { 40 } else { 4 });
        assert_eq!(waits.iter().filter(|wait| wait.is_zero()).count(), 1);
        let catch_up = waits.iter().position(|wait| wait.is_zero()).expect("no catch-up") + 1;
        for pair in starts[catch_up..].windows(2) {
            assert_eq!(pair[1] - pair[0], interval(), "the overrun leaked into later frames");
        }
    }

    #[test]
    fn a_late_frame_is_caught_up_exactly_once() {
        // A stall must not produce a burst: every deadline that was missed while the frame ran
        // is skipped, and only the frame that runs immediately at the end of the stall is the
        // catch-up.
        let (waits, _) = simulate(40, |index| if index == 10 { 40 } else { 2 });
        assert_eq!(waits.iter().filter(|wait| wait.is_zero()).count(), 1);
    }

    #[test]
    fn a_steady_run_never_spins_on_zero_waits() {
        // A busy loop would show up as zero-length waits. None are permitted while the frame
        // work fits the interval.
        let (waits, _) = simulate(240, |_| 3);
        assert!(waits.iter().all(|wait| !wait.is_zero()), "the loop spun instead of blocking");
        for wait in &waits[1..] {
            assert_eq!(*wait, interval() - Duration::from_millis(3));
        }
    }

    #[test]
    fn an_early_wake_returns_immediately_and_keeps_the_deadline() {
        let base = Instant::now();
        let mut pacing = Pacing::default();

        let (deadline, wait) = pacing.plan(base, interval());
        assert_eq!(wait, interval());
        // Returning immediately is the `resume` path: the deadline is carried over untouched.
        pacing.resume(deadline);

        let woke_at = at(base, 4);
        let (deadline_again, remaining) = pacing.plan(woke_at, interval());
        assert_eq!(deadline_again, deadline, "an early wake moved the deadline");
        assert_eq!(remaining, interval() - Duration::from_millis(4));
    }

    #[test]
    fn an_idle_pause_clears_the_grid() {
        let base = Instant::now();
        let mut pacing = Pacing::default();
        let (deadline, _) = pacing.plan(base, interval());
        pacing.settle(deadline, at(base, FRAME_INTERVAL_MS), false, interval());
        assert!(pacing.next_deadline.is_some());

        pacing.idle();
        let (fresh, wait) = pacing.plan(base, interval());
        assert_eq!(fresh, at(base, FRAME_INTERVAL_MS));
        assert_eq!(wait, interval());
    }

    #[test]
    fn the_blocking_primitive_gets_the_remaining_deadline() {
        store_pacing(Pacing::default());
        let requested = Cell::new(Duration::ZERO);
        pause_between_frames_with(true, |timeout| {
            requested.set(timeout);
            true
        });
        assert_eq!(requested.get(), interval(), "the first owed frame must wait one interval");

        // The early wake above kept the deadline, so the second pause asks for the remainder
        // of the same interval rather than starting a new one.
        let remaining = Cell::new(Duration::ZERO);
        pause_between_frames_with(true, |timeout| {
            remaining.set(timeout);
            true
        });
        assert!(remaining.get() > Duration::ZERO);
        assert!(remaining.get() <= interval());
    }

    #[test]
    fn an_idle_pause_waits_only_for_the_backstop_and_rebases() {
        store_pacing(Pacing::default());
        pause_between_frames_with(true, |_| true);

        let idle = Cell::new(Duration::ZERO);
        pause_between_frames_with(false, |timeout| {
            idle.set(timeout);
            false
        });
        assert_eq!(idle.get(), Duration::from_millis(IDLE_FRAME_INTERVAL_MS));

        let fresh = Cell::new(Duration::ZERO);
        pause_between_frames_with(true, |timeout| {
            fresh.set(timeout);
            false
        });
        assert_eq!(fresh.get(), interval(), "the idle pause did not rebase the grid");
    }

    #[test]
    fn a_backend_without_vsync_keeps_the_fixed_grid() {
        // The default `supports_vsync_frame` answer is `false`; such a backend never reaches the
        // presented-time path, so this is the unchanged fixed-grid contract.
        store_pacing(Pacing::default());
        let requested = Cell::new(Duration::ZERO);
        pause_between_frames_with(true, |timeout| {
            requested.set(timeout);
            false
        });
        assert_eq!(requested.get(), interval());
    }

    #[test]
    fn a_mock_vsync_deadline_produces_the_expected_wait() {
        let base = Instant::now();
        let mut pacing = VsyncPacing::default();
        let first = at(base, FRAME_INTERVAL_MS);

        assert_eq!(pacing.wait_for(base, first), interval());
        assert_eq!(pacing.observe(first), FRAME_INTERVAL_MS as u32);

        // Work that takes 4 ms is absorbed by a shorter wait; the period stays one interval.
        let after_work = at(first, 4);
        let second = at(first, FRAME_INTERVAL_MS);
        assert_eq!(pacing.wait_for(after_work, second), interval() - Duration::from_millis(4));
        assert_eq!(pacing.observe(second), FRAME_INTERVAL_MS as u32);
    }

    #[test]
    fn a_mock_vsync_deadline_paces_the_blocking_wait() {
        // The backend's absolute deadline, not the 16 ms grid, decides the timeout. The mock is
        // far enough away that scheduler noise cannot make the assertion ambiguous.
        store_pacing(Pacing::default());
        VSYNC_PACING.with(|cell| cell.set(VsyncPacing::default()));
        let mock = Duration::from_millis(200);
        let deadline = Instant::now() + mock;
        let requested = Cell::new(Duration::ZERO);

        let delta = pause_between_frames_with_vsync(
            true,
            || Some(deadline),
            |timeout| {
                requested.set(timeout);
                false
            },
        );

        assert!(delta.is_some(), "a live deadline must report a measured delta");
        assert!(requested.get() > interval(), "the fixed grid overrode the mock deadline");
        assert!(requested.get() <= mock, "the wait ran past the mock deadline");
    }

    #[test]
    fn early_presented_frames_do_not_accumulate_drift() {
        // A display that presents 12 ms early must not pull the grid forward frame after frame:
        // every wait is measured from the previous absolute deadline, so the total equals the
        // final deadline minus the first clock reading.
        let base = Instant::now();
        let mut pacing = VsyncPacing::default();
        let mut deadline = at(base, 12);
        let mut now = at(base, 3);
        let mut waited = Duration::ZERO;

        assert_eq!(pacing.wait_for(now, deadline), Duration::from_millis(9));
        waited += Duration::from_millis(9);
        assert_eq!(pacing.observe(deadline), FRAME_INTERVAL_MS as u32);
        now = deadline;

        for _ in 1..5 {
            deadline += Duration::from_millis(12);
            let wait = pacing.wait_for(now, deadline);
            assert_eq!(wait, Duration::from_millis(12));
            waited += wait;
            assert_eq!(pacing.observe(deadline), 12);
            now = deadline;
        }

        assert_eq!(waited, at(base, 60).saturating_duration_since(at(base, 3)));
    }

    #[test]
    fn late_presented_frames_do_not_accumulate_drift() {
        // One frame overruns a whole interval. The measured delta reports the overrun once, and
        // the following deadline is still the display's absolute grid -- it is not shifted by
        // the overrun.
        let base = Instant::now();
        let mut pacing = VsyncPacing::default();
        let first = at(base, FRAME_INTERVAL_MS);
        assert_eq!(pacing.wait_for(base, first), interval());
        assert_eq!(pacing.observe(first), FRAME_INTERVAL_MS as u32);

        // The second frame starts at 34 ms, 2 ms after its 32 ms deadline. The measured delta
        // reports that 18 ms at once, and the next deadline is still the absolute grid point at
        // 48 ms rather than 34 + 16.
        let late_start = at(base, 34);
        let missed = at(base, 32);
        assert_eq!(pacing.wait_for(late_start, missed), Duration::ZERO);
        assert_eq!(pacing.observe(late_start), 18);

        let next = at(base, 48);
        assert_eq!(pacing.wait_for(late_start, next), Duration::from_millis(14));
        assert_eq!(pacing.observe(next), 14);

        // And the one after it is 64 ms: the overrun leaked into no later delta.
        let after = at(base, 52);
        let following = at(base, 64);
        assert_eq!(pacing.wait_for(after, following), Duration::from_millis(12));
        assert_eq!(pacing.observe(following), FRAME_INTERVAL_MS as u32);
    }

    #[test]
    fn a_missing_vsync_deadline_falls_back_to_the_fixed_grid() {
        store_pacing(Pacing::default());
        VSYNC_PACING.with(|cell| cell.set(VsyncPacing::default()));
        let requested = Cell::new(Duration::ZERO);

        let delta = pause_between_frames_with_vsync(
            true,
            || None,
            |timeout| {
                requested.set(timeout);
                false
            },
        );

        assert_eq!(delta, None, "a backend without a deadline must not report a vsync delta");
        assert_eq!(requested.get(), interval(), "the fixed grid must still pace the frame");
    }

    #[test]
    fn an_early_vsync_wake_measures_elapsed_time_without_shifting_the_grid() {
        store_pacing(Pacing::default());
        VSYNC_PACING.with(|cell| cell.set(VsyncPacing::default()));
        let deadline = Instant::now() + interval();

        let woke = pause_between_frames_with_vsync(true, || Some(deadline), |_| true)
            .expect("an early wake must still report a measured delta");
        assert!(woke <= FRAME_INTERVAL_MS as u32, "the delta exceeded one interval");

        // The deadline was not consumed: the next wait asks only for what remains of it.
        let remaining = Cell::new(Duration::ZERO);
        let _ = pause_between_frames_with_vsync(
            true,
            || Some(deadline),
            |timeout| {
                remaining.set(timeout);
                false
            },
        );
        assert!(remaining.get() > Duration::ZERO);
        assert!(remaining.get() <= interval());
    }

    #[test]
    fn presented_delta_reports_the_measured_period() {
        VSYNC_PACING.with(|cell| cell.set(VsyncPacing::default()));
        // Two-second steps keep the tolerance independent of test scheduling: the deadlines stay
        // in the future even if the test thread is descheduled for a moment.
        let base = Instant::now();
        let step = Duration::from_secs(2);
        let first = base + step;
        let second = first + step;
        let third = second + step;

        assert_eq!(presented_delta_ms(Some(first)), FRAME_INTERVAL_MS as u32);
        assert_eq!(presented_delta_ms(Some(second)), 2_000);
        // Losing the signal clears the phase instead of carrying a stale predecessor.
        assert_eq!(presented_delta_ms(None), FRAME_INTERVAL_MS as u32);
        assert_eq!(presented_delta_ms(Some(third)), FRAME_INTERVAL_MS as u32);
    }

    #[test]
    fn a_frame_that_is_already_due_gets_a_zero_timeout() {
        store_pacing(Pacing { next_deadline: Some(Instant::now()) });
        let timeout = Cell::new(interval());
        pause_between_frames_with(true, |requested| {
            timeout.set(requested);
            false
        });
        assert_eq!(timeout.get(), Duration::ZERO, "a frame that is already due must not wait");

        // The catch-up re-anchors the grid, so the iteration after it waits again instead of
        // spinning through a backlog.
        let next = Cell::new(Duration::ZERO);
        pause_between_frames_with(true, |requested| {
            next.set(requested);
            false
        });
        assert!(next.get() > Duration::ZERO);
        assert!(next.get() <= interval());
    }
}
