// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Core signal implementation with thread-safe slot storage.
//!
//! Provides [`Signal<T>`] — a typed signal that delivers `Arc<T>` payloads to
//! registered slots.  Slots are stored in a `RwLock`-protected `HashMap` for
//! concurrent read (emit) and exclusive write (connect/disconnect/block).
//!
//! # Architecture
//!
//! ```text
//! Signal<T>
//!   └── Arc<SignalInner<T>>
//!         └── RwLock<HashMap<ConnectionHandle, SlotEntry<T>>>
//!               ├── callback: Box<dyn FnMut(Arc<T>) + Send + Sync>
//!               ├── once: bool          — auto-disconnect after first emit
//!               ├── blocked: bool       — skip this slot on emit
//!               └── priority: Priority  — High > Normal > Low
//! ```
//!
//! On emit, slots are sorted by priority into three buckets and invoked
//! High → Normal → Low.  Inside each bucket, slots fire in insertion order.

use crate::compat::HashMap;
use crate::compat::{Mutex, RwLock};
use alloc::sync::Arc;
use core::sync::atomic::{AtomicU64, Ordering};

/// Slot execution priority. Higher-priority slots fire before lower-priority ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Priority {
    /// High priority — fired first.
    High,
    /// Normal priority — default.
    #[default]
    Normal,
    /// Low priority — fired last.
    Low,
}

impl Priority {
    fn rank(&self) -> u8 {
        match self {
            Priority::High => 0,
            Priority::Normal => 1,
            Priority::Low => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
/// Opaque connection handle used to disconnect a slot.
pub struct ConnectionHandle(pub u64);

static NEXT_HANDLE: AtomicU64 = AtomicU64::new(1);

type SlotFn<T> = Box<dyn FnMut(Arc<T>) + Send + Sync + 'static>;

struct SlotEntry<T: Clone + Send + 'static> {
    callback: Option<SlotFn<T>>,
    once: bool,
    blocked: bool,
    priority: Priority,
}

struct SignalInner<T: Clone + Send + 'static> {
    slots: RwLock<HashMap<ConnectionHandle, SlotEntry<T>>>,
}

impl<T: Clone + Send + 'static> SignalInner<T> {
    fn disconnect(&self, handle: ConnectionHandle) -> bool {
        self.slots.write().expect("signal lock poisoned").remove(&handle).is_some()
    }

    fn block(&self, handle: ConnectionHandle) -> bool {
        if let Some(entry) = self.slots.write().expect("signal lock poisoned").get_mut(&handle) {
            entry.blocked = true;
            true
        } else {
            false
        }
    }

    fn unblock(&self, handle: ConnectionHandle) -> bool {
        if let Some(entry) = self.slots.write().expect("signal lock poisoned").get_mut(&handle) {
            entry.blocked = false;
            true
        } else {
            false
        }
    }

    fn is_blocked(&self, handle: ConnectionHandle) -> Option<bool> {
        self.slots.read().expect("signal lock poisoned").get(&handle).map(|entry| entry.blocked)
    }

    fn set_priority(&self, handle: ConnectionHandle, priority: Priority) -> bool {
        if let Some(entry) = self.slots.write().expect("signal lock poisoned").get_mut(&handle) {
            entry.priority = priority;
            true
        } else {
            false
        }
    }
}

/// Owner scope that automatically disconnects tracked signal connections on drop.
#[derive(Default)]
pub struct ConnectionScope {
    disconnectors: Mutex<Vec<Box<dyn FnOnce() + Send + 'static>>>,
}

impl ConnectionScope {
    /// Create an empty connection scope.
    pub fn new() -> Self {
        Self::default()
    }

    /// Manually clear all tracked connections without dropping the scope.
    pub fn clear(&self) {
        let mut disconnectors = self.disconnectors.lock().unwrap_or_else(|e| e.into_inner());
        while let Some(disconnector) = disconnectors.pop() {
            disconnector();
        }
    }

    /// Returns the number of connections currently tracked by this scope.
    pub fn disconnect_count(&self) -> usize {
        self.disconnectors.lock().unwrap_or_else(|e| e.into_inner()).len()
    }

    fn track(&self, disconnector: Box<dyn FnOnce() + Send + 'static>) {
        self.disconnectors.lock().unwrap_or_else(|e| e.into_inner()).push(disconnector);
    }
}

