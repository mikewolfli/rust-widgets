// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use crate::compat::{vec, Box, Vec};
use alloc::alloc::{alloc, dealloc};
use core::alloc::Layout;
use core::ptr::NonNull;

/// Counters describing how much memory an application has requested and how well
/// its pools are serving those requests.
///
/// All byte counts are in bytes and only count what was explicitly reported via
/// [`MemoryStats::record_allocation`] / [`MemoryStats::record_deallocation`];
/// this struct observes nothing by itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MemoryStats {
    /// Cumulative bytes ever reported as allocated.
    pub total_allocated: usize,
    /// Cumulative bytes ever reported as freed.
    pub total_freed: usize,
    /// Bytes currently considered live, i.e. allocated minus freed.
    pub current_usage: usize,
    /// High-water mark of [`Self::current_usage`] since the last reset.
    pub peak_usage: usize,
    /// Number of times a request was served from a pool instead of freshly
    /// allocated.
    pub pool_hits: usize,
    /// Number of times a request had to allocate because no pool was available.
    pub pool_misses: usize,
}

impl MemoryStats {
    /// Records that `size` bytes were allocated, in bytes.
    ///
    /// Raises [`Self::current_usage`] and, if it exceeds it, [`Self::peak_usage`].
    /// Both are plain additions with no overflow check, so counters are not
    /// saturating.
    pub fn record_allocation(&mut self, size: usize) {
        self.total_allocated = self.total_allocated.saturating_add(size);
        self.current_usage = self.current_usage.saturating_add(size);
        if self.current_usage > self.peak_usage {
            self.peak_usage = self.current_usage;
        }
    }

    /// Records that `size` bytes were freed, in bytes.
    ///
    /// Lowers [`Self::current_usage`] with a saturating subtraction, so freeing
    /// more than was ever allocated clamps at zero rather than wrapping. Peak usage
    /// is deliberately left alone, since it is a high-water mark.
    pub fn record_deallocation(&mut self, size: usize) {
        self.total_freed = self.total_freed.saturating_add(size);
        self.current_usage = self.current_usage.saturating_sub(size);
    }

    /// Counts one request that was satisfied from a pool.
    #[inline]
    pub fn record_pool_hit(&mut self) {
        self.pool_hits = self.pool_hits.saturating_add(1);
    }

    /// Counts one request that had to allocate rather than reuse a pooled buffer.
    #[inline]
    pub fn record_pool_miss(&mut self) {
        self.pool_misses = self.pool_misses.saturating_add(1);
    }

    /// Fraction of pool requests that were hits, in `0.0..=1.0`.
    ///
    /// Returns `0.0` when no requests have been recorded at all, so the "no data"
    /// case is indistinguishable from "every request missed".
    pub fn pool_hit_rate(&self) -> f32 {
        let total = self.pool_hits.saturating_add(self.pool_misses);
        if total == 0 {
            0.0
        } else {
            self.pool_hits as f32 / total as f32
        }
    }
}

/// Request options for an allocator-backed allocation.
#[derive(Debug, Clone, Copy)]
pub struct AllocationOptions {
    /// Required alignment in bytes. Must be a power of two, as required by
    /// [`Layout`].
    pub alignment: usize,
    /// Whether the returned memory must be zero-filled. `false` leaves the bytes
    /// as garbage, so readers must initialise before use.
    pub zeroed: bool,
}

impl Default for AllocationOptions {
    /// Defaults to 8-byte alignment and no zero-filling.
    fn default() -> Self {
        Self { alignment: 8, zeroed: false }
    }
}

/// A bump allocator over one contiguous, heap-owned block.
///
/// This is a *general arena*, not a buffer pool: [`Self::allocate`] hands out
/// raw, uninitialised memory of any requested type, and there is no per-object
/// reuse or reset of contents. Allocation is O(1); freeing is only possible in
/// bulk via [`Self::reset`] or by dropping the whole arena, so any memory the
/// caller allocated stays live (and uninit) until then.
///
/// The backing block is allocated once in [`Self::new`], so it is held for the
/// lifetime of the value regardless of use.
#[derive(Debug)]
pub struct ArenaAllocator {
    buffer: NonNull<u8>,
    layout: Layout,
    offset: usize,
}

