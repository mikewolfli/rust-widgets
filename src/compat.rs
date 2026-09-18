// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! BLUE13 Phase 3: Alloc bridge — unified imports for std and no_std.
//!
//! All crate files should import common types from here instead of `std`
//! when those types are not available in `core`/`alloc`. This keeps the
//! codebase ready for `#![cfg_attr(feature = "mini", no_std)]` without
//! `#[cfg]` in 200+ files. Note: that attribute is not yet enabled — the
//! `mini` profile currently compiles on std.
//!
//! # The rule
//!
//! Inside this crate, a name that exists in `core` or `alloc` must be imported
//! as `crate::compat::Name`, never as `std::Name`. The re-exports below resolve
//! to the same types today (this build links std regardless), so the rule buys
//! nothing at present except that it is already true at every one of the ~120
//! call sites when `no_std` is switched on. A direct `use std::...` is the one
//! thing that would have to be undone everywhere, which is why the ban is on the
//! import rather than on the capability.
//!
//! # What this module is *not*
//!
//! Switching `mini` to `no_std` is **not** simply a matter of enabling the
//! attribute: the `#[cfg(alloc_frugal)]` arms below are written for a single
//! threaded, allocation-frugal target, but several are still backed by `std`
//! types (`RwLock`, `Mutex`, `Instant`, `RwLock`-guarded `Condvar` paths, and the
//! `std::sync::mpsc` re-export). Do not read the presence of a `no_std`-shaped
//! alias here as evidence that its `alloc_frugal` arm is `no_std`-clean.
//!
//! # Reachability
//!
//! **State:** Production callers: `src/widget/base.rs:43` (`crate::compat::MiniVec<ObjectId>`), `src/widget/base.rs:47` (`crate::compat::MiniString`). 63 files reference the `alloc`/`std` boundary through this module.

// ── core re-exports (exported unconditionally, always available) ──
pub use core::any::Any;
pub use core::cell::{Cell, RefCell};
pub use core::fmt;
pub use core::hash::{Hash, Hasher};
pub use core::sync::atomic;
pub use core::time::Duration;

// ── RwLock (thread-safe in both profiles) ──
/// Reader/writer lock used across the crate.
#[cfg(alloc_frugal)]
pub use spin::RwLock;
/// Read guard returned by [`RwLock::read`].
#[cfg(alloc_frugal)]
pub use spin::RwLockReadGuard;
/// Write guard returned by [`RwLock::write`].
#[cfg(alloc_frugal)]
pub use spin::RwLockWriteGuard;
#[cfg(not(alloc_frugal))]
pub use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

// ── alloc re-exports (available in both std and no_std) ──
pub use alloc::boxed::Box;
pub use alloc::collections::BTreeMap;
pub use alloc::collections::VecDeque;
pub use alloc::format;
pub use alloc::rc::Rc;
pub use alloc::string::{String, ToString};
pub use alloc::sync::Arc;
// The no-std prelude does not carry `ToString`, so under `alloc_frugal` every
// `to_string()` call site — including on `&str`, which needs this trait rather
// than an inherent method — stops compiling unless it names the trait itself.
// Re-exporting it from `compat` is what lets those call sites stay
// profile-agnostic: they import the trait from the same module that already
// sorts out the profile's `String`/`Vec`/`Mutex`.
//
// Named `MiniToString` rather than re-exported as `ToString` so that a call site
// can import both names into one list — `use crate::compat::{MiniToString,
// String, ToString}` — without `E0252`. That is only a convenience on desktop,
// where `ToString` also resolves through the prelude, but it keeps a line an
// author writes once working in both profiles. (The trait is not renameable the
// other way: the call is `s.to_string()`, so the *trait* has to be in scope; the
// alias only decides how the import is spelled.)
pub use alloc::string::ToString as MiniToString;
pub use alloc::vec;
pub use alloc::vec::Vec;

// ── heapless/MiniVec for compile-time fixed-size collections (BLUE13 R5.3-R5.4) ──
// Under mini, Vec and String are replaced with fixed-capacity alternatives.
// Under desktop/full, they remain dynamic (alloc::vec::Vec, alloc::string::String).