impl Drop for ConnectionScope {
    fn drop(&mut self) {
        let mut disconnectors = self.disconnectors.lock().unwrap_or_else(|e| e.into_inner());
        while let Some(disconnector) = disconnectors.pop() {
            disconnector();
        }
    }
}

/// Generic signal type with typed payload, `once` slots, and scoped auto-disconnect.
#[derive(Clone)]
pub struct Signal<T: Clone + Send + 'static> {
    inner: Arc<SignalInner<T>>,
}

impl<T: Clone + Send + 'static> Signal<T> {
    /// Create an empty signal.
    pub fn new() -> Self {
        Self { inner: Arc::new(SignalInner { slots: RwLock::new(HashMap::new()) }) }
    }

    /// Connect a slot and return its connection handle.
    pub fn connect<F>(&self, slot: F) -> ConnectionHandle
    where
        F: FnMut(Arc<T>) + Send + Sync + 'static,
    {
        self.connect_with_priority(slot, Priority::Normal)
    }

    /// Connect a slot with a specific priority.
    /// High-priority slots fire before Normal, Normal before Low.
    pub fn connect_with_priority<F>(&self, slot: F, priority: Priority) -> ConnectionHandle
    where
        F: FnMut(Arc<T>) + Send + Sync + 'static,
    {
        let handle = ConnectionHandle(NEXT_HANDLE.fetch_add(1, Ordering::Relaxed));
        self.inner.slots.write().expect("signal lock poisoned").insert(
            handle,
            SlotEntry { callback: Some(Box::new(slot)), once: false, blocked: false, priority },
        );
        handle
    }

    /// Connect a slot that is invoked once and then disconnected automatically.
    pub fn connect_once<F>(&self, slot: F) -> ConnectionHandle
    where
        F: FnMut(Arc<T>) + Send + Sync + 'static,
    {
        let handle = ConnectionHandle(NEXT_HANDLE.fetch_add(1, Ordering::Relaxed));
        self.inner.slots.write().expect("signal lock poisoned").insert(
            handle,
            SlotEntry {
                callback: Some(Box::new(slot)),
                once: true,
                blocked: false,
                priority: Priority::Normal,
            },
        );
        handle
    }

    /// Connect a slot bound to a connection scope. It disconnects when the scope is dropped.
    pub fn connect_scoped<F>(&self, owner: &ConnectionScope, slot: F) -> ConnectionHandle
    where
        F: FnMut(Arc<T>) + Send + Sync + 'static,
    {
        let handle = self.connect(slot);
        self.track_owner(owner, handle);
        handle
    }

    /// Connect a once-slot bound to a connection scope.
    pub fn connect_once_scoped<F>(&self, owner: &ConnectionScope, slot: F) -> ConnectionHandle
    where
        F: FnMut(Arc<T>) + Send + Sync + 'static,
    {
        let handle = self.connect_once(slot);
        self.track_owner(owner, handle);
        handle
    }

    /// Disconnect slot by handle. Returns true if the handle was valid.
    pub fn disconnect(&self, handle: ConnectionHandle) -> bool {
        self.inner.disconnect(handle)
    }

    /// Disconnect all slots registered on this signal.
    pub fn disconnect_all(&self) {
        self.inner.slots.write().expect("signal lock poisoned").clear();
    }

    /// Temporarily block a slot without disconnecting it. Returns true if the handle was valid.
    pub fn block(&self, handle: ConnectionHandle) -> bool {
        self.inner.block(handle)
    }

    /// Unblock a previously blocked slot. Returns true if the handle was valid.
    pub fn unblock(&self, handle: ConnectionHandle) -> bool {
        self.inner.unblock(handle)
    }

    /// Returns `Some(true/false)` if the handle exists, `None` if invalid.
    pub fn is_blocked(&self, handle: ConnectionHandle) -> Option<bool> {
        self.inner.is_blocked(handle)
    }

    /// Returns `true` if the handle is still connected (valid).
    pub fn is_connected(&self, handle: ConnectionHandle) -> bool {
        self.inner.slots.read().expect("signal lock poisoned").contains_key(&handle)
    }

    /// Change the priority of an existing connection. Returns true if the handle was valid.
    pub fn set_priority(&self, handle: ConnectionHandle, priority: Priority) -> bool {
        self.inner.set_priority(handle, priority)
    }

    /// Emit a cloned value to all connected (non-blocked) slots.
    ///
    /// This method safely processes slots **one at a time** by temporarily
    /// taking each slot's callback (via `Option::take`) under a write lock,
    /// leaving the handle **in** the HashMap so that `disconnect(own_handle)`
    /// can find and remove it. The callback is invoked **outside** the lock,
    /// and if the handle still exists afterward (i.e., was not self-disconnected),
    /// the callback is restored. Once-slots are removed unconditionally after
    /// invocation. Callbacks may safely call `connect`, `disconnect`,
    /// `disconnect_all`, `block`, `unblock`, or `emit` on **the same Signal**
    /// without deadlocking. Self-disconnect from within a callback is honored
    /// and does not get undone by a stale re-insertion.
    ///
    /// Slots are invoked in priority order (High → Normal → Low).
    /// Blocked slots are skipped entirely.
    ///
    /// # Re-entrancy
    ///
    /// A slot that emits the **same** signal re-enters this function. The
    /// re-entrant pass deliberately skips any slot whose callback is currently
    /// executing: the outer pass owns that callback (it was `take`n out of the
    /// map), so there is nothing to call. This makes recursive emission
    /// terminate rather than recurse without bound — a slot that emits the signal
    /// it is handling would otherwise loop forever.
    ///
    /// The observable consequence, and it is deliberate: **a re-entrant emit does
    /// not deliver to the slot(s) already on the stack.** It delivers to every other
    /// slot, and to slots connected after the outer pass took its snapshot.
    /// [`a_re_entrant_emit_skips_the_slot_still_on_the_stack`] pins this.
    pub fn emit(&self, value: T) {
        let arc_value = Arc::new(value);

        // 1. Snapshot handles and priorities under a read lock.
        let snapshot: Vec<(ConnectionHandle, Priority)> = {
            let slots = self.inner.slots.read().expect("signal lock poisoned");
            slots.iter().map(|(h, e)| (*h, e.priority)).collect()
        };

        // 2. Sort by priority (High first).
        let mut snapshot = snapshot;
        snapshot.sort_by_key(|a| a.1.rank());

        // 3. Process each slot individually against the real HashMap.
        //    The callback is temporarily taken (via Option::take) under a write
        //    lock, leaving the handle in the map so that if the callback calls
        //    `disconnect(own_handle)`, the disconnect can find and remove the
        //    handle. After invocation, if the handle still exists in the map
        //    (i.e., was not self-disconnected), the callback is restored.
        //    Once-slots are removed unconditionally after invocation.
        for (handle, _priority) in snapshot {
            // Temporarily take the callback under a write lock, leaving None.
            // The handle stays in the HashMap so disconnect() can find it.
            let taken = {
                let mut slots = self.inner.slots.write().expect("signal lock poisoned");
                if let Some(entry) = slots.get_mut(&handle) {
                    if entry.blocked {
                        None
                    } else {
                        entry.callback.take()
                    }
                } else {
                    // Handle was disconnected by a prior callback in this emit loop.
                    None
                }
            };

            if let Some(mut callback) = taken {
                callback(arc_value.clone());

                // After callback: if it was a once-slot, remove the entry.
                // Otherwise, re-install the callback only if the handle still
                // exists (i.e., the callback did not call disconnect on itself).
                let mut slots = self.inner.slots.write().expect("signal lock poisoned");
                if let Some(entry) = slots.get_mut(&handle) {
                    if entry.once {
                        // Once-slot: remove the entry entirely.
                        slots.remove(&handle);
                    } else {
                        // Non-once, still connected: restore the callback.
                        entry.callback = Some(callback);
                    }
                }
                // If handle was removed by self-disconnect, callback is dropped.
            }
        }
    }

    /// Return number of currently connected slots.
    pub fn slot_count(&self) -> usize {
        self.inner.slots.read().expect("signal lock poisoned").len()
    }

    fn track_owner(&self, owner: &ConnectionScope, handle: ConnectionHandle) {
        let weak = Arc::downgrade(&self.inner);
        owner.track(Box::new(move || {
            if let Some(inner) = weak.upgrade() {
                let _ = inner.disconnect(handle);
            }
        }));
    }
}

