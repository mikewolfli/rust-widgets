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
}
