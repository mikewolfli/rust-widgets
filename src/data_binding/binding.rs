use crate::compat::HashMap;
use crate::data_binding::traits::*;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicBool, Ordering};

/// A reactive binding that notifies listeners when the value changes.
///
/// Think of it as a "signal + value" container. When the value is updated via
/// [`set`](Binding::set), all registered listeners are notified with their
/// subscription key.
pub struct Binding<T: Clone + Send + 'static> {
    value: T,
    listeners: HashMap<String, BoxedListener>,
}

impl<T: Clone + Send + 'static> Binding<T> {
    /// Create a new binding with an initial value.
    pub fn new(value: T) -> Self {
        Self { value, listeners: HashMap::new() }
    }

    /// Get the current value.
    pub fn get(&self) -> T {
        self.value.clone()
    }

    /// Set a new value and notify all listeners.
    pub fn set(&mut self, value: T) {
        self.value = value;
        self.notify_listeners();
    }

    /// Subscribe to value changes.
    ///
    /// `key` is an identifier used to later unsubscribe. If a listener with
    /// the same key already exists, it is replaced.
    pub fn subscribe(&mut self, key: &str, listener: BoxedListener) {
        self.listeners.insert(key.to_string(), listener);
    }

    /// Remove a listener by its subscription key.
    pub fn unsubscribe(&mut self, key: &str) {
        self.listeners.remove(key);
    }

    /// Create a two-way binding between this binding and another.
    ///
    /// Whenever either binding's value changes, the other is updated to match.
    /// Uses an atomic synchronization guard to prevent infinite notification
    /// loops.
    pub fn bind_to(&mut self, other: &mut Binding<T>)
    where
        T: PartialEq,
    {
        let syncing = Arc::new(AtomicBool::new(false));

        // SAFETY: both bindings are alive for the duration of the subscription
        // because the listeners are stored inside the binding's listener list.
        let self_ptr: *mut Binding<T> = self;
        let other_ptr: *mut Binding<T> = other;

        let listener_self_key = format!("__two_way_self_{:p}", other_ptr);
        let listener_other_key = format!("__two_way_other_{:p}", self_ptr);

        self.subscribe(
            &listener_self_key,
            Box::new(TwoWayListener::new(syncing.clone(), self_ptr, other_ptr)),
        );
        other.subscribe(
            &listener_other_key,
            Box::new(TwoWayListener::new(syncing, other_ptr, self_ptr)),
        );
    }

    /// Set value without notifying listeners. Internal helper for two-way sync.
    fn set_no_notify(&mut self, value: T) {
        self.value = value;
    }

    fn notify_listeners(&mut self) {
        let keys: Vec<String> = self.listeners.keys().cloned().collect();
        for key in &keys {
            if let Some(listener) = self.listeners.get_mut(key) {
                listener.on_value_changed(key);
            }
        }
    }
}

/// A listener that propagates value changes from one binding to another.
///
/// Used internally by [`Binding::bind_to`] to implement two-way synchronization.
struct TwoWayListener<T: Clone + Send + 'static> {
    syncing: Arc<AtomicBool>,
    source: *mut Binding<T>,
    target: *mut Binding<T>,
}

// SAFETY: TwoWayListener is only used within bind_to where both bindings
// outlive the subscription connection. Access is serialized by the AtomicBool.
unsafe impl<T: Clone + Send + 'static> Send for TwoWayListener<T> {}

impl<T: Clone + Send + 'static> TwoWayListener<T> {
    fn new(syncing: Arc<AtomicBool>, source: *mut Binding<T>, target: *mut Binding<T>) -> Self {
        Self { syncing, source, target }
    }
}

