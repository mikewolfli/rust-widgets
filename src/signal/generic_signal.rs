use super::{ConnectionHandle, ConnectionScope, Signal};
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
        F: FnMut() + Send + Sync + 'static,
    {
        self.inner.connect(move |_| slot())
    }
    /// Connect zero-argument once-slot and return connection handle.
    pub fn connect_once<F>(&self, mut slot: F) -> ConnectionHandle
    where
        F: FnMut() + Send + Sync + 'static,
    {
        self.inner.connect_once(move |_| slot())
    }
    /// Connect zero-argument slot bound to an owner scope.
    pub fn connect_scoped<F>(&self, owner: &ConnectionScope, mut slot: F) -> ConnectionHandle
    where
        F: FnMut() + Send + Sync + 'static,
    {
        self.inner.connect_scoped(owner, move |_| slot())
    }
    /// Connect zero-argument once-slot bound to an owner scope.
    pub fn connect_once_scoped<F>(&self, owner: &ConnectionScope, mut slot: F) -> ConnectionHandle
    where
        F: FnMut() + Send + Sync + 'static,
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
        self.inner.emit(())
    }
    /// Return number of currently connected slots.
    pub fn slot_count(&self) -> usize {
        self.inner.slot_count()
    }
}
/// Backward-compatible single-argument signal alias.
pub type Signal1<T> = Signal<T>;
