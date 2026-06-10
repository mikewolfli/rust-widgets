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

#[cfg(feature = "mini")]
pub use core::cell::RefCell as Mutex;
#[cfg(not(feature = "mini"))]
pub use std::sync::Mutex;

#[cfg(feature = "mini")]
pub use core::cell::RefMut as MutexGuard;
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
    bump: core::cell::RefCell<bumpalo::Bump>,
}

#[cfg(feature = "mini")]
impl MiniArena {
    /// Create a new arena with default capacity (~16KB).
    pub fn new() -> Self {
        Self { bump: core::cell::RefCell::new(bumpalo::Bump::new()) }
    }

    /// Allocate a value in the arena. Returns a mutable reference.
    /// The value lives until the arena is reset.
    pub fn alloc<T>(&self, val: T) -> &mut T {
        self.bump.borrow_mut().alloc(val)
    }

    /// Allocate a slice by copying from an iterator.
    pub fn alloc_slice<T: Copy>(&self, slice: &[T]) -> &mut [T] {
        self.bump.borrow_mut().alloc_slice_copy(slice)
    }

    /// Reset the arena, freeing all allocations.
    pub fn reset(&self) {
        self.bump.borrow_mut().reset();
    }

    /// Remaining capacity hint.
    pub fn allocated_bytes(&self) -> usize {
        self.bump.borrow().allocated_bytes()
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
        use core::cell::UnsafeCell;
        use core::mem::MaybeUninit;
        use core::sync::atomic::{AtomicBool, Ordering};
        static INIT: AtomicBool = AtomicBool::new(false);
        static ARENA: UnsafeCell<MaybeUninit<MiniArena>> = UnsafeCell::new(MaybeUninit::uninit());
        if !INIT.load(Ordering::Acquire) {
            let arena = unsafe { &mut *ARENA.get() };
            arena.write(MiniArena::new());
            INIT.store(true, Ordering::Release);
        }
        unsafe { (*ARENA.get()).assume_init_ref() }
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
