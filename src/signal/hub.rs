// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use super::{ConnectionHandle, ConnectionScope, GenericSignal, Priority};
use crate::compat::{lock, HashMap, Mutex, String};

/// Registry of dynamically named zero-argument signals.
///
/// All methods take `&self` because the internal state is guarded by a `Mutex`.
/// This is consistent with `Signal`, `GenericSignal`, and `Signal1` which all
/// use `&self` for mutation.
///
/// **Note:** Read-only queries (`contains`, `signal_count`, `is_empty`) acquire
/// the same exclusive `Mutex` lock as mutating methods. If lock contention becomes
/// a bottleneck, consider switching the internal storage to an `RwLock`.
pub struct CustomSignalHub {
    signals: Mutex<HashMap<String, GenericSignal>>,
}

impl CustomSignalHub {
    /// Create an empty hub.
    pub fn new() -> Self {
        Self { signals: Mutex::new(HashMap::new()) }
    }

    /// Defines a named signal if it does not already exist.
    pub fn define(&self, name: impl Into<String>) {
        lock(&self.signals).entry(name.into()).or_default();
    }

    /// Emits a named signal when present.
    ///
    /// # Why the signal is cloned out of the map before emitting
    ///
    /// The hub's `Mutex` is not re-entrant. A slot is allowed to call back into the
    /// hub — most naturally to emit another (or the same) named signal — so holding
    /// the map lock across `signal.emit()` deadlocks: the nested call blocks on the
    /// lock this frame still holds, and nothing can release it.
    ///
    /// Cloning the `GenericSignal` (an `Arc` bump, not a deep copy) and dropping the
    /// guard before emitting keeps the lock held only for the lookup. The slot then
    /// runs with no hub lock held, exactly as `Signal::emit` runs its slots with no
    /// signal lock held.
    ///
    /// A missing name stays a no-op rather than an error, so an emit that races a
    /// `remove` from another thread simply does nothing.
    pub fn emit(&self, name: &str) {
        let signal = {
            let signals = lock(&self.signals);
            signals.get(name).cloned()
        };
        if let Some(signal) = signal {
            signal.emit();
        }
    }

    /// Connects a slot to a named signal, creating it when missing.
    pub fn connect<F>(&self, name: impl Into<String>, slot: F) -> ConnectionHandle
    where
        F: FnMut() + Send + Sync + 'static,
    {
        lock(&self.signals).entry(name.into()).or_default().connect(slot)
    }

    /// Disconnect a specific handle from a named signal.
    /// Returns true if the handle was valid and disconnected.
    pub fn disconnect(&self, name: &str, handle: ConnectionHandle) -> bool {
        // Clone the signal out and drop the hub lock **before** disconnecting. The
        // disconnected slot owns the user closure's captures, so running the
        // disconnect while the hub lock is held would drop a capture whose `Drop`
        // re-enters the hub (a documented thing a slot may do) and self-deadlock on
        // the hub's non-reentrant mutex. Cloning the `GenericSignal` is an `Arc`
        // bump, not a deep copy.
        let signal = {
            let signals = lock(&self.signals);
            signals.get(name).cloned()
        };
        signal.is_some_and(|signal| signal.disconnect(handle))
    }

    /// Disconnect all slots from a named signal.
    pub fn disconnect_all(&self, name: &str) {
        // Clone out, release the hub lock, then clear: the cleared slots own user
        // captures, and a capture that re-enters the hub while the map lock is held
        // would self-deadlock (see [`CustomSignalHub::disconnect`]).
        let signal = {
            let signals = lock(&self.signals);
            signals.get(name).cloned()
        };
        if let Some(signal) = signal {
            signal.disconnect_all();
        }
    }

    /// Remove a named signal entirely, disconnecting all its slots.
    pub fn remove(&self, name: &str) -> bool {
        // Remove under the lock, but drop the removed signal **after** releasing it.
        // A `GenericSignal` owns its `SignalInner`, hence its slots, hence the user
        // closures' captures; dropping it while the hub map lock is held would drop
        // those captures with the lock held, and a capture whose `Drop` re-enters the
        // hub would self-deadlock. `removed` is destroyed after the guard's scope ends.
        let removed = lock(&self.signals).remove(name);
        let present = removed.is_some();
        drop(removed);
        present
    }

