use crate::compat::HashMap;
use crate::compat::{Mutex, RwLock};
use alloc::sync::Arc;
use core::sync::atomic::{AtomicU64, Ordering};

/// Slot execution priority. Higher-priority slots fire before lower-priority ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Priority {
    /// High priority — fired first.
    High,
    /// Normal priority — default.
    #[default]
    Normal,
    /// Low priority — fired last.
    Low,
}

impl Priority {
    fn rank(&self) -> u8 {
        match self {
            Priority::High => 0,
            Priority::Normal => 1,
            Priority::Low => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
/// Opaque connection handle used to disconnect a slot.
pub struct ConnectionHandle(pub u64);

static NEXT_HANDLE: AtomicU64 = AtomicU64::new(1);

type SlotFn<T> = Box<dyn FnMut(Arc<T>) + Send + Sync + 'static>;

struct SlotEntry<T: Clone + Send + 'static> {
    callback: Option<SlotFn<T>>,
    once: bool,
    blocked: bool,
    priority: Priority,
}

struct SignalInner<T: Clone + Send + 'static> {
    slots: RwLock<HashMap<ConnectionHandle, SlotEntry<T>>>,
}

impl<T: Clone + Send + 'static> SignalInner<T> {
    fn disconnect(&self, handle: ConnectionHandle) -> bool {
        self.slots.write().expect("signal lock poisoned").remove(&handle).is_some()
    }

    fn block(&self, handle: ConnectionHandle) -> bool {
        if let Some(entry) = self.slots.write().expect("signal lock poisoned").get_mut(&handle) {
            entry.blocked = true;
            true
        } else {
            false
        }
    }

    fn unblock(&self, handle: ConnectionHandle) -> bool {
        if let Some(entry) = self.slots.write().expect("signal lock poisoned").get_mut(&handle) {
            entry.blocked = false;
            true
        } else {
            false
        }
    }

    fn is_blocked(&self, handle: ConnectionHandle) -> Option<bool> {
        self.slots.read().expect("signal lock poisoned").get(&handle).map(|entry| entry.blocked)
    }

