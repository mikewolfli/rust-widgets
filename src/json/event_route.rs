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

use crate::compat::String;
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
/// is what lets the gate in `tools/check_json_event_route.py` check it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonEventBinding {
    /// A name the capability table publishes. Wired to the control's own signal, so a control
    /// that gains a published event becomes declarable without touching the loader.
    Published {
        /// The published event name, spelled exactly as `connect_event` accepts it.
        name: &'static str,
        /// The trigger marker reported to the handler. `None` for a payload-free published name,
        /// because the handler already knows which name it was bound to; a marker would be a
        /// second spelling of the same fact.
        marker: Option<JsonTriggerMarker>,
    },
    /// A compatibility key whose meaning is a trigger *intent*, not a published name.
    Marker {
        /// The `on_*` key as it appears in the document.
        key: &'static str,
        /// The marker the handler receives.
        marker: JsonTriggerMarker,
    },
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

/// A handler named in a layout, resolved against the node it was declared on.
///
/// Owned rather than borrowed: the loader hands this to a closure that outlives the JSON
/// document, so a `&str` into the parsed tree would not compile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredHandler {
    /// The global handler name to invoke.
    pub handler: String,
    /// The trigger the handler is told about.
    pub marker: JsonTriggerMarker,
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
}
