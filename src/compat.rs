//! BLUE13 Phase 3: Alloc bridge — unified imports for std and no_std.
//!
//! All crate files should import common types from here instead of `std`
//! when those types are not available in `core`/`alloc`. This enables
//! `#![cfg_attr(feature = "mini", no_std)]` without `#[cfg]` in 200+ files.

// ── core re-exports (exported unconditionally, always available) ──
pub use core::any::Any;
pub use core::cell::{Cell, RefCell};
pub use core::fmt;
pub use core::hash::{Hash, Hasher};
pub use core::sync::atomic;
pub use core::time::Duration;

// ── alloc re-exports (available in both std and no_std) ──
pub use alloc::boxed::Box;
pub use alloc::collections::BTreeMap;
pub use alloc::collections::VecDeque;
pub use alloc::format;
pub use alloc::rc::Rc;
pub use alloc::string::{String, ToString};
pub use alloc::sync::Arc;
pub use alloc::vec;
pub use alloc::vec::Vec;

// ── heapless/MiniVec for compile-time fixed-size collections (BLUE13 R5.3-R5.4) ──
// Under mini, Vec and String are replaced with fixed-capacity alternatives.
// Under desktop/full, they remain dynamic (alloc::vec::Vec, alloc::string::String).

/// Fixed-capacity vector for mini builds. Falls back to `Vec<T>` on desktop.
#[cfg(feature = "mini")]
pub type MiniVec<T> = heapless::Vec<T, 64>;
#[cfg(not(feature = "mini"))]
pub type MiniVec<T> = alloc::vec::Vec<T>;

/// Fixed-capacity string for mini builds. Falls back to `String` on desktop.
#[cfg(feature = "mini")]
pub type MiniString = heapless::String<256>;
#[cfg(not(feature = "mini"))]
pub type MiniString = alloc::string::String;

/// Convert a `&str` to `MiniString`. Under mini, copies into fixed buffer.
/// Under desktop, creates an owned `String`.
pub fn into_mini(s: &str) -> MiniString {
    #[cfg(feature = "mini")]
    {
        let mut ms = MiniString::new();
        let _ = ms.push_str(s);
        ms
    }
    #[cfg(not(feature = "mini"))]
    {
        MiniString::from(s)
    }
}

/// Convert a `String` to `MiniString` (consumes the String).
/// Under mini, copies into fixed buffer. Under desktop, zero-cost.
pub fn mini_string_from(s: String) -> MiniString {
    #[cfg(feature = "mini")]
    {
        into_mini(&s)
    }
    #[cfg(not(feature = "mini"))]
    {
        s
    }
}

// ── std→alloc bridge (conditional: BTreeMap stands in for HashMap under mini) ──
#[cfg(feature = "mini")]
pub use alloc::collections::BTreeMap as HashMap;
#[cfg(not(feature = "mini"))]
pub use std::collections::HashMap;

/// A thread-safe mutual exclusion primitive.
/// Under mini (no_std), wraps `RefCell` with `lock()` → `RefMut`.
/// Under desktop, re-exports `std::sync::Mutex`.
#[cfg(feature = "mini")]
pub struct Mutex<T> {
    inner: core::cell::RefCell<T>,
}

/// A poison error wrapper for mini builds (no actual poisoning).
#[cfg(feature = "mini")]
#[derive(Debug)]
pub struct PoisonError<T> {
    inner: T,
}

#[cfg(feature = "mini")]
impl<T> PoisonError<T> {
    pub fn into_inner(self) -> T {
        self.inner
    }
}

#[cfg(feature = "mini")]
impl<T> Mutex<T> {
    pub const fn new(value: T) -> Self {
        Self { inner: core::cell::RefCell::new(value) }
    }

    pub fn lock(&self) -> Result<MutexGuard<'_, T>, PoisonError<MutexGuard<'_, T>>> {
        Ok(MutexGuard { inner: self.inner.borrow_mut() })
    }

    pub fn into_inner(self) -> T {
        self.inner.into_inner()
    }
}

#[cfg(feature = "mini")]
impl<T: core::fmt::Debug> core::fmt::Debug for Mutex<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Mutex").field("inner", &self.inner).finish()
    }
}

#[cfg(feature = "mini")]
impl<T: Default> Default for Mutex<T> {
    fn default() -> Self {
        Self { inner: core::cell::RefCell::new(T::default()) }
    }
}

// SAFETY: Under mini (single-threaded), no concurrent access is possible.
#[cfg(feature = "mini")]
unsafe impl<T> Send for Mutex<T> {}
#[cfg(feature = "mini")]
unsafe impl<T> Sync for Mutex<T> {}

#[cfg(feature = "mini")]
pub struct MutexGuard<'a, T> {
    inner: core::cell::RefMut<'a, T>,
}

