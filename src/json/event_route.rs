// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The **published-name** event route for JSON-declared layouts.
//!
//! # Why this module exists (BLUE19 rule #101)
//!
//! `src/json/loader.rs` grew a second, independent event path: eight hard-coded keys
//! (`on_click`, `on_change`, `on_close`, `on_double_click`, `on_focus`, `on_blur`,
//! `on_selection_changed`, `on_value_changed`) that it matched by hand. The capability table
//! publishes **186** event names. Measured, the two sets do not intersect at the level that
//! matters: `on_click` is not a published name (`clicked` is), and adding a published event
//! never made the JSON side support it. Nothing covered that path — `grep -rn "on_click" tools/`
//! returned nothing — so it could drift silently, and it did.
//!
//! This module is the merge. A node declares handlers against the **published name**:
//!
//! ```json
//! { "button": { "text": "Go", "events": { "clicked": "on_go" } } }
//! ```
//!
//! # Division of labour between the two spellings
//!
//! The two keys are not duplicates of one concept, and keeping both is only defensible if each
//! has a job the other cannot do. The boundary is:
//!
//! | key | what it is | why it exists |
//! |---|---|---|
//! | `events: { "<published_name>": "<handler>" }` | the published route | a name the capability table validates; adding a published event makes it declarable with no loader edit |
//! | `on_*` | the compatibility route | the same handler reached through a **marker**: `on_close` means `Closed`, `on_selection_changed` means `SelectionChanged`. These are trigger *intents* the published table does not name, and `on_double_click`/`on_focus`/`on_blur` exist because a pointer-driven control routes them through a value callback that the published signal cannot address. |
//!
//! So: `events:` is the route a designer writes, and `on_*` is the route an existing hand-written
//! layout keeps. `tools/check_json_event_route.sh` asserts this boundary mechanically — every
//! `on_*` key the loader reads must carry a trigger marker, and every `events:` name must be a
//! name the capability publishes.

use crate::json::EventHandlerContext;
use crate::WidgetTriggerEvent;

/// Which event a declared handler is bound to.
///
/// # Why this is a *value* and not the published name alone
///
/// A layout declares a handler against a name. Two of those names are validated by the capability
/// table (`clicked`, `value_changed` — rule #101's published route), and two are **trigger
/// markers** that the loader's marker callback recognises (`Closed`, `SelectionChanged`), which
/// are not published names at all. Keeping the distinction in the data rather than in a `match`
/// is what lets the gate in `tools/check_json_event_route.py` check it — and it is what lets
/// [`JsonEventBinding::uses_value_callback`] answer "which of the two callback tables does this
/// reach?" without asking a live control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonEventBinding {
    /// A name the capability table publishes. Wired to the control's own signal, so a control
    /// that gains a published event becomes declarable without touching the loader.
    Published {
        /// The published event name, spelled exactly as `connect_event` accepts it.
        name: &'static str,
        /// Whether the event's declared payload is non-empty.
        ///
        /// Read from the capability's [`EventSchema`](crate::widget::capability::EventSchema) at
        /// declaration time rather than assumed. A payload-free name travels the click callback and
        /// a payload-carrying one the value callback; binding a name to the wrong one is what made
        /// `"events": {"value_changed": "h"}` fire on a *press*.
        has_payload: bool,
        /// The trigger intent the published **name** names, independent of its payload.
        ///
        /// # Why the name decides this, not only the payload
        ///
        /// `has_payload` can only choose between "click" and "value". A payload-free name like
        /// `closed` or `double_clicked` is neither a click nor a value, yet the first revision
        /// derived `Clicked` from *any* payload-free name — so `events: {"closed": "h"}` bound the
        /// click callback and the handler ran on a **click** while being told the control had
        /// closed. The name carries the intent; the payload only decides which of the two
        /// callbacks a *routable* intent travels on.
        marker: JsonTriggerMarker,
    },
    /// A compatibility key whose meaning is a trigger *intent*, not a published name.
    Marker {
        /// The `on_*` key as it appears in the document.
        key: &'static str,
        /// The marker the handler receives.
        marker: JsonTriggerMarker,
    },
}

