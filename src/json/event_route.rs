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
//! | `on_*` | the compatibility route | the same handler reached through a **marker**: `on_close` means `Closed`, `on_selection_changed` means `SelectionChanged`. A marker names a trigger *intent*, which is a different thing from a published name. |
//!
//! So: `events:` is the route a designer writes, and `on_*` is the route an existing hand-written
//! layout keeps. `tools/check_json_event_route.sh` asserts this boundary mechanically — every
//! `on_*` key the loader reads must carry a trigger marker, and every `events:` name must be a
//! name the capability publishes.
//!
//! # How a marker reaches its trigger
//!
//! A marker intent is carried by exactly one channel, and the three questions below are the whole
//! decision — the loader asks them in this order and has no per-key `match` of its own:
//!
//! 1. [`JsonTriggerMarker::base_signal`] — the widget's own base signal (`clicked`, `closed`,
//!    `focus_gained`, `focus_lost`). The runtime's focus router drives the two focus signals
//!    through `BaseWidget::handle_event`, so `on_focus` / `on_blur` have a real channel even though
//!    the capability table publishes no per-control focus event.
//! 2. [`JsonTriggerMarker::uses_value_callback`] — the app layer's value-trigger channel, which is
//!    the only route to `ValueChanged` / `SelectionChanged` for controls whose value signal the
//!    capability table does not publish under a name.
//! 3. [`JsonTriggerMarker::published_counterpart`] — a published name for an intent no widget
//!    callback can carry. `on_double_click` is the case: the click callback reports one click and
//!    cannot distinguish two, so the intent is wired through the published `double_clicked` event
//!    that the controls distinguishing a second click actually publish.
//!
//! # What a document still cannot express
//!
//! The gesture recognisers in `src/gesture/` emit the spellings of `GestureKind::name()` — `tap`,
//! `double_tap`, `long_press`, `long_press_drag`, `pan`, `swipe`, `fling`, `two_finger_tap`,
//! `two_finger_swipe`, `pinch`, `rotate`. The capability table publishes **none** of them, so no
//! `events:` name and no `on_*` key can reach a gesture. Inventing a name here would create exactly
//! the second name source rule #101 forbids (the loader's `events:` route validates against the
//! capability table, and the table has no row to validate against), so the gap is stated instead of
//! papered over. It closes when a control publishes a gesture event and resolves it through
//! `event_signal_dyn`; this module then needs no edit at all.

use crate::json::EventHandlerContext;
use crate::WidgetTriggerEvent;

/// The base-widget signal that announces one trigger intent.
///
/// # Why a marker points at a signal rather than at a callback
///
/// [`BaseWidget`](crate::widget::BaseWidget) emits `clicked`, `closed`, `focus_gained` and
/// `focus_lost` itself; the runtime focus router reaches the last two through
/// `BaseWidget::handle_event`. Binding a marker to the signal is what gives `on_focus` / `on_blur` a
/// channel without inventing a published name the capability table does not declare — the marker
/// travels on the widget's own event, not on a handle callback that has no way to report focus.
///
/// Crate-internal: [`JsonTriggerMarker::base_signal`] is the public answer, and the loader is the
/// only consumer that needs to know *which* signal to connect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BaseSignal {
    /// `BaseWidget::clicked`.
    Clicked,
    /// `BaseWidget::closed`.
    Closed,
    /// `BaseWidget::focus_gained`.
    FocusGained,
    /// `BaseWidget::focus_lost`.
    FocusLost,
}

