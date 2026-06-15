use crate::compat::HashMap;
use crate::compat::Mutex;
use crate::data_binding::traits::*;
use alloc::sync::{Arc, Weak};
use core::sync::atomic::{AtomicBool, Ordering};

/// A reactive binding that notifies listeners when the value changes.
///
/// Think of it as a "signal + value" container. When the value is updated via
/// [`set`](Binding::set), all registered listeners are notified with their
/// subscription key.
///
/// Internally the binding's state is guarded by a `Mutex`, so all mutation
/// methods take `&self` rather than `&mut self`.  This makes it possible to
/// safely share a `Binding` across threads or use it with shared references.
pub struct Binding<T: Clone + Send + 'static> {
    inner: Arc<Mutex<BindingInner<T>>>,
}

struct BindingInner<T: Clone + Send + 'static> {
    value: T,
    listeners: HashMap<String, BoxedListener>,
}

impl<T: Clone + Send + 'static> Binding<T> {
    /// Create a new binding with an initial value.
    pub fn new(value: T) -> Self {
        Self { inner: Arc::new(Mutex::new(BindingInner { value, listeners: HashMap::new() })) }
    }

    /// Get the current value.
    pub fn get(&self) -> T {
        self.inner.lock().unwrap_or_else(|e| e.into_inner()).value.clone()
    }

    /// Set a new value and notify all listeners.
    ///
    /// Notifications are dispatched **outside** the Mutex lock to prevent
    /// re-entrancy deadlocks (e.g. when a TwoWayListener tries to lock the
    /// same binding's Mutex while propagating a value change).
    ///
    /// Listeners are temporarily removed from the map, notified, then
    /// restored (unless a new listener was subscribed under the same key
    /// during notification, in which case the new one takes precedence).
    pub fn set(&self, value: T) {
        // ── Phase 1: Lock, update value, take all listeners ──
        let mut listeners: Vec<(String, BoxedListener)>;
        {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            inner.value = value;
            listeners = inner.listeners.drain().collect();
        } // Mutex lock released.

        // ── Phase 2: Notify outside lock (safe from re-entrancy) ──
        for (key, ref mut listener) in &mut listeners {
            listener.on_value_changed(key, "set");
        }

        // ── Phase 3: Restore listeners that weren't re-subscribed ──
        {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            for (key, listener) in listeners {
                // If no new listener was subscribed under this key
                // during notification, put the original one back.
                inner.listeners.entry(key).or_insert(listener);
            }
        }
    }

    /// Subscribe to value changes.
    ///
    /// `key` is an identifier used to later unsubscribe. If a listener with
    /// the same key already exists, it is replaced.
    pub fn subscribe(&self, key: &str, listener: BoxedListener) {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .listeners
            .insert(key.to_string(), listener);
    }

    /// Remove a listener by its subscription key.
    pub fn unsubscribe(&self, key: &str) {
        self.inner.lock().unwrap_or_else(|e| e.into_inner()).listeners.remove(key);
    }

    /// Create a two-way binding between this binding and another.
    ///
    /// Whenever either binding's value changes, the other is updated to match.
    /// Uses an atomic synchronization guard to prevent infinite notification
    /// loops. The two-way connection uses `Weak` references to avoid reference
    /// cycles and prevent use-after-free if one binding is dropped.
    pub fn bind_to(&self, other: &Binding<T>)
    where
        T: PartialEq,
    {
        let syncing = Arc::new(AtomicBool::new(false));

        let self_weak = Arc::downgrade(&self.inner);
        let other_weak = Arc::downgrade(&other.inner);

        let listener_self_key = format!("__two_way_self_{:p}", Arc::as_ptr(&self.inner));
        let listener_other_key = format!("__two_way_other_{:p}", Arc::as_ptr(&other.inner));

        self.subscribe(
            &listener_self_key,
            Box::new(TwoWayListener::new(syncing.clone(), self_weak.clone(), other_weak.clone())),
        );
        other.subscribe(
            &listener_other_key,
            Box::new(TwoWayListener::new(syncing, other_weak, self_weak)),
        );
    }

    /// Return the number of currently registered listeners.
    pub fn listener_count(&self) -> usize {
        self.inner.lock().unwrap_or_else(|e| e.into_inner()).listeners.len()
    }
}

impl<T: Clone + Send + 'static> BindingInner<T> {
    /// Set value without notifying listeners.
    /// Used by TwoWayListener to propagate changes silently.
    fn set_no_notify(&mut self, value: T) {
        self.value = value;
    }
}

/// A listener that propagates value changes from one binding to another.
///
/// Used internally by [`Binding::bind_to`] to implement two-way synchronization.
/// Uses `Weak<Mutex<BindingInner<T>>>` internally so that if one binding is
/// dropped, the listener on the other safely detects this and becomes a no-op.
struct TwoWayListener<T: Clone + Send + 'static> {
    syncing: Arc<AtomicBool>,
    source: Weak<Mutex<BindingInner<T>>>,
    target: Weak<Mutex<BindingInner<T>>>,
}

