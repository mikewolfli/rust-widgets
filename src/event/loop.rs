//! Event loop implementation.
use super::event_queue::EventQueue;
use super::types::{Event, EventPriority};
use crate::core::ObjectId;
#[cfg(feature = "touch")]
use crate::gesture::GestureEngine;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
#[cfg(feature = "touch")]
use std::time::{SystemTime, UNIX_EPOCH};

/// Type alias for event dispatch function.
pub type EventDispatchFn = Arc<dyn Fn(ObjectId, &Event) + Send + Sync>;

/// Helper to recover from a poisoned mutex by extracting the inner value.
fn recover_lock<T>(
    e: std::sync::PoisonError<std::sync::MutexGuard<'_, T>>,
) -> std::sync::MutexGuard<'_, T> {
    e.into_inner()
}

/// Returns the current timestamp in milliseconds since UNIX epoch.
#[cfg(feature = "touch")]
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Main event loop for processing events.
pub struct EventLoop {
    /// Event queue for processing.
    queue: Arc<Mutex<EventQueue>>,
    /// Shared flag indicating if the loop is running.
    running: Arc<Mutex<bool>>,
    /// Processing thread handle.
    thread_handle: Option<thread::JoinHandle<()>>,
    /// Optional dispatch callback invoked for each event.
    dispatch_fn: Option<EventDispatchFn>,
}

impl EventLoop {
    /// Creates a new event loop.
    pub fn new() -> Self {
        Self {
            queue: Arc::new(Mutex::new(EventQueue::new())),
            running: Arc::new(Mutex::new(false)),
            thread_handle: None,
            dispatch_fn: None,
        }
    }

    /// Starts the event loop in a separate thread.
    pub fn start(&mut self) {
        if *self.running.lock().unwrap_or_else(recover_lock) {
            return;
        }
        *self.running.lock().unwrap_or_else(recover_lock) = true;
        let running = Arc::clone(&self.running);
        let queue = Arc::clone(&self.queue);
        let dispatch_fn = self.dispatch_fn.clone();
        #[cfg(feature = "touch")]
        let mut gesture_engine = GestureEngine::new();
        let handle = thread::spawn(move || {
            while *running.lock().unwrap_or_else(recover_lock) {
                // Process events from the queue
                if let Some((_target, event, _priority)) =
                    queue.lock().unwrap_or_else(recover_lock).dequeue()
                {
                    // Route through gesture engine for touch events
                    #[cfg(feature = "touch")]
                    let maybe_gesture_event = if event.is_touch() {
                        gesture_engine.process(&event, now_ms())
                    } else {
                        None
                    };

                    // Dispatch event to the target widget if a dispatch function is set
                    if let Some(ref dispatch) = dispatch_fn {
                        dispatch(_target, &event);
                        #[cfg(feature = "touch")]
                        if let Some(ref gesture) = maybe_gesture_event {
                            dispatch(_target, gesture);
                        }
                    } else {
                        // Fallback: consume values when no dispatch function is set
                        let _ = _target;
                        let _ = event;
                        #[cfg(feature = "touch")]
                        let _ = maybe_gesture_event;
                    }
                }
                // Sleep to prevent busy waiting
                thread::sleep(Duration::from_millis(10));
            }
        });
        self.thread_handle = Some(handle);
    }

    /// Stops the event loop.
    pub fn stop(&mut self) {
        *self.running.lock().unwrap_or_else(recover_lock) = false;
        if let Some(handle) = self.thread_handle.take() {
            if let Err(e) = handle.join() {
                log::error!("[event-loop] Thread join failed: {:?}", e);
            }
        }
    }

    /// Posts an event to the event loop.
    pub fn post_event(
        &self,
        target: ObjectId,
        event: Event,
        priority: EventPriority,
    ) -> Result<(), String> {
        self.queue
            .lock()
            .unwrap_or_else(recover_lock)
            .sender()
            .post_with_priority(target, event, priority)
            .map_err(|e| e.to_string())
    }

    /// Sets the dispatch callback invoked for each dequeued event.
    pub fn set_dispatch_fn(&mut self, f: EventDispatchFn) {
        self.dispatch_fn = Some(f);
    }

    /// Checks if the event loop is running.
    pub fn is_running(&self) -> bool {
        *self.running.lock().unwrap_or_else(recover_lock)
    }
}

impl Default for EventLoop {
    fn default() -> Self {
        Self::new()
    }
}
