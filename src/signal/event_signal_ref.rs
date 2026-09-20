// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! A control's signal, addressed by name and erased to one type.
//!
//! # The gap this closes
//!
//! A control's signals are typed and live on the concrete type: `Button::clicked` is a
//! `GenericSignal`, `Slider::value_changed` is a `Signal1<i32>`. A designer's wiring, by contrast,
//! exists only as text — the user connected `value_changed` to something, and the program learns
//! which name that was **at run time**. Nothing could previously turn a name into a subscription
//! without the host writing one `match` arm per control, which is exactly what a designer cannot
//! pre-generate (§3.4 of BLUE19, rule #98).
//!
//! [`EventSignalRef`] is that name-to-signal step, with the payload erased to
//! [`CapabilityValue`] — the same type the property API already uses, so an event value and a
//! property value travel through one representation rather than two (rule #54).
//!
//! # Why the value is boxed behind a closure rather than matched
//!
//! Erasing `Signal1<T>` to something nameable has to happen where `T` is still known, which is in
//! the control's own `event_signal_dyn`. Each control therefore supplies a **subscribe** closure
//! and a **slot count** closure that already have the payload conversion baked in. The alternative
//! — an enum listing every payload type — would have to be extended for every new event, which is
//! the per-event host work this type exists to remove.

use super::{ConnectionHandle, GenericSignal, Signal1};
use crate::compat::Box;
use crate::widget::capability::CapabilityValue;
use alloc::sync::Arc;

/// A subscriber that receives an event's payload.
///
/// A type alias rather than a bare `dyn FnMut` because it appears in three signatures and the
/// three must agree exactly for a control's `event_signal_dyn` to satisfy the trait.
pub type EventSlot = dyn FnMut(&CapabilityValue) + Send + Sync;

/// One control signal, addressed by the name its capability publishes.
///
/// Construct one through [`EventSignalRef::unit`] or [`EventSignalRef::mapped`] inside a control's
/// `event_signal_dyn`; the binder and the wiring query then work with the erased form alone.
pub struct EventSignalRef {
    /// The published name this signal answers to.
    name: &'static str,
    /// Subscribes an **owned** slot and returns the handle that removes it.
    ///
    /// The slot is owned rather than borrowed because the underlying signal stores it for the rest
    /// of its life: a `&mut dyn FnMut` would be borrowed only for the duration of this call, and a
    /// signal holding a reference to a caller's stack frame could not exist. `Box` at the boundary
    /// is what turns "borrowed for one call" into "owned until disconnected".
    ///
    /// A closure rather than a signal reference because the payload conversion has to happen here,
    /// while the concrete `T` is still a type parameter.
    #[allow(clippy::type_complexity)]
    subscribe: Box<dyn Fn(Box<EventSlot>) -> ConnectionHandle + Send + Sync>,
    /// Removes a subscription made through [`EventSignalRef::subscribe`].
    #[allow(clippy::type_complexity)]
    disconnect: Box<dyn Fn(ConnectionHandle) -> bool + Send + Sync>,
    /// How many slots are currently connected to this signal.
    ///
    /// This is what makes "is anything wired?" answerable (rule #97) without a second registry:
    /// the signal itself already knows, and a separate record of what was wired could disagree with
    /// it.
    slot_count: Box<dyn Fn() -> usize + Send + Sync>,
}

impl EventSignalRef {
    /// A signal that carries no payload.
    ///
    /// The subscriber receives [`CapabilityValue::Null`], which is the honest value for "this event
    /// reported nothing" and is the same value an absent optional payload uses.
    pub fn unit(name: &'static str, signal: &GenericSignal) -> Self {
        let for_subscribe = signal.clone();
        let for_count = signal.clone();
        let for_disconnect = signal.clone();
        Self {
            name,
            // The inner signal delivers `()`, so the conversion is the only place the payload
            // shape is decided — and it is decided once, here, rather than at every call site.
            subscribe: Box::new(move |mut slot| {
                for_subscribe.connect(move || slot(&CapabilityValue::Null))
            }),
            disconnect: Box::new(move |handle| for_disconnect.disconnect(handle)),
            slot_count: Box::new(move || for_count.slot_count()),
        }
    }

