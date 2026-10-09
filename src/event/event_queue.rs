// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Event queue implementation.
use super::types::{Event, EventPriority};
use crate::compat::mpsc::{Receiver, Sender};
use crate::compat::{format, mpsc, String, Vec};
use crate::core::ObjectId;

/// Index of the earliest envelope at the highest priority present (D09-EVT-06).
///
/// Priority dominates arrival order; among events of that same priority the earliest (lowest
/// index, which is the oldest because the buffer is in FIFO order) wins. `buffered` is never
/// empty at the call site, but the empty case returns `0` rather than panicking so the helper
/// has no reachable panic of its own.
fn highest_priority_index(buffered: &[EventEnvelope]) -> usize {
    let mut best = 0usize;
    for (index, envelope) in buffered.iter().enumerate() {
        if priority_rank(envelope.priority) > priority_rank(buffered[best].priority) {
            best = index;
        }
    }
    best
}

/// Orders the priorities so a larger value means "dispatch first" (High > Normal > Idle).
fn priority_rank(priority: EventPriority) -> u8 {
    match priority {
        EventPriority::High => 2,
        EventPriority::Normal => 1,
        EventPriority::Idle => 0,
    }
}
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

    /// Dequeues the highest-priority event available, preserving FIFO within a priority
    /// (D09-EVT-06).
    ///
    /// # Why this is needed
    ///
    /// The `mini` event pump dequeued the oldest event regardless of priority, so the
    /// metadata the queue stores was never consulted at dispatch: a Normal/Idle event posted
    /// before a High one was dispatched first, and a sustained FIFO load could delay a High
    /// event without bound. This method gives the pump the same "High before Normal before
    /// Idle" selection the threaded loop performs in its phased dispatch, so both profiles
    /// agree on the ordering semantics.
    ///
    /// # How selection is done over a FIFO channel
    ///
    /// The channel offers no peek, so the currently available events are drained, the first
    /// of the highest present priority is chosen, and the rest are re-posted **in their
    /// original order**. Draining a bounded snapshot and re-appending preserves the relative
    /// order of the unselected events, which is what keeps same-priority events FIFO.
    ///
    /// This is intended for the single-threaded `mini` pump; there, no producer can post
    /// concurrently while the drain/re-post runs. If it were called on a profile with
    /// concurrent producers, an event posted during the drain would be appended ahead of the
    /// re-posted remainder, which is the documented caveat for a non-atomic selection.
    pub fn dequeue_priority_first(&self) -> Option<(ObjectId, Event, EventPriority)> {
        let mut buffered: Vec<EventEnvelope> = Vec::new();
        while let Ok(envelope) = self.receiver.try_recv() {
            buffered.push(envelope);
        }
        if buffered.is_empty() {
            return None;
        }

        // Pick the earliest event at the highest priority present.
        let chosen = highest_priority_index(&buffered);
        let selected = buffered.remove(chosen);
        // Re-post the remainder in order so same-priority ordering is preserved.
        for envelope in buffered {
            let _ = self.sender.inner.send(envelope);
        }
        Some((selected.target, selected.event, selected.priority))
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

    /// D09-EVT-06: a High event posted after Normal/Idle work is selected first.
    #[test]
    fn priority_first_selects_high_over_earlier_normal_and_idle() {
        let q = EventQueue::new();
        let sender = q.sender();

        sender.post_with_priority(1, Event::Paint, EventPriority::Normal).unwrap();
        sender.post_with_priority(2, Event::Paint, EventPriority::Idle).unwrap();
        sender.post_with_priority(3, Event::Paint, EventPriority::High).unwrap();

        let (target, _, priority) = q.dequeue_priority_first().expect("one event must be selected");
        assert_eq!(priority, EventPriority::High, "High must be selected over older Normal/Idle");
        assert_eq!(target, 3);

        // The remainder keeps FIFO among the lower priorities: Normal (posted first) then Idle.
        let (target, _, priority) = q.dequeue_priority_first().unwrap();
        assert_eq!(priority, EventPriority::Normal);
        assert_eq!(target, 1);
        let (target, _, priority) = q.dequeue_priority_first().unwrap();
        assert_eq!(priority, EventPriority::Idle);
        assert_eq!(target, 2);
        assert!(q.dequeue_priority_first().is_none());
    }

    /// D09-EVT-06: Normal is selected over an earlier Idle event.
    #[test]
    fn priority_first_selects_normal_over_earlier_idle() {
        let q = EventQueue::new();
        let sender = q.sender();

        sender.post_idle(1, Event::Paint).unwrap();
        sender.post_with_priority(2, Event::Paint, EventPriority::Normal).unwrap();

        let (target, _, priority) = q.dequeue_priority_first().unwrap();
        assert_eq!(priority, EventPriority::Normal, "Normal outranks the older Idle event");
        assert_eq!(target, 2);
        let (target, _, priority) = q.dequeue_priority_first().unwrap();
        assert_eq!(priority, EventPriority::Idle);
        assert_eq!(target, 1);
    }

    /// D09-EVT-06: among events of the same priority, arrival order (FIFO) is preserved.
    #[test]
    fn priority_first_preserves_fifo_within_a_priority() {
        let q = EventQueue::new();
        let sender = q.sender();

        for id in 0..4u64 {
            sender.post_with_priority(id, Event::Paint, EventPriority::High).unwrap();
        }
        let mut order = Vec::new();
        while let Some((target, _, priority)) = q.dequeue_priority_first() {
            assert_eq!(priority, EventPriority::High);
            order.push(target);
        }
        assert_eq!(order, vec![0, 1, 2, 3], "same-priority events must stay in FIFO order");
    }

    /// D09-EVT-06: an empty queue selects nothing.
    #[test]
    fn priority_first_on_empty_queue_returns_none() {
        let q = EventQueue::new();
        assert!(q.dequeue_priority_first().is_none());
    }

    /// D09-EVT-06: an all-Idle queue still yields every event, in order.
    #[test]
    fn priority_first_drains_idle_only_queue_in_order() {
        let q = EventQueue::new();
        let sender = q.sender();
        for id in 0..3u64 {
            sender.post_idle(id, Event::Paint).unwrap();
        }
        let mut order = Vec::new();
        while let Some((target, _, priority)) = q.dequeue_priority_first() {
            assert_eq!(priority, EventPriority::Idle);
            order.push(target);
        }
        assert_eq!(order, vec![0, 1, 2]);
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