impl JsonEventBinding {
    /// Whether this binding must reach the **value** callback table rather than the click table.
    ///
    /// # The rule, in one place
    ///
    /// A published name's answer is its **marker's**: a value-bearing intent travels on
    /// `on_value_changed` and a pointer-driven one on `on_click`. Deriving it from the marker rather
    /// than the raw payload keeps the callback consistent with the trigger the handler is told
    /// about. A marker key carries its own answer for the same reason.
    ///
    /// This is the single place the choice is made, so the wiring and any test that asserts the
    /// wiring cannot disagree about it.
    pub fn uses_value_callback(self) -> bool {
        match self {
            Self::Published { marker, .. } => marker.uses_value_callback(),
            Self::Marker { marker, .. } => marker.uses_value_callback(),
        }
    }

    /// The marker the handler is told about.
    ///
    /// For a published name the marker was derived from the **name** (with the payload as a
    /// tie-breaker) when the binding was built, so it travels with the binding rather than being
    /// re-guessed here. A marker binding carries the marker it was declared with.
    pub fn marker(self) -> JsonTriggerMarker {
        match self {
            Self::Published { marker, .. } => marker,
            Self::Marker { marker, .. } => marker,
        }
    }

    /// The published event name when this is a published binding, for diagnostics.
    pub fn published_name(self) -> Option<&'static str> {
        match self {
            Self::Published { name, .. } => Some(name),
            Self::Marker { key, .. } => {
                let _ = key;
                None
            }
        }
    }
}

/// The trigger a JSON handler is told about.
///
/// A closed set, not a free string: the loader used to pass `WidgetTriggerKind` values it had
/// chosen per call site, and two keys with the same meaning could report different markers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonTriggerMarker {
    /// A left click / primary activation.
    Clicked,
    /// A second click within the double-click interval.
    DoubleClicked,
    /// The control gained focus.
    FocusGained,
    /// The control lost focus.
    FocusLost,
    /// The control's value changed.
    ValueChanged,
    /// The selected item changed. Distinct from `ValueChanged` because a selection is not a
    /// value: a list's selected row can change without its contents changing.
    SelectionChanged,
    /// The control was closed.
    Closed,
}

impl JsonTriggerMarker {
    /// Whether this marker's handler must be reached through the **value** callback table.
    ///
    /// # Why some markers share a kind
    ///
    /// `DoubleClicked`, `FocusGained` and `FocusLost` all report `Clicked` or `ValueChanged`
    /// because the routing callback they arrive through cannot carry a finer distinction: the
    /// pointer path only tells the loader "a press arrived" and the value path only "a value
    /// arrived". Stating that here — rather than at each call site — is what keeps two keys with
    /// the same limitation from appearing to behave differently.
    ///
    /// That same limitation is what decides the callback: a marker the pointer path produces
    /// (`Clicked`, `DoubleClicked`, `Closed`) travels on `on_click`, and one the value path
    /// produces (`ValueChanged`, `SelectionChanged`, both focus markers) on `on_value_changed`.
    /// Deriving it from [`Self::trigger_kind`] keeps the two answers from disagreeing, which is
    /// how `on_double_click` came to be wired to a callback that could never report a double
    /// click.
    pub fn uses_value_callback(self) -> bool {
        matches!(
            self.trigger_kind(),
            crate::platform::WidgetTriggerKind::ValueChanged
                | crate::platform::WidgetTriggerKind::SelectionChanged
        )
    }

    /// Whether this marker travels on the **close** callback (the widget's `closed` signal).
    ///
    /// # Why `Closed` now has a channel
    ///
    /// It used to have none: no backend produced `WidgetTriggerKind::Closed` through the handle
    /// layer, so [`Self::unroutable_reason`] refused it. [`BaseWidget::closed`](crate::widget::BaseWidget::closed)
    /// now exists and every closeable control emits it from its own `close`/`dismiss` path, so
    /// `Closed` has a real signal to reach and is bound rather than refused.
    pub fn uses_close_callback(self) -> bool {
        matches!(self, Self::Closed)
    }

