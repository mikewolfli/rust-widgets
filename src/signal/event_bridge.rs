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
//! # Wiring status (read this before assuming an event fires)
//!
//! **No control in this library wires itself to a hub.** Signals and the hub are two
//! separate worlds by design: a control owns its typed signals and knows nothing
//! about an application-level name space, and a hub knows nothing about controls. The
//! join is the *host's* job, performed at mount time.
//!
//! `EventSignalBinder` is the mechanism for that join, and
//! `tests/event_signal_bridge_test.rs` is its proof — but that test performs the
//! wiring itself. It proves the binder **works**, not that this crate already did the
//! wiring for any control. Two consequences a caller must know:
//!
//! * `WidgetFactory::connect_event` returns `Ok` for any published name whether or not
//!   anything is wired to it, because "is this name valid?" and "is something
//!   emitting it?" are different questions. Subscribing therefore cannot fail just
//!   because a host skipped the wiring step — an event that is valid but unwired is
//!   indistinguishable from an event that never occurs.
//! * `EventSignalBinder::detached()` exists for a control built without a hub; its
//!   forward calls are documented no-ops, so a host that uses it has opted out
//!   explicitly rather than silently.
//!
//! [`crate::signal::EventSignalBinder::forward_widget_events`] is the host-facing
//! convenience entry point: one call per control, wiring the one name every `Widget` is
//! guaranteed to have — the base `clicked` signal. It does **not** cover a control's
//! other published names (`value_changed` is `Signal1<i32>`, `toggled` is `Signal1<bool>`),
//! which need a per-event payload decision and so are wired with `forward_mapped` at the
//! control's own construction site. The count it returns is the number of names it wired,
//! so a caller that receives `0` can see the control contributed nothing instead of
//! assuming it did — this doc previously said it covered "every name that control's
//! capability publishes", which the implementation never did and which would have led a
//! host to believe the remaining events were wired for it.
//!
//! Exhaustiveness of the *subscribable* side is stated by
//! `tests/event_signal_bridge_test.rs`'s `every_published_event_accepts_a_forwarding_call`,
//! which requires every name a capability publishes to be accepted by `forward_unit`/`forward_mapped`
//! (i.e. to have a hub destination). That is a real guarantee, but it is narrower than it sounds:
//! it proves a forwarding call **can** be made for each published name, not that this crate
//! **does** make one. Nothing here wires a control's non-`clicked` events automatically —
//! see "Wiring status" above — so a control whose events are never forwarded by its host still
//! has published names that are valid and inert.

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

    /// Wires **every** event a mounted control publishes, in one call.
    ///
    /// # Why this is the entry point a designer needs
    ///
    /// A designer's wires are decided at run time: the user drew a line from `value_changed`, and
    /// the program learns which name that was when the project is loaded. So the host cannot be
    /// asked to write `forward_unit("clicked", ..)` / `forward_mapped("value_changed", ..)` per
    /// control — those lines would have to be generated for wires that do not exist yet, which is
    /// exactly the host work rule #98 rules out.
    ///
    /// This walks the control's *published* events instead, asking the control for each one's signal
    /// through [`Widget::event_signal_dyn`], and subscribes to all of them. A converted control
    /// therefore needs **one** line at its mount site, and every name its capability offers in the
    /// panel **is** wired.
    ///
    /// # What happens to the payload
    ///
    /// The hub carries names, not values, so the slot forwards the name and drops the value — the
    /// same loss [`Self::forward_unit`] makes, stated here rather than left implicit. A consumer
    /// that needs the value subscribes through `connect_event` (which reaches the same slot) and
    /// reads it from the binding layer's event object.
    ///
    /// # Returns
    ///
    /// The number of events wired. A control that has not been converted yet reports `0`, so a
    /// caller can see that it contributed nothing instead of assuming it did.
    ///
    /// # Relationship to [`Self::forward_widget_events`]
    ///
    /// That method is the earlier, weaker version: it wires `clicked` and nothing else, because when
    /// it was written a control had no way to name its other signals. This one supersedes it for a
    /// call that is choosing between the two; the narrower method remains so an existing caller
    /// keeps working.
    ///
    /// [`Widget::event_signal_dyn`]: crate::widget::Widget::event_signal_dyn
    pub fn forward_all<W>(&mut self, widget: &W) -> usize
    where
        W: crate::widget::Widget,
    {
        // A stripped profile compiles the capability table out, so there is no published name list
        // to walk. `0` is the honest answer: the method reports how many events it wired.
        #[cfg(full_widgets)]
        {
            let factory = crate::widget::capability::WidgetFactory::new_with_defaults();
            let Some(capability) = factory.capability_for_kind_instance(widget) else {
                return 0;
            };
            let mut wired = 0usize;
            for schema in capability.events {
                // A control that publishes a name it cannot resolve would otherwise be wired
                // silently short. Treating it as zero keeps the return value honest, and
                // `tools/check_event_signal_dyn.sh` fails the build-time counterpart of this case.
                if self.wire_one(widget, schema.name) {
                    wired += 1;
                }
            }
            wired
        }
        #[cfg(not(full_widgets))]
        {
            let _ = widget;
            0
        }
    }

    /// Wires one published event of `widget`, reporting whether it was wired.
    ///
    /// The single-event form of [`Self::forward_all`], for a caller that wants to skip a name (a
    /// designer with one hand-made exception) or to check a name without wiring the rest.
    pub fn forward_one<W>(&mut self, widget: &W, event_name: &str) -> bool
    where
        W: crate::widget::Widget,
    {
        self.wire_one(widget, event_name)
    }

    /// The shared body of [`Self::forward_all`] and [`Self::forward_one`].
    ///
    /// Keeping the two entry points on one implementation is what stops the single-event form from
    /// drifting from the all-events form: a fix to how a name is resolved is a fix to both.
    fn wire_one<W>(&mut self, widget: &W, event_name: &str) -> bool
    where
        W: crate::widget::Widget,
    {
        let Some(reference) = widget.event_signal_dyn(event_name) else {
            return false;
        };
        let Some(hub) = self.hub.clone() else {
            // A detached binder registers nothing; it is documented as a no-op, and reporting
            // `true` here would claim a wire that no slot backs.
            return false;
        };
        // The slot owns the name because it outlives this call.
        let name = alloc::string::String::from(event_name);
        let handle = reference.subscribe(alloc::boxed::Box::new(move |_value| hub.emit(&name)));

        // `EventSignalRef` is not `Clone` (its closures are not), so the disconnect closure resolves
        // the signal a second time from the widget. That lookup is cheap and — unlike making the
        // type cloneable — cannot leave two copies of a slot count that disagree.
        let source = widget.event_signal_dyn(event_name);
        self.forwards.push(Forwarded {
            source: ForwardSource::Erased(alloc::boxed::Box::new(move |handle| {
                // The boolean is discarded because `ForwardSource::disconnect` reports nothing; it
                // is here so the closure's return type matches the shared alias.
                let _ = source.as_ref().is_some_and(|reference| reference.disconnect(handle));
            })),
            handle,
        });
        true
    }

    /// Reports whether any subscriber reaches `widget`'s `event_name`.
    ///
    /// # Why this is the query rule #97 requires
    ///
    /// `WidgetFactory::connect_event` returns `Ok` for a published name whether or not anything was
    /// ever wired to it: "is this name valid?" and "is something emitting it?" are different
    /// questions. Until this method existed, the difference was **unaskable**, which made
    /// "subscribed successfully but never called" a silent failure with no way to detect it.
    ///
    /// `false` means exactly that: the name is valid (a caller should not receive `false` for a name
    /// the control does not publish — see [`Self::forward_all`]'s return value for that case) but no
    /// wire reaches it.
    pub fn event_is_wired<W>(&self, widget: &W, event_name: &str) -> bool
    where
        W: crate::widget::Widget,
    {
        match widget.event_signal_dyn(event_name) {
            // A subscriber count on the signal itself, so this cannot report wired when the slot was
            // dropped, nor unwired when a slot another binder added is live.
            Some(reference) => reference.slot_count() > 0,
            None => false,
        }
    }

    /// Wires every published event a mounted widget can actually emit.
    ///
    /// # Why this exists
    ///
    /// [`WidgetFactory::connect_event`](crate::widget::capability::WidgetFactory::connect_event)
    /// validates a name against the capability table and registers a slot. It cannot
    /// check that anything emits that name, because a control's typed signals and the
    /// hub's names are separate worlds until a binder joins them. A host that only
    /// called `connect_event` therefore got a subscriber that was never invoked.
    ///
    /// This is that join, in one call: it connects the widget's own `clicked` signal
    /// to the hub under the names the widget's capability publishes, and owns the
    /// subscriptions so they are released with the binder.
    ///
    /// # Which names it wires
    ///
    /// Only the names backed by a signal **every** `Widget` has — the base
    /// `clicked` signal. A control's other events (`value_changed`, `toggled`, …) are
    /// typed `Signal1<T>` and live on the concrete type, so bridging them needs a
    /// payload decision this helper cannot make generically; use [`Self::forward_mapped`]
    /// at the control's own construction site for those. Returning the number wired
    /// lets a caller see that a widget contributed nothing rather than assuming it did.
    ///
    /// # Usage
    ///
    /// Call once per mounted widget, right after it is registered:
    ///
    /// ```ignore
    /// let mut binder = EventSignalBinder::new(hub);
    /// binder.forward_widget_events(widget.as_ref());
    /// ```
    pub fn forward_widget_events<W>(&mut self, widget: &W) -> usize
    where
        W: crate::widget::Widget,
    {
        // The capability registry is compiled out of a stripped profile, so there is no
        // published name list to consult there. Returning `0` is the honest answer: the
        // helper is documented as reporting how many events it wired.
        #[cfg(full_widgets)]
        {
            let factory = crate::widget::capability::WidgetFactory::new_with_defaults();
            let Some(capability) = factory.capability_for_kind_instance(widget) else {
                return 0;
            };
            if !capability.events.iter().any(|schema| schema.name == "clicked") {
                return 0;
            }
            // `clicked_signal` is a `Widget` trait method with a default body that reads
            // the base signal, so it is available on every control without naming the
            // concrete type.
            self.forward_unit("clicked", widget.clicked_signal());
            1
        }
        #[cfg(not(full_widgets))]
        {
            let _ = widget;
            0
        }
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
    /// A signal resolved by name, erased by the control itself.
    ///
    /// Separate from `Typed` because the handle does not come from a signal this module can name:
    /// it came from `EventSignalRef`, whose whole purpose is to hide which concrete signal it is.
    Erased(Box<dyn Fn(ConnectionHandle) + Send>),
}

impl ForwardSource {
    fn disconnect(&self, handle: ConnectionHandle) {
        match self {
            Self::Unit(signal) => {
                signal.disconnect(handle);
            }
            Self::Typed(disconnect) => disconnect(handle),
            Self::Erased(disconnect) => disconnect(handle),
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