impl<T: Clone + Send + 'static> Default for Signal<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Characterisation tests for [`Signal::emit`].
///
/// # Why these exist
///
/// `emit` is the hot path every one of the crate's ~184 `emit()` call sites goes
/// through, and it carries a subtle invariant: a callback is temporarily *taken*
/// out of the slot map while it runs (so the lock can be released), while its
/// handle stays in the map (so a callback that disconnects itself can be found).
/// That two-piece state is what makes re-entrant `connect` / `disconnect` / `emit`
/// safe.
///
/// It had **no tests**. The module's other tests covered priorities, blocking and
/// scopes, but nothing pinned what happens when a callback mutates the signal it is
/// being called from — which is the only reason the take/restore dance exists. An
/// optimisation of this loop could therefore have silently changed that behaviour
/// with every existing test still green.
///
/// These tests pin the *current, documented* behaviour. They are deliberately
/// characterisation tests: if one fails after a change to `emit`, the change altered
/// observable semantics, and that must be a deliberate decision rather than a
/// side effect.
#[cfg(test)]
mod emit_behaviour_tests {
    use super::*;
    use alloc::sync::Arc;
    use core::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    /// Records the order in which slots ran, so ordering assertions read clearly.
    #[derive(Default)]
    struct Trace {
        entries: Mutex<alloc::vec::Vec<&'static str>>,
    }