    /// Returns true if a named signal exists in the hub.
    pub fn contains(&self, name: &str) -> bool {
        lock(&self.signals).contains_key(name)
    }

    /// Returns the number of named signals defined.
    pub fn signal_count(&self) -> usize {
        lock(&self.signals).len()
    }

    /// Returns true if the hub has no named signals.
    pub fn is_empty(&self) -> bool {
        lock(&self.signals).is_empty()
    }

    /// Remove all named signals and their slots.
    pub fn clear(&self) {
        // Take the map under the lock and drop it **after** releasing: the removed
        // signals own the user closures' captures, and a capture whose `Drop`
        // re-enters the hub would self-deadlock on the map lock if it were clear
        // while held. `drained` is destroyed with no lock held.
        let drained = core::mem::take(&mut *lock(&self.signals));
        drop(drained);
    }

    /// Temporarily block a slot on a named signal without disconnecting it.
    /// Returns true if the handle was valid.
    pub fn block(&self, name: &str, handle: ConnectionHandle) -> bool {
        lock(&self.signals).get(name).map(|s| s.block(handle)).unwrap_or(false)
    }

    /// Unblock a previously blocked slot on a named signal.
    /// Returns true if the handle was valid.
    pub fn unblock(&self, name: &str, handle: ConnectionHandle) -> bool {
        lock(&self.signals).get(name).map(|s| s.unblock(handle)).unwrap_or(false)
    }

    /// Returns `Some(true/false)` if the handle exists on the named signal,
    /// `None` if the signal or handle is invalid.
    pub fn is_blocked(&self, name: &str, handle: ConnectionHandle) -> Option<bool> {
        lock(&self.signals).get(name).and_then(|s| s.is_blocked(handle))
    }

    /// Change the priority of an existing connection on a named signal.
    /// Returns true if the handle was valid.
    pub fn set_priority(&self, name: &str, handle: ConnectionHandle, priority: Priority) -> bool {
        lock(&self.signals).get(name).map(|s| s.set_priority(handle, priority)).unwrap_or(false)
    }

    /// Connect a once-slot to a named signal, creating it when missing.
    /// The slot fires once and is then disconnected automatically.
    pub fn connect_once<F>(&self, name: impl Into<String>, slot: F) -> ConnectionHandle
    where
        F: FnMut() + Send + Sync + 'static,
    {
        lock(&self.signals).entry(name.into()).or_default().connect_once(slot)
    }

    /// Connect a slot to a named signal with a specific priority.
    pub fn connect_with_priority<F>(
        &self,
        name: impl Into<String>,
        slot: F,
        priority: Priority,
    ) -> ConnectionHandle
    where
        F: FnMut() + Send + Sync + 'static,
    {
        lock(&self.signals).entry(name.into()).or_default().connect_with_priority(slot, priority)
    }

    /// Connect a slot bound to an owner scope on a named signal.
    pub fn connect_scoped<F>(
        &self,
        name: impl Into<String>,
        owner: &ConnectionScope,
        slot: F,
    ) -> ConnectionHandle
    where
        F: FnMut() + Send + Sync + 'static,
    {
        lock(&self.signals).entry(name.into()).or_default().connect_scoped(owner, slot)
    }

    /// Connect a once-slot bound to an owner scope on a named signal.
    pub fn connect_once_scoped<F>(
        &self,
        name: impl Into<String>,
        owner: &ConnectionScope,
        slot: F,
    ) -> ConnectionHandle
    where
        F: FnMut() + Send + Sync + 'static,
    {
        lock(&self.signals).entry(name.into()).or_default().connect_once_scoped(owner, slot)
    }

    /// Return the number of slots connected to a named signal.
    /// Returns 0 if the signal does not exist.
    pub fn slot_count(&self, name: &str) -> usize {
        lock(&self.signals).get(name).map(|s| s.slot_count()).unwrap_or(0)
    }
}

