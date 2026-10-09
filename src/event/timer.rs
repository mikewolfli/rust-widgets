// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Runtime timer manager that emits `Event::Timer` into the event queue.
use super::event_queue::EventSender;
#[cfg(not(alloc_frugal))]
use super::types::Event;
#[cfg(not(alloc_frugal))]
use crate::compat::Condvar;
use crate::compat::{format, lock, Box, HashMap, Instant, MiniToString, Mutex, String, Vec};
use crate::core::ObjectId;
use alloc::sync::Arc;
use core::time::Duration;
#[cfg(not(alloc_frugal))]
use std::thread;
struct TimerEntry {
    interval: Duration,
    repeating: bool,
    next_fire: Instant,
}

/// Computes `Instant::now() + interval`, returning `Err` instead of panicking when the
/// deadline is not representable on the platform clock.
///
/// `compat::Instant` is `std::time::Instant` on desktop (which has `checked_add`) and a thin
/// wrapper on the frugal profile (which does not), so the two arms meet here rather than
/// branching at the call site.
#[cfg(not(alloc_frugal))]
fn next_fire_deadline(interval: Duration) -> Result<Instant, String> {
    Instant::now()
        .checked_add(interval)
        .ok_or_else(|| format!("timer interval {interval:?} overflows the platform clock"))
}

/// The frugal spelling of [`next_fire_deadline`].
#[cfg(alloc_frugal)]
fn next_fire_deadline(interval: Duration) -> Result<Instant, String> {
    // The frugal `Instant` wrapper has no `checked_add` and its `Add` impl panics on overflow.
    // A monotonic clock on every supported target can represent far more than this ceiling, so
    // rejecting the `Duration::MAX`-scale tail here keeps the `Add` below panic-free without
    // naming a platform-specific limit.
    const CEILING: Duration = Duration::from_secs(10 * 365 * 24 * 3600);
    if interval > CEILING {
        return Err(format!("timer interval {interval:?} overflows the platform clock"));
    }
    Ok(Instant::now() + interval)
}

#[derive(Default)]
struct TimerState {
    timers: HashMap<(ObjectId, u32), TimerEntry>,
    running: bool,
}

/// Shared timer state plus the signal the worker waits on (D09-EVT-05).
///
/// # Why the worker no longer polls every 2 ms
///
/// The old worker slept a fixed 2 ms between scans, so an idle `EventLoop` kept a
/// thread waking ~500 times per second and re-locking the timer map even with **zero**
/// timers registered, and the scan cost grew linearly with the number of live timers.
/// The signal below lets the worker block until something actually changes.
///
/// The worker waits on `state`'s monitor under `signal`. Every mutation of the timer
/// set (`start_timer`, `stop_timer`, `stop_timers_for_target`, `clear`, and the drop of
/// the manager's last handle) notifies it, so:
///
/// * with no timers, it parks on the monitor until a timer is added or the manager stops;
/// * with timers, it parks at most until the earliest deadline (`wait_timeout`), so a due
///   timer is not delayed beyond the clock resolution;
/// * a concurrent add/remove/stop interrupts the park immediately rather than waiting
///   out the previous deadline.
///
/// The classic condition-variable pattern is preserved: the flag (`running`) and the
/// set (which timers exist) are only read while holding the same mutex the notifier
/// holds when it changes them, so no wake-up can be lost between a state change and the
/// park.
#[cfg(not(alloc_frugal))]
struct TimerShared {
    state: Mutex<TimerState>,
    signal: Condvar,
    /// Number of worker iterations observed since the manager was created.
    ///
    /// This is a diagnostic counter, and it is what lets a regression test assert that an
    /// idle manager **stops polling** rather than waking on a fixed interval (D09-EVT-05):
    /// a parked worker does not increment it, whereas the old 2 ms scan did. It is kept
    /// uncompiled-out so the timer producer (`Event::timer`) that the event-variant gate
    /// scans for stays in this file's production region; the single relaxed atomic
    /// increment per worker iteration is negligible next to the work the worker does.
    #[allow(dead_code)]
    wake_count: core::sync::atomic::AtomicU64,
}