    impl Trace {
        fn push(&self, label: &'static str) {
            self.entries.lock().unwrap().push(label);
        }

        fn snapshot(&self) -> alloc::vec::Vec<&'static str> {
            self.entries.lock().unwrap().clone()
        }
    }

    /// A callback that disconnects itself must not be invoked again, must not
    /// panic, and must not be resurrected by the restore step.
    ///
    /// This is the invariant the take/restore design exists for: the handle stays in
    /// the map during the call precisely so the callback's own `disconnect` finds it.
    /// A naive "snapshot the callbacks and call them" loop would re-insert the
    /// callback afterwards and silently undo the disconnect.
    ///
    /// The handle has to be shared through an atomic, because it does not exist until
    /// `connect` returns — yet the closure passed to `connect` has to be able to name
    /// it. That ordering constraint is why this pattern is written out in full rather
    /// than hidden behind a helper.
    #[test]
    fn a_slot_may_disconnect_itself_from_inside_its_callback() {
        use core::sync::atomic::AtomicU64;

        let signal = Signal::<u32>::new();
        let calls = Arc::new(AtomicUsize::new(0));
        let shared_handle = Arc::new(AtomicU64::new(0));

        let calls_self = Arc::clone(&calls);
        let signal_for_self = signal.clone();
        let handle_slot = Arc::clone(&shared_handle);
        let handle = signal.connect(move |_| {
            calls_self.fetch_add(1, Ordering::SeqCst);
            let own = ConnectionHandle(handle_slot.load(Ordering::SeqCst));
            assert!(
                signal_for_self.disconnect(own),
                "a callback must be able to find and remove its own handle"
            );
        });
        shared_handle.store(handle.0, Ordering::SeqCst);

        signal.emit(1);
        assert_eq!(calls.load(Ordering::SeqCst), 1, "the slot must run exactly once");
        assert!(
            !signal.is_connected(handle),
            "self-disconnect must survive the restore step, not be undone by it"
        );

        // A second emit must not reach the disconnected slot.
        signal.emit(2);
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "a self-disconnected slot must not be called by the next emit"
        );
    }

    /// A callback that disconnects a *later* slot must prevent that slot from running
    /// in the same emit pass — the snapshot is taken once, but membership is re-checked
    /// per slot before the call.
    #[test]
    fn a_slot_may_disconnect_a_later_slot_in_the_same_pass() {
        let signal = Signal::<u32>::new();
        let trace = Arc::new(Trace::default());

        let target = {
            let trace = Arc::clone(&trace);
            signal.connect(move |_| trace.push("target"))
        };

        let signal_for_cutter = signal.clone();
        let trace_first = Arc::clone(&trace);
        signal.connect_with_priority(
            move |_| {
                trace_first.push("cutter");
                signal_for_cutter.disconnect(target);
            },
            Priority::High,
        );

        signal.emit(1);

        assert_eq!(
            trace.snapshot(),
            alloc::vec!["cutter"],
            "the disconnected slot must be skipped in the same pass"
        );
    }

    /// A callback that connects a *new* slot must not cause that slot to run in the
    /// current pass: the handle list is snapshot before any callback runs.
    ///
    /// The newly connected slot runs on the *next* pass — but which position it holds
    /// is not asserted, because the slot map is a `HashMap` and its iteration order is
    /// unspecified. Asserting order here would make the test flake on a hash seed
    /// rather than catch a real regression.
    #[test]
    fn connecting_inside_a_callback_does_not_run_in_the_same_pass() {
        let signal = Signal::<u32>::new();
        let trace = Arc::new(Trace::default());

        let signal_for_adder = signal.clone();
        let trace_adder = Arc::clone(&trace);
        signal.connect(move |_| {
            trace_adder.push("adder");
            let trace_late = Arc::clone(&trace_adder);
            signal_for_adder.connect(move |_| trace_late.push("late"));
        });

        signal.emit(1);
        assert_eq!(
            trace.snapshot(),
            alloc::vec!["adder"],
            "a slot connected during emit must wait for the next emit"
        );

        trace.entries.lock().unwrap().clear();
        signal.emit(2);
        let mut order = trace.snapshot();
        order.sort_unstable();
        assert_eq!(
            order,
            alloc::vec!["adder", "late"],
            "both the original and the newly connected slot must run on the next emit"
        );
        // Two passes ran the adder, so it connected two new slots; plus itself = 3.
        // This is expected accumulation, not a leak: a slot that connects a slot on
        // every emit really does grow the signal.
        assert_eq!(signal.slot_count(), 3);
    }

    /// A re-entrant `emit` on the same signal must not deadlock, and must skip the slot
    /// that is still on the stack.
    ///
    /// # What this pins
    ///
    /// The outer pass *takes* the running callback out of the slot map (that is what
    /// lets it call the callback without holding the lock). A nested emit therefore
    /// finds no callback for that handle and skips it. This is what makes recursion
    /// terminate: without it, a slot that emits the signal it is handling would loop
    /// forever.
    ///
    /// The test asserts both halves: the nested pass ran at all (so the skip is not
    /// just "nothing happened"), and it ran a *different* slot while excluding the one
    /// on the stack.
    #[test]
    fn a_re_entrant_emit_skips_the_slot_still_on_the_stack() {
        let signal = Signal::<u32>::new();
        let trace = Arc::new(Trace::default());
        let nested_seen = Arc::new(AtomicUsize::new(0));

        // A second slot, at a lower priority, so the recursive slot runs first.
        {
            let trace = Arc::clone(&trace);
            let nested = Arc::clone(&nested_seen);
            signal.connect_with_priority(
                move |_| {
                    trace.push("observer");
                    nested.fetch_add(1, Ordering::SeqCst);
                },
                Priority::Low,
            );
        }

        let signal_for_reentry = signal.clone();
        let trace_recur = Arc::clone(&trace);
        let depth = Arc::new(AtomicUsize::new(0));
        let depth_inner = Arc::clone(&depth);
        signal.connect_with_priority(
            move |_| {
                trace_recur.push("recur");
                if depth_inner.fetch_add(1, Ordering::SeqCst) == 0 {
                    // Re-enter once. The re-entrant pass must skip *this* slot (it is on
                    // the stack) and must therefore reach the observer without looping.
                    signal_for_reentry.emit(0);
                }
            },
            Priority::High,
        );

        signal.emit(1);

        let order = trace.snapshot();
        assert_eq!(
            order.iter().filter(|l| **l == "recur").count(),
            1,
            "the recursive slot must run once, not twice: {order:?}"
        );
        assert!(
            order.iter().filter(|l| **l == "observer").count() >= 2,
            "the re-entrant pass must still reach the other slot: {order:?}"
        );
        assert_eq!(
            nested_seen.load(Ordering::SeqCst),
            2,
            "the observer must have been called by both the outer and the nested pass"
        );
    }

    /// A `once` slot runs on the first emit and is gone afterwards.
    #[test]
    fn a_once_slot_runs_once_and_is_removed() {
        let signal = Signal::<u32>::new();
        let calls = Arc::new(AtomicUsize::new(0));

        let calls_once = Arc::clone(&calls);
        let handle = signal.connect_once(move |_| {
            calls_once.fetch_add(1, Ordering::SeqCst);
        });

        signal.emit(1);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(!signal.is_connected(handle), "a once slot must be removed after it runs");

        signal.emit(2);
        assert_eq!(calls.load(Ordering::SeqCst), 1, "a once slot must not run twice");
    }

    /// Blocked slots are skipped, and unblocking restores them.
    #[test]
    fn a_blocked_slot_is_skipped_and_unblocking_restores_it() {
        let signal = Signal::<u32>::new();
        let calls = Arc::new(AtomicUsize::new(0));

        let calls_inner = Arc::clone(&calls);
        let handle = signal.connect(move |_| {
            calls_inner.fetch_add(1, Ordering::SeqCst);
        });

        assert!(signal.block(handle));
        signal.emit(1);
        assert_eq!(calls.load(Ordering::SeqCst), 0, "a blocked slot must not run");
        assert!(signal.is_connected(handle), "blocking must not disconnect");

        assert!(signal.unblock(handle));
        signal.emit(2);
        assert_eq!(calls.load(Ordering::SeqCst), 1, "an unblocked slot must run again");
    }

    /// Slots run in priority order, and equal priorities preserve a stable order.
    ///
    /// A `HashMap` has no iteration order, so this is what the `sort_by_key` in
    /// `emit` actually buys: without it the order across a multi-slot signal would be
    /// arbitrary and this test would flake rather than fail honestly.
    #[test]
    fn slots_run_in_priority_order() {
        let signal = Signal::<u32>::new();
        let trace = Arc::new(Trace::default());

        for (label, priority) in [
            ("low", Priority::Low),
            ("normal", Priority::Normal),
            ("high", Priority::High),
            ("normal2", Priority::Normal),
        ] {
            let trace = Arc::clone(&trace);
            signal.connect_with_priority(move |_| trace.push(label), priority);
        }

        signal.emit(1);
        let order = trace.snapshot();

        assert_eq!(order.first(), Some(&"high"), "High must run first");
        assert_eq!(order.last(), Some(&"low"), "Low must run last");
        assert_eq!(order.len(), 4, "every non-blocked slot must run exactly once, got {order:?}");
    }

    /// Changing a connection's priority mid-emit must not corrupt the pass: the
    /// snapshot's order is fixed when emit starts.
    #[test]
    fn set_priority_inside_a_callback_does_not_reorder_the_current_pass() {
        let signal = Signal::<u32>::new();
        let trace = Arc::new(Trace::default());

        let later = {
            let trace = Arc::clone(&trace);
            signal.connect_with_priority(move |_| trace.push("later"), Priority::Low)
        };

        let signal_for_bump = signal.clone();
        let trace_first = Arc::clone(&trace);
        signal.connect_with_priority(
            move |_| {
                trace_first.push("first");
                // Promote the low-priority slot to High. The current pass already
                // snapshotted the order, so this must not move it ahead of us.
                signal_for_bump.set_priority(later, Priority::High);
            },
            Priority::High,
        );

        signal.emit(1);
        assert_eq!(
            trace.snapshot(),
            alloc::vec!["first", "later"],
            "the pass order is fixed at snapshot time"
        );
    }

    /// `disconnect_all` from inside a callback must stop the remaining slots.
    #[test]
    fn disconnect_all_inside_a_callback_stops_the_rest_of_the_pass() {
        let signal = Signal::<u32>::new();
        let trace = Arc::new(Trace::default());

        {
            let trace = Arc::clone(&trace);
            signal.connect_with_priority(move |_| trace.push("second"), Priority::Normal);
        }

        let signal_for_clear = signal.clone();
        let trace_first = Arc::clone(&trace);
        signal.connect_with_priority(
            move |_| {
                trace_first.push("first");
                signal_for_clear.disconnect_all();
            },
            Priority::High,
        );

        signal.emit(1);
        assert_eq!(
            trace.snapshot(),
            alloc::vec!["first"],
            "slots after disconnect_all must be skipped"
        );
        assert_eq!(signal.slot_count(), 0u64 as usize, "no slots must remain");
    }

    /// Dropping a `ConnectionScope` disconnects the connections registered through it,
    /// including one whose callback is currently running.
    #[test]
    fn a_scoped_connection_is_dropped_with_its_scope() {
        let signal = Signal::<u32>::new();
        let calls = Arc::new(AtomicUsize::new(0));

        {
            let scope = ConnectionScope::new();
            let calls_inner = Arc::clone(&calls);
            signal.connect_scoped(&scope, move |_| {
                calls_inner.fetch_add(1, Ordering::SeqCst);
            });
            signal.emit(1);
            assert_eq!(calls.load(Ordering::SeqCst), 1, "the scoped slot must run while alive");
        }

        signal.emit(2);
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "the scoped slot must be gone once its scope drops"
        );
    }
}