/// Fixed-capacity vector for mini builds. Falls back to `Vec<T>` on desktop.
///
/// Under `alloc_frugal` this is `heapless::Vec<T, 64>`: **pushing beyond 64
/// elements fails** rather than reallocating. Under desktop builds it is an
/// ordinary growable `alloc::vec::Vec<T>`, so code must not rely on the
/// capacity limit being enforced.
#[cfg(alloc_frugal)]
pub type MiniVec<T> = heapless::Vec<T, 64>;
/// Growable vector alias used on desktop builds; see the `alloc_frugal`
/// definition for the capacity-limited variant.
#[cfg(not(alloc_frugal))]
pub type MiniVec<T> = alloc::vec::Vec<T>;

/// Fixed-capacity string for mini builds. Falls back to `String` on desktop.
///
/// Under `alloc_frugal` this is `heapless::String<256>`, so at most 256 bytes
/// of UTF-8 are retained; see [`into_mini`], which silently truncates on
/// overflow. Under desktop builds it is an unbounded `alloc::string::String`.
#[cfg(alloc_frugal)]
pub type MiniString = heapless::String<256>;
/// Growable string alias used on desktop builds; see the `alloc_frugal`
/// definition for the capacity-limited variant.
#[cfg(not(alloc_frugal))]
pub type MiniString = alloc::string::String;

/// Convert a `&str` to `MiniString`. Under mini, copies into fixed buffer.
/// Under desktop, creates an owned `String`.
///
/// Truncation is **silent** under `alloc_frugal`: the `heapless` push fails on
/// the byte that would cross 256, so the result is the longest whole-prefix of
/// `s` that fits, cut at a UTF-8 boundary, with no error reported. A caller that
/// must know the text survived intact has to compare lengths.
pub fn into_mini(s: &str) -> MiniString {
    #[cfg(alloc_frugal)]
    {
        let mut ms = MiniString::new();
        let _ = ms.push_str(s);
        ms
    }
    #[cfg(not(alloc_frugal))]
    {
        MiniString::from(s)
    }
}

/// Convert a `String` to `MiniString` (consumes the String).
/// Under mini, copies into fixed buffer. Under desktop, zero-cost.
///
/// Under `alloc_frugal` the original heap `String` is dropped after being copied
/// into the fixed buffer, so this is not a move — the allocation is released and
/// the same silent 256-byte truncation as [`into_mini`] applies. Under desktop
/// it is a genuine no-op move.
pub fn mini_string_from(s: String) -> MiniString {
    #[cfg(alloc_frugal)]
    {
        into_mini(&s)
    }
    #[cfg(not(alloc_frugal))]
    {
        s
    }
}

// ── std→alloc bridge (conditional: BTreeMap stands in for HashMap under mini) ──
/// Map type used across the crate, so call sites do not name a concrete map.
///
/// Resolves to `std::collections::HashMap` on desktop builds and to
/// `alloc::collections::BTreeMap` under `alloc_frugal`. The two are only
/// interchangeable through this alias: they do not share an implementation, and
/// the difference is observable in iteration order (hashed vs sorted by key) and
/// in the key requirements — `BTreeMap` needs `Ord` where `HashMap` needs `Hash`.
/// Code that relies on either property through this alias will not compile, or
/// will silently change behaviour, on the other profile; treat it as an
/// unordered map.
#[cfg(alloc_frugal)]
pub use alloc::collections::BTreeMap as HashMap;
#[cfg(not(alloc_frugal))]
pub use std::collections::HashMap;

// ── Mutex (thread-safe in both profiles) ──
// Under `alloc_frugal` (mini) the crate is no_std, so the `std::sync` locks are
// unavailable and `spin`'s stand in. Both provide the same contract; the
// difference is that `spin` has no poison state, which is what [`lock`],
// [`read_lock`] and [`write_lock`] below normalise so call sites need no `#[cfg]`.
// A RefCell-backed "Mutex" was rejected: it panics on re-entrant/concurrent
// access — not a real mutual-exclusion primitive.
/// Mutual-exclusion lock used across the crate.
#[cfg(alloc_frugal)]
pub use spin::Mutex;
/// Guard returned by [`Mutex::lock`]. Its lifetime ties the guard to the lock, so
/// it cannot outlive the mutex it came from.
#[cfg(alloc_frugal)]
pub use spin::MutexGuard;
#[cfg(not(alloc_frugal))]
pub use std::sync::Mutex;
#[cfg(not(alloc_frugal))]
pub use std::sync::MutexGuard;