#[cfg(not(alloc_frugal))]
impl TimerShared {
    /// Wakes the worker after a state change (a timer added, removed, or stopped).
    fn notify(&self) {
        self.signal.notify_one();
    }

    /// The earliest `next_fire` among the registered timers, if any.
    ///
    /// Called with the state lock held. `None` means "no timer to wait for", which is
    /// what makes the worker block indefinitely instead of polling.
    fn nearest_deadline(state: &TimerState) -> Option<Instant> {
        state.timers.values().map(|entry| entry.next_fire).min()
    }
}

/// Emits timer events into the event queue for one-shot and repeating timers.
pub struct TimerManager {
    #[cfg(not(alloc_frugal))]
    state: Arc<TimerShared>,
    #[cfg(alloc_frugal)]
    state: Arc<Mutex<TimerState>>,
    #[cfg(not(alloc_frugal))]
    thread_handle: Option<thread::JoinHandle<()>>,
    /// Field kept in `mini` so the shared method bodies compile unchanged.
    ///
    /// `mini` has no timer thread — the caller drives `pump`, so there is no handle to
    /// join. This mirrors the threaded field rather than forking every method, which is
    /// the same profile-parity pattern `EventLoop` uses.
    #[cfg(alloc_frugal)]
    #[cfg_attr(alloc_frugal, allow(dead_code))]
    thread_handle: Option<()>,
    /// Sender used by the mini `pump` to post due timer events.
    #[cfg(alloc_frugal)]
    sender: EventSender,
}

impl TimerManager {
    /// Create a timer manager bound to an event sender.
    #[cfg(not(alloc_frugal))]
    pub fn new(sender: EventSender) -> Self {
        let shared = Arc::new(TimerShared {
            state: Mutex::new(TimerState { timers: HashMap::new(), running: true }),
            signal: Condvar::new(),
            wake_count: core::sync::atomic::AtomicU64::new(0),
        });

        let worker_shared = Arc::clone(&shared);
        let worker_sender = sender;
        // D09-EVT-05: the worker parks on the condvar instead of polling.
        //
        // * No timers -> `wait` (unbounded) until notified. An idle manager therefore
        //   performs **no** periodic wake-ups.
        // * Timers present -> `wait_timeout` for the nearest deadline, so a short timer
        //   fires as soon as it is due rather than on a fixed 2 ms tick, and a late
        //   notification recomputes the wait rather than delaying the next check.
        let thread_handle = thread::spawn(move || loop {
            worker_shared.wake_count.fetch_add(1, core::sync::atomic::Ordering::SeqCst);
            let mut due_events: Vec<(ObjectId, u32)> = Vec::new();

            {
                let mut guard = lock(&worker_shared.state);
                if !guard.running {
                    return;
                }

                let now = Instant::now();

                // Collect everything already due and advance repeating deadlines.
                let keys: Vec<(ObjectId, u32)> = guard.timers.keys().copied().collect();
                for key in keys {
                    if let Some(entry) = guard.timers.get_mut(&key) {
                        if now >= entry.next_fire {
                            due_events.push(key);
                            if entry.repeating {
                                // Use Instant::now() + interval to avoid cumulative drift.
                                // If a timer fires late due to load, reset based on current time
                                // so subsequent firings are not delayed by the accumulated drift.
                                entry.next_fire = Instant::now() + entry.interval;
                            }
                        }
                    }
                }

                for &(target, id) in &due_events {
                    if let Some(entry) = guard.timers.get(&(target, id)) {
                        if !entry.repeating {
                            guard.timers.remove(&(target, id));
                        }
                    }
                }

                if due_events.is_empty() {
                    // Nothing to emit: park until the nearest deadline, or indefinitely when
                    // no timer is registered. `spurious_wakeup` loops rather than assuming
                    // the notification was the one we expected — the state is re-read under
                    // the lock each time, which is the condition-variable contract.
                    match TimerShared::nearest_deadline(&guard) {
                        Some(deadline) => {
                            let wait_for = deadline.saturating_duration_since(Instant::now());
                            // A deadline already in the past collapses to a zero timeout,
                            // which returns immediately; the next loop iteration then fires
                            // it, so no due timer is missed.
                            let _ = worker_shared.signal.wait_timeout(guard, wait_for);
                        }
                        None => {
                            guard = worker_shared
                                .signal
                                .wait(guard)
                                .unwrap_or_else(|poisoned| poisoned.into_inner());
                        }
                    }
                    continue;
                }
            }

            for (target, id) in due_events {
                if worker_sender.post(target, Event::timer(id)).is_err() {
                    lock(&worker_shared.state).timers.remove(&(target, id));
                }
            }
        });

        Self { state: shared, thread_handle: Some(thread_handle) }
    }

