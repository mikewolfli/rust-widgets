//! Signal and slot implementation.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// Opaque connection handle used to disconnect a slot.
pub struct ConnectionHandle(pub u64);

static NEXT_HANDLE: AtomicU64 = AtomicU64::new(1);

type SlotFn<T> = Box<dyn FnMut(T) + Send + 'static>;

struct SlotEntry<T: Clone + Send + 'static> {
    callback: SlotFn<T>,
    once: bool,
}

struct SignalInner<T: Clone + Send + 'static> {
    slots: Mutex<HashMap<ConnectionHandle, SlotEntry<T>>>,
}

impl<T: Clone + Send + 'static> SignalInner<T> {
    fn disconnect(&self, handle: ConnectionHandle) -> bool {
        self.slots
            .lock()
            .expect("signal lock poisoned")
            .remove(&handle)
            .is_some()
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

    fn track(&self, disconnector: Box<dyn FnOnce() + Send + 'static>) {
        self.disconnectors
            .lock()
            .expect("connection scope lock poisoned")
            .push(disconnector);
    }
}

impl Drop for ConnectionScope {
    fn drop(&mut self) {
        let mut disconnectors = self
            .disconnectors
            .lock()
            .expect("connection scope lock poisoned");
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
        Self {
            inner: Arc::new(SignalInner {
                slots: Mutex::new(HashMap::new()),
            }),
        }
    }

    /// Connect a slot and return its connection handle.
    pub fn connect<F>(&self, slot: F) -> ConnectionHandle
    where
        F: FnMut(T) + Send + 'static,
    {
        let handle = ConnectionHandle(NEXT_HANDLE.fetch_add(1, Ordering::Relaxed));
        self.inner.slots.lock().expect("signal lock poisoned").insert(
            handle,
            SlotEntry {
                callback: Box::new(slot),
                once: false,
            },
        );
        handle
    }

    /// Connect a slot that is invoked once and then disconnected automatically.
    pub fn connect_once<F>(&self, slot: F) -> ConnectionHandle
    where
        F: FnMut(T) + Send + 'static,
    {
        let handle = ConnectionHandle(NEXT_HANDLE.fetch_add(1, Ordering::Relaxed));
        self.inner.slots.lock().expect("signal lock poisoned").insert(
            handle,
            SlotEntry {
                callback: Box::new(slot),
                once: true,
            },
        );
        handle
    }

    /// Connect a slot bound to a connection scope. It disconnects when the scope is dropped.
    pub fn connect_scoped<F>(&self, owner: &ConnectionScope, slot: F) -> ConnectionHandle
    where
        F: FnMut(T) + Send + 'static,
    {
        let handle = self.connect(slot);
        self.track_owner(owner, handle);
        handle
    }

    /// Connect a once-slot bound to a connection scope.
    pub fn connect_once_scoped<F>(&self, owner: &ConnectionScope, slot: F) -> ConnectionHandle
    where
        F: FnMut(T) + Send + 'static,
    {
        let handle = self.connect_once(slot);
        self.track_owner(owner, handle);
        handle
    }

    /// Disconnect slot by handle.
    pub fn disconnect(&self, handle: ConnectionHandle) -> bool {
        self.inner.disconnect(handle)
    }

    /// Disconnect all slots registered on this signal.
    pub fn disconnect_all(&self) {
        self.inner.slots.lock().expect("signal lock poisoned").clear();
    }

    /// Emit a cloned value to all connected slots.
    pub fn emit(&self, value: T) {
        let mut slots = self.inner.slots.lock().expect("signal lock poisoned");
        let mut once_handles = Vec::new();
        for (handle, slot) in slots.iter_mut() {
            (slot.callback)(value.clone());
            if slot.once {
                once_handles.push(*handle);
            }
        }
        for handle in once_handles {
            let _ = slots.remove(&handle);
        }
    }

