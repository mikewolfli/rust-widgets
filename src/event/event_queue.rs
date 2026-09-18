// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Event queue implementation.
use super::types::{Event, EventPriority};
use crate::compat::mpsc::{Receiver, Sender};
use crate::compat::{format, mpsc, String};
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
        self.inner.send(EventEnvelope { target: object_id, event, priority }).map_err(|_| {
            format!(
                "event for object {object_id} could not be queued: the receiving end was \
                     dropped, so the queue is closed"
            )
        })
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
    ///
    /// # Blocking is only available where threads are
    ///
    /// Under the `mini` (alloc-frugal) profile there is no second thread to wake a
    /// waiter, so this **cannot** block: the queue's `recv` is a poll that returns
    /// `Err` at once. The method is therefore not compiled there — callers use
    /// [`EventQueue::dequeue`], which is what the `mini` event loop already does.
    ///
    /// This used to be compiled unconditionally and called the profile's `recv`,
    /// which made it a silent `try_recv` under `mini` — a method documented as
    /// blocking that returned immediately. Gating it is the honest expression of the
    /// capability: the compiler now enforces which profiles may use it, instead of
    /// the doc asking the reader to remember.
    #[cfg(not(alloc_frugal))]
    pub fn dequeue_blocking(&self) -> Option<(ObjectId, Event, EventPriority)> {
        match self.receiver.recv() {
            Ok(envelope) => Some((envelope.target, envelope.event, envelope.priority)),
            Err(_) => None,
        }
    }
}
crate::impl_default_via_new!(EventQueue);

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

    /// `dequeue_blocking` only exists where threads do, so this test does too.
    ///
    /// Under `mini` there is no waiter to wake and the method is not compiled; the
    /// `mini` event loop drains with `dequeue` instead. Gating the test alongside the
    /// method keeps the two from drifting apart.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn test_event_queue_dequeue_blocking() {
        let q = EventQueue::new();
        let sender = q.sender();

        sender.post(42, Event::Quit).unwrap();
        let (target, _evt, _) = q.dequeue_blocking().unwrap();
        assert_eq!(target, 42);
        assert!(matches!(_evt, Event::Quit));
    }

    /// The non-blocking dequeue is the one every profile has.
    #[test]
    fn test_event_queue_dequeue_returns_posted_event() {
        let q = EventQueue::new();
        q.sender().post(7, Event::Quit).unwrap();
        let (target, _evt, _) = q.dequeue().expect("a posted event must be dequeued");
        assert_eq!(target, 7);
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
