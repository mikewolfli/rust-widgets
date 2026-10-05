// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

#[cfg(not(alloc_frugal))]
use crate::compat::Condvar;
#[cfg(not(alloc_frugal))]
use crate::compat::Instant;
#[cfg(not(alloc_frugal))]
use crate::compat::Mutex;
// The helper is the profile-agnostic spelling of "take the guard"; importing it
// only where the mutex lives keeps `mini` — which never compiles these queues —
// free of an unused import.
#[cfg(not(alloc_frugal))]
use crate::compat::lock;
#[cfg(not(alloc_frugal))]
use alloc::collections::VecDeque;
#[cfg(not(alloc_frugal))]
use core::time::Duration;

/// Errors returned by queue operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueError {
    /// The queue is at capacity and the item was not stored.
    Full,
    /// No item was available within the caller's deadline.
    Empty,
    /// The queue was closed; no further items can be pushed or popped.
    Closed,
}

/// The mutable state shared by every operation on a [`BlockingQueue`].
///
/// # Why the queue and the closed flag live in one mutex
///
/// They used to be two independent `Mutex`es. `pop` would check `closed` under
/// one lock, decide to wait, and only then call `condvar.wait(queue)` — which
/// takes *two* guards in sequence. A `close()` landing in that gap set the flag
/// and notified while no thread was blocked on the condition variable yet, so
/// the wake-up was lost and `pop()` blocked forever. A condition variable only
/// makes a predicate safe when the predicate is read under the same lock that
/// `wait` atomically releases, so the flag is stored *beside* the queue here:
/// `push`/`pop`/`close` all linearise on one lock, and `close` cannot slip
/// between a waiter's predicate check and its wait.
#[cfg(not(alloc_frugal))]
#[derive(Debug)]
struct QueueState<T> {
    /// Items waiting to be consumed, oldest at the front.
    items: VecDeque<T>,
    /// Set once [`BlockingQueue::close`] runs; latched, never cleared.
    closed: bool,
}