#[cfg(feature = "mini")]
unsafe impl<'a, T: Send> Send for MutexGuard<'a, T> {}
#[cfg(feature = "mini")]
unsafe impl<'a, T: Sync> Sync for MutexGuard<'a, T> {}

#[cfg(feature = "mini")]
impl<'a, T> core::fmt::Debug for MutexGuard<'a, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("MutexGuard").finish_non_exhaustive()
    }
}

#[cfg(feature = "mini")]
impl<'a, T> core::ops::Deref for MutexGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.inner.deref()
    }
}

#[cfg(feature = "mini")]
impl<'a, T> core::ops::DerefMut for MutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.inner.deref_mut()
    }
}

#[cfg(not(feature = "mini"))]
pub use std::sync::Mutex;

#[cfg(not(feature = "mini"))]
pub use std::sync::MutexGuard;

// ── Bump arena allocator (BLUE13 R5.5) ──
// Under mini, a pre-allocated bump arena replaces the global heap allocator.
// This eliminates the need for a full `alloc` runtime while keeping Box-like
// allocation via `arena_box()`. The arena is reset on each frame cycle.

/// Bump arena allocator. On mini, backed by a single-threaded `bumpalo::Bump`.
/// On desktop, this is a no-op wrapper (allocation goes through the global allocator).
#[cfg(feature = "mini")]
pub struct MiniArena {
    // Use UnsafeCell instead of RefCell because bumpalo::Bump::alloc() returns
    // references tied to &self, which is incompatible with temporary RefMut guards.
    // Under mini (single-threaded), this is safe.
    bump: core::cell::UnsafeCell<bumpalo::Bump>,
}

#[cfg(feature = "mini")]
impl MiniArena {
    /// Create a new arena with default capacity (~16KB).
    pub fn new() -> Self {
        Self { bump: core::cell::UnsafeCell::new(bumpalo::Bump::new()) }
    }

    /// Allocate a value in the arena. Returns a mutable reference.
    /// The value lives until the arena is reset.
    pub fn alloc<T>(&self, val: T) -> &mut T {
        // SAFETY: Under mini (single-threaded), no concurrent access.
        // The Bump is only borrowed mutably here, and the returned reference
        // is valid until reset() is called.
        unsafe { (*self.bump.get()).alloc(val) }
    }

    /// Allocate a slice by copying from an iterator.
    pub fn alloc_slice<T: Copy>(&self, slice: &[T]) -> &mut [T] {
        // SAFETY: Same reasoning as alloc().
        unsafe { (*self.bump.get()).alloc_slice_copy(slice) }
    }

    /// Reset the arena, freeing all allocations.
    pub fn reset(&self) {
        // SAFETY: Under mini (single-threaded), no concurrent access.
        unsafe {
            (*self.bump.get()).reset();
        }
    }

    /// Remaining capacity hint.
    pub fn allocated_bytes(&self) -> usize {
        // SAFETY: allocated_bytes() is a read-only operation safe under single-threaded.
        unsafe { (*self.bump.get()).allocated_bytes() }
    }
}

#[cfg(not(feature = "mini"))]
#[derive(Default)]
pub struct MiniArena;

#[cfg(not(feature = "mini"))]
impl MiniArena {
    pub const fn new() -> Self {
        Self
    }
    pub fn alloc<T>(&self, val: T) -> alloc::boxed::Box<T> {
        alloc::boxed::Box::new(val)
    }
    pub fn alloc_slice<T: Copy>(&self, slice: &[T]) -> alloc::vec::Vec<T> {
        slice.to_vec()
    }
    pub fn reset(&self) {}
    pub fn allocated_bytes(&self) -> usize {
        0
    }
}

/// Get the global frame arena. Under mini, allocations live until `reset_frame_arena()`.
/// Under desktop, this is a no-op (uses `Box::new` directly).
pub fn frame_arena() -> &'static MiniArena {
    #[cfg(feature = "mini")]
    {
        // Use compat OnceLock which is unconditionally Sync under mini.
        static ARENA: OnceLock<MiniArena> = OnceLock::new();
        ARENA.get_or_init(MiniArena::new)
    }
    #[cfg(not(feature = "mini"))]
    {
        static ARENA: MiniArena = MiniArena::new();
        &ARENA
    }
}

/// Reset the global frame arena. Under mini, frees all arena allocations.
/// Under desktop, this is a no-op.
pub fn reset_frame_arena() {
    frame_arena().reset();
}

// ── OnceLock compat (thread-safe static init for both std and no_std) ──

/// Thread-safe once-cell for static initialization.
/// Under mini (no_std), backed by a spin-based atomic flag + UnsafeCell.
/// Under desktop, re-exports `std::sync::OnceLock`.
#[cfg(feature = "mini")]
pub struct OnceLock<T> {
    initialized: core::sync::atomic::AtomicBool,
    data: core::cell::UnsafeCell<core::mem::MaybeUninit<T>>,
}