impl ArenaAllocator {
    /// Creates an arena owning a `capacity`-byte block, 8-byte aligned.
    ///
    /// A `capacity` of zero is legal and yields an **empty arena**: the backing block is a dangling,
    /// never-dereferenced pointer and every [`Self::allocate`] returns `None`. Zero must not reach
    /// `alloc`, whose contract requires a non-zero layout, so the empty case is represented without
    /// an allocation rather than by passing a zero-size layout through.
    ///
    /// # Panics
    ///
    /// Panics if the layout is otherwise invalid (i.e. the size cannot fit an 8-byte alignment),
    /// and aborts on out-of-memory — there is no fallible constructor.
    pub fn new(capacity: usize) -> Self {
        if capacity == 0 {
            // An empty arena never owns a block: keep the layout as the zero-size/8-align layout
            // (used only to report `capacity()`) and a dangling, aligned pointer that `allocate`
            // rejects before it is ever read. `Drop` skips deallocation in this case.
            let layout = Layout::from_size_align(0, 8)
                .expect("a zero size with an 8-byte alignment is a valid layout");
            return Self { buffer: NonNull::dangling(), layout, offset: 0 };
        }
        let layout = Layout::from_size_align(capacity, 8).expect(
            "capacity must be non-zero and fit an 8-byte alignment, which `Arena::new` documents \
             as a panic condition",
        );
        // SAFETY: layout is validated by Layout::from_size_align, which ensures
        // non-zero size and valid alignment. alloc() is guaranteed to return a
        // properly aligned pointer or abort on OOM.
        let ptr = unsafe { alloc(layout) };
        let buffer = NonNull::new(ptr).expect(
            "the global allocator returned null for a non-zero layout, which aborts by \
                     contract",
        );
        Self { buffer, layout, offset: 0 }
    }

    /// Bumps the arena forward by `size_of::<T>()` and returns a pointer to the
    /// new slot.
    ///
    /// Returns `None` — rather than growing or panicking — when the arena lacks
    /// room, leaving the arena unchanged, so a caller can recover or allocate
    /// elsewhere. The returned memory is *not* initialised and `T` is never
    /// constructed; the caller must write it (e.g. `ptr::write`) exactly once. The
    /// arena will not drop `T` for you. The type's own alignment is honoured, which
    /// may skip a few padding bytes before the slot.
    pub fn allocate<T>(&mut self) -> Option<NonNull<T>> {
        let size = core::mem::size_of::<T>();
        let align = core::mem::align_of::<T>();
        // An empty arena (or one shrunk to nothing) has no block to bump; return `None` before any
        // pointer arithmetic so the dangling base is never dereferenced.
        if self.layout.size() == 0 || self.offset > self.layout.size() {
            return None;
        }
        // Align the *absolute* address, not just the offset. The backing block is
        // only 8-byte aligned (see `Self::new`), so masking the offset alone cannot
        // satisfy a caller that needs, say, 64-byte alignment: the base address's own
        // misalignment has to be folded into the padding. Compute the offset that
        // makes `base + offset` a multiple of `align`, using the pointer's address as
        // the authority rather than assuming the base is aligned.
        let base = self.buffer.as_ptr() as usize;
        let aligned_addr = base.checked_add(self.offset)?.checked_add(align - 1)? & !(align - 1);
        let aligned_offset = aligned_addr.checked_sub(base)?;
        // Checked, not plain: the `?` above guards only the alignment padding, so
        // without this an `aligned_offset` within `size` of `usize::MAX` would wrap
        // `new_offset` to a small value, pass the bounds test below, and hand out a
        // pointer past the end of the buffer. The layout makes that unreachable
        // today (a real allocation is far from `usize::MAX`), but this function
        // returns a raw pointer, so the check is worth having in the arithmetic
        // rather than in an argument about the caller.
        let new_offset = aligned_offset.checked_add(size)?;
        if new_offset > self.layout.size() {
            return None;
        }
        self.offset = new_offset;
        // SAFETY: aligned_offset is verified against self.layout.size() above,
        // ensuring the pointer stays within the allocated buffer. The absolute
        // address base+aligned_offset is a multiple of align, so the returned pointer
        // is correctly aligned for T. The buffer is guaranteed to be live since
        // ArenaAllocator owns it and keeps it until drop.
        let ptr = unsafe {
            let base = self.buffer.as_ptr();
            NonNull::new_unchecked(base.add(aligned_offset) as *mut T)
        };
        Some(ptr)
    }