    /// Creates a timer manager for `mini`, which has no background thread.
    ///
    /// Timers fire when the caller drives `pump`, so this stores the sender that
    /// `pump` posts through instead of spawning. Single-threaded by design: `mini`
    /// carries no `Send`/`Sync` requirement on the queue.
    #[cfg(alloc_frugal)]
    pub fn new(sender: EventSender) -> Self {
        let state = Arc::new(Mutex::new(TimerState { timers: HashMap::new(), running: true }));
        Self { state, sender, thread_handle: None }
    }

    /// Acquire the lock on timer state, recovering from poisoning.
    #[cfg(not(alloc_frugal))]
    fn lock_timers(&self) -> crate::compat::MutexGuard<'_, TimerState> {
        lock(&self.state.state)
    }

    /// Acquire the lock on timer state, recovering from poisoning.
    #[cfg(alloc_frugal)]
    fn lock_timers(&self) -> crate::compat::MutexGuard<'_, TimerState> {
        lock(&self.state)
    }

    /// Wake the worker after a state change (D09-EVT-05).
    ///
    /// A no-op under `mini`, which has no worker thread and no condvar; the caller's
    /// `pump` observes the changed set directly.
    #[cfg(not(alloc_frugal))]
    fn notify_worker(&self) {
        self.state.notify();
    }

    /// Wake the worker after a state change (D09-EVT-05).
    #[cfg(alloc_frugal)]
    fn notify_worker(&self) {}

    /// Start or replace a timer for `(target, id)`.
    pub fn start_timer(
        &self,
        target: ObjectId,
        id: u32,
        interval: Duration,
        repeating: bool,
    ) -> Result<(), String> {
        if interval.is_zero() {
            return Err("timer interval must be > 0".to_string());
        }

        let entry = TimerEntry { interval, repeating, next_fire: next_fire_deadline(interval)? };
        self.lock_timers().timers.insert((target, id), entry);
        // The worker may be parked indefinitely (no timers) or until an older deadline;
        // an added timer can be nearer than that, so it must be woken to recompute.
        self.notify_worker();
        Ok(())
    }

    /// Stop one timer by `(target, id)`.
    pub fn stop_timer(&self, target: ObjectId, id: u32) -> bool {
        let removed = self.lock_timers().timers.remove(&(target, id)).is_some();
        if removed {
            // The stopped timer may have been the nearest deadline; the worker must
            // recompute rather than wake at a deadline that no longer exists.
            self.notify_worker();
        }
        removed
    }

    /// Stop all timers belonging to one target.
    pub fn stop_timers_for_target(&self, target: ObjectId) -> usize {
        let mut guard = self.lock_timers();
        let before = guard.timers.len();
        guard.timers.retain(|(timer_target, _), _| *timer_target != target);
        let removed = before.saturating_sub(guard.timers.len());
        drop(guard);
        if removed > 0 {
            // The nearest deadline may have belonged to this target.
            self.notify_worker();
        }
        removed
    }

    /// Remove all active timers.
    pub fn clear(&self) {
        let had_timers = {
            let mut guard = self.lock_timers();
            let had = !guard.timers.is_empty();
            guard.timers.clear();
            had
        };
        if had_timers {
            // Waking here turns "cleared" into a prompt recompute instead of the
            // worker sleeping out a deadline for a timer that no longer exists.
            self.notify_worker();
        }
    }

    /// Pump due timers synchronously, posting their `Event::Timer` events.
    ///
    /// Mini mode has no background worker thread, so callers (e.g. an event
    /// loop iteration) invoke this periodically to fire due timers.
    #[cfg(alloc_frugal)]
    pub fn pump(&self) {
        let now = Instant::now();
        let mut due_events = Vec::new();

        {
            let mut guard = lock(&self.state);
            if !guard.running {
                return;
            }

            let keys: Vec<(ObjectId, u32)> = guard.timers.keys().copied().collect();
            for key in keys {
                if let Some(entry) = guard.timers.get_mut(&key) {
                    if now >= entry.next_fire {
                        due_events.push(key);
                        if entry.repeating {
                            // Reset based on current time to avoid cumulative drift.
                            entry.next_fire = Instant::now() + entry.interval;
                        }
                    }
                }
            }

            for &(target, id) in &due_events {
                if let Some(entry) = guard.timers.get(&(target, id)) {
                    if !entry.repeating {
                        guard.timers.remove(&(target, id));
                    }
                }
            }
        }

        for (target, id) in due_events {
            let _ = self.sender.post(target, crate::event::types::Event::timer(id));
        }
    }
}

