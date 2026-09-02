#[cfg(not(feature = "mini"))]
use crate::compat::Condvar;
#[cfg(not(feature = "mini"))]
use crate::compat::Instant;
#[cfg(not(feature = "mini"))]
use crate::compat::Mutex;
use alloc::collections::VecDeque;
#[cfg(not(feature = "mini"))]
use core::time::Duration;
/// Default capacity for fixed-size and bounded queues.
pub const DEFAULT_QUEUE_CAPACITY: usize = 256;

/// Errors returned by queue operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueError {
    Full,
    Empty,
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
    pub fn new() -> Self {
        Self { buffer: core::array::from_fn(|_| None), head: 0, tail: 0, len: 0 }
    }
    pub fn push(&mut self, item: T) -> Result<(), QueueError> {
        if self.len >= N {
            return Err(QueueError::Full);
        }
        self.buffer[self.tail] = Some(item);
        self.tail = (self.tail + 1) % N;
        self.len += 1;
        Ok(())
    }
    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }
        let item = self.buffer[self.head].take();
        self.head = (self.head + 1) % N;
        self.len -= 1;
        item
    }
    pub fn peek(&self) -> Option<&T> {
        if self.len == 0 {
            None
        } else {
            self.buffer[self.head].as_ref()
        }
    }
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    pub fn is_full(&self) -> bool {
        self.len >= N
    }
    pub fn capacity(&self) -> usize {
        N
    }
    pub fn clear(&mut self) {
        for i in 0..N {
            self.buffer[i] = None;
        }
        self.head = 0;
        self.tail = 0;
        self.len = 0;
    }
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
    pub fn new() -> Self {
        Self { queues: Default::default(), len: 0 }
    }
    pub fn push(&mut self, item: T, priority: u8) {
        let priority = (priority.min(7)) as usize;
        self.queues[priority].push_back(item);
        self.len += 1;
    }
    pub fn pop(&mut self) -> Option<T> {
        for queue in &mut self.queues {
            if let Some(item) = queue.pop_front() {
                self.len -= 1;
                return Some(item);
            }
        }
        None
    }
    pub fn peek(&self) -> Option<&T> {
        for queue in &self.queues {
            if let Some(item) = queue.front() {
                return Some(item);
            }
        }
        None
    }
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    pub fn clear(&mut self) {
        for queue in &mut self.queues {
            queue.clear();
        }
        self.len = 0;
    }
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
#[cfg(not(feature = "mini"))]
#[derive(Debug)]
pub struct BlockingQueue<T> {
    queue: Mutex<VecDeque<T>>,
    condvar: Condvar,
    closed: Mutex<bool>,
}
#[cfg(not(feature = "mini"))]
impl<T> BlockingQueue<T> {
    pub fn new() -> Self {
        Self {
            queue: Mutex::new(VecDeque::new()),
            condvar: Condvar::new(),
            closed: Mutex::new(false),
        }
    }
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            queue: Mutex::new(VecDeque::with_capacity(capacity)),
            condvar: Condvar::new(),
            closed: Mutex::new(false),
        }
    }
    pub fn push(&self, item: T) -> Result<(), QueueError> {
        if *self.closed.lock().unwrap_or_else(|e| e.into_inner()) {
            return Err(QueueError::Closed);
        }
        let mut queue = self.queue.lock().unwrap_or_else(|e| e.into_inner());
        queue.push_back(item);
        self.condvar.notify_one();
        Ok(())
    }
    pub fn pop(&self) -> Result<T, QueueError> {
        let mut queue = self.queue.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            if let Some(item) = queue.pop_front() {
                return Ok(item);
            }
            if *self.closed.lock().unwrap_or_else(|e| e.into_inner()) {
                return Err(QueueError::Closed);
            }
            queue = self.condvar.wait(queue).unwrap_or_else(|e| e.into_inner());
        }
    }
    pub fn pop_timeout(&self, timeout: Duration) -> Result<T, QueueError> {
        let start = Instant::now();
        let mut queue = self.queue.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            if let Some(item) = queue.pop_front() {
                return Ok(item);
            }
            if *self.closed.lock().unwrap_or_else(|e| e.into_inner()) {
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
    pub fn try_pop(&self) -> Option<T> {
        let mut queue = self.queue.lock().unwrap_or_else(|e| e.into_inner());
        queue.pop_front()
    }
    pub fn close(&self) {
        *self.closed.lock().unwrap_or_else(|e| e.into_inner()) = true;
        self.condvar.notify_all();
    }
    pub fn is_closed(&self) -> bool {
        *self.closed.lock().unwrap_or_else(|e| e.into_inner())
    }
    pub fn len(&self) -> usize {
        self.queue.lock().unwrap_or_else(|e| e.into_inner()).len()
    }
    pub fn is_empty(&self) -> bool {
        self.queue.lock().unwrap_or_else(|e| e.into_inner()).is_empty()
    }
    pub fn clear(&self) {
        self.queue.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }
}
#[cfg(not(feature = "mini"))]
impl<T> Default for BlockingQueue<T> {
    fn default() -> Self {
        Self::new()
    }
}
#[cfg(not(feature = "mini"))]
#[derive(Debug)]
pub struct BoundedQueue<T> {
    queue: Mutex<VecDeque<T>>,
    condvar_not_full: Condvar,
    condvar_not_empty: Condvar,
    capacity: usize,
    closed: Mutex<bool>,
}
#[cfg(not(feature = "mini"))]
impl<T> BoundedQueue<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            queue: Mutex::new(VecDeque::with_capacity(capacity)),
            condvar_not_full: Condvar::new(),
            condvar_not_empty: Condvar::new(),
            capacity,
            closed: Mutex::new(false),
        }
    }
    pub fn push(&self, item: T) -> Result<(), QueueError> {
        let mut queue = self.queue.lock().unwrap_or_else(|e| e.into_inner());
        while queue.len() >= self.capacity {
            if *self.closed.lock().unwrap_or_else(|e| e.into_inner()) {
                return Err(QueueError::Closed);
            }
            queue = self.condvar_not_full.wait(queue).unwrap_or_else(|e| e.into_inner());
        }
        queue.push_back(item);
        self.condvar_not_empty.notify_one();
        Ok(())
    }
    pub fn try_push(&self, item: T) -> Result<(), QueueError> {
        if *self.closed.lock().unwrap_or_else(|e| e.into_inner()) {
            return Err(QueueError::Closed);
        }
        let mut queue = self.queue.lock().unwrap_or_else(|e| e.into_inner());
        if queue.len() >= self.capacity {
            return Err(QueueError::Full);
        }
        queue.push_back(item);
        self.condvar_not_empty.notify_one();
        Ok(())
    }
    pub fn pop(&self) -> Result<T, QueueError> {
        let mut queue = self.queue.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            if let Some(item) = queue.pop_front() {
                self.condvar_not_full.notify_one();
                return Ok(item);
            }
            if *self.closed.lock().unwrap_or_else(|e| e.into_inner()) {
                return Err(QueueError::Closed);
            }
            queue = self.condvar_not_empty.wait(queue).unwrap_or_else(|e| e.into_inner());
        }
    }
    pub fn try_pop(&self) -> Option<T> {
        let mut queue = self.queue.lock().unwrap_or_else(|e| e.into_inner());
        let item = queue.pop_front();
        if item.is_some() {
            self.condvar_not_full.notify_one();
        }
        item
    }
    pub fn close(&self) {
        *self.closed.lock().unwrap_or_else(|e| e.into_inner()) = true;
        self.condvar_not_full.notify_all();
        self.condvar_not_empty.notify_all();
    }
    pub fn is_closed(&self) -> bool {
        *self.closed.lock().unwrap_or_else(|e| e.into_inner())
    }
    pub fn len(&self) -> usize {
        self.queue.lock().unwrap_or_else(|e| e.into_inner()).len()
    }
    pub fn is_empty(&self) -> bool {
        self.queue.lock().unwrap_or_else(|e| e.into_inner()).is_empty()
    }
    pub fn is_full(&self) -> bool {
        self.queue.lock().unwrap_or_else(|e| e.into_inner()).len() >= self.capacity
    }
    pub fn capacity(&self) -> usize {
        self.capacity
    }
    pub fn clear(&self) {
        let mut queue = self.queue.lock().unwrap_or_else(|e| e.into_inner());
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
