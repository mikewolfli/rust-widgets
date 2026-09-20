// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use crate::compat::{format, lock, Box, HashMap, MiniToString, Mutex, String, Vec};
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
    /// `true` while `set` is running its notification pass outside the lock.
    ///
    /// `listeners` is empty for the duration of that pass, so [`unsubscribe`] cannot
    /// tell "not subscribed" from "temporarily taken" — this flag is what lets it
    /// record the intent instead of guessing.
    notifying: bool,
    /// Keys removed by [`unsubscribe`] during the current notification pass.
    ///
    /// Cleared when a pass starts and again once its restore step has run, so it only
    /// ever describes the pass currently in flight.
    unsubscribed_during_notify: alloc::collections::BTreeSet<String>,
}

impl<T: Clone + Send + 'static> BindingInner<T> {
    /// An empty binding state holding `value`.
    ///
    /// Used by `Binding::new` and by test fixtures that need an `Arc<Mutex<BindingInner>>`
    /// without going through a `Binding`. Exists so adding bookkeeping fields to the
    /// struct does not require touching every construction site.
    fn new(value: T) -> Self {
        Self {
            value,
            listeners: HashMap::new(),
            notifying: false,
            unsubscribed_during_notify: alloc::collections::BTreeSet::new(),
        }
    }
}

impl<T: Clone + Send + 'static> Binding<T> {
    /// Create a new binding with an initial value.
    pub fn new(value: T) -> Self {
        Self { inner: Arc::new(Mutex::new(BindingInner::new(value))) }
    }

    /// Get the current value.
    ///
    /// For `T: Copy` types, use [`get_copy`](Self::get_copy) to avoid the clone.
    #[inline(always)]
    pub fn get(&self) -> T {
        lock(&self.inner).value.clone()
    }

    /// Set a new value and notify all listeners.
    ///
    /// Notifications are dispatched **outside** the Mutex lock to prevent
    /// re-entrancy deadlocks (e.g. when a TwoWayListener tries to lock the
    /// same binding's Mutex while propagating a value change).
    ///
    /// # Restoring the listener map
    ///
    /// Listeners are temporarily removed from the map, notified, then restored.
    /// Restoring means "still subscribed afterwards", and the only authoritative
    /// record of that is the map itself during the notification window: a listener
    /// that calls [`unsubscribe`](Self::unsubscribe) from its own callback — or is
    /// unsubscribed by an earlier listener in the same pass — must stay removed.
    ///
    /// The restore used to be `entry(key).or_insert(listener)`, which cannot tell
    /// "re-subscribed" from "just unsubscribed": both leave the key absent, so a
    /// self-unsubscribing listener was put straight back and fired again on the next
    /// `set`. The removal set below is what distinguishes them.
    pub fn set(&self, value: T) {
        // ── Phase 1: Lock, update value, take all listeners ──
        let mut listeners: Vec<(String, BoxedListener)>;
        {
            let mut inner = lock(&self.inner);
            inner.value = value;
            inner.unsubscribed_during_notify.clear();
            listeners = core::mem::take(&mut inner.listeners).into_iter().collect();
        } // Mutex lock released.

        // ── Phase 2: Notify outside lock (safe from re-entrancy) ──
        // `notifying` brackets the pass so `unsubscribe` knows the map is temporarily
        // empty rather than genuinely missing the key.
        {
            lock(&self.inner).notifying = true;
        }
        for (key, ref mut listener) in &mut listeners {
            listener.on_value_changed(key, "set");
        }

        // ── Phase 3: Restore only the listeners that are still subscribed ──
        {
            let mut inner = lock(&self.inner);
            for (key, listener) in listeners {
                if inner.unsubscribed_during_notify.contains(&key) {
                    // Removed by `unsubscribe` while this pass was notifying; honour it.
                    continue;
                }
                // If no new listener was subscribed under this key during
                // notification, put the original one back. A *new* listener wins,
                // which is the pre-existing behaviour and is kept here.
                inner.listeners.entry(key).or_insert(listener);
            }
            inner.unsubscribed_during_notify.clear();
            inner.notifying = false;
        }
    }

    /// Subscribe to value changes.
    ///
    /// `key` is an identifier used to later unsubscribe. If a listener with
    /// the same key already exists, it is replaced.
    pub fn subscribe(&self, key: &str, listener: BoxedListener) {
        lock(&self.inner).listeners.insert(key.to_string(), listener);
    }

    /// Remove a listener by its subscription key.
    ///
    /// Safe to call from inside a listener callback: the removal is recorded on the
    /// binding and honoured when [`set`](Self::set) restores its listener map, so the
    /// listener does not come back. See `set`'s "Restoring the listener map" section.
    pub fn unsubscribe(&self, key: &str) {
        let mut inner = lock(&self.inner);
        inner.listeners.remove(key);
        // The key is also absent from `listeners` while a notification pass is in
        // flight, so `remove` returning `None` there is not proof of a no-op. Record
        // the intent unconditionally; the set is cleared at the start of every pass
        // and after the restore, so a stale entry cannot outlive its notification.
        if inner.notifying {
            inner.unsubscribed_during_notify.insert(key.to_string());
        }
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

    /// Get the current value without cloning (available for `Copy` types).
    ///
    /// This avoids the `.clone()` that [`get`](Self::get) always performs.
    #[inline(always)]
    pub fn get_copy(&self) -> T
    where
        T: Copy,
    {
        lock(&self.inner).value
    }

    /// Return the number of currently registered listeners.
    pub fn listener_count(&self) -> usize {
        lock(&self.inner).listeners.len()
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
        // Re-entrancy guard: the first thread in wins, and the flag is cleared by
        // the RAII guard below on **every** exit — including a panic from
        // `set_no_notify`. A bare `store(false)` at the end of the body would be
        // skipped if anything panicked, leaving `syncing` stuck at `true` and
        // silently disabling this two-way binding forever.
        if self.syncing.swap(true, Ordering::SeqCst) {
            return;
        }
        let _reset = SyncingGuard { flag: &self.syncing };

        // Read value from source, then release source's Mutex lock BEFORE
        // locking the target.  This avoids a re-entrant-Mutex deadlock when
        // the outer `set()` already holds the source binding's lock.
        let val = self.source.upgrade().map(|source| lock(&source).value.clone());

        // If either binding has been dropped, skip gracefully.
        if let Some(val) = val {
            if let Some(target) = self.target.upgrade() {
                lock(&target).set_no_notify(val);
            }
        }
    }
}

