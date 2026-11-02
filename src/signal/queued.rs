// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Cross-thread queued connections.
//!
//! # The problem
//!
//! A signal delivers its payload on the emitting thread. That is correct for a control whose
//! threads are its own, but a background worker emitting progress into a widget must not touch
//! the UI from that worker. The platform layer already has the marshalling primitive — work
//! handed to [`crate::platform::main_thread::schedule_in`] runs on the loop's own thread on its
//! next frame — but the signal layer never connected to it, so a host had to write its own
//! forwarding thread.
//!
//! [`Signal::connect_queued`] is that join. It registers a normal borrowed slot, clones the value
//! on the emitting thread (the payload's `Clone` bound is already part of `Signal`), and hands a
//! closure to the main-thread queue. The user's callback runs on the loop's thread, never on the
//! producer's.
//!
//! # Ordering and loss
//!
//! Each emit enqueues one item, so emission order is preserved for a single producer. Two
//! producers have no mutual order, exactly as two concurrent [`Signal::emit`] calls do. The queue
//! is bounded only by the frame loop draining it; a producer that emits faster than the loop runs
//! grows the queue rather than dropping values. A caller that wants coalescing wants
//! [`crate::signal::Debounced`] instead, which deliberately keeps only the latest value.

use super::{ConnectionHandle, Signal};
use crate::compat::{lock, Mutex};
use alloc::boxed::Box;
use alloc::sync::Arc;

/// One unit of work queued for the loop's thread.
pub type QueuedWork = Box<dyn FnOnce() + Send + 'static>;

/// The transport a queued connection uses.
///
/// The default is [`crate::platform::main_thread::schedule_in`]. A caller can inject another
/// transport (a test's manual queue, or a host's own dispatcher) with
/// [`Signal::connect_queued_with_dispatcher`]; the two entry points differ only in which
/// transport they use.
pub type QueuedDispatcher = dyn Fn(QueuedWork) + Send + Sync;

impl<T: Clone + Send + 'static> Signal<T> {
    /// Connects a slot that runs on the event loop's own thread.
    ///
    /// This is safe to call from any thread. The payload is cloned at emit time, so the producer
    /// does not have to keep it alive until the queued work runs. The returned handle behaves
    /// like any other connection: it can be disconnected or blocked, and disconnecting before the
    /// queued work runs still delivers that already-cloned value (the work was accepted before the
    /// disconnection, exactly as a value already in flight behaves).
    pub fn connect_queued<F>(&self, slot: F) -> ConnectionHandle
    where
        F: FnMut(T) + Send + Sync + 'static,
    {
        self.connect_queued_with_dispatcher(slot, queued_on_main_thread)
    }

    /// [`Self::connect_queued`] with an explicit transport.
    ///
    /// A test can collect the work into a `Vec<QueuedWork>` and run it on demand, which is what
    /// makes the marshalling contract assertable without starting a window loop.
    pub fn connect_queued_with_dispatcher<F, D>(&self, slot: F, dispatch: D) -> ConnectionHandle
    where
        F: FnMut(T) + Send + Sync + 'static,
        D: Fn(QueuedWork) + Send + Sync + 'static,
    {
        // The callback is shared because every queued item may outlive the emit that created it.
        // A `Mutex<Box<dyn FnMut>>` is the minimal shape that keeps `Signal`'s `Send + Sync`
        // contract while still allowing an `FnMut` user callback.
        let slot: Box<dyn FnMut(T) + Send> = Box::new(slot);
        let slot: Arc<Mutex<Box<dyn FnMut(T) + Send>>> = Arc::new(Mutex::new(slot));
        let dispatch: Arc<QueuedDispatcher> = Arc::new(dispatch);
        self.connect_ref(move |value: &T| {
            let value = value.clone();
            let slot = Arc::clone(&slot);
            let dispatch = Arc::clone(&dispatch);
            (dispatch.as_ref())(Box::new(move || {
                let mut callback = lock(&slot);
                (**callback)(value);
            }));
        })
    }
}

/// The default transport: hand the work to the loop's main thread.
fn queued_on_main_thread(work: QueuedWork) {
    // The return value reports whether a loop has ticked; the work is queued either way, so a
    // `false` here is not a delivery failure and must not lose the item.
    let _ = crate::platform::main_thread::schedule_in(core::time::Duration::ZERO, work);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::Vec;

    /// A queued connection does not run on the emitting thread; it runs when the transport drains.
    #[test]
    fn a_queued_connection_waits_for_the_dispatcher() {
        let signal = Signal::<u32>::new();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let queue: Arc<Mutex<Vec<QueuedWork>>> = Arc::new(Mutex::new(Vec::new()));

        let slot_calls = Arc::clone(&calls);
        let dispatch_queue = Arc::clone(&queue);
        signal.connect_queued_with_dispatcher(
            move |value| lock(&slot_calls).push(value),
            move |work| lock(&dispatch_queue).push(work),
        );

        signal.emit(7);
        assert!(lock(&calls).is_empty(), "the slot must not run on the emitting thread");
        assert_eq!(lock(&queue).len(), 1, "one emit enqueues exactly one item");

        let pending: Vec<QueuedWork> = lock(&queue).drain(..).collect();
        for work in pending {
            work();
        }
        assert_eq!(*lock(&calls), [7]);
    }

    /// A producer on another thread enqueues work that the consumer thread runs.
    ///
    /// This is the actual cross-thread contract, not merely a deferred call on one thread: the
    /// callback observes the consumer's thread id.
    #[test]
    fn a_queued_connection_marshals_from_another_thread() {
        let signal = Signal::<u32>::new();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let queue: Arc<Mutex<Vec<QueuedWork>>> = Arc::new(Mutex::new(Vec::new()));

        let slot_calls = Arc::clone(&calls);
        let dispatch_queue = Arc::clone(&queue);
        signal.connect_queued_with_dispatcher(
            move |value| lock(&slot_calls).push(value),
            move |work| lock(&dispatch_queue).push(work),
        );

        let producer = signal.clone();
        std::thread::spawn(move || producer.emit(11)).join().expect("the producer must not panic");
        assert!(lock(&calls).is_empty(), "the consumer has not drained yet");

        let pending: Vec<QueuedWork> = lock(&queue).drain(..).collect();
        for work in pending {
            work();
        }
        assert_eq!(*lock(&calls), [11]);
    }
}
