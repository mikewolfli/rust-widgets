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

/// A thread-safe, unbounded FIFO queue that blocks consumers until an item
/// arrives.
///
/// Any number of producer threads may call [`BlockingQueue::push`] while any
/// number of consumers block in [`BlockingQueue::pop`]. Because it is unbounded,
/// a faster producer can grow the queue without limit; use [`BoundedQueue`] when
/// back-pressure is required. Only available when the `alloc_frugal` feature is
/// off.
#[cfg(not(alloc_frugal))]
#[derive(Debug)]
pub struct BlockingQueue<T> {
    queue: Mutex<VecDeque<T>>,
    condvar: Condvar,
    closed: Mutex<bool>,
}
#[cfg(not(alloc_frugal))]
impl<T> BlockingQueue<T> {
    /// Creates an empty queue with no pre-allocated storage.
    pub fn new() -> Self {
        Self {
            queue: Mutex::new(VecDeque::new()),
            condvar: Condvar::new(),
            closed: Mutex::new(false),
        }
    }
    /// Creates an empty queue whose backing `VecDeque` is pre-allocated for
    /// `capacity` items.
    ///
    /// `capacity` is only a hint used to avoid early reallocations; the queue is
    /// still unbounded and will grow past it as needed.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            queue: Mutex::new(VecDeque::with_capacity(capacity)),
            condvar: Condvar::new(),
            closed: Mutex::new(false),
        }
    }
    /// Appends `item` and wakes one blocked consumer.
    ///
    /// Never blocks on capacity, but returns [`QueueError::Closed`] without
    /// storing the item once [`Self::close`] has been called. A poisoned mutex is
    /// recovered from rather than propagated, so this cannot panic the caller.
    pub fn push(&self, item: T) -> Result<(), QueueError> {
        if *lock(&self.closed) {
            return Err(QueueError::Closed);
        }
        let mut queue = lock(&self.queue);
        queue.push_back(item);
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
        let mut queue = lock(&self.queue);
        loop {
            if let Some(item) = queue.pop_front() {
                return Ok(item);
            }
            if *lock(&self.closed) {
                return Err(QueueError::Closed);
            }
            queue = self.condvar.wait(queue).unwrap_or_else(|e| e.into_inner());
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
        let mut queue = lock(&self.queue);
        loop {
            if let Some(item) = queue.pop_front() {
                return Ok(item);
            }
            if *lock(&self.closed) {
                return Err(QueueError::Closed);
            }
            let elapsed = start.elapsed();
            if elapsed >= timeout {
                return Err(QueueError::Empty);
            }
            let remaining = timeout - elapsed;
            let result =
                self.condvar.wait_timeout(queue, remaining).unwrap_or_else(|e| e.into_inner());
            queue = result.0;
        }
    }
    /// Removes and returns the front item if one is already available, without
    /// blocking; `None` means the queue was momentarily empty.
    pub fn try_pop(&self) -> Option<T> {
        let mut queue = lock(&self.queue);
        queue.pop_front()
    }
    /// Closes the queue and wakes every blocked consumer and producer.
    ///
    /// Subsequent [`Self::push`] calls fail with [`QueueError::Closed`], and
    /// blocked [`Self::pop`] calls return [`QueueError::Closed`]. Items already
    /// queued are *not* discarded, and already-queued items are still returned by
    /// [`Self::pop`] before the closed check is reached. `close` is idempotent and
    /// cannot be undone.
    pub fn close(&self) {
        *lock(&self.closed) = true;
        self.condvar.notify_all();
    }
    /// Returns `true` once [`Self::close`] has been called.
    pub fn is_closed(&self) -> bool {
        *lock(&self.closed)
    }
    /// Number of items currently queued. Takes the same lock as [`Self::push`] and
    /// [`Self::pop`], so the value can be stale as soon as it is returned.
    pub fn len(&self) -> usize {
        lock(&self.queue).len()
    }
    /// Returns `true` when no items are queued at the moment of the call.
    pub fn is_empty(&self) -> bool {
        lock(&self.queue).is_empty()
    }
    /// Removes all queued items, dropping each one. Does not change the closed
    /// state and does not wake blocked consumers, which will simply keep waiting
    /// for the next [`Self::push`].
    pub fn clear(&self) {
        lock(&self.queue).clear();
    }
}
#[cfg(not(alloc_frugal))]
impl<T> Default for BlockingQueue<T> {
    fn default() -> Self {
        Self::new()
    }
}