#[cfg(feature = "mini")]
impl<T> OnceLock<T> {
    pub const fn new() -> Self {
        Self {
            initialized: core::sync::atomic::AtomicBool::new(false),
            data: core::cell::UnsafeCell::new(core::mem::MaybeUninit::uninit()),
        }
    }

    pub fn get_or_init<F: FnOnce() -> T>(&self, f: F) -> &T {
        if !self.initialized.load(core::sync::atomic::Ordering::Acquire) {
            let val = f();
            unsafe {
                (*self.data.get()).write(val);
            }
            self.initialized.store(true, core::sync::atomic::Ordering::Release);
        }
        unsafe { (*self.data.get()).assume_init_ref() }
    }

    pub fn get(&self) -> Option<&T> {
        if self.initialized.load(core::sync::atomic::Ordering::Acquire) {
            Some(unsafe { (*self.data.get()).assume_init_ref() })
        } else {
            None
        }
    }
}

#[cfg(feature = "mini")]
// SAFETY: Under mini (no_std, single-threaded), no concurrent access is possible.
unsafe impl<T> Sync for OnceLock<T> {}
#[cfg(feature = "mini")]
unsafe impl<T> Send for OnceLock<T> {}

#[cfg(not(feature = "mini"))]
pub use std::sync::OnceLock;

// ── Instant compat (no_std stub for mini builds) ──

/// A measurement of a monotonically non-decreasing clock.
/// Under mini, always returns `Duration::ZERO` for `elapsed()`.
/// Under desktop, re-exports `std::time::Instant`.
#[cfg(feature = "mini")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Instant;

#[cfg(feature = "mini")]
impl Instant {
    pub fn now() -> Self {
        Self
    }
    pub fn elapsed(&self) -> Duration {
        Duration::ZERO
    }
    pub fn duration_since(&self, _other: &Instant) -> Duration {
        Duration::ZERO
    }
    pub fn checked_duration_since(&self, _other: &Instant) -> Option<Duration> {
        Some(Duration::ZERO)
    }
    pub fn saturating_duration_since(&self, _other: &Instant) -> Duration {
        Duration::ZERO
    }
    pub fn checked_add(&self, _duration: Duration) -> Option<Instant> {
        Some(*self)
    }
    pub fn checked_sub(&self, _duration: Duration) -> Option<Instant> {
        Some(*self)
    }
    pub fn as_nanos(&self) -> u64 {
        0
    }
}

#[cfg(not(feature = "mini"))]
pub use std::time::Instant;

// ── mpsc compat (single-threaded channel for mini builds) ──

/// Single-threaded channel for mini (no_std) builds.
/// Wraps a `VecDeque` behind `RefCell` + `Arc`.
#[cfg(feature = "mini")]
pub mod mpsc {
    use alloc::collections::VecDeque;
    use alloc::sync::Arc;
    use core::cell::RefCell;

    pub struct Sender<T> {
        inner: Arc<RefCell<VecDeque<T>>>,
    }

    impl<T> Clone for Sender<T> {
        fn clone(&self) -> Self {
            Self { inner: self.inner.clone() }
        }
    }

    impl<T> Sender<T> {
        pub fn send(&self, value: T) -> Result<(), ()> {
            self.inner.borrow_mut().push_back(value);
            Ok(())
        }
    }

    pub struct Receiver<T> {
        inner: Arc<RefCell<VecDeque<T>>>,
    }

    impl<T> Receiver<T> {
        pub fn try_recv(&self) -> Result<T, ()> {
            self.inner.borrow_mut().pop_front().ok_or(())
        }
        pub fn recv(&self) -> Result<T, ()> {
            loop {
                if let Some(val) = self.inner.borrow_mut().pop_front() {
                    return Ok(val);
                }
                // No threads under mini, so no events to wait on — return error
                return Err(());
            }
        }
    }

    pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
        let inner = Arc::new(RefCell::new(VecDeque::new()));
        (Sender { inner: inner.clone() }, Receiver { inner })
    }
}

#[cfg(not(feature = "mini"))]
pub use std::sync::mpsc;

// ── Condvar compat (no_std stub for mini builds) ──

/// A condition variable for thread synchronization.
/// Under mini, all operations are no-ops (single-threaded).
/// Under desktop, re-exports `std::sync::Condvar`.
#[cfg(feature = "mini")]
pub struct Condvar;

#[cfg(feature = "mini")]
impl Condvar {
    pub fn new() -> Self {
        Self
    }
    pub fn notify_all(&self) {}
    pub fn notify_one(&self) {}
}

#[cfg(not(feature = "mini"))]
pub use std::sync::Condvar;