/// Acquire a [`Mutex`], recovering from poisoning if there is any.
///
/// The two profiles disagree about what `lock()` returns: `std::sync::Mutex`
/// returns `LockResult<MutexGuard<T>>` (poisoning is a real state), while
/// `spin::Mutex` returns a bare `MutexGuard<T>` (a spin lock has no poison
/// state to report). Normalising that difference here is what lets call sites
/// be identical in both profiles instead of carrying a `#[cfg]` each.
///
/// Poison recovery is the same policy the crate already applied by hand:
/// `unwrap_or_else(|e| e.into_inner())` takes the guard rather than propagating
/// a panic, because a poisoned lock still holds usable data.
pub fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    #[cfg(not(alloc_frugal))]
    {
        mutex.lock().unwrap_or_else(|e| e.into_inner())
    }
    #[cfg(alloc_frugal)]
    {
        mutex.lock()
    }
}

/// Acquire a [`RwLock`] for reading, recovering from poisoning if there is any.
///
/// The counterpart of [`lock`] for the reader side: same profile split (`std`
/// returns `LockResult`, `spin` returns the guard directly), same policy of
/// taking the guard rather than propagating the poison.
pub fn read_lock<T>(lock: &RwLock<T>) -> RwLockReadGuard<'_, T> {
    #[cfg(not(alloc_frugal))]
    {
        lock.read().unwrap_or_else(|e| e.into_inner())
    }
    #[cfg(alloc_frugal)]
    {
        lock.read()
    }
}

/// Acquire a [`RwLock`] for writing, recovering from poisoning if there is any.
///
/// The write-side counterpart of [`read_lock`].
pub fn write_lock<T>(lock: &RwLock<T>) -> RwLockWriteGuard<'_, T> {
    #[cfg(not(alloc_frugal))]
    {
        lock.write().unwrap_or_else(|e| e.into_inner())
    }
    #[cfg(alloc_frugal)]
    {
        lock.write()
    }
}

// ── Bump arena allocator (BLUE13 R5.5) ──
// Under mini, a pre-allocated bump arena replaces the global heap allocator.
// This eliminates the need for a full `alloc` runtime while keeping Box-like
// allocation via `arena_box()`. The arena is reset on each frame cycle.

/// Bump arena allocator. On mini, backed by a single-threaded `bumpalo::Bump`.
/// On desktop, this is a no-op wrapper (allocation goes through the global allocator).
///
/// # Invariants
///
/// * **Single-threaded.** The safety of the aliasing rules below rests on `mini`
///   never running the arena from two threads; there is no synchronisation
///   inside it.
/// * **Nothing is freed until [`MiniArena::reset`].** A dropped arena value, a
///   dropped allocation, a dropped box — none of them release the backing block.
///   `reset` frees *everything* at once, so it invalidates every reference the
///   arena has handed out.
/// * **The returned references are not tied to a borrow.** See
///   [`MiniArena::alloc`].
///
/// Under `not(alloc_frugal)` this is a zero-sized type; see the desktop
/// definition below.
#[cfg(alloc_frugal)]
pub struct MiniArena {
    // Use UnsafeCell instead of RefCell because bumpalo::Bump::alloc() returns
    // references tied to &self, which is incompatible with temporary RefMut guards.
    // Under mini (single-threaded), this is safe.
    bump: core::cell::UnsafeCell<bumpalo::Bump>,
}

#[cfg(alloc_frugal)]
impl MiniArena {
    /// Create a new arena with default capacity (~16KB).
    ///
    /// The ~16 KiB is `bumpalo::Bump::new()`'s first chunk; the arena grows in
    /// further chunks on demand, so this is a starting size and not a cap.
    pub fn new() -> Self {
        Self { bump: core::cell::UnsafeCell::new(bumpalo::Bump::new()) }
    }

    /// Allocate a value in the arena. Returns a mutable reference.
    /// The value lives until the arena is reset.
    // NOTE: `&self -> &mut T` is the arena contract — the returned reference is
    // exclusive until `reset()` because the arena is single-threaded under mini.
    ///
    /// # Why `&self` yields `&mut T`
    ///
    /// `&self` is taken so a caller can allocate through a `&'static MiniArena`
    /// (see [`frame_arena`]) without a mutable borrow of a global. The exclusivity
    /// of the result is therefore a *convention*, not something the compiler
    /// checks: two live references to the same allocation, or a reference that is
    /// still in use when [`Self::reset`] runs, are undefined behaviour. The value
    /// is moved into the arena and is dropped only when the arena resets, so it
    /// must not own a resource whose release is time-critical.
    #[allow(clippy::mut_from_ref)]
    pub fn alloc<T>(&self, val: T) -> &mut T {
        // SAFETY: Under mini (single-threaded), no concurrent access.
        // The Bump is only borrowed mutably here, and the returned reference
        // is valid until reset() is called.
        unsafe { (*self.bump.get()).alloc(val) }
    }

