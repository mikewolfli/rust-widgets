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
use alloc::collections::VecDeque;
#[cfg(not(alloc_frugal))]
use core::time::Duration;
/// Default capacity for fixed-size and bounded queues.
pub const DEFAULT_QUEUE_CAPACITY: usize = 256;

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
/// A fixed-capacity ring buffer queue with O(1) push/pop.
///
/// Useful when allocation is undesirable or when a hard upper bound
/// on queued items is known at compile time.
#[derive(Debug)]
pub struct FixedSizeQueue<T, const N: usize = DEFAULT_QUEUE_CAPACITY> {
    buffer: [Option<T>; N],
    head: usize,
    tail: usize,
    len: usize,
}
impl<T, const N: usize> FixedSizeQueue<T, N> {
    /// Creates an empty queue backed by a stack-allocated `[Option<T>; N]` buffer.
    ///
    /// The whole buffer is initialised immediately, so `N` should stay modest for
    /// large `T`. Nothing is allocated on the heap.
    pub fn new() -> Self {
        Self { buffer: core::array::from_fn(|_| None), head: 0, tail: 0, len: 0 }
    }
    /// Appends `item` to the back of the queue.
    ///
    /// Returns [`QueueError::Full`] without storing anything when all `N` slots are
    /// occupied; this queue never grows and never blocks.
    pub fn push(&mut self, item: T) -> Result<(), QueueError> {
        if self.len >= N {
            return Err(QueueError::Full);
        }
        self.buffer[self.tail] = Some(item);
        self.tail = (self.tail + 1) % N;
        self.len += 1;
        Ok(())
    }
    /// Removes and returns the item at the front of the queue.
    ///
    /// Returns `None` when the queue is empty. The vacated slot is set back to
    /// `None`, so the removed value is dropped exactly once.
    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }
        let item = self.buffer[self.head].take();
        self.head = (self.head + 1) % N;
        self.len -= 1;
        item
    }
    /// Returns a reference to the item at the front of the queue without
    /// removing it, or `None` when the queue is empty.
    pub fn peek(&self) -> Option<&T> {
        if self.len == 0 {
            None
        } else {
            self.buffer[self.head].as_ref()
        }
    }
    /// Number of items currently held (always in `0..=N`).
    pub fn len(&self) -> usize {
        self.len
    }
    /// Returns `true` when no items are held.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    /// Returns `true` when all `N` slots are occupied, so the next [`Self::push`]
    /// would fail with [`QueueError::Full`].
    pub fn is_full(&self) -> bool {
        self.len >= N
    }
    /// Total number of slots, i.e. the const generic `N`. Constant for the
    /// lifetime of the queue.
    pub fn capacity(&self) -> usize {
        N
    }
    /// Removes all items, dropping each one, and resets head/tail to slot 0.
    pub fn clear(&mut self) {
        for i in 0..N {
            self.buffer[i] = None;
        }
        self.head = 0;
        self.tail = 0;
        self.len = 0;
    }
    /// Number of free slots, i.e. `capacity() - len()`. Never underflows because
    /// `len` is capped at `N`.
    pub fn available(&self) -> usize {
        N - self.len
    }
}
impl<T, const N: usize> Default for FixedSizeQueue<T, N> {
    fn default() -> Self {
        Self::new()
    }
}
/// A multi-level priority queue backed by 8 internal VecDeques (levels 0-7).
///
/// Items are dequeued from the highest non-empty level first.
#[derive(Debug)]
pub struct PriorityQueue<T> {
    queues: [VecDeque<T>; 8],
    len: usize,
}
impl<T> PriorityQueue<T> {
    /// Creates an empty queue with all 8 priority levels allocated (all heap-backed
    /// `VecDeque`s, each growing as needed).
    pub fn new() -> Self {
        Self { queues: Default::default(), len: 0 }
    }
    /// Appends `item` to the given priority level.
    ///
    /// `priority` is clamped to `0..=7`: `0` is dequeued first and `7` last, and
    /// any value above `7` is treated as `7`. Overflowing the level is impossible
    /// because this queue grows without bound; it never blocks and never returns
    /// an error.
    pub fn push(&mut self, item: T, priority: u8) {
        let priority = (priority.min(7)) as usize;
        self.queues[priority].push_back(item);
        self.len += 1;
    }
    /// Removes and returns the item at the front of the highest-priority
    /// non-empty level, or `None` when the queue is empty.
    ///
    /// Each level is itself FIFO, so equal-priority items keep insertion order.
    pub fn pop(&mut self) -> Option<T> {
        for queue in &mut self.queues {
            if let Some(item) = queue.pop_front() {
                self.len -= 1;
                return Some(item);
            }
        }
        None
    }
    /// Returns the item that the next [`Self::pop`] would return, without removing
    /// it, or `None` when the queue is empty.
    pub fn peek(&self) -> Option<&T> {
        for queue in &self.queues {
            if let Some(item) = queue.front() {
                return Some(item);
            }
        }
        None
    }
    /// Total number of items across all 8 priority levels.
    pub fn len(&self) -> usize {
        self.len
    }
    /// Returns `true` when no items are queued at any priority level.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    /// Removes all items from every priority level, dropping each one.
    pub fn clear(&mut self) {
        for queue in &mut self.queues {
            queue.clear();
        }
        self.len = 0;
    }
    /// Number of items currently queued at `priority`.
    ///
    /// `priority` is clamped to `0..=7` exactly as in [`Self::push`], so a
    /// priority of `9` reports the count for level `7`.
    pub fn count_priority(&self, priority: u8) -> usize {
        let priority = (priority.min(7)) as usize;
        self.queues[priority].len()
    }
}
impl<T> Default for PriorityQueue<T> {
    fn default() -> Self {
        Self::new()
    }
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
/// A thread-safe FIFO queue with a fixed maximum number of items, providing
/// back-pressure to producers.
///
/// Safe to share between threads. When full, [`BoundedQueue::push`] blocks the
/// producer instead of dropping or overwriting items; use
/// [`BoundedQueue::try_push`] for a non-blocking variant. Only available when the
/// `alloc_frugal` feature is off.
#[cfg(not(alloc_frugal))]
#[derive(Debug)]
pub struct BoundedQueue<T> {
    queue: Mutex<VecDeque<T>>,
    condvar_not_full: Condvar,
    condvar_not_empty: Condvar,
    capacity: usize,
    closed: Mutex<bool>,
}
#[cfg(not(alloc_frugal))]
impl<T> BoundedQueue<T> {
    /// Creates an empty queue holding at most `capacity` items.
    ///
    /// `capacity` is required, not advisory: once the queue holds that many items
    /// [`Self::push`] blocks until a consumer makes room. A `capacity` of `0`
    /// makes every [`Self::push`] block forever (until [`Self::close`]), so it is
    /// rarely useful. The backing `VecDeque` is pre-allocated for `capacity` and
    /// will never grow beyond it.
    pub fn new(capacity: usize) -> Self {
        Self {
            queue: Mutex::new(VecDeque::with_capacity(capacity)),
            condvar_not_full: Condvar::new(),
            condvar_not_empty: Condvar::new(),
            capacity,
            closed: Mutex::new(false),
        }
    }
    /// Appends `item`, blocking the calling thread while the queue is full.
    ///
    /// Wakes one blocked consumer on success. Returns [`QueueError::Closed`] if
    /// [`Self::close`] happens while blocked (or was already called); in that case
    /// `item` is not stored. Never returns [`QueueError::Full`] — that is reserved
    /// for [`Self::try_push`].
    pub fn push(&self, item: T) -> Result<(), QueueError> {
        let mut queue = lock(&self.queue);
        while queue.len() >= self.capacity {
            if *lock(&self.closed) {
                return Err(QueueError::Closed);
            }
            queue = self.condvar_not_full.wait(queue).unwrap_or_else(|e| e.into_inner());
        }
        queue.push_back(item);
        self.condvar_not_empty.notify_one();
        Ok(())
    }
    /// Appends `item` without ever blocking.
    ///
    /// Returns [`QueueError::Full`] if the queue is at capacity, or
    /// [`QueueError::Closed`] if the queue has been closed; in both cases `item`
    /// is handed back by being dropped, and nothing is queued.
    pub fn try_push(&self, item: T) -> Result<(), QueueError> {
        if *lock(&self.closed) {
            return Err(QueueError::Closed);
        }
        let mut queue = lock(&self.queue);
        if queue.len() >= self.capacity {
            return Err(QueueError::Full);
        }
        queue.push_back(item);
        self.condvar_not_empty.notify_one();
        Ok(())
    }
    /// Removes and returns the front item, blocking until one is available and
    /// waking one blocked producer to claim the freed slot.
    ///
    /// Returns [`QueueError::Closed`] only when the queue is empty *and* has been
    /// closed; items still queued are drained normally after {@link close}.
    pub fn pop(&self) -> Result<T, QueueError> {
        let mut queue = lock(&self.queue);
        loop {
            if let Some(item) = queue.pop_front() {
                self.condvar_not_full.notify_one();
                return Ok(item);
            }
            if *lock(&self.closed) {
                return Err(QueueError::Closed);
            }
            queue = self.condvar_not_empty.wait(queue).unwrap_or_else(|e| e.into_inner());
        }
    }
    /// Removes and returns the front item if one is available, without blocking.
    /// `None` means the queue was momentarily empty; it does not indicate a
    /// closed queue. Wakes one blocked producer when an item was removed.
    pub fn try_pop(&self) -> Option<T> {
        let mut queue = lock(&self.queue);
        let item = queue.pop_front();
        if item.is_some() {
            self.condvar_not_full.notify_one();
        }
        item
    }
    /// Closes the queue, waking all blocked producers and consumers.
    ///
    /// Blocked producers give up with [`QueueError::Closed`] and blocked
    /// consumers wake so they can observe the closed flag; already-queued items
    /// remain available to [`Self::pop`]. Idempotent and irreversible.
    pub fn close(&self) {
        *lock(&self.closed) = true;
        self.condvar_not_full.notify_all();
        self.condvar_not_empty.notify_all();
    }
    /// Returns `true` once [`Self::close`] has been called.
    pub fn is_closed(&self) -> bool {
        *lock(&self.closed)
    }
    /// Number of items currently queued; at most [`Self::capacity`].
    pub fn len(&self) -> usize {
        lock(&self.queue).len()
    }
    /// Returns `true` when no items are queued at the moment of the call.
    pub fn is_empty(&self) -> bool {
        lock(&self.queue).is_empty()
    }
    /// Returns `true` when the queue holds [`Self::capacity`] items, meaning the
    /// next [`Self::push`] would block.
    pub fn is_full(&self) -> bool {
        lock(&self.queue).len() >= self.capacity
    }
    /// Maximum number of items the queue can hold, as passed to [`Self::new`].
    pub fn capacity(&self) -> usize {
        self.capacity
    }
    /// Removes all queued items, dropping each one, and wakes all blocked
    /// producers so they can refill the space.
    pub fn clear(&self) {
        let mut queue = lock(&self.queue);
        queue.clear();
        self.condvar_not_full.notify_all();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_fixed_size_queue() {
        let mut queue: FixedSizeQueue<i32, 4> = FixedSizeQueue::new();
        assert!(queue.is_empty());
        assert_eq!(queue.capacity(), 4);
        queue.push(1).unwrap();
        queue.push(2).unwrap();
        queue.push(3).unwrap();
        assert_eq!(queue.len(), 3);
        assert_eq!(queue.pop(), Some(1));
        assert_eq!(queue.pop(), Some(2));
        assert_eq!(queue.pop(), Some(3));
        assert_eq!(queue.pop(), None);
    }
    #[test]
    fn test_priority_queue() {
        let mut queue: PriorityQueue<&str> = PriorityQueue::new();
        queue.push("low", 7);
        queue.push("high", 0);
        queue.push("medium", 4);
        assert_eq!(queue.pop(), Some("high"));
        assert_eq!(queue.pop(), Some("medium"));
        assert_eq!(queue.pop(), Some("low"));
    }
}