    /// Return number of currently connected slots.
    pub fn slot_count(&self) -> usize {
        self.inner.slots.lock().expect("signal lock poisoned").len()
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

/// Zero-argument compatibility signal type.
#[derive(Clone, Default)]
pub struct GenericSignal {
    inner: Signal<()>,
}

impl GenericSignal {
    /// Create empty zero-argument signal.
    pub fn new() -> Self {
        Self { inner: Signal::new() }
    }

    /// Connect zero-argument slot and return connection handle.
    pub fn connect<F>(&self, mut slot: F) -> ConnectionHandle
    where
        F: FnMut() + Send + 'static,
    {
        self.inner.connect(move |_| slot())
    }

    /// Connect zero-argument once-slot and return connection handle.
    pub fn connect_once<F>(&self, mut slot: F) -> ConnectionHandle
    where
        F: FnMut() + Send + 'static,
    {
        self.inner.connect_once(move |_| slot())
    }

    /// Connect zero-argument slot bound to an owner scope.
    pub fn connect_scoped<F>(&self, owner: &ConnectionScope, mut slot: F) -> ConnectionHandle
    where
        F: FnMut() + Send + 'static,
    {
        self.inner.connect_scoped(owner, move |_| slot())
    }

    /// Connect zero-argument once-slot bound to an owner scope.
    pub fn connect_once_scoped<F>(&self, owner: &ConnectionScope, mut slot: F) -> ConnectionHandle
    where
        F: FnMut() + Send + 'static,
    {
        self.inner.connect_once_scoped(owner, move |_| slot())
    }

    /// Disconnect slot by handle.
    pub fn disconnect(&self, handle: ConnectionHandle) -> bool {
        self.inner.disconnect(handle)
    }

    /// Disconnect all slots registered on this signal.
    pub fn disconnect_all(&self) {
        self.inner.disconnect_all();
    }

    /// Emit signal to all currently connected slots.
    pub fn emit(&self) {
        eprintln!("[GenericSignal] emit: slot_count = {}", self.slot_count());
        self.inner.emit(())
    }

    /// Return number of currently connected slots.
    pub fn slot_count(&self) -> usize {
        self.inner.slot_count()
    }
}

/// Backward-compatible single-argument signal alias.
pub type Signal1<T> = Signal<T>;

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

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    use super::{ConnectionScope, GenericSignal, Signal};

    #[test]
    fn signal_emits_to_multiple_slots() {
        let signal = Signal::<u32>::new();
        let sum = Arc::new(AtomicUsize::new(0));

        let sum_a = Arc::clone(&sum);
        signal.connect(move |value| {
            sum_a.fetch_add(value as usize, Ordering::SeqCst);
        });

        let sum_b = Arc::clone(&sum);
        signal.connect(move |value| {
            sum_b.fetch_add((value as usize) * 2, Ordering::SeqCst);
        });

        signal.emit(3);
        assert_eq!(sum.load(Ordering::SeqCst), 9);
    }

    #[test]
    fn signal_once_disconnects_after_first_emit() {
        let signal = GenericSignal::new();
        let hits = Arc::new(AtomicUsize::new(0));
        let hits_ref = Arc::clone(&hits);

        signal.connect_once(move || {
            hits_ref.fetch_add(1, Ordering::SeqCst);
        });

        signal.emit();
        signal.emit();

        assert_eq!(hits.load(Ordering::SeqCst), 1);
        assert_eq!(signal.slot_count(), 0);
    }

    #[test]
    fn scoped_connection_disconnects_on_owner_drop() {
        let signal = GenericSignal::new();
        let hits = Arc::new(AtomicUsize::new(0));

        {
            let owner = ConnectionScope::new();
            let hits_ref = Arc::clone(&hits);
            signal.connect_scoped(&owner, move || {
                hits_ref.fetch_add(1, Ordering::SeqCst);
            });
            signal.emit();
        }

        signal.emit();

        assert_eq!(hits.load(Ordering::SeqCst), 1);
        assert_eq!(signal.slot_count(), 0);
    }
}