    /// Rewinds the bump pointer to the start, making the whole block available
    /// again.
    ///
    /// This does not run destructors and does not clear the memory, so any values
    /// previously placed here are leaked (in the `mem::forget` sense) and later
    /// allocations will alias their old bytes.
    pub fn reset(&mut self) {
        self.offset = 0;
    }

    /// Total size of the arena block in bytes, as requested in [`Self::new`].
    pub fn capacity(&self) -> usize {
        self.layout.size()
    }

    /// Bytes handed out so far, in bytes. Includes alignment padding, so it can
    /// exceed the sum of the requested allocation sizes.
    pub fn used(&self) -> usize {
        self.offset
    }

    /// Bytes never handed out, i.e. `capacity() - used()`. Does not account for
    /// padding that future allocations may need.
    pub fn available(&self) -> usize {
        self.layout.size() - self.offset
    }
}

impl Drop for ArenaAllocator {
    fn drop(&mut self) {
        // An empty arena never allocated a block, so there is nothing to free. Calling dealloc with
        // a zero-size layout would violate `dealloc`'s non-zero-size contract, so it is skipped.
        if self.layout.size() == 0 {
            return;
        }
        // SAFETY: self.buffer was allocated with self.layout in ArenaAllocator::new(),
        // and this is the only deallocation (no aliased frees). The buffer is
        // guaranteed to be non-null and valid until this Drop runs.
        unsafe {
            dealloc(self.buffer.as_ptr(), self.layout);
        }
    }
}

/// SAFETY: the arena owns its buffer exclusively and exposes only `&mut self`
/// accessors, so moving it between threads transfers the sole handle.
unsafe impl Send for ArenaAllocator {}

/// A bump allocator with LIFO marker support, backed by a heap `Vec<u8>`.
///
/// Like [`ArenaAllocator`] this is a general scratch allocator, not a pool: it
/// hands out raw uninitialised bytes and never reuses a region's contents. Its
/// distinguishing feature is [`Self::push_marker`] / [`Self::pop_to_marker`],
/// which allow nesting scopes and reclaiming everything allocated inside one. Not
/// thread-safe on its own.
#[derive(Debug)]
pub struct StackAllocator {
    buffer: Vec<u8>,
    offset: usize,
    marks: Vec<usize>,
}

impl StackAllocator {
    /// Creates an allocator with a zero-initialised `capacity`-byte buffer.
    ///
    /// The buffer is allocated immediately and zero-filled, so the cost is paid up
    /// front even if nothing is allocated from it.
    pub fn new(capacity: usize) -> Self {
        Self { buffer: vec![0u8; capacity], offset: 0, marks: Vec::new() }
    }

    /// Allocates `size` bytes at the given `align` boundary (in bytes) and returns
    /// a pointer to them.
    ///
    /// Returns `None` without changing state when the buffer is too small **or** when
    /// `align` is not a power of two — a non-power-of-two alignment would silently
    /// produce a misaligned pointer (undefined behaviour once dereferenced), so it is
    /// rejected rather than rounded to an arbitrary boundary. `size` of zero is allowed
    /// and yields a pointer that must not be dereferenced.
    pub fn allocate(&mut self, size: usize, align: usize) -> Option<*mut u8> {
        // A zero or non-power-of-two alignment cannot be honoured by the masking
        // trick below; refusing is the only correct answer because a misaligned
        // pointer is UB for the caller that dereferences it.
        if !align.is_power_of_two() {
            return None;
        }
        // Align the *absolute* address: a `Vec<u8>` guarantees only byte alignment,
        // so masking the offset alone would leave the base's own misalignment in the
        // returned pointer. Fold the base address into the padding computation.
        let base = self.buffer.as_mut_ptr() as usize;
        let aligned_addr = base.checked_add(self.offset)?.checked_add(align - 1)? & !(align - 1);
        let aligned_offset = aligned_addr.checked_sub(base)?;
        let new_offset = aligned_offset.checked_add(size)?;
        if new_offset > self.buffer.len() {
            return None;
        }
        self.offset = new_offset;
        // SAFETY: aligned_offset is checked against self.buffer.len() above,
        // guaranteeing the pointer is within the allocated Vec's storage. The
        // absolute address base+aligned_offset is a multiple of align, so the
        // returned pointer is correctly aligned. The Vec's buffer is guaranteed to
        // be valid until the Vec is dropped (or reallocated, which never happens here
        // because the buffer is fixed at construction).
        Some(unsafe { self.buffer.as_mut_ptr().add(aligned_offset) })
    }

