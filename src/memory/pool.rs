// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use crate::compat::{lock, vec, Any, Box, Mutex, String, Vec};
use alloc::sync::Arc;
/// Types that can be recycled through an [`ObjectPool`].
///
/// The bound is `Default + Clone`, where `Default` supplies the state for a
/// freshly minted object and `Clone` lets a pool prototype be copied. `Clone` is
/// required even though pooling reuses values in place rather than cloning them.
/// Reuse is only observably safe because [`Poolable::reset`] is expected to wipe
/// any per-use state.
pub trait Poolable: Default + Clone {
    /// Restores `self` to the same logical state as `Self::default()`.
    ///
    /// Called by the pool once, when an object is handed back through
    /// [`ObjectPool::release`]; an object taken by [`ObjectPool::acquire`] is
    /// therefore already clean and is handed out without a second reset. Because
    /// the pool trusts this call for every later reuse, implementations are
    /// expected to be cheap and must leave no per-use state behind. Capacity-like
    /// resources (allocated buffers, reserved slots) are usually *kept* rather than
    /// freed; only the logical contents must be discarded.
    fn reset(&mut self);
}

/// A pooled value plus the debug-only witness that it was reset before storage.
///
/// [`ObjectPool::acquire`] hands a pooled value out **without** resetting it,
/// because [`ObjectPool::release`] already reset it on the way in (G-75). This
/// wrapper is what makes that invariant checkable: [`Self::take`] asserts the
/// witness in debug builds, while in release builds the witness field does not
/// exist, so the pool pays no bookkeeping for a guarantee it never re-checks.
struct PoolSlot<T> {
    value: T,
    #[cfg(debug_assertions)]
    clean: bool,
}

impl<T> PoolSlot<T> {
    /// Wraps a value that is already in its reset state: a fresh `T::default()` or a
    /// value just returned by `Poolable::reset`.
    fn clean(value: T) -> Self {
        Self {
            value,
            #[cfg(debug_assertions)]
            clean: true,
        }
    }

