use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// Opaque connection handle used to disconnect a slot.
pub struct ConnectionHandle(pub u64);
static NEXT_HANDLE: AtomicU64 = AtomicU64::new(1);
type SlotFn<T> = Box<dyn FnMut(Arc<T>) + Send + Sync + 'static>;
struct SlotEntry<T: Clone + Send + 'static> {
    callback: SlotFn<T>,
    once: bool,
}
struct SignalInner<T: Clone + Send + 'static> {
    slots: RwLock<HashMap<ConnectionHandle, SlotEntry<T>>>,
}
impl<T: Clone + Send + 'static> SignalInner<T> {
    fn disconnect(&self, handle: ConnectionHandle) -> bool {
        self.slots.write().expect("signal lock poisoned").remove(&handle).is_some()
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
        self.disconnectors.lock().unwrap_or_else(|e| e.into_inner()).push(disconnector);
    }
}
impl Drop for ConnectionScope {
    fn drop(&mut self) {
        let mut disconnectors = self.disconnectors.lock().unwrap_or_else(|e| e.into_inner());
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
        Self { inner: Arc::new(SignalInner { slots: RwLock::new(HashMap::new()) }) }
    }
    /// Connect a slot and return its connection handle.
    pub fn connect<F>(&self, slot: F) -> ConnectionHandle
    where
        F: FnMut(Arc<T>) + Send + Sync + 'static,
    {
        let handle = ConnectionHandle(NEXT_HANDLE.fetch_add(1, Ordering::Relaxed));
        self.inner
            .slots
            .write()
            .expect("signal lock poisoned")
            .insert(handle, SlotEntry { callback: Box::new(slot), once: false });
        handle
    }
    /// Connect a slot that is invoked once and then disconnected automatically.
    pub fn connect_once<F>(&self, slot: F) -> ConnectionHandle
    where
        F: FnMut(Arc<T>) + Send + Sync + 'static,
    {
        let handle = ConnectionHandle(NEXT_HANDLE.fetch_add(1, Ordering::Relaxed));
        self.inner
            .slots
            .write()
            .expect("signal lock poisoned")
            .insert(handle, SlotEntry { callback: Box::new(slot), once: true });
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
    /// Disconnect slot by handle.
    pub fn disconnect(&self, handle: ConnectionHandle) -> bool {
        self.inner.disconnect(handle)
    }
    /// Disconnect all slots registered on this signal.
    pub fn disconnect_all(&self) {
        self.inner.slots.write().expect("signal lock poisoned").clear();
    }
    /// Emit a cloned value to all connected slots.
    ///
    /// This method **avoids** deadlocks by draining all slot entries under
    /// a single write lock, invoking every callback **outside** the lock,
    /// and then re-inserting the non-`once` slots afterwards.  This means
    /// callbacks may safely call `connect`, `disconnect`, `disconnect_all`,
    /// or `emit` on **the same Signal** without deadlocking.
    pub fn emit(&self, value: T) {
        let arc_value = Arc::new(value);

        // 1. Drain all entries under the write lock.
        let entries: Vec<(ConnectionHandle, SlotEntry<T>)> = {
            let mut slots = self.inner.slots.write().expect("signal lock poisoned");
            slots.drain().collect()
        };

        // 2. Invoke callbacks outside the lock — safe for re-entrant signals.
        let mut to_reinsert = Vec::new();
        for (handle, mut entry) in entries {
            (entry.callback)(arc_value.clone());
            if !entry.once {
                to_reinsert.push((handle, entry));
            }
        }

        // 3. Re-insert non-once slots under a fresh write lock.
        if !to_reinsert.is_empty() {
            let mut slots = self.inner.slots.write().expect("signal lock poisoned");
            for (handle, entry) in to_reinsert {
                slots.insert(handle, entry);
            }
        }
    }
    /// Return number of currently connected slots.
    pub fn slot_count(&self) -> usize {
        self.inner.slots.read().expect("signal lock poisoned").len()
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