/// Clears the `syncing` flag when dropped, so the re-entrancy guard cannot be
/// left armed by an unwinding panic.
struct SyncingGuard<'a> {
    flag: &'a AtomicBool,
}

impl Drop for SyncingGuard<'_> {
    fn drop(&mut self) {
        self.flag.store(false, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::{lock, Mutex};
    use core::sync::atomic::AtomicI32;

    #[test]
    fn test_binding_get_set() {
        let b = Binding::new(42);
        assert_eq!(b.get(), 42);
        b.set(100);
        assert_eq!(b.get(), 100);
    }

    /// A panic while propagating must not leave the two-way re-entrancy guard
    /// permanently armed.
    ///
    /// `TwoWayListener` raises `syncing` on entry to suppress the echo from the
    /// reverse direction. If the propagation body panicked, the old code's final
    /// `store(false)` was skipped, so `syncing` stayed `true` and the binding
    /// silently stopped syncing forever. `SyncingGuard` resets it on every exit,
    /// which this test pins down by unwinding through the guarded section.
    #[test]
    fn syncing_guard_clears_flag_on_panic() {
        let flag = AtomicBool::new(false);
        assert!(!flag.swap(true, Ordering::SeqCst), "flag starts unset");

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _reset = SyncingGuard { flag: &flag };
            panic!("boom inside the guarded section");
        }));

        assert!(result.is_err(), "the probe must panic");
        assert!(
            !flag.load(Ordering::SeqCst),
            "the guard must clear `syncing` while unwinding, otherwise the binding is dead forever"
        );
    }

    /// The guard clears the flag on the normal (non-panicking) path as well.
    #[test]
    fn syncing_guard_clears_flag_on_normal_exit() {
        let flag = AtomicBool::new(false);
        flag.store(true, Ordering::SeqCst);
        {
            let _reset = SyncingGuard { flag: &flag };
        }
        assert!(!flag.load(Ordering::SeqCst), "the guard must clear `syncing` on drop");
    }

    /// A two-way binding still synchronises after its listener ran once.
    #[test]
    fn two_way_listener_propagates_and_rearms() {
        let syncing = Arc::new(AtomicBool::new(false));
        let source: Arc<Mutex<BindingInner<i32>>> = Arc::new(Mutex::new(BindingInner::new(7)));
        let target: Arc<Mutex<BindingInner<i32>>> = Arc::new(Mutex::new(BindingInner::new(0)));

        let mut listener = TwoWayListener::new(
            Arc::clone(&syncing),
            Arc::downgrade(&source),
            Arc::downgrade(&target),
        );

        listener.on_value_changed("k", "set");
        assert_eq!(lock(&target).value, 7, "the value must propagate source -> target");
        assert!(!syncing.load(Ordering::SeqCst), "the guard must re-arm the listener");

        // A second call must still work (the guard did not get stuck).
        lock(&source).value = 9;
        listener.on_value_changed("k", "set");
        assert_eq!(lock(&target).value, 9, "a later change must still propagate");
        assert!(!syncing.load(Ordering::SeqCst));
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
            *lock(&rk) = key.to_string();
        }));
        b.subscribe("my_key", listener);
        b.set(99);
        assert_eq!(*lock(&received_key), "my_key");
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

#[cfg(test)]
mod unsubscribe_during_notify_tests {
    use super::*;
    use crate::compat::lock;
    use alloc::sync::Arc;
    use core::sync::atomic::AtomicI32;