crate::impl_default_via_new!(CustomSignalHub);

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::sync::Arc;
    use core::sync::atomic::{AtomicUsize, Ordering};

    /// The hub is a thin name→signal table, so it must inherit `Signal`'s
    /// re-entrancy rule: a slot that emits its own signal re-enters the hub, and
    /// the re-entrant pass must skip the slot still on the stack.
    ///
    /// This is worth a test of its own because the hub adds a second lock (the
    /// `signals` map) on top of the signal's own. If either lock were held across
    /// the callback, this would deadlock rather than merely mis-count.
    #[test]
    fn a_re_entrant_hub_emit_does_not_deadlock() {
        // Every hub method takes `&self`, so `Arc` is enough to share it with the
        // callback — the hub is deliberately not `Clone` (a clone would duplicate
        // the signal table rather than share it).
        let hub = Arc::new(CustomSignalHub::new());
        let calls = Arc::new(AtomicUsize::new(0));

        let hub_for_reentry = Arc::clone(&hub);
        let calls_outer = Arc::clone(&calls);
        hub.connect("changed", move || {
            calls_outer.fetch_add(1, Ordering::SeqCst);
            // Re-enter the hub for the same signal, once.
            if calls_outer.load(Ordering::SeqCst) == 1 {
                hub_for_reentry.emit("changed");
            }
        });

        hub.emit("changed");
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "the re-entrant emit must skip the slot already on the stack, so it runs once"
        );
    }

    /// An emit for a name with no registered signal must be a no-op, not a panic.
    #[test]
    fn emitting_an_unknown_name_is_a_no_op() {
        let hub = CustomSignalHub::new();
        hub.emit("nobody-listens");
        assert!(!hub.contains("nobody-listens"));
    }

    /// Removing a named signal must not drop the slots' captures under the hub lock.
    ///
    /// A slot's closure can capture an `Arc<ConnectionScope>` whose last reference,
    /// when dropped, runs a disconnector that calls back into the hub (`remove`/`clear`
    /// is the natural one). Dropping the `GenericSignal` inside
    /// `lock(&self.signals).remove(name)` freed that capture with the map lock held,
    /// so the re-entrant call deadlocked on the non-reentrant mutex. Bounded channel:
    /// a regression fails as a timeout rather than hanging the suite.
    #[cfg(all(test, not(alloc_frugal)))]
    #[test]
    fn remove_does_not_drop_a_reentrant_capture_under_the_lock() {
        use core::time::Duration;
        use std::sync::mpsc;

        let hub = Arc::new(CustomSignalHub::new());
        hub.define("changed");

        // A scope whose disconnector will call `hub.remove("changed")` — a re-entrant
        // path through the hub's own map lock. `ConnectionScope` runs its
        // disconnectors when its last reference drops.
        let hub_for_scope = Arc::clone(&hub);
        let scope = Arc::new(ConnectionScope::new());
        let _scoped = hub.connect_scoped("changed", &scope, move || {
            hub_for_scope.remove("changed");
        });
        // A second slot captures the last strong reference to `scope`.
        let captured = Arc::clone(&scope);
        hub.connect("changed", move || {
            let _ = &captured;
        });
        drop(scope);

        let (done_tx, done_rx) = mpsc::channel();
        let worker_hub = Arc::clone(&hub);
        let worker = std::thread::spawn(move || {
            let removed = worker_hub.remove("changed");
            let _ = done_tx.send(removed);
        });

        let removed = done_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("remove must complete without dropping the capture while holding the hub lock");
        assert!(removed, "the named signal existed, so remove must report true");
        worker.join().expect("the remove worker must not panic");
        assert!(!hub.contains("changed"));
    }

    /// `clear` must not drop the slots' captures under the hub lock either.
    #[cfg(all(test, not(alloc_frugal)))]
    #[test]
    fn clear_does_not_drop_a_reentrant_capture_under_the_lock() {
        use core::time::Duration;
        use std::sync::mpsc;

        let hub = Arc::new(CustomSignalHub::new());
        hub.define("changed");

        let hub_for_scope = Arc::clone(&hub);
        let scope = Arc::new(ConnectionScope::new());
        let _scoped = hub.connect_scoped("changed", &scope, move || {
            hub_for_scope.clear();
        });
        let captured = Arc::clone(&scope);
        hub.connect("changed", move || {
            let _ = &captured;
        });
        drop(scope);

        let (done_tx, done_rx) = mpsc::channel();
        let worker_hub = Arc::clone(&hub);
        let worker = std::thread::spawn(move || {
            worker_hub.clear();
            let _ = done_tx.send(());
        });

        assert!(
            done_rx.recv_timeout(Duration::from_secs(5)).is_ok(),
            "clear must complete without dropping captures while holding the hub lock"
        );
        worker.join().expect("the clear worker must not panic");
        assert!(hub.is_empty());
    }
}