    /// Allocate a slice by copying from an iterator.
    // NOTE: Same arena contract as `alloc` — see above.
    ///
    /// `T: Copy` rather than `T: Clone` because the source is copied bytewise
    /// into the arena; the returned slice is arena-owned and carries the same
    /// lifetime convention as [`Self::alloc`] — inclusive of the requirement that
    /// it must not outlive a [`Self::reset`].
    #[allow(clippy::mut_from_ref)]
    pub fn alloc_slice<T: Copy>(&self, slice: &[T]) -> &mut [T] {
        // SAFETY: Same reasoning as alloc().
        unsafe { (*self.bump.get()).alloc_slice_copy(slice) }
    }

    /// Reset the arena, freeing all allocations.
    ///
    /// This is the only way memory returns to the allocator, and it is total:
    /// every reference the arena has handed out is dangling afterwards. Callers
    /// must stop using them (and drop anything holding one) before calling this,
    /// which is why the frame cycle that calls it also has to know that no
    /// frame-scoped value outlives the frame.
    ///
    /// The arena stays usable afterwards and keeps its largest chunk, so a reset
    /// between frames does not re-grow from scratch.
    pub fn reset(&self) {
        // SAFETY: Under mini (single-threaded), no concurrent access.
        unsafe {
            (*self.bump.get()).reset();
        }
    }

    /// Remaining capacity hint.
    ///
    /// Returns the number of bytes currently **allocated** from this arena, in
    /// bytes — not the remaining headroom, despite the name. Compare it against
    /// the budget the caller has in mind, or against a previous reading to see
    /// how much a frame accounted for; it falls back to zero after a reset.
    pub fn allocated_bytes(&self) -> usize {
        // SAFETY: allocated_bytes() is a read-only operation safe under single-threaded.
        unsafe { (*self.bump.get()).allocated_bytes() }
    }
}

#[cfg(alloc_frugal)]
crate::impl_default_via_new!(MiniArena);

#[cfg(not(alloc_frugal))]
#[derive(Default)]
/// No-op arena used on desktop builds.
///
/// Allocation is delegated to the global allocator, and [`MiniArena::reset`]
/// does nothing because there is nothing arena-owned to free. It exists so the
/// arena call sites compile unchanged on both profiles.
///
/// It is zero-sized, so this type costs nothing to pass around, and the methods
/// below deliberately differ in signature from the `alloc_frugal` ones — see
/// [`MiniArena::alloc`] — because a global-heap allocation *is* owned by its
/// caller rather than borrowed from a shared arena.
pub struct MiniArena;

#[cfg(not(alloc_frugal))]
impl MiniArena {
    /// Creates the desktop no-op arena; there is exactly one, but constructing
    /// extra values is harmless since it carries no state.
    pub const fn new() -> Self {
        Self
    }
    /// Allocates on the global heap and returns an owning `Box<T>`.
    ///
    /// Unlike the `alloc_frugal` version, the result is a normal owned pointer
    /// that is freed when dropped rather than borrowed from an arena.
    pub fn alloc<T>(&self, val: T) -> alloc::boxed::Box<T> {
        alloc::boxed::Box::new(val)
    }
    /// Copies `slice` into a freshly allocated `Vec<T>` owned by the caller.
    ///
    /// The `Vec` is returned by value and is freed when it is dropped; unlike the
    /// `alloc_frugal` version, the copy is not borrowed from an arena and there is
    /// no `reset` that would invalidate it.
    pub fn alloc_slice<T: Copy>(&self, slice: &[T]) -> alloc::vec::Vec<T> {
        slice.to_vec()
    }
    /// No-op on desktop: nothing is arena-owned, so there is nothing to free.
    ///
    /// In particular it does **not** free the `Box`es and `Vec`s returned by
    /// [`Self::alloc`] / [`Self::alloc_slice`]; those are owned by their callers.
    pub fn reset(&self) {}
    /// Always `0` on desktop, because no bytes are tracked by this no-op arena.
    /// Do not use this as a memory-usage measurement.
    pub fn allocated_bytes(&self) -> usize {
        0
    }
}

