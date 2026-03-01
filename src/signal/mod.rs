//! Signal and slot implementation.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// Opaque connection handle used to disconnect a slot.
pub struct ConnectionHandle(pub u64);

static NEXT_HANDLE: AtomicU64 = AtomicU64::new(1);

type Slot0 = Box<dyn FnMut() + Send + 'static>;
type Slot1<T> = Box<dyn FnMut(T) + Send + 'static>;

/// Common signal behavior for signal implementations.
pub trait Signal {
    /// Disconnect all slots registered on this signal.
    fn disconnect_all(&self);
}

/// Zero-argument signal type.
#[derive(Clone)]
pub struct GenericSignal {
    slots: Arc<Mutex<HashMap<ConnectionHandle, Slot0>>>,
}

impl GenericSignal {
    /// Create empty zero-argument signal.
    pub fn new() -> Self {
        Self {
            slots: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Connect zero-argument slot and return connection handle.
    pub fn connect<F>(&self, slot: F) -> ConnectionHandle
    where
        F: FnMut() + Send + 'static,
    {
        let handle = ConnectionHandle(NEXT_HANDLE.fetch_add(1, Ordering::Relaxed));
        self.slots.lock().expect("signal lock poisoned").insert(handle, Box::new(slot));
        handle
    }

    /// Disconnect slot by handle.
    pub fn disconnect(&self, handle: ConnectionHandle) -> bool {
        self.slots.lock().expect("signal lock poisoned").remove(&handle).is_some()
    }

    /// Emit signal to all currently connected slots.
    pub fn emit(&self) {
        let mut slots = self.slots.lock().expect("signal lock poisoned");
        for slot in slots.values_mut() {
            slot();
        }
    }
}

impl Default for GenericSignal {
    fn default() -> Self {
        Self::new()
    }
}

impl Signal for GenericSignal {
    fn disconnect_all(&self) {
        self.slots.lock().expect("signal lock poisoned").clear();
    }
}

/// Single-argument signal type.
#[derive(Clone)]
pub struct Signal1<T: Clone + Send + 'static> {
    slots: Arc<Mutex<HashMap<ConnectionHandle, Slot1<T>>>>,
}

impl<T: Clone + Send + 'static> Signal1<T> {
    /// Create empty single-argument signal.
    pub fn new() -> Self {
        Self {
            slots: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Connect single-argument slot and return connection handle.
    pub fn connect<F>(&self, slot: F) -> ConnectionHandle
    where
        F: FnMut(T) + Send + 'static,
    {
        let handle = ConnectionHandle(NEXT_HANDLE.fetch_add(1, Ordering::Relaxed));
        self.slots.lock().expect("signal lock poisoned").insert(handle, Box::new(slot));
        handle
    }

    /// Disconnect slot by handle.
    pub fn disconnect(&self, handle: ConnectionHandle) -> bool {
        self.slots.lock().expect("signal lock poisoned").remove(&handle).is_some()
    }

    /// Emit a cloned value to all connected slots.
    pub fn emit(&self, value: T) {
        let mut slots = self.slots.lock().expect("signal lock poisoned");
        for slot in slots.values_mut() {
            slot(value.clone());
        }
    }
}

impl<T: Clone + Send + 'static> Default for Signal1<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone + Send + 'static> Signal for Signal1<T> {
    fn disconnect_all(&self) {
        self.slots.lock().expect("signal lock poisoned").clear();
    }
}

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
        F: FnMut() + Send + 'static,
    {
        self.signals.entry(name.into()).or_default().connect(slot)
    }
}
