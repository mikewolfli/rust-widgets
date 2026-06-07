//! Event queue implementation.
use super::types::{Event, EventPriority};
use crate::core::ObjectId;
use std::sync::mpsc::{self, Receiver, Sender};
#[derive(Debug, Clone)]
struct EventEnvelope {
    target: ObjectId,
    event: Event,
    priority: EventPriority,
}
/// Sender handle used to enqueue events.
#[derive(Clone)]
pub struct EventSender {
    inner: Sender<EventEnvelope>,
}
impl EventSender {
    /// Post event for a target object id.
    pub fn post(&self, object_id: ObjectId, event: Event) -> Result<(), String> {
        self.post_with_priority(object_id, event, EventPriority::Normal)
    }
    /// Post event with explicit priority.
    pub fn post_with_priority(
        &self,
        object_id: ObjectId,
        event: Event,
        priority: EventPriority,
    ) -> Result<(), String> {
        self.inner
            .send(EventEnvelope { target: object_id, event, priority })
            .map_err(|_| "event queue disconnected".to_string())
    }
    /// Post idle-priority event.
    pub fn post_idle(&self, object_id: ObjectId, event: Event) -> Result<(), String> {
        self.post_with_priority(object_id, event, EventPriority::Idle)
    }
}
/// Queue pair used by `EventLoop` internals.
pub struct EventQueue {
    sender: EventSender,
    receiver: Receiver<EventEnvelope>,
}
impl EventQueue {
    /// Create unbounded queue and sender/receiver pair.
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Self { sender: EventSender { inner: tx }, receiver: rx }
    }
    /// Returns a cloneable sender handle for posting events.
    pub fn sender(&self) -> EventSender {
        self.sender.clone()
    }
    /// Dequeues the next event, if available.
    pub fn dequeue(&self) -> Option<(ObjectId, Event, EventPriority)> {
        match self.receiver.try_recv() {
            Ok(envelope) => Some((envelope.target, envelope.event, envelope.priority)),
            Err(_) => None,
        }
    }
    /// Dequeues the next event, blocking if none available.
    pub fn dequeue_blocking(&self) -> Option<(ObjectId, Event, EventPriority)> {
        match self.receiver.recv() {
            Ok(envelope) => Some((envelope.target, envelope.event, envelope.priority)),
            Err(_) => None,
        }
    }
}
impl Default for EventQueue {
    fn default() -> Self {
        Self::new()
    }
}