/// A thread-safe, **unbounded** FIFO queue that blocks consumers until an item
/// arrives.
///
/// Any number of producer threads may call [`BlockingQueue::push`] while any
/// number of consumers block in [`BlockingQueue::pop`]. Because it is unbounded,
/// a faster producer can grow the queue without limit; **there is no bounded
/// variant in this crate**, so back-pressure must be applied by the producer
/// (for example by checking [`BlockingQueue::len`] before pushing) or by using a
/// different structure entirely. Only available when the `alloc_frugal` feature
/// is off.
#[cfg(not(alloc_frugal))]
#[derive(Debug)]
pub struct BlockingQueue<T> {
    state: Mutex<QueueState<T>>,
    condvar: Condvar,
}
#[cfg(not(alloc_frugal))]
impl<T> BlockingQueue<T> {
    /// Creates an empty queue with no pre-allocated storage.
    pub fn new() -> Self {
        Self {
            state: Mutex::new(QueueState { items: VecDeque::new(), closed: false }),
            condvar: Condvar::new(),
        }
    }
    /// Creates an empty queue whose backing `VecDeque` is pre-allocated for
    /// `capacity` items.
    ///
    /// `capacity` is only a hint used to avoid early reallocations; the queue is
    /// still unbounded and will grow past it as needed.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            state: Mutex::new(QueueState {
                items: VecDeque::with_capacity(capacity),
                closed: false,
            }),
            condvar: Condvar::new(),
        }
    }
    /// Appends `item` and wakes one blocked consumer.
    ///
    /// Never blocks on capacity, but returns [`QueueError::Closed`] without
    /// storing the item once [`Self::close`] has been called. `push` and
    /// [`Self::close`] linearise on the same lock, so once `close` returns no
    /// later `push` can succeed: either it acquired the lock first (and its item
    /// is queued before the close) or it sees `closed` and is refused. A poisoned
    /// mutex is recovered from rather than propagated, so this cannot panic the
    /// caller.
    pub fn push(&self, item: T) -> Result<(), QueueError> {
        let mut state = lock(&self.state);
        if state.closed {
            return Err(QueueError::Closed);
        }
        state.items.push_back(item);
        // The lock is released before notifying; the waiter re-checks the shared
        // predicate under the same lock on wake, so it cannot miss the item.
        drop(state);
        self.condvar.notify_one();
        Ok(())
    }
    /// Removes and returns the front item, blocking the calling thread until one
    /// is available.
    ///
    /// Blocks indefinitely; the only way out without an item is
    /// [`QueueError::Closed`] after a concurrent [`Self::close`]. Callers that
    /// need a deadline should use [`Self::pop_timeout`] instead.
    pub fn pop(&self) -> Result<T, QueueError> {
        let mut state = lock(&self.state);
        loop {
            if let Some(item) = state.items.pop_front() {
                return Ok(item);
            }
            if state.closed {
                return Err(QueueError::Closed);
            }
            // `wait` releases `state` and re-acquires it on wake, so `closed`
            // and `items` are always read atomically with respect to `close` and
            // `push`. A `close` that lands after the check above therefore sees
            // this thread already parked and notifies it.
            state = self.condvar.wait(state).unwrap_or_else(|e| e.into_inner());
        }
    }
    /// Removes and returns the front item, waiting at most `timeout` for one to
    /// arrive.
    ///
    /// The timeout is measured against a start [`Instant`] captured on entry and
    /// spans the whole call, so spurious wake-ups cannot extend it. Returns
    /// [`QueueError::Empty`] if the deadline passes first, or
    /// [`QueueError::Closed`] if [`Self::close`] races the wait.
    pub fn pop_timeout(&self, timeout: Duration) -> Result<T, QueueError> {
        let start = Instant::now();
        let mut state = lock(&self.state);
        loop {
            if let Some(item) = state.items.pop_front() {
                return Ok(item);
            }
            if state.closed {
                return Err(QueueError::Closed);
            }
            let elapsed = start.elapsed();
            if elapsed >= timeout {
                return Err(QueueError::Empty);
            }
            let remaining = timeout - elapsed;
            let result =
                self.condvar.wait_timeout(state, remaining).unwrap_or_else(|e| e.into_inner());
            state = result.0;
        }
    }
    /// Removes and returns the front item if one is already available, without
    /// blocking; `None` means the queue was momentarily empty.
    pub fn try_pop(&self) -> Option<T> {
        lock(&self.state).items.pop_front()
    }
    /// Closes the queue and wakes every blocked consumer and producer.
    ///
    /// Subsequent [`Self::push`] calls fail with [`QueueError::Closed`], and
    /// blocked [`Self::pop`] calls return [`QueueError::Closed`]. Items already
    /// queued are *not* discarded, and already-queued items are still returned by
    /// [`Self::pop`] before the closed check is reached. `close` is idempotent and
    /// cannot be undone.
    pub fn close(&self) {
        // Set the flag under the state lock so it linearises against `push`, then
        // notify after releasing it. A waiter checks `closed` under this same
        // lock, so it either observes the flag immediately or is already parked
        // and gets woken here — the wake-up cannot be lost.
        lock(&self.state).closed = true;
        self.condvar.notify_all();
    }
    /// Returns `true` once [`Self::close`] has been called.
    pub fn is_closed(&self) -> bool {
        lock(&self.state).closed
    }
    /// Number of items currently queued. Takes the same lock as [`Self::push`] and
    /// [`Self::pop`], so the value can be stale as soon as it is returned.
    pub fn len(&self) -> usize {
        lock(&self.state).items.len()
    }
    /// Returns `true` when no items are queued at the moment of the call.
    pub fn is_empty(&self) -> bool {
        lock(&self.state).items.is_empty()
    }
    /// Removes all queued items, dropping each one. Does not change the closed
    /// state and does not wake blocked consumers, which will simply keep waiting
    /// for the next [`Self::push`].
    pub fn clear(&self) {
        lock(&self.state).items.clear();
    }
}
#[cfg(not(alloc_frugal))]
impl<T> Default for BlockingQueue<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(all(test, not(alloc_frugal)))]
mod tests {
    use super::*;
    use crate::compat::Arc;
    use core::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Barrier;
    use std::thread;
    use std::time::Duration;

    /// The sequence this pins: `pop` must not miss a `push` that lands before the
    /// waiter parks. With a separate `closed` lock, `close` could set the flag and
    /// notify while no waiter was parked yet; the flag here shares one lock with
    /// the item deque, so a waiter either sees the item under the lock or is
    /// parked when the notify happens.
    #[test]
    fn a_push_before_a_parked_waiter_is_not_lost() {
        let q: Arc<BlockingQueue<i32>> = Arc::new(BlockingQueue::new());
        let started = Arc::new(Barrier::new(2));

        let waiter = {
            let q = Arc::clone(&q);
            let started = Arc::clone(&started);
            thread::spawn(move || {
                started.wait();
                q.pop()
            })
        };

        started.wait();
        // Give the consumer every chance to enter `pop` and check the predicate
        // before the item exists; even if it does, the shared-lock ordering means
        // the push cannot be missed.
        thread::sleep(Duration::from_millis(30));
        q.push(7).expect("push succeeds before close");

        let popped = waiter.join().expect("consumer thread joins");
        assert_eq!(
            popped,
            Ok(7),
            "the item pushed just before the waiter parked must be delivered"
        );
    }