#[cfg(not(alloc_frugal))]
impl Drop for TimerManager {
    fn drop(&mut self) {
        {
            let mut guard = lock(&self.state.state);
            guard.running = false;
            guard.timers.clear();
        }
        // D09-EVT-05: the worker may be parked in `wait` (no timers) or `wait_timeout`,
        // so clearing `running` is not enough — it has to be woken to observe the flag,
        // or `join` below would block until its next deadline.
        self.state.signal.notify_all();
        // `mini` has no `JoinHandle` here — the field is `Option<()>` to keep this
        // body shared with the threaded arm — so there is nothing to join and no
        // worker to stop beyond clearing `running` above.
        if let Some(handle) = self.thread_handle.take() {
            if let Err(e) = handle.join() {
                log::error!("[timer-manager] Thread join failed: {e:?}");
            }
        }
    }
}

/// Idle task that runs when the event loop has no higher-priority events (BLUE11 R8.5).
///
/// Priority is expressed as a cooldown rather than a queue: the event loop ticks
/// the task every frame with [`IdleTask::tick`], and the callback actually runs
/// only once `threshold_frames` frames have elapsed since it last ran. A large
/// threshold therefore means "only when things are quiet".
pub struct IdleTask {
    /// Caller-assigned identifier. The task machinery never rewrites it, so it
    /// stays whatever `new` was given; it is the only stable way to refer to a
    /// task across ticks.
    pub id: u64,
    /// The work to perform. Called from the event loop with no arguments; it
    /// cannot report failure, so a task that fails must log internally.
    /// Boxed and `Send` because the manager owns tasks across threads.
    pub callback: Box<dyn FnMut() + Send>,
    /// Minimum number of frames between runs. `0` means the callback runs on
    /// every tick, which defeats the point of an idle task but is permitted.
    pub threshold_frames: u32,
    /// Frames elapsed since the last run. Managed by [`IdleTask::tick`]; public
    /// for inspection, but writing it changes when the next run happens.
    pub frames_since_run: u32,
}

impl IdleTask {
    /// Creates a task that has not yet run, so its first run happens after
    /// `threshold_frames` ticks rather than on the first one.
    pub fn new<F>(id: u64, threshold_frames: u32, callback: F) -> Self
    where
        F: FnMut() + Send + 'static,
    {
        Self { id, callback: Box::new(callback), threshold_frames, frames_since_run: 0 }
    }

    /// Called each frame by the event loop. Returns true if the task ran this frame.
    pub fn tick(&mut self) -> bool {
        self.frames_since_run += 1;
        if self.frames_since_run >= self.threshold_frames {
            self.frames_since_run = 0;
            (self.callback)();
            true
        } else {
            false
        }
    }
}

