use core::any::Any;

/// A listener that gets notified when a binding value changes.
///
/// Implementors receive a `key` string identifying which binding changed,
/// allowing a single listener to observe multiple bindings.
pub trait BindingListener: Any + Send {
    /// Called when a bound value changes.
    ///
    /// `key` is the subscription key provided at subscribe time.
    /// `operation` is a context label describing what triggered the change
    /// (e.g. `"set"`, `"push"`, `"invalidate"`, or `""` for generic updates).
    fn on_value_changed(&mut self, key: &str, operation: &str);
}

/// Boxed heap-allocated listener for storage in binding containers.
pub type BoxedListener = Box<dyn BindingListener>;

/// A simple listener that invokes a closure on change.
///
/// The closure receives `(key, operation)` where `key` is the subscription
/// key and `operation` is a context label describing the change trigger.
pub struct FnListener<F: FnMut(&str, &str) + Send> {
    f: F,
}

impl<F: FnMut(&str, &str) + Send + 'static> FnListener<F> {
    /// Create a new function listener.
    ///
    /// The closure receives `(key, operation)` where `key` is the subscription
    /// key and `operation` is a context label describing the change trigger.
    pub fn new(f: F) -> Self {
        Self { f }
    }
}

impl<F: FnMut(&str, &str) + Send + 'static> BindingListener for FnListener<F> {
    fn on_value_changed(&mut self, key: &str, operation: &str) {
        (self.f)(key, operation);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::Mutex;
    use alloc::sync::Arc;
    use core::sync::atomic::{AtomicI32, Ordering};

    #[test]
    fn test_fn_listener_invocation() {
        let invoked_key = Arc::new(Mutex::new(String::new()));
        let ik = invoked_key.clone();
        let mut listener = FnListener::new(move |key, _op| {
            *ik.lock().unwrap() = key.to_string();
        });
        listener.on_value_changed("test_key", "");
        assert_eq!(*invoked_key.lock().unwrap(), "test_key");
    }

    #[test]
    fn test_fn_listener_multiple_calls() {
        let count = Arc::new(AtomicI32::new(0));
        let c = count.clone();
        let mut listener = FnListener::new(move |_, _| {
            c.fetch_add(1, Ordering::SeqCst);
        });
        listener.on_value_changed("a", "");
        listener.on_value_changed("b", "");
        listener.on_value_changed("c", "");
        assert_eq!(count.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn test_boxed_listener_send() {
        let listener: BoxedListener = Box::new(FnListener::new(|_, _| {}));
        // Verify it implements Send by moving to a thread
        let handle = std::thread::spawn(move || {
            let _ = listener;
        });
        handle.join().unwrap();
    }
}
