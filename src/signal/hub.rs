use super::{ConnectionHandle, GenericSignal};
use std::collections::HashMap;
/// Registry of dynamically named zero-argument signals.
#[derive(Default)]
pub struct CustomSignalHub {
    /// Named signal registry for dynamic/custom event channels.
    signals: HashMap<String, GenericSignal>,
}
impl CustomSignalHub {
    /// Defines a named signal if it does not already exist.
    pub fn define(&mut self, name: impl Into<String>) {
        self.signals.entry(name.into()).or_default();
    }
    /// Emits a named signal when present.
    pub fn emit(&self, name: &str) {
        if let Some(signal) = self.signals.get(name) {
            signal.emit();
        }
    }
    /// Connects a slot to a named signal, creating it when missing.
    pub fn connect<F>(&mut self, name: impl Into<String>, slot: F) -> ConnectionHandle
    where
        F: FnMut() + Send + Sync + 'static,
    {
        self.signals.entry(name.into()).or_default().connect(slot)
    }
    /// Disconnect all slots from a named signal.
    pub fn disconnect_all(&mut self, name: &str) {
        if let Some(signal) = self.signals.get(name) {
            signal.disconnect_all();
        }
    }
    /// Remove a named signal entirely, disconnecting all its slots.
    pub fn remove(&mut self, name: &str) -> bool {
        self.signals.remove(name).is_some()
    }
    /// Returns true if a named signal exists in the hub.
    pub fn contains(&self, name: &str) -> bool {
        self.signals.contains_key(name)
    }
    /// Returns the number of named signals defined.
    pub fn signal_count(&self) -> usize {
        self.signals.len()
    }
    /// Returns true if the hub has no named signals.
    pub fn is_empty(&self) -> bool {
        self.signals.is_empty()
    }
    /// Remove all named signals and their slots.
    pub fn clear(&mut self) {
        self.signals.clear();
    }
}
