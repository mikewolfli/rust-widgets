//! Event queue implementation.
use super::types::{Event, EventPriority};
use crate::compat::mpsc;
use crate::compat::mpsc::{Receiver, Sender};
use crate::core::ObjectId;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::Event;

    #[test]
    fn test_event_queue_new_is_empty() {
        let q = EventQueue::new();
        assert!(q.dequeue().is_none());
    }

    #[test]
    fn test_event_queue_post_and_dequeue_roundtrip() {
        let q = EventQueue::new();
        let sender = q.sender();
        let id = 42;
        let event = Event::Paint;

        assert!(sender.post(id, event).is_ok());
        let (target, _evt, pri) = q.dequeue().unwrap();
        assert_eq!(target, id);
        assert!(matches!(_evt, Event::Paint));
        assert_eq!(pri, EventPriority::Normal);
    }

    #[test]
    fn test_event_queue_post_idle() {
        let q = EventQueue::new();
        let sender = q.sender();
        let id = 42;
        let event = Event::Paint;

        assert!(sender.post_idle(id, event).is_ok());
        let (target, _evt, pri) = q.dequeue().unwrap();
        assert_eq!(target, id);
        assert!(matches!(_evt, Event::Paint));
        assert_eq!(pri, EventPriority::Idle);
    }

    #[test]
    fn test_event_queue_fifo_order() {
        let q = EventQueue::new();
        let sender = q.sender();

        sender.post(10, Event::Paint).unwrap();
        sender.post(20, Event::Timer { id: 1 }).unwrap();

        let (t1, _, _) = q.dequeue().unwrap();
        let (t2, _, _) = q.dequeue().unwrap();
        assert_eq!(t1, 10);
        assert_eq!(t2, 20);
        assert!(q.dequeue().is_none());
    }

    #[test]
    fn test_event_queue_dequeue_blocking() {
        let q = EventQueue::new();
        let sender = q.sender();

        sender.post(42, Event::Quit).unwrap();
        let (target, _evt, _) = q.dequeue_blocking().unwrap();
        assert_eq!(target, 42);
        assert!(matches!(_evt, Event::Quit));
    }

    #[test]
    fn test_event_sender_clone() {
        let q = EventQueue::new();
        let s1 = q.sender();
        let s2 = q.sender();

        s1.post(10, Event::Paint).unwrap();
        s2.post(20, Event::Quit).unwrap();

        assert!(q.dequeue().is_some());
        assert!(q.dequeue().is_some());
        assert!(q.dequeue().is_none());
    }
}
