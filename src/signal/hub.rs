// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use super::{ConnectionHandle, ConnectionScope, GenericSignal, Priority};
use crate::compat::HashMap;
use crate::compat::Mutex;

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
        self.signals.lock().unwrap_or_else(|e| e.into_inner()).entry(name.into()).or_default();
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
            let signals = self.signals.lock().unwrap_or_else(|e| e.into_inner());
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
        self.signals
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(name.into())
            .or_default()
            .connect(slot)
    }

    /// Disconnect a specific handle from a named signal.
    /// Returns true if the handle was valid and disconnected.
    pub fn disconnect(&self, name: &str, handle: ConnectionHandle) -> bool {
        let signals = self.signals.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(signal) = signals.get(name) {
            signal.disconnect(handle)
        } else {
            false
        }
    }

    /// Disconnect all slots from a named signal.
    pub fn disconnect_all(&self, name: &str) {
        if let Some(signal) = self.signals.lock().unwrap_or_else(|e| e.into_inner()).get(name) {
            signal.disconnect_all();
        }
    }

    /// Remove a named signal entirely, disconnecting all its slots.
    pub fn remove(&self, name: &str) -> bool {
        self.signals.lock().unwrap_or_else(|e| e.into_inner()).remove(name).is_some()
    }

    /// Returns true if a named signal exists in the hub.
    pub fn contains(&self, name: &str) -> bool {
        self.signals.lock().unwrap_or_else(|e| e.into_inner()).contains_key(name)
    }

    /// Returns the number of named signals defined.
    pub fn signal_count(&self) -> usize {
        self.signals.lock().unwrap_or_else(|e| e.into_inner()).len()
    }

    /// Returns true if the hub has no named signals.
    pub fn is_empty(&self) -> bool {
        self.signals.lock().unwrap_or_else(|e| e.into_inner()).is_empty()
    }

    /// Remove all named signals and their slots.
    pub fn clear(&self) {
        self.signals.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }

    /// Temporarily block a slot on a named signal without disconnecting it.
    /// Returns true if the handle was valid.
    pub fn block(&self, name: &str, handle: ConnectionHandle) -> bool {
        self.signals
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(name)
            .map(|s| s.block(handle))
            .unwrap_or(false)
    }

    /// Unblock a previously blocked slot on a named signal.
    /// Returns true if the handle was valid.
    pub fn unblock(&self, name: &str, handle: ConnectionHandle) -> bool {
        self.signals
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(name)
            .map(|s| s.unblock(handle))
            .unwrap_or(false)
    }

    /// Returns `Some(true/false)` if the handle exists on the named signal,
    /// `None` if the signal or handle is invalid.
    pub fn is_blocked(&self, name: &str, handle: ConnectionHandle) -> Option<bool> {
        self.signals
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(name)
            .and_then(|s| s.is_blocked(handle))
    }

    /// Change the priority of an existing connection on a named signal.
    /// Returns true if the handle was valid.
    pub fn set_priority(&self, name: &str, handle: ConnectionHandle, priority: Priority) -> bool {
        self.signals
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(name)
            .map(|s| s.set_priority(handle, priority))
            .unwrap_or(false)
    }

    /// Connect a once-slot to a named signal, creating it when missing.
    /// The slot fires once and is then disconnected automatically.
    pub fn connect_once<F>(&self, name: impl Into<String>, slot: F) -> ConnectionHandle
    where
        F: FnMut() + Send + Sync + 'static,
    {
        self.signals
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(name.into())
            .or_default()
            .connect_once(slot)
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
        self.signals
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(name.into())
            .or_default()
            .connect_with_priority(slot, priority)
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
        self.signals
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(name.into())
            .or_default()
            .connect_scoped(owner, slot)
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
        self.signals
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(name.into())
            .or_default()
            .connect_once_scoped(owner, slot)
    }

    /// Return the number of slots connected to a named signal.
    /// Returns 0 if the signal does not exist.
    pub fn slot_count(&self, name: &str) -> usize {
        self.signals
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(name)
            .map(|s| s.slot_count())
            .unwrap_or(0)
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
}
