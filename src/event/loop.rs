//! Event loop implementation.

use crate::core::ObjectId;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use super::queue::EventQueue;
use super::types::{Event, EventPriority};

/// Main event loop for processing events.
#[derive(Debug)]
pub struct EventLoop {
    /// Event queue for processing.
    queue: Arc<Mutex<EventQueue>>,
    /// Flag indicating if the loop is running.
    running: bool,
    /// Processing thread handle.
    thread_handle: Option<thread::JoinHandle<()>>,
}

impl EventLoop {
    /// Creates a new event loop.
    pub fn new() -> Self {
        Self {
            queue: Arc::new(Mutex::new(EventQueue::new())),
            running: false,
            thread_handle: None,
        }
    }

    /// Starts the event loop in a separate thread.
    pub fn start(&mut self) {
        if self.running {
            return;
        }

        self.running = true;
        let queue_clone = Arc::clone(&self.queue);
        let running_clone = Arc::new(Mutex::new(true));

        let handle = thread::spawn(move || {
            let running = running_clone;
            let queue = queue_clone;

            while *running.lock().unwrap() {
                // Process events from the queue
                if let Some(event) = queue.lock().unwrap().dequeue() {
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
        self.running = false;
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }

    /// Posts an event to the event loop.
    pub fn post_event(&self, target: ObjectId, event: Event, priority: EventPriority) -> bool {
        self.queue.lock().unwrap().enqueue(target, event, priority)
    }

    /// Checks if the event loop is running.
    pub fn is_running(&self) -> bool {
        self.running
    }
}

impl Default for EventLoop {
    fn default() -> Self {
        Self::new()
    }
}
