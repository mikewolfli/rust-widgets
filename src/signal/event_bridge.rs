// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Forwards a control's own signal emissions to a name-addressed
//! [`CustomSignalHub`].
//!
//! # The gap this closes
//!
//! [`WidgetFactory::connect_event`](crate::widget::capability::WidgetFactory::connect_event)
//! lets a caller subscribe to an event name a capability publishes, and its tests prove
//! the subscription is validated and delivers. What they do **not** prove — because it is
//! not true of `connect_event` alone — is that the *control* reaches that subscriber.
//! `connect_event` registers a slot on a hub; the control emits its own typed signals
//! (`Button`'s `clicked`, `Slider`'s `value_changed`). Nothing joined the two, so a
//! subscriber validated against a published name would still never be called by a real
//! control.
//!
//! # Why this is a binder and not automatic
//!
//! A control's signals are **typed**: `clicked` is `GenericSignal`, `value_changed` is
//! `Signal1<i32>`, `toggled` is `Signal1<bool>`. A hub name is untyped
//! (`CustomSignalHub::emit(name)` carries no value). Bridging them therefore requires a
//! per-event decision about what happens to the payload, and that decision belongs to the
//! caller: one that does not use the values does not pay for them, and one that does
//! supplies the mapping rather than receiving a lossy default.
//!
//! This type is the reusable half: it owns the subscriptions and removes them together,
//! so a dropped binder leaves nothing behind.
//!
//! # What a control author has to do
//!
//! One call per published event:
//!
//! ```ignore
//! let mut binder = EventSignalBinder::new(hub);
//! binder.forward_unit("clicked", &self.base.clicked);
//! binder.forward_mapped("value_changed", &self.value_changed, |_| {});
//! ```
//!
//! Exhaustiveness is stated by `tests/event_signal_bridge_test.rs`, which requires every
//! name a capability publishes to be forwardable, so an event added to a capability
//! without a forwarding site is caught rather than silently never firing.

use super::CustomSignalHub;
use crate::signal::{ConnectionHandle, GenericSignal, Signal1};
use alloc::boxed::Box;
use alloc::sync::Arc;

/// A set of hub subscriptions that forwards control signals, removed together.
///
/// The subscription list is kept so dropping the binder unsubscribes everything it added.
/// Without it, a rebuilt control would gain a subscriber per rebuild — the leak a
/// "connect and forget" binding produces.
#[derive(Default)]
pub struct EventSignalBinder {
    /// `Arc`, not `CustomSignalHub`, because the hub's state is a `Mutex` and it is
    /// therefore not `Clone`. A slot must own a handle to emit through, so the shared
    /// handle is an `Arc` — the same shape every other signal in this module uses.
    hub: Option<Arc<CustomSignalHub>>,
    forwards: alloc::vec::Vec<Forwarded>,
}

impl EventSignalBinder {
    /// Creates an empty binder that forwards into `hub`.
    pub fn new(hub: Arc<CustomSignalHub>) -> Self {
        Self { hub: Some(hub), forwards: alloc::vec::Vec::new() }
    }

    /// Creates a binder with nowhere to forward, so the forwarding calls are no-ops.
    ///
    /// Used by a control constructed without an application hub. The alternative —
    /// forcing every caller to supply one — would make the hub a mandatory part of every
    /// constructor, which is the global state this design avoids.
    pub fn detached() -> Self {
        Self { hub: None, forwards: alloc::vec::Vec::new() }
    }

    /// Reports whether this binder forwards into a hub.
    pub fn is_attached(&self) -> bool {
        self.hub.is_some()
    }

    /// Number of subscriptions this binder owns.
    pub fn len(&self) -> usize {
        self.forwards.len()
    }

    /// Reports whether no subscription has been registered.
    pub fn is_empty(&self) -> bool {
        self.forwards.is_empty()
    }