    /// A signal whose payload is converted to a [`CapabilityValue`] by `convert`.
    ///
    /// `convert` is supplied by the control and is where the concrete payload type is known:
    /// `|value: &i32| CapabilityValue::Int(*value as i64)`. Doing it here rather than in the
    /// subscriber keeps the erased form free of `Any` downcasts, which would turn a type mistake
    /// into a run-time no-op instead of a compile-time error.
    pub fn mapped<T, F>(name: &'static str, signal: &Signal1<T>, convert: F) -> Self
    where
        T: Clone + Send + 'static,
        F: Fn(&T) -> CapabilityValue + Send + Sync + Clone + 'static,
    {
        let source = signal.clone();
        let for_count = signal.clone();
        let for_disconnect = signal.clone();
        Self {
            name,
            subscribe: Box::new(move |mut slot| {
                // `convert` is cloned per subscription so the same reference can be subscribed
                // more than once; the alternative — requiring the caller to hand over ownership —
                // would make a second subscription impossible from one reference.
                let convert = convert.clone();
                source.connect(move |value: Arc<T>| slot(&convert(&value)))
            }),
            disconnect: Box::new(move |handle| for_disconnect.disconnect(handle)),
            slot_count: Box::new(move || for_count.slot_count()),
        }
    }

    /// The published name this signal answers to.
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// Subscribes `slot`, which receives the event's payload as a [`CapabilityValue`].
    ///
    /// The returned [`ConnectionHandle`] removes exactly this subscription; the caller that owns
    /// it is responsible for disconnecting, which is what lets a rebuilt control release the slots
    /// it held.
    pub fn subscribe(&self, slot: Box<EventSlot>) -> ConnectionHandle {
        (self.subscribe)(slot)
    }

    /// How many slots are currently subscribed to this signal.
    ///
    /// Zero means the event is live but nothing is listening — the difference rule #97 requires to
    /// be queryable, because `connect_event` returning `Ok` only proves the *name* is valid.
    pub fn slot_count(&self) -> usize {
        (self.slot_count)()
    }

    /// Removes the subscription `handle` refers to, reporting whether it was still connected.
    ///
    /// The counterpart of [`Self::subscribe`], and needed for the same reason: whoever subscribes
    /// through the erased form must be able to unsubscribe through it, or a rebuilt control would
    /// accumulate one live slot per rebuild.
    ///
    /// `false` means the handle belonged to a different signal or was already removed, which is the
    /// same answer the concrete signals give.
    pub fn disconnect(&self, handle: ConnectionHandle) -> bool {
        (self.disconnect)(handle)
    }
}

impl core::fmt::Debug for EventSignalRef {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // The closures are opaque; the name and the live subscriber count are what a log line can
        // act on, and they are precisely the two facts this type exists to expose.
        f.debug_struct("EventSignalRef")
            .field("name", &self.name)
            .field("slots", &self.slot_count())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::{lock, Mutex, Vec};
    use core::sync::atomic::{AtomicUsize, Ordering};

    /// A unit signal delivers `Null`, and the value is the only thing a subscriber can observe.
    #[test]
    fn a_unit_signal_delivers_null() {
        let signal = GenericSignal::new();
        let reference = EventSignalRef::unit("clicked", &signal);

        let seen = Arc::new(Mutex::new(Vec::<CapabilityValue>::new()));
        let recorder = Arc::clone(&seen);
        let _handle = reference.subscribe(Box::new(move |value| {
            lock(&recorder).push(value.clone());
        }));

        signal.emit();
        assert_eq!(*lock(&seen), vec![CapabilityValue::Null]);
    }

    /// A mapped signal delivers the control's own conversion of its payload.
    #[test]
    fn a_mapped_signal_delivers_its_converted_payload() {
        let signal: Signal1<i32> = Signal1::new();
        let reference = EventSignalRef::mapped("value_changed", &signal, |value| {
            CapabilityValue::Int(*value as i64)
        });

        let seen = Arc::new(Mutex::new(Vec::<CapabilityValue>::new()));
        let recorder = Arc::clone(&seen);
        let _handle = reference.subscribe(Box::new(move |value| {
            lock(&recorder).push(value.clone());
        }));

        signal.emit(42);
        assert_eq!(*lock(&seen), vec![CapabilityValue::Int(42)]);
    }

    /// The slot count must report the live subscriptions, which is what "is this wired?" reads.
    #[test]
    fn the_slot_count_tracks_subscriptions() {
        let signal = GenericSignal::new();
        let reference = EventSignalRef::unit("clicked", &signal);
        assert_eq!(reference.slot_count(), 0, "a fresh reference has no subscribers");

        let handle = reference.subscribe(Box::new(|_| {}));
        assert_eq!(reference.slot_count(), 1, "a subscription must be visible in the count");

        signal.disconnect(handle);
        assert_eq!(reference.slot_count(), 0, "a removed subscription must not be counted");
    }

    /// One reference must accept more than one subscription, which a designer needs when two wires
    /// leave the same event.
    #[test]
    fn a_reference_accepts_several_subscriptions() {
        let signal = GenericSignal::new();
        let reference = EventSignalRef::unit("clicked", &signal);

        let calls = Arc::new(AtomicUsize::new(0));
        let first = Arc::clone(&calls);
        let second = Arc::clone(&calls);
        let _a = reference.subscribe(Box::new(move |_| {
            first.fetch_add(1, Ordering::SeqCst);
        }));
        let _b = reference.subscribe(Box::new(move |_| {
            second.fetch_add(1, Ordering::SeqCst);
        }));

        assert_eq!(reference.slot_count(), 2);
        signal.emit();
        assert_eq!(calls.load(Ordering::SeqCst), 2, "both wires must receive the event");
    }

    /// A subscriber that disconnects itself must be honoured, which is the pre-existing contract
    /// of the underlying signal and must survive this erasure.
    #[test]
    fn a_self_disconnecting_subscriber_is_honoured() {
        let signal = GenericSignal::new();
        let reference = EventSignalRef::unit("clicked", &signal);

        let calls = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&calls);
        let holder = Arc::new(Mutex::new(None::<ConnectionHandle>));
        let slot_holder = Arc::clone(&holder);
        let slot_signal = signal.clone();

        let handle = reference.subscribe(Box::new(move |_value| {
            counter.fetch_add(1, Ordering::SeqCst);
            if let Some(own) = lock(&slot_holder).take() {
                slot_signal.disconnect(own);
            }
        }));
        *lock(&holder) = Some(handle);

        signal.emit();
        signal.emit();
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "a subscriber that removed itself must not be called again"
        );
    }
}