/// Get the global frame arena. Under mini, allocations live until `reset_frame_arena()`.
/// Under desktop, this is a no-op (uses `Box::new` directly).
///
/// The arena is process-global, so every caller shares one allocation pool and a
/// [`reset_frame_arena`] from any of them invalidates all of them. Under
/// `alloc_frugal` it is initialised on first use through [`OnceLock`], so the
/// first call allocates and later ones do not.
pub fn frame_arena() -> &'static MiniArena {
    #[cfg(alloc_frugal)]
    {
        // Use compat OnceLock which is unconditionally Sync under mini.
        static ARENA: OnceLock<MiniArena> = OnceLock::new();
        ARENA.get_or_init(MiniArena::new)
    }
    #[cfg(not(alloc_frugal))]
    {
        static ARENA: MiniArena = MiniArena::new();
        &ARENA
    }
}

/// Reset the global frame arena. Under mini, frees all arena allocations.
/// Under desktop, this is a no-op.
///
/// The word "frame" is a convention, not a clock: nothing calls this
/// automatically, so a build that never calls it never reclaims arena memory.
/// Under `alloc_frugal` it must be called at a point where no arena reference is
/// still live (see [`MiniArena::reset`]).
pub fn reset_frame_arena() {
    frame_arena().reset();
}

// ── OnceLock compat (thread-safe static init for both std and no_std) ──

/// Thread-safe once-cell for static initialization.
/// Under mini (no_std), backed by a spin-based atomic flag + UnsafeCell.
/// Under desktop, re-exports `std::sync::OnceLock`.
///
/// # Why not `std::sync::OnceLock`
///
/// `std`'s version is the default whenever the symlink below resolves to it; this
/// type exists for the `alloc_frugal` arm, where the cell has to be constructible
/// in a `static` (`const fn new`) and usable without the std runtime.
///
/// # Thread safety
///
/// `Send` and `Sync` are asserted unconditionally below even though the type
/// holds a `UnsafeCell<MaybeUninit<T>>` and never synchronises a *second*
/// initialiser: the initialisation path is a plain load, a write, and a store,
/// with no compare-exchange, so two threads racing [`OnceLock::get_or_init`] on
/// the same cell would both write. That is sound only because `mini` is
/// single-threaded. [`OnceLock::set`] is the exception — it does use a
/// compare-exchange, so it reports the loser rather than overwriting.
#[cfg(alloc_frugal)]
pub struct OnceLock<T> {
    initialized: core::sync::atomic::AtomicBool,
    data: core::cell::UnsafeCell<core::mem::MaybeUninit<T>>,
}

#[cfg(alloc_frugal)]
impl<T> OnceLock<T> {
    /// Creates an empty cell. `const`, so it can initialise a `static` directly.
    pub const fn new() -> Self {
        Self {
            initialized: core::sync::atomic::AtomicBool::new(false),
            data: core::cell::UnsafeCell::new(core::mem::MaybeUninit::uninit()),
        }
    }

    /// Returns the cell's value, initialising it with `f` if this is the first
    /// call.
    ///
    /// `f` runs at most once *when calls are serialised*; see the type-level note
    /// on concurrency. The returned reference is valid for as long as the cell is,
    /// and the value is never dropped — a `OnceLock` in a `static` leaks its
    /// contents at process exit, which is the intended meaning of "lives
    /// forever".
    pub fn get_or_init<F: FnOnce() -> T>(&self, f: F) -> &T {
        if !self.initialized.load(core::sync::atomic::Ordering::Acquire) {
            let val = f();
            // SAFETY: Under mini (single-threaded), no concurrent access is possible.
            // The Acquire-Release ordering on `initialized` guarantees that the write
            // in `get_or_init` is visible to any subsequent `get` call.
            unsafe {
                (*self.data.get()).write(val);
            }
            self.initialized.store(true, core::sync::atomic::Ordering::Release);
        }
        // SAFETY: Once `initialized` is true, the data has been written and will not
        // be mutated again. The Acquire ordering ensures we see the write above.
        unsafe { (*self.data.get()).assume_init_ref() }
    }

    /// Returns the value if it has been set, without initialising it.
    ///
    /// Returns `None` for a never-initialised cell. There is no way to tell a cell
    /// whose `f` has not run from one whose `f` has not been *provided* — this
    /// method never takes one — so it is only useful after some other code has
    /// populated the cell.
    pub fn get(&self) -> Option<&T> {
        if self.initialized.load(core::sync::atomic::Ordering::Acquire) {
            // SAFETY: Same as get_or_init — once initialized, data is immutable.
            Some(unsafe { (*self.data.get()).assume_init_ref() })
        } else {
            None
        }
    }

