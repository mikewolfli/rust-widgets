//! Signal and slot implementation.
mod core_signal;
mod generic_signal;
mod hub;
pub use core_signal::{ConnectionHandle, ConnectionScope, Signal};
pub use generic_signal::{GenericSignal, Signal1};
pub use hub::CustomSignalHub;
#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use super::{ConnectionScope, GenericSignal, Signal};
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
}