    /// Records the current bump position on an internal stack.
    ///
    /// Markers nest; the most recent one is what [`Self::pop_to_marker`] returns
    /// to. Pushing is cheap but markers are never cleaned up implicitly, so unmatched
    /// pushes accumulate until [`Self::clear`].
    pub fn push_marker(&mut self) {
        self.marks.push(self.offset);
    }

    /// Discards the most recent marker and rewinds the bump pointer to it,
    /// reclaiming everything allocated since that marker.
    //
    // No destructors run and the reclaimed bytes are left as-is.
    /// Does nothing when no marker is outstanding.
    pub fn pop_to_marker(&mut self) {
        if let Some(marker) = self.marks.pop() {
            self.offset = marker;
        }
    }

    /// Rewinds the bump pointer to the start and discards every outstanding
    /// marker.
    ///
    /// Does not free the backing buffer or clear its bytes.
    pub fn clear(&mut self) {
        self.offset = 0;
        self.marks.clear();
    }

    /// Size of the backing buffer in bytes; fixed for the lifetime of the
    /// allocator.
    pub fn capacity(&self) -> usize {
        self.buffer.len()
    }

    /// Bytes handed out so far, including alignment padding, so this can exceed
    /// the sum of requested sizes.
    pub fn used(&self) -> usize {
        self.offset
    }

    /// Bytes remaining before the next allocation would fail, i.e.
    /// `capacity() - used()`. Padding for a future aligned allocation is not
    /// deducted.
    pub fn available(&self) -> usize {
        self.buffer.len() - self.offset
    }
}

impl Default for StackAllocator {
    /// Defaults to a 4096-byte (4 KiB) buffer.
    fn default() -> Self {
        Self::new(4096)
    }
}

/// A coarse classification of how close memory usage is to its limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MemoryPressure {
    /// Under 50% of the budget is in use. The default level.
    #[default]
    None,
    /// At least 50% but under 70% is in use.
    Low,
    /// At least 70% but under 85% is in use.
    Medium,
    /// At least 85% but under 95% is in use.
    High,
    /// 95% or more of the budget is in use.
    Critical,
}

impl MemoryPressure {
    /// Classifies `used` bytes against a `total`-byte budget.
    ///
    /// Both are byte counts, but only the ratio between them matters. Thresholds
    /// are, in ascending order, 50%, 70%, 85% and 95%; the highest matching band
    /// wins, so `used == total` is [`MemoryPressure::Critical`].
    ///
    /// A `total` of zero is *not* special-cased: the division yields NaN, every
    /// comparison fails, and the result is [`MemoryPressure::Critical`].
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

/// Callback invoked when an application's memory pressure level changes.
///
/// Implementors are stored in a [`MemoryMonitor`] and invoked on its thread, so
/// they should be cheap and must not block indefinitely. `Send + Sync` is required
/// because the monitor may be moved between threads.
pub trait MemoryPressureHandler: Send + Sync {
    /// Runs when the level transitions to `pressure`, which is always different
    /// from the level previously reported. Handlers are called in registration
    /// order.
    fn on_pressure(&mut self, pressure: MemoryPressure);
}

/// Tracks memory usage counters and notifies handlers on pressure transitions.
///
/// This is a passive monitor: it never queries the system, and `stats` is only
/// updated by explicit calls to [`Self::update`], [`Self::record_allocation`] and
/// [`Self::record_deallocation`]. Not thread-safe; not internally synchronised.
pub struct MemoryMonitor {
    stats: MemoryStats,
    pressure: MemoryPressure,
    handlers: Vec<Box<dyn MemoryPressureHandler>>,
    warning_threshold: usize,
    critical_threshold: usize,
}

impl MemoryMonitor {
    /// Creates a monitor with the given byte thresholds and no handlers.
    ///
    /// Both thresholds are byte counts, in bytes. Because [`Self::update`] checks
    /// `critical` first, a `critical_threshold` below `warning_threshold` simply
    /// makes the warning band unreachable rather than panicking. The initial
    /// pressure is [`MemoryPressure::None`] regardless of the thresholds.
    pub fn new(warning_threshold: usize, critical_threshold: usize) -> Self {
        Self {
            stats: MemoryStats::default(),
            pressure: MemoryPressure::None,
            handlers: Vec::new(),
            warning_threshold,
            critical_threshold,
        }
    }

    /// Borrows the accumulated counters. Only the fields written by this monitor
    /// are meaningful; peak and pool counters come from the recorded calls.
    pub fn stats(&self) -> &MemoryStats {
        &self.stats
    }