    /// Stores a value, mirroring `std::sync::OnceLock::set`.
    ///
    /// Returns `Ok(())` on first initialization, or `Err(value)` with the
    /// rejected value if the cell was already set.
    ///
    /// Unlike [`Self::get_or_init`], the already-set case is detected with a
    /// compare-exchange, so this is safe against a racing writer and returns the
    /// value rather than dropping it.
    pub fn set(&self, value: T) -> Result<(), T> {
        if self
            .initialized
            .compare_exchange(
                false,
                true,
                core::sync::atomic::Ordering::AcqRel,
                core::sync::atomic::Ordering::Acquire,
            )
            .is_ok()
        {
            // SAFETY: We won the compare-exchange race, so this is the only
            // writer and no reader can observe the cell until the Release store
            // above flips `initialized`. Under mini (single-threaded) there is
            // no concurrent access; the ordering keeps the API equivalent to std.
            unsafe {
                (*self.data.get()).write(value);
            }
            Ok(())
        } else {
            Err(value)
        }
    }
}

#[cfg(alloc_frugal)]
impl<T> Default for OnceLock<T> {
    /// An empty cell, same as [`OnceLock::new`].
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(alloc_frugal)]
// SAFETY: Under mini (no_std, single-threaded), no concurrent access is possible.
unsafe impl<T> Sync for OnceLock<T> {}
#[cfg(alloc_frugal)]
unsafe impl<T> Send for OnceLock<T> {}

#[cfg(not(alloc_frugal))]
pub use std::sync::OnceLock;

// ── Instant (real clock in both profiles) ──
// Both arms must be a *real* monotonic clock. A zero-valued stub was rejected:
// it would silently break every timing-based subsystem (timers, FPS counters,
// animation frames) rather than fail visibly.
//
// No arm is a bare `pub use std::time::Instant`. The `alloc_frugal` arm is a
// wrapper whose methods delegate to `std::time::Instant`, so a caller sees one
// type with one behaviour in both profiles:
//
// * `spin`, the profile's lock crate, carries no clock at all — a spin lock has
//   nothing to read time from — so it cannot supply one;
// * `core::time` provides only `Duration`, with no way to read a clock;
// * and the `std` the crate does link under `mini` (see the
//   `#[macro_use] extern crate std` in `lib.rs`) *does* have a real monotonic
//   clock. Reaching it from here, rather than from `compat`'s callers, is what
//   keeps the `std` dependency confined to the one module whose job is to
//   reconcile the two profiles.

/// Monotonic clock reading used across the crate.
#[cfg(not(alloc_frugal))]
pub use std::time::Instant;

/// Monotonic clock reading used across the crate.
///
/// The `alloc_frugal` spelling of `std::time::Instant`. It is the same clock with
/// the same semantics — `now`, `duration_since`, `elapsed` and the `Sub`/`Add`
/// arithmetic — so timing code behaves identically in both profiles. The wrapper
/// exists so that `compat`, not each call site, answers which `std` items this
/// profile is allowed to name.
#[cfg(alloc_frugal)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Instant(std::time::Instant);

#[cfg(alloc_frugal)]
impl Instant {
    /// Reads the monotonic clock.
    pub fn now() -> Self {
        Self(std::time::Instant::now())
    }

    /// Time elapsed since `earlier`, saturating at zero.
    ///
    /// Saturating rather than panicking keeps a caller that fed the readings in
    /// the wrong order to a defined answer (`Duration::ZERO`) instead of
    /// aborting — the same reasoning as the desktop arm, where a `Instant`
    /// comparison, not `duration_since`, is normally what guards this.
    pub fn duration_since(&self, earlier: Instant) -> Duration {
        self.0.saturating_duration_since(earlier.0)
    }

    /// Time elapsed since `earlier`, or `None` when `earlier` is the later of the
    /// two readings — the same contract as `std::time::Instant`'s method.
    pub fn checked_duration_since(&self, earlier: Instant) -> Option<Duration> {
        self.0.checked_duration_since(earlier.0)
    }

    /// Alias of [`Instant::duration_since`], spelled as callers of the newer
    /// `std` API write it.
    pub fn saturating_duration_since(&self, earlier: Instant) -> Duration {
        self.0.saturating_duration_since(earlier.0)
    }

    /// Time elapsed since this reading was taken.
    pub fn elapsed(&self) -> Duration {
        self.0.elapsed()
    }
}

#[cfg(alloc_frugal)]
impl core::ops::Sub for Instant {
    type Output = Duration;

