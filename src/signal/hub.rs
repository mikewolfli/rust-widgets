use super::{ConnectionHandle, GenericSignal};
use crate::compat::HashMap;
use crate::compat::Mutex;

/// Registry of dynamically named zero-argument signals.
///
/// All methods take `&self` because the internal state is guarded by a `Mutex`.
/// This is consistent with `Signal`, `GenericSignal`, and `Signal1` which all
/// use `&self` for mutation.
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
}

impl Default for CustomSignalHub {
    fn default() -> Self {
        Self::new()
    }
}