    /// The most recently computed pressure level.
    ///
    /// This is the level from the last [`Self::update`] that changed it, so it is
    /// [`MemoryPressure::None`] until an update crosses the warning threshold.
    /// Note this is the monitor's own coarse banding and is unrelated to
    /// [`MemoryPressure::from_usage`]'s ratio bands.
    pub fn pressure(&self) -> MemoryPressure {
        self.pressure
    }

    /// Appends a handler to be notified on future pressure transitions.
    ///
    /// Handlers are never removed individually and are called in registration
    /// order; only transitions fire, so registering now does not replay the
    /// current level.
    pub fn register_handler(&mut self, handler: Box<dyn MemoryPressureHandler>) {
        self.handlers.push(handler);
    }

    /// Reports the current total usage, in bytes, and notifies handlers if the
    /// pressure band changed.
    ///
    /// Assigns [`MemoryStats::current_usage`] directly from `current_usage` (it
    /// does not add to it, unlike `record_allocation`) and recomputes pressure:
    /// [`MemoryPressure::Critical`] at or above `critical_threshold`,
    /// [`MemoryPressure::High`] at or above `warning_threshold`, otherwise
    /// [`MemoryPressure::None`]. Handlers therefore only ever observe these three
    /// levels, never the finer bands of [`MemoryPressure::from_usage`]. Update is
    /// not idempotent in the sense that repeated calls with the same value do not
    /// re-notify; only actual transitions fire.
    pub fn update(&mut self, current_usage: usize) {
        self.stats.current_usage = current_usage;
        // `update` reports live usage directly, so it must also raise the high-water mark the same
        // way `record_allocation` does; otherwise peak_usage would stay at zero while current_usage
        // climbed, and the two paths would disagree about what "peak" means.
        if current_usage > self.stats.peak_usage {
            self.stats.peak_usage = current_usage;
        }
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

    /// Forwards `size` bytes to [`MemoryStats::record_allocation`], in bytes.
    ///
    /// Updates the counters only; it recalculates neither pressure nor handler
    /// notifications, unlike [`Self::update`].
    pub fn record_allocation(&mut self, size: usize) {
        self.stats.record_allocation(size);
    }

    /// Forwards `size` bytes to [`MemoryStats::record_deallocation`], in bytes.
    ///
    /// Counters only; pressure and handlers are untouched, so a monitor can report
    /// low usage while its pressure level is stale.
    pub fn record_deallocation(&mut self, size: usize) {
        self.stats.record_deallocation(size);
    }
}

impl Default for MemoryMonitor {
    /// Defaults to an 100 MiB warning threshold and a 200 MiB critical threshold.
    fn default() -> Self {
        Self::new(1024 * 1024 * 100, 1024 * 1024 * 200)
    }
}

impl core::fmt::Debug for MemoryMonitor {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("MemoryMonitor")
            .field("stats", &self.stats)
            .field("pressure", &self.pressure)
            .field("warning_threshold", &self.warning_threshold)
            .field("critical_threshold", &self.critical_threshold)
            .finish()
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

    /// A zero-capacity arena is legal: it never passes a zero-size layout to `alloc` (which would
    /// violate the allocation contract), cannot hand out any memory, and can be dropped safely
    /// because it never owns a block to free.
    #[test]
    fn a_zero_capacity_arena_is_safe_to_construct_use_and_drop() {
        let mut arena = ArenaAllocator::new(0);
        assert_eq!(arena.capacity(), 0);
        assert_eq!(arena.used(), 0);
        assert_eq!(arena.available(), 0);
        assert!(arena.allocate::<u64>().is_none(), "an empty arena can hand out nothing");
        arena.reset();
        assert_eq!(arena.used(), 0);
        // Dropping the arena must not call dealloc with a zero-size layout.
        drop(arena);
    }