#[cfg(all(test, not(alloc_frugal), not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use crate::event::EventQueue;
    #[cfg(not(alloc_frugal))]
    use std::thread;
    #[cfg(not(alloc_frugal))]
    use std::time::Instant;

    #[test]
    fn one_shot_timer_emits_single_event() {
        let queue = EventQueue::new();
        let manager = TimerManager::new(queue.sender());
        manager
            .start_timer(7, 11, Duration::from_millis(20), false)
            .expect("one-shot timer should start");

        let deadline = Instant::now() + Duration::from_millis(400);
        let mut hit_count = 0usize;

        while Instant::now() < deadline {
            if let Some((target, event, _priority)) = queue.dequeue() {
                if target == 7 && matches!(event, Event::Timer { id: 11 }) {
                    hit_count += 1;
                }
            }
            thread::sleep(Duration::from_millis(2));
        }

        assert_eq!(hit_count, 1);
    }

    #[test]
    fn repeating_timer_can_be_stopped() {
        let queue = EventQueue::new();
        let manager = TimerManager::new(queue.sender());
        manager
            .start_timer(9, 3, Duration::from_millis(15), true)
            .expect("repeating timer should start");

        let deadline = Instant::now() + Duration::from_millis(300);
        let mut hits = 0usize;

        while Instant::now() < deadline && hits < 2 {
            if let Some((target, event, _priority)) = queue.dequeue() {
                if target == 9 && matches!(event, Event::Timer { id: 3 }) {
                    hits += 1;
                }
            }
            thread::sleep(Duration::from_millis(2));
        }

        assert!(hits >= 2);
        assert!(manager.stop_timer(9, 3));

        let post_stop_deadline = Instant::now() + Duration::from_millis(120);
        let mut post_stop_hits = 0usize;
        while Instant::now() < post_stop_deadline {
            if let Some((target, event, _priority)) = queue.dequeue() {
                if target == 9 && matches!(event, Event::Timer { id: 3 }) {
                    post_stop_hits += 1;
                }
            }
            thread::sleep(Duration::from_millis(2));
        }

        assert_eq!(post_stop_hits, 0);
    }

    /// An interval that overflows the platform clock is rejected as an `Err`, and the existing
    /// timer for the same key is not replaced.
    #[test]
    fn an_unrepresentable_interval_is_rejected_without_replacing() {
        let queue = EventQueue::new();
        let manager = TimerManager::new(queue.sender());
        manager.start_timer(5, 1, Duration::from_millis(10), false).expect("a valid timer starts");

        let result = manager.start_timer(5, 1, Duration::MAX, false);
        assert!(result.is_err(), "an unrepresentable time must be an Err, not a panic");
        assert!(manager.stop_timer(5, 1), "the previous timer must still be present");
    }

    /// D09-EVT-05: with zero timers the worker parks instead of polling every 2 ms.
    ///
    /// # The defect this pins
    ///
    /// The worker slept a fixed 2 ms each round, so an idle `TimerManager` woke ~500
    /// times per second and re-locked the timer map for nothing. The worker now blocks on
    /// the condition variable when the set is empty, so the number of worker iterations
    /// over a quiet window must stay at the handful needed to reach the park — not the
    /// dozens a 2 ms poll produces.
    #[test]
    fn idle_manager_does_not_poll_periodically() {
        let queue = EventQueue::new();
        let manager = TimerManager::new(queue.sender());

        // Give the worker time to take the lock once and park. A 2 ms poll would run
        // ~75 iterations in this window; a parked worker runs a small constant (with
        // headroom for the odd spurious wake-up).
        thread::sleep(Duration::from_millis(150));
        let iterations = manager.state.wake_count.load(core::sync::atomic::Ordering::SeqCst);
        assert!(
            iterations < 20,
            "an idle TimerManager must not poll periodically; it ran {iterations} worker \
             iterations in 150 ms, which is a fixed-interval scan rather than a park"
        );
    }

    /// D09-EVT-05: a short timer is delivered promptly once started from a parked worker.
    ///
    /// The parked worker must be woken by the registration (`notify_one`) and fire at the
    /// deadline, not be stranded until an old timeout elapses.
    #[test]
    fn a_short_timer_started_from_a_parked_worker_is_not_delayed() {
        let queue = EventQueue::new();
        let manager = TimerManager::new(queue.sender());

        // Let the worker reach the unbounded wait first, so the registration must wake it.
        thread::sleep(Duration::from_millis(30));

        let start = Instant::now();
        manager
            .start_timer(21, 5, Duration::from_millis(20), false)
            .expect("a short timer should start");

        let deadline = start + Duration::from_millis(500);
        let mut fired_at = None;
        while Instant::now() < deadline {
            if let Some((target, event, _)) = queue.dequeue() {
                if target == 21 && matches!(event, Event::Timer { id: 5 }) {
                    fired_at = Some(Instant::now());
                    break;
                }
            }
            thread::sleep(Duration::from_millis(2));
        }

        let fired_at = fired_at.expect("the short timer must fire");
        let latency = fired_at.saturating_duration_since(start);
        assert!(
            latency < Duration::from_millis(250),
            "a 20 ms timer must not be delayed beyond tolerance; it took {latency:?} \
             to be observed"
        );
    }

    /// D09-EVT-05: concurrent add/remove/stop from another thread wake the parked worker
    /// and the manager still stops (joins) without deadlock.
    ///
    /// The worker parks with an empty set; the mutations below happen while it is parked,
    /// so each must notify or the worker would either miss the wake or block the final
    /// `drop`/`join`. A watchdog thread turns a deadlock into a test failure rather than a
    /// hung suite.
    #[test]
    fn concurrent_mutations_wake_a_parked_worker_without_deadlock() {
        use std::sync::mpsc::channel;

        let (done_tx, done_rx) = channel();
        let worker = thread::spawn(move || {
            let queue = EventQueue::new();
            let manager = TimerManager::new(queue.sender());

            // Park with no timers.
            thread::sleep(Duration::from_millis(20));

            // Add, clear, and stop while parked; none may block.
            manager.start_timer(1, 1, Duration::from_millis(5), false).unwrap();
            manager.stop_timers_for_target(1);
            manager.start_timer(2, 2, Duration::from_millis(5), true).unwrap();
            assert!(manager.stop_timer(2, 2));
            manager.clear();

            // A timer started after all that must still fire, proving the worker was
            // woken and stayed live rather than exiting on a lost notification.
            manager.start_timer(3, 3, Duration::from_millis(10), false).unwrap();
            let deadline = Instant::now() + Duration::from_millis(500);
            let mut fired = false;
            while Instant::now() < deadline {
                if let Some((target, event, _)) = queue.dequeue() {
                    if target == 3 && matches!(event, Event::Timer { id: 3 }) {
                        fired = true;
                        break;
                    }
                }
                thread::sleep(Duration::from_millis(2));
            }
            assert!(fired, "a timer started after the parked mutations must still fire");

            // Dropping the manager joins the parked worker; this must return.
            drop(manager);
            let _ = done_tx.send(());
        });

        match done_rx.recv_timeout(Duration::from_secs(5)) {
            Ok(()) => {}
            Err(_) => panic!("the timer worker deadlocked on a concurrent mutation or stop"),
        }
        worker.join().expect("the timer worker thread must join cleanly");
    }
}

#[cfg(all(test, alloc_frugal))]
mod mini_tests {
    use super::*;
    use crate::event::EventQueue;

    #[test]
    fn mini_pump_fires_due_timers() {
        let queue = EventQueue::new();
        let manager = TimerManager::new(queue.sender());
        manager
            .start_timer(7, 11, Duration::from_millis(1), false)
            .expect("one-shot timer should start");

        let mut found = false;
        for _ in 0..1000 {
            manager.pump();
            if let Some((target, event, _priority)) = queue.dequeue() {
                if target == 7 && matches!(event, crate::event::types::Event::Timer { id: 11 }) {
                    found = true;
                    break;
                }
            }
            std::thread::sleep(Duration::from_millis(1));
        }

        assert!(found, "mini pump should have posted the due timer event");
    }
}
