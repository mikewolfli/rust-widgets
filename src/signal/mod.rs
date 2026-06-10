//! Signal and slot implementation.
mod core_signal;
mod generic_signal;
mod hub;
pub use core_signal::{ConnectionHandle, ConnectionScope, Signal};
pub use generic_signal::{GenericSignal, Signal1};
pub use hub::CustomSignalHub;
#[cfg(test)]
mod tests {
    use super::{ConnectionScope, GenericSignal, Signal};
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
}
