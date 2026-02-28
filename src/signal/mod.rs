//! Signal and slot implementation.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConnectionHandle(pub u64);

static NEXT_HANDLE: AtomicU64 = AtomicU64::new(1);

type Slot0 = Box<dyn FnMut() + Send + 'static>;
type Slot1<T> = Box<dyn FnMut(T) + Send + 'static>;

pub trait Signal {
    fn disconnect_all(&self);
}

#[derive(Clone)]
pub struct GenericSignal {
    slots: Arc<Mutex<HashMap<ConnectionHandle, Slot0>>>,
}

impl GenericSignal {
    pub fn new() -> Self {
        Self {
            slots: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn connect<F>(&self, slot: F) -> ConnectionHandle
    where
        F: FnMut() + Send + 'static,
    {
        let handle = ConnectionHandle(NEXT_HANDLE.fetch_add(1, Ordering::Relaxed));
        self.slots.lock().expect("signal lock poisoned").insert(handle, Box::new(slot));
        handle
    }

    pub fn disconnect(&self, handle: ConnectionHandle) -> bool {
        self.slots.lock().expect("signal lock poisoned").remove(&handle).is_some()
    }

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

#[derive(Clone)]
pub struct Signal1<T: Clone + Send + 'static> {
    slots: Arc<Mutex<HashMap<ConnectionHandle, Slot1<T>>>>,
}

impl<T: Clone + Send + 'static> Signal1<T> {
    pub fn new() -> Self {
        Self {
            slots: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn connect<F>(&self, slot: F) -> ConnectionHandle
    where
        F: FnMut(T) + Send + 'static,
    {
        let handle = ConnectionHandle(NEXT_HANDLE.fetch_add(1, Ordering::Relaxed));
        self.slots.lock().expect("signal lock poisoned").insert(handle, Box::new(slot));
        handle
    }

    pub fn disconnect(&self, handle: ConnectionHandle) -> bool {
        self.slots.lock().expect("signal lock poisoned").remove(&handle).is_some()
    }

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

#[derive(Default)]
pub struct CustomSignalHub {
    signals: HashMap<String, GenericSignal>,
}

impl CustomSignalHub {
    pub fn define(&mut self, name: impl Into<String>) {
        self.signals.entry(name.into()).or_default();
    }

    pub fn emit(&self, name: &str) {
        if let Some(signal) = self.signals.get(name) {
            signal.emit();
        }
    }

    pub fn connect<F>(&mut self, name: impl Into<String>, slot: F) -> ConnectionHandle
    where
        F: FnMut() + Send + 'static,
    {
        self.signals.entry(name.into()).or_default().connect(slot)
    }
}
