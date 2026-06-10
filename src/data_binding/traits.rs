use std::any::Any;

/// A listener that gets notified when a binding value changes.
///
/// Implementors receive a `key` string identifying which binding changed,
/// allowing a single listener to observe multiple bindings.
pub trait BindingListener: Any + Send {
    /// Called when a bound value changes.
    ///
    /// `key` is the subscription key provided at subscribe time.
    fn on_value_changed(&mut self, key: &str);
}

/// Boxed heap-allocated listener for storage in binding containers.
pub type BoxedListener = Box<dyn BindingListener>;

/// A simple listener that invokes a closure on change.
pub struct FnListener<F: FnMut(&str) + Send> {
    f: F,
}

impl<F: FnMut(&str) + Send + 'static> FnListener<F> {
    /// Create a new function listener.
    pub fn new(f: F) -> Self {
        Self { f }
    }
}

impl<F: FnMut(&str) + Send + 'static> BindingListener for FnListener<F> {
    fn on_value_changed(&mut self, key: &str) {
        (self.f)(key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicI32, Ordering};
    use std::sync::{Arc, Mutex};

    #[test]
    fn test_fn_listener_invocation() {
        let invoked_key = Arc::new(Mutex::new(String::new()));
        let ik = invoked_key.clone();
        let mut listener = FnListener::new(move |key| {
            *ik.lock().unwrap() = key.to_string();
        });
        listener.on_value_changed("test_key");
        assert_eq!(*invoked_key.lock().unwrap(), "test_key");
    }

    #[test]
    fn test_fn_listener_multiple_calls() {
        let count = Arc::new(AtomicI32::new(0));
        let c = count.clone();
        let mut listener = FnListener::new(move |_| {
            c.fetch_add(1, Ordering::SeqCst);
        });
        listener.on_value_changed("a");
        listener.on_value_changed("b");
        listener.on_value_changed("c");
        assert_eq!(count.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn test_boxed_listener_send() {
        let listener: BoxedListener = Box::new(FnListener::new(|_| {}));
        // Verify it implements Send by moving to a thread
        let handle = std::thread::spawn(move || {
            let _ = listener;
        });
        handle.join().unwrap();
    }
}