    /// Why this marker cannot be routed to a live callback, or `None` when it can.
    ///
    /// # Why a marker is refused rather than approximated
    ///
    /// [`Self::uses_value_callback`] can only choose between two callbacks — the click one
    /// and the value one — so a marker whose real trigger is neither cannot be honoured. It
    /// used to be bound to whichever was nearer, which produced a handler that ran on the
    /// **wrong** event and was told the right one had happened:
    ///
    /// * `on_double_click` → the click callback, i.e. the *single*-click signal. The handler
    ///   fired on the first click and could never be reached by a double click.
    /// * `on_focus` / `on_blur` → the value callback, which fires when the control's *value*
    ///   changes, not when it gains or loses focus.
    ///
    /// `Closed` used to be on this list too ("no producer"). It is now routed through
    /// [`Self::uses_close_callback`] to the widget's `closed` signal, so it is no longer refused.
    ///
    /// Returning a reason instead of a guess is the same principle the crate applies
    /// elsewhere: a capability that is absent is reported, never fabricated.
    pub fn unroutable_reason(self) -> Option<&'static str> {
        match self {
            Self::DoubleClicked => Some(
                "`on_double_click` has no double-click signal to connect to; a control's click \
                 signal does not distinguish one click from two, so binding it there would fire \
                 the handler on a single click",
            ),
            Self::FocusGained | Self::FocusLost => Some(
                "`on_focus` / `on_blur` have no focus signal to connect to through this route; \
                 the value callback fires on a value change, not on focus",
            ),
            Self::Clicked | Self::ValueChanged | Self::SelectionChanged | Self::Closed => None,
        }
    }

    /// The `WidgetTriggerKind` this marker reports as.
    ///
    /// # Why some markers share a kind
    ///
    /// `DoubleClicked`, `FocusGained` and `FocusLost` all report `Clicked` or `ValueChanged`
    /// because the routing callback they arrive through cannot carry a finer distinction: the
    /// pointer path only tells the loader "a press arrived" and the value path only "a value
    /// arrived". Stating that here — rather than at each call site — is what keeps two keys with
    /// the same limitation from appearing to behave differently.
    pub fn trigger_kind(self) -> crate::platform::WidgetTriggerKind {
        use crate::platform::WidgetTriggerKind;
        match self {
            Self::Clicked | Self::DoubleClicked => WidgetTriggerKind::Clicked,
            Self::FocusGained | Self::FocusLost | Self::ValueChanged => {
                WidgetTriggerKind::ValueChanged
            }
            Self::SelectionChanged => WidgetTriggerKind::SelectionChanged,
            Self::Closed => WidgetTriggerKind::Closed,
        }
    }

    /// The trigger intent a **published event name** names, with `has_payload` as a tie-breaker.
    ///
    /// # Why the published route needs its own classifier
    ///
    /// A published name is free-form (`closed`, `double_clicked`, `dismissed`, `deselected`,
    /// `selection_changed`, `value_changed`, …), so the payload alone cannot say what it means:
    /// `closed` and `clicked` both carry no payload, yet one is a dismissal and the other an
    /// activation. Deriving the marker from the name keeps `events: {"closed": "h"}` from being
    /// bound to the click callback — which reported the *wrong* trigger to the handler and made a
    /// genuinely closed control indistinguishable from a clicked one.
    ///
    /// The mapping is deliberately conservative: a name whose intent this function does not
    /// recognise falls back to the payload question (`has_payload` → `ValueChanged`, else
    /// `Clicked`), which is exactly the previous behaviour and remains correct for the common
    /// `clicked` / `value_changed` cases. A name that *is* recognised as an intent with no live
    /// signal (`closed`, `double_clicked`, focus) then travels through
    /// [`Self::unroutable_reason`] and is refused rather than mis-bound.
    pub fn for_published_name(name: &str, has_payload: bool) -> Self {
        let lower = name.to_ascii_lowercase();
        // Order matters: the most specific intents are checked before the generic suffixes, so
        // `double_clicked` is not swallowed by the `clicked` check.
        if lower.contains("double_click") || lower == "dblclick" {
            return Self::DoubleClicked;
        }
        if lower == "closed" || lower == "close" || lower.ends_with("_closed") {
            return Self::Closed;
        }
        if lower.contains("focus_gained") || lower == "focused" || lower == "focus" {
            return Self::FocusGained;
        }
        if lower.contains("focus_lost") || lower == "blurred" || lower == "blur" {
            return Self::FocusLost;
        }
        if lower.contains("selection_changed")
            || lower.contains("select_changed")
            || lower.contains("selected_changed")
        {
            return Self::SelectionChanged;
        }
        if has_payload {
            Self::ValueChanged
        } else {
            Self::Clicked
        }
    }
}

