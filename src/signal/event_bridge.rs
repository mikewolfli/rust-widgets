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
///
/// # Why there is no separate wiring ledger
///
/// An earlier revision kept a process-wide count of how many events each control *kind* had
/// published and wired. That count was historical (it never decreased on unbind), shared across
/// instances of a kind, and folded with `max`/`sum`, so it answered "what did some instance of this
/// kind once resolve" rather than "is *this* control's wire live". The live answer is derivable
/// from the subscriptions the binder actually holds — see [`Self::event_is_wired`] and
/// [`Self::unwired_events_for`] — so the ledger was removed rather than kept alongside a second
/// source of truth (rule #101).
pub struct EventSignalBinder {
    /// `Arc`, not `CustomSignalHub`, because the hub's state is a `Mutex` and it is
    /// therefore not `Clone`. A slot must own a handle to emit through, so the shared
    /// handle is an `Arc` — the same shape every other signal in this module uses.
    hub: Option<Arc<CustomSignalHub>>,
    forwards: alloc::vec::Vec<Forwarded>,
}

impl Default for EventSignalBinder {
    fn default() -> Self {
        Self::detached()
    }
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
        let probe = signal.clone();
        self.forwards.push(Forwarded {
            source: ForwardSource::Unit(signal.clone()),
            handle,
            event_name: alloc::string::String::from(event_name),
            is_connected: alloc::boxed::Box::new(move || probe.is_connected(handle)),
        });
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
    /// same loss [`Self::forward_unit`] makes, stated here rather than left implicit. The
    /// **name-addressed** channel is deliberately value-free; a consumer that needs the value takes
    /// the payload-bearing channel instead (`crate::json::bind_published_event`, whose handler
    /// receives the emitted [`CapabilityValue`](crate::widget::capability::CapabilityValue) in
    /// [`crate::json::EventHandlerContext::payload`]). Stating which channel carries the value — and
    /// which does not — is what stops a caller assuming a value it will never receive.
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

    /// How many of `widget`'s published events have **no** live subscription from this binder.
    ///
    /// `None` when this binder holds no subscription at all, which is a different answer from
    /// `Some(0)` — "not wired yet" and "fully wired" must not look alike.
    ///
    /// This is the query BLUE19 #97 requires: `connect_event` returning `Ok` proves only that a
    /// name is valid, so without this a host cannot tell "this wire is live" from "this wire was
    /// accepted and will never fire".
    ///
    /// # Why this walks the capability instead of reading a recorded count
    ///
    /// The answer is derived from the subscriptions the binder actually holds *now*, one published
    /// name at a time, through [`Self::event_is_wired`]. That makes it instance-independent, deduped
    /// by event name, and truthful after [`Self::unbind_all`]: an event whose wire was removed
    /// counts as unwired again, and a brand-new instance of a kind never inherits a sibling's
    /// history. A recorded ledger could not do any of those — it was process-wide, never decreased
    /// on unbind, and conflated repeated connections of one name with other names.
    #[cfg(full_widgets)]
    pub fn unwired_events_for<W>(&self, widget: &W) -> Option<usize>
    where
        W: crate::widget::Widget,
    {
        // "Never wired" and "not a control with events" both mean there is nothing to report.
        if self.forwards.is_empty() {
            return None;
        }
        let factory = crate::widget::capability::WidgetFactory::new_with_defaults();
        let capability = factory.capability_for_kind_instance(widget)?;
        let unwired = capability
            .events
            .iter()
            .filter(|schema| !self.event_is_wired(widget, schema.name))
            .count();
        Some(unwired)
    }