    /// Ordinary counters saturate rather than wrap. Reporting a byte count twice, the second time
    /// larger than what remains, must clamp the running totals instead of overflowing.
    #[test]
    fn memory_stats_counters_saturate_instead_of_wrapping() {
        let mut stats = MemoryStats::default();
        stats.record_allocation(usize::MAX - 5);
        stats.record_allocation(100);
        assert_eq!(stats.total_allocated, usize::MAX, "total_allocated saturates");
        assert_eq!(stats.current_usage, usize::MAX, "current_usage saturates");
        assert_eq!(stats.peak_usage, usize::MAX);

        stats.record_deallocation(usize::MAX - 1);
        stats.record_deallocation(100);
        assert_eq!(stats.total_freed, usize::MAX, "total_freed saturates");
        assert_eq!(stats.current_usage, 0);

        for _ in 0..3 {
            stats.record_pool_hit();
        }
        stats.pool_hits = usize::MAX;
        stats.record_pool_hit();
        assert_eq!(stats.pool_hits, usize::MAX, "pool_hits saturates");
        stats.pool_misses = usize::MAX;
        stats.record_pool_miss();
        assert_eq!(stats.pool_misses, usize::MAX, "pool_misses saturates");
        // The rate must not divide by a wrapped-to-zero total.
        assert!(stats.pool_hit_rate() >= 0.0 && stats.pool_hit_rate() <= 1.0);
    }

    /// `update` reports live usage directly, so it must raise the high-water mark just like
    /// `record_allocation`. Before the fix, `peak_usage` stayed at zero while `current_usage` climbed.
    #[test]
    fn memory_monitor_update_maintains_peak_usage() {
        let mut monitor = MemoryMonitor::new(100, 200);
        monitor.update(100);
        assert_eq!(monitor.stats().current_usage, 100);
        assert_eq!(monitor.stats().peak_usage, 100, "update must raise peak_usage");

        // Dropping back down keeps the high-water mark.
        monitor.update(20);
        assert_eq!(monitor.stats().current_usage, 20);
        assert_eq!(monitor.stats().peak_usage, 100, "peak is a high-water mark");

        monitor.update(150);
        assert_eq!(monitor.stats().peak_usage, 150);
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

    /// Every allocation must land on an address that is a multiple of the
    /// requested alignment, for alignments far larger than the allocator's own
    /// base alignment. This is the regression test for S-01: masking the offset
    /// while ignoring the base address's own misalignment produced pointers whose
    /// remainder modulo the requested alignment was non-zero.
    #[test]
    fn high_alignment_requests_land_on_aligned_absolute_addresses() {
        let mut stack = StackAllocator::new(1 << 16);
        for align in [1usize, 8, 64, 256, 4096] {
            // Allocate a few odd-sized blocks first so the base offset moves off
            // zero and previously-misaligned cases would surface.
            for size in [1usize, 3, 7, 13] {
                let ptr = stack.allocate(size, align).expect("room for the request");
                let addr = ptr as usize;
                assert_eq!(addr % align, 0, "stack align {align} size {size} gave {addr:#x}");
            }
        }

        // A type whose natural alignment exceeds the backing block's alignment.
        #[repr(align(4096))]
        #[derive(Default)]
        struct OverAligned([u8; 1]);

        let mut arena = ArenaAllocator::new(1 << 16);
        // Move the bump pointer off zero before requesting the over-aligned type.
        let _ = arena.allocate::<u8>();
        let _ = arena.allocate::<u64>();
        let ptr = arena.allocate::<OverAligned>().expect("room for the request");
        assert_eq!(ptr.as_ptr() as usize % 4096, 0, "arena over-aligned type was misaligned");
        // Write through the pointer so the payload is observably used, not just the address.
        unsafe { (*ptr.as_ptr()).0[0] = 7 };
        assert_eq!(unsafe { (*ptr.as_ptr()).0[0] }, 7);
    }

    /// A request that cannot fit must be refused without consuming space, and the
    /// next smaller request must still succeed at the requested alignment.
    #[test]
    fn an_impossible_request_leaves_the_allocator_unchanged() {
        let mut stack = StackAllocator::new(64);
        let before = stack.used();
        assert!(stack.allocate(128, 8).is_none());
        assert_eq!(stack.used(), before, "a rejected request must not move the cursor");
        let ptr = stack.allocate(16, 64).expect("a fitting request still succeeds");
        assert_eq!(ptr as usize % 64, 0);
    }

    #[test]
    fn test_memory_pressure() {
        assert_eq!(MemoryPressure::from_usage(25, 100), MemoryPressure::None);
        assert_eq!(MemoryPressure::from_usage(60, 100), MemoryPressure::Low);
        assert_eq!(MemoryPressure::from_usage(80, 100), MemoryPressure::Medium);
        assert_eq!(MemoryPressure::from_usage(90, 100), MemoryPressure::High);
        assert_eq!(MemoryPressure::from_usage(98, 100), MemoryPressure::Critical);
    }
}