impl<T: Clone + Send + 'static> TwoWayListener<T> {
    fn new(
        syncing: Arc<AtomicBool>,
        source: Weak<Mutex<BindingInner<T>>>,
        target: Weak<Mutex<BindingInner<T>>>,
    ) -> Self {
        Self { syncing, source, target }
    }
}

impl<T: Clone + Send + 'static + PartialEq> BindingListener for TwoWayListener<T> {
    fn on_value_changed(&mut self, _key: &str, _operation: &str) {
        if self.syncing.swap(true, Ordering::SeqCst) {
            return;
        }

        // Read value from source, then release source's Mutex lock BEFORE
        // locking the target.  This avoids a re-entrant-Mutex deadlock when
        // the outer `set()` already holds the source binding's lock.
        let val = self
            .source
            .upgrade()
            .map(|source| source.lock().unwrap_or_else(|e| e.into_inner()).value.clone());

        // If either binding has been dropped, skip gracefully.
        if let Some(val) = val {
            if let Some(target) = self.target.upgrade() {
                target.lock().unwrap_or_else(|e| e.into_inner()).set_no_notify(val);
            }
        }

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
        let b = Binding::new(42);
        assert_eq!(b.get(), 42);
        b.set(100);
        assert_eq!(b.get(), 100);
    }

    #[test]
    fn test_binding_listener_notification() {
        let b = Binding::new("hello".to_string());
        let notified = Arc::new(AtomicBool::new(false));
        let n = notified.clone();
        let listener = Box::new(FnListener::new(move |_key, _op| {
            n.store(true, Ordering::SeqCst);
        }));
        b.subscribe("test", listener);
        b.set("world".to_string());
        assert!(notified.load(Ordering::SeqCst));
    }

    #[test]
    fn test_binding_unsubscribe() {
        let b = Binding::new(0);
        let count = Arc::new(AtomicI32::new(0));
        let c = count.clone();
        let listener = Box::new(FnListener::new(move |_key, _op| {
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
        let b = Binding::new(0);
        let count_a = Arc::new(AtomicI32::new(0));
        let count_b = Arc::new(AtomicI32::new(0));

        let ca = count_a.clone();
        b.subscribe(
            "a",
            Box::new(FnListener::new(move |_, _| {
                ca.fetch_add(1, Ordering::SeqCst);
            })),
        );
        let cb = count_b.clone();
        b.subscribe(
            "b",
            Box::new(FnListener::new(move |_, _| {
                cb.fetch_add(1, Ordering::SeqCst);
            })),
        );
        b.set(1);
        assert_eq!(count_a.load(Ordering::SeqCst), 1);
        assert_eq!(count_b.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_binding_listener_receive_key() {
        let b = Binding::new(0);
        let received_key = Arc::new(Mutex::new(String::new()));
        let rk = received_key.clone();
        let listener = Box::new(FnListener::new(move |key, _op| {
            *rk.lock().unwrap() = key.to_string();
        }));
        b.subscribe("my_key", listener);
        b.set(99);
        assert_eq!(*received_key.lock().unwrap(), "my_key");
    }

    #[test]
    fn test_binding_two_way_sync() {
        let a = Binding::new(10);
        let b = Binding::new(20);

        a.bind_to(&b);

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
        let a = Binding::new(0);
        let b = Binding::new(0);
        let a_count = Arc::new(AtomicI32::new(0));
        let b_count = Arc::new(AtomicI32::new(0));

        let ac = a_count.clone();
        a.subscribe(
            "a_count",
            Box::new(FnListener::new(move |_, _| {
                ac.fetch_add(1, Ordering::SeqCst);
            })),
        );
        let bc = b_count.clone();
        b.subscribe(
            "b_count",
            Box::new(FnListener::new(move |_, _| {
                bc.fetch_add(1, Ordering::SeqCst);
            })),
        );

        a.bind_to(&b);

        // Changing a should notify a's listeners once and propagate to b
        // via set_no_notify (b's listeners are NOT fired by the TwoWayListener).
        a.set(42);
        assert_eq!(a_count.load(Ordering::SeqCst), 1);
        assert_eq!(b_count.load(Ordering::SeqCst), 0);

        // Setting b directly should fire b's listeners
        b.set(100);
        assert_eq!(a_count.load(Ordering::SeqCst), 1); // a's listeners unchanged
        assert_eq!(b_count.load(Ordering::SeqCst), 1); // b's listener fired for b.set()
    }

    #[test]
    fn test_binding_drop_safety() {
        // Verify that dropping one binding doesn't cause UB in the other's listener.
        let a = Arc::new(Binding::new(10));
        let b = Arc::new(Binding::new(20));
        a.bind_to(&b);

        // Drop 'a' — b's listener holds a Weak to a's inner, which should
        // gracefully become a no-op.
        drop(a);

        // Setting b should not panic or cause UB
        b.set(99);
        assert_eq!(b.get(), 99);
    }
}
