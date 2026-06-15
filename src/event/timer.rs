//! Runtime timer manager that emits `Event::Timer` into the event queue.
use super::event_queue::EventSender;
use super::types::Event;
use crate::compat::HashMap;
use crate::compat::Instant;
use crate::compat::Mutex;
use crate::core::ObjectId;
use alloc::sync::Arc;
use core::time::Duration;
#[cfg(not(feature = "mini"))]
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

#[cfg(not(feature = "mini"))]
fn recover_lock<T>(
    e: std::sync::PoisonError<crate::compat::MutexGuard<'_, T>>,
) -> crate::compat::MutexGuard<'_, T> {
    e.into_inner()
}

/// Emits timer events into the event queue for one-shot and repeating timers.
pub struct TimerManager {
    state: Arc<Mutex<TimerState>>,
    #[cfg(not(feature = "mini"))]
    thread_handle: Option<thread::JoinHandle<()>>,
    #[cfg(feature = "mini")]
    thread_handle: Option<()>,
}

impl TimerManager {
    /// Create a timer manager bound to an event sender.
    #[cfg(not(feature = "mini"))]
    pub fn new(sender: EventSender) -> Self {
        let state = Arc::new(Mutex::new(TimerState { timers: HashMap::new(), running: true }));

        let worker_state = Arc::clone(&state);
        let worker_sender = sender;
        let thread_handle = thread::spawn(move || loop {
            let now = Instant::now();
            let mut due_events = Vec::new();

            {
                let mut guard = worker_state.lock().unwrap_or_else(recover_lock);
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
                    let mut guard = worker_state.lock().unwrap_or_else(recover_lock);
                    guard.timers.remove(&(target, id));
                }
            }

            thread::sleep(Duration::from_millis(2));
        });

        Self { state, thread_handle: Some(thread_handle) }
    }

    /// Create a timer manager (mini stub — single-threaded, no background thread).
    #[cfg(feature = "mini")]
    pub fn new(_sender: EventSender) -> Self {
        let state = Arc::new(Mutex::new(TimerState { timers: HashMap::new(), running: true }));
        Self { state, thread_handle: None }
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

        #[cfg(not(feature = "mini"))]
        {
            let mut guard = self.state.lock().unwrap_or_else(recover_lock);
            guard.timers.insert((target, id), entry);
        }
        #[cfg(feature = "mini")]
        {
            let mut guard = self.state.lock().unwrap_or_else(|p| p.into_inner());
            guard.timers.insert((target, id), entry);
        }
        Ok(())
    }

    /// Stop one timer by `(target, id)`.
    pub fn stop_timer(&self, target: ObjectId, id: u32) -> bool {
        #[cfg(not(feature = "mini"))]
        {
            let mut guard = self.state.lock().unwrap_or_else(recover_lock);
            guard.timers.remove(&(target, id)).is_some()
        }
        #[cfg(feature = "mini")]
        {
            let mut guard = self.state.lock().unwrap_or_else(|p| p.into_inner());
            guard.timers.remove(&(target, id)).is_some()
        }
    }

    /// Stop all timers belonging to one target.
    pub fn stop_timers_for_target(&self, target: ObjectId) -> usize {
        #[cfg(not(feature = "mini"))]
        {
            let mut guard = self.state.lock().unwrap_or_else(recover_lock);
            let before = guard.timers.len();
            guard.timers.retain(|(timer_target, _), _| *timer_target != target);
            before.saturating_sub(guard.timers.len())
        }
        #[cfg(feature = "mini")]
        {
            let mut guard = self.state.lock().unwrap_or_else(|p| p.into_inner());
            let before = guard.timers.len();
            guard.timers.retain(|(timer_target, _), _| *timer_target != target);
            before.saturating_sub(guard.timers.len())
        }
    }

    /// Remove all active timers.
    #[cfg(not(feature = "mini"))]
    pub fn clear(&self) {
        let mut guard = self.state.lock().unwrap_or_else(recover_lock);
        guard.timers.clear();
    }

    /// Remove all active timers (mini stub).
    #[cfg(feature = "mini")]
    pub fn clear(&self) {
        let mut guard = self.state.lock().unwrap_or_else(|p| p.into_inner());
        guard.timers.clear();
    }
}

#[cfg(not(feature = "mini"))]
impl Drop for TimerManager {
    fn drop(&mut self) {
        {
            let mut guard = self.state.lock().unwrap_or_else(recover_lock);
            guard.running = false;
            guard.timers.clear();
        }
        if let Some(handle) = self.thread_handle.take() {
            if let Err(e) = handle.join() {
                log::error!("[timer-manager] Thread join failed: {:?}", e);
            }
        }
    }
}

/// Idle task that runs when the event loop has no higher-priority events (BLUE11 R8.5).
pub struct IdleTask {
    pub id: u64,
    pub callback: Box<dyn FnMut() + Send>,
    pub threshold_frames: u32,
    frames_since_run: u32,
}

impl IdleTask {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::EventQueue;
    #[cfg(not(feature = "mini"))]
    use std::thread;
    #[cfg(not(feature = "mini"))]
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
