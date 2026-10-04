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
//!               ├── slot: Arc<SlotSlot<T>>
//!               │     ├── callback: Mutex<FnMut(Arc<T>)>
//!               │     └── pending: Mutex<VecDeque<T>>
//!               ├── once: bool          — auto-disconnect after first emit
//!               ├── blocked: bool       — skip this slot on emit
//!               └── priority: Priority  — High > Normal > Low
//! ```
//!
//! On emit, slots are sorted by priority into three buckets and invoked
//! High → Normal → Low.  Inside each bucket, slots fire in insertion order.

use crate::compat::{lock, read_lock, write_lock, Box, HashMap, Vec};
use crate::compat::{Mutex, RwLock, VecDeque};
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

/// A slot's callback and its deferred-delivery queue, so concurrent emits neither drop a value nor
/// hold a lock across a callback that may emit another signal.
///
/// # Why a per-slot `Arc` with a pending queue (BLUE-issues E-21 and E-29)
///
/// `emit` must invoke a `FnMut`, which needs exclusive access to the closure. Taking the callback
/// **out** of the slot map for the call (leaving `None`) made a concurrent emit on another thread see
/// a connected, unblocked slot as absent and silently drop its value (E-21). Holding a lock across the
/// callback fixed that but deadlocked when two signals forwarded into each other across threads: each
/// thread held one slot's lock while waiting for the other's (E-29).
///
/// The queue breaks the cycle. The callback mutex is taken with `try_lock`:
/// * **success** \u2014 run the callback, then drain any values another thread deferred while we held it;
/// * **failure** \u2014 the slot is executing elsewhere. Rather than block (which can cycle) or drop (which
///   loses the value), the value is appended to `pending` and the running thread delivers it before it
///   releases. No lock is ever held while acquiring another slot's lock, so no lock-order cycle can
///   form, and no value is dropped.
struct SlotSlot<T: Clone + Send + 'static> {
    callback: Mutex<SlotFn<T>>,
    /// Values deferred by a concurrent emit whose `try_lock` found the callback busy.
    ///
    /// Stored as owned `T` rather than `Arc<T>` so this type is `Send + Sync` from `T: Send` alone,
    /// matching `Signal<T>`'s public bound. FIFO order preserves the order in which concurrent
    /// emitters acquired this queue; each value is re-wrapped in an `Arc` when delivered.
    pending: Mutex<VecDeque<T>>,
}

impl<T: Clone + Send + 'static> SlotSlot<T> {
    fn new(callback: SlotFn<T>) -> Self {
        Self { callback: Mutex::new(callback), pending: Mutex::new(VecDeque::new()) }
    }

    /// Runs `value` on this slot if the callback is free, otherwise defers it.
    ///
    /// Returns `true` if the callback ran here (the caller then owns draining the queue).
    fn deliver(&self, value: Arc<T>) -> bool {
        let Some(mut callback) = crate::compat::try_lock_recover(&self.callback) else {
            // Busy: defer rather than block (which could cycle) or drop (which loses the value).
            lock(&self.pending).push_back((*value).clone());
            return false;
        };
        callback(value);
        // Deliver everything another thread queued while we held the callback, before releasing it.
        loop {
            let next = lock(&self.pending).pop_front();
            match next {
                Some(queued) => callback(Arc::new(queued)),
                None => break,
            }
        }
        true
    }
}

struct SlotEntry<T: Clone + Send + 'static> {
    /// Present while the entry is connected; absent only after logical removal.
    slot: Option<Arc<SlotSlot<T>>>,
    once: bool,
    blocked: bool,
    priority: Priority,
    /// Monotonic connection order, used only to break ties between equal-priority slots.
    ///
    /// # Why the handle alone cannot order slots
    ///
    /// Slots live in a `HashMap`, whose iteration order is unspecified and can differ between runs
    /// and between map layouts. Sorting by priority alone therefore left equal-priority slots in
    /// whatever order the map happened to yield them, so "same priority fires in connection order" —
    /// the module's documented contract — was not actually provided. The sequence is assigned from
    /// the same monotonic counter as the handle, so it is unique and strictly increasing in
    /// connection order.
    sequence: u64,
}