    /// A listener that unsubscribes itself must stay unsubscribed.
    ///
    /// `set` takes every listener out of the map before notifying and puts them back
    /// afterwards. The restore was `entry(key).or_insert(listener)`, which cannot tell
    /// "re-subscribed" from "just unsubscribed" — both leave the key absent — so a
    /// listener calling `unsubscribe(&self.key)` from its own callback was put straight
    /// back and fired again on the next `set`. For the natural "remove me once I have
    /// seen what I need" pattern that is unbounded listener accumulation.
    #[test]
    fn a_listener_that_unsubscribes_itself_stays_removed() {
        let binding: Arc<Binding<i32>> = Arc::new(Binding::new(0));
        let weak = Arc::downgrade(&binding);
        let calls = Arc::new(AtomicI32::new(0));

        let counter = Arc::clone(&calls);
        binding.subscribe(
            "self_removing",
            Box::new(FnListener::new(move |_key: &str, _value: &str| {
                counter.fetch_add(1, Ordering::SeqCst);
                if let Some(b) = weak.upgrade() {
                    b.unsubscribe("self_removing");
                }
            })),
        );

        binding.set(1);
        assert_eq!(calls.load(Ordering::SeqCst), 1, "the listener must fire once");
        assert_eq!(binding.listener_count(), 0, "it must not be restored after unsubscribing");

        binding.set(2);
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "a listener that unsubscribed itself must not fire again"
        );
    }

    /// A listener removed by an earlier listener in the same pass must not fire later.
    ///
    /// The iterate-and-restore loop used to re-insert it, so it survived the very `set`
    /// that removed it and fired on the next one.
    #[test]
    fn a_listener_removed_by_an_earlier_listener_does_not_fire() {
        let binding: Arc<Binding<i32>> = Arc::new(Binding::new(0));
        let victim_calls = Arc::new(AtomicI32::new(0));

        let counter = Arc::clone(&victim_calls);
        binding.subscribe(
            "victim",
            Box::new(FnListener::new(move |_key: &str, _value: &str| {
                counter.fetch_add(1, Ordering::SeqCst);
            })),
        );

        let weak = Arc::downgrade(&binding);
        binding.subscribe(
            "remover",
            Box::new(FnListener::new(move |_key: &str, _value: &str| {
                if let Some(b) = weak.upgrade() {
                    b.unsubscribe("victim");
                }
            })),
        );

        binding.set(1);
        assert_eq!(binding.listener_count(), 1, "only the remover must remain");
        assert_eq!(victim_calls.load(Ordering::SeqCst), 1, "it was notified this pass");

        binding.set(2);
        assert_eq!(
            victim_calls.load(Ordering::SeqCst),
            1,
            "the removed listener must not be notified again"
        );
    }

    /// Re-subscribing during notification still wins over the restore.
    ///
    /// This is the pre-existing behaviour the fix had to preserve: a key present in the
    /// map when the restore runs means a *new* listener took the slot, and that new one
    /// must be the one that survives.
    #[test]
    fn resubscribing_during_notify_keeps_the_new_listener() {
        let binding: Arc<Binding<i32>> = Arc::new(Binding::new(0));
        let old_calls = Arc::new(AtomicI32::new(0));
        let new_calls = Arc::new(AtomicI32::new(0));

        let old_counter = Arc::clone(&old_calls);
        let weak = Arc::downgrade(&binding);
        let new_counter = Arc::clone(&new_calls);
        binding.subscribe(
            "slot",
            Box::new(FnListener::new(move |_key: &str, _value: &str| {
                old_counter.fetch_add(1, Ordering::SeqCst);
                if let Some(b) = weak.upgrade() {
                    let counter = Arc::clone(&new_counter);
                    b.subscribe(
                        "slot",
                        Box::new(FnListener::new(move |_key: &str, _value: &str| {
                            counter.fetch_add(1, Ordering::SeqCst);
                        })),
                    );
                }
            })),
        );

        binding.set(1);
        assert_eq!(old_calls.load(Ordering::SeqCst), 1);
        assert_eq!(binding.listener_count(), 1, "the slot holds exactly one listener");

        binding.set(2);
        assert_eq!(new_calls.load(Ordering::SeqCst), 1, "the replacement listener runs");
        assert_eq!(old_calls.load(Ordering::SeqCst), 1, "the replaced listener does not");
    }

    /// The bookkeeping must not leak between notification passes.
    #[test]
    fn unsubscribing_outside_a_notification_is_not_remembered() {
        let binding: Arc<Binding<i32>> = Arc::new(Binding::new(0));
        binding.subscribe("a", Box::new(FnListener::new(|_: &str, _: &str| {})));
        binding.subscribe("b", Box::new(FnListener::new(|_: &str, _: &str| {})));
        binding.unsubscribe("a");
        assert_eq!(binding.listener_count(), 1);

        // The stray removal of "a" must not suppress a later re-subscription.
        binding.subscribe("a", Box::new(FnListener::new(|_: &str, _: &str| {})));
        binding.set(1);
        assert_eq!(binding.listener_count(), 2, "both listeners must survive the pass");
        let inner = lock(&binding.inner);
        assert!(inner.unsubscribed_during_notify.is_empty(), "the set must be cleared");
    }
}