    /// Forwards a payload-free signal to the hub under `event_name`.
    ///
    /// The typical case: `clicked`, `dismissed` — the event carries the fact that it
    /// happened, and the name carries the rest.
    pub fn forward_unit(&mut self, event_name: &str, signal: &GenericSignal) {
        let Some(hub) = self.hub.clone() else {
            return;
        };
        // The slot outlives this call, so it must own the name rather than borrow it.
        let name = alloc::string::String::from(event_name);
        let handle = signal.connect(move || hub.emit(&name));
        self.forwards.push(Forwarded { source: ForwardSource::Unit(signal.clone()), handle });
    }

    /// Forwards a signal whose payload the hub cannot carry, running `observe` first.
    ///
    /// # Why the payload needs an explicit destination
    ///
    /// `CustomSignalHub::emit(name)` is untyped, so a `Signal1<T>` payload has nowhere to
    /// go. Silently dropping it would make the bridge lossy in a way the caller cannot
    /// see; requiring `observe` makes the loss explicit and hands over the value in the
    /// same call. A caller with no use for it passes `|_| {}` and has *said* so, rather
    /// than having it decided for them.
    pub fn forward_mapped<T, F>(&mut self, event_name: &str, signal: &Signal1<T>, mut observe: F)
    where
        T: Clone + Send + 'static,
        F: FnMut(&T) + Send + Sync + 'static,
    {
        let Some(hub) = self.hub.clone() else {
            return;
        };
        let name = alloc::string::String::from(event_name);
        let handle = signal.connect(move |value: Arc<T>| {
            observe(&value);
            hub.emit(&name);
        });
        // `Signal<T>` is generic, so its disconnect cannot be stored type-erased the way
        // `GenericSignal`'s can — the closure is built here, while `T` is still known.
        let source = signal.clone();
        self.forwards.push(Forwarded {
            source: ForwardSource::Typed(Box::new(move |handle| {
                source.disconnect(handle);
            })),
            handle,
        });
    }

    /// Removes every subscription this binder registered, leaving the binder reusable.
    ///
    /// Safe to call more than once. The hub is retained, so a later `forward_*` attaches
    /// again — which is what a control that rebuilds its internals needs.
    ///
    /// # Why the *source* signal is remembered, not the hub
    ///
    /// The handle `forward_*` returns belongs to the **control's** signal. An earlier
    /// revision called `hub.disconnect(name, handle)` and therefore disconnected nothing:
    /// the handle is not a key in the hub, the call returned `false`, and every forwarded
    /// slot leaked — a rebuilt control would accumulate one live subscriber per rebuild.
    /// `dropping_the_binder_unsubscribes` caught it, and the fix is to hold the signal the
    /// handle came from.
    pub fn unbind_all(&mut self) {
        for entry in core::mem::take(&mut self.forwards) {
            entry.source.disconnect(entry.handle);
        }
    }
}

/// How to remove a forwarded slot, captured beside the handle it removes.
///
/// Two variants because `GenericSignal` is a concrete type while `Signal1<T>` is generic:
/// the typed case stores a disconnect closure rather than the signal, erasing `T` at the
/// one point where it is still known.
enum ForwardSource {
    /// A payload-free signal, kept directly.
    Unit(GenericSignal),
    /// A payload-carrying signal, erased to a disconnect closure.
    Typed(Box<dyn Fn(ConnectionHandle) + Send>),
}

impl ForwardSource {
    fn disconnect(&self, handle: ConnectionHandle) {
        match self {
            Self::Unit(signal) => {
                signal.disconnect(handle);
            }
            Self::Typed(disconnect) => disconnect(handle),
        }
    }
}

/// One forwarded event: the signal it came from and the handle that removes it.
struct Forwarded {
    source: ForwardSource,
    handle: ConnectionHandle,
}

impl core::fmt::Debug for EventSignalBinder {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // The handles are opaque and the hub is shared state; reporting the shape is what
        // a caller reading a log line can act on.
        f.debug_struct("EventSignalBinder")
            .field("attached", &self.is_attached())
            .field("subscriptions", &self.forwards.len())
            .finish()
    }
}

impl Drop for EventSignalBinder {
    fn drop(&mut self) {
        // A binder that outlives its control must not leave slots pointing into it.
        self.unbind_all();
    }
}