    /// A consumer blocked in `pop` must be released by `close` with `Closed`.
    #[test]
    fn close_while_waiting_returns_closed() {
        let q: Arc<BlockingQueue<i32>> = Arc::new(BlockingQueue::new());
        let started = Arc::new(Barrier::new(2));

        let waiter = {
            let q = Arc::clone(&q);
            let started = Arc::clone(&started);
            thread::spawn(move || {
                started.wait();
                q.pop()
            })
        };

        started.wait();
        thread::sleep(Duration::from_millis(30));
        q.close();

        let result = waiter.join().expect("consumer thread joins");
        assert_eq!(result, Err(QueueError::Closed), "close must unblock a waiting consumer");
    }

    /// A push racing a close must linearise: either the item is queued before the
    /// close (and survives for a `pop`), or the push is refused. It must never be
    /// accepted *and* invisible, nor accepted after `close` returned.
    #[test]
    fn close_and_push_linearize() {
        for _ in 0..64 {
            let q: Arc<BlockingQueue<i32>> = Arc::new(BlockingQueue::new());
            let go = Arc::new(Barrier::new(3));
            let pushed_ok = Arc::new(AtomicBool::new(false));

            let pusher = {
                let q = Arc::clone(&q);
                let go = Arc::clone(&go);
                let pushed_ok = Arc::clone(&pushed_ok);
                thread::spawn(move || {
                    go.wait();
                    if q.push(1).is_ok() {
                        pushed_ok.store(true, Ordering::SeqCst);
                    }
                })
            };
            let closer = {
                let q = Arc::clone(&q);
                let go = Arc::clone(&go);
                thread::spawn(move || {
                    go.wait();
                    q.close();
                })
            };

            go.wait();
            pusher.join().expect("producer thread joins");
            closer.join().expect("closer thread joins");

            // Once `close` has returned, no further push may be accepted.
            assert_eq!(
                q.push(2),
                Err(QueueError::Closed),
                "no push may succeed after close returns"
            );

            // A push that reported success must still be observable.
            if pushed_ok.load(Ordering::SeqCst) {
                assert_eq!(q.try_pop(), Some(1), "an accepted push must be queued, not dropped");
            }
            assert!(q.is_closed());
        }
    }

    /// `pop_timeout` must still respect its deadline when nothing arrives.
    #[test]
    fn pop_timeout_respects_the_deadline() {
        let q: BlockingQueue<i32> = BlockingQueue::new();
        let start = Instant::now();
        let result = q.pop_timeout(Duration::from_millis(40));
        let waited = start.elapsed();
        assert_eq!(result, Err(QueueError::Empty), "an empty queue times out as `Empty`");
        assert!(
            waited >= Duration::from_millis(30),
            "the call must have actually waited: {waited:?}"
        );
    }

    /// `pop_timeout` returns a queued item immediately, without waiting out the
    /// deadline.
    #[test]
    fn pop_timeout_returns_a_ready_item() {
        let q = BlockingQueue::new();
        q.push(11).unwrap();
        let result = q.pop_timeout(Duration::from_millis(200));
        assert_eq!(result, Ok(11));
    }

    /// A close that races a `pop_timeout` waiter still reports `Closed` rather
    /// than waiting out the whole deadline.
    #[test]
    fn close_wakes_a_timed_waiter() {
        let q: Arc<BlockingQueue<i32>> = Arc::new(BlockingQueue::new());
        let started = Arc::new(Barrier::new(2));

        let waiter = {
            let q = Arc::clone(&q);
            let started = Arc::clone(&started);
            thread::spawn(move || {
                started.wait();
                q.pop_timeout(Duration::from_secs(10))
            })
        };

        started.wait();
        thread::sleep(Duration::from_millis(30));
        q.close();

        let result = waiter.join().expect("consumer thread joins");
        assert_eq!(result, Err(QueueError::Closed), "close must short-circuit a timed wait");
    }

    /// Items already queued are delivered before the closed check is reached.
    #[test]
    fn queued_items_survive_close() {
        let q: BlockingQueue<i32> = BlockingQueue::new();
        q.push(1).unwrap();
        q.push(2).unwrap();
        q.close();
        assert_eq!(q.pop(), Ok(1));
        assert_eq!(q.pop(), Ok(2));
        assert_eq!(q.pop(), Err(QueueError::Closed));
    }
}
