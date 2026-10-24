// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! CORE-N02: the emit path's allocation behaviour, measured rather than described.
//!
//! `Signal::emit` used to start with `Arc::new(value)`, so every emit — including a `bool`
//! or a unit signal — allocated on the heap. The fix gives slots two calling conventions:
//!
//! * `connect_ref(FnMut(&T))` reads the value `emit` already owns → no allocation;
//! * `connect(FnMut(Arc<T>))` promises the callback may keep the payload → one `Arc` per
//!   emit pass, shared by every `Arc`-taking slot in that pass.
//!
//! These tests pin both halves with a counting allocator: "zero" cannot be observed through
//! the public API, and asserting only the delivered value would pass for an implementation
//! that still allocates. The counter is **thread-local**: a process-global counter also counted
//! whatever the test harness allocated on its own thread while the window was open, which was
//! measured as an intermittent extra allocation (one run in fifteen). Counting only the thread
//! that runs the operation is the property these tests actually assert, and it also lets the
//! file's tests run in parallel again.
//!
//! The OpenHarmony cross target expands `thread_local!` through a std shim whose const path
//! the lint cannot see, so an already-`const` initialiser there is reported as non-const.
#![allow(clippy::missing_const_for_thread_local)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};

use rust_widgets::action::Action;
use rust_widgets::signal::{GenericSignal, Signal};

struct CountingAllocator;

thread_local! {
    /// Whether *this thread* is inside a measuring window.
    static COUNTING: Cell<bool> = const { Cell::new(false) };
    /// Allocations this thread made while counting.
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

// SAFETY: every operation delegates to `System` with the caller's own layout and pointer.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if COUNTING.with(Cell::get) {
            ALLOCATIONS.with(|count| count.set(count.get() + 1));
        }
        // SAFETY: `layout` comes from the allocator caller and is passed through unchanged.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: `ptr` and `layout` come from the allocator caller and are passed through.
        unsafe { System.dealloc(ptr, layout) }
    }
}

/// Serialises this file's tests so their thread-local windows cannot overlap.
fn exclusive() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Runs `body` with counting on and reports how many allocations it made on this thread.
fn allocations_during<R>(body: impl FnOnce() -> R) -> (R, usize) {
    // Touch both thread-locals **before** the window opens: the first access initialises the
    // TLS block, and that setup must not be counted as the operation's cost.
    let _ = COUNTING.with(Cell::get);
    let _ = ALLOCATIONS.with(Cell::get);
    ALLOCATIONS.with(|count| count.set(0));
    COUNTING.with(|flag| flag.set(true));
    let value = body();
    COUNTING.with(|flag| flag.set(false));
    (value, ALLOCATIONS.with(Cell::get))
}

/// A payload-carrying signal whose only subscriber reads the value allocates nothing.
#[test]
fn a_borrowed_slot_emit_allocates_nothing() {
    let _guard = exclusive();
    let signal = Signal::<u32>::new();
    let seen = std::sync::Arc::new(AtomicU32::new(0));
    let target = std::sync::Arc::clone(&seen);
    signal.connect_ref(move |value: &u32| target.store(*value, Ordering::SeqCst));

    // Warm-up: the per-thread "slot is running here" stack grows on the first delivery.
    // That is one-time thread-local setup, not a per-emit cost.
    let _ = allocations_during(|| signal.emit(1));

    let (_, allocations) = allocations_during(|| signal.emit(7));
    assert_eq!(allocations, 0, "a borrow-only emit must not allocate");
    assert_eq!(seen.load(Ordering::SeqCst), 7);
}

/// An `Arc`-taking slot forces the shared payload, and the whole pass shares one `Arc`.
#[test]
fn owned_slots_allocate_one_arc_per_emit_pass() {
    let _guard = exclusive();
    let signal = Signal::<u32>::new();
    let first = std::sync::Arc::new(AtomicU32::new(0));
    let second = std::sync::Arc::new(AtomicU32::new(0));

    let a = std::sync::Arc::clone(&first);
    signal.connect(move |value: std::sync::Arc<u32>| a.store(*value, Ordering::SeqCst));
    let b = std::sync::Arc::clone(&second);
    signal.connect(move |value: std::sync::Arc<u32>| b.store(*value, Ordering::SeqCst));

    // Warm-up: the multi-subscriber snapshot is a thread-local that grows on first use.
    let _ = allocations_during(|| signal.emit(1));

    let (_, allocations) = allocations_during(|| signal.emit(7));
    assert_eq!(allocations, 1, "one Arc for the pass, not one per slot");
    assert_eq!(first.load(Ordering::SeqCst), 7);
    assert_eq!(second.load(Ordering::SeqCst), 7);
}

/// Unit signals are the hot path (`clicked`, `closed`, `redraw_requested`); their wrapper
/// connects borrowed slots internally, so they never materialise an `Arc<()>`.
#[test]
fn a_unit_signal_emit_allocates_nothing() {
    let _guard = exclusive();
    let signal = GenericSignal::new();
    let calls = std::sync::Arc::new(AtomicUsize::new(0));
    let target = std::sync::Arc::clone(&calls);
    signal.connect(move || {
        target.fetch_add(1, Ordering::SeqCst);
    });

    // Warm-up: one-time thread-local setup on the first delivery (see the note above).
    let _ = allocations_during(|| signal.emit());
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    let (_, allocations) = allocations_during(|| signal.emit());
    assert_eq!(allocations, 0, "a unit signal must not allocate an Arc<()>");
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

/// The read-only `Action` forms exist precisely so a caller that only observes the new value
/// does not pay for shared ownership.
#[test]
fn read_only_action_subscribers_emit_without_allocating() {
    let _guard = exclusive();
    let mut action = Action::new("bold", "Bold");
    action.set_checkable(true);
    let seen = std::sync::Arc::new(AtomicBool::new(false));
    let target = std::sync::Arc::clone(&seen);
    action.connect_toggled_ref(move |value: &bool| target.store(*value, Ordering::SeqCst));

    // Warm-up: one-time thread-local setup on the first delivery (see the note above).
    let _ = allocations_during(|| action.set_checked(true));
    assert!(seen.load(Ordering::SeqCst));

    let (_, allocations) = allocations_during(|| action.set_checked(false));
    assert_eq!(allocations, 0, "a read-only toggled subscriber must not allocate");
    assert!(!seen.load(Ordering::SeqCst));
}

/// The documented alternative still works, and its cost is the one `connect` promises:
/// a single `Arc` per state change.
#[test]
fn owned_action_subscribers_allocate_one_arc() {
    let _guard = exclusive();
    let mut action = Action::new("bold", "Bold");
    action.set_checkable(true);
    let seen = std::sync::Arc::new(AtomicBool::new(false));
    let target = std::sync::Arc::clone(&seen);
    action.connect_toggled(move |value: std::sync::Arc<bool>| {
        target.store(*value, Ordering::SeqCst);
    });

    // Warm-up so the measurement is of the emit, not of any first-use lazy state.
    let _ = allocations_during(|| action.set_checked(true));

    let (_, allocations) = allocations_during(|| action.set_checked(false));
    assert_eq!(allocations, 1, "an Arc-taking slot costs one Arc for that change");
    assert!(!seen.load(Ordering::SeqCst));
}