    /// Both operands are `Copy`, so the subtraction borrows nothing and can be
    /// spelled `a - b` as it is on desktop. `duration_since` is the correct
    /// choice over `self.0 - other.0`: the two readings may be in either order,
    /// and a negative result has to clamp rather than panic.
    fn sub(self, rhs: Instant) -> Duration {
        self.duration_since(rhs)
    }
}

#[cfg(alloc_frugal)]
impl core::ops::Sub<Duration> for Instant {
    type Output = Instant;

    /// Moves a reading backwards in time: `Instant::now() - Duration::from_secs(300)`
    /// seeds a derived field with a reading that has already elapsed. This is the
    /// spelling the desktop arm inherits from `std`, and `compat` exists so that the
    /// same expression compiles in both profiles.
    ///
    /// Arithmetic on this type cannot produce a reading earlier than the epoch of the
    /// wrapped clock, so the subtraction saturates there instead of panicking. That
    /// is the pessimistic end of the range: a saturated value only makes a
    /// derived cooldown look *staler* than requested, which lets the owning check
    /// run rather than holding it back.
    fn sub(self, rhs: Duration) -> Instant {
        Instant(self.0.checked_sub(rhs).unwrap_or_else(|| {
            // `checked_sub` yields `None` only when the result would precede the
            // clock's epoch. Stepping forward from a floor built at the epoch, then
            // walking back to it, is what expresses "as early as this clock can go"
            // without naming a platform-specific minimum.
            let base = std::time::Instant::now();
            let elapsed = base.elapsed();
            base - elapsed
        }))
    }
}

#[cfg(alloc_frugal)]
impl core::ops::SubAssign<Duration> for Instant {
    /// The in-place spelling of `SubAssign`, matching the `a -= interval` form the
    /// desktop arm inherits from `std`.
    ///
    /// Written as a code span rather than a link: these impls only exist under
    /// `alloc_frugal`, so on a `mini` doc build the traits are not in the resolved
    /// scope and `rustdoc` reported `unresolved link to `Sub``.
    fn sub_assign(&mut self, rhs: Duration) {
        *self = *self - rhs;
    }
}

#[cfg(alloc_frugal)]
impl core::ops::Add<Duration> for Instant {
    type Output = Instant;

    /// Deadlines are written `Instant::now() + interval` throughout the crate,
    /// which is why this arm exists rather than a named `advance` method: the
    /// timer and animation call sites are shared with the desktop arm, and
    /// carrying a `#[cfg]` at each of them is exactly what `compat` is for.
    fn add(self, rhs: Duration) -> Instant {
        Instant(self.0 + rhs)
    }
}

#[cfg(alloc_frugal)]
impl core::ops::AddAssign<Duration> for Instant {
    /// Provides the `a += interval` spelling used by the animation clock, which
    /// is the same deadline arithmetic as the `Add` impl written in place. Spelled
    /// as a code span because `Add` does not resolve in every doc profile.
    fn add_assign(&mut self, rhs: Duration) {
        self.0 += rhs;
    }
}

// ── mpsc compat (single-threaded channel for mini builds) ──

/// Single-threaded channel for mini (no_std) builds.
/// Wraps a `VecDeque` behind `RefCell` + `Arc`.
///
/// # Not a real channel
///
/// `std::sync::mpsc` blocks in `Receiver::recv`; this one **cannot**, because
/// there is no second thread to wake it. `recv` is defined as a non-blocking poll
/// that reports `Err(())` — the same shape an empty `try_recv` has — so waiters
/// written for the std API must poll instead. `mini`'s event loop does exactly
/// that: it drains with `dequeue()` rather than `dequeue_blocking()`. A caller
/// that reaches for `recv` expecting it to wait will spin or give up early, not
/// block.
///
/// A side effect of the std-shaped surface is that the exact failure reason is
/// lost: there is no `TryRecvError` here, so "empty" and "disconnected" are both
/// `Err(())`. Sharing the queue through `Arc<RefCell<..>>` also means a re-entrant
/// `send` from inside a `recv` would panic on the borrow, and the pair is `!Send`
/// and `!Sync` — deliberately, since it is single-threaded by construction.
#[cfg(alloc_frugal)]
pub mod mpsc {
    use alloc::collections::VecDeque;
    use alloc::sync::Arc;
    use core::cell::RefCell;

    /// Sending half of the channel; cloning shares the same queue.
    pub struct Sender<T> {
        inner: Arc<RefCell<VecDeque<T>>>,
    }

