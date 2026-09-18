// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Runtime timer manager that emits `Event::Timer` into the event queue.
use super::event_queue::EventSender;
#[cfg(not(alloc_frugal))]
use super::types::Event;
use crate::compat::{lock, Box, HashMap, Instant, MiniToString, Mutex, String, Vec};
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

#[derive(Default)]
struct TimerState {
    timers: HashMap<(ObjectId, u32), TimerEntry>,
    running: bool,
}

/// Emits timer events into the event queue for one-shot and repeating timers.
pub struct TimerManager {
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
        let state = Arc::new(Mutex::new(TimerState { timers: HashMap::new(), running: true }));

        let worker_state = Arc::clone(&state);
        let worker_sender = sender;
        let thread_handle = thread::spawn(move || loop {
            let now = Instant::now();
            let mut due_events = Vec::new();

            {
                let mut guard = lock(&worker_state);
                if !guard.running {
                    return;
                }

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
            }

            for (target, id) in due_events {
                if worker_sender.post(target, Event::timer(id)).is_err() {
                    lock(&worker_state).timers.remove(&(target, id));
                }
            }

            thread::sleep(Duration::from_millis(2));
        });

        Self { state, thread_handle: Some(thread_handle) }
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
    fn lock_timers(&self) -> crate::compat::MutexGuard<'_, TimerState> {
        lock(&self.state)
    }

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

        let entry = TimerEntry { interval, repeating, next_fire: Instant::now() + interval };
        self.lock_timers().timers.insert((target, id), entry);
        Ok(())
    }

    /// Stop one timer by `(target, id)`.
    pub fn stop_timer(&self, target: ObjectId, id: u32) -> bool {
        self.lock_timers().timers.remove(&(target, id)).is_some()
    }

    /// Stop all timers belonging to one target.
    pub fn stop_timers_for_target(&self, target: ObjectId) -> usize {
        let mut guard = self.lock_timers();
        let before = guard.timers.len();
        guard.timers.retain(|(timer_target, _), _| *timer_target != target);
        before.saturating_sub(guard.timers.len())
    }

    /// Remove all active timers.
    pub fn clear(&self) {
        self.lock_timers().timers.clear();
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
            let mut guard = lock(&self.state);
            guard.running = false;
            guard.timers.clear();
        }
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