/// Which event a declared handler is bound to.
///
/// # Why this is a *value* and not the published name alone
///
/// A layout declares a handler against a name. Some names are validated by the capability table
/// (`clicked`, `value_changed` — rule #101's published route); others are **trigger markers** that
/// name an intent (`Closed`, `SelectionChanged`, `FocusGained`), which are not published names at
/// all. Keeping the distinction in the data rather than in a `match` is what lets the gate in
/// `tools/check_json_event_route.py` check it — and it is what lets
/// [`JsonEventBinding::published_name`] answer "is this the published route or a marker route?"
/// without asking a live control.
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
        /// declaration time rather than assumed, so a designer can tell a unit event from a
        /// value-carrying one without a live control. The loader itself no longer chooses a
        /// callback from it: a published name is subscribed through
        /// [`Widget::event_signal_dyn`](crate::widget::Widget::event_signal_dyn), which delivers the
        /// payload whichever shape it is. The flag survives because the gate and the round-trip
        /// tests assert it against the table (rule #95), and because a binding that disagreed with
        /// the table would mean the two name sources had drifted.
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
    /// Which channel the binding's **marker** needs, for each route.
    ///
    /// # Where this is consulted
    ///
    /// The loader reaches a published name through the control's own dynamic signal (which carries
    /// the payload and which no callback table can select), so this answers the *marker* route: a
    /// value-bearing marker (`ValueChanged`, `SelectionChanged`) travels the app layer's
    /// value-trigger channel, and every other marker has a signal or a published name of its own
    /// (see `JsonTriggerMarker::base_signal` and
    /// [`JsonTriggerMarker::published_counterpart`]).
    ///
    /// It is derived from the marker on **both** variants rather than from the raw payload: that is
    /// the single place the answer is stated, so a test asserting the wiring and the wiring itself
    /// cannot disagree.
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

    /// The published event this binding corresponds to, when it has one.
    ///
    /// A [`Self::Published`] binding returns its own name; a [`Self::Marker`] binding returns the
    /// counterpart recorded in `MARKER_PUBLISHED_EVENTS`. `None` therefore means "this binding
    /// rides the compatibility route and no published name exists for its intent".
    pub fn published_counterpart(self) -> Option<&'static str> {
        match self {
            Self::Published { name, .. } => Some(name),
            Self::Marker { marker, .. } => marker.published_counterpart(),
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
    /// # Why the answer is the marker's own, not its trigger kind
    ///
    /// Only two intents are value-trigger facts: a value change and a selection change. They travel
    /// the app layer's value-trigger channel because a control whose value signal the capability
    /// table publishes under no name has no other way for the loader to reach it.
    ///
    /// Every other intent has a signal of its own — `clicked` / `closed` / `focus_gained` /
    /// `focus_lost` on the base widget, `double_clicked` through its published name — so answering
    /// from [`Self::trigger_kind`] would be wrong for the two focus markers: no
    /// `WidgetTriggerKind` variant names a focus change, and their kind is therefore the coarse
    /// `Unknown` (see [`Self::trigger_kind`]). Deriving the callback from that coarse kind is how
    /// `on_double_click` came to be wired to a callback that could never report a double click, and
    /// how `on_focus` / `on_blur` would be wired to the value callback again if the loader's
    /// channel order ever changed.
    pub fn uses_value_callback(self) -> bool {
        matches!(self, Self::ValueChanged | Self::SelectionChanged)
    }

    /// Whether this marker travels on the **close** signal (the widget's `closed`).
    ///
    /// # Why `Closed` now has a channel
    ///
    /// It used to have none: no backend produced `WidgetTriggerKind::Closed` through the handle
    /// layer, so [`Self::unroutable_reason`] refused it. [`BaseWidget::closed`](crate::widget::BaseWidget::closed)
    /// now exists and every closeable control emits it from its own `close`/`dismiss` path, so
    /// `Closed` has a real signal to reach and is bound rather than refused.
    ///
    /// Derived from `Self::base_signal` rather than from a second `match`, so the two answers
    /// cannot disagree about which channel a close takes.
    pub fn uses_close_callback(self) -> bool {
        matches!(self.base_signal(), Some(BaseSignal::Closed))
    }

    /// The base-widget signal that carries this marker's trigger, when one does.
    ///
    /// `None` means the intent is not a base-widget fact: a value change is a per-control value
    /// signal ([`Self::uses_value_callback`]) and a double click is carried by its published name
    /// ([`Self::published_counterpart`]).
    ///
    /// # Why focus belongs here (W-05)
    ///
    /// `on_focus` / `on_blur` used to have no channel at all: the marker was refused because the
    /// only callbacks available were the click one and the value one, and neither fires on focus.
    /// [`BaseWidget::focus_gained`](crate::widget::BaseWidget::focus_gained) and
    /// [`BaseWidget::focus_lost`](crate::widget::BaseWidget::focus_lost) do, and the runtime's focus
    /// router drives them through `BaseWidget::handle_event`, so the intent has a real emitter.
    /// Pointing at it is what lets the two keys route without a published name the capability table
    /// does not declare.
    pub(crate) fn base_signal(self) -> Option<BaseSignal> {
        match self {
            Self::Clicked => Some(BaseSignal::Clicked),
            Self::Closed => Some(BaseSignal::Closed),
            Self::FocusGained => Some(BaseSignal::FocusGained),
            Self::FocusLost => Some(BaseSignal::FocusLost),
            Self::DoubleClicked | Self::ValueChanged | Self::SelectionChanged => None,
        }
    }

    /// Why this marker cannot be routed through a widget **callback**, or `None` when it can.
    ///
    /// # Why a marker is never approximated
    ///
    /// The click callback and the value callback are the only two a widget exposes generically, so
    /// a marker whose real trigger is neither cannot be honoured by either. It used to be bound to
    /// whichever was nearer, which produced a handler that ran on the **wrong** event and was told
    /// the right one had happened:
    ///
    /// * `on_double_click` → the click callback, i.e. the *single*-click signal. The handler fired
    ///   on the first click and could never be reached by a double click.
    /// * `on_focus` / `on_blur` → the value callback, which fires when the control's *value*
    ///   changes, not when it gains or loses focus.
    ///
    /// All three markers named above now have a real channel, so none of them is left unrouted:
    ///
    /// * `on_focus` / `on_blur` travel on the widget's own focus signals
    ///   (`Self::base_signal`) and no longer report a reason at all.
    /// * `on_double_click` keeps its reason — no widget callback can carry it — and travels on its
    ///   published counterpart, `double_clicked` ([`Self::published_counterpart`]); the loader wires
    ///   that name and refuses with a message naming it when the control does not publish it.
    ///
    /// `Closed` was on this list once too ("no producer"); it is now the widget's own `closed`
    /// signal ([`Self::uses_close_callback`]).
    ///
    /// So this answers "can a *widget callback* carry it?" — `Some` is a refusal by the callback
    /// route, not a refusal of the binding. The loader consults it to pick the published channel
    /// instead of a callback, and a marker with neither a counterpart nor a callback is refused
    /// outright. Returning a reason instead of a guess is the same principle the crate applies
    /// elsewhere: a capability that is absent is reported, never fabricated.
    pub fn unroutable_reason(self) -> Option<&'static str> {
        match self {
            Self::DoubleClicked => Some(
                "`on_double_click` cannot be carried by a widget callback: the click callback \
                 reports a single click and the value callback reports value changes, so neither \
                 can tell the handler that a second click arrived. It is wired through the \
                 published `double_clicked` event, which only controls that distinguish a second \
                 click publish",
            ),
            Self::Clicked | Self::ValueChanged | Self::SelectionChanged | Self::Closed => None,
            // Focus now has a channel of its own; see `Self::base_signal`.
            Self::FocusGained | Self::FocusLost => None,
        }
    }

    /// The published event name this marker corresponds to, or `None` when the intent has no
    /// published spelling.
    ///
    /// See `MARKER_PUBLISHED_EVENTS` for why the answer is data rather than a second `match`.
    pub fn published_counterpart(self) -> Option<&'static str> {
        MARKER_PUBLISHED_EVENTS
            .iter()
            .find(|(marker, _)| *marker == self)
            .and_then(|(_, name)| *name)
    }

    /// Whether this marker has any published equivalent at all.
    ///
    /// The compatibility route may carry an intent with no published name, but that fact is
    /// explicit: a gate can require a *new* marker without a counterpart to be recorded here
    /// rather than inferred from silence. Every shipped marker currently has one.
    pub fn is_publishable(self) -> bool {
        self.published_counterpart().is_some()
    }

    /// The `WidgetTriggerKind` this marker reports as.
    ///
    /// # Why some markers share a kind, and the focus markers report `Unknown`
    ///
    /// The platform enum is coarser than the marker set. `DoubleClicked` reports `Clicked` because no
    /// variant names a second click.
    ///
    /// The two **focus** markers report
    /// [`WidgetTriggerKind::Unknown`](crate::platform::WidgetTriggerKind::Unknown): no variant names a
    /// focus change, and the two nearest claims are both false — `ValueChanged` says the control's
    /// value changed and `Clicked` says it was activated. `Unknown` is the enum's own word for "no
    /// concrete trigger semantic is known", which is true, and the handler still knows which wire
    /// fired because it registered for `on_focus` or `on_blur`. Reporting a wrong-but-specific kind
    /// is the failure this module refuses elsewhere (a handler told the right event happened while
    /// running on another one).
    ///
    /// The **channel** a marker travels on is `Self::base_signal` / [`Self::uses_value_callback`]
    /// / [`Self::published_counterpart`], not this value: the kind is what the handler is told, not
    /// how the binding is made.
    pub fn trigger_kind(self) -> crate::platform::WidgetTriggerKind {
        use crate::platform::WidgetTriggerKind;
        match self {
            Self::Clicked | Self::DoubleClicked => WidgetTriggerKind::Clicked,
            Self::ValueChanged => WidgetTriggerKind::ValueChanged,
            Self::SelectionChanged => WidgetTriggerKind::SelectionChanged,
            Self::Closed => WidgetTriggerKind::Closed,
            Self::FocusGained | Self::FocusLost => WidgetTriggerKind::Unknown,
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
    /// `clicked` / `value_changed` cases. A name that *is* recognised as an intent with no signal of
    /// its own (`closed`, `double_clicked`, focus) then travels its own channel, and a name whose
    /// channel the control cannot provide is refused rather than mis-bound.
    pub fn for_published_name(name: &str, has_payload: bool) -> Self {
        // Normalise before classifying, not only before matching the schema. The schema lookup uses
        // `normalize_key` (which strips `_`/`-`/space and lower-cases), so `selection-changed` and
        // `selection_changed` resolve to the *same* published name. Classifying the raw spelling
        // instead made the two disagree about intent: the underscore form matched
        // `contains("selection_changed")` and reported `SelectionChanged`, while the hyphen form fell
        // through to the payload question and reported `ValueChanged` — the same event with two
        // different trigger kinds depending on how the author spelled it (BLUE-issue E-24).
        let lower = crate::widget::capability::normalize_key(name);
        // Order matters: the most specific intents are checked before the generic suffixes, so
        // `double_clicked` is not swallowed by the `clicked` check.
        if lower.contains("doubleclick") {
            return Self::DoubleClicked;
        }
        if lower == "closed" || lower == "close" || lower.ends_with("closed") {
            return Self::Closed;
        }
        if lower.contains("focusgained") || lower == "focused" || lower == "focus" {
            return Self::FocusGained;
        }
        if lower.contains("focuslost") || lower == "blurred" || lower == "blur" {
            return Self::FocusLost;
        }
        if lower.contains("selectionchanged")
            || lower.contains("selectchanged")
            || lower.contains("selectedchanged")
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

/// The published event name each compatibility marker corresponds to, or `None`.
///
/// # Why the correspondence is data
///
/// `on_*` keys are trigger *intents*; `events:` names are names the capability table publishes.
/// Two of the intents overlap a published name (`clicked`, `value_changed`), `selection_changed`
/// and `closed` overlap one each, and `double_clicked` is published by controls that actually
/// distinguish a second click (the canvas and the image view; the grid/table/tree publish their own
/// `cell_double_clicked` / `node_double_clicked` spellings and are not the same intent).
///
/// # Why the focus rows name `focus_gained` / `focus_lost` (W-302)
///
/// Focus moved between the two routes historically. The two names that mention focus are different
/// intents: `focus_changed` is a **dialog's** report that keyboard focus moved between its own
/// children (its payload is the focused child's id), and `window_focus_changed` is a window-chrome
/// report that the whole window gained or lost focus. Wiring `on_focus` to either would run the
/// handler on the wrong event while telling it the control had gained focus.
///
/// `focus_gained` / `focus_lost` are the control's *own* events: `BaseWidget` emits them from its
/// focus handler for every control, and [`find_event_schema`](crate::widget::capability::find_event_schema)
/// accepts them for every capability through one shared table. They are therefore a published
/// spelling now, and the marker binds through the base signal first
/// ([`JsonTriggerMarker::base_signal`]) — the published name adds the declarative route without
/// changing which signal fires.
///
/// The initial prose version of this table stated the correspondence only in words, so a new marker
/// could quietly be added with no published route and nothing would notice.
///
/// Keeping the answer in one table lets [`JsonTriggerMarker::published_counterpart`] answer it for
/// the binder and lets a gate reject a future marker that claims a published name which no control
/// actually publishes. `None` is a promise, not an omission: it says this intent has no published
/// spelling and must stay on the compatibility route.
pub const MARKER_PUBLISHED_EVENTS: &[(JsonTriggerMarker, Option<&'static str>)] = &[
    (JsonTriggerMarker::Clicked, Some("clicked")),
    (JsonTriggerMarker::ValueChanged, Some("value_changed")),
    (JsonTriggerMarker::SelectionChanged, Some("selection_changed")),
    (JsonTriggerMarker::Closed, Some("closed")),
    (JsonTriggerMarker::DoubleClicked, Some("double_clicked")),
    (JsonTriggerMarker::FocusGained, Some("focus_gained")),
    (JsonTriggerMarker::FocusLost, Some("focus_lost")),
];

/// Every marker that has no published counterpart.
///
/// A gate reads this to require an explicit decision for a new marker: a marker added to
/// [`MARKER_KEYS`] without a row in `MARKER_PUBLISHED_EVENTS` is absent from the table rather
/// than silently "unpublished", and a marker whose row is `None` is recorded here.
///
/// `None` means "no published spelling", **not** "no channel": a marker can still be carried by
/// the widget's own signals (`JsonTriggerMarker::base_signal`). Every shipped marker has a
/// published name, so this iterator is empty today; it stays as the gate's hook for a new one.
pub fn markers_without_published_counterpart() -> impl Iterator<Item = JsonTriggerMarker> {
    MARKER_PUBLISHED_EVENTS.iter().filter_map(|(marker, name)| name.is_none().then_some(*marker))
}

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

/// The published name a document consulted before the table could answer it.
///
/// [`json_event_binding`] used to return `Published { name: "clicked", .. }` for any name the
/// control did not publish, so a misspelt wire bound the click signal and ran on the wrong event
/// (W-303). A binding now carries this sentinel instead: `bind_wired` sees a name the control does
/// not resolve, reports the refusal and records it as unwired, exactly as it does for any other
/// name with no live signal. The warning emitted at construction names the miss, so the author
/// learns why rather than discovering a handler that never ran.
pub const UNRESOLVED_PUBLISHED_NAME: &str = "";

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
    let factory = crate::widget::capability::WidgetFactory::shared();
    let has_payload = factory
        .capability(widget_type)
        .and_then(|capability| crate::widget::capability::find_event_schema(capability, name))
        .is_some_and(|schema| schema.payload.is_some());
    // The event's own spelling from the table, which is what `connect_event` accepts — a document
    // that wrote a differently-cased name still binds to the published one. A name no control
    // publishes must **not** fall back to `clicked` (W-303): the sentinel below makes the binder
    // refuse it, and this warning is the diagnostic the author needs.
    let canonical = published_event_name(widget_type, name);
    if canonical.is_none() {
        log::warn!(
            "[{widget_type}] `{name}` is not a published event, so it has no signal to bind; the \
             binding is refused rather than retargeted to `clicked`"
        );
    }
    JsonEventBinding::Published {
        name: canonical.unwrap_or(UNRESOLVED_PUBLISHED_NAME),
        has_payload,
        // The name carries the intent; see `JsonTriggerMarker::for_published_name`. This is what
        // keeps `events: {"closed": …}` / `events: {"double_clicked": …}` from being bound to
        // the click callback while being told the wrong trigger had happened.
        marker: JsonTriggerMarker::for_published_name(name, has_payload),
    }
}

/// The published spelling of `name` on `widget_type`, or `None` when it is not published.
pub fn published_event_name(widget_type: &str, name: &str) -> Option<&'static str> {
    let factory = crate::widget::capability::WidgetFactory::shared();
    factory.capability(widget_type).and_then(|capability| {
        crate::widget::capability::find_event_schema(capability, name).map(|schema| schema.name)
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

/// A handle to one dynamically-added JSON event binding, which can release just that binding.
///
/// # Why this exists (BLUE-issue E-22)
///
/// `JsonLoader::bind_one` returns a `bool` and drops the connection handle the published route
/// created, so a host that added a wire at run time had no way to remove it again without destroying
/// the control. A designer that replaces or deletes a single wire needs exactly that, so the binding
/// is now represented by a token whose `release` disconnects the one subscription it made.
///
/// Binding is **additive**: two tokens over the same declaration are two subscriptions and one
/// emission runs the handler twice. `release` is idempotent \u2014 a second call does nothing.
pub struct DynamicBinding {
    release: Option<alloc::boxed::Box<dyn FnOnce() + Send>>,
}

impl DynamicBinding {
    /// Wraps a release closure. Crate-internal: the loader is the only producer.
    pub(crate) fn new(release: alloc::boxed::Box<dyn FnOnce() + Send>) -> Self {
        Self { release: Some(release) }
    }

    /// Disconnects exactly the subscription this token represents. Safe to call more than once.
    pub fn release(&mut self) {
        if let Some(release) = self.release.take() {
            release();
        }
    }

    /// Whether this token still owns a live subscription (i.e. `release` has not been called).
    pub fn is_live(&self) -> bool {
        self.release.is_some()
    }
}

impl Drop for DynamicBinding {
    fn drop(&mut self) {
        // Dropping the token releases its binding, matching the `ConnectionScope` contract: a host
        // that forgets a token does not leak a subscription into a still-living control.
        self.release();
    }
}

impl core::fmt::Debug for DynamicBinding {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("DynamicBinding").field("live", &self.is_live()).finish()
    }
}

// ── Load-time binding ownership (BLUE-issue E-27) ──────────────────

// `JsonLoader::bind_one` wires a document's `events` and returns only a `bool`. It used to
// `core::mem::forget` its token so the subscription outlived the call — but the token owns a release
// closure that captures an `EventSignalRef`, which holds a strong reference to the control's signal.
// Forgetting it kept the signal (and therefore every slot on it, plus their captures) alive forever,
// even after the control was destroyed. The token is now **retained** here, keyed by the control it
// belongs to, and released when that control is torn down — so the subscription lives exactly as long
// as the control does, and nothing outlives it.

std::thread_local! {
    static LOADED_BINDINGS: core::cell::RefCell<
        crate::compat::HashMap<crate::core::ObjectId, alloc::vec::Vec<DynamicBinding>>,
    > = core::cell::RefCell::new(crate::compat::HashMap::new());
}

/// Retains a load-time binding token until its control is torn down.
///
/// Called by the loader's no-token `bind_one` for **both** routes — a published name and an `on_*`
/// marker each yield a token, so unloading a document releases the whole set rather than only the
/// published half. A caller that wants to release its own wire uses
/// [`crate::json::JsonLoader::bind_one_releasable`] instead and owns the token.
pub(crate) fn retain_binding(widget_id: crate::core::ObjectId, binding: DynamicBinding) {
    LOADED_BINDINGS.with(|map| {
        map.borrow_mut().entry(widget_id).or_default().push(binding);
    });
}

/// Releases every load-time binding registered for `widget_id`.
///
/// Called when a node is torn down, so a destroyed control's subscriptions (and the signal references
/// they hold) are dropped with it rather than leaked. Idempotent: releasing an id with no bindings is
/// a no-op.
pub fn release_widget_bindings(widget_id: crate::core::ObjectId) {
    LOADED_BINDINGS.with(|map| {
        // `remove` drops the `Vec<DynamicBinding>`, and each `Drop` disconnects its subscription.
        let _ = map.borrow_mut().remove(&widget_id);
    });
}

/// How many load-time binding tokens are currently retained for `widget_id`. For tests.
pub fn retained_binding_count(widget_id: crate::core::ObjectId) -> usize {
    LOADED_BINDINGS.with(|map| map.borrow().get(&widget_id).map(Vec::len).unwrap_or(0))
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

/// Wires every published event of `widget_type` for which a global handler of the **same name** is
/// already registered on this thread (W-301).
///
/// # Why a batch entry point
///
/// A host that registers handlers under their published names (`clicked`, `value_changed`, …) had to
/// repeat those names once per control in a `bind_published_event` loop. Forgetting one was silent:
/// the control kept a handler that never ran, which is the failure rule #97 forbids. The batch form
/// reads the same capability table and the same handler registry the single-name form does, so the
/// two cannot disagree about what is bindable.
///
/// # What it does not do
///
/// It binds only names the control actually publishes *and* the thread's registry holds. A name in
/// the registry that the control does not publish is left alone rather than logged as an error: a
/// host may register one handler set for several controls, and a control that cannot carry an event
/// is not a defect in the document. Each successful binding is additive, exactly as
/// [`bind_published_event`] is.
///
/// Returns the number of bindings made.
pub fn bind_all_published_events(widget_id: crate::core::ObjectId, widget_type: &str) -> usize {
    let mut bound = 0;
    for name in crate::json::events::global_handler_names() {
        if published_event_name(widget_type, &name).is_some()
            && bind_published_event(widget_id, widget_type, &name, &name)
        {
            bound += 1;
        }
    }
    bound
}

/// Wires one **compatibility** key (`on_click`, `on_change`, …) to a live control.
///
/// See [`bind_published_event`] for why this is public. `key` must be one the marker table lists;
/// an unknown key is reported and refused rather than guessed at.
///
/// # Which channel the key reaches
///
/// The marker decides, not this function: the widget's own base signal, the app layer's
/// value-trigger channel, or — for an intent no widget callback can carry, such as
/// `on_double_click` — the published name its capability counterpart declares. The channel choice
/// and the refusal when none is available are therefore the same in the loader and in a generated
/// program, which is the point of routing both through [`crate::json::bind_event_binding`].
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

    /// W-303: a name the control does not publish is refused, never retargeted to `clicked`.
    ///
    /// The defect was `unwrap_or("clicked")`: a misspelt `events` name bound the click callback and
    /// ran on the wrong trigger while reporting the right one. The binding now carries the sentinel,
    /// which the binder refuses because the control resolves no signal for it.
    #[test]
    fn an_undeclared_published_name_is_not_retargeted_to_clicked() {
        let binding = json_event_binding("button", "no_such_event");
        let JsonEventBinding::Published { name, marker, .. } = binding else {
            panic!("a published lookup must produce a published binding");
        };
        assert_eq!(name, UNRESOLVED_PUBLISHED_NAME);
        assert_ne!(name, "clicked", "an unknown name must not become the click wire");
        // The intent classifier still reads the spelling the author wrote, so a diagnostic can say
        // what they meant; the name is what refuses the binding.
        assert!(matches!(marker, JsonTriggerMarker::Clicked));
        assert!(!bind_published_event(0u64, "button", "no_such_event", "on_typo"));
    }

    /// W-302: every shipped marker has a published spelling, including the two focus intents.
    #[test]
    fn every_marker_has_a_published_counterpart() {
        for (marker, name) in MARKER_PUBLISHED_EVENTS {
            assert!(name.is_some(), "`{marker:?}` must name a published event");
            assert_eq!(marker.published_counterpart(), *name);
            assert!(marker.is_publishable());
        }
        assert_eq!(JsonTriggerMarker::FocusGained.published_counterpart(), Some("focus_gained"));
        assert_eq!(JsonTriggerMarker::FocusLost.published_counterpart(), Some("focus_lost"));
        assert_eq!(markers_without_published_counterpart().count(), 0);
    }

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
    fn a_handler_is_told_the_nearest_kind_the_platform_enum_has() {
        // No `WidgetTriggerKind` variant names a second click, so a double click reports the click
        // kind — the marker, not the kind, is what keeps `on_double_click` off the click signal.
        assert_eq!(
            JsonTriggerMarker::DoubleClicked.trigger_kind(),
            JsonTriggerMarker::Clicked.trigger_kind()
        );
        // No variant names a focus change either, and both nearer claims are false (`ValueChanged`
        // says the value changed, `Clicked` says the control was activated), so the focus markers
        // report `Unknown` rather than a specific lie.
        assert_eq!(
            JsonTriggerMarker::FocusGained.trigger_kind(),
            crate::platform::WidgetTriggerKind::Unknown
        );
        assert_eq!(
            JsonTriggerMarker::FocusLost.trigger_kind(),
            crate::platform::WidgetTriggerKind::Unknown
        );
    }

    /// A marker whose intent **no widget callback** can carry states so, and the loader then picks
    /// the channel that can. `trigger_kind` collapses `DoubleClicked` onto `Clicked` precisely
    /// because the click callback cannot carry the difference — so binding it there made the handler
    /// fire on a single click while being told a double click had happened.
    ///
    /// `Closed` and the two focus markers are deliberately **not** in this list any more: each has a
    /// real channel of its own (the widget's `closed` / `focus_gained` / `focus_lost` signals, see
    /// [`JsonTriggerMarker::base_signal`]), so they are bound rather than rerouted.
    #[test]
    fn a_marker_no_callback_can_carry_states_so_and_keeps_a_published_route() {
        let marker = JsonTriggerMarker::DoubleClicked;
        let reason = marker
            .unroutable_reason()
            .expect("`on_double_click` must state why a widget callback cannot carry it");
        assert!(
            reason.contains("on_") && reason.contains("double_clicked"),
            "the reason must name both the key an author wrote and the published name that does \
             carry it: {reason}"
        );
        assert_eq!(
            marker.published_counterpart(),
            Some("double_clicked"),
            "a marker refused by the callback route must have a published route to fall back on"
        );
    }

    /// Every marker has a channel, so the refusal cannot quietly grow to cover a working key.
    ///
    /// This is the invariant the loader relies on: a marker is either carried by the widget's own
    /// base signal, by the value-trigger channel, or by its published counterpart. A marker with
    /// none of the three would be a key that validates and never fires — exactly what rule #97
    /// forbids — so it must report a reason instead.
    #[test]
    fn every_marker_has_a_channel_or_a_stated_reason() {
        use JsonTriggerMarker::*;
        for marker in
            [Clicked, DoubleClicked, FocusGained, FocusLost, ValueChanged, SelectionChanged, Closed]
        {
            let channels = usize::from(marker.base_signal().is_some())
                + usize::from(marker.uses_value_callback())
                + usize::from(marker.published_counterpart().is_some());
            assert!(
                channels >= 1,
                "{marker:?} has no channel at all, so a handler could never run"
            );
            if marker.unroutable_reason().is_some() {
                assert!(
                    marker.published_counterpart().is_some(),
                    "{marker:?} is refused by the callback route with no published name to fall \
                     back on, so the binding would be dropped"
                );
            }
        }
    }

    /// The focus intents travel on the widget's own focus signals (W-05).
    ///
    /// # The defect this pins
    ///
    /// `on_focus` / `on_blur` were refused outright: the only callbacks the loader could reach were
    /// the click one and the value one, and neither fires on focus. They have no published
    /// counterpart either (the capability table publishes no per-control focus event), so both
    /// routes were dead. Pointing the markers at `BaseWidget::focus_gained` / `focus_lost` — which
    /// the runtime's focus router emits — is the channel that makes them route.
    #[test]
    fn the_focus_markers_route_on_the_widgets_own_signals() {
        assert_eq!(JsonTriggerMarker::FocusGained.base_signal(), Some(BaseSignal::FocusGained));
        assert_eq!(JsonTriggerMarker::FocusLost.base_signal(), Some(BaseSignal::FocusLost));
        assert!(
            JsonTriggerMarker::FocusGained.unroutable_reason().is_none()
                && JsonTriggerMarker::FocusLost.unroutable_reason().is_none(),
            "the focus markers now have a real emitter, so refusing them would drop the handler"
        );
        assert!(
            !JsonTriggerMarker::FocusGained.uses_value_callback()
                && !JsonTriggerMarker::FocusLost.uses_value_callback(),
            "a focus change is not a value change"
        );
    }

    /// A marker with a published counterpart is still refused **by the callback route**, so the
    /// loader cannot accidentally bind `on_double_click` to a single click.
    ///
    /// `Closed` is here because its producer now exists.
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
    /// `closed` then travels the widget's `closed` signal, and `double_clicked` is carried by its
    /// published name rather than by a widget callback.
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
        // signal, so it is **routed** (not refused); `double_clicked` is refused by the *callback*
        // route and wired through its published name instead (see `unroutable_reason`).
        assert!(
            JsonTriggerMarker::for_published_name("closed", false).uses_close_callback(),
            "`closed` must route to the close channel"
        );
        assert!(
            JsonTriggerMarker::for_published_name("closed", false).unroutable_reason().is_none(),
            "`closed` has a producer now, so it must not be refused"
        );
        let double_click = JsonTriggerMarker::for_published_name("double_clicked", false);
        assert!(double_click.unroutable_reason().is_some());
        assert_eq!(
            double_click.published_counterpart(),
            Some("double_clicked"),
            "the callback route's refusal must leave the published name as the channel"
        );
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

    /// Every accepted spelling of one published name resolves to the **same** marker.
    ///
    /// # The defect this pins (BLUE-issue E-24)
    ///
    /// `normalize_key` accepts `selection-changed`, `selection changed` and `SELECTIONCHANGED` as the
    /// same name, but the marker classifier compared the raw spelling, so the hyphen form fell
    /// through to the payload question and reported `ValueChanged` while the underscore form
    /// reported `SelectionChanged`. The same event therefore reported different trigger kinds — and
    /// so reached different callbacks — depending on how the author spelled the name.
    #[test]
    fn every_accepted_spelling_of_one_name_resolves_to_the_same_marker() {
        for spelling in [
            "selection_changed",
            "selection-changed",
            "selection changed",
            "SelectionChanged",
            "SELECTION_CHANGED",
        ] {
            assert_eq!(
                JsonTriggerMarker::for_published_name(spelling, false),
                JsonTriggerMarker::SelectionChanged,
                "`{spelling}` must classify as a selection change"
            );
        }

        for spelling in ["double_clicked", "double-clicked", "DoubleClicked"] {
            assert_eq!(
                JsonTriggerMarker::for_published_name(spelling, false),
                JsonTriggerMarker::DoubleClicked,
                "`{spelling}` must classify as a double click"
            );
        }

        for spelling in ["closed", "close", "_closed", "page_closed"] {
            assert_eq!(
                JsonTriggerMarker::for_published_name(spelling, false),
                JsonTriggerMarker::Closed,
                "`{spelling}` must classify as a close"
            );
        }
    }

    /// Gate for BIND-12: every marker the compatibility route can express must have an `on_*` key,
    /// and the forward/reverse lookups must agree.
    ///
    /// Adding a `JsonTriggerMarker` variant now fails here until the key table lists a spelling for
    /// it — the drift the old hand-written extraction permitted. The alias pair is asserted too:
    /// `on_change` and `on_value_changed` are both real spellings of the same marker, so a future
    /// edit that "simplifies" one away is caught rather than silently dropping documents' handlers.
    #[test]
    fn every_marker_has_a_compatibility_key_and_round_trips() {
        use JsonTriggerMarker::*;
        for marker in
            [Clicked, DoubleClicked, FocusGained, FocusLost, ValueChanged, SelectionChanged, Closed]
        {
            let key = marker_key_for(marker)
                .unwrap_or_else(|| panic!("{marker:?} has no `on_*` compatibility key"));
            assert_eq!(marker_for_key(key), Some(marker), "`{key}` must round-trip");
        }

        assert_eq!(marker_for_key("on_change"), Some(ValueChanged));
        assert_eq!(marker_for_key("on_value_changed"), Some(ValueChanged));
    }
}
