// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use crate::compat::{Box, HashMap, MiniToString, String, Vec};
use crate::data_binding::traits::*;

/// A computed/derived value that auto-updates when its dependencies change.
///
/// The dirty flag is a scheduling signal: invalidation only marks the value as
/// stale; the actual recomputation runs on the next [`get`](Computed::get). A
/// listener is notified only when the recomputed value differs from the cached
/// value, so notification reflects real state transitions rather than the mere
/// act of invalidating a dependency.
pub struct Computed<T: Clone + Send + 'static> {
    compute_fn: Box<dyn Fn() -> T>,
    cached: T,
    dirty: bool,
    listeners: HashMap<String, BoxedListener>,
}

impl<T: Clone + Send + 'static> Computed<T> {
    /// Create a new computed value with an initial value.
    ///
    /// `compute` is the derivation function. `initial` is the starting value
    /// before any dependencies are invalidated. The value is **not** recomputed
    /// on the first [`get`](Self::get); call [`invalidate`](Self::invalidate)
    /// to mark it dirty when dependencies change.
    pub fn new<F>(compute: F, initial: T) -> Self
    where
        F: Fn() -> T + 'static,
    {
        Self {
            compute_fn: Box::new(compute),
            cached: initial,
            dirty: false,
            listeners: HashMap::new(),
        }
    }

    /// Get the current value, recomputing if the value is dirty.
    ///
    /// If the recomputed value differs from the cached value (using `PartialEq`),
    /// listeners are notified after the update.
    pub fn get(&mut self) -> T
    where
        T: PartialEq,
    {
        if self.dirty {
            let new_value = (self.compute_fn)();
            if new_value != self.cached {
                self.cached = new_value;
                self.dirty = false;
                self.notify_listeners();
            } else {
                self.dirty = false;
            }
        }
        self.cached.clone()
    }

    /// Get the current value without checking dirtiness (returns cached value).
    ///
    /// Useful when you know the value is clean and want to avoid recomputation.
    pub fn get_cached(&self) -> T {
        self.cached.clone()
    }

    /// Mark the computed value as dirty and schedule a recomputation on the next
    /// [`get`](Computed::get) call.
    ///
    /// Invalidating a dependency does not imply the value changed; listeners are
    /// notified only after a real recomputation produces a different output.
    pub fn invalidate(&mut self) {
        self.dirty = true;
    }

    /// Check whether the computed value has been invalidated.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Subscribe to value changes.
    ///
    /// The listener is notified when [`get`](Computed::get) recomputes a value
    /// that differs from the cached value.
    pub fn subscribe(&mut self, key: &str, listener: BoxedListener) {
        self.listeners.insert(key.to_string(), listener);
    }

    fn notify_listeners(&mut self) {
        let keys: Vec<String> = self.listeners.keys().cloned().collect();
        for key in &keys {
            if let Some(listener) = self.listeners.get_mut(key) {
                listener.on_value_changed(key, "invalidate");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::sync::Arc;
    use core::sync::atomic::{AtomicI32, Ordering};

    #[test]
    fn test_computed_initial_value() {
        let mut c: Computed<i32> = Computed::new(|| 42, 0);
        // With dirty: false, the initial value is returned directly
        assert_eq!(c.get(), 0);
        // After invalidation, the compute function runs
        c.invalidate();
        assert_eq!(c.get(), 42);
    }

    #[test]
    fn test_computed_recomputation_on_invalidate() {
        let factor = Arc::new(AtomicI32::new(2));
        let f = factor.clone();
        let mut c = Computed::new(move || f.load(Ordering::SeqCst) * 10, 0);

        // Initially dirty=false — need to invalidate first to trigger compute
        c.invalidate();
        assert_eq!(c.get(), 20);

        factor.store(5, Ordering::SeqCst);
        c.invalidate();
        assert!(c.is_dirty());
        assert_eq!(c.get(), 50);
        assert!(!c.is_dirty());
    }

    #[test]
    fn test_computed_no_recomputation_when_clean() {
        let compute_count = Arc::new(AtomicI32::new(0));
        let cc = compute_count.clone();
        let mut c = Computed::new(
            move || {
                cc.fetch_add(1, Ordering::SeqCst);
                100
            },
            0,
        );

        // Initially dirty=false, so get() returns initial without computing
        assert_eq!(c.get(), 0);
        assert_eq!(compute_count.load(Ordering::SeqCst), 0);

        // After invalidate, get() triggers computation
        c.invalidate();
        assert_eq!(c.get(), 100);
        assert_eq!(compute_count.load(Ordering::SeqCst), 1);

        // Second get on clean value does not recompute
        assert_eq!(c.get(), 100);
        assert_eq!(compute_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_computed_listener_notification() {
        let inner = Arc::new(AtomicI32::new(10));
        let i = inner.clone();
        let mut c2 = Computed::new(move || i.load(Ordering::SeqCst), 0);

        let notified_count = Arc::new(AtomicI32::new(0));
        let nc = notified_count.clone();
        c2.subscribe(
            "test",
            Box::new(FnListener::new(move |_, _| {
                nc.fetch_add(1, Ordering::SeqCst);
            })),
        );

        // Initial value is cached and clean; no notification yet.
        assert_eq!(c2.get(), 0);
        assert_eq!(notified_count.load(Ordering::SeqCst), 0);

        // Invalidating a dependency does not notify listeners immediately.
        inner.store(10, Ordering::SeqCst);
        c2.invalidate();
        assert_eq!(notified_count.load(Ordering::SeqCst), 0);

        // The next get() recomputes and emits only on real change.
        assert_eq!(c2.get(), 10);
        assert_eq!(notified_count.load(Ordering::SeqCst), 1);

        // A second invalidation without a derived change does not notify.
        inner.store(10, Ordering::SeqCst);
        c2.invalidate();
        assert_eq!(notified_count.load(Ordering::SeqCst), 1);
        assert_eq!(c2.get(), 10);
        assert_eq!(notified_count.load(Ordering::SeqCst), 1);

        // A true value change emits once after recomputation.
        inner.store(20, Ordering::SeqCst);
        c2.invalidate();
        assert_eq!(notified_count.load(Ordering::SeqCst), 1);
        assert_eq!(c2.get(), 20);
        assert_eq!(notified_count.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn test_computed_get_cached() {
        let mut c = Computed::new(|| 99, 0);

        // get_cached returns the initial value before any get() call
        assert_eq!(c.get_cached(), 0);

        // After invalidate+get(), cached is updated
        c.invalidate();
        assert_eq!(c.get(), 99);
        assert_eq!(c.get_cached(), 99);

        // After invalidate, get_cached still returns old value until get()
        c.invalidate();
        assert_eq!(c.get_cached(), 99);
        assert_eq!(c.get(), 99);
    }
}