/// Every `on_*` key the loader reads, with the marker it means.
///
/// # The single list
///
/// The loader previously extracted these with two hand-written functions returning a six-tuple
/// that callers destructured positionally. A key added to one function and not the other was
/// wired into the wrong slot, which is why the table is now one array read by name.
pub const MARKER_KEYS: &[(&str, JsonTriggerMarker)] = &[
    ("on_click", JsonTriggerMarker::Clicked),
    ("on_change", JsonTriggerMarker::ValueChanged),
    ("on_close", JsonTriggerMarker::Closed),
    ("on_double_click", JsonTriggerMarker::DoubleClicked),
    ("on_focus", JsonTriggerMarker::FocusGained),
    ("on_blur", JsonTriggerMarker::FocusLost),
    ("on_selection_changed", JsonTriggerMarker::SelectionChanged),
    ("on_value_changed", JsonTriggerMarker::ValueChanged),
];

/// The JSON key under which published-name handlers are declared.
pub const EVENTS_KEY: &str = "events";

/// Every `on_*` key, for the loader's "this key is not a property" list.
///
/// Derived from [`MARKER_KEYS`] so the two cannot disagree about which keys the loader consumes.
pub fn marker_key_names() -> impl Iterator<Item = &'static str> {
    MARKER_KEYS.iter().map(|(key, _)| *key)
}

/// Whether `key` is an `on_*` event key rather than a state property.
pub fn is_marker_key(key: &str) -> bool {
    MARKER_KEYS.iter().any(|(candidate, _)| *candidate == key)
}

/// The marker `key` means, or `None` when it is not an event key.
pub fn marker_for_key(key: &str) -> Option<JsonTriggerMarker> {
    MARKER_KEYS.iter().find(|(candidate, _)| *candidate == key).map(|(_, marker)| *marker)
}

/// The binding a published event name declares, resolved against `widget_type`'s capability.
///
/// # Why the name is copied into a `&'static str`
///
/// The capability table's event names **are** `&'static str` (the table is a `static`, produced by
/// `events_of!` from a generated schema), so the name already outlives the JSON document and can be
/// stored by reference. A name the control does not publish cannot reach here: the caller checks
/// `control_publishes` first and reports the miss.
///
/// # Why the payload question is asked here
///
/// Which callback a published name reaches is decided by whether it carries a value, and the only
/// authority on that is the control's own [`EventSchema`](crate::widget::capability::EventSchema).
/// Asking here — once, at bind time — means the wiring never has to guess, and the answer travels
/// with the binding so a test can assert it without a live control.
pub fn json_event_binding(widget_type: &str, name: &str) -> JsonEventBinding {
    let factory = crate::widget::capability::WidgetFactory::new_with_defaults();
    let normalized = crate::widget::capability::normalize_key(name);
    let has_payload = factory
        .capability(widget_type)
        .and_then(|capability| {
            capability
                .events
                .iter()
                .find(|schema| crate::widget::capability::normalize_key(schema.name) == normalized)
        })
        .is_some_and(|schema| schema.payload.is_some());
    JsonEventBinding::Published {
        // The event's own spelling from the table, which is what `connect_event` accepts — a
        // document that wrote a differently-cased name still binds to the published one.
        name: published_event_name(widget_type, name).unwrap_or("clicked"),
        has_payload,
        // The name carries the intent; see `JsonTriggerMarker::for_published_name`. This is what
        // keeps `events: {"closed": …}` / `events: {"double_clicked": …}` from being bound to
        // the click callback while being told the wrong trigger had happened.
        marker: JsonTriggerMarker::for_published_name(name, has_payload),
    }
}

