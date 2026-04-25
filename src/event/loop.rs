//! Event loop implementation.
use super::event_queue::EventQueue;
use super::types::{Event, EventPriority};
use crate::core::ObjectId;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
/// Main event loop for processing events.
pub struct EventLoop {
    /// Event queue for processing.
    queue: Arc<Mutex<EventQueue>>,
    /// Shared flag indicating if the loop is running.
    running: Arc<Mutex<bool>>,
    /// Processing thread handle.
    thread_handle: Option<thread::JoinHandle<()>>,
}
impl EventLoop {
    /// Creates a new event loop.
    pub fn new() -> Self {
        Self {
            queue: Arc::new(Mutex::new(EventQueue::new())),
            running: Arc::new(Mutex::new(false)),
            thread_handle: None,
        }
    }
    /// Starts the event loop in a separate thread.
    pub fn start(&mut self) {
        if *self.running.lock().unwrap() {
            return;
        }
        *self.running.lock().unwrap() = true;
        let running = Arc::clone(&self.running);
        let queue = Arc::clone(&self.queue);
        let handle = thread::spawn(move || {
            while *running.lock().unwrap() {
                // Process events from the queue
                if let Some(_event) = queue.lock().unwrap().dequeue() {
                    // Process the event
                    // In a real implementation, this would dispatch to widgets
                }
                // Sleep to prevent busy waiting
                thread::sleep(Duration::from_millis(10));
            }
        });
        self.thread_handle = Some(handle);
    }
    /// Stops the event loop.
    pub fn stop(&mut self) {
        *self.running.lock().unwrap() = false;
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
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
            .unwrap()
            .sender()
            .post_with_priority(target, event, priority)
            .map_err(|e| format!("{e}"))
    }
    /// Checks if the event loop is running.
    pub fn is_running(&self) -> bool {
        *self.running.lock().unwrap()
    }
}
impl Default for EventLoop {
    fn default() -> Self {
        Self::new()
    }
}