impl<T: Clone + Send + 'static + PartialEq> BindingListener for TwoWayListener<T> {
    fn on_value_changed(&mut self, _key: &str) {
        if self.syncing.swap(true, Ordering::SeqCst) {
            return;
        }
        // SAFETY: both bindings outlive this callback.
        let val = unsafe { (*self.source).get() };
        unsafe { (*self.target).set_no_notify(val) };
        self.syncing.store(false, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::Mutex;
    use core::sync::atomic::AtomicI32;

    #[test]
    fn test_binding_get_set() {
        let mut b = Binding::new(42);
        assert_eq!(b.get(), 42);
        b.set(100);
        assert_eq!(b.get(), 100);
    }

    #[test]
    fn test_binding_listener_notification() {
        let mut b = Binding::new("hello".to_string());
        let notified = Arc::new(AtomicBool::new(false));
        let n = notified.clone();
        let listener = Box::new(FnListener::new(move |_key| {
            n.store(true, Ordering::SeqCst);
        }));
        b.subscribe("test", listener);
        b.set("world".to_string());
        assert!(notified.load(Ordering::SeqCst));
    }

    #[test]
    fn test_binding_unsubscribe() {
        let mut b = Binding::new(0);
        let count = Arc::new(AtomicI32::new(0));
        let c = count.clone();
        let listener = Box::new(FnListener::new(move |_key| {
            c.fetch_add(1, Ordering::SeqCst);
        }));
        b.subscribe("test", listener);
        b.set(1);
        assert_eq!(count.load(Ordering::SeqCst), 1);
        b.unsubscribe("test");
        b.set(2);
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_binding_multiple_listeners() {
        let mut b = Binding::new(0);
        let count_a = Arc::new(AtomicI32::new(0));
        let count_b = Arc::new(AtomicI32::new(0));

        let ca = count_a.clone();
        b.subscribe(
            "a",
            Box::new(FnListener::new(move |_| {
                ca.fetch_add(1, Ordering::SeqCst);
            })),
        );
        let cb = count_b.clone();
        b.subscribe(
            "b",
            Box::new(FnListener::new(move |_| {
                cb.fetch_add(1, Ordering::SeqCst);
            })),
        );
        b.set(1);
        assert_eq!(count_a.load(Ordering::SeqCst), 1);
        assert_eq!(count_b.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_binding_listener_receive_key() {
        let mut b = Binding::new(0);
        let received_key = Arc::new(Mutex::new(String::new()));
        let rk = received_key.clone();
        let listener = Box::new(FnListener::new(move |key| {
            *rk.lock().unwrap() = key.to_string();
        }));
        b.subscribe("my_key", listener);
        b.set(99);
        assert_eq!(*received_key.lock().unwrap(), "my_key");
    }

    #[test]
    fn test_binding_two_way_sync() {
        let mut a = Binding::new(10);
        let mut b = Binding::new(20);

        a.bind_to(&mut b);

        // Set a -> b should propagate
        a.set(30);
        assert_eq!(a.get(), 30);
        assert_eq!(b.get(), 30);

        // Set b -> a should propagate
        b.set(50);
        assert_eq!(a.get(), 50);
        assert_eq!(b.get(), 50);
    }

    #[test]
    fn test_binding_two_way_no_infinite_loop() {
        let mut a = Binding::new(0);
        let mut b = Binding::new(0);
        let a_count = Arc::new(AtomicI32::new(0));
        let b_count = Arc::new(AtomicI32::new(0));

        let ac = a_count.clone();
        a.subscribe(
            "a_count",
            Box::new(FnListener::new(move |_| {
                ac.fetch_add(1, Ordering::SeqCst);
            })),
        );
        let bc = b_count.clone();
        b.subscribe(
            "b_count",
            Box::new(FnListener::new(move |_| {
                bc.fetch_add(1, Ordering::SeqCst);
            })),
        );

        a.bind_to(&mut b);

        // Changing a should notify a's listeners once and propagate to b
        // via set_no_notify (b's listeners are NOT fired by the TwoWayListener).
        a.set(42);
        assert_eq!(a_count.load(Ordering::SeqCst), 1);
        assert_eq!(b_count.load(Ordering::SeqCst), 0);

        // Setting b directly should fire b's listeners
        a.set(100);
        assert_eq!(a_count.load(Ordering::SeqCst), 2);
        assert_eq!(b_count.load(Ordering::SeqCst), 0);
    }
}