/// The published spelling of `name` on `widget_type`, or `None` when it is not published.
pub fn published_event_name(widget_type: &str, name: &str) -> Option<&'static str> {
    let factory = crate::widget::capability::WidgetFactory::new_with_defaults();
    let normalized = crate::widget::capability::normalize_key(name);
    factory.capability(widget_type).and_then(|capability| {
        capability
            .events
            .iter()
            .find(|schema| crate::widget::capability::normalize_key(schema.name) == normalized)
            .map(|schema| schema.name)
    })
}

/// The `on_*` key the marker table lists for `marker`, or `None` when it lists none.
///
/// The reverse of [`marker_for_key`]. Two markers can share a trigger kind (`on_click` and
/// `on_double_click` both report `Clicked`), so the forward direction is not invertible in general —
/// which is exactly why the key a caller *wrote* is carried through the binding rather than
/// re-derived from the kind.
pub fn marker_key_for(marker: JsonTriggerMarker) -> Option<&'static str> {
    MARKER_KEYS.iter().find(|(_, m)| *m == marker).map(|(key, _)| *key)
}

/// Builds the [`EventHandlerContext`] for a declared handler.
///
/// One constructor for both routes, so a `marker` binding and a published binding cannot report
/// different context shapes for the same trigger.
pub fn context_for(
    widget_id: crate::core::ObjectId,
    marker: JsonTriggerMarker,
) -> EventHandlerContext {
    EventHandlerContext::new(WidgetTriggerEvent { widget_id, kind: marker.trigger_kind() })
}

/// Builds the [`EventHandlerContext`] for a declared handler **with the event's payload**.
///
/// The payload-carrying counterpart of [`context_for`], used by the published-name route where the
/// control's dynamic signal delivered a [`CapabilityValue`](crate::widget::capability::CapabilityValue).
/// Keeping one constructor per shape (with and without a value) is what stops the two routes from
/// disagreeing about the trigger, because both still go through [`JsonTriggerMarker::trigger_kind`].
pub fn context_for_with_payload(
    widget_id: crate::core::ObjectId,
    marker: JsonTriggerMarker,
    payload: crate::widget::capability::CapabilityValue,
) -> EventHandlerContext {
    EventHandlerContext::new(WidgetTriggerEvent { widget_id, kind: marker.trigger_kind() })
        .with_payload(payload)
}

/// Wires one **published** event (`events: { <name>: <handler> }`) to a live control.
///
/// # Why this is public rather than loader-private
///
/// The designer's generated program builds controls through its own `create_for` and never runs the
/// JSON loader (that is the point of generating code: the program links no parser). But the *wiring*
/// it must perform is exactly the loader's — validate the name against the control's capability,
/// choose the callback from the payload observation, and refuse a name that has no signal. Rule #98
/// makes that the library's job: a generator (or any host that learns its wiring at run time) must
/// not have to reimplement it, because a second implementation is a second set of rules that agrees
/// only until one is edited.
///
/// Before this existed the generator had **no** way to emit a wire at all, so a document declaring
/// `events: { clicked: "on_save" }` produced a program with no handler — silently, because the name
/// was validated at generation time and looked fine.
///
/// Returns whether the handler was bound. A name the control does not publish, or a marker whose
/// trigger has no signal (see [`JsonTriggerMarker::unroutable_reason`]), returns `false` and is
/// reported at `warn!`: the caller gets a fact rather than a binding that can never fire.
pub fn bind_published_event(
    widget_id: crate::core::ObjectId,
    widget_type: &str,
    name: &str,
    handler: &str,
) -> bool {
    if published_event_name(widget_type, name).is_none() {
        log::warn!(
            "[{widget_type}] `events.{name}` is not a published event; the binding is skipped, \
             so the handler `{handler}` would never run"
        );
        return false;
    }
    crate::json::bind_event_binding(
        widget_id,
        json_event_binding(widget_type, name),
        handler.to_owned(),
    )
}

