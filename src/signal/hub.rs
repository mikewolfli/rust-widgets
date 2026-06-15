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
    pub fn emit(&self, name: &str) {
        if let Some(signal) = self.signals.lock().unwrap_or_else(|e| e.into_inner()).get(name) {
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

impl Default for CustomSignalHub {
    fn default() -> Self {
        Self::new()
    }
}
