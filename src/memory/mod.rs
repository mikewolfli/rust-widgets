mod pool;

pub use pool::*;

use std::alloc::{alloc, dealloc, Layout};
use std::ptr::NonNull;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryStats {
    pub total_allocated: usize,
    pub total_freed: usize,
    pub current_usage: usize,
    pub peak_usage: usize,
    pub pool_hits: usize,
    pub pool_misses: usize,
}

impl Default for MemoryStats {
    fn default() -> Self {
        Self {
            total_allocated: 0,
            total_freed: 0,
            current_usage: 0,
            peak_usage: 0,
            pool_hits: 0,
            pool_misses: 0,
        }
    }
}

impl MemoryStats {
    pub fn record_allocation(&mut self, size: usize) {
        self.total_allocated += size;
        self.current_usage += size;
        if self.current_usage > self.peak_usage {
            self.peak_usage = self.current_usage;
        }
    }

    pub fn record_deallocation(&mut self, size: usize) {
        self.total_freed += size;
        self.current_usage = self.current_usage.saturating_sub(size);
    }

    pub fn record_pool_hit(&mut self) {
        self.pool_hits += 1;
    }

    pub fn record_pool_miss(&mut self) {
        self.pool_misses += 1;
    }

    pub fn pool_hit_rate(&self) -> f32 {
        let total = self.pool_hits + self.pool_misses;
        if total == 0 {
            0.0
        } else {
            self.pool_hits as f32 / total as f32
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AllocationOptions {
    pub alignment: usize,
    pub zeroed: bool,
}

impl Default for AllocationOptions {
    fn default() -> Self {
        Self {
            alignment: 8,
            zeroed: false,
        }
    }
}

pub struct ArenaAllocator {
    buffer: NonNull<u8>,
    layout: Layout,
    offset: usize,
}

impl ArenaAllocator {
    pub fn new(capacity: usize) -> Self {
        let layout = Layout::from_size_align(capacity, 8).expect("Invalid layout");
        let ptr = unsafe { alloc(layout) };
        let buffer = NonNull::new(ptr).expect("Allocation failed");

        Self {
            buffer,
            layout,
            offset: 0,
        }
    }

    pub fn allocate<T>(&mut self) -> Option<NonNull<T>> {
        let size = std::mem::size_of::<T>();
        let align = std::mem::align_of::<T>();

        let aligned_offset = (self.offset + align - 1) & !(align - 1);
        let new_offset = aligned_offset + size;

        if new_offset > self.layout.size() {
            return None;
        }

        self.offset = new_offset;

        let ptr = unsafe {
            let base = self.buffer.as_ptr() as *mut u8;
            NonNull::new_unchecked(base.add(aligned_offset) as *mut T)
        };

        Some(ptr)
    }

    pub fn reset(&mut self) {
        self.offset = 0;
    }

    pub fn capacity(&self) -> usize {
        self.layout.size()
    }

    pub fn used(&self) -> usize {
        self.offset
    }

    pub fn available(&self) -> usize {
        self.layout.size() - self.offset
    }
}

impl Drop for ArenaAllocator {
    fn drop(&mut self) {
        unsafe {
            dealloc(self.buffer.as_ptr(), self.layout);
        }
    }
}

unsafe impl Send for ArenaAllocator {}

pub struct StackAllocator {
    buffer: Vec<u8>,
    offset: usize,
    marks: Vec<usize>,
}

impl StackAllocator {
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: vec![0u8; capacity],
            offset: 0,
            marks: Vec::new(),
        }
    }

    pub fn allocate(&mut self, size: usize, align: usize) -> Option<*mut u8> {
        let aligned_offset = (self.offset + align - 1) & !(align - 1);
        let new_offset = aligned_offset + size;

        if new_offset > self.buffer.len() {
            return None;
        }

        self.offset = new_offset;
        Some(unsafe { self.buffer.as_mut_ptr().add(aligned_offset) })
    }

    pub fn push_marker(&mut self) {
        self.marks.push(self.offset);
    }

    pub fn pop_to_marker(&mut self) {
        if let Some(marker) = self.marks.pop() {
            self.offset = marker;
        }
    }

    pub fn clear(&mut self) {
        self.offset = 0;
        self.marks.clear();
    }

    pub fn capacity(&self) -> usize {
        self.buffer.len()
    }

    pub fn used(&self) -> usize {
        self.offset
    }

    pub fn available(&self) -> usize {
        self.buffer.len() - self.offset
    }
}

impl Default for StackAllocator {
    fn default() -> Self {
        Self::new(4096)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryPressure {
    None,
    Low,
    Medium,
    High,
    Critical,
}

impl Default for MemoryPressure {
    fn default() -> Self {
        Self::None
    }
}

impl MemoryPressure {
    pub fn from_usage(used: usize, total: usize) -> Self {
        let ratio = used as f32 / total as f32;

        if ratio < 0.5 {
            Self::None
        } else if ratio < 0.7 {
            Self::Low
        } else if ratio < 0.85 {
            Self::Medium
        } else if ratio < 0.95 {
            Self::High
        } else {
            Self::Critical
        }
    }
}

pub trait MemoryPressureHandler: Send + Sync {
    fn on_pressure(&mut self, pressure: MemoryPressure);
}

pub struct MemoryMonitor {
    stats: MemoryStats,
    pressure: MemoryPressure,
    handlers: Vec<Box<dyn MemoryPressureHandler>>,
    warning_threshold: usize,
    critical_threshold: usize,
}

impl MemoryMonitor {
    pub fn new(warning_threshold: usize, critical_threshold: usize) -> Self {
        Self {
            stats: MemoryStats::default(),
            pressure: MemoryPressure::None,
            handlers: Vec::new(),
            warning_threshold,
            critical_threshold,
        }
    }

    pub fn stats(&self) -> &MemoryStats {
        &self.stats
    }

    pub fn pressure(&self) -> MemoryPressure {
        self.pressure
    }

    pub fn register_handler(&mut self, handler: Box<dyn MemoryPressureHandler>) {
        self.handlers.push(handler);
    }

    pub fn update(&mut self, current_usage: usize) {
        self.stats.current_usage = current_usage;

        let new_pressure = if current_usage >= self.critical_threshold {
            MemoryPressure::Critical
        } else if current_usage >= self.warning_threshold {
            MemoryPressure::High
        } else {
            MemoryPressure::None
        };

        if new_pressure != self.pressure {
            self.pressure = new_pressure;
            for handler in &mut self.handlers {
                handler.on_pressure(self.pressure);
            }
        }
    }

    pub fn record_allocation(&mut self, size: usize) {
        self.stats.record_allocation(size);
    }

    pub fn record_deallocation(&mut self, size: usize) {
        self.stats.record_deallocation(size);
    }
}

impl Default for MemoryMonitor {
    fn default() -> Self {
        Self::new(1024 * 1024 * 100, 1024 * 1024 * 200)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_stats() {
        let mut stats = MemoryStats::default();

        stats.record_allocation(100);
        assert_eq!(stats.current_usage, 100);
        assert_eq!(stats.peak_usage, 100);

        stats.record_deallocation(50);
        assert_eq!(stats.current_usage, 50);
    }

    #[test]
    fn test_stack_allocator() {
        let mut allocator = StackAllocator::new(1024);

        allocator.push_marker();
        let ptr1 = allocator.allocate(100, 8);
        assert!(ptr1.is_some());

        allocator.pop_to_marker();
        assert_eq!(allocator.used(), 0);
    }

    #[test]
    fn test_memory_pressure() {
        assert_eq!(MemoryPressure::from_usage(25, 100), MemoryPressure::None);
        assert_eq!(MemoryPressure::from_usage(60, 100), MemoryPressure::Low);
        assert_eq!(MemoryPressure::from_usage(80, 100), MemoryPressure::Medium);
        assert_eq!(MemoryPressure::from_usage(90, 100), MemoryPressure::High);
        assert_eq!(
            MemoryPressure::from_usage(98, 100),
            MemoryPressure::Critical
        );
    }
}