/// Wires one **compatibility** key (`on_click`, `on_change`, …) to a live control.
///
/// See [`bind_published_event`] for why this is public. `key` must be one the marker table lists;
/// an unknown key is reported and refused rather than guessed at.
pub fn bind_marker_event(
    widget_id: crate::core::ObjectId,
    key: &str,
    marker: JsonTriggerMarker,
    handler: &str,
) -> bool {
    // The binding carries a `&'static str` because the key outlives the document: the marker table
    // is a `static`. A caller that passes a borrowed key is therefore resolved to the table's own
    // spelling rather than having its slice stored — which is also what makes a differently-cased
    // key bind to the canonical one instead of creating a second spelling that no other code reads.
    let Some(canonical) = MARKER_KEYS.iter().find(|(candidate, _)| *candidate == key) else {
        log::warn!(
            "`{key}` is not an `on_*` event key the marker table lists; the binding for \
             `{handler}` is skipped"
        );
        return false;
    };
    crate::json::bind_event_binding(
        widget_id,
        JsonEventBinding::Marker { key: canonical.0, marker },
        handler.to_owned(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_marker_key_resolves_to_its_marker() {
        for (key, marker) in MARKER_KEYS {
            assert_eq!(marker_for_key(key), Some(*marker), "`{key}` must resolve to its marker");
            assert!(is_marker_key(key));
        }
        assert_eq!(marker_for_key("on_click"), Some(JsonTriggerMarker::Clicked));
        assert_eq!(marker_for_key("on_close"), Some(JsonTriggerMarker::Closed));
    }

    #[test]
    fn a_non_event_key_is_not_a_marker_key() {
        for key in ["text", "width", "on_", "clicked", "events"] {
            assert!(!is_marker_key(key), "`{key}` is not an `on_*` event key");
            assert_eq!(marker_for_key(key), None);
        }
    }

    #[test]
    fn the_marker_key_list_has_no_duplicates() {
        let mut keys: Vec<&str> = marker_key_names().collect();
        let count = keys.len();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), count, "a duplicated key would be extracted twice");
    }

    #[test]
    fn markers_report_distinct_trigger_kinds() {
        // `SelectionChanged` and `ValueChanged` are different facts and must not collapse to one
        // trigger: a list whose selection moved did not change its value.
        assert_ne!(
            JsonTriggerMarker::SelectionChanged.trigger_kind(),
            JsonTriggerMarker::ValueChanged.trigger_kind()
        );
    }

    #[test]
    fn markers_that_share_a_callback_share_their_kind() {
        // The pointer path cannot distinguish one press from two, and the value path cannot
        // distinguish focus from a value change. Collapsing them is honest; inventing a
        // distinction the callback cannot carry would be a marker the handler can never observe.
        assert_eq!(
            JsonTriggerMarker::DoubleClicked.trigger_kind(),
            JsonTriggerMarker::Clicked.trigger_kind()
        );
        assert_eq!(
            JsonTriggerMarker::FocusGained.trigger_kind(),
            JsonTriggerMarker::ValueChanged.trigger_kind()
        );
        assert_eq!(
            JsonTriggerMarker::FocusLost.trigger_kind(),
            JsonTriggerMarker::ValueChanged.trigger_kind()
        );
    }

    /// A marker whose real trigger has **no** callback is refused, not bound to the nearer
    /// one. `trigger_kind` collapses `DoubleClicked` onto `Clicked` precisely because the
    /// route cannot carry the difference — so binding it to the click callback made the
    /// handler fire on a single click while being told a double click had happened.
    ///
    /// `Closed` is deliberately **not** in this list any more: it now has a real channel (the
    /// widget's `closed` signal, see [`JsonTriggerMarker::uses_close_callback`]), so it is bound
    /// rather than refused.
    #[test]
    fn a_marker_with_no_signal_to_reach_is_refused() {
        for marker in [
            JsonTriggerMarker::DoubleClicked,
            JsonTriggerMarker::FocusGained,
            JsonTriggerMarker::FocusLost,
        ] {
            let reason = marker.unroutable_reason();
            assert!(reason.is_some(), "{marker:?} must state why it cannot be routed");
            let reason = reason.expect("checked above");
            assert!(
                reason.contains("on_"),
                "{marker:?}'s reason must name the key an author wrote: {reason}"
            );
        }
    }

    /// The markers that *do* have a reachable signal stay routable, so the refusal cannot
    /// quietly grow to cover the working keys. `Closed` is here because its producer now exists.
    #[test]
    fn the_routable_markers_stay_routable() {
        assert!(JsonTriggerMarker::Clicked.unroutable_reason().is_none());
        assert!(JsonTriggerMarker::ValueChanged.unroutable_reason().is_none());
        assert!(JsonTriggerMarker::SelectionChanged.unroutable_reason().is_none());
        assert!(
            JsonTriggerMarker::Closed.unroutable_reason().is_none(),
            "`closed` has a real signal (the widget's `closed`), so it must be routable"
        );
    }

    /// `Closed` must be routed to the **close** channel, not the click or value one.
    #[test]
    fn closed_uses_the_close_channel_and_not_click_or_value() {
        assert!(JsonTriggerMarker::Closed.uses_close_callback());
        assert!(!JsonTriggerMarker::Closed.uses_value_callback(), "a close is not a value change");
        // The other routable markers are not close callbacks.
        assert!(!JsonTriggerMarker::Clicked.uses_close_callback());
        assert!(!JsonTriggerMarker::ValueChanged.uses_close_callback());
        assert!(!JsonTriggerMarker::SelectionChanged.uses_close_callback());
    }

    /// A published name's intent comes from the **name**, not only from its payload.
    ///
    /// Pins the defect: `json_event_binding` derived `Clicked` from *any* payload-free name, so
    /// `events: {"closed": …}` bound the click callback and the handler ran on a click while
    /// being told the control had closed. `closed` and `double_clicked` are payload-free and yet
    /// name intents the click signal cannot carry, so they must resolve to their own markers —
    /// which then travel through `unroutable_reason` and are refused.
    #[test]
    fn a_published_name_carries_its_own_intent() {
        // The genuine click stays a click.
        assert_eq!(
            JsonTriggerMarker::for_published_name("clicked", false),
            JsonTriggerMarker::Clicked
        );
        // A value-bearing name stays a value change.
        assert_eq!(
            JsonTriggerMarker::for_published_name("value_changed", true),
            JsonTriggerMarker::ValueChanged
        );
        // The non-click, payload-free intents are recognised...
        assert_eq!(
            JsonTriggerMarker::for_published_name("closed", false),
            JsonTriggerMarker::Closed
        );
        assert_eq!(
            JsonTriggerMarker::for_published_name("double_clicked", false),
            JsonTriggerMarker::DoubleClicked
        );
        // ...and each is routed through the channel its intent needs. `closed` now has a real
        // signal, so it is **routed** (not refused); `double_clicked` still has none, so it is
        // refused rather than mis-bound to the click callback.
        assert!(
            JsonTriggerMarker::for_published_name("closed", false).uses_close_callback(),
            "`closed` must route to the close channel"
        );
        assert!(
            JsonTriggerMarker::for_published_name("closed", false).unroutable_reason().is_none(),
            "`closed` has a producer now, so it must not be refused"
        );
        assert!(JsonTriggerMarker::for_published_name("double_clicked", false)
            .unroutable_reason()
            .is_some());
    }

    /// `double_clicked` must not be swallowed by the `clicked` substring check.
    #[test]
    fn the_double_click_check_precedes_the_click_check() {
        assert_ne!(
            JsonTriggerMarker::for_published_name("double_clicked", false),
            JsonTriggerMarker::Clicked
        );
    }

    /// A published selection event resolves to the selection marker, not a value change.
    #[test]
    fn a_published_selection_event_is_a_selection_change() {
        assert_eq!(
            JsonTriggerMarker::for_published_name("selection_changed", false),
            JsonTriggerMarker::SelectionChanged
        );
        assert_eq!(
            JsonTriggerMarker::for_published_name("selected_changed", true),
            JsonTriggerMarker::SelectionChanged
        );
    }
}