// Marks a slot as currently executing **on this thread**, so a same-thread re-entrant emit skips it
// instead of dead-locking on its own mutex.
//
// The set is keyed by `(signal identity, connection handle)`, which together identify one slot.
// A plain comment rather than `///`: `thread_local!` does not turn leading doc comments into item
// docs, and the `cfg` in between would make them dangle anyway.
thread_local! {
    #[allow(clippy::missing_const_for_thread_local)]
    static EXECUTING_SLOTS: core::cell::RefCell<crate::compat::Vec<(usize, ConnectionHandle)>> =
        core::cell::RefCell::new(crate::compat::Vec::new());
}

/// Whether `(identity, handle)` is currently executing on this thread.
fn slot_running_here(identity: usize, handle: ConnectionHandle) -> bool {
    EXECUTING_SLOTS.with(|set| set.borrow().contains(&(identity, handle)))
}

/// Records `(identity, handle)` as executing on this thread for the guard's lifetime.
struct SlotRunning(usize, ConnectionHandle);

impl SlotRunning {
    fn new(identity: usize, handle: ConnectionHandle) -> Self {
        EXECUTING_SLOTS.with(|set| set.borrow_mut().push((identity, handle)));
        Self(identity, handle)
    }
}

impl Drop for SlotRunning {
    fn drop(&mut self) {
        EXECUTING_SLOTS.with(|set| {
            let mut set = set.borrow_mut();
            if let Some(pos) = set.iter().position(|entry| *entry == (self.0, self.1)) {
                set.remove(pos);
            }
        });
    }
}

struct SignalInner<T: Clone + Send + 'static> {
    slots: RwLock<HashMap<ConnectionHandle, SlotEntry<T>>>,
}

impl<T: Clone + Send + 'static> SignalInner<T> {
    fn new() -> Self {
        Self { slots: RwLock::new(HashMap::new()) }
    }
    fn disconnect(&self, handle: ConnectionHandle) -> bool {
        write_lock(&self.slots).remove(&handle).is_some()
    }

    fn block(&self, handle: ConnectionHandle) -> bool {
        if let Some(entry) = write_lock(&self.slots).get_mut(&handle) {
            entry.blocked = true;
            true
        } else {
            false
        }
    }

    fn unblock(&self, handle: ConnectionHandle) -> bool {
        if let Some(entry) = write_lock(&self.slots).get_mut(&handle) {
            entry.blocked = false;
            true
        } else {
            false
        }
    }

    fn is_blocked(&self, handle: ConnectionHandle) -> Option<bool> {
        read_lock(&self.slots).get(&handle).map(|entry| entry.blocked)
    }

    fn set_priority(&self, handle: ConnectionHandle, priority: Priority) -> bool {
        if let Some(entry) = write_lock(&self.slots).get_mut(&handle) {
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
        let mut disconnectors = lock(&self.disconnectors);
        while let Some(disconnector) = disconnectors.pop() {
            disconnector();
        }
    }

    /// Returns the number of connections currently tracked by this scope.
    pub fn disconnect_count(&self) -> usize {
        lock(&self.disconnectors).len()
    }

    fn track(&self, disconnector: Box<dyn FnOnce() + Send + 'static>) {
        lock(&self.disconnectors).push(disconnector);
    }
}