    /// Consumes the slot, proving in debug builds that the value was reset before it
    /// was stored in the pool.
    fn take(self) -> T {
        #[cfg(debug_assertions)]
        debug_assert!(self.clean, "ObjectPool handed out an object that was not reset");
        self.value
    }
}
/// Tuning parameters for [`ObjectPool`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PoolConfig {
    /// Number of reusable objects created eagerly by [`ObjectPool::new`]. Also the
    /// initial heap reservation for the backing `Vec`. Zero is allowed.
    pub initial_size: usize,
    /// Upper bound on how many objects [`ObjectPool::release`] keeps in the pool.
    /// Objects handed back once the pool is this full are dropped, so the count
    /// reported by [`ObjectPool::available`] never exceeds it.
    pub max_size: usize,
    /// Retained for compatibility and currently *unused*: `ObjectPool` grows
    /// lazily by allocating a `Default` value on demand, so no capacity is
    /// pre-computed from this factor.
    pub growth_factor: f32,
}
impl Default for PoolConfig {
    fn default() -> Self {
        Self { initial_size: 16, max_size: 1024, growth_factor: 1.5 }
    }
}
/// A recycling pool of mutable objects of a single type.
///
/// Unlike a general-purpose allocator ([`crate::memory::allocators`]), this is a
/// *fixed-shape* pool: every slot holds an already-constructed `T`, and reuse
/// means calling [`Poolable::reset`] rather than re-running a constructor.
///
/// The pool never blocks and never fails. When it runs dry, [`Self::acquire`]
/// silently manufactures a fresh `T::default()` instead of growing a pool of
/// entries, so over-drawing degrades to plain allocation rather than an error.
///
/// Not thread-safe on its own; wrap it in [`SharedPool`] or an external lock to
/// share it between threads.
pub struct ObjectPool<T: Poolable> {
    pool: Vec<PoolSlot<T>>,
    config: PoolConfig,
    allocated: usize,
}
impl<T: Poolable> ObjectPool<T> {
    /// Creates a pool pre-filled with `config.initial_size` objects, each produced
    /// by `T::default()`.
    ///
    /// This is a constructor, not an allocation-free one: `initial_size` objects
    /// exist (and are initialised) immediately, so large `T` values make this
    /// expensive.
    pub fn new(config: PoolConfig) -> Self {
        // Clamp the eager fill to `max_size`: the documented bound on `available()` is `max_size`,
        // and pre-creating more than that would report a count the pool is not allowed to keep.
        let initial = config.initial_size.min(config.max_size);
        let mut pool = Vec::with_capacity(initial);
        for _ in 0..initial {
            pool.push(PoolSlot::clean(T::default()));
        }
        Self { pool, config, allocated: 0 }
    }
    /// Takes an object out of the pool.
    ///
    /// If the pool is empty a brand new `T::default()` is returned instead of
    /// blocking — the call never fails. A reused object was reset by
    /// [`Self::release`] before it entered the pool, so acquire does **not** reset
    /// it again (G-75); in debug builds an assertion proves the slot carried the
    /// reset witness. Whether memory is *zeroed* still depends entirely on each
    /// [`Poolable::reset`] implementation.
    pub fn acquire(&mut self) -> T {
        self.allocated += 1;
        match self.pool.pop() {
            // `release` and the eager fill are the only writers of `pool`, and both
            // store values that are already in their reset state.
            Some(slot) => slot.take(),
            None => T::default(),
        }
    }
    /// Returns an object to the pool for reuse, resetting it first.
    ///
    /// The object is moved into its reset state, then kept only while the pool
    /// holds fewer than `max_size` objects; otherwise it is dropped and the memory
    /// is freed. This is the only place a pooled object is reset, which is what
    /// makes the acquire side allocation- and call-free (G-75). The `allocated`
    /// counter is decremented (saturating at zero) even when the object is
    /// discarded, so a release without a matching acquire does not make the counter
    /// wrap.
    pub fn release(&mut self, mut obj: T) {
        obj.reset();
        if self.pool.len() < self.config.max_size {
            self.pool.push(PoolSlot::clean(obj));
        }
        self.allocated = self.allocated.saturating_sub(1);
    }
    /// Number of objects currently sitting in the pool ready to be handed out.
    /// Bounded by [`PoolConfig::max_size`].
    pub fn available(&self) -> usize {
        self.pool.len()
    }
    /// Net number of acquisitions that have not yet been released.
    ///
    /// This is a running counter, not a measurement of live objects: an acquire
    /// that had to construct a fresh `T` still increments it, and releasing an
    /// object that came from outside the pool still decrements it. It can
    /// therefore disagree with the number of objects the caller can account for.
    pub fn allocated(&self) -> usize {
        self.allocated
    }
    /// Current heap capacity of the pool's backing storage, in objects.
    ///
    /// This is a `Vec` capacity, so it is at least [`Self::available`] and can be
    /// larger after objects were dropped on release.
    pub fn capacity(&self) -> usize {
        self.pool.capacity()
    }
    /// Drops every pooled object and resets the `allocated` counter to zero.
    ///
    /// Objects currently checked out by callers are unaffected; the pool simply
    /// forgets about them, so later [`Self::release`] calls will decrement the
    /// counter from its reset value.
    pub fn clear(&mut self) {
        self.pool.clear();
        self.allocated = 0;
    }
    /// Shrinks the pool's backing storage to fit its current contents.
    ///
    /// Frees heap memory but does not change how many objects are pooled; the pool
    /// will simply reallocate on the next [`Self::release`].
    pub fn shrink_to_fit(&mut self) {
        self.pool.shrink_to_fit();
    }
}
impl<T: Poolable> Default for ObjectPool<T> {
    fn default() -> Self {
        Self::new(PoolConfig::default())
    }
}
/// A reference-counted, thread-safe handle to an [`ObjectPool`].
///
/// Cloning a `SharedPool` is cheap and yields another handle onto the *same* pool,
/// so multiple threads or widgets can share one cache. All operations take an
/// internal mutex; on a poisoned lock the inner value is recovered instead of
/// panicking. Requires `T: Send` because pooled objects travel between threads.
pub struct SharedPool<T: Poolable + Send> {
    pool: Arc<Mutex<ObjectPool<T>>>,
}
impl<T: Poolable + Send> SharedPool<T> {
    /// Creates a new shared pool around a freshly built [`ObjectPool`].
    pub fn new(config: PoolConfig) -> Self {
        Self { pool: Arc::new(Mutex::new(ObjectPool::new(config))) }
    }
    /// Takes an object from the shared pool, blocking only for the duration of the
    /// internal mutex.
    ///
    /// Same semantics as [`ObjectPool::acquire`]: the pool never runs out, a fresh
    /// `T::default()` is substituted when empty, and a reused value was reset by
    /// [`ObjectPool::release`] before it entered the pool.
    pub fn acquire(&self) -> T {
        lock(&self.pool).acquire()
    }
    /// Returns an object to the shared pool; see [`ObjectPool::release`] for the
    /// reset and `max_size` behaviour.
    pub fn release(&self, obj: T) {
        lock(&self.pool).release(obj);
    }
    /// Captures a consistent snapshot of the pool's counters under a single lock.
    ///
    /// Cheap enough to poll for diagnostics. Like any snapshot it can be stale as
    /// soon as it is returned if other threads are acquiring or releasing.
    pub fn stats(&self) -> PoolStats {
        let pool = lock(&self.pool);
        PoolStats {
            available: pool.available(),
            allocated: pool.allocated(),
            capacity: pool.capacity(),
        }
    }
}
impl<T: Poolable + Send> Clone for SharedPool<T> {
    fn clone(&self) -> Self {
        Self { pool: Arc::clone(&self.pool) }
    }
}
impl<T: Poolable + Send> Default for SharedPool<T> {
    fn default() -> Self {
        Self::new(PoolConfig::default())
    }
}
/// A snapshot of pool occupancy, as returned by [`SharedPool::stats`].
#[derive(Debug, Clone, Copy)]
pub struct PoolStats {
    /// Objects idle in the pool, ready to be acquired (bounded by
    /// [`PoolConfig::max_size`]).
    pub available: usize,
    /// Net acquisitions not yet released; see [`ObjectPool::allocated`].
    pub allocated: usize,
    /// Heap capacity of the pool's backing storage, in objects.
    pub capacity: usize,
}
/// A heterogenous registry of [`SharedPool`]s keyed only by type.
///
/// Lets an application keep its pools in one place and drop them together. The
/// registry does not own the pools: it stores a `SharedPool` clone per entry, so
/// outstanding handles stay valid after the manager is dropped or cleared.
pub struct PoolManager {
    pools: Vec<Box<dyn Any + Send>>,
}
impl PoolManager {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self { pools: Vec::new() }
    }
    /// Creates a new pool for `T`, records it in the registry, and returns a
    /// handle to it.
    ///
    /// Types are not deduplicated by `T`: calling this twice for the same `T`
    /// registers and returns two independent pools, so callers that want a single
    /// cache per type must track that themselves.
    pub fn register<T: Poolable + Send + 'static>(&mut self, config: PoolConfig) -> SharedPool<T> {
        let pool = SharedPool::new(config);
        self.pools.push(Box::new(pool.clone()));
        pool
    }
    /// Forgets every registered pool.
    ///
    /// The pools themselves are only freed if no other `SharedPool` handle
    /// remains; live handles keep working and keep sharing the same state.
    pub fn clear_all(&mut self) {
        self.pools.clear();
    }
}
crate::impl_default_via_new!(PoolManager);
/// A pool of equally sized byte buffers.
///
/// Unlike [`ObjectPool`] this is a *fixed-size buffer pool*: only buffers whose
/// capacity exactly equals [`Self::buffer_size`] are retained, so the pool hands
/// out uniformly shaped allocations and never has to grow one. Not thread-safe;
/// use one pool per thread or guard it externally.
pub struct BufferPool {
    buffers: Vec<Vec<u8>>,
    buffer_size: usize,
    max_buffers: usize,
}
impl BufferPool {
    /// Creates a pool of `buffer_size`-byte buffers, pre-allocating `initial_count`
    /// of them.
    ///
    /// `buffer_size` is the exact capacity the pool recycles; `max_buffers` caps
    /// how many [`Self::release`] will keep. Both bounds are inclusive and may be
    /// zero. Every pre-allocated buffer is zero-filled.
    pub fn new(buffer_size: usize, initial_count: usize, max_buffers: usize) -> Self {
        // The documented bound on `available()` is `max_buffers`, so the eager fill is clamped to
        // it rather than pre-creating buffers the pool would refuse to keep on release.
        let initial_count = initial_count.min(max_buffers);
        let mut buffers = Vec::with_capacity(initial_count);
        for _ in 0..initial_count {
            buffers.push(vec![0u8; buffer_size]);
        }
        Self { buffers, buffer_size, max_buffers }
    }
    /// Hands out a buffer of exactly [`Self::buffer_size`] bytes.
    ///
    /// Never blocks and never fails: when the pool is empty a fresh zero-filled
    /// buffer is allocated. A recycled buffer is handed out **without** being
    /// wiped — it still holds whatever its previous user wrote, so callers must
    /// overwrite the range they use. Callers that need zeroed bytes should use
    /// [`Self::acquire_sized`], which performs the single clear.
    pub fn acquire(&mut self) -> Vec<u8> {
        // `release` restores the pool's invariant that every stored buffer has length
        // `buffer_size`, so taking one is a pure pop: no resize, no memset (G-74).
        match self.buffers.pop() {
            Some(buffer) => buffer,
            None => vec![0u8; self.buffer_size],
        }
    }
    /// Hands out a zero-filled buffer of exactly `size` bytes.
    ///
    /// Sizes at or below [`Self::buffer_size`] reuse a pooled buffer, so the whole
    /// `size` range is zeroed before returning and the capacity may still exceed
    /// `size`. Larger sizes are allocated fresh, unpooled, and are discarded on
    /// release because their capacity no longer matches the pool — repeatedly
    /// asking for oversized buffers gets no reuse at all.
    pub fn acquire_sized(&mut self, size: usize) -> Vec<u8> {
        if size <= self.buffer_size {
            let mut buf = self.acquire();
            // Trim to the requested size and clear exactly that range once. The old
            // version cleared and resized a buffer that `acquire` had already zero-filled
            // end to end, so the same bytes were written twice (G-74).
            buf.truncate(size);
            buf.fill(0);
            buf
        } else {
            vec![0u8; size]
        }
    }
    /// Returns a buffer for reuse.
    //
    // The buffer is kept only when its capacity is exactly `buffer_size` and the
    // pool is below `max_buffers`; otherwise it is dropped and freed.
    //
    /// Contents are **not** wiped: the pool keeps the bytes and only restores the
    /// length to [`Self::buffer_size`] when a shorter buffer comes back (one obtained
    /// from [`Self::acquire_sized`]). Keeping every stored buffer at full length is
    /// what lets [`Self::acquire`] hand out exactly `buffer_size` bytes without a
    /// memset (G-74).
    pub fn release(&mut self, mut buffer: Vec<u8>) {
        if buffer.capacity() == self.buffer_size && self.buffers.len() < self.max_buffers {
            if buffer.len() != self.buffer_size {
                // Only a shortened `acquire_sized` buffer reaches this branch; a full
                // buffer is stored untouched. `resize` cannot exceed `capacity`, which
                // the check above pins to `buffer_size`.
                buffer.resize(self.buffer_size, 0);
            }
            self.buffers.push(buffer);
        }
    }
    /// Number of pooled buffers ready to be handed out; bounded by `max_buffers`.
    pub fn available(&self) -> usize {
        self.buffers.len()
    }
    /// Exact buffer capacity the pool pools and hands out, in bytes.
    pub fn buffer_size(&self) -> usize {
        self.buffer_size
    }
    /// Drops every pooled buffer, freeing their memory.
    ///
    /// Buffers already checked out by callers are unaffected and can still be
    /// released back into the now-empty pool.
    pub fn clear(&mut self) {
        self.buffers.clear();
    }
}
impl Default for BufferPool {
    fn default() -> Self {
        Self::new(4096, 4, 64)
    }
}
/// A pool of reusable, empty `String`s with pre-reserved capacity.
///
/// A fixed-size style pool: only strings whose capacity is at least
/// the pool's default capacity are retained, which keeps reallocation out of the
/// steady state. Acquired strings are always empty. Not thread-safe.
pub struct StringPool {
    strings: Vec<String>,
    default_capacity: usize,
    max_strings: usize,
}
impl StringPool {
    /// Creates a pool reserving `default_capacity` bytes per string, with
    /// `initial_count` strings pre-created.
    ///
    /// `max_strings` caps how many [`Self::release`] will keep. Note that a
    /// `default_capacity` of zero makes the capacity check in `release` always
    /// succeed, so effectively every returned string is kept up to `max_strings`.
    pub fn new(default_capacity: usize, initial_count: usize, max_strings: usize) -> Self {
        // Clamped to `max_strings` so `available()` never exceeds the documented bound.
        let initial_count = initial_count.min(max_strings);
        let mut strings = Vec::with_capacity(initial_count);
        for _ in 0..initial_count {
            strings.push(String::with_capacity(default_capacity));
        }
        Self { strings, default_capacity, max_strings }
    }
    /// Hands out an empty `String`, reusing a pooled one when possible.
    ///
    /// Never blocks and never fails; allocates a fresh `String` when the pool is
    /// empty. The returned string always has zero length, and its capacity is at
    /// least `default_capacity` — but callers must not assume a particular
    /// capacity, since pooled strings may be larger.
    pub fn acquire(&mut self) -> String {
        self.strings.pop().unwrap_or_else(|| String::with_capacity(self.default_capacity))
    }
    /// Returns a string for reuse.
    ///
    /// The string is cleared (contents discarded, capacity kept) and pooled only
    /// if its capacity is at least `default_capacity` and the pool is below
    /// `max_strings`; otherwise it is dropped. Contents are never preserved, so
    /// callers cannot hand off data through the pool.
    pub fn release(&mut self, mut s: String) {
        s.clear();
        if s.capacity() >= self.default_capacity && self.strings.len() < self.max_strings {
            self.strings.push(s);
        }
    }
    /// Number of strings currently pooled; bounded by `max_strings`.
    pub fn available(&self) -> usize {
        self.strings.len()
    }
    /// Drops every pooled string, freeing their memory. Strings already handed out
    /// remain valid and may still be released back.
    pub fn clear(&mut self) {
        self.strings.clear();
    }
}
impl Default for StringPool {
    fn default() -> Self {
        Self::new(64, 8, 128)
    }
}
/// A pool of reusable, empty `Vec<T>`s with pre-reserved capacity.
///
/// The generic analogue of [`BufferPool`] and [`StringPool`]: it recycles
/// containers rather than their element types, keeping each vector's heap
/// allocation alive across acquires. Only vectors whose capacity is at least
/// `default_capacity` are retained. Not thread-safe.
pub struct VecPool<T> {
    vecs: Vec<Vec<T>>,
    default_capacity: usize,
    max_vecs: usize,
}
impl<T> VecPool<T> {
    /// Creates a pool of vectors with `default_capacity` reserved elements each,
    /// pre-creating `initial_count` of them.
    ///
    /// Nothing is written into the vectors, so `T` needs no `Default` bound; the
    /// capacity is merely reserved. `max_vecs` caps how many [`Self::release`]
    /// will keep.
    pub fn new(default_capacity: usize, initial_count: usize, max_vecs: usize) -> Self {
        // Clamped to `max_vecs` so `available()` never exceeds the documented bound.
        let initial_count = initial_count.min(max_vecs);
        let mut vecs = Vec::with_capacity(initial_count);
        for _ in 0..initial_count {
            vecs.push(Vec::with_capacity(default_capacity));
        }
        Self { vecs, default_capacity, max_vecs }
    }
    /// Hands out an empty `Vec<T>`, reusing a pooled one when possible.
    ///
    /// Never blocks and never fails; allocates a fresh vector when the pool is
    /// empty. The result is guaranteed empty, and its capacity is at least
    /// `default_capacity` when it came from the pool's initial fill. No element is
    /// ever constructed and no memory is cleared, so `T` need not be zeroable.
    pub fn acquire(&mut self) -> Vec<T> {
        self.vecs.pop().unwrap_or_else(|| Vec::with_capacity(self.default_capacity))
    }
    /// Returns a vector for reuse.
    ///
    /// The vector is cleared first, dropping any contained `T` values, then pooled
    /// only if its capacity is at least `default_capacity` and the pool is below
    /// `max_vecs`; otherwise it is dropped and freed. Clearing keeps the heap
    /// allocation but always discards the elements, so the pool must not be used to
    /// pass data between owners.
    pub fn release(&mut self, mut v: Vec<T>) {
        v.clear();
        if v.capacity() >= self.default_capacity && self.vecs.len() < self.max_vecs {
            self.vecs.push(v);
        }
    }
    /// Number of vectors currently pooled; bounded by `max_vecs`.
    pub fn available(&self) -> usize {
        self.vecs.len()
    }
    /// Drops every pooled vector, freeing their element storage. Vectors already
    /// handed out remain valid and may still be released back.
    pub fn clear(&mut self) {
        self.vecs.clear();
    }
}
impl<T> Default for VecPool<T> {
    fn default() -> Self {
        Self::new(16, 4, 64)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default, Clone)]
    struct TestObject {
        value: i32,
        data: Vec<u8>,
    }
    impl Poolable for TestObject {
        fn reset(&mut self) {
            self.value = 0;
            self.data.clear();
        }
    }
    #[test]
    fn test_object_pool() {
        let mut pool: ObjectPool<TestObject> =
            ObjectPool::new(PoolConfig { initial_size: 4, max_size: 8, growth_factor: 1.5 });
        assert_eq!(pool.available(), 4);
        let obj1 = pool.acquire();
        assert_eq!(pool.allocated(), 1);
        pool.release(obj1);
        assert_eq!(pool.available(), 4);
    }
    #[test]
    fn test_buffer_pool() {
        let mut pool = BufferPool::new(1024, 2, 4);
        let buf1 = pool.acquire();
        assert_eq!(buf1.len(), 1024);
        pool.release(buf1);
        assert_eq!(pool.available(), 2);
    }

    /// A reused object must arrive in its reset state even though `acquire` no longer
    /// resets it: the single reset happens on `release`, before the object is pooled
    /// (G-75). The debug-only slot witness asserted in `acquire` proves the invariant
    /// on the same path.
    #[test]
    fn a_pooled_object_is_reset_on_release_before_any_reacquire() {
        let mut pool: ObjectPool<TestObject> =
            ObjectPool::new(PoolConfig { initial_size: 0, max_size: 4, growth_factor: 1.5 });
        let mut obj = pool.acquire();
        obj.value = 7;
        obj.data.extend_from_slice(&[1, 2, 3]);
        pool.release(obj);

        let reused = pool.acquire();
        assert_eq!(reused.value, 0, "release must clear scalar state before pooling");
        assert!(reused.data.is_empty(), "release must clear container state before pooling");
    }

    /// A released buffer is no longer cleared, so `acquire` must return the same length
    /// (`buffer_size`) without wiping the bytes (G-74). Callers are documented to
    /// overwrite what they use; `acquire_sized` is the zero-filled path.
    #[test]
    fn buffer_pool_acquire_does_not_wipe_recycled_contents() {
        let mut pool = BufferPool::new(16, 1, 4);
        let mut buf = pool.acquire();
        buf[..4].copy_from_slice(&[1, 2, 3, 4]);
        pool.release(buf);

        let again = pool.acquire();
        assert_eq!(again.len(), 16, "acquire always yields buffer_size bytes");
        assert_eq!(&again[..4], &[1, 2, 3, 4], "acquire must not clear a recycled buffer");
    }

    /// `acquire_sized` is the only path that promises zeroed bytes, and it clears the
    /// returned range exactly once even when the recycled buffer held stale data (G-74).
    #[test]
    fn buffer_pool_acquire_sized_returns_zeroed_bytes() {
        let mut pool = BufferPool::new(16, 1, 4);
        let mut dirty = pool.acquire();
        dirty.fill(0xFF);
        pool.release(dirty);

        let sized = pool.acquire_sized(8);
        assert_eq!(sized.len(), 8, "acquire_sized returns exactly the requested size");
        assert!(sized.iter().all(|&byte| byte == 0), "the requested range is zero-filled");
    }

    /// A released buffer is restored to the pool's full length, so a second `acquire`
    /// still returns exactly `buffer_size` bytes even after an `acquire_sized` call.
    #[test]
    fn buffer_pool_re_acquire_restores_the_documented_length() {
        let mut pool = BufferPool::new(16, 1, 4);
        let first = pool.acquire();
        assert_eq!(first.len(), 16, "first acquire is a full buffer");
        pool.release(first);
        let second = pool.acquire();
        assert_eq!(second.len(), 16, "re-acquire must restore the length, not return len=0");
        assert!(second.capacity() >= 16);
    }

    /// A buffer obtained through `acquire_sized` is smaller than the pool buffer, and once released
    /// and re-acquired the normal `acquire` must still yield the full `buffer_size` length.
    #[test]
    fn buffer_pool_acquire_sized_then_acquire_restores_the_length() {
        let mut pool = BufferPool::new(16, 1, 4);
        let small = pool.acquire_sized(8);
        assert_eq!(small.len(), 8, "acquire_sized returns exactly the requested size");
        pool.release(small);
        let full = pool.acquire();
        assert_eq!(full.len(), 16, "a normal acquire is always a full buffer");
    }

    /// Every pool constructor clamps its eager fill to its own max, so `available()` never exceeds
    /// the documented bound. `initial > max` used to create more entries than `max` immediately.
    #[test]
    fn pool_constructors_clamp_initial_fill_to_the_maximum() {
        let object_pool = ObjectPool::<TestObject>::new(PoolConfig {
            initial_size: 3,
            max_size: 1,
            growth_factor: 1.5,
        });
        assert_eq!(object_pool.available(), 1, "object pool is bounded by max_size");

        let buffer_pool = BufferPool::new(8, 3, 1);
        assert_eq!(buffer_pool.available(), 1, "buffer pool is bounded by max_buffers");

        let string_pool = StringPool::new(4, 3, 1);
        assert_eq!(string_pool.available(), 1, "string pool is bounded by max_strings");

        let vec_pool: VecPool<u8> = VecPool::new(4, 3, 1);
        assert_eq!(vec_pool.available(), 1, "vec pool is bounded by max_vecs");
    }

    /// A max of zero still yields an empty pool (nothing is clamped to being kept), and a normal
    /// `initial <= max` case is unchanged.
    #[test]
    fn pool_constructors_preserve_zero_max_and_normal_cases() {
        assert_eq!(
            ObjectPool::<TestObject>::new(PoolConfig {
                initial_size: 3,
                max_size: 0,
                growth_factor: 1.5,
            })
            .available(),
            0
        );
        assert_eq!(BufferPool::new(8, 3, 0).available(), 0);
        assert_eq!(StringPool::new(4, 3, 0).available(), 0);
        assert_eq!(VecPool::<u8>::new(4, 3, 0).available(), 0);

        assert_eq!(
            ObjectPool::<TestObject>::new(PoolConfig {
                initial_size: 2,
                max_size: 8,
                growth_factor: 1.5,
            })
            .available(),
            2
        );
        assert_eq!(BufferPool::new(8, 2, 8).available(), 2);
        assert_eq!(StringPool::new(4, 2, 8).available(), 2);
        assert_eq!(VecPool::<u8>::new(4, 2, 8).available(), 2);
    }
}
