//! Signal and slot implementation.
mod core_signal;
mod generic_signal;
mod hub;
pub use core_signal::{ConnectionHandle, ConnectionScope, Priority, Signal};
pub use generic_signal::{GenericSignal, Signal1};
pub use hub::CustomSignalHub;
#[cfg(test)]
mod tests {
    use super::{ConnectionScope, GenericSignal, Priority, Signal};
    use alloc::sync::Arc;
    use core::sync::atomic::{AtomicUsize, Ordering};
    #[test]
    fn signal_emits_to_multiple_slots() {
        let signal = Signal::<u32>::new();
        let sum = Arc::new(AtomicUsize::new(0));
        let sum_a = Arc::clone(&sum);
        signal.connect(move |value: Arc<u32>| {
            sum_a.fetch_add(*value as usize, Ordering::SeqCst);
        });
        let sum_b = Arc::clone(&sum);
        signal.connect(move |value: Arc<u32>| {
            sum_b.fetch_add((*value as usize) * 2, Ordering::SeqCst);
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

    #[test]
    fn signal_reentrant_emit_does_not_deadlock() {
        let signal = Signal::<u32>::new();
        let emitted = Arc::new(AtomicUsize::new(0));
        let e1 = emitted.clone();
        let e2 = emitted.clone();
        let s2 = signal.clone();

        // First callback connects another callback (re-entrant)
        signal.connect(move |v| {
            e1.fetch_add(1, Ordering::SeqCst);
            if *v == 1 {
                let e2_clone = e2.clone();
                s2.connect(move |_| {
                    e2_clone.fetch_add(1, Ordering::SeqCst);
                });
            }
        });

        signal.emit(1); // First emit, callback connects another
        signal.emit(2); // Second emit, both callbacks fire

        assert_eq!(
            emitted.load(Ordering::SeqCst),
            3,
            "Both callbacks should fire without deadlock"
        );
    }

    #[test]
    fn signal_block_unblock_works() {
        let signal = Signal::<u32>::new();
        let hits = Arc::new(AtomicUsize::new(0));
        let h = hits.clone();
        let handle = signal.connect(move |v| {
            h.fetch_add(*v as usize, Ordering::SeqCst);
        });

        signal.emit(1);
        assert_eq!(hits.load(Ordering::SeqCst), 1);

        assert!(signal.block(handle));
        assert_eq!(signal.is_blocked(handle), Some(true));
        signal.emit(2);
        assert_eq!(hits.load(Ordering::SeqCst), 1); // Not incremented

        assert!(signal.unblock(handle));
        assert_eq!(signal.is_blocked(handle), Some(false));
        signal.emit(3);
        assert_eq!(hits.load(Ordering::SeqCst), 4);
    }

    #[test]
    fn signal_is_connected_works() {
        let signal = Signal::<String>::new();
        let handle = signal.connect(|_| {});
        assert!(signal.is_connected(handle));
        signal.disconnect(handle);
        assert!(!signal.is_connected(handle));
    }

    #[test]
    fn signal_priority_ordering() {
        let signal = Signal::<u32>::new();
        let order = Arc::new(std::sync::Mutex::new(Vec::new()));

        let o1 = order.clone();
        signal.connect_with_priority(
            move |_| {
                o1.lock().unwrap().push("low");
            },
            Priority::Low,
        );

        let o2 = order.clone();
        signal.connect_with_priority(
            move |_| {
                o2.lock().unwrap().push("high");
            },
            Priority::High,
        );

        let o3 = order.clone();
        signal.connect(move |_| {
            o3.lock().unwrap().push("normal");
        });

        signal.emit(0);
        let ord = order.lock().unwrap();
        assert_eq!(ord[0], "high");
        assert_eq!(ord[1], "normal");
        assert_eq!(ord[2], "low");
    }

    #[test]
    fn connection_scope_clear_and_count() {
        let signal = GenericSignal::new();
        let scope = ConnectionScope::new();
        let hits = Arc::new(AtomicUsize::new(0));
        let h = hits.clone();
        signal.connect_scoped(&scope, move || {
            h.fetch_add(1, Ordering::SeqCst);
        });

        assert_eq!(scope.disconnect_count(), 1);
        signal.emit();
        assert_eq!(hits.load(Ordering::SeqCst), 1);

        scope.clear();
        assert_eq!(scope.disconnect_count(), 0);
        signal.emit();
        assert_eq!(hits.load(Ordering::SeqCst), 1); // No more increments
    }

    #[test]
    fn generic_signal_is_empty_and_clear() {
        let signal = GenericSignal::new();
        assert!(signal.is_empty());
        signal.connect(|| {});
        assert!(!signal.is_empty());
        signal.clear();
        assert!(signal.is_empty());
    }

    #[test]
    fn custom_signal_hub_remove_and_contains() {
        let hub = super::CustomSignalHub::default();
        hub.define("test");
        assert!(hub.contains("test"));
        assert_eq!(hub.signal_count(), 1);
        assert!(!hub.is_empty());
        hub.remove("test");
        assert!(!hub.contains("test"));
        assert!(hub.is_empty());
        hub.define("a");
        hub.define("b");
        assert_eq!(hub.signal_count(), 2);
        hub.clear();
        assert!(hub.is_empty());
    }
}