    fn set_priority(&self, handle: ConnectionHandle, priority: Priority) -> bool {
        if let Some(entry) = self.slots.write().expect("signal lock poisoned").get_mut(&handle) {
            entry.priority = priority;
            true
        } else {
            false
        }
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

    /// Manually clear all tracked connections without dropping the scope.
    pub fn clear(&self) {
        let mut disconnectors = self.disconnectors.lock().unwrap_or_else(|e| e.into_inner());
        while let Some(disconnector) = disconnectors.pop() {
            disconnector();
        }
    }

    /// Returns the number of connections currently tracked by this scope.
    pub fn disconnect_count(&self) -> usize {
        self.disconnectors.lock().unwrap_or_else(|e| e.into_inner()).len()
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
        self.connect_with_priority(slot, Priority::Normal)
    }

    /// Connect a slot with a specific priority.
    /// High-priority slots fire before Normal, Normal before Low.
    pub fn connect_with_priority<F>(&self, slot: F, priority: Priority) -> ConnectionHandle
    where
        F: FnMut(Arc<T>) + Send + Sync + 'static,
    {
        let handle = ConnectionHandle(NEXT_HANDLE.fetch_add(1, Ordering::Relaxed));
        self.inner.slots.write().expect("signal lock poisoned").insert(
            handle,
            SlotEntry { callback: Some(Box::new(slot)), once: false, blocked: false, priority },
        );
        handle
    }

    /// Connect a slot that is invoked once and then disconnected automatically.
    pub fn connect_once<F>(&self, slot: F) -> ConnectionHandle
    where
        F: FnMut(Arc<T>) + Send + Sync + 'static,
    {
        let handle = ConnectionHandle(NEXT_HANDLE.fetch_add(1, Ordering::Relaxed));
        self.inner.slots.write().expect("signal lock poisoned").insert(
            handle,
            SlotEntry {
                callback: Some(Box::new(slot)),
                once: true,
                blocked: false,
                priority: Priority::Normal,
            },
        );
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

    /// Disconnect slot by handle. Returns true if the handle was valid.
    pub fn disconnect(&self, handle: ConnectionHandle) -> bool {
        self.inner.disconnect(handle)
    }

    /// Disconnect all slots registered on this signal.
    pub fn disconnect_all(&self) {
        self.inner.slots.write().expect("signal lock poisoned").clear();
    }

    /// Temporarily block a slot without disconnecting it. Returns true if the handle was valid.
    pub fn block(&self, handle: ConnectionHandle) -> bool {
        self.inner.block(handle)
    }

    /// Unblock a previously blocked slot. Returns true if the handle was valid.
    pub fn unblock(&self, handle: ConnectionHandle) -> bool {
        self.inner.unblock(handle)
    }

    /// Returns `Some(true/false)` if the handle exists, `None` if invalid.
    pub fn is_blocked(&self, handle: ConnectionHandle) -> Option<bool> {
        self.inner.is_blocked(handle)
    }

    /// Returns `true` if the handle is still connected (valid).
    pub fn is_connected(&self, handle: ConnectionHandle) -> bool {
        self.inner.slots.read().expect("signal lock poisoned").contains_key(&handle)
    }

    /// Change the priority of an existing connection. Returns true if the handle was valid.
    pub fn set_priority(&self, handle: ConnectionHandle, priority: Priority) -> bool {
        self.inner.set_priority(handle, priority)
    }

    /// Emit a cloned value to all connected (non-blocked) slots.
    ///
    /// This method safely processes slots **one at a time** by temporarily
    /// taking each slot's callback (via `Option::take`) under a write lock,
    /// leaving the handle **in** the HashMap so that `disconnect(own_handle)`
    /// can find and remove it. The callback is invoked **outside** the lock,
    /// and if the handle still exists afterward (i.e., was not self-disconnected),
    /// the callback is restored. Once-slots are removed unconditionally after
    /// invocation. Callbacks may safely call `connect`, `disconnect`,
    /// `disconnect_all`, `block`, `unblock`, or `emit` on **the same Signal**
    /// without deadlocking. Self-disconnect from within a callback is now
    /// correctly honored and does not get undone by a stale re-insertion.
    ///
    /// Slots are invoked in priority order (High → Normal → Low).
    /// Blocked slots are skipped entirely.
    pub fn emit(&self, value: T) {
        let arc_value = Arc::new(value);

        // 1. Snapshot handles and priorities under a read lock.
        let snapshot: Vec<(ConnectionHandle, Priority)> = {
            let slots = self.inner.slots.read().expect("signal lock poisoned");
            slots.iter().map(|(h, e)| (*h, e.priority)).collect()
        };

        // 2. Sort by priority (High first).
        let mut snapshot = snapshot;
        snapshot.sort_by_key(|a| a.1.rank());

        // 3. Process each slot individually against the real HashMap.
        //    The callback is temporarily taken (via Option::take) under a write
        //    lock, leaving the handle in the map so that if the callback calls
        //    `disconnect(own_handle)`, the disconnect can find and remove the
        //    handle. After invocation, if the handle still exists in the map
        //    (i.e., was not self-disconnected), the callback is restored.
        //    Once-slots are removed unconditionally after invocation.
        for (handle, _priority) in snapshot {
            // Temporarily take the callback under a write lock, leaving None.
            // The handle stays in the HashMap so disconnect() can find it.
            let taken = {
                let mut slots = self.inner.slots.write().expect("signal lock poisoned");
                if let Some(entry) = slots.get_mut(&handle) {
                    if entry.blocked {
                        None
                    } else {
                        entry.callback.take()
                    }
                } else {
                    // Handle was disconnected by a prior callback in this emit loop.
                    None
                }
            };

            if let Some(mut callback) = taken {
                callback(arc_value.clone());

                // After callback: if it was a once-slot, remove the entry.
                // Otherwise, re-install the callback only if the handle still
                // exists (i.e., the callback did not call disconnect on itself).
                let mut slots = self.inner.slots.write().expect("signal lock poisoned");
                if let Some(entry) = slots.get_mut(&handle) {
                    if entry.once {
                        // Once-slot: remove the entry entirely.
                        slots.remove(&handle);
                    } else {
                        // Non-once, still connected: restore the callback.
                        entry.callback = Some(callback);
                    }
                }
                // If handle was removed by self-disconnect, callback is dropped.
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
