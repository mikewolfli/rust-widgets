//! Runtime timer manager that emits `Event::Timer` into the event queue.
use super::event_queue::EventSender;
use super::types::Event;
use crate::core::ObjectId;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy)]
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

fn recover_lock<T>(
    e: std::sync::PoisonError<std::sync::MutexGuard<'_, T>>,
) -> std::sync::MutexGuard<'_, T> {
    e.into_inner()
}

/// Emits timer events into the event queue for one-shot and repeating timers.
pub struct TimerManager {
    state: Arc<Mutex<TimerState>>,
    thread_handle: Option<thread::JoinHandle<()>>,
}

impl TimerManager {
    /// Create a timer manager bound to an event sender.
    pub fn new(sender: EventSender) -> Self {
        let state = Arc::new(Mutex::new(TimerState {
            timers: HashMap::new(),
            running: true,
        }));

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
                                entry.next_fire = now + entry.interval;
                            }
                        }
                    }
                }

                for (target, id) in &due_events {
                    if let Some(entry) = guard.timers.get(&(*target, *id)) {
                        if !entry.repeating {
                            guard.timers.remove(&(*target, *id));
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

        Self {
            state,
            thread_handle: Some(thread_handle),
        }
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

        let entry = TimerEntry {
            interval,
            repeating,
            next_fire: Instant::now() + interval,
        };

        let mut guard = self.state.lock().unwrap_or_else(recover_lock);
        guard.timers.insert((target, id), entry);
        Ok(())
    }

    /// Stop one timer by `(target, id)`.
    pub fn stop_timer(&self, target: ObjectId, id: u32) -> bool {
        let mut guard = self.state.lock().unwrap_or_else(recover_lock);
        guard.timers.remove(&(target, id)).is_some()
    }

    /// Stop all timers belonging to one target.
    pub fn stop_timers_for_target(&self, target: ObjectId) -> usize {
        let mut guard = self.state.lock().unwrap_or_else(recover_lock);
        let before = guard.timers.len();
        guard
            .timers
            .retain(|(timer_target, _), _| *timer_target != target);
        before.saturating_sub(guard.timers.len())
    }

    /// Remove all active timers.
    pub fn clear(&self) {
        let mut guard = self.state.lock().unwrap_or_else(recover_lock);
        guard.timers.clear();
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::EventQueue;
    use std::thread;
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