impl Drop for ConnectionScope {
    fn drop(&mut self) {
        let mut disconnectors = lock(&self.disconnectors);
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
        Self { inner: Arc::new(SignalInner::new()) }
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
        let sequence = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed);
        let handle = ConnectionHandle(sequence);
        write_lock(&self.inner.slots).insert(
            handle,
            SlotEntry {
                slot: Some(Arc::new(SlotSlot::new(Box::new(slot)))),
                once: false,
                blocked: false,
                priority,
                sequence,
            },
        );
        handle
    }

    /// Connect a slot that is invoked once and then disconnected automatically.
    pub fn connect_once<F>(&self, slot: F) -> ConnectionHandle
    where
        F: FnMut(Arc<T>) + Send + Sync + 'static,
    {
        let sequence = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed);
        let handle = ConnectionHandle(sequence);
        write_lock(&self.inner.slots).insert(
            handle,
            SlotEntry {
                slot: Some(Arc::new(SlotSlot::new(Box::new(slot)))),
                once: true,
                blocked: false,
                priority: Priority::Normal,
                sequence,
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
        write_lock(&self.inner.slots).clear();
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
        read_lock(&self.inner.slots).contains_key(&handle)
    }

    /// A process-unique identity for this signal instance.
    ///
    /// # Why an identity is needed
    ///
    /// Two signals of the same type and name are still different signals, and a query that
    /// asks "is *this* control's event wired?" must compare the actual signal, not merely its
    /// name. Without an identity, a subscription made on one instance's signal (an `A` that was
    /// wired) made every sibling instance (`B`, which was never wired) report itself wired, because
    /// the binder could only see that a forward existed for that *event name*. This is the address
    /// of the shared `SignalInner`, so all clones of one signal answer with the same value and a
    /// distinct signal answers with a different one.
    ///
    /// # Why an address rather than a counter
    ///
    /// The value is only ever compared for equality against another identity taken from a signal
    /// that is alive at the same time, so it never needs to be globally unique across the process's
    /// lifetime — only distinct between two live signals, which distinct `Arc` allocations are.
    /// It must not outlive the signal it was taken from; callers compare it while both signals are
    /// reachable (the binder stores it beside the handle it refers to, and only compares it while
    /// resolving a live control's own signal).
    pub fn identity(&self) -> usize {
        Arc::as_ptr(&self.inner) as usize
    }

    /// Change the priority of an existing connection. Returns true if the handle was valid.
    pub fn set_priority(&self, handle: ConnectionHandle, priority: Priority) -> bool {
        self.inner.set_priority(handle, priority)
    }

    /// Emit a cloned value to all connected (non-blocked) slots.
    ///
    /// The slot list is snapshotted under a read lock. Each slot is then looked
    /// up under a short map lock and invoked without holding the signal lock.
    /// Its callback has a per-slot mutex; a concurrent emit that finds it busy
    /// defers its value to that slot's pending queue rather than blocking and
    /// risking a cross-signal deadlock or dropping the value. Once-slots are
    /// removed under the map lock before their callback is invoked, so only one
    /// concurrent emit can claim them. Callbacks may safely call `connect`,
    /// `disconnect`, `disconnect_all`, `block`, `unblock`, or `emit` on **the
    /// same Signal** without deadlocking. Self-disconnect from within an ordinary
    /// callback is honored because the callback is never reinserted.
    ///
    /// Slots are invoked in priority order (High → Normal → Low).
    /// Blocked slots are skipped entirely.
    ///
    /// # Re-entrancy
    ///
    /// A slot that emits the **same** signal re-enters this function. The
    /// re-entrant pass deliberately skips any slot whose callback is currently
    /// executing. This makes recursive emission
    /// terminate rather than recurse without bound — a slot that emits the signal
    /// it is handling would otherwise loop forever.
    ///
    /// The observable consequence, and it is deliberate: **a re-entrant emit does
    /// not deliver to the slot(s) already on the stack.** It delivers to every other
    /// slot, and to slots connected after the outer pass took its snapshot.
    /// `a_re_entrant_emit_skips_the_slot_still_on_the_stack` pins this.
    ///
    /// # Concurrent emission from other threads
    ///
    /// Re-entrancy is a *same-thread* fact: the slot is on this thread's stack. An emit on a
    /// **different** thread is not re-entrancy, and the value it carries must not be dropped merely
    /// because this thread happens to be inside a callback. Each slot's callback lives behind its own
    /// mutex, so a concurrent emit defers its value to that slot and the running thread delivers it
    /// after the current callback; it never holds a signal-global lock (BLUE-issues E-21 and E-29).
    pub fn emit(&self, value: T) {
        self.emit_inner(Arc::new(value));
    }

    /// The emit pass: snapshot the slot list, then invoke each slot in priority order.
    fn emit_inner(&self, arc_value: Arc<T>) {
        // 1. Snapshot handles, priorities and connection order under a read lock.
        let snapshot: Vec<(ConnectionHandle, Priority, u64)> = {
            let slots = read_lock(&self.inner.slots);
            slots.iter().map(|(h, e)| (*h, e.priority, e.sequence)).collect()
        };

        // 2. Sort by priority (High first), then by connection order within a priority.
        //
        // The sequence is the tie-breaker that makes "same priority fires in connection order",
        // the module's documented contract, actually hold: the snapshot comes from a `HashMap`,
        // whose iteration order is unspecified, so without it the within-priority order was the
        // map's and could change between runs.
        let mut snapshot = snapshot;
        snapshot.sort_by_key(|a| (a.1.rank(), a.2));

        let identity = self.identity();

        // 3. Invoke each slot by cloning its `Arc<Mutex<..>>` under a short read lock, then running it
        //    with **no signal lock held**. Holding no lock across the callback is what removes the
        //    cross-signal lock cycle (E-29); the per-slot mutex is what keeps a concurrent emit from
        //    dropping the value (E-21).
        for (handle, _priority, _sequence) in snapshot {
            // Claim once-slots atomically with the lookup: removing one under the same map lock
            // prevents another emit that already snapshotted this handle from cloning the slot
            // while the first callback is still running.
            let slot = {
                let mut slots = write_lock(&self.inner.slots);
                let Some(entry) = slots.get(&handle) else {
                    continue;
                };
                if entry.blocked {
                    None
                } else {
                    let slot = entry.slot.clone();
                    if entry.once {
                        slots.remove(&handle);
                    }
                    slot
                }
            };
            let Some(slot) = slot else {
                // Blocked, or disconnected by a prior callback in this pass.
                continue;
            };

            // A same-thread re-entrant emit must skip the slot already on the stack: its callback is
            // executing on this very thread, so running it again would recurse without bound. The
            // documented contract is that the slot does not run twice from one stack. Cross-thread
            // callers are NOT skipped — they defer their value to the running thread (see `deliver`).
            if slot_running_here(identity, handle) {
                continue;
            }

            // Record that this slot is executing on this thread before running it, so a nested emit of
            // the same signal skips it instead of recursing. The guard clears the marker on normal
            // return and on an unwind out of the callback.
            let _running = SlotRunning::new(identity, handle);

            // `deliver` takes the per-slot callback with `try_lock`: it runs the callback here when
            // free, and otherwise **defers** the value to the thread already running it. No lock is
            // held while another slot's lock is acquired, so a mutual cross-signal forward cannot
            // dead-lock, and a concurrent emit cannot drop its value (BLUE-issues E-21, E-29).
            let _ = slot.deliver(arc_value.clone());
        }
    }

    /// Return number of currently connected slots.
    pub fn slot_count(&self) -> usize {
        read_lock(&self.inner.slots).len()
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
    use crate::compat::lock;
    use alloc::sync::Arc;
    use core::sync::atomic::{AtomicUsize, Ordering};

    /// Records the order in which slots ran, so ordering assertions read clearly.
    #[derive(Default)]
    struct Trace {
        entries: crate::compat::Mutex<alloc::vec::Vec<&'static str>>,
    }

    impl Trace {
        fn push(&self, label: &'static str) {
            lock(&self.entries).push(label);
        }

        fn snapshot(&self) -> alloc::vec::Vec<&'static str> {
            lock(&self.entries).clone()
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

        lock(&trace.entries).clear();
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

    /// Concurrent emitters must not both claim a once-slot from the same snapshot.
    #[test]
    fn concurrent_emits_claim_a_once_slot_only_once() {
        use std::sync::Barrier;

        const ROUNDS: usize = 200;
        let signal = Signal::<u32>::new();
        let start = Arc::new(Barrier::new(3));
        let done = Arc::new(Barrier::new(3));
        let mut workers = Vec::new();

        for _ in 0..2 {
            let worker_signal = signal.clone();
            let worker_start = Arc::clone(&start);
            let worker_done = Arc::clone(&done);
            workers.push(std::thread::spawn(move || {
                for round in 0..ROUNDS {
                    worker_start.wait();
                    worker_signal.emit(round as u32);
                    worker_done.wait();
                }
            }));
        }

        for _ in 0..ROUNDS {
            let calls = Arc::new(AtomicUsize::new(0));
            let calls_once = Arc::clone(&calls);
            let handle = signal.connect_once(move |_| {
                calls_once.fetch_add(1, Ordering::SeqCst);
            });

            start.wait();
            done.wait();
            assert_eq!(
                calls.load(Ordering::SeqCst),
                1,
                "simultaneous emitters must invoke a once-slot exactly once"
            );
            assert!(!signal.is_connected(handle), "the once-slot must be removed");
        }

        for worker in workers {
            worker.join().expect("emit worker must not panic");
        }
    }

    /// Values deferred while a callback is busy must retain their arrival order.
    #[test]
    fn concurrent_pending_values_are_delivered_fifo() {
        use std::sync::Barrier;

        let signal = Signal::<u32>::new();
        let entered = Arc::new(Barrier::new(2));
        let release = Arc::new(Barrier::new(2));
        let observed = Arc::new(std::sync::Mutex::new(Vec::new()));

        let slot_entered = Arc::clone(&entered);
        let slot_release = Arc::clone(&release);
        let slot_observed = Arc::clone(&observed);
        signal.connect(move |value| {
            let value = *value;
            slot_observed.lock().unwrap().push(value);
            if value == 1 {
                slot_entered.wait();
                slot_release.wait();
            }
        });

        let worker_signal = signal.clone();
        let worker = std::thread::spawn(move || worker_signal.emit(1));
        entered.wait();
        signal.emit(2);
        signal.emit(3);
        release.wait();
        worker.join().expect("the in-flight emitter must complete");

        assert_eq!(*observed.lock().unwrap(), vec![1, 2, 3]);
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

    /// Equal-priority slots fire in **connection order**, not in the map's arbitrary order.
    ///
    /// # What this pins
    ///
    /// The module documents "inside each bucket, slots fire in insertion order". The snapshot is
    /// taken from a `HashMap`, whose iteration order is unspecified, so before the `sequence`
    /// tie-breaker the within-priority order was the map's and could differ between runs or map
    /// layouts. This connects several slots at one priority and asserts the run order is exactly the
    /// connection order; it also re-connects after removals to exercise a rearranged map.
    #[test]
    fn equal_priority_slots_fire_in_connection_order() {
        let signal = Signal::<u32>::new();
        let trace = Arc::new(Trace::default());

        let mut handles = alloc::vec::Vec::new();
        for label in ["first", "second", "third", "fourth"] {
            let trace = Arc::clone(&trace);
            handles.push(signal.connect(move |_| trace.push(label)));
        }

        signal.emit(1);
        assert_eq!(
            trace.snapshot(),
            alloc::vec!["first", "second", "third", "fourth"],
            "same-priority slots must run in connection order"
        );

        // Disconnect the first two, then connect a fresh slot. The map is now sparse, so its
        // iteration order is very unlikely to coincide with the surviving connection order; the new
        // slot is the one that records into a second trace.
        signal.disconnect(handles[0]);
        signal.disconnect(handles[1]);
        let trace2 = Arc::new(Trace::default());
        {
            let trace2 = Arc::clone(&trace2);
            signal.connect(move |_| trace2.push("fifth"));
        }
        signal.emit(2);
        // `third` and `fourth` still run, but they append to `trace`; the surviving order there is
        // what pins the tie-break after the map has been rearranged.
        assert_eq!(
            trace.snapshot(),
            alloc::vec!["first", "second", "third", "fourth", "third", "fourth"],
            "the two surviving original slots must run in connection order"
        );
        assert_eq!(trace2.snapshot(), alloc::vec!["fifth"], "the new slot must run once");
    }

    /// A slot that unwinds must not leave the signal with "a slot that counts but never runs".
    ///
    /// # What this pins
    ///
    /// `emit` takes the callback out of the map while it runs and restores it afterwards. If the
    /// restore is plain code after the call, a panicking callback leaves the entry in the map with
    /// `callback == None` — `slot_count` still counts it, a later emit silently skips it, and the
    /// "is this wired?" query (rule #97) reports a live wire that can never fire again. The restore
    /// is therefore a `Drop` guard, and this test proves it runs on the unwind path too.
    #[test]
    fn a_panicking_slot_is_restored_so_later_emits_still_reach_it() {
        let signal = Signal::<u32>::new();
        let calls = Arc::new(AtomicUsize::new(0));
        let should_panic = Arc::new(core::sync::atomic::AtomicBool::new(true));

        let calls_slot = Arc::clone(&calls);
        let panic_flag = Arc::clone(&should_panic);
        signal.connect(move |_| {
            calls_slot.fetch_add(1, Ordering::SeqCst);
            if panic_flag.load(Ordering::SeqCst) {
                panic!("deliberate unwind from a slot callback");
            }
        });

        // The first emit unwinds; the host catches it, exactly as a GUI host would.
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| signal.emit(1)));
        assert!(caught.is_err(), "the callback must have unwound");

        // The slot is still present — the guard restored it — and a later emit reaches it.
        assert_eq!(signal.slot_count(), 1, "the slot must still be counted after an unwind");
        should_panic.store(false, Ordering::SeqCst);
        signal.emit(2);
        assert_eq!(
            calls.load(Ordering::SeqCst),
            2,
            "the restored slot must run on the next emit, not be left as a dead entry"
        );
    }

    /// A concurrent emit on another thread must deliver, not be silently dropped.
    ///
    /// # The defect this pins (BLUE-issue E-21)
    ///
    /// `emit` takes a slot's callback out of the map while it runs. A second emit running
    /// **concurrently on another thread** therefore found `callback == None` for an entry that was
    /// still connected and not blocked, and skipped it: the value was lost with no queue, error or
    /// diagnostic. Emits are now serialised per signal, so the second emit waits and then delivers.
    ///
    /// The first callback is held open on a barrier so the second emit is *guaranteed* to overlap it
    /// rather than merely likely to; the test is deterministic, not timing-dependent.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn a_concurrent_emit_is_delivered_rather_than_silently_dropped() {
        use std::sync::Barrier;

        let signal = Signal::<u32>::new();
        let delivered = Arc::new(AtomicUsize::new(0));
        // Two barriers: the callback signals it has started, then waits for the main thread to say
        // its second emit has been issued (or is blocked waiting for the serialisation lock).
        let entered = Arc::new(Barrier::new(2));
        let release = Arc::new(Barrier::new(2));

        let delivered_slot = Arc::clone(&delivered);
        let entered_slot = Arc::clone(&entered);
        let release_slot = Arc::clone(&release);
        signal.connect(move |_| {
            delivered_slot.fetch_add(1, Ordering::SeqCst);
            // Only the first delivery blocks; the second must pass straight through.
            if delivered_slot.load(Ordering::SeqCst) == 1 {
                entered_slot.wait();
                release_slot.wait();
            }
        });

        let worker_signal = signal.clone();
        let worker = std::thread::spawn(move || {
            worker_signal.emit(1);
        });

        // Wait until the worker is inside the callback (holding the serialisation lock).
        entered.wait();

        // Now emit a second value from this thread. Before the fix this found the callback taken and
        // dropped the value; with serialisation it blocks until the worker's callback returns, then
        // delivers.
        let emitter_signal = signal.clone();
        let emitter = std::thread::spawn(move || {
            emitter_signal.emit(2);
        });

        // Let the first callback finish so the second emit can proceed, then join both threads.
        release.wait();
        worker.join().expect("the worker thread must not panic");
        emitter.join().expect("the emitter thread must not panic");

        assert_eq!(
            delivered.load(Ordering::SeqCst),
            2,
            "both emits must reach the connected slot; a dropped value is the silent-loss defect"
        );
    }

    /// Two signals that forward into each other across threads must both finish.
    ///
    /// # The defect this pins (BLUE-issue E-29)
    ///
    /// The E-21 fix serialised a whole emit on a **signal-level** mutex. When thread T1 ran `A`'s
    /// callback and, inside it, emitted `B`, while T2 ran `B`'s callback and emitted `A`, T1 held
    /// `A`'s lock waiting for `B`'s while T2 held `B`'s waiting for `A`'s \u2014 a lock-order cycle that
    /// never resolves. Locks are now **per slot** and none is held across the callback, so the two
    /// forwards run in a per-call order that cannot cycle.
    ///
    /// The test uses a three-way barrier so both callbacks are provably inside their first delivery
    /// before either forwards, and bounded channel receives so a regression fails as a timeout rather
    /// than hanging the suite.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn cross_signal_forwards_across_threads_do_not_deadlock() {
        use std::sync::mpsc;
        use std::sync::Barrier;
        use std::time::Duration;

        let a = Signal::<u32>::new();
        let b = Signal::<u32>::new();
        // Both threads announce they are inside their first callback, then proceed together.
        let both_inside = Arc::new(Barrier::new(2));

        // A's slot forwards payload 1 to B with payload 2 (only for payload 1, so the forward is not
        // an unbounded recursion: payload 2's delivery does not forward again).
        let b_for_a = b.clone();
        let barrier_a = Arc::clone(&both_inside);
        a.connect(move |value: Arc<u32>| {
            if *value == 1 {
                barrier_a.wait();
                b_for_a.emit(2);
            }
        });
        let a_for_b = a.clone();
        let barrier_b = Arc::clone(&both_inside);
        b.connect(move |value: Arc<u32>| {
            if *value == 1 {
                barrier_b.wait();
                a_for_b.emit(2);
            }
        });

        let (done_tx_1, done_rx_1) = mpsc::channel();
        let (done_tx_2, done_rx_2) = mpsc::channel();
        let a1 = a.clone();
        let b2 = b.clone();
        let t1 = std::thread::spawn(move || {
            a1.emit(1);
            let _ = done_tx_1.send(());
        });
        let t2 = std::thread::spawn(move || {
            b2.emit(1);
            let _ = done_tx_2.send(());
        });

        assert!(
            done_rx_1.recv_timeout(Duration::from_secs(5)).is_ok(),
            "thread 1 must complete: a mutual cross-signal forward must not dead-lock"
        );
        assert!(
            done_rx_2.recv_timeout(Duration::from_secs(5)).is_ok(),
            "thread 2 must complete: a mutual cross-signal forward must not dead-lock"
        );
        t1.join().expect("thread 1 must not panic");
        t2.join().expect("thread 2 must not panic");
    }
}