    impl<T> Clone for Sender<T> {
        /// Shares the queue rather than duplicating it, so every clone delivers to
        /// the same receiver.
        fn clone(&self) -> Self {
            Self { inner: self.inner.clone() }
        }
    }

    impl<T> Sender<T> {
        /// Appends `value` to the queue.
        ///
        /// Always succeeds, even with no receiver left — the queue is unbounded and
        /// does not track whether the receiving half is still alive, so a `Sender`
        /// held after the `Receiver` is dropped just grows a queue nobody reads.
        ///
        /// The `Result<(), ()>` return mirrors `std::sync::mpsc`; this arm never
        /// takes the `Err`.
        // `Result<(), ()>` mirrors the `std::sync::mpsc` API shape.
        #[allow(clippy::result_unit_err)]
        pub fn send(&self, value: T) -> Result<(), ()> {
            self.inner.borrow_mut().push_back(value);
            Ok(())
        }
    }

    /// Receiving half of the channel.
    pub struct Receiver<T> {
        inner: Arc<RefCell<VecDeque<T>>>,
    }

    impl<T> Receiver<T> {
        /// Removes and returns the oldest queued value, or `Err(())` when empty.
        ///
        /// The `Err` carries no reason: see [`Sender`] for why "empty" and
        /// "disconnected" are indistinguishable here.
        #[allow(clippy::result_unit_err)]
        pub fn try_recv(&self) -> Result<T, ()> {
            self.inner.borrow_mut().pop_front().ok_or(())
        }
        /// Removes and returns the oldest queued value, or `Err(())` when empty.
        ///
        /// **Does not block**, unlike `std::sync::mpsc::Receiver::recv` whose name it
        /// takes. There is no thread to wait for under `mini`, so an empty queue is
        /// reported immediately; a caller that must wait has to poll.
        ///
        /// This is kept only so that the profile-independent call sites compile
        /// unchanged; it is an alias for [`Self::try_recv`]. Callers that could block
        /// are gated `not(alloc_frugal)` instead of relying on this method's name —
        /// see [`crate::event::queue`].
        #[allow(clippy::result_unit_err)]
        pub fn recv(&self) -> Result<T, ()> {
            // No threads under mini, so there is nothing to wait on —
            // return the first available value or an error.
            self.inner.borrow_mut().pop_front().ok_or(())
        }
    }

    /// Creates a connected sender/receiver pair over one shared queue.
    ///
    /// The queue starts empty and grows without bound; its capacity is bounded
    /// only by the values the caller posts. The pair is `!Send`, so it cannot be
    /// moved to another thread — see the module-level note.
    pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
        let inner = Arc::new(RefCell::new(VecDeque::new()));
        (Sender { inner: inner.clone() }, Receiver { inner })
    }
}

#[cfg(not(alloc_frugal))]
pub use std::sync::mpsc;

// ── Condvar compat (no_std stub for mini builds) ──

/// A condition variable for thread synchronization.
/// Under mini, all operations are no-ops (single-threaded).
/// Under desktop, re-exports `std::sync::Condvar`.
///
/// # This one is a genuine stub
///
/// The `alloc_frugal` version below is **not** a usable condition variable: it has
/// no `wait` and no `wait_timeout`, so it cannot park a caller, and both notify
/// methods do nothing. Code that needs to wait cannot be written against this
/// type at all — which is why the crate's waiting queues are gated the other way
/// around (see [`crate::event::queue`], whose blocking paths are
/// `not(alloc_frugal)`). The desktop alias is the real thing.
#[cfg(alloc_frugal)]
pub struct Condvar;

#[cfg(alloc_frugal)]
impl Condvar {
    /// Creates the stub. `const`-compatible, and carries no state.
    pub fn new() -> Self {
        Self
    }
    /// Does nothing: there is no waiter to wake.
    pub fn notify_all(&self) {}
    /// Does nothing: there is no waiter to wake.
    pub fn notify_one(&self) {}
}

#[cfg(alloc_frugal)]
crate::impl_default_via_new!(Condvar);

#[cfg(not(alloc_frugal))]
/// A condition variable for thread synchronization.
///
/// The real `std::sync::Condvar`, re-exported so call sites need no profile
/// branch. Unlike the `alloc_frugal` stub it has `wait` / `wait_timeout`, which is
/// what makes the blocking paths of `crate::event::queue` possible on this
/// profile.
pub use std::sync::Condvar;