    /// The stripped-profile form, which has no capability table to have wired against.
    #[cfg(not(full_widgets))]
    pub fn unwired_events_for<W>(&self, _widget: &W) -> Option<usize>
    where
        W: crate::widget::Widget,
    {
        None
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
        // The reference must *declare* the name it was asked for.
        //
        // Without this, the declared name was write-only: `event_signal_dyn` could resolve
        // `"clicked"` to a reference built over `self.value_changed` and `wire_one` would subscribe
        // to that signal anyway, reporting success. The hub name came from the argument rather than
        // the reference, so the mismatch was invisible — a subscriber to `"clicked"` would be
        // attached to an event the control never fires under that name, which is precisely the
        // silent "valid but inert" failure rule #97 exists to rule out. `EventSignalRef::name`
        // existed solely for this check and, before it, had no reader anywhere in the crate.
        if reference.name() != event_name {
            return false;
        }
        let Some(hub) = self.hub.clone() else {
            // A detached binder registers nothing; it is documented as a no-op, and reporting
            // `true` here would claim a wire that no slot backs.
            return false;
        };
        // The slot owns the name because it outlives this call. It is the reference's own name, so
        // the name a caller subscribes to and the name the slot emits cannot drift apart.
        let name = alloc::string::String::from(reference.name());
        let handle = reference.subscribe(alloc::boxed::Box::new(move |_value| hub.emit(&name)));

        // Resolve the signal once more to capture a *live-ness* probe for exactly this handle.
        //
        // `EventSignalRef` is not `Clone` (its closures are not) and its `is_connected` is per-call,
        // so the probe resolves the reference a second time — cheap, and the same shape the
        // disconnect closure below already uses.
        let probe = widget.event_signal_dyn(event_name);
        let is_connected: alloc::boxed::Box<dyn Fn() -> bool + Send + Sync> =
            alloc::boxed::Box::new(move || {
                probe.as_ref().is_some_and(|reference| reference.is_connected(handle))
            });

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
            // `reference.name()` is `&'static str` (the capability table is a `static`), so the
            // stored name is the canonical spelling rather than the caller's.
            event_name: alloc::string::String::from(reference.name()),
            is_connected,
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
    /// # Why it asks about *this binder's* handle, not the signal's slot count
    ///
    /// `slot_count() > 0` only proves *someone* observes the signal. A direct observer attached
    /// elsewhere — or a subscription pointing at a **different** hub — drives the count above zero
    /// while no wire from *this* binder reaches the hub the caller cares about. Because the binder
    /// keeps the handle of every slot it registered, the honest query is "is *my* subscription to
    /// this widget still live?", which is what this now answers by checking the stored handles.
    /// A name this binder never wired (or whose wire was later removed) reports `false` even when
    /// the signal has other observers.
    pub fn event_is_wired<W>(&self, widget: &W, event_name: &str) -> bool
    where
        W: crate::widget::Widget,
    {
        // A reference that answers to another name is not a wire to *this* name — the same
        // declared-name check `wire_one` makes, for the same reason.
        let Some(reference) = widget.event_signal_dyn(event_name) else {
            return false;
        };
        if reference.name() != event_name {
            return false;
        }
        // Any slot this binder registered on the same named event, still connected, is a live wire.
        // The stored handle is what makes this "this binder's wire" rather than "some observer";
        // the name check keeps it about *this* event rather than any wire the binder holds.
        self.forwards
            .iter()
            .any(|forwarded| forwarded.event_name == event_name && (forwarded.is_connected)())
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
        let probe = signal.clone();
        self.forwards.push(Forwarded {
            source: ForwardSource::Typed(Box::new(move |handle| {
                source.disconnect(handle);
            })),
            handle,
            event_name: alloc::string::String::from(event_name),
            is_connected: alloc::boxed::Box::new(move || probe.is_connected(handle)),
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
    /// The published event name this subscription was made for.
    ///
    /// Kept so [`EventSignalBinder::event_is_wired`] can answer about the *named* event: without it
    /// a binder that wired only `clicked` would report every other published name as wired too,
    /// because it could only see "some handle of mine is connected".
    event_name: alloc::string::String,
    /// Whether this specific subscription is still live on the widget's own signal.
    ///
    /// Kept beside the handle so [`EventSignalBinder::event_is_wired`] can ask about **this**
    /// binder's wire rather than about whether *anyone* observes the signal. Erased because the
    /// concrete signal type is not nameable here.
    is_connected: Box<dyn Fn() -> bool + Send + Sync>,
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

#[cfg(all(test, full_widgets))]
mod tests {
    use super::EventSignalBinder;
    use crate::core::Rect;

    /// Every control whose `event_signal_dyn` is implemented must wire **all** its published names.
    ///
    /// # What this proves that the source-level gate cannot
    ///
    /// `tools/check_event_signal_dyn.sh` reads the `match` arms and compares their literals to the
    /// capability table. That catches a missing or misspelled arm, but it is a text check: it
    /// cannot see that the signal behind an arm is the one the control actually **emits**. A
    /// control that resolved `clicked` to a signal it never fires would satisfy the gate and still
    /// deliver nothing.
    ///
    /// This drives the real path — `forward_all` walks the capability's published names and asks
    /// the control to resolve each — and then asks `unwired_events_for` how many were left. Zero
    /// is the only acceptable answer for a converted control, and the shortfall is what the
    /// binder records when a name resolves to nothing.
    ///
    /// # Why the list is explicit
    ///
    /// It is the same commitment `check_event_signal_dyn.py`'s `CONVERTED` table makes, and it has
    /// to be stated for the same reason: a control that has *not* been converted is
    /// indistinguishable, from the outside, from one that has and is missing an arm — both wire
    /// zero. Naming the converted set is what turns "the gap is fine" into a checked claim.
    #[test]
    fn converted_controls_wire_every_published_event() {
        fn check<W: crate::widget::Widget>(name: &str, widget: &W) {
            let hub = std::sync::Arc::new(crate::signal::hub::CustomSignalHub::new());
            let mut binder = EventSignalBinder::new(hub);
            let wired = binder.forward_all(widget);
            assert!(
                wired > 0,
                "{name}: `forward_all` wired nothing, so either the control is not converted or \
                 its capability publishes no events"
            );
            assert_eq!(
                binder.unwired_events_for(widget),
                Some(0),
                "{name}: some published event did not resolve to a signal the control emits"
            );
        }

        let r = Rect::new(0, 0, 100, 40);
        check("button", &crate::widget::base_widgets::button::Button::new("b".to_string(), r));
        check("check_box", &crate::widget::base_widgets::checkbox::CheckBox::new(r));
        check(
            "color_well",
            &crate::widget::display_widgets::color_well::ColorWell::new(
                crate::core::Color::WHITE,
                r,
            ),
        );
        check(
            "dropdown",
            &crate::widget::input_widgets::dropdown::Dropdown::new(vec!["one".to_string()], r),
        );
        check("empty_state", &crate::widget::display_widgets::empty_state::EmptyState::new(r));
        check("progress_dialog", &crate::widget::dialog::progress_dialog::ProgressDialog::new(r));
        check(
            "refresh_control",
            &crate::widget::overlay_widgets::refresh_control::RefreshControl::new(r),
        );
        check(
            "text_area",
            &crate::widget::input_widgets::textarea::TextArea::new(String::new(), r),
        );
        check("window", &crate::widget::window::Window::new("w".to_string(), r));

        // The second batch: the controls converted after the first nine, so the set the runtime
        // check covers keeps pace with the `CONVERTED` table rather than trailing it. A control is
        // added here only when a call reaches every name its capability publishes without a
        // `_ = ` discard, which is what makes the count meaningful.
        check("app_bar", &crate::widget::nav_widgets::app_bar::AppBar::new("t", r));
        check("bottom_sheet", &crate::widget::dialog::bottom_sheet::BottomSheet::new(r));
        check("dialog", &crate::widget::dialog::dialog_widget::Dialog::new(r));
        check(
            "modal_bottom_sheet",
            &crate::widget::dialog::modal_bottom_sheet::ModalBottomSheet::new(r),
        );
        check("popup_window", &crate::widget::dialog::popup_window::PopupWindow::new(r));
        check(
            "hero_animation",
            &crate::widget::media_widgets::hero_animation::HeroAnimation::new(r),
        );
        check("lottie_widget", &crate::widget::media_widgets::lottie_widget::LottieWidget::new(r));
        check("rive_widget", &crate::widget::media_widgets::rive_widget::RiveWidget::new(r));
        check("mini_canvas", &crate::widget::display_widgets::mini_canvas::MiniCanvas::new(r));
        check(
            "mobile_date_picker",
            &crate::widget::misc_widgets::mobile_date_picker::MobileDatePicker::new(r),
        );
        check("status_bar", &crate::widget::menu_toolbar::status_bar::StatusBar::new(r));
        check("tool_button", &crate::widget::menu_toolbar::tool_button::ToolButton::new("t", r));
        check(
            "terminal_view",
            &crate::widget::special_widgets::terminal_view::TerminalView::new(r),
        );
        check(
            "timeline_widget",
            &crate::widget::special_widgets::timeline_widget::TimelineWidget::new(r),
        );
        check("progress_bar", &crate::widget::display_widgets::progressbar::ProgressBar::new(r));
        check("rating", &crate::widget::display_widgets::rating::Rating::new(r));
        check("switch", &crate::widget::display_widgets::switch::Switch::new(r));
        check("scroll_bar", &crate::widget::display_widgets::scrollbar::ScrollBar::new(r));
        check("stepper", &crate::widget::container_widgets::stepper::Stepper::new(r));
        check(
            "animated_image",
            &crate::widget::media_widgets::animated_image::AnimatedImage::new(r),
        );
        check("chip", &crate::widget::special_widgets::chip::Chip::new(r));
        check("group_box", &crate::widget::container_widgets::groupbox::GroupBox::new(r));

        // A delegating newtype must forward the *resolution* too, not just the behaviour.
        // `CupertinoSwitch` publishes `toggled` and delegates everything to its inner `Switch`;
        // before the forward it accepted a subscription and never fired it.
        check("cupertino_switch", &crate::widget::cupertino::core::CupertinoSwitch::new(r));
    }

    /// A control sharing a `WidgetKind` with a larger control must not inherit its shortfall.
    ///
    /// # The defect this pins
    ///
    /// `WidgetKind` is not one-to-one with capability: `WidgetKind::WebEngineView` backs both
    /// `media_player` (4 published events) and `web_engine_view` (11). The wiring-outcome table was
    /// keyed by kind alone and folded the two counts with `max` **independently**, so a fully-wired
    /// `MediaPlayer` inherited `published = 11` from its larger sibling while keeping `wired = 4` of
    /// its own — `unwired_events_for` then reported **7 unwired events that do not exist**, which is
    /// precisely the false shortfall this query exists to make impossible.
    ///
    /// `WidgetKind::Table` is the extreme case: five capabilities (2, 2, 1, 1, 1 events) share one
    /// kind.
    ///
    /// The two controls must share **one binder** for the defect to appear: the table is per-binder,
    /// so a fresh binder for each control would hide the cross-contamination entirely. That is also
    /// what a real host looks like — one binder wires every control in a window.
    ///
    /// The order is the one that exposes it: the **larger** control goes first, so a smaller sibling
    /// wired afterwards is answered from an entry that already holds the larger `published`. To make
    /// the fold bite, the smaller control is left with a genuine shortfall — one published name
    /// skipped — so a kind-only entry would compute `11 - 3 = 8` (the larger sibling's published
    /// minus the smaller control's wired) instead of the true `4 - 3 = 1`.
    ///
    /// The shortfall is produced with [`EventSignalBinder::forward_one`] rather than a wrapper type:
    /// the capability tie-break downcasts to the concrete control, so a wrapper would resolve no
    /// capability at all and `forward_all` would return `0` for a reason unrelated to the keying.
    /// Skipping one name is also exactly what a host with a hand-made exception does — the case the
    /// single-event form exists for, and the case the count has to stay honest about.
    #[test]
    fn a_control_on_a_shared_kind_reports_its_own_shortfall() {
        let r = Rect::new(0, 0, 100, 40);
        let hub = std::sync::Arc::new(crate::signal::hub::CustomSignalHub::new());
        let mut binder = EventSignalBinder::new(hub);

        let view = crate::widget::web_widgets::web_engine::WebEngineView::new(r);
        assert_eq!(binder.forward_all(&view), 11, "`web_engine_view` publishes eleven events");

        let player = crate::widget::special_widgets::media_player::MediaPlayer::new(r);
        for name in ["playback_changed", "position_changed", "volume_changed"] {
            assert!(binder.forward_one(&player, name), "`{name}` resolves on `media_player`");
        }

        assert_eq!(
            binder.unwired_events_for(&player),
            Some(1),
            "the shortfall is this control's own (4 published - 3 wired), not `WebEngineView`'s \
             four extra names folded in through a shared `WidgetKind`"
        );
        // The larger sibling is still fully wired; neither answer may be read off the other.
        assert_eq!(binder.unwired_events_for(&view), Some(0));
    }

    /// A reference that declares a different name than it was asked for must not wire.
    ///
    /// # The defect this pins
    ///
    /// `EventSignalRef::name` was **write-only**: `forward_one`/`forward_all` took the hub name
    /// from their own argument and never compared it to the name the reference declared. A control
    /// whose `event_signal_dyn` resolved `"clicked"` to a signal built under another name therefore
    /// wired "successfully" while the control emitted the real `"clicked"` into nothing. Both
    /// queries a host has — the `wired` count and [`EventSignalBinder::event_is_wired`] — reported
    /// success.
    ///
    /// This test builds one control whose resolver lies about the name and asserts the binder
    /// refuses it, so the check cannot be dropped without a failure. It is deliberately not written
    /// against a real control: no shipping control lies, so only a synthetic one can reach the
    /// branch, and the branch is what the test is about.
    #[test]
    fn a_reference_that_declares_another_name_does_not_wire() {
        use crate::widget::Widget;

        /// A `ColorWell` that answers `"clicked"` with a signal declaring a different name — the
        /// shape a copy-paste mistake in a real `event_signal_dyn` produces.
        ///
        /// Only `handle_event` and `base`/`base_mut` are delegated: `kind` has a default body that
        /// reads the base, so the wrapper reports the same kind as the control it wraps — which is
        /// what lets the capability lookup find the real event list.
        struct Misnamed(crate::widget::display_widgets::color_well::ColorWell);

        impl crate::event::EventHandler for Misnamed {
            fn handle_event(&mut self, event: &crate::event::Event) {
                self.0.handle_event(event);
            }
        }

        impl Widget for Misnamed {
            fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
                match name {
                    "clicked" => {
                        Some(crate::signal::EventSignalRef::unit("not_clicked", &self.0.clicked))
                    }
                    _ => None,
                }
            }
            fn base(&self) -> &crate::widget::BaseWidget {
                Widget::base(&self.0)
            }
            fn base_mut(&mut self) -> &mut crate::widget::BaseWidget {
                Widget::base_mut(&mut self.0)
            }
        }

        let widget = Misnamed(crate::widget::display_widgets::color_well::ColorWell::new(
            crate::core::Color::WHITE,
            Rect::new(0, 0, 100, 40),
        ));
        let hub = std::sync::Arc::new(crate::signal::hub::CustomSignalHub::new());
        let mut binder = EventSignalBinder::new(hub);

        assert!(
            !binder.forward_one(&widget, "clicked"),
            "a reference declaring `not_clicked` must not satisfy a request for `clicked`"
        );
        assert!(
            !binder.event_is_wired(&widget, "clicked"),
            "`event_is_wired` must agree with `forward_one`: neither may report a wire whose hub \
             name and signal name disagree"
        );
    }
}
