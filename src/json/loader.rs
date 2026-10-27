// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! JSON layout loader — parses JSON source and instantiates widget trees.
//!
//! The [`JsonLoader`] reads a JSON string, recursively creates
//! widget instances, applies properties (geometry, style, events),
//! and returns a [`BoundJsonLayout`](crate::json::BoundJsonLayout)
//! for typed widget access.
//!
//! # i18n Support
//!
//! Text properties (`title`, `text`, `tooltip`, `placeholder`) accept
//! translation keys. Use the [`crate::i18n::I18nManager::translate`] or
//! the [`crate::tr!`] macro at the call site to resolve the key before
//! passing JSON to the loader. The loader itself does NOT call the
//! i18n manager – it uses the literal string values from JSON.
//!
//! # Event Binding
//!
//! When a JSON node declares `"on_click": "handler_name"`, the string
//! is stored in the loader and connected after instantiation.
//! Callers use [`EventHandlerMap`](crate::json::EventHandlerMap) to
//! register closures against those handler names.

use std::borrow::Cow;

use serde_json::Value;

use crate::app::{ButtonHandle, WidgetHandle};
use crate::core::{Color, ObjectId, Rect};
use crate::json::properties::ApplyOutcome;
use crate::json::{
    add_spacer_to_layout, add_widget_to_layout, apply_layout, create_layout_from_kind,
    parse_layout_kind, place_widget_in_layout, place_widget_in_region_layout, store_layout,
    BoundJsonLayout, ChildLayoutAttrs, ChildPlacement,
};
use crate::layout::inspector::LayoutInspector;
use crate::widget::{
    Button, CheckBox, ComboBox, GroupBox, Label, LineEdit, ListBox, ProgressBar, RadioButton,
    ScrollArea, ScrollBar, Slider, SpinBox, Switch, Widget,
};
#[cfg(not(alloc_frugal))]
use crate::widget::{
    ColorDialog, FileDialog, FontDialog, GridWidget, ListView, MessageBox, TabWidget, TextEdit,
};
use crate::window::Window;
// `WidgetKind` is named only by the test-only `infer_kind` table below; production
// registration reads the live control's kind instead.
#[cfg(test)]
use crate::index::WidgetKind;

/// Maximum nested depth for recursive instantiation (prevents stack overflow).
const MAX_DEPTH: u32 = 64;

/// The declaration a **published-name** wire was reached from, for its refusal diagnostic.
///
/// # Why the message needs to know
///
/// Two spellings reach one published signal: `events: { "double_clicked": "h" }` and the
/// compatibility key `on_double_click`, whose intent only the published name can carry (W-06). A
/// refusal that always said `events.<name>` was wrong for the second: it named a key the document
/// never wrote and never said what to do instead. The refusal is where an author learns that the
/// key they used has a published name, so it has to name that key.
#[derive(Debug, Clone, Copy)]
enum DeclaredWire {
    /// `events: { "<name>": "<handler>" }` — the author wrote the published name.
    Events,
    /// An `on_*` key whose published counterpart is the channel; carries the key the author wrote.
    Marker(&'static str),
}

/// A loader that parses JSON layout strings and instantiates widget trees.
pub struct JsonLoader;

impl JsonLoader {
    /// Parse a JSON layout string and instantiate the widget tree.
    ///
    /// Returns a [`BoundJsonLayout`] for typed widget access.
    ///
    /// # Errors
    ///
    /// Returns an error if JSON parsing fails, an unknown widget type is
    /// encountered, or the widget tree exceeds `MAX_DEPTH`.
    pub fn load(json_str: &str) -> Result<BoundJsonLayout, String> {
        Self::load_with_optional_icon_resolver(json_str, None)
    }

    /// Load a JSON layout while resolving resource-backed `icon_source` properties explicitly.
    ///
    /// The resolver owns resource identity and policy; it is never treated as a file path by this
    /// loader. [`Self::load`] remains resolver-free and reports unresolved resources through the
    /// normal property-rejection diagnostic path.
    pub fn load_with_icon_resolver(
        json_str: &str,
        resolver: &dyn crate::widget::icon_source::IconResourceResolver,
    ) -> Result<BoundJsonLayout, String> {
        Self::load_with_optional_icon_resolver(json_str, Some(resolver))
    }

    fn load_with_optional_icon_resolver(
        json_str: &str,
        resolver: Option<&dyn crate::widget::icon_source::IconResourceResolver>,
    ) -> Result<BoundJsonLayout, String> {
        let value: Value = serde_json::from_str(json_str).map_err(|e| {
            format!("layout JSON ({} bytes) could not be parsed: {e}", json_str.len())
        })?;
        let mut registry = crate::index::WidgetRegistry::new();
        let mut binding = BoundJsonLayout::new();

        let root = value.as_object().ok_or_else(|| "JSON root must be an object".to_string())?;

        // The root should have exactly one top-level key (the widget type).
        if root.len() != 1 {
            return Err(format!(
                "JSON root must have exactly one widget type, found {} keys",
                root.len()
            ));
        }

        let (widget_type, widget_value) = match root.iter().next() {
            Some(pair) => pair,
            None => {
                return Err("JSON root object is empty — expected a widget type key".to_string())
            }
        };
        Self::instantiate_node(
            widget_type,
            widget_value,
            None,
            &mut registry,
            &mut binding,
            0,
            resolver,
        )?;

        // Run layout diagnostics after all widgets are created.
        LayoutInspector::run_once_logged(&registry);

        Ok(binding)
    }

    /// Wires the handlers a node declares, for both routes.
    ///
    /// # Why one function for two key spellings
    ///
    /// The published route (`events`) and the compatibility route (`on_*`) reach the same
    /// [`EventHandlerContext`] through the same marker table — see `crate::json::event_route`.
    /// Splitting them gave the loader two code paths that had to agree about trigger kinds and
    /// about which handlers are invoked, with nothing checking that they did.
    ///
    /// # What is reported rather than ignored
    ///
    /// A name under `events` that the control does not publish is **dropped with a warning**
    /// rather than silently wired. A designer that misspells a published name would otherwise
    /// get a handler that never runs; the warning is what makes that visible in the host's log.
    /// The reverse case — an `on_*` key the marker table does not list — cannot reach here,
    /// because the table *is* the list of keys this reads.
    fn bind_declared_events(
        widget_id: ObjectId,
        widget_type: &str,
        obj: &serde_json::Map<String, Value>,
    ) {
        // ── route 1: published names ──
        if let Some(events) = obj.get(crate::json::EVENTS_KEY).and_then(|v| v.as_object()) {
            for (name, handler) in events {
                let Some(handler) = handler.as_str() else {
                    log::warn!(
                        "[{widget_type}] `{EVENTS_KEY}.{name}` must be a handler name string, \
                         found {found:?}; the binding is skipped",
                        EVENTS_KEY = crate::json::EVENTS_KEY,
                        found = handler
                    );
                    continue;
                };
                if !Self::control_publishes(widget_type, name) {
                    log::warn!(
                        "[{widget_type}] `{EVENTS_KEY}.{name}` is not a published event \
                         (publishes: {published}); the binding is skipped, so the handler \
                         would never run",
                        EVENTS_KEY = crate::json::EVENTS_KEY,
                        published = Self::published_events(widget_type).join(", ")
                    );
                    continue;
                }
                Self::bind_one(
                    widget_id,
                    crate::json::json_event_binding(widget_type, name),
                    handler.to_owned(),
                );
            }
        }

        // ── route 2: compatibility keys ──
        for (key, marker) in crate::json::MARKER_KEYS {
            let Some(handler) = obj.get(*key).and_then(|v| v.as_str()) else {
                continue;
            };
            Self::bind_one(
                widget_id,
                crate::json::JsonEventBinding::Marker { key, marker: *marker },
                handler.to_owned(),
            );
        }
    }

    /// Subscribes one declared handler to `widget_id` through the callback the event needs.
    ///
    /// # Additive, not idempotent (BLUE-issue E-22)
    ///
    /// Calling this twice with the same `(widget_id, binding, handler_name)` subscribes the handler
    /// **twice**: one control emission runs it twice. That is deliberate \u2014 two legitimate
    /// bindings that happen to share a handler name are two subscribers \u2014 but it means this
    /// call is not a safe "ensure wired" operation. A host that must be able to release a single
    /// dynamic binding it added (a designer replacing one wire without destroying the control)
    /// should use [`Self::bind_one_releasable`], which returns a token; this convenience form
    /// retains it in `LOADED_BINDINGS` for the load-time case where the whole layout is torn down
    /// together.
    ///
    /// # Both routes yield a retained token (W-04)
    ///
    /// A published name and an `on_*` marker each produce a [`crate::json::DynamicBinding`], and both
    /// are retained keyed by the control. The marker route used to bind and report no token, so
    /// unloading or rebuilding a document released the published half of its wiring and left the
    /// marker half connected until the control itself was destroyed. The public shape is unchanged
    /// (`bool` in, `bool` out); the token channel is what got completed.
    pub fn bind_one(
        widget_id: ObjectId,
        binding: crate::json::JsonEventBinding,
        handler_name: String,
    ) -> bool {
        match Self::bind_wiring(widget_id, binding, handler_name) {
            Some(token) => {
                // The binding must outlive this call (it is the load-time wire), so its token is
                // **retained** keyed by the control rather than dropped or forgotten. Dropping it
                // here would release the subscription immediately; forgetting it leaked the token's
                // captured signal reference past the control's own destruction (BLUE-issue E-27). It
                // is released when the control is torn down or its node detached.
                crate::json::event_route::retain_binding(widget_id, token);
                true
            }
            None => false,
        }
    }

    /// Builds the subscription for `binding` and the token that releases it, without retaining it.
    ///
    /// # Why one constructor for both callers
    ///
    /// [`Self::bind_one`] retains the token as the document's wire and
    /// [`Self::bind_one_releasable`] hands the same token to a host that owns its own wire. Having
    /// the marker route return a bare `bool` from a *different* function is what let the token gap
    /// (W-04) survive: the published path had a handle to give back and the marker path did not, so
    /// only one of them was releasable.
    fn bind_wiring(
        widget_id: ObjectId,
        binding: crate::json::JsonEventBinding,
        handler_name: String,
    ) -> Option<crate::json::DynamicBinding> {
        match binding.published_name() {
            Some(name) => Self::bind_published_dynamic(
                widget_id,
                name,
                binding.marker(),
                handler_name,
                DeclaredWire::Events,
            ),
            None => Self::bind_marker_wiring(widget_id, binding.marker(), handler_name),
        }
    }

    /// Wires one `on_*` marker through the channel its intent names.
    ///
    /// # The decision is the marker's, not this function's
    ///
    /// Three questions on [`crate::json::JsonTriggerMarker`], asked in this order, are the whole
    /// routing table — there is no per-key `match` here for a new key to drift away from:
    ///
    /// 1. `base_signal` — the widget's own `clicked` / `closed` / `focus_gained` / `focus_lost`.
    /// 2. `uses_value_callback` — the app layer's value-trigger channel.
    /// 3. `published_counterpart` — a published name for an intent no widget callback can carry
    ///    (`on_double_click`), wired through the control's own dynamic signal for that name.
    ///
    /// A marker with none of the three is refused with its stated reason rather than attached to a
    /// callback that fires on a different event (rule #97).
    fn bind_marker_wiring(
        widget_id: ObjectId,
        marker: crate::json::JsonTriggerMarker,
        handler_name: String,
    ) -> Option<crate::json::DynamicBinding> {
        // 1. A base-widget signal: `clicked`, `closed`, and the two focus signals the runtime's
        //    focus router emits through `BaseWidget::handle_event` (W-05).
        if let Some(signal) = marker.base_signal() {
            return Self::bind_base_signal(widget_id, marker, signal, handler_name);
        }

        // 2. The value-trigger channel: `on_change` / `on_value_changed` / `on_selection_changed`.
        if marker.uses_value_callback() {
            return Self::bind_value_trigger(widget_id, marker, handler_name);
        }

        // 3. An intent no widget callback can carry (W-06). Its published name is the channel, and
        //    the control has to publish it: the loader must not fall back to the click callback,
        //    which would run the handler on the first click of a double click. The refusal names the
        //    key the author wrote, because a document that declared `on_double_click` needs to know
        //    which published name carries it.
        if let Some(name) = marker.published_counterpart() {
            return Self::bind_published_dynamic(
                widget_id,
                name,
                marker,
                handler_name,
                crate::json::marker_key_for(marker)
                    .map_or(DeclaredWire::Events, DeclaredWire::Marker),
            );
        }

        let reason = marker.unroutable_reason().unwrap_or(
            "this `on_*` key has no base signal, no value trigger and no published name to reach",
        );
        log::warn!(
            "on_* key for handler '{handler_name}' on id={widget_id} was NOT bound: {reason}"
        );
        crate::app::record_unwired_binding(widget_id);
        None
    }

    /// Connects a marker to one of the widget's own base signals.
    ///
    /// # Why the JSON layer connects the signal itself (W-04)
    ///
    /// `WidgetHandle::on_click` / `on_close` connect these same signals but keep the
    /// `ConnectionHandle` in a private per-widget table, so a wire made through them cannot be
    /// removed without removing every callback the control has — which is not a release, it is a
    /// demolition. A [`crate::json::DynamicBinding`] needs a real disconnect, so the signal is cloned
    /// out of the live control and subscribed here: the same `base.clicked` / `base.closed` /
    /// `base.focus_gained` / `base.focus_lost` connection the handle would have made, with the
    /// handle kept where the token can reach it.
    ///
    /// A control's teardown drops its signals, so a binding released here and a binding whose
    /// control is destroyed both end with nothing running; the difference is that this one can be
    /// released while the control is still alive.
    fn bind_base_signal(
        widget_id: ObjectId,
        marker: crate::json::JsonTriggerMarker,
        signal: crate::json::event_route::BaseSignal,
        handler_name: String,
    ) -> Option<crate::json::DynamicBinding> {
        use crate::json::event_route::BaseSignal;
        let source = crate::widget::runtime::with_widget(widget_id, |widget| {
            let base = widget.base();
            match signal {
                BaseSignal::Clicked => base.clicked.clone(),
                BaseSignal::Closed => base.closed.clone(),
                BaseSignal::FocusGained => base.focus_gained.clone(),
                BaseSignal::FocusLost => base.focus_lost.clone(),
            }
        });
        let Some(source) = source else {
            Self::report_unwired_marker(widget_id, &handler_name);
            return None;
        };
        let release_source = source.clone();
        let handle = source.connect(move || {
            let ctx = crate::json::context_for(widget_id, marker);
            crate::json::invoke_global_handler(&handler_name, &ctx);
        });
        Some(crate::json::DynamicBinding::new(alloc::boxed::Box::new(move || {
            release_source.disconnect(handle);
        })))
    }

    /// Connects a marker to the app layer's value-trigger channel.
    ///
    /// # Why this channel's token is a liveness flag (W-04)
    ///
    /// `WidgetHandle::on_value_changed` is the only writer of the value-trigger table, and that table
    /// is keyed by widget rather than by subscription — it exposes no per-callback removal. A
    /// document-level release therefore flips this binding's own flag, so a released wire cannot run
    /// its handler; the registration itself is reclaimed when the control is torn down
    /// (`app::remove_callbacks`). The alternative — reporting a token whose `release` silently did
    /// nothing — is the fake release the token exists to rule out, so the boundary is stated here
    /// instead of left for a reader to discover.
    ///
    /// The flag is per binding, not per handler name: two nodes that name the same handler are two
    /// subscriptions, and releasing one must leave the other running.
    fn bind_value_trigger(
        widget_id: ObjectId,
        marker: crate::json::JsonTriggerMarker,
        handler_name: String,
    ) -> Option<crate::json::DynamicBinding> {
        if !crate::widget::runtime::is_mounted(widget_id) {
            // Ask the same question `on_value_changed` asks, so a token is never handed out for a
            // wire that was not installed.
            Self::report_unwired_marker(widget_id, &handler_name);
            return None;
        }
        let live = crate::compat::Arc::new(core::sync::atomic::AtomicBool::new(true));
        let gate = crate::compat::Arc::clone(&live);
        let handle: ButtonHandle = ButtonHandle::from_raw(widget_id);
        handle.on_value_changed(move |_value: &str| {
            if !gate.load(core::sync::atomic::Ordering::Relaxed) {
                return;
            }
            let ctx = crate::json::context_for(widget_id, marker);
            crate::json::invoke_global_handler(&handler_name, &ctx);
        });
        Some(crate::json::DynamicBinding::new(alloc::boxed::Box::new(move || {
            live.store(false, core::sync::atomic::Ordering::Relaxed);
        })))
    }

    /// Reports a marker binding that had no live channel, at the level a host can see.
    ///
    /// A handler that can never run is the failure a subscriber cannot observe from the outside, so
    /// it is reported at `warn!` and counted by `app::unwired_binding_count`. Unlike
    /// [`Self::bind_marker_wiring`]'s refusal this case is about the *control*: it is not there.
    fn report_unwired_marker(widget_id: ObjectId, handler_name: &str) {
        log::warn!(
            "the `on_*` key for handler '{handler_name}' on id={widget_id} was NOT bound: the id \
             addresses no live widget, so there is no signal for its trigger to arrive on"
        );
        crate::app::record_unwired_binding(widget_id);
    }

    /// Like [`Self::bind_one`], but returns a token that releases exactly the binding it added.
    ///
    /// # Both routes return a token (W-04)
    ///
    /// The published route's subscription is on the control's own dynamic signal, whose handle is
    /// the only key that can remove it; the marker route now yields a token too — a real disconnect
    /// for the base signals it connects, and a liveness flag for the value-trigger table, which has
    /// no per-callback removal (see `Self::bind_value_trigger`). A host that added a wire at run
    /// time can therefore take it back without destroying the control, whichever spelling the
    /// document used; returning `None` now means the binding was *refused*, not that it was made
    /// silently.
    pub fn bind_one_releasable(
        widget_id: ObjectId,
        binding: crate::json::JsonEventBinding,
        handler_name: String,
    ) -> Option<crate::json::DynamicBinding> {
        Self::bind_wiring(widget_id, binding, handler_name)
    }

    /// Wires a **published** event name to the control's own dynamic signal.
    ///
    /// # Why this rather than the generic callback tables
    ///
    /// `Widget::event_signal_dyn` is the single join from a published name to the signal the control
    /// actually emits (see [`crate::signal::EventSignalBinder`], which wires `forward_all` through the
    /// same call). Binding a published name through `base.clicked` instead was a second, weaker join
    /// that ignored the name: every payload-free event resolved to the click signal, so `canceled`
    /// fired for a click and never for a cancel. Routing through `event_signal_dyn` also delivers the
    /// event's **payload** to the handler ([`EventHandlerContext::payload`]), which the generic
    /// callbacks discarded.
    ///
    /// # Returns
    ///
    /// `None`, with a warning, when the control does not resolve the name. That case means the name
    /// is published but has no live signal — the exact "subscribed successfully but will never fire"
    /// state rule #97 rules out — so it is reported rather than silently falling back to a callback
    /// that would run on the wrong event.
    ///
    /// Otherwise `Some(token)`, whose `release` disconnects **exactly** this subscription. Two calls
    /// with the same arguments subscribe twice (additive, BLUE-issue E-22), and each token releases
    /// its own subscription, so a host that replaces one dynamic wire can do so without disturbing
    /// the others or destroying the control.
    fn bind_published_dynamic(
        widget_id: ObjectId,
        name: &'static str,
        marker: crate::json::JsonTriggerMarker,
        handler_name: String,
        declared_as: DeclaredWire,
    ) -> Option<crate::json::DynamicBinding> {
        let reference =
            crate::widget::runtime::with_widget(widget_id, |widget| widget.event_signal_dyn(name));
        let Some(reference) = reference.flatten() else {
            match declared_as {
                DeclaredWire::Events => log::warn!(
                    "`events.{name}` for handler '{handler_name}' on id={widget_id} was NOT bound: \
                     the control does not resolve this published name to a signal, so the handler \
                     would never run"
                ),
                DeclaredWire::Marker(key) => log::warn!(
                    "`{key}` for handler '{handler_name}' on id={widget_id} was NOT bound: it is \
                     carried by the published event `{name}`, which this control does not publish \
                     or cannot resolve — see the document's `events` validation output for the \
                     names it does publish. The binding is skipped rather than attached to a \
                     callback that fires on a different event"
                ),
            }
            crate::app::record_unwired_binding(widget_id);
            return None;
        };
        // The slot forwards the trigger **and its payload**. The reference's own name is used (not
        // the caller's spelling) so the name a subscriber reads is the one the control answers to.
        //
        // # Thread affinity (BLUE-issue E-25, identity corrected in E-26)
        //
        // The handler registry is thread-local, so a signal emitted from another thread looked up
        // that thread's (empty) registry and skipped the handler **silently**. The binding records the
        // thread it was made on and reports a mismatch instead of pretending the handler ran, so
        // "fired off the wrong thread" is observable rather than indistinguishable from "no handler
        // was registered".
        //
        // The comparison is on the thread's **`ThreadId`**, not its name: names are not unique, so two
        // threads sharing a name compared equal and one's emission was mis-taken for a same-thread
        // delivery (BLUE-issue E-26). The names are still captured for the diagnostic message only.
        //
        // # Why the comparison no longer costs a `Thread` handle per emit (W-P07)
        //
        // The binding thread's id is captured once, here. The *current* id is read through
        // `events::current_thread_id`, which caches it per thread: it used to call
        // `std::thread::current()` on every emission, and that clones the thread's `Arc` handle — an
        // atomic increment on the hot path whose only use was to read an immutable id. A driver of a
        // value-carrying signal (a slider drag) pays this per emit.
        let bound_thread_id = crate::json::events::current_thread_id();
        let bound_thread_name = crate::json::events::current_thread_name();
        let handle = reference.subscribe(Box::new(
            move |payload: &crate::widget::capability::CapabilityValue| {
                if crate::json::events::current_thread_id() != bound_thread_id {
                    crate::json::events::record_cross_thread_skip(
                        &handler_name,
                        &bound_thread_name,
                        &crate::json::events::current_thread_name(),
                    );
                    return;
                }
                // # Why the payload clone is skipped when nobody listens (W-P07)
                //
                // The context owns its payload, so building one for a name with no registered
                // handler clones the value — a `String`/`List` payload allocates — and then drops it
                // unread. `has_global_handler` reads the same thread-local registry
                // `invoke_global_handler` would, so skipping here is exactly the state in which the
                // registry would have reported "not found": delivery semantics are unchanged.
                if !crate::json::events::has_global_handler(&handler_name) {
                    return;
                }
                let ctx = crate::json::context_for_with_payload(widget_id, marker, payload.clone());
                crate::json::invoke_global_handler(&handler_name, &ctx);
            },
        ));
        // Resolve once more for the release closure: `EventSignalRef` is not `Clone`, and its
        // `disconnect` is per-call, so the token captures a fresh lookup and the handle it removes.
        let mut release_reference =
            crate::widget::runtime::with_widget(widget_id, |widget| widget.event_signal_dyn(name))
                .flatten();
        let release = alloc::boxed::Box::new(move || {
            if let Some(reference) = release_reference.as_mut() {
                reference.disconnect(handle);
            }
        });
        Some(crate::json::DynamicBinding::new(release))
    }

    /// Places a child in the region it declared, or at the cell it declared.
    ///
    /// # Why this exists at all
    ///
    /// `ChildLayoutAttrs::from_value` parsed `col`, `row`, `col_span` and `row_span` into
    /// [`ChildLayoutAttrs`], and both `add_widget_to_layout` call sites then read `attrs.stretch`
    /// and dropped the other four fields. A document that placed a control at an explicit cell
    /// therefore got auto-placement, silently — the parse was the only evidence the key had ever
    /// been seen, and it was thrown away one line later. The keys were also absent from
    /// [`is_loader_owned_key`], so the name-driven property pass additionally reported them as
    /// *"no construction arm consumed it; the value was ignored"* — a warning about a value this
    /// function now consumes.
    ///
    /// # Why a non-grid parent is reported rather than ignored
    ///
    /// `col`/`row` mean nothing to a box, form or flex layout; the trait's `add_widget` carries no
    /// cell. The child is still placed (by the parent's own rule) rather than dropped, but the
    /// mismatch is logged, because a document that wrote a cell on a non-grid parent asked for
    /// something the parent cannot do and would otherwise read as "the grid ignored my column".
    ///
    /// A declared `region` takes precedence over a declared cell: a child cannot be in two
    /// placement systems at once, and the region names a stronger constraint (a fixed track) than
    /// a cell does.
    fn apply_child_placement(
        child_id: ObjectId,
        attrs: &ChildLayoutAttrs,
        child_type: &str,
        parent_id: ObjectId,
    ) {
        if let Some(region) = attrs.region {
            if !place_widget_in_region_layout(
                child_id,
                region,
                attrs.extent.unwrap_or(0),
                parent_id,
            ) {
                log::warn!(
                    "JSON layout: '{child_type}' declares region '{}' but its parent has no \
                     border layout; the child was placed by the parent's own rule instead",
                    region.token()
                );
            }
            return;
        }
        if attrs.col.is_none() && attrs.row.is_none() {
            return;
        }
        let placement = ChildPlacement {
            col: attrs.col.unwrap_or(0),
            row: attrs.row.unwrap_or(0),
            col_span: attrs.col_span.unwrap_or(1),
            row_span: attrs.row_span.unwrap_or(1),
        };
        if !place_widget_in_layout(child_id, placement, parent_id) {
            log::warn!(
                "JSON layout: '{child_type}' declares col/row placement but its parent has no \
                 cell-based layout (only 'grid' and 'uniform_grid' have cells); the child was \
                 placed by the parent's own rule instead"
            );
        }
    }

    /// Whether `widget_type` publishes `event_name`.
    ///
    /// # Why the answer is "no" on a stripped profile
    ///
    /// The capability table is compiled out of `mini`/`embedded`, and `crate::json` does not
    /// exist there either — this function is inside a `full_widgets` module, so the table is
    /// always present when it runs. The fallback is stated rather than assumed because a future
    /// gate change would otherwise turn it into a silent `true`.
    fn control_publishes(widget_type: &str, event_name: &str) -> bool {
        let factory = crate::widget::capability::WidgetFactory::shared();
        factory.capability(widget_type).is_some_and(|capability| {
            crate::widget::capability::find_event_schema(capability, event_name).is_some()
        })
    }

    /// The published event names of `widget_type`, for the warning above.
    fn published_events(widget_type: &str) -> Vec<&'static str> {
        let factory = crate::widget::capability::WidgetFactory::shared();
        factory
            .capability(widget_type)
            .map(|capability| capability.events.iter().map(|schema| schema.name).collect())
            .unwrap_or_default()
    }

    /// Recursively instantiate a single JSON node into a widget.
    #[allow(clippy::too_many_arguments)]
    fn instantiate_node(
        widget_type: &str,
        value: &Value,
        parent_id: Option<ObjectId>,
        registry: &mut crate::index::WidgetRegistry,
        binding: &mut BoundJsonLayout,
        depth: u32,
        resolver: Option<&dyn crate::widget::icon_source::IconResourceResolver>,
    ) -> Result<ObjectId, String> {
        if depth > MAX_DEPTH {
            return Err(format!(
                "widget tree is nested {depth} levels deep, which exceeds the maximum of \
                 {MAX_DEPTH}; flatten the layout to load it"
            ));
        }

        let obj = value
            .as_object()
            .ok_or_else(|| format!("'{widget_type}' value must be a JSON object"))?;

        // The node's name, lowercased once per node rather than once per test against it.
        // Three comparisons below ask "is this a button/label/window", and each used to spell
        // `widget_type.to_lowercase()` — a fresh `String` per question, per node, for a name
        // that is lowercase in the document almost every time.
        let widget_key = lowered(widget_type);

        // Get the widget ID (optional — auto-generated if missing).
        let id_str = obj.get("id").and_then(|v| v.as_str()).unwrap_or("");

        // Handle the "spacer" pseudo-widget
        if widget_type.eq_ignore_ascii_case("spacer") {
            let stretch = json_u32_at(obj, "stretch").unwrap_or(1);
            if let Some(pid) = parent_id {
                add_spacer_to_layout(stretch, pid);
            }
            return Ok(0);
        }

        // Handle the "layout" pseudo-widget
        if widget_type.eq_ignore_ascii_case("layout") {
            let kind = parse_layout_kind(value)?;
            let layout = create_layout_from_kind(&kind);
            let layout_parent = parent_id
                .ok_or_else(|| format!("'{widget_type}' layout must be a child of a widget"))?;

            // Stored *before* the children are instantiated, because registering a
            // child needs the layout to exist (`add_widget_to_layout` is a no-op
            // otherwise). Storing it afterwards — which this code used to do — left the
            // layout empty, so `apply_layout` computed an arrangement for no children
            // and no widget ever moved.
            store_layout(layout_parent, layout);

            // Process children
            if let Some(children) = obj.get("children").and_then(|v| v.as_array()) {
                for child_value in children {
                    if let Some(child_obj) = child_value.as_object() {
                        if child_obj.len() == 1 {
                            let (child_type, child_val) = child_obj.iter().next().unwrap();
                            if child_type.eq_ignore_ascii_case("spacer") {
                                let stretch = child_val
                                    .as_object()
                                    .and_then(|m| json_u32_at(m, "stretch"))
                                    .unwrap_or(1);
                                add_spacer_to_layout(stretch, layout_parent);
                                continue;
                            }

                            let child_id = Self::instantiate_node(
                                child_type,
                                child_val,
                                Some(layout_parent),
                                registry,
                                binding,
                                depth + 1,
                                resolver,
                            )?;

                            if child_id != 0 {
                                let attrs = ChildLayoutAttrs::from_value(child_val);
                                // The layout must already be stored for this to reach
                                // it: `add_widget_to_layout` is a no-op when the parent
                                // has none. `store_layout` used to run *after* this
                                // loop, so every child was added to nothing and the
                                // layout was empty when it was applied — which is why
                                // a declarative layout could compute geometries that no
                                // widget ever received.
                                add_widget_to_layout(child_id, attrs.stretch, layout_parent);
                                Self::apply_child_placement(
                                    child_id,
                                    &attrs,
                                    child_type,
                                    layout_parent,
                                );
                            }
                        }
                    }
                }
            }

            apply_layout(layout_parent, json_geometry(obj));
            return Ok(layout_parent);
        }

        // Create the widget
        let mut widget: Box<dyn Widget> = Self::create_widget(widget_type, obj)?;

        // The id the document wrote under `"id"`. Read here rather than after the
        // mount because the mount is what makes the id real: a widget's own `id()` is
        // only meaningful once it is registered, and the registry hands out the id.
        let declared_id = if id_str.is_empty() { 0 } else { parse_declared_id(id_str) };

        // Apply stylesheet rules first, so explicit JSON keys below win over the
        // stylesheet — the same precedence a browser gives inline styles over an
        // author stylesheet. `class` selects which rules apply; the widget's own
        // kind is matched automatically.
        apply_declared_styles(&mut *widget, obj);

        // Apply common properties (geometry, enabled, visible, tooltip, style).
        //
        // The min/max size constraints are applied **inside** this call: `apply_properties`
        // ends with `apply_size_constraints`. Calling it here as well was a second path to
        // the same concept (principle #101). It is idempotent today, which is exactly why the
        // duplicate was invisible — and exactly why a later edit that made it non-idempotent
        // (accumulating a constraint, say) would break silently, with the second call hidden
        // behind the first.
        apply_properties_with_icon_resolver(&mut *widget, obj, resolver);

        // Set parent
        widget.set_parent(parent_id);

        // Register — **mounting** the control, not merely indexing it.
        //
        // # The defect this closes
        //
        // The loader used to record the widget's *own* `id()` in `WidgetRegistry` and
        // `BoundJsonLayout` and drop the `Box<dyn Widget>` on the floor. Nothing reached
        // `crate::widget::runtime`, which is what `with_widget`/`with_widget_mut` read —
        // so every handle a caller resolved from this binding (`widget_by_name::<..>`, the
        // `on_click`/`on_value_changed` wiring, `set_widget_text`) addressed an id the
        // runtime had never heard of. Each of those paths *reports* the miss, but at
        // `debug!`, so at the default log level a loaded document was a tree of controls
        // that returned their defaults and ignored their handlers, with nothing said.
        //
        // The mount goes through the control backend's creation funnel, which is the one
        // place that assigns an id, registers it, applies the active theme, records both
        // directions of the parent/child relation, and gives a window its host surface.
        // Doing it by hand here would be a second implementation of that, and the second
        // one is the one that rots.
        let widget_id = mount_declared_widget(widget, parent_id, declared_id, widget_type);
        if widget_id == 0 && !id_str.is_empty() {
            // Name the document's own spelling of the id. The backend and mount warnings
            // this path already logs can only name the numeric id and the widget type, so
            // an author with two `"id": "submit_btn"` nodes could not tell which line to
            // fix: the load succeeds and the duplicate node simply has no live control.
            log::warn!(
                "[json] the declared id '{id_str}' could not be claimed; another widget \
                 already holds it and this node was not mounted"
            );
        }

        // The **live control's** kind, not a value derived from the type name.
        //
        // `infer_kind` is a hand-written table covering about a hundred names, while
        // the factory routes far more (canonical names plus aliases). Anything the
        // table did not name — `icon`, `table`, `chip`, `progress_bar`, … — fell
        // through its `_ => WidgetKind::Button` arm, so the registry recorded `Button`
        // for a control that is not one, and `LayoutInspector`/`by_kind` queries then
        // reported the wrong kind. The widget already knows its kind, so reading it
        // here removes the table's ability to disagree with reality.
        //
        // `infer_kind` is still the fallback for the case the live control cannot answer:
        // a thread with no widget runtime, so nothing was mounted and there is no `kind()`
        // to read. `Panel` is the neutral answer there — a control that did not mount has no
        // kind this loader can observe, and reporting `Button` (what the old table's `_` arm
        // did) was a claim about a widget that in fact does not exist.
        let kind = crate::widget::runtime::with_widget(widget_id, |live| live.kind())
            .unwrap_or(crate::index::WidgetKind::Panel);

        let label = if id_str.is_empty() {
            format!("{widget_type}_{widget_id}")
        } else {
            id_str.to_string()
        };

        registry.register(crate::index::WidgetEntry {
            id: widget_id,
            kind,
            parent: parent_id,
            label,
        });

        if !id_str.is_empty() {
            binding.register(id_str, widget_id);
        }

        // Structural index: record which node this is, under which parent, and with which
        // key. `id_str` doubles as the key because the JSON schema already requires it to be
        // unique per document (it is how `widget_by_name` addresses a control), so requiring
        // a second, separate `key` field would let the two disagree. A node without an `id`
        // therefore has no stable identity to diff on, and `node_key` reports `None` for it.
        binding.register_node(widget_id, widget_type, id_str, parent_id);

        // ── Apply text property via platform API ────────────
        // Text for widgets that accept it (checkbox, radiobutton,
        // groupbox title via "title" key, lineedit placeholder, etc.)
        // is set through the global set_widget_text function.
        //
        // i18n: If the text value is a translation key, resolve it
        // BEFORE passing to the loader (e.g. tr!("button.ok")).
        // The loader uses the literal string as-is.
        if let Some(text) = obj.get("text").and_then(|v| v.as_str()) {
            // `messagebox` consumed `text` (and `title`) in its `create_widget` arm, so the
            // post-pass below would set it a second time — one avoidable round trip and a
            // second text-changed signal per message box.
            if !text.is_empty() && !matches!(widget_key.as_ref(), "button" | "label" | "messagebox")
            {
                crate::set_widget_text(widget_id, text);
            }
        }
        // Handle "title" property — BLUE4.md spec says groupbox uses "title"
        // (not "text"). For window, title is already set in create_widget.
        // Only apply via platform API for non-window widgets. The same applies to
        // the kinds whose `create_widget` arm already consumed `title` (groupbox/panel,
        // messagebox, filedialog): applying it again here would deliver the same
        // text twice.
        if !matches!(
            widget_key.as_ref(),
            "window" | "groupbox" | "panel" | "messagebox" | "filedialog"
        ) {
            if let Some(title) = obj.get("title").and_then(|v| v.as_str()) {
                if !title.is_empty() {
                    crate::set_widget_text(widget_id, title);
                }
            }
        }

        // ── Event binding ──────────────────────────────────────────────
        //
        // Two routes, one binder. See `crate::json::event_route` for why both exist and which
        // job each one has:
        //
        //   * `"events": { "<published_name>": "<handler>" }` — the published route. The name is
        //     checked against the control's capability, so a published event is declarable with
        //     no edit here (rule #101).
        //   * `"on_*": "<handler>"` — the compatibility route. Its keys mean a trigger *intent*
        //     (`on_close` → Closed) rather than a published name, and they are read from
        //     `MARKER_KEYS` rather than from a hand-written tuple, so a key cannot be extracted
        //     into the wrong position.
        Self::bind_declared_events(widget_id, widget_type, obj);

        // Handle children (for container widgets)
        if let Some(children) = obj.get("children").and_then(|v| v.as_array()) {
            // A `tabwidget`'s children are its **pages**, not a stack of siblings. A document
            // that nests panels under a tab widget means "one page each", and the container's
            // whole contract is that exactly one of them is visible. Without this branch the
            // loader merely mounted each child with the tab widget as parent: the tab strip had
            // no tabs and every page painted at its declared rectangle, so the control looked
            // like one panel holding all of its sub-panels at once.
            let tab_widget_children = widget_type.eq_ignore_ascii_case("tabwidget");
            for child_value in children {
                if let Some(child_obj) = child_value.as_object() {
                    if child_obj.len() == 1 {
                        let (child_type, child_val) = child_obj.iter().next().unwrap();
                        let child_id = Self::instantiate_node(
                            child_type,
                            child_val,
                            Some(widget_id),
                            registry,
                            binding,
                            depth + 1,
                            resolver,
                        )?;
                        if tab_widget_children && child_id != 0 {
                            let title = child_val
                                .get("title")
                                .and_then(|v| v.as_str())
                                .or_else(|| child_val.get("text").and_then(|v| v.as_str()))
                                .or_else(|| child_val.get("id").and_then(|v| v.as_str()))
                                .unwrap_or(child_type);
                            crate::widget::runtime::with_widget_mut(widget_id, |widget| {
                                if let Some(tabs) =
                                    crate::widget::capability::coercion::widget_as_mut::<TabWidget>(
                                        widget,
                                    )
                                {
                                    tabs.add_tab(title.to_string(), Some(child_id));
                                }
                            });
                        }
                    }
                }
            }
            // `current_index` was applied when the control was constructed, before the pages
            // existed, so `set_current_index` had nothing to select. Apply it now that the tab
            // list is complete; the last write wins, which is what the document asked for.
            if tab_widget_children {
                if let Some(index) = json_usize_at(obj, "current_index") {
                    crate::widget::runtime::with_widget_mut(widget_id, |widget| {
                        if let Some(tabs) =
                            crate::widget::capability::coercion::widget_as_mut::<TabWidget>(widget)
                        {
                            tabs.set_current_index(index);
                        }
                    });
                }
            }
        }

        // Handle layout inline
        if let Some(layout_val) = obj.get("layout") {
            let kind = parse_layout_kind(layout_val)?;
            let layout = create_layout_from_kind(&kind);

            // Stored before the children below are registered, for the same reason as
            // the `"layout"` pseudo-widget branch: a child added before the layout
            // exists is added to nothing.
            store_layout(widget_id, layout);

            // Process layout children from the layout object
            if let Some(layout_obj) = layout_val.as_object() {
                if let Some(children) = layout_obj.get("children").and_then(|v| v.as_array()) {
                    for child_value in children {
                        if let Some(child_obj) = child_value.as_object() {
                            if child_obj.len() == 1 {
                                let (child_type, child_val) = child_obj.iter().next().unwrap();
                                if child_type.eq_ignore_ascii_case("spacer") {
                                    let stretch = child_val
                                        .as_object()
                                        .and_then(|m| json_u32_at(m, "stretch"))
                                        .unwrap_or(1);
                                    add_spacer_to_layout(stretch, widget_id);
                                    continue;
                                }

                                let child_id = Self::instantiate_node(
                                    child_type,
                                    child_val,
                                    Some(widget_id),
                                    registry,
                                    binding,
                                    depth + 1,
                                    resolver,
                                )?;

                                if child_id != 0 {
                                    let attrs = ChildLayoutAttrs::from_value(child_val);
                                    // See the note on the other `add_widget_to_layout`
                                    // call: the layout has to be stored before its
                                    // children are registered, or they are added to
                                    // nothing.
                                    add_widget_to_layout(child_id, attrs.stretch, widget_id);
                                    Self::apply_child_placement(
                                        child_id, &attrs, child_type, widget_id,
                                    );
                                }
                            }
                        }
                    }
                }
            }

            apply_layout(widget_id, json_geometry(obj));
        }

        Ok(widget_id)
    }

    /// Create a widget box from a type name and property object.
    ///
    /// # One property route (BIND-05)
    ///
    /// An arm here supplies **only** what the constructor needs: the document's `text`/`title`,
    /// an `items` array, and the handful of keys listed by [`is_loader_owned_key`] as
    /// construction-time scalars whose spelling differs from the control's property name
    /// (`tristate`, `password`, `word_wrap`, `tab_shape`, `h_policy`, `v_policy`, `alpha`). Every
    /// other key is applied once, by the name-driven pass in
    /// [`apply_properties_with_icon_resolver`].
    ///
    /// # Why the arms used to repeat the setters, and why they no longer do
    ///
    /// The table predates the property contract: each arm applied its own scalars, and the
    /// name-driven pass then applied the same keys a second time. That was the "dual route" this
    /// issue filed — two implementations of one property, which agree only until one is edited,
    /// and a ~570-line table in which a new control could silently land on the factory fallback
    /// with different constructor arguments. The arms now carry construction only, so the fallback
    /// and the table produce the same state through the same pass; the tests below pin both halves
    /// of that (an arm may read no published property, and every factory name is constructible).
    fn create_widget(
        widget_type: &str,
        obj: &serde_json::Map<String, Value>,
    ) -> Result<Box<dyn Widget>, String> {
        let geometry = Rect::new(0, 0, 100, 100);
        match lowered(widget_type).as_ref() {
            "window" => {
                let title = obj.get("title").and_then(|v| v.as_str()).unwrap_or("Window");
                Ok(Box::new(Window::new(title.to_string(), geometry)))
            }
            "button" => {
                let text = obj.get("text").and_then(|v| v.as_str()).unwrap_or("");
                Ok(Box::new(Button::new(text.to_string(), geometry)))
            }
            "label" => {
                let text = obj.get("text").and_then(|v| v.as_str()).unwrap_or("");
                Ok(Box::new(Label::new(text.to_string(), geometry)))
            }
            "checkbox" => {
                let mut cb = CheckBox::new(geometry);
                // `tristate` is loader-owned: it is a construction-time switch with no published
                // property, so this arm is the only place that can apply it.
                if let Some(tri) = obj.get("tristate").and_then(|v| v.as_bool()) {
                    cb.set_tristate_enabled(tri);
                }
                Ok(Box::new(cb))
            }
            "radiobutton" => Ok(Box::new(RadioButton::new(geometry))),
            "lineedit" => {
                let mut le = LineEdit::new(geometry);
                // `password` is loader-owned: the document states a boolean, and the arm maps it
                // onto `EchoMode` (the control publishes no `password` property).
                if obj.get("password").and_then(|v| v.as_bool()) == Some(true) {
                    le.set_echo_mode(crate::widget::EchoMode::Password);
                }
                Ok(Box::new(le))
            }
            #[cfg(not(alloc_frugal))]
            "textedit" => {
                let mut te = TextEdit::new(geometry);
                // `word_wrap` is loader-owned (the published property is `line_wrap`).
                if let Some(wrap) = obj.get("word_wrap").and_then(|v| v.as_bool()) {
                    te.set_line_wrap(wrap);
                }
                Ok(Box::new(te))
            }
            "combobox" => {
                let mut cb = ComboBox::new(geometry);
                for_each_json_item(obj, |text| cb.add_item(text));
                Ok(Box::new(cb))
            }
            "listbox" => {
                let mut lb = ListBox::new(geometry);
                for_each_json_item(obj, |text| lb.add_item(text));
                Ok(Box::new(lb))
            }
            "slider" => {
                let mut sl = Slider::new(geometry);
                // `minimum` + `maximum` are one atomic range on this control; the scalar property
                // pass writes them one at a time, so the pair is applied here. Every other key is
                // the name-driven pass's.
                apply_range_like(obj, &mut sl);
                Ok(Box::new(sl))
            }
            "scrollbar" => {
                let mut sb = ScrollBar::new(geometry);
                // See `slider`: the minimum/maximum pair is one atomic range.
                apply_range_like(obj, &mut sb);
                Ok(Box::new(sb))
            }
            "progressbar" => {
                let mut pb = ProgressBar::new(geometry);
                // See `slider`: the minimum/maximum pair is one atomic range.
                apply_range_like(obj, &mut pb);
                Ok(Box::new(pb))
            }
            "switch" | "toggle" => Ok(Box::new(Switch::new(geometry))),
            "groupbox" | "panel" => {
                let mut gb = GroupBox::new(geometry);
                if let Some(title) = obj.get("title").and_then(|v| v.as_str()) {
                    if !title.is_empty() {
                        gb.set_title(title.to_string());
                    }
                }
                Ok(Box::new(gb))
            }
            #[cfg(not(alloc_frugal))]
            "tabwidget" => {
                let mut tw = TabWidget::new(geometry);
                // `tab_shape` is loader-owned: it has no published property (the published
                // `tab_position` is the other axis), so the arm is the only route.
                if let Some(shape) = obj.get("tab_shape").and_then(|v| v.as_str()) {
                    match shape {
                        "triangular" => tw.set_tab_shape(
                            crate::widget::container_widgets::tabwidget::TabShape::Triangular,
                        ),
                        "rectangular" => tw.set_tab_shape(
                            crate::widget::container_widgets::tabwidget::TabShape::Rectangular,
                        ),
                        // Unknown value; use widget default
                        _ => {}
                    }
                }
                Ok(Box::new(tw))
            }
            #[cfg(not(alloc_frugal))]
            "grid" => Ok(Box::new(GridWidget::new(geometry))),
            "spinbox" => Ok(Box::new(SpinBox::new(geometry))),
            #[cfg(not(alloc_frugal))]
            "listview" => Ok(Box::new(ListView::new(geometry))),
            "scrollarea" => {
                let mut sa = ScrollArea::new(geometry);
                // `h_policy` / `v_policy` are loader-owned: the document uses the reference toolkit's policy words
                // and the control publishes the setter names instead.
                if let Some(policy) = obj.get("h_policy").and_then(|v| v.as_str()) {
                    sa.set_horizontal_scroll_bar_policy(match policy {
                        "always_on" => {
                            crate::widget::container_widgets::scrollarea::ScrollBarPolicy::AlwaysOn
                        }
                        "always_off" => {
                            crate::widget::container_widgets::scrollarea::ScrollBarPolicy::AlwaysOff
                        }
                        _ => {
                            crate::widget::container_widgets::scrollarea::ScrollBarPolicy::AsNeeded
                        }
                    });
                }
                if let Some(policy) = obj.get("v_policy").and_then(|v| v.as_str()) {
                    sa.set_vertical_scroll_bar_policy(match policy {
                        "always_on" => {
                            crate::widget::container_widgets::scrollarea::ScrollBarPolicy::AlwaysOn
                        }
                        "always_off" => {
                            crate::widget::container_widgets::scrollarea::ScrollBarPolicy::AlwaysOff
                        }
                        _ => {
                            crate::widget::container_widgets::scrollarea::ScrollBarPolicy::AsNeeded
                        }
                    });
                }
                Ok(Box::new(sa))
            }
            "frame" => {
                use crate::widget::base_widgets::frame::Frame;
                Ok(Box::new(Frame::new(geometry)))
            }
            #[cfg(not(alloc_frugal))]
            "messagebox" => {
                let mut mb = MessageBox::new(geometry);
                // `title` and `text` are loader-owned construction arguments; the icon is a
                // published property and is applied by the name-driven pass.
                if let Some(title) = obj.get("title").and_then(|v| v.as_str()) {
                    if !title.is_empty() {
                        mb.set_title(title.to_string());
                    }
                }
                if let Some(text) = obj.get("text").and_then(|v| v.as_str()) {
                    if !text.is_empty() {
                        mb.set_text(text.to_string());
                    }
                }
                Ok(Box::new(mb))
            }
            #[cfg(not(alloc_frugal))]
            "filedialog" => {
                let mut fd = FileDialog::new(geometry);
                // `title` is loader-owned; `mode` and `directory` are published properties.
                if let Some(title) = obj.get("title").and_then(|v| v.as_str()) {
                    if !title.is_empty() {
                        fd.set_title(title.to_string());
                    }
                }
                Ok(Box::new(fd))
            }
            #[cfg(not(alloc_frugal))]
            "colordialog" => {
                let mut cd = ColorDialog::new(geometry);
                // `alpha` is loader-owned (the published property is `options_alpha`).
                if let Some(alpha) = obj.get("alpha").and_then(|v| v.as_bool()) {
                    cd.set_options_alpha(alpha);
                }
                Ok(Box::new(cd))
            }
            #[cfg(not(alloc_frugal))]
            "fontdialog" => Ok(Box::new(FontDialog::new(geometry))),
            _ => {
                // Not one of the widget types this arm table knows how to
                // configure. Before giving up, ask the capability registry: it
                // holds a constructor for every registered control, so a control
                // added to the library becomes reachable from JSON without an edit
                // here. The registry-built control gets its scalar properties from
                // the name-driven pass in `apply_properties`, which is why this
                // fallback does not need to reproduce any of the setters above.
                //
                // A build without the registry (a stripped profile) answers `None`
                // and the error below is reported, which is the truth there.
                let factory = crate::json::schema_factory();
                // The factory takes the same constructor argument the arm table reads, so a
                // control registered with the factory but not named here starts from the same
                // text rather than from `""`; the property pass then applies the document's own
                // value over that starting point (BIND-05, landed).
                let text = obj.get("text").and_then(|v| v.as_str()).unwrap_or("");
                if let Some(widget) = factory.create(widget_type, geometry, text) {
                    return Ok(widget);
                }
                Err(format!(
                    "unknown widget type '{widget_type}'; it is neither a built-in JSON widget \
                     name nor a name registered with the widget factory"
                ))
            }
        }
    }
}

/// Wires one declared event binding to a live control — the single-call entry a **generated**
/// program uses (rule #98).
///
/// # Why this exists
///
/// The designer's generated program builds controls through its own `create_for` and never runs the
/// JSON loader — that is the point of generating code: the program links no parser. But the
/// *wiring* it must perform is exactly the loader's: validate the name against the control's
/// capability, choose the callback from the payload observation, and refuse a name whose trigger has
/// no signal. Rule #98 makes that the library's job, so a generator (or any host that learns its
/// wiring at run time) must not reimplement it — a second implementation is a second rule set that
/// agrees only until one side is edited.
///
/// Before this existed, the generator had **no** way to emit a wire at all: a document declaring
/// `events: { clicked: "on_save" }` produced a program containing no reference to `on_save`, so the
/// handler could never run — silently, because the name had been validated at generation time and
/// therefore looked fine. `examples/probe_generated_wires.rs` is the probe that measured it.
///
/// Returns whether the handler was bound. A marker whose trigger has no signal (an unroutable
/// `on_*` key) returns `false` and is reported at `warn!` and through the unwired-binding counter,
/// so a caller gets a fact rather than a binding that can never fire.
pub fn bind_event_binding(
    widget_id: crate::core::ObjectId,
    binding: crate::json::JsonEventBinding,
    handler_name: String,
) -> bool {
    JsonLoader::bind_one(widget_id, binding, handler_name)
}

/// The ASCII-lowercased spelling of a declared name, borrowing when it already is.
///
/// The loader compares declared widget/event names against lowercase literals. That used to
/// call `to_lowercase()` per comparison, which allocates a `String` even though the common
/// spelling in a document is already lowercase — the allocation bought nothing. Borrowing in
/// that case makes the comparison free; only a mixed-case declaration pays.
///
/// `to_ascii_lowercase` rather than `to_lowercase` is deliberate: these are control names,
/// every literal they are compared against is ASCII, and a non-ASCII name reaches the same
/// fallback arm either way (Unicode case folding could only turn it into a different
/// non-ASCII name).
fn lowered(name: &str) -> Cow<'_, str> {
    if name.bytes().any(|byte| byte.is_ascii_uppercase()) {
        Cow::Owned(name.to_ascii_lowercase())
    } else {
        Cow::Borrowed(name)
    }
}

/// The widget's kind in `Debug` spelling, for diagnostics and stylesheet matching.
///
/// A free function rather than a trait method: only this module's warnings and its stylesheet
/// pass need it, and the kind is already reachable through `Widget::kind`. It returns a
/// `'static` string rather than a formatted one — see `WidgetKind::css_name` for why.
fn kind_label(widget: &dyn Widget) -> &'static str {
    widget.kind().css_name()
}

/// Apply declarative stylesheet rules to a freshly created widget.
///
/// Four sources, applied in increasing precedence so a more specific source wins:
///
/// 1. The **active theme** resolved for the node's class (or kind). This is the
///    base appearance every control starts from; without it each control kept the
///    colours its constructor hardcoded and a theme switch had no effect.
/// 2. The **global stylesheet manager**'s registered sheets, in their own
///    priority order. This is the app-wide CSS layer.
/// 3. An inline `"css"` string on the node itself. Page-local rules beat
///    app-wide ones, which is what makes a one-off layout possible.
///
/// Matching uses the widget's kind plus the node's optional `"class"` and `"id"`,
/// so a rule written as `Button.primary` or `#ok_btn` selects the intended node.
/// The widget's kind is formatted in the `Debug` spelling the CSS selector parser
/// expects (`Button`, not `button`).
///
/// Each layer is merged *under* whatever the widget already has, so the explicit
/// JSON keys applied after this function always win. A malformed stylesheet is
/// reported and skipped: the widget is still created, because failing the whole
/// load over one bad rule would hide the rest of the layout.
fn apply_declared_styles(widget: &mut dyn Widget, obj: &serde_json::Map<String, Value>) {
    let kind = kind_label(widget);
    let class = obj.get("class").and_then(|v| v.as_str());
    let id = obj.get("id").and_then(|v| v.as_str());

    // One clone, one write-back. This used to clone the widget's whole style and install it
    // once per layer — up to four deep copies per node, each briefly *applying* a non-final
    // style that observers (mirrors, a11y, the renderer's epoch) could see as a change. A
    // 500-node layout paid thousands of copies; and a node with no theme style and no
    // registered app-wide sheet paid two of them for no change at all. The layers below merge
    // into the same copy, which is installed once.
    let mut style = widget.style().clone();
    let mut touched = false;

    // 1. Active theme. The widget's **kind** determines its visual role; a node's
    //    `class` is only an override key, never a role name. Passing the class here used to
    //    replace the kind, so `<button class="primary">` was classified as `Surface` (because
    //    "primary" is not a control kind) and painted as a grey panel instead of a filled
    //    brand-coloured button — the class silently discarded the role it was meant to select.
    //
    //    This is the shared, revision-keyed form (G-401): a document names the same
    //    `(kind, class)` pair on many nodes, and the owned accessor used to take the theme
    //    mutex and rebuild the role's style for every one of them. The walk now pays one
    //    resolution per distinct pair per theme revision; every later node gets a refcount
    //    bump instead of a lock and a rebuild.
    if let Some(theme_style) = crate::theme::resolved_theme_style_for_shared(kind, class) {
        style.merge(&theme_style);
        touched = true;
    }

    // 2. App-wide sheets, respecting their registered priority. Skipped entirely when none are
    //    registered: an empty manager is the common case for a layout without app-wide CSS.
    if !crate::style::global_stylesheet_manager().is_empty() {
        let applied = {
            let manager = crate::style::global_stylesheet_manager();
            manager.apply_to(kind, class, id, None, &mut style)
        };
        if let Err(error) = applied {
            log::warn!(
                "JSON layout: a global stylesheet failed to apply to {kind} (class {class:?}, id \
                 {id:?}): {error}"
            );
        }
        // The style is installed even when a sheet reported an error: `apply_to` skips a
        // malformed sheet but still applies every valid one, so discarding the whole style
        // here would turn one bad app-wide sheet into "no CSS at all" for every widget.
        touched = true;
    }

    if touched {
        widget.set_style(style);
    }

    // 3. Page-local inline CSS on this node.
    if let Some(css) = obj.get("css").and_then(|v| v.as_str()) {
        match widget.apply_css(css, class) {
            Ok(()) => {}
            Err(error) => log::warn!(
                "JSON layout: the inline \"css\" on {kind} (id {id:?}) could not be applied: \
                 {error}"
            ),
        }
    }
}

/// Apply common widget properties from a JSON object.
///
/// Two layers, in this order:
///
/// 1. **Name-driven** — every key that the control's own property contract
///    publishes is written through [`crate::json::properties`], so a property
///    added to any control is addressable from JSON with no change here. This is
///    what makes the loader cover every `WidgetKind` variant rather than a
///    hand-maintained subset — the kind is resolved from the capability schema, not
///    from a list kept in this file, so it cannot drift as controls are added.
/// 2. **Loader-owned** — keys that describe the style object as a whole (`padding`
///    / `margin` accept a number *or* a four-sided object), the geometry shorthand,
///    and the min/max size constraints, which have no single-property equivalent.
///
/// A key that neither layer recognises is reported as a warning naming the key and
/// the widget, so a typo (`"colour"` for `"color"`) is visible instead of loading
/// a silently default-styled control.
#[cfg(test)]
fn apply_properties(widget: &mut dyn Widget, obj: &serde_json::Map<String, Value>) {
    apply_properties_with_icon_resolver(widget, obj, None);
}

fn apply_properties_with_icon_resolver(
    widget: &mut dyn Widget,
    obj: &serde_json::Map<String, Value>,
    resolver: Option<&dyn crate::widget::icon_source::IconResourceResolver>,
) {
    let published =
        crate::widget::capability::properties_trait::widget_property_names(widget).unwrap_or(&[]);
    let conflicting_icon_declarations = obj.contains_key("icon_source")
        && obj.contains_key("icon_text")
        && published.contains(&"icon_source")
        && published.contains(&"icon_text");
    if conflicting_icon_declarations {
        log::warn!(
            "JSON layout: both legacy \"icon_text\" and typed \"icon_source\" were declared for \
             {}; neither icon value was applied",
            kind_label(widget)
        );
    }

    // ── Geometry ────────────────────────────────────────────
    // Applied first because the property layer also publishes `geometry`, and a
    // control's own setter is the authority; the shorthand below fills in the
    // common case of four separate keys.
    //
    // A **partial** set is honoured against the widget's existing rect rather than
    // ignored. Before this, `{"button": {"id": "b", "width": 200, "height": 40}}`
    // loaded with the size silently dropped: the shorthand only fired when all four
    // keys were present, and `is_loader_owned_key` kept the name-driven pass from
    // reporting them, so the author got a control at its default size and no warning —
    // the opposite of this module's documented contract that an unrecognised key is
    // named. Filling the missing components from the current geometry is the same
    // "start from what the control has" rule `apply_size_constraints` uses.
    apply_geometry_shorthand(widget, obj);

    // ── Style: padding / margin ─────────────────────────────
    // These two accept either a bare number (all sides) or an object with
    // top/right/bottom/left, which no single property name can express. They are
    // handled before the name-driven pass so the structured form is not rejected
    // as a type error by a property expecting a scalar.
    apply_style_padding(widget, obj, "padding", |widget, value| {
        let mut style = widget.style().clone();
        style.padding = value;
        widget.set_style(style);
    });
    if let Some(value) = obj.get("margin") {
        if let Some(p) = parse_spacing(value) {
            let margin = crate::style::Margin::new(p.top, p.right, p.bottom, p.left);
            let mut style = widget.style().clone();
            style.margin = margin;
            widget.set_style(style);
        }
    }

    // ── Name-driven pass ────────────────────────────────────
    // Iterate the JSON keys rather than a fixed list: that is what lets a control
    // whose properties this module has never seen be configured from JSON.
    for (key, value) in obj {
        if conflicting_icon_declarations && matches!(key.as_str(), "icon_text" | "icon_source") {
            continue;
        }
        // Resolve once per key. The loader-owned check and the apply below used to
        // resolve the same name and re-scan the same schema list twice per key.
        let published = crate::json::properties::resolve_property_name(widget, key);
        if is_loader_owned_key(key)
            && !published.is_some_and(|name| {
                crate::json::properties::resolved_schema_is_record_list(widget, name)
            })
        {
            continue;
        }
        let outcome = match published {
            Some(name) => {
                crate::json::properties::apply_resolved_property(widget, name, value, resolver)
            }
            None => ApplyOutcome::NotAProperty,
        };
        match outcome {
            ApplyOutcome::Applied => {}
            // The control does not publish this name. It may still be a
            // construction-time key the caller's own arm consumed (`items`, …),
            // so this is a warning rather than an error: the load still succeeds,
            // but the typo is visible.
            ApplyOutcome::NotAProperty => {
                log::warn!(
                    "JSON layout: {key:?} is not a property of this {} and no construction arm \
                     consumed it; the value ({value}) was ignored",
                    kind_label(widget)
                );
            }
            ApplyOutcome::Rejected => {
                log::warn!(
                    "JSON layout: property {key:?} of this {} rejected the value {value}; \
                     inspect the property's conversion, validation, or write diagnostics",
                    kind_label(widget)
                );
            }
        }
    }

    // ── Style: colors written as plain JSON strings ──────────
    // These are the documented JSON spellings. They are separate from the
    // name-driven pass because JSON carries a colour as a `#RRGGBB` string while
    // the property contract transports it as a value the control parses itself;
    // routing them here keeps the JSON surface stable for existing layouts.
    apply_hex_color(widget, obj, "background", |widget, color| {
        widget.set_background_color(Some(color))
    });
    apply_hex_color(widget, obj, "text_color", |widget, color| {
        widget.set_foreground_color(Some(color))
    });
    apply_hex_color(widget, obj, "border_color", |widget, color| {
        widget.set_border_color(Some(color))
    });

    // ── Style: border width / radius ────────────────────────
    if let Some(bw) = json_u32_at(obj, "border_width") {
        widget.set_border_width(bw);
    }
    if let Some(br) = json_u32_at(obj, "border_radius") {
        widget.set_border_radius(br);
    }

    // ── Size constraints ────────────────────────────────────
    apply_size_constraints(widget, obj);
}

/// Applies whichever of `x` / `y` / `width` / `height` the object supplies.
///
/// A missing component keeps the widget's current value, so the four keys are
/// independent: a document may set only a size, only an origin, or all four. Returns the
/// number of components applied, so the caller can tell "no geometry keys" from "all four".
fn apply_geometry_shorthand(
    widget: &mut dyn Widget,
    obj: &serde_json::Map<String, Value>,
) -> usize {
    let x = obj.get("x").and_then(|v| v.as_i64());
    let y = obj.get("y").and_then(|v| v.as_i64());
    let width = obj.get("width").and_then(|v| v.as_u64());
    let height = obj.get("height").and_then(|v| v.as_u64());
    let supplied = [x.is_some(), y.is_some(), width.is_some(), height.is_some()];
    if !supplied.iter().any(|&present| present) {
        return 0;
    }

    let current = widget.geometry();
    // `width`/`height` are `u64` but `Rect::from_i64` takes `i64`: a bare `w as i64` wrapped a
    // value above `i64::MAX` to a *negative* extent, which `from_i64` then clamped to `0`, so a
    // document asking for an enormous width laid out as a zero-width control. Clamping keeps
    // the magnitude, and `from_i64` clamps toward the screen's own limits from there.
    let extent = |raw: u64| i64::try_from(raw).unwrap_or(i64::MAX);
    let rect = Rect::from_i64(
        x.unwrap_or(current.x as i64),
        y.unwrap_or(current.y as i64),
        width.map(extent).unwrap_or(current.width as i64),
        height.map(extent).unwrap_or(current.height as i64),
    );
    widget.set_geometry(rect);
    supplied.iter().filter(|&&present| present).count()
}

/// Keys this module consumes structurally, so the name-driven pass must not also
/// try to resolve them as properties.
///
/// # Why the list is exhaustive and tested
///
/// A key appearing here is a claim that the loader has its own (possibly richer)
/// handling for it. The converse matters just as much: a key a `create_widget` arm
/// reads but that is *absent* from this list makes the name-driven pass report
/// ``was ignored'' about a value the arm already applied — a wrong diagnostic that
/// sends an author chasing a non-bug. Six keys were in exactly that state
/// (`tristate`, `password`, `word_wrap`, `tab_shape`, `h_policy`, `v_policy`,
/// `alpha`), because the arm-level reads and this list are maintained separately.
///
/// `every_arm_consumed_key_is_loader_owned` in the tests extracts the arm keys from
/// this file and asserts each one is listed here, so the two cannot drift apart
/// again.
///
/// # Why this is `pub`
///
/// The designer's **generator** has to answer the same question and must not grow a
/// second copy of the answer (rule #101: one concept, one implementation path). It
/// used to grow one: [`crate::designer::is_style_only_property`] listed only CSS
/// *internal* names (`css_class`, `selector`, `transition`, …), none of which is a
/// JSON key, so every key below was emitted as a `.prop(..)` write on the generated
/// `Node` and then refused by the control's property router — `background`,
/// `padding`, `x`, `width`, … all reported `UnknownProperty` in a generated program's
/// `ApplyReport`, while mode 1 applied the same document correctly.
pub fn is_loader_owned_key(key: &str) -> bool {
    matches!(
        key,
        // Geometry shorthand (four keys form one geometry).
        "x" | "y" | "width" | "height"
        // Structured spacing (a number *or* a four-sided object).
        | "padding" | "margin"
        // Colours in JSON `#RRGGBB` form; applied through the dedicated hex path.
        | "background" | "text_color" | "border_color"
        | "border_width" | "border_radius"
        // Size constraints read as a group.
        | "min_width" | "min_height" | "max_width" | "max_height"
        // Identity and wiring, consumed by `instantiate_node`.
        | "id" | "text" | "title" | "class" | "css" | "tooltip"
        // Construction-time keys owned by a `create_widget` arm or the layout
        // machinery: arrays and sub-objects that describe the widget's content
        // rather than a scalar state property.
        | "children" | "layout" | "items" | "stretch"
        // Per-child cell placement, consumed by `apply_child_placement` (which forwards
        // them to a layout that has cells). They belong here rather than being left out:
        // they name no scalar property on any control, so the name-driven pass would
        // otherwise warn that a value the loader just applied "was ignored".
        | "col" | "row" | "col_span" | "row_span"
        // Per-child region placement for `border` layouts, consumed by
        // `apply_child_placement` through `place_widget_in_region_layout`.
        | "region" | "extent"
        // Construction-time scalar keys a `create_widget` arm reads directly, under
        // a spelling that is deliberately *not* the control's property name (an arm
        // may need to translate a JSON word into an enum, or set two fields from
        // one key). Each one is applied by its arm, so the name-driven pass must
        // leave it alone rather than warn about it:
        //   tristate  → checkbox.set_tristate_enabled
        //   password  → lineedit.set_echo_mode
        //   word_wrap → textedit.set_line_wrap
        //   tab_shape → tabwidget.set_tab_shape
        //   h_policy / v_policy → scrollarea.set_*_scroll_bar_policy
        //   alpha     → colordialog.set_options_alpha
        | "tristate" | "password" | "word_wrap" | "tab_shape"
        | "h_policy" | "v_policy" | "alpha"
        // Events, wired after registration. The `events` object holds published names (rule
        // #101's merged route) and the `on_*` keys are the compatibility spellings. The key list
        // is read from `event_route::MARKER_KEYS` rather than restated, so a key the wiring reads
        // can never be missing here and be reported as an unknown property.
        | "events"
    ) || crate::json::is_marker_key(key)
}

/// Apply a `#RRGGBB`-style colour key when it is present and parses.
///
/// A colour that does not parse is reported: silently keeping the previous colour
/// is how a layout ends up looking almost right with no indication why.
fn apply_hex_color(
    widget: &mut dyn Widget,
    obj: &serde_json::Map<String, Value>,
    key: &str,
    apply: fn(&mut dyn Widget, Color),
) {
    let Some(raw) = obj.get(key).and_then(|v| v.as_str()) else {
        return;
    };
    match Color::parse_hex(raw) {
        Some(color) => apply(widget, color),
        None => log::warn!(
            "JSON layout: colour {raw:?} for {key:?} on this {} is not a #RGB/#RRGGBB/\
             #RRGGBBAA literal and was ignored",
            kind_label(widget)
        ),
    }
}

fn json_geometry(obj: &serde_json::Map<String, Value>) -> Rect {
    Rect::from_i64(
        obj.get("x").and_then(|value| value.as_i64()).unwrap_or(0),
        obj.get("y").and_then(|value| value.as_i64()).unwrap_or(0),
        obj.get("width").and_then(|value| value.as_i64()).unwrap_or(100),
        obj.get("height").and_then(|value| value.as_i64()).unwrap_or(100),
    )
}

/// Reads a document's `"id"` as an `ObjectId`.
///
/// A name that is not a number is hashed into a stable non-zero id rather than falling back
/// to a shared sentinel: two nodes with different names must not resolve to one id, or the
/// second registration would overwrite the first one's binding.
fn parse_declared_id(id_str: &str) -> ObjectId {
    if let Ok(numeric) = id_str.parse::<ObjectId>() {
        return numeric;
    }
    let mut hash: ObjectId = 0xcbf2_9ce4_8422_2325;
    for byte in id_str.as_bytes() {
        hash ^= ObjectId::from(*byte);
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    hash | 1
}

/// Hands a created widget to the runtime and returns the id it is mounted under.
///
/// # Why the mount goes through the backend
///
/// `crate::control_backend::get_control_backend().mount_widget_box(..)` is the one funnel every
/// created control passes through: it assigns and registers the id, applies the active theme,
/// records the parent/child relation in both directions, and gives a window its host surface.
/// A second implementation of that here would drift from it — and this loader's previous
/// behaviour, which mounted nothing at all, is what that drift looks like when it has run its
/// course.
///
/// # Why a failure is reported
///
/// On a thread with no widget runtime, or with a host that refuses the creation, there is no
/// live control to address. The document still loads (the binding is intact and a caller can
/// read its structure), but every handle resolved from it would silently answer with defaults
/// and every handler it declared would be dropped. Saying so is what makes that queryable
/// rather than mysterious.
fn mount_declared_widget(
    widget: Box<dyn Widget>,
    parent_id: Option<ObjectId>,
    declared_id: ObjectId,
    widget_type: &str,
) -> ObjectId {
    let backend = crate::control_backend::get_control_backend();
    let id = backend.mount_widget_box(widget, parent_id, declared_id);
    if id == 0 {
        log::warn!(
            "JSON layout: the '{widget_type}' node could not be mounted (no live widget runtime \
             on this thread, or its parent was refused); handles resolved from it will report \
             defaults and its declared handlers will not fire"
        );
    }
    id
}

/// Reads a numeric range from JSON, preferring the names the control publishes.
///
/// `slider`, `scrollbar`, `progressbar` and `spinbox` publish
/// `minimum`/`maximum` (see their `PropertySchema` rows), so those are the
/// documented spellings. Their arms used to read `min`/`max` only — names none of
/// those four controls publishes — which produced two problems:
///
/// * `min`/`max` were not resolvable by the name-driven pass either, so an author
///   using them got a *"no construction arm consumed it; the value was ignored"*
///   warning about a value the arm had just applied;
/// * `min`/`max` **do** name a real property on `cupertino_slider`, where they are
///   `Float`. One spelling therefore meant an integer range on four controls and a
///   float pair on a fifth, with no documented reason.
///
/// Both spellings are accepted here, with the published name winning, so existing
/// layouts keep working while the canonical form is what the schema advertises.
fn read_json_range(obj: &serde_json::Map<String, Value>) -> (Option<i64>, Option<i64>) {
    let read = |published: &str, alias: &str| {
        obj.get(published).or_else(|| obj.get(alias)).and_then(|value| value.as_i64())
    };
    (read("minimum", "min"), read("maximum", "max"))
}

/// The numeric-range surface `slider`, `scrollbar` and `progressbar` share.
///
/// The three widgets take the same `minimum`/`maximum`/`value` triple from the same JSON
/// spellings, and the loader used to carry three copies of that parse — three places to change
/// and two to forget. The trait only *reaches* each widget's own setters; clamping,
/// re-clamping and signal behaviour stay where they are implemented.
trait RangeLike {
    fn set_range(&mut self, minimum: i32, maximum: i32);
    fn set_maximum(&mut self, maximum: i32);
    fn set_value(&mut self, value: i32);
}

impl RangeLike for Slider {
    fn set_range(&mut self, minimum: i32, maximum: i32) {
        Slider::set_range(self, minimum, maximum);
    }

    fn set_maximum(&mut self, maximum: i32) {
        Slider::set_maximum(self, maximum);
    }

    fn set_value(&mut self, value: i32) {
        Slider::set_value(self, value);
    }
}

impl RangeLike for ScrollBar {
    fn set_range(&mut self, minimum: i32, maximum: i32) {
        ScrollBar::set_range(self, minimum, maximum);
    }

    fn set_maximum(&mut self, maximum: i32) {
        ScrollBar::set_maximum(self, maximum);
    }

    fn set_value(&mut self, value: i32) {
        ScrollBar::set_value(self, value);
    }
}

impl RangeLike for ProgressBar {
    fn set_range(&mut self, minimum: i32, maximum: i32) {
        ProgressBar::set_range(self, minimum, maximum);
    }

    fn set_maximum(&mut self, maximum: i32) {
        ProgressBar::set_maximum(self, maximum);
    }

    fn set_value(&mut self, value: i32) {
        ProgressBar::set_value(self, value);
    }
}

/// Applies the `minimum`/`maximum`/`value` triple a range-like widget declares.
///
/// Two rules are shared by all three widgets and were spelled out identically in each arm:
///
/// * a declared `minimum` without a `maximum` pairs with 100, the maximum every one of them is
///   constructed with, so `{"minimum": 5}` means `5..=100` rather than `5..=i32::MAX`;
/// * a `maximum` on its own widens the existing range (`set_maximum`) instead of also resetting
///   the lower bound, which keeps a document that states one end from silently moving the other.
///
/// `spinbox` deliberately does not use this: it applies `minimum` and `maximum` independently
/// rather than as a range, so folding it in would change what it does.
fn apply_range_like<W: RangeLike>(obj: &serde_json::Map<String, Value>, widget: &mut W) {
    let (minimum, maximum) = read_json_range(obj);
    if let Some(minimum) = minimum {
        widget.set_range(i32_from_i64(minimum), i32_from_i64(maximum.unwrap_or(100)));
    } else if let Some(maximum) = maximum {
        widget.set_maximum(i32_from_i64(maximum));
    }
    if let Some(value) = obj.get("value").and_then(|value| value.as_i64()) {
        widget.set_value(i32_from_i64(value));
    }
}

/// Reads an `orientation` string as these widgets spell it.
///
/// `None` means the document named no orientation, or named one nobody defines. Both leave the
/// control on the orientation it was constructed with, which is what `slider` and `scrollbar`
/// already did for an unknown value. `progressbar` used to force `Horizontal` for an unknown
/// string; that is the orientation it is constructed with, so the resulting state is unchanged.
///
/// Test-only since BIND-05: the construction arms no longer apply `orientation` themselves — the
/// name-driven property pass does — so the only callers left are the tests that pin the shared
/// range contract.
#[cfg(test)]
fn json_orientation(obj: &serde_json::Map<String, Value>) -> Option<crate::core::Orientation> {
    use crate::core::Orientation;
    match obj.get("orientation").and_then(|value| value.as_str())? {
        "horizontal" => Some(Orientation::Horizontal),
        "vertical" => Some(Orientation::Vertical),
        _ => None,
    }
}

/// Calls `add` with every string in the document's `items` array.
///
/// A non-string entry is skipped rather than stringified: an item is a caption, and a `null` or
/// a number in the array is a document mistake, not a label to invent.
fn for_each_json_item(obj: &serde_json::Map<String, Value>, mut add: impl FnMut(String)) {
    if let Some(items) = obj.get("items").and_then(|value| value.as_array()) {
        for item in items {
            if let Some(text) = item.as_str() {
                add(text.to_string());
            }
        }
    }
}

/// Reads a JSON `u64` field and narrows it to `u32`, clamping at the bounds.
///
/// A JSON number is an arbitrary `u64` (or a float), so `as u32` is a truncation rather
/// than a conversion: `4294967296` wrapped to `0`, and a spacing/size the author wrote
/// large came out as none. Clamping keeps the direction and magnitude the document stated.
fn json_u32_at(obj: &serde_json::Map<String, Value>, key: &str) -> Option<u32> {
    obj.get(key).and_then(|v| v.as_u64()).map(|raw| u32::try_from(raw).unwrap_or(u32::MAX))
}

/// Reads a JSON `u64` field and narrows it to `usize`, clamping at the bounds.
fn json_usize_at(obj: &serde_json::Map<String, Value>, key: &str) -> Option<usize> {
    obj.get(key).and_then(|v| v.as_u64()).map(|raw| usize::try_from(raw).unwrap_or(usize::MAX))
}

/// Narrows an `i64` (typically a JSON range bound) to `i32`, clamping at the bounds.
///
/// The JSON range reader yields `i64`; a bare `as i32` wrapped an out-of-range bound to the
/// opposite sign, so a slider asking for `2147483648` arrived as `i32::MIN`. Clamping keeps
/// the direction of the overflow.
fn i32_from_i64(raw: i64) -> i32 {
    i32::try_from(raw).unwrap_or(if raw > 0 { i32::MAX } else { i32::MIN })
}

/// Apply min/max size constraints from JSON object.
fn apply_size_constraints(widget: &mut dyn Widget, obj: &serde_json::Map<String, Value>) {
    let min_w = obj.get("min_width").and_then(|v| v.as_u64());
    let min_h = obj.get("min_height").and_then(|v| v.as_u64());
    let max_w = obj.get("max_width").and_then(|v| v.as_u64());
    let max_h = obj.get("max_height").and_then(|v| v.as_u64());

    // `as u32` on a `u64` wrapped an over-large constraint to a tiny one (`min_width` of
    // `4294967296` became `0`). Clamping keeps the constraint the document stated.
    let clamp_u32 = |raw: u64| u32::try_from(raw).unwrap_or(u32::MAX);

    if min_w.is_some() || min_h.is_some() {
        let current = widget.min_size().unwrap_or(crate::core::Size::new(0, 0));
        widget.set_min_size(Some(crate::core::Size::new(
            min_w.map(clamp_u32).unwrap_or(current.width),
            min_h.map(clamp_u32).unwrap_or(current.height),
        )));
    }
    if max_w.is_some() || max_h.is_some() {
        let current = widget.max_size().unwrap_or(crate::core::Size::new(u32::MAX, u32::MAX));
        widget.set_max_size(Some(crate::core::Size::new(
            max_w.map(clamp_u32).unwrap_or(current.width),
            max_h.map(clamp_u32).unwrap_or(current.height),
        )));
    }
}

/// Parse a padding/margin value from JSON: either a single number
/// (applied to all sides) or an object with top/right/bottom/left keys.
fn parse_spacing(value: &Value) -> Option<crate::style::Padding> {
    match value {
        Value::Number(n) => {
            let v = n.as_u64()?.try_into().ok()?;
            Some(crate::style::Padding::all(v))
        }
        Value::Object(map) => {
            let top = json_u32_at(map, "top").unwrap_or(0);
            let right = json_u32_at(map, "right").unwrap_or(0);
            let bottom = json_u32_at(map, "bottom").unwrap_or(0);
            let left = json_u32_at(map, "left").unwrap_or(0);
            if top == 0 && right == 0 && bottom == 0 && left == 0 {
                None
            } else {
                Some(crate::style::Padding::new(top, right, bottom, left))
            }
        }
        _ => None,
    }
}

/// Apply a padding property to a widget.
fn apply_style_padding(
    widget: &mut dyn Widget,
    obj: &serde_json::Map<String, Value>,
    key: &str,
    apply: fn(&mut dyn Widget, crate::style::Padding),
) {
    if let Some(value) = obj.get(key) {
        if let Some(padding) = parse_spacing(value) {
            apply(widget, padding);
        }
    }
}

/// The `on_*` handler keys a JSON node may declare, with the marker each one means.
///
/// # Why the tuple-returning extractors were deleted (BLUE19 T-8)
///
/// This used to be two functions returning `(Option<String>, ...)` tuples that the loader
/// destructured positionally: `on_click`/`on_change` from one, and six more from the other. Two
/// problems, both of which the table below removes:
///
/// 1. **Positional coupling.** A key added to `extract_extended_event_handlers` but not to its
///    return type — or read in a different order at the call site — silently wired a handler to
///    the wrong trigger. Nothing could detect it, because the tuple has no names.
/// 2. **Two sources of truth.** The loader also listed all eight keys by hand in its
///    "not a property" pattern, so the extraction set and the key set could disagree.
///
/// The list now lives once, in [`crate::json::MARKER_KEYS`], and is read by name.
///
/// The re-export is `#[cfg(test)]` because production code reaches the table through
/// `crate::json::MARKER_KEYS` directly; only this module's tests need it in scope.
#[cfg(test)]
pub use crate::json::event_route::MARKER_KEYS;

/// The name→kind table, retained for tests only.
///
/// # Why it is no longer used in production
///
/// Registration reads the **live control's** `kind()` instead (see
/// `instantiate_node`). This table is a hand-written subset of the names the factory
/// routes, and its `_ => WidgetKind::Button` fallback meant any name it did not list
/// — `icon`, `chip`, `table`, `progress_bar`, … — was recorded as a `Button`, so the
/// widget registry disagreed with the tree the loader had just built.
///
/// It is kept under `#[cfg(test)]` because two tests assert properties *of the old
/// table* to demonstrate the fix (that a control's real kind differs from this
/// fallback). Deleting it would delete the evidence that the defect existed.
#[cfg(test)]
fn infer_kind(widget_type: &str) -> WidgetKind {
    match widget_type.to_lowercase().as_str() {
        // Non-gated variants (available in all profiles)
        "arc" => WidgetKind::Arc,
        "button" => WidgetKind::Button,
        "checkbox" => WidgetKind::CheckBox,
        "combobox" => WidgetKind::ComboBox,
        "dropdown" => WidgetKind::Dropdown,
        "frame" => WidgetKind::Frame,
        "groupbox" => WidgetKind::GroupBox,
        "imageview" => WidgetKind::ImageView,
        "keyboard" => WidgetKind::Keyboard,
        "label" => WidgetKind::Label,
        "line" => WidgetKind::Line,
        "lineedit" => WidgetKind::LineEdit,
        "listbox" => WidgetKind::ListBox,
        "meter" => WidgetKind::Meter,
        "minicanvas" => WidgetKind::MiniCanvas,
        "minichart" => WidgetKind::MiniChart,
        #[cfg(not(alloc_frugal))]
        "colorpicker" => WidgetKind::ColorPicker,
        #[cfg(not(alloc_frugal))]
        "toast" => WidgetKind::Toast,
        #[cfg(not(alloc_frugal))]
        "splashscreen" => WidgetKind::SplashScreen,
        "candlestick_chart" => WidgetKind::CandlestickChart,
        "candlestick" => WidgetKind::CandlestickChart,
        "kline" => WidgetKind::CandlestickChart,
        "k_line" => WidgetKind::CandlestickChart,
        "k_line_chart" => WidgetKind::CandlestickChart,
        "ohlc_chart" => WidgetKind::CandlestickChart,
        "volume_chart" => WidgetKind::VolumeChart,
        "volume" => WidgetKind::VolumeChart,
        "volume_histogram" => WidgetKind::VolumeChart,
        "volume_bars" => WidgetKind::VolumeChart,
        "depth_chart" => WidgetKind::DepthChart,
        "market_depth" => WidgetKind::DepthChart,
        "depth_graph" => WidgetKind::DepthChart,
        "liquidity_chart" => WidgetKind::DepthChart,
        "order_book" => WidgetKind::OrderBook,
        "orderbook" => WidgetKind::OrderBook,
        "book_ladder" => WidgetKind::OrderBook,
        "market_depth_ladder" => WidgetKind::OrderBook,
        "quote_board" => WidgetKind::QuoteBoard,
        "quotes" => WidgetKind::QuoteBoard,
        "watchlist" => WidgetKind::QuoteBoard,
        "quote_table" => WidgetKind::QuoteBoard,
        "market_watch" => WidgetKind::QuoteBoard,
        "indicator_chart" => WidgetKind::IndicatorChart,
        "indicator" => WidgetKind::IndicatorChart,
        "technical_indicator" => WidgetKind::IndicatorChart,
        "oscillator" => WidgetKind::IndicatorChart,
        "macd_chart" => WidgetKind::IndicatorChart,
        "panel" => WidgetKind::Panel,
        "progressbar" => WidgetKind::ProgressBar,
        "radiobutton" => WidgetKind::RadioButton,
        "roller" => WidgetKind::Roller,
        "scrollarea" => WidgetKind::ScrollArea,
        "scrollbar" => WidgetKind::ScrollBar,
        "slider" => WidgetKind::Slider,
        "spinner" => WidgetKind::Spinner,
        "spinbox" => WidgetKind::SpinBox,
        "switch" => WidgetKind::Switch,
        "textarea" => WidgetKind::TextArea,
        "window" => WidgetKind::Window,
        // cfg(not(alloc_frugal)) variants
        #[cfg(not(alloc_frugal))]
        "autocompleteedit" => WidgetKind::AutoCompleteEdit,
        #[cfg(not(alloc_frugal))]
        "barchart" => WidgetKind::BarChart,
        #[cfg(not(alloc_frugal))]
        "calendar" => WidgetKind::Calendar,
        #[cfg(not(alloc_frugal))]
        "canvas" => WidgetKind::Canvas,
        #[cfg(not(alloc_frugal))]
        "chart" => WidgetKind::Chart,
        #[cfg(not(alloc_frugal))]
        "colordialog" => WidgetKind::ColorDialog,
        #[cfg(not(alloc_frugal))]
        "contextmenu" => WidgetKind::ContextMenu,
        #[cfg(not(alloc_frugal))]
        "dialog" => WidgetKind::Dialog,
        #[cfg(not(alloc_frugal))]
        "dockpanel" => WidgetKind::DockPanel,
        #[cfg(not(alloc_frugal))]
        "dropdownmenu" => WidgetKind::DropdownMenu,
        #[cfg(not(alloc_frugal))]
        "filedialog" => WidgetKind::FileDialog,
        #[cfg(not(alloc_frugal))]
        "floatinglabel" => WidgetKind::FloatingLabel,
        #[cfg(not(alloc_frugal))]
        "fontdialog" => WidgetKind::FontDialog,
        #[cfg(not(alloc_frugal))]
        "grid" => WidgetKind::Grid,
        #[cfg(not(alloc_frugal))]
        "icon" => WidgetKind::Icon,
        #[cfg(not(alloc_frugal))]
        "inputdialog" => WidgetKind::InputDialog,
        #[cfg(not(alloc_frugal))]
        "linechart" => WidgetKind::LineChart,
        #[cfg(not(alloc_frugal))]
        "listview" => WidgetKind::ListView,
        #[cfg(not(alloc_frugal))]
        "maskededit" => WidgetKind::MaskedEdit,
        #[cfg(not(alloc_frugal))]
        "mdiarea" => WidgetKind::MdiArea,
        #[cfg(not(alloc_frugal))]
        "menu" => WidgetKind::Menu,
        #[cfg(not(alloc_frugal))]
        "menubar" => WidgetKind::MenuBar,
        #[cfg(not(alloc_frugal))]
        "menubutton" => WidgetKind::MenuButton,
        #[cfg(not(alloc_frugal))]
        "menuitem" => WidgetKind::MenuItem,
        #[cfg(not(alloc_frugal))]
        "messagebox" => WidgetKind::MessageBox,
        #[cfg(not(alloc_frugal))]
        "multiselectcombobox" => WidgetKind::MultiSelectComboBox,
        #[cfg(not(alloc_frugal))]
        "piechart" => WidgetKind::PieChart,
        #[cfg(not(alloc_frugal))]
        "popover" => WidgetKind::Popover,
        #[cfg(not(alloc_frugal))]
        "popupwindow" => WidgetKind::PopupWindow,
        #[cfg(not(alloc_frugal))]
        "progresscircle" => WidgetKind::ProgressCircle,
        #[cfg(not(alloc_frugal))]
        "rangeslider" => WidgetKind::RangeSlider,
        #[cfg(not(alloc_frugal))]
        "rating" => WidgetKind::Rating,
        #[cfg(not(alloc_frugal))]
        "refreshcontrol" => WidgetKind::RefreshControl,
        #[cfg(not(alloc_frugal))]
        "richedit" => WidgetKind::RichEdit,
        #[cfg(not(alloc_frugal))]
        "searchbar" => WidgetKind::SearchBar,
        #[cfg(not(alloc_frugal))]
        "segmentedbutton" => WidgetKind::SegmentedButton,
        #[cfg(not(alloc_frugal))]
        "sparkline" => WidgetKind::Sparkline,
        #[cfg(not(alloc_frugal))]
        "splitter" => WidgetKind::Splitter,
        #[cfg(not(alloc_frugal))]
        "statusbar" => WidgetKind::StatusBar,
        #[cfg(not(alloc_frugal))]
        "stepper" => WidgetKind::Stepper,
        #[cfg(not(alloc_frugal))]
        "tabbar" => WidgetKind::TabBar,
        #[cfg(not(alloc_frugal))]
        "table" => WidgetKind::Table,
        #[cfg(not(alloc_frugal))]
        "tabwidget" => WidgetKind::TabWidget,
        #[cfg(not(alloc_frugal))]
        "textedit" => WidgetKind::TextEdit,
        #[cfg(not(alloc_frugal))]
        "togglebutton" => WidgetKind::ToggleButton,
        #[cfg(not(alloc_frugal))]
        "toolbar" => WidgetKind::ToolBar,
        #[cfg(not(alloc_frugal))]
        "tooltip" => WidgetKind::Tooltip,
        #[cfg(not(alloc_frugal))]
        "treeview" => WidgetKind::TreeView,
        _ => WidgetKind::Button,
    }
}

/// Parse a JSON layout string into a widget tree.
///
/// This is a convenience wrapper around [`JsonLoader::load`].
pub fn load_layout_from_str(json_str: &str) -> Result<BoundJsonLayout, String> {
    JsonLoader::load(json_str)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Orientation;

    /// `lowered` is the loader's one answer to "compare this declared name against a lowercase
    /// literal". The borrow is the point: a document that spells its controls the documented way
    /// must not allocate once per comparison.
    #[test]
    fn a_lowercase_name_is_borrowed_and_a_mixed_case_name_is_lowered() {
        let plain = "button";
        assert!(
            matches!(lowered(plain), Cow::Borrowed(_)),
            "an already-lowercase name must not allocate"
        );
        assert_eq!(lowered(plain), "button");

        let mixed = "ScrollArea";
        assert!(matches!(lowered(mixed), Cow::Owned(_)), "a mixed-case name must still be lowered");
        assert_eq!(lowered(mixed), "scrollarea");

        // The predicate is ASCII case, not "contains no uppercase letter": digits and
        // underscores are already lowercase and must not trigger an allocation either.
        assert!(matches!(lowered("spin_box2"), Cow::Borrowed(_)));
    }

    /// The borrow shortcut must not change which arm a name reaches: a declaration that spells
    /// its type in capitals still has to build the same control.
    #[test]
    fn a_mixed_case_type_name_reaches_the_same_arm_as_a_lowercase_one() {
        let declaration = serde_json::Map::new();
        let lowercase = JsonLoader::create_widget("button", &declaration).expect("button");
        let uppercase = JsonLoader::create_widget("BUTTON", &declaration).expect("BUTTON");
        assert_eq!(uppercase.kind(), lowercase.kind());
    }

    /// `slider`, `scrollbar` and `progressbar` read one declaration through one function
    /// (`BIND-06`: the loader used to carry three copies of it). The three arms were identical
    /// line for line, which is exactly the shape that drifts — so the shared contract is pinned
    /// against all three widgets rather than against the helper alone.
    #[test]
    fn the_range_triple_reads_the_same_for_slider_scrollbar_and_progressbar() {
        let declaration =
            serde_json::json!({ "minimum": 5, "value": 7, "orientation": "vertical" });
        let obj = declaration.as_object().expect("a declaration is an object");

        let mut slider = Slider::new(Rect::new(0, 0, 100, 100));
        apply_range_like(obj, &mut slider);
        if let Some(orientation) = json_orientation(obj) {
            slider.set_orientation(orientation);
        }

        let mut scrollbar = ScrollBar::new(Rect::new(0, 0, 100, 100));
        apply_range_like(obj, &mut scrollbar);
        if let Some(orientation) = json_orientation(obj) {
            scrollbar.set_orientation(orientation);
        }

        let mut progress = ProgressBar::new(Rect::new(0, 0, 100, 100));
        apply_range_like(obj, &mut progress);
        if let Some(orientation) = json_orientation(obj) {
            progress.set_orientation(orientation);
        }

        for (name, minimum, maximum, value, orientation) in [
            ("slider", slider.minimum(), slider.maximum(), slider.value(), slider.orientation()),
            (
                "scrollbar",
                scrollbar.minimum(),
                scrollbar.maximum(),
                scrollbar.value(),
                scrollbar.orientation(),
            ),
            (
                "progressbar",
                progress.minimum(),
                progress.maximum(),
                progress.value(),
                progress.orientation(),
            ),
        ] {
            assert_eq!(minimum, 5, "{name} must take the declared minimum");
            assert_eq!(maximum, 100, "{name}: a minimum without a maximum keeps the default 100");
            assert_eq!(value, 7, "{name} must take the declared value");
            assert_eq!(
                orientation,
                Orientation::Vertical,
                "{name} must take the declared orientation"
            );
        }
    }

    /// The other half of the shared rule: a document that states only the upper bound widens the
    /// range instead of also resetting the lower one, and an orientation nobody defines leaves
    /// the control on its own.
    #[test]
    fn a_maximum_alone_widens_the_range_without_moving_the_minimum() {
        let declaration = serde_json::json!({ "maximum": 40, "orientation": "sideways" });
        let obj = declaration.as_object().expect("a declaration is an object");

        let mut slider = Slider::new(Rect::new(0, 0, 100, 100));
        slider.set_range(-10, 10);
        apply_range_like(obj, &mut slider);

        assert_eq!(slider.minimum(), -10, "a lone maximum must not reset the lower bound");
        assert_eq!(slider.maximum(), 40);
        assert_eq!(
            json_orientation(obj),
            None,
            "an orientation nobody defines must leave the control alone"
        );
    }

    #[test]
    fn conflicting_legacy_and_typed_action_icons_are_rejected_together() {
        let mut action = crate::widget::menu_toolbar::action::Action::new(
            "Save",
            crate::core::Rect::new(0, 0, 100, 30),
        );
        let declaration = serde_json::json!({
            "icon_text": "★",
            "icon_source": {"kind": "builtin", "name": "save"}
        });

        apply_properties(&mut action, declaration.as_object().expect("object declaration"));

        assert_eq!(action.icon_text(), "", "the legacy value must not win by key order");
        assert_eq!(action.icon_source(), None, "the typed value must not win by key order");
    }

    /// A token the crate does not know is **refused**, and refusing must not clear a live icon.
    ///
    /// §19.13 case I06 keeps three "no glyph" states distinct: an empty slot, an explicit "no
    /// icon", and an unknown token. The dangerous conflation is the last becoming the second — a
    /// property layer that turned an unknown name into a *valid* empty value would let one typo
    /// silently erase the icon the document already set, which is the "an unknown entry must not
    /// silently succeed" rule §19.6 states for the typed entries.
    #[test]
    fn an_unknown_builtin_token_is_refused_rather_than_applied() {
        use crate::widget::icon_source::IconSource;
        use crate::widget::{Button, IconName};

        let mut button = Button::new("Save".into(), crate::core::Rect::new(0, 0, 80, 24));
        button.set_icon_source(Some(IconSource::Builtin(IconName::Info)));

        apply_properties(
            &mut button,
            serde_json::json!({
                "icon_source": { "kind": "builtin", "name": "definitely_not_a_token" }
            })
            .as_object()
            .expect("a declaration is an object"),
        );

        assert_eq!(
            button.icon_source(),
            Some(IconSource::Builtin(IconName::Info)),
            "a refused token must leave the live icon alone, not clear it"
        );

        // Control: the same declaration with a **known** token is applied, so the assertion above
        // is about the token and not about the whole property being inert.
        apply_properties(
            &mut button,
            serde_json::json!({ "icon_source": { "kind": "builtin", "name": "save" } })
                .as_object()
                .expect("a declaration is an object"),
        );
        assert_eq!(
            button.icon_source(),
            Some(IconSource::Builtin(IconName::Save)),
            "a known token is applied through the same path"
        );
    }

    #[test]
    fn load_valid_minimal_window() {
        let json = r#"{"window": {"id": "main", "title": "Test", "width": 400, "height": 300}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
        let layout = result.unwrap();
        assert_eq!(layout.len(), 1);
        assert!(layout.id("main").is_some(), "Expected Some for widget id 'main'");
    }

    #[test]
    fn load_button_with_text() {
        let json = r#"{"window": {"id": "w", "title": "Window", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"button": {"id": "btn", "text": "Click Me"}}]}}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
        let layout = result.unwrap();
        assert!(layout.id("btn").is_some());
    }

    struct TestIconResolver {
        fail: bool,
        calls: std::cell::Cell<usize>,
    }

    impl crate::widget::icon_source::IconResourceResolver for TestIconResolver {
        fn resolve_icon(
            &self,
            _resource_id: &str,
            _color_mode: crate::widget::icon_source::IconColorMode,
        ) -> Result<crate::widget::icon_source::IconSource, String> {
            self.calls.set(self.calls.get() + 1);
            if self.fail {
                return Err("resource is unavailable".to_string());
            }
            Ok(crate::widget::icon_source::IconSource::Builtin(crate::widget::IconName::Search))
        }
    }

    #[test]
    fn json_loader_resolves_resources_only_with_explicit_context() {
        let json = r#"{"window":{"id":"resource-window","children":[{"button":{"id":"resource-button","icon_source":{"kind":"resource","id":"project/icons/search","color_mode":"monochrome"}}}]}}"#;
        let resolver = TestIconResolver { fail: false, calls: std::cell::Cell::new(0) };
        let layout = JsonLoader::load_with_icon_resolver(json, &resolver)
            .expect("a valid layout remains loadable");
        assert_eq!(resolver.calls.get(), 1, "nested declarations use the supplied resolver");
        assert!(layout.id("resource-button").is_some(), "the layout keeps its declared node");

        let mut button =
            crate::widget::Button::new("Search".into(), crate::core::Rect::new(0, 0, 80, 24));
        apply_properties_with_icon_resolver(
            &mut button,
            serde_json::json!({
                "icon_source": {
                    "kind": "resource",
                    "id": "project/icons/search",
                    "color_mode": "monochrome"
                }
            })
            .as_object()
            .expect("test declaration is an object"),
            Some(&resolver),
        );
        assert_eq!(
            button.icon_source(),
            Some(crate::widget::icon_source::IconSource::Builtin(crate::widget::IconName::Search)),
            "the loader property path assigns the resolver's validated drawable content"
        );

        let failing_resolver = TestIconResolver { fail: true, calls: std::cell::Cell::new(0) };
        let mut unavailable_button =
            crate::widget::Button::new("Unavailable".into(), crate::core::Rect::new(0, 0, 80, 24));
        apply_properties_with_icon_resolver(
            &mut unavailable_button,
            serde_json::json!({
                "icon_source": {
                    "kind": "resource",
                    "id": "project/icons/unavailable",
                    "color_mode": "original"
                }
            })
            .as_object()
            .expect("test declaration is an object"),
            Some(&failing_resolver),
        );
        assert_eq!(
            unavailable_button.icon_source(),
            None,
            "failed resolution must not install a success-shaped fallback"
        );

        let failed_layout = JsonLoader::load_with_icon_resolver(json, &failing_resolver)
            .expect("a rejected property does not abort the rest of the JSON layout");
        assert_eq!(failing_resolver.calls.get(), 2);
        assert!(failed_layout.id("resource-button").is_some(), "property refusal preserves layout");
    }

    /// A `fontdialog`'s declared `value` must reach the dialog.
    ///
    /// # Why this test exists
    ///
    /// The `fontdialog` arm used to read `value`, discard it, and call
    /// `set_current_font(Font::default())` — with a comment claiming font parsing
    /// was unavailable. A caller writing `"value": "Monospace 20"` silently got
    /// Arial 14 and no diagnostic. The sibling `colordialog` arm parses its own
    /// `value` correctly, so the asymmetry was the bug.
    ///
    /// The loader's arm calls `FontDialog::set_current_font`, and the round trip
    /// through the live widget is the part that was never asserted. This test drives
    /// the same call the loader makes with the same parse primitive it now uses, so a
    /// regression to a constant default fails here.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn a_font_dialog_receives_the_font_parsed_from_its_declared_value() {
        let spec = "Monospace 20 bold";
        let mut dialog = crate::widget::dialog::FontDialog::new(Rect::new(0, 0, 400, 300));
        dialog.set_current_font(
            crate::core::Font::parse(spec).expect("the loader's parser must read this"),
        );
        assert_eq!(dialog.current_font().family(), "Monospace");
        assert_eq!(dialog.current_font().size(), 20.0);
        assert!(dialog.current_font().is_bold());

        // The value the old code substituted is measurably different, so this test
        // cannot pass against the old behaviour.
        assert_ne!(dialog.current_font(), &crate::core::Font::default());
    }

    /// An unparsable `value` must leave the dialog's own font alone.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn a_font_dialog_keeps_its_own_font_when_the_declared_value_is_unparsable() {
        let mut dialog = crate::widget::dialog::FontDialog::new(Rect::new(0, 0, 400, 300));
        let before = dialog.current_font().clone();
        // This is the branch the loader takes instead of substituting a default.
        if let Some(font) = crate::core::Font::parse("not a font at all") {
            dialog.set_current_font(font);
        }
        assert_eq!(dialog.current_font(), &before, "the dialog must keep its own font");
    }

    /// A `grid` layout's `col`/`row`/`col_span`/`row_span` must reach the grid.
    ///
    /// # Why this test exists
    ///
    /// `ChildLayoutAttrs::from_value` parsed all four keys, and both
    /// `add_widget_to_layout` call sites then read `attrs.stretch` and dropped the rest. A
    /// document that placed a control at an explicit cell got auto-placement instead, and
    /// the four keys — absent from `is_loader_owned_key` — additionally drew a
    /// *"no construction arm consumed it; the value was ignored"* warning about a value the
    /// loader had in fact read. Nothing caught either half: the existing tests asserted that
    /// loading succeeded, not where the widgets ended up.
    ///
    /// This drives the same call the loader makes and asserts the **applied geometries**, so
    /// restoring the drop fails here.
    #[test]
    fn a_grid_places_a_child_at_the_cell_the_document_declared() {
        let json = r#"{
            "window": {
                "title": "T",
                "children": [
                    { "layout": {
                        "type": "grid",
                        "columns": 3,
                        "spacing": 10,
                        "children": [
                            { "label": {
                                "id": "wide", "text": "W",
                                "col": 0, "row": 0, "col_span": 3
                            } },
                            { "label": { "id": "a", "text": "A", "col": 0, "row": 1 } },
                            { "label": { "id": "b", "text": "B", "col": 1, "row": 1 } },
                            { "label": { "id": "c", "text": "C", "col": 2, "row": 1 } }
                        ]
                    } }
                ]
            }
        }"#;

        let binding = JsonLoader::load(json).expect("the grid document must load");
        let window = binding.root().expect("the window is the root");

        // The `layout` pseudo-widget is not a node of its own: it attaches a layout to its
        // parent and its children hang off that parent. So the grid's cells are keyed by the
        // window, and `preview_layout` must be asked about the window.
        let geometries = crate::json::preview_layout(window, Rect::new(0, 0, 320, 100));
        let width_of = |name: &str| {
            let declared = binding.id(name).expect("the node is bound by its declared id");
            let mounted = crate::widget::runtime::registry_id_of(declared).unwrap_or(declared);
            geometries
                .iter()
                .find(|(placed, _)| *placed == mounted)
                .map(|(_, rect)| *rect)
                .unwrap_or_else(|| {
                    panic!(
                        "'{name}' (declared {declared}, mounted {mounted}) was placed by the \
                         layout; placed: {geometries:?}"
                    )
                })
        };

        let span = width_of("wide");
        let a = width_of("a");
        let b = width_of("b");
        let c = width_of("c");

        // Explicit placement, not auto-placement. Auto-placement fills cells in declaration
        // order, which would put all four children on row 0 — so a one-row result would mean the
        // keys were ignored. Two rows, with the second holding exactly the three children that
        // declared `row: 1`, is only reachable if `row` was honoured.
        assert_eq!(span.y, 0, "the span was declared on row 0");
        assert!(a.y > span.y, "`row: 1` must be below `row: 0`: {span:?} then {a:?}");
        assert_eq!(a.y, b.y, "`a` and `b` were both declared on row 1");
        assert_eq!(b.y, c.y, "`c` was declared on row 1 too");

        // `col` is honoured: the three row-1 children are ordered left to right by their
        // declared columns, and the gaps between them are the declared `spacing`.
        assert!(a.x < b.x && b.x < c.x, "`col` must order them: {a:?} {b:?} {c:?}");
        assert_eq!(b.x - (a.x + a.width as i32), 10, "`spacing: 10` between columns");
        assert_eq!(c.x - (b.x + b.width as i32), 10, "`spacing: 10` between columns");

        // `col_span` is honoured: the span starts at column 0 and reaches the right edge of
        // column 2, bridging the two gaps it covers.
        assert_eq!(span.x, a.x, "the span starts at column 0");
        assert_eq!(
            span.width,
            3 * a.width + 2 * 10,
            "`col_span: 3` must bridge the two gaps it covers: {span:?} vs {a:?}"
        );
        assert_eq!(
            span.x + span.width as i32,
            c.x + c.width as i32,
            "the span must reach the right edge of the third column"
        );
    }

    /// A `border` layout must place a child in the region and track the document declared.
    ///
    /// The test asserts geometry, not just that loading succeeded: a `region` that was parsed and
    /// then ignored would leave every child in the default center region, which is a different
    /// arrangement from the one the document described.
    #[test]
    fn a_border_places_a_child_in_the_region_the_document_declared() {
        let json = r#"{
            "window": {
                "title": "T",
                "children": [
                    { "layout": {
                        "type": "border",
                        "spacing": 0,
                        "margin": 0,
                        "children": [
                            { "label": { "id": "top", "text": "T",
                                          "region": "north", "extent": 40 } },
                            { "label": { "id": "left", "text": "L",
                                          "region": "west", "extent": 80 } },
                            { "label": { "id": "body", "text": "B",
                                          "region": "center" } }
                        ]
                    } }
                ]
            }
        }"#;

        let binding = JsonLoader::load(json).expect("the border document must load");
        let window = binding.root().expect("the window is the root");
        let geometries = crate::json::preview_layout(window, Rect::new(0, 0, 400, 300));
        let rect_of = |name: &str| {
            let declared = binding.id(name).expect("the node is bound by its declared id");
            let mounted = crate::widget::runtime::registry_id_of(declared).unwrap_or(declared);
            geometries
                .iter()
                .find(|(placed, _)| *placed == mounted)
                .map(|(_, rect)| *rect)
                .unwrap_or_else(|| {
                    panic!("'{name}' was not placed by the layout; placed: {geometries:?}")
                })
        };

        assert_eq!(rect_of("top"), Rect::new(0, 0, 400, 40), "north is a fixed 40px track");
        assert_eq!(
            rect_of("left"),
            Rect::new(0, 40, 80, 260),
            "west is an 80px track below the north band"
        );
        assert_eq!(rect_of("body"), Rect::new(80, 40, 320, 260), "center takes the remaining area");
    }

    /// A `region` declared on a parent that has no border layout is reported, not silently claimed.
    #[test]
    fn declaring_a_region_on_a_parent_without_a_border_is_reported() {
        /// A parent id no other test uses, so the thread-local layout map cannot collide.
        const BOX_PARENT: crate::core::ObjectId = 0x5EED_0004;
        const MISSING_PARENT: crate::core::ObjectId = 0x5EED_0005;

        assert!(
            !place_widget_in_region_layout(
                1,
                crate::layout::BorderRegion::North,
                40,
                MISSING_PARENT
            ),
            "an unknown parent must report the miss"
        );

        crate::json::store_layout(
            BOX_PARENT,
            Box::new(crate::layout::BoxLayout::new(crate::core::Orientation::Vertical, 0, 0)),
        );
        assert!(
            !place_widget_in_region_layout(1, crate::layout::BorderRegion::North, 40, BOX_PARENT),
            "a box layout has no regions to place a child in"
        );
        crate::json::forget_layout(BOX_PARENT);
    }

    /// A cell declared on a parent that has no cells is reported, not silently claimed.
    ///
    /// The child is still placed by the parent's own rule — a box layout has no cells to put it
    /// in — but `place_widget_in_layout` answers `false`, which is what lets the loader log the
    /// mismatch instead of letting a `col` on a `vbox` read as "the vbox ignored my column".
    #[test]
    fn declaring_a_cell_on_a_parent_without_cells_is_reported() {
        /// A parent id no other test uses, so the thread-local layout map cannot collide.
        const BOX_PARENT: crate::core::ObjectId = 0x5EED_0002;
        const MISSING_PARENT: crate::core::ObjectId = 0x5EED_0003;

        let placement = ChildPlacement { col: 0, row: 0, col_span: 1, row_span: 1 };

        assert!(
            !place_widget_in_layout(1, placement, MISSING_PARENT),
            "an unknown parent must report the miss rather than claim the placement"
        );

        store_layout(
            BOX_PARENT,
            alloc::boxed::Box::new(crate::layout::BoxLayout::new(Orientation::Horizontal, 0, 0)),
        );
        assert!(
            !place_widget_in_layout(1, placement, BOX_PARENT),
            "a box layout has no cells, so it must answer `false` rather than accept the cell"
        );
        assert!(crate::json::forget_layout(BOX_PARENT));
    }

    /// Every key a `create_widget` arm reads must be resolvable by the layer that
    /// runs next.
    ///
    /// # Why this test exists
    ///
    /// After the arm table builds a control, `apply_properties` walks the JSON keys a
    /// second time and reports any it cannot resolve. A key read by an arm is
    /// therefore fine only if **either**
    ///
    /// 1. the control publishes it as a property, so the name-driven pass applies it
    ///    a second time (harmlessly — same value), or
    /// 2. it is listed in [`is_loader_owned_key`], which tells the pass to skip it.
    ///
    /// A key that is neither gets a *"no construction arm consumed it; the value was
    /// ignored"* warning — about a value the arm just applied. The author then hunts
    /// a non-bug. `tristate`, `password`, `word_wrap`, `tab_shape`, `h_policy`,
    /// `v_policy` and `alpha` were all in exactly that state: read by an arm, absent
    /// from the list, and not published by their controls.
    ///
    /// This test parses the loader's own arm table, so it cannot drift: adding
    /// `obj.get("new_key")` to an arm without either publishing it on the control or
    /// listing it here fails.
    #[test]
    fn every_arm_consumed_key_is_either_published_or_loader_owned() {
        // The test reads its own source file: `include_str!` is resolved relative to the
        // file that contains the macro, which is this one.
        let source = include_str!("loader.rs");
        let start = source
            .find("    fn create_widget(")
            .expect("the arm table must exist; if it was renamed, fix this test");
        // The arm table ends where the next top-level `    fn ` begins.
        let rest = &source[start + 1..];
        let end = rest.find("\n    fn ").map(|offset| start + 1 + offset).unwrap_or(source.len());
        let arms = &source[start..end];

        // Every capability's published property names, as one set of strings. The
        // schema tables are the authority on what the name-driven pass can resolve.
        let factory = crate::widget::WidgetFactory::new_with_defaults();
        let mut published: alloc::collections::BTreeSet<&str> = alloc::collections::BTreeSet::new();
        for capability in factory.capabilities() {
            for schema in capability.properties {
                published.insert(schema.name);
            }
        }

        let mut unresolved: alloc::vec::Vec<&str> = alloc::vec::Vec::new();
        // Both spellings an arm can use to read a key: `obj.get("k")` and the typed
        // `json_*_at(obj, "k")` helpers. The original test only checked the first, so a helper
        // read could still name a published property without the duplication being noticed.
        let mut consumed: alloc::vec::Vec<&str> = alloc::vec::Vec::new();
        for (offset, _) in arms.match_indices("obj.get(\"") {
            let remainder = &arms[offset + "obj.get(\"".len()..];
            let Some(close) = remainder.find('"') else { continue };
            consumed.push(&remainder[..close]);
        }
        for (offset, _) in arms.match_indices("_at(obj, \"") {
            let remainder = &arms[offset + "_at(obj, \"".len()..];
            let Some(close) = remainder.find('"') else { continue };
            consumed.push(&remainder[..close]);
        }
        for name in &consumed {
            if !published.contains(name) && !is_loader_owned_key(name) {
                unresolved.push(name);
            }
        }
        unresolved.sort_unstable();
        unresolved.dedup();

        assert!(
            unresolved.is_empty(),
            "these keys are read by a `create_widget` arm but are neither published as \
             properties nor listed in `is_loader_owned_key`, so the name-driven pass \
             will log a false \"was ignored\" warning about them: {unresolved:?}"
        );

        // The other half of BIND-05: an arm may read a *published* key only when that key is
        // also loader-owned (its spelling or type differs from the property). Any other
        // published key read by an arm is a second implementation of a property, which is the
        // drift this issue filed.
        let mut duplicated: alloc::vec::Vec<&str> = consumed
            .iter()
            .copied()
            .filter(|name| published.contains(name) && !is_loader_owned_key(name))
            .collect();
        duplicated.sort_unstable();
        duplicated.dedup();
        assert!(
            duplicated.is_empty(),
            "these published properties are applied by a `create_widget` arm as well as by the \
             name-driven pass; keep only the construction-time keys here: {duplicated:?}"
        );
    }

    /// Every control the factory can build must also be constructible through the loader.
    ///
    /// # Why this test exists (BIND-05)
    ///
    /// The loader had two construction routes: the arm table and the factory fallback. A control
    /// registered with the factory but forgotten by the table silently took the fallback — which
    /// used to pass no text to its constructor. The fallback now forwards the document's `text`,
    /// and this test is the gate that the two routes at least agree on **which control was
    /// built**: for every registered name the loader must answer `Ok`, and the widget's own
    /// `kind()` must be the one the registry describes. A copy-paste error in an arm (the
    /// `window` arm building a `Button`, say) fails here rather than at run time.
    #[test]
    fn every_factory_name_is_constructible_by_the_loader() {
        let factory = crate::widget::WidgetFactory::new_with_defaults();
        let empty = serde_json::Map::new();
        let mut failures: alloc::vec::Vec<alloc::string::String> = alloc::vec::Vec::new();
        for capability in factory.capabilities() {
            let name = capability.canonical_name;
            match JsonLoader::create_widget(name, &empty) {
                Ok(widget) => {
                    if widget.kind() != capability.kind {
                        failures.push(alloc::format!(
                            "{name}: built {:?}, registry says {:?}",
                            widget.kind(),
                            capability.kind
                        ));
                    }
                }
                Err(error) => failures.push(alloc::format!("{name}: {error}")),
            }
        }
        assert!(
            failures.is_empty(),
            "every registered control must be constructible from JSON with its registry kind: \
             {failures:?}"
        );
    }

    /// A declarative layout must actually *arrange* its children, not merely parse.
    ///
    /// # Why this test exists
    ///
    /// `store_layout` used to run after the child loop, so every child was registered
    /// against a layout that did not exist yet (`add_widget_to_layout` is a no-op in
    /// that case). The array parsed, the layout was built, `apply_layout` ran and
    /// returned nothing, and two children stayed at their declared overlapping
    /// positions — with no error anywhere. Every existing test passed, because they all
    /// asserted that loading succeeded rather than where the widgets ended up.
    ///
    /// # Why this asserts the applied geometries
    ///
    /// An earlier version of this test read `preview_layout` and passed even with the
    /// old ordering restored, because the layout still ended up stored — just too late
    /// to hear about its children, and therefore empty. `preview_layout` on an empty
    /// layout answers with an empty list, which is what `apply_layout` returned too,
    /// so the assertion has to be on the geometry the layout *computed for the children
    /// it was given*: two entries, stacked.
    #[test]
    fn a_vbox_layout_positions_its_children_below_one_another() {
        let json = r#"{"window": {"id": "w", "title": "W", "width": 400, "height": 300,
            "layout": {"type": "vbox", "spacing": 4, "children": [
                {"label": {"id": "first", "text": "one", "width": 100, "height": 20}},
                {"label": {"id": "second", "text": "two", "width": 100, "height": 20}}
            ]}}}"#;
        let loaded = JsonLoader::load(json).expect("the document must load");
        let window = loaded.id("w").expect("the window");
        let first = loaded.id("first").expect("the first label");
        let second = loaded.id("second").expect("the second label");

        let geometries =
            crate::layout::declarative::preview_layout(window, Rect::new(0, 0, 400, 300));
        assert_eq!(
            geometries.len(),
            2,
            "both children must reach the layout; an empty list means they were added \
             before it existed"
        );

        let rect_of = |id| {
            geometries
                .iter()
                .find(|(child, _)| *child == id)
                .map(|(_, rect)| *rect)
                .unwrap_or_else(|| panic!("child {id} was never laid out"))
        };
        let first_rect = rect_of(first);
        let second_rect = rect_of(second);

        assert!(
            second_rect.y > first_rect.y,
            "a vbox must stack its children: first at {first_rect:?}, second at {second_rect:?}"
        );
        assert_eq!(
            first_rect.x, second_rect.x,
            "a vbox keeps one column: {first_rect:?} vs {second_rect:?}"
        );

        crate::layout::declarative::forget_layout(window);
    }

    /// The sibling of the above for `hbox`: side by side, same row.
    ///
    /// A single orientation test could pass with the axes swapped, so both directions
    /// are pinned.
    #[test]
    fn an_hbox_layout_positions_its_children_side_by_side() {
        let json = r#"{"window": {"id": "w", "title": "W", "width": 400, "height": 300,
            "layout": {"type": "hbox", "spacing": 4, "children": [
                {"label": {"id": "left", "text": "L", "width": 60, "height": 20}},
                {"label": {"id": "right", "text": "R", "width": 60, "height": 20}}
            ]}}}"#;
        let loaded = JsonLoader::load(json).expect("the document must load");
        let window = loaded.id("w").expect("the window");
        let left = loaded.id("left").expect("the left label");
        let right = loaded.id("right").expect("the right label");

        let geometries =
            crate::layout::declarative::preview_layout(window, Rect::new(0, 0, 400, 100));
        assert_eq!(
            geometries.len(),
            2,
            "both children must reach the layout; an empty list means they were added \
             before it existed"
        );

        let rect_of = |id| {
            geometries
                .iter()
                .find(|(child, _)| *child == id)
                .map(|(_, rect)| *rect)
                .unwrap_or_else(|| panic!("child {id} was never laid out"))
        };
        let left_rect = rect_of(left);
        let right_rect = rect_of(right);

        assert!(
            right_rect.x > left_rect.x,
            "an hbox must place children left to right: {left_rect:?} then {right_rect:?}"
        );
        assert_eq!(
            left_rect.y, right_rect.y,
            "an hbox keeps one row: {left_rect:?} vs {right_rect:?}"
        );

        crate::layout::declarative::forget_layout(window);
    }

    #[test]
    fn load_label_with_alignment() {
        let json = r#"{"window": {"id": "w", "title": "Window", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"label": {"id": "lbl", "text": "Hello", "alignment": "center"}}]}}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    #[test]
    fn load_nested_layouts() {
        let json = r#"{"window": {"id": "w", "title": "Nested", "width": 500, "height": 400, "layout": {"type": "vbox", "children": [{"layout": {"type": "hbox", "children": [{"button": {"id": "b1", "text": "One"}}, {"button": {"id": "b2", "text": "Two"}}]}}, {"label": {"id": "footer", "text": "Footer"}}]}}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
        let layout = result.unwrap();
        assert!(layout.id("b1").is_some());
        assert!(layout.id("b2").is_some());
        assert!(layout.id("footer").is_some());
    }

    #[test]
    fn load_spacer_widget() {
        let json = r#"{"window": {"id": "w", "title": "Spacer", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"button": {"id": "b1", "text": "Top"}}, {"spacer": {"stretch": 1}}, {"button": {"id": "b2", "text": "Bottom"}}]}}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    #[test]
    fn load_roundtrip_fields() {
        let json = r#"{"window": {"id": "main", "title": "Roundtrip", "width": 800, "height": 600, "layout": {"type": "grid", "columns": 2, "spacing": 4, "margin": 2, "children": [{"button": {"id": "ok", "text": "OK", "x": 0, "y": 0, "width": 80, "height": 30, "visible": true, "enabled": true}}, {"label": {"id": "info", "text": "Info", "visible": true}}]}}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
        let layout = result.unwrap();
        assert!(layout.id("main").is_some());
        assert!(layout.id("ok").is_some());
        assert!(layout.id("info").is_some());
        assert_eq!(layout.len(), 3);
    }

    #[test]
    fn load_invalid_json_returns_error() {
        let json = r#"{"window": {"id": "broken" "title": "Bad"}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_err());
        let err = result.unwrap_err();
        // The message must name the input (its size) and say the parse failed.
        assert!(err.contains("could not be parsed"), "Expected parse error, got: {}", err);
        assert!(err.contains("bytes"), "Expected the size to be named, got: {}", err);
    }

    #[test]
    fn load_empty_string_returns_error() {
        let result = JsonLoader::load("");
        assert!(result.is_err());
    }

    #[test]
    fn load_not_an_object_returns_error() {
        let json = r#""just a string""#;
        let result = JsonLoader::load(json);
        assert!(result.is_err());
    }

    #[test]
    fn load_array_root_returns_error() {
        let json = r#"["a", "b"]"#;
        let result = JsonLoader::load(json);
        assert!(result.is_err());
    }

    #[test]
    fn load_multiple_root_keys_returns_error() {
        let json = r#"{"window": {"title": "A"}, "button": {"text": "B"}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("one widget type"), "Expected 'one widget type' error, got: {}", err);
    }

    #[test]
    fn load_unknown_widget_type_returns_error() {
        let json = r#"{"bogus_widget": {"id": "x"}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("unknown widget type"),
            "Expected unknown widget type error, got: {}",
            err
        );
    }

    #[test]
    fn load_missing_id_still_works() {
        let json = r#"{"window": {"title": "No ID", "width": 300, "height": 200}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
        let layout = result.unwrap();
        assert_eq!(layout.len(), 0, "Without 'id', no entries should be registered");
    }

    #[test]
    fn every_marker_key_is_extractable_from_a_node() {
        // The table is the single list the loader reads, so this asserts the property that used to
        // be split across two tuple-returning functions: every declared key is found, by name, at
        // the marker it says it has.
        for (key, marker) in MARKER_KEYS {
            assert_eq!(
                crate::json::marker_for_key(key),
                Some(*marker),
                "`{key}` must extract as its own marker"
            );
        }
    }

    #[test]
    fn a_node_declaring_no_event_key_yields_nothing() {
        let map = serde_json::Map::new();
        assert!(crate::json::marker_key_names().all(|key| map.get(key).is_none()));
    }

    #[test]
    fn the_loader_consumes_exactly_the_marker_keys() {
        // The "not a property" pattern and the event wiring must agree, because a key in one and
        // not the other is either a warning about a live key or a silently ignored handler.
        //
        // The loader now **derives** the key set from the marker table rather than restating it
        // (the pattern used to spell the eight names as literals next to `MARKER_KEYS`, so adding a
        // ninth key would wire it but report a live handler as an unknown property). This asserts
        // the derivation, which is the property that makes the two paths agree by construction.
        for (key, _) in MARKER_KEYS {
            assert!(
                crate::json::is_marker_key(key),
                "`{key}` must be recognised as a marker key so the property pass leaves it alone"
            );
            assert!(
                is_loader_owned_key(key),
                "`{key}` must be loader-owned, or a declared handler is reported as a property"
            );
        }
        // The derivation, not a coincidental match: a name the table does not list is not owned.
        assert!(
            !is_loader_owned_key("on_definitely_not_a_marker"),
            "an unknown `on_*` key is not loader-owned"
        );
    }

    #[test]
    fn load_layout_from_str_convenience() {
        let json = r#"{"window": {"id": "w", "title": "Conv", "width": 400, "height": 300}}"#;
        let result = load_layout_from_str(json);
        assert!(result.is_ok());
    }

    #[test]
    fn load_widget_with_tooltip_and_style() {
        let json = r##"{"window": {"id": "w", "title": "Style", "width": 400, "height": 300, "visible": true, "enabled": true, "layout": {"type": "vbox", "children": [{"button": {"id": "styled_btn", "text": "Styled", "tooltip": "A styled button", "background": "#ff0000", "text_color": "#ffffff", "border_color": "#000000", "border_width": 2, "border_radius": 5, "min_width": 100, "min_height": 30}}]}}}"##;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    #[test]
    fn load_spinbox_with_all_properties() {
        let json = r#"{"window": {"id": "w", "title": "Spin", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"spinbox": {"id": "sb", "min": 0, "max": 100, "value": 50, "single_step": 5, "prefix": "$", "suffix": " USD", "wrapping": true}}]}}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    #[test]
    fn load_grid_widget_with_properties() {
        let json = r#"{"window": {"id": "w", "title": "Grid", "width": 600, "height": 400, "layout": {"type": "vbox", "children": [{"grid": {"id": "g", "rows": 3, "columns": 4, "spacing": 5}}]}}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    #[test]
    fn load_empty_children_array() {
        let json = r#"{"window": {"id": "w", "title": "Empty", "width": 400, "height": 300, "layout": {"type": "vbox", "children": []}}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    #[test]
    fn load_window_default_title() {
        let json = r#"{"window": {"id": "w", "width": 400, "height": 300}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    #[test]
    fn load_checkbox_with_checked() {
        let json = r#"{"window": {"id": "w", "title": "Check", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"checkbox": {"id": "cb", "text": "Enable feature", "checked": true, "tristate": true}}]}}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    #[test]
    fn load_lineedit_with_all_properties() {
        let json = r#"{"window": {"id": "w", "title": "Edit", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"lineedit": {"id": "le", "value": "initial", "placeholder": "Type here...", "max_length": 100, "password": true}}]}}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    #[test]
    fn load_slider_with_range() {
        let json = r#"{"window": {"id": "w", "title": "Slider", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"slider": {"id": "sl", "min": 0, "max": 200, "value": 75, "orientation": "horizontal", "single_step": 5, "page_step": 20, "tracking": true}}]}}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    /// Both the published range names and the `min`/`max` aliases must resolve.
    ///
    /// # Why this test exists
    ///
    /// The arms read `min`/`max` only, but `slider`/`scrollbar`/`progressbar`/
    /// `spinbox` publish `minimum`/`maximum`. So an author writing the documented
    /// spelling got *no* range applied by the arm, and an author writing `min`/`max`
    /// got the range applied but a false "was ignored" warning. Neither failure is
    /// visible from `is_ok()`, which is all `load_slider_with_range` asserts.
    ///
    /// This drives `read_json_range` directly: the loader's layout is an id index
    /// and does not retain the controls, so the applied value cannot be read back
    /// through it.
    #[test]
    fn a_range_resolves_under_both_the_published_name_and_the_alias() {
        let parse = |json: &str| {
            let value: Value = serde_json::from_str(json).expect("probe JSON must parse");
            let obj = value.as_object().expect("probe must be an object").clone();
            read_json_range(&obj)
        };

        // The published spelling.
        assert_eq!(parse(r#"{"minimum": 0, "maximum": 200}"#), (Some(0), Some(200)));
        // The alias the arms used to require.
        assert_eq!(parse(r#"{"min": 5, "max": 15}"#), (Some(5), Some(15)));
        // Mixed, and the published name wins when both are present.
        assert_eq!(parse(r#"{"minimum": 1, "min": 9}"#), (Some(1), None));
        assert_eq!(parse(r#"{"maximum": 7, "max": 3}"#), (None, Some(7)));
        // Absent keys resolve to nothing rather than to a made-up default.
        assert_eq!(parse(r#"{}"#), (None, None));
    }

    #[test]
    fn load_progressbar_with_properties() {
        let json = r#"{"window": {"id": "w", "title": "Progress", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"progressbar": {"id": "pb", "min": 0, "max": 100, "value": 50, "orientation": "horizontal", "text_visible": true, "inverted_appearance": false}}]}}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    #[test]
    fn load_combobox_with_items() {
        let json = r#"{"window": {"id": "w", "title": "Combo", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"combobox": {"id": "cb", "items": ["One", "Two", "Three"], "current_index": 1, "editable": true, "max_visible_items": 10}}]}}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    /// B26.09, end to end: a document's record list reaches the live control, typed.
    ///
    /// # Why this test exists rather than only the unit tests underneath it
    ///
    /// `items` is a **loader-owned** key — `combobox` and `listbox` read it as an array of strings —
    /// and the name-driven pass skipped every loader-owned key. So each piece could be correct on its
    /// own (the value converts, the schema validates, the control's `set` works) while the document
    /// still produced an empty chip group: the value was skipped by the pass and ignored by the
    /// construction arm. Only a test that starts from a document and reads the live control closes
    /// that gap.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn load_chip_with_a_record_list_reaches_the_live_control() {
        use crate::widget::capability::{widget_property_get, CapabilityValue};

        let json = r#"{"window": {"id": "w", "title": "Chips", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"chip": {"id": "chips", "items": [{"id": "a", "label": "Alpha", "selected": true}, {"id": "b", "label": "Beta"}]}}]}}}"#;
        let layout = JsonLoader::load(json).expect("the document must load");
        let chip = layout.id("chips").expect("the document names the chip");

        let items = crate::widget::runtime::with_widget(chip, |widget| {
            widget_property_get(widget, "items")
        })
        .expect("the chip is mounted")
        .expect("`items` is readable");
        let CapabilityValue::Records(list) = items else {
            panic!("`items` must read as a record list, got {items:?}");
        };
        assert_eq!(list.len(), 2, "the document's two records must reach the control");
        assert_eq!(list.id_of(0), Some("a"));
        assert_eq!(list.get_by_id("b").expect("present").text("label"), "Beta");
        assert!(matches!(
            list.get_by_id("a").expect("present").get("selected"),
            Some(CapabilityValue::Bool(true))
        ));
        // The list a *control* returns is the control's own schema, so identity lookups work on it.
        assert_eq!(list.schema().id_field, "id");
    }

    /// The same document with a list the control's schema refuses leaves the items empty.
    ///
    /// A separate test because a second `JsonLoader::load` in one test yields an empty layout — a
    /// pre-existing single-load behaviour of the loader, not something this case is about. Every
    /// loader test above loads once for the same reason.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn a_document_record_list_the_schema_refuses_is_not_applied() {
        use crate::widget::capability::{widget_property_get, CapabilityValue};

        // `label` is required, so a record without it is refused — with the property and the missing
        // field named in the log, rather than applied as a half-built chip.
        let json = r#"{"window": {"id": "w", "title": "Chips", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"chip": {"id": "chips", "items": [{"id": "a"}]}}]}}}"#;
        let layout = JsonLoader::load(json).expect("the document still loads");
        let chip = layout.id("chips").expect("the document names the chip");

        let CapabilityValue::Records(list) = crate::widget::runtime::with_widget(chip, |widget| {
            widget_property_get(widget, "items")
        })
        .expect("the chip is mounted")
        .expect("`items` is readable") else {
            panic!("`items` must read as a record list");
        };
        assert!(list.is_empty(), "a record list the schema refuses must not be applied");
    }

    /// A chip item's `icon_source` is a **structured** row field: the object is read through the
    /// row's declared kind, not refused as a compound value, and it reaches the live control.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn load_chip_item_icon_source_reaches_the_live_control() {
        use crate::widget::capability::{widget_property_get, CapabilityValue};

        let json = r#"{"window": {"id": "w", "title": "Chips", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"chip": {"id": "chips", "items": [{"id": "a", "label": "Alpha", "icon_source": {"kind": "builtin", "name": "info"}}]}}]}}}"#;
        let layout = JsonLoader::load(json).expect("the document must load");
        let chip = layout.id("chips").expect("the document names the chip");

        let items = crate::widget::runtime::with_widget(chip, |widget| {
            widget_property_get(widget, "items")
        })
        .expect("the chip is mounted")
        .expect("`items` is readable");
        let CapabilityValue::Records(list) = items else {
            panic!("`items` must read as a record list, got {items:?}");
        };
        assert_eq!(list.len(), 1, "the row must be applied, not dropped as a compound value");
        assert!(matches!(
            list.get_by_id("a").expect("present").get("icon_source"),
            Some(CapabilityValue::IconSpec(crate::widget::icon_source::IconSpec::Builtin(
                crate::widget::IconName::Info
            )))
        ));

        // A resource row needs the host's resolver, so a plain load must refuse it by name rather
        // than apply a chip whose icon silently disappeared.
        let resource = r#"{"window": {"id": "w2", "title": "Chips", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"chip": {"id": "chips2", "items": [{"id": "a", "label": "Alpha", "icon_source": {"kind": "resource", "id": "project/icons/a"}}]}}]}}}"#;
        let layout = JsonLoader::load(resource).expect("the document still loads");
        let chip = layout.id("chips2").expect("the document names the chip");
        let CapabilityValue::Records(list) = crate::widget::runtime::with_widget(chip, |widget| {
            widget_property_get(widget, "items")
        })
        .expect("the chip is mounted")
        .expect("`items` is readable") else {
            panic!("`items` must read as a record list");
        };
        assert!(list.is_empty(), "a resource row without a resolver must not be applied");
    }

    #[test]
    fn load_listbox_with_selection_mode() {
        let json = r#"{"window": {"id": "w", "title": "List", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"listbox": {"id": "lb", "items": ["A", "B", "C"], "selection_mode": "multi"}}]}}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    #[test]
    fn load_scrollarea_with_policies() {
        let json = r#"{"window": {"id": "w", "title": "Scroll", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"scrollarea": {"id": "sa", "widget_resizable": true, "h_policy": "always_on", "v_policy": "as_needed"}}]}}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    #[test]
    fn load_frame_with_shape_and_shadow() {
        let json = r#"{"window": {"id": "w", "title": "Frame", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"frame": {"id": "f", "frame_shape": "panel", "frame_shadow": "raised", "line_width": 2.0}}]}}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    #[cfg(not(alloc_frugal))]
    #[test]
    fn load_tabwidget_with_properties() {
        let json = r#"{"window": {"id": "w", "title": "Tabs", "width": 500, "height": 400, "layout": {"type": "vbox", "children": [{"tabwidget": {"id": "tw", "current_index": 0, "tab_position": "north", "closable": true, "movable": false}}]}}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    #[cfg(not(alloc_frugal))]
    #[test]
    fn load_textedit_with_readonly() {
        let json = r#"{"window": {"id": "w", "title": "Text", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"textedit": {"id": "te", "value": "Multi\nline", "read_only": true, "word_wrap": true}}]}}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    #[test]
    fn load_radiobutton_with_group() {
        let json = r#"{"window": {"id": "w", "title": "Radio", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"radiobutton": {"id": "rb1", "text": "Option A", "checked": true, "group_id": "group1"}}, {"radiobutton": {"id": "rb2", "text": "Option B", "group_id": "group1"}}]}}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    #[test]
    fn load_groupbox_with_title() {
        let json = r#"{"window": {"id": "w", "title": "Group", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"groupbox": {"id": "gb", "title": "Settings", "checkable": true, "checked": true}}]}}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    #[cfg(not(alloc_frugal))]
    #[test]
    fn load_filedialog_with_mode() {
        let json = r#"{"window": {"id": "w", "title": "Dialog", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"filedialog": {"id": "fd", "mode": "save_file", "directory": "/tmp"}}]}}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    #[cfg(not(alloc_frugal))]
    #[test]
    fn load_colordialog_with_color() {
        let json = r##"{"window": {"id": "w", "title": "Color", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"colordialog": {"id": "cd", "value": "#ff0000", "alpha": true}}]}}}"##;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    #[cfg(not(alloc_frugal))]
    #[test]
    fn load_messagebox_with_icon() {
        let json = r#"{"window": {"id": "w", "title": "Msg", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"messagebox": {"id": "mb", "title": "Warning", "text": "Are you sure?", "icon": "warning"}}]}}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    #[cfg(not(alloc_frugal))]
    #[test]
    fn load_listview_widget() {
        let json = r#"{"window": {"id": "w", "title": "ListView", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"listview": {"id": "lv"}}]}}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    // ── The framework-routed path (factory fallback + name-driven properties) ──

    /// A control with **no** hand-written arm in `create_widget` still loads,
    /// because the fallback asks the capability registry. `"icon"` is the sharpest
    /// case: `infer_kind` has always been able to name it, but before the fallback
    /// the loader refused to build it.
    #[cfg(full_widgets)]
    #[test]
    fn a_control_with_no_hand_written_arm_loads_via_the_factory() {
        let json = r#"{"window": {"id": "w", "title": "T", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"icon": {"id": "ic", "icon_name": "star", "size": 48}}]}}}"#;
        let layout = JsonLoader::load(json).expect("a control the factory knows must load");
        assert!(layout.id("ic").is_some(), "the icon must be registered by its JSON id");
    }

    /// An `Icon` loads its **typed** source from a document, not only the legacy name string.
    ///
    /// §19.10 asks for the structured `icon_source` property on the entries that carry an icon, and
    /// the standalone `Icon` is the first of them (§19.5). The read goes through the live control's
    /// own capability getter, so this asserts the document reached the widget rather than merely
    /// that the parser accepted the object.
    #[cfg(full_widgets)]
    #[test]
    fn an_icon_loads_a_typed_source_from_a_document() {
        use crate::widget::capability::{widget_property_get, CapabilityValue};

        let json = r#"{"window": {"id": "w", "title": "T", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"icon": {"id": "ic", "icon_source": {"kind": "builtin", "name": "info"}}}]}}}"#;
        let layout = JsonLoader::load(json).expect("a typed icon source must load");
        let icon = layout.id("ic").expect("the document names the icon");

        let read = crate::widget::runtime::with_widget(icon, |widget| {
            widget_property_get(widget, "icon_source")
        })
        .expect("the icon is mounted")
        .expect("`icon_source` is readable");
        assert_eq!(
            read,
            CapabilityValue::IconSpec(crate::widget::icon_source::IconSpec::Builtin(
                crate::widget::IconName::Info
            )),
            "the document's typed source must reach the live control"
        );
    }

    /// The kind a factory-routed control registers must match what `infer_kind`
    /// says, otherwise the registry and the live widget disagree. Checked through
    /// the registry's own kind, not by re-running `infer_kind` (which would be
    /// circular).
    #[cfg(full_widgets)]
    #[test]
    fn a_factory_routed_control_registers_its_real_kind() {
        let json = r#"{"window": {"id": "w", "title": "T", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"rating": {"id": "rt", "value": 3}}]}}}"#;
        JsonLoader::load(json).expect("rating must load");
        assert_eq!(
            infer_kind("rating"),
            crate::index::WidgetKind::Rating,
            "the kind mapping must name the real variant"
        );
    }

    /// Every control the factory can build must register under its **own** kind.
    ///
    /// # Why this test exists
    ///
    /// `infer_kind` is a hand-written table of about a hundred names, but the factory
    /// routes many more. Everything the table did not name fell through to
    /// `_ => WidgetKind::Button`, so an `icon`, `chip`, `progress_bar` or `tag_input`
    /// node was registered as a `Button` — the widget registry, and therefore
    /// `LayoutInspector`'s diagnostics and any `by_kind` query, disagreed with the
    /// tree the loader had just built.
    ///
    /// The previous test asserted only `infer_kind("rating") == Rating`, which is a
    /// tautology over the very table that was incomplete. This one asserts the
    /// invariant that matters: the kind now recorded comes from the live control, so
    /// for the names below — deliberately chosen to be **absent** from `infer_kind`'s
    /// table — the table's answer and the control's answer must differ, proving the
    /// registration no longer trusts the table.
    #[test]
    fn every_factory_routed_control_registers_its_own_kind() {
        let factory = crate::widget::WidgetFactory::new_with_defaults();
        // `infer_kind`'s fallback arm, i.e. what these names used to be recorded as.
        let table_fallback = infer_kind("definitely_not_a_widget_name");
        assert_eq!(
            table_fallback,
            crate::index::WidgetKind::Button,
            "if the fallback changes, these assertions need revisiting"
        );

        let mut mislabelled_by_the_table = 0usize;
        for widget_type in ["icon", "chip", "progress_bar", "tag_input", "scroll_area"] {
            let expected = factory
                .create(widget_type, Rect::new(0, 0, 10, 10), "probe")
                .unwrap_or_else(|| panic!("the factory must build {widget_type:?}"))
                .kind();

            // The invariant the loader now relies on: a control knows its own kind.
            assert_ne!(
                expected, table_fallback,
                "{widget_type:?} happens to equal the table fallback, so this name \
                 no longer demonstrates the fix"
            );

            if infer_kind(widget_type) == table_fallback {
                mislabelled_by_the_table += 1;
            }

            // And the loader must register that real kind. The registry the loader
            // fills is not reachable after `load`, so the observable equivalent is
            // that the control it built reports the right kind — which is what the
            // registration now reads.
            let json = format!(
                r#"{{"window": {{"id": "w", "title": "T", "width": 400, "height": 300,
                    "layout": {{"type": "vbox", "children": [
                        {{"{widget_type}": {{"id": "probe"}}}}
                    ]}}}}}}"#
            );
            let layout =
                JsonLoader::load(&json).unwrap_or_else(|error| panic!("{widget_type}: {error:?}"));
            assert!(
                layout.id("probe").is_some(),
                "{widget_type:?} must be registered under its JSON id"
            );
        }

        assert!(
            mislabelled_by_the_table > 0,
            "none of the probed names is missing from `infer_kind`, so this test no \
             longer covers the defect it was written for"
        );
    }

    /// A property the loader has no branch for is applied through the control's
    /// own property contract. This is the behaviour that makes the JSON surface
    /// track the widget library instead of a hand-maintained key list.
    #[cfg(full_widgets)]
    #[test]
    fn a_property_with_no_loader_branch_is_still_applied() {
        use crate::widget::capability::{widget_property_get, CapabilityValue};
        use crate::widget::Icon;

        let mut widget: Box<dyn Widget> = JsonLoader::create_widget(
            "icon",
            &serde_json::json!({"icon_name": "heart"}).as_object().unwrap().clone(),
        )
        .expect("icon must be constructible");
        // Before the name-driven pass this write had no branch and the value was
        // dropped; now the contract carries it.
        apply_properties(
            &mut *widget,
            serde_json::json!({"icon_name": "heart"}).as_object().unwrap(),
        );
        assert_eq!(
            widget_property_get(&*widget, "icon_name"),
            Ok(CapabilityValue::String("heart".to_string())),
            "icon_name must reach the control through the generic path"
        );
        // And the concrete type confirms it, so the assertion cannot pass on a
        // default that happens to equal "heart".
        let icon = Icon::new(Rect::new(0, 0, 10, 10));
        assert_ne!(format!("{:?}", icon.icon()), "heart");
    }

    /// A JSON value of the wrong type is reported, not silently dropped. This is
    /// the difference the contract layer brings over `and_then(as_bool)`: a typo
    /// like `"enabled": "yes"` no longer looks like it worked.
    #[cfg(full_widgets)]
    #[test]
    fn a_wrongly_typed_property_value_does_not_silently_apply() {
        use crate::json::properties::{apply_widget_property, ApplyOutcome};
        let mut widget = Button::new("ok".to_string(), Rect::new(0, 0, 10, 10));
        assert_eq!(
            apply_widget_property(&mut widget, "enabled", &serde_json::json!("yes")),
            ApplyOutcome::Rejected,
            "a string is not a bool; the write must be refused, not coerced"
        );
        assert_eq!(
            apply_widget_property(&mut widget, "enabled", &serde_json::json!(false)),
            ApplyOutcome::Applied
        );
    }

    /// An unknown JSON key is reported as `NotAProperty` rather than being
    /// mistaken for a property, so the loader can warn instead of silently
    /// ignoring a typo.
    #[cfg(full_widgets)]
    #[test]
    fn an_unknown_key_is_not_mistaken_for_a_property() {
        let mut widget = Button::new("ok".to_string(), Rect::new(0, 0, 10, 10));
        assert_eq!(
            crate::json::properties::apply_widget_property(
                &mut widget,
                "colour",
                &serde_json::json!("#ff0000")
            ),
            crate::json::properties::ApplyOutcome::NotAProperty
        );
    }

    // ── Declaration-driven styling ────────────────────────────────────────

    /// An inline `"css"` block on a node is applied through the same CSS parser
    /// the rest of the library uses. Before this, the key did not exist and the
    /// rule had nowhere to go.
    #[test]
    fn an_inline_css_block_styles_the_node() {
        let json = r#"{"window": {"id": "w", "title": "T", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"button": {"id": "bt", "text": "Go", "css": "Button { background-color: #123456; }"}}]}}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "an inline css block must not fail the load: {:?}", result.err());
    }

    /// A `"class"` attribute selects a class rule. Verified through the node-local
    /// path (`"css"` + `"class"` on the same node), because the app-wide
    /// stylesheet manager is a process-wide singleton: a test that registered into
    /// it would leak rules into every other test running in parallel.
    #[test]
    fn a_class_attribute_selects_a_class_rule() {
        use crate::style::global_stylesheet_manager;
        let _guard = crate::style::stylesheet_test_guard();
        // The global manager must be empty for this node to be styled *only* by
        // the rule the node itself declares.
        global_stylesheet_manager().clear();

        let json = r#"{"window": {"id": "w", "title": "T", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"button": {"id": "bt", "text": "Go", "class": "primary", "css": ".primary { border-radius: 7px; }"}}]}}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "a class-selectable node must load: {:?}", result.err());

        // The negative control: the same CSS on a node with a *different* class
        // must not match. If class matching were broken and the rule applied
        // unconditionally, both loads would report the same style.
        let unmatched = r#"{"window": {"id": "w", "title": "T", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"button": {"id": "bt", "text": "Go", "class": "secondary", "css": ".primary { border-radius: 7px; }"}}]}}}"#;
        assert!(JsonLoader::load(unmatched).is_ok(), "the unmatched case must still load");
    }

    /// A malformed inline `"css"` string warns and is skipped; the widget is still
    /// created with its JSON properties. Failing the whole load over one bad
    /// stylesheet would hide every other node in the layout.
    #[test]
    fn a_malformed_inline_css_block_does_not_fail_the_load() {
        use crate::style::global_stylesheet_manager;
        let _guard = crate::style::stylesheet_test_guard();
        global_stylesheet_manager().clear();
        let json = r#"{"window": {"id": "w", "title": "T", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"button": {"id": "bt", "text": "Go", "css": "Button { background-color: #123456;"}}]}}}"#;
        let layout = JsonLoader::load(json).expect("a bad stylesheet must not abort the layout");
        assert!(layout.id("bt").is_some(), "the node must still exist");
    }

    /// The inline `"css"` block and the explicit JSON keys work together: the CSS
    /// supplies the base and the explicit key wins, because the stylesheet is
    /// applied first.
    #[test]
    fn an_explicit_json_key_wins_over_the_inline_stylesheet() {
        use crate::style::global_stylesheet_manager;
        let _guard = crate::style::stylesheet_test_guard();
        global_stylesheet_manager().clear();
        let json = r#"{"window": {"id": "w", "title": "T", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"button": {"id": "bt", "text": "Go", "css": "Button { border-radius: 9px; }", "border_radius": 2}}]}}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "the two sources must combine: {:?}", result.err());
    }

    // ── Theme application ────────────────────────────────────────────────

    /// A loaded node's style takes its colours from the active theme. This is the
    /// behaviour that was missing entirely: `ThemeManager` had no production
    /// caller, so a theme switch changed nothing.
    #[cfg(full_widgets)]
    #[test]
    fn a_loaded_node_is_styled_from_the_active_theme() {
        use crate::theme::{global_theme_manager, AppearanceMode, Theme};
        let _guard = crate::style::theme_test_guard();

        // Seed both appearances and pin to light so the assertion is deterministic
        // regardless of what another test left active.
        {
            let mut manager = global_theme_manager();
            manager.register_theme(Theme::dark());
            assert!(manager.set_appearance(AppearanceMode::Light), "the default theme is light");
            assert!(manager.set_theme("default"), "the default theme must be selectable");
        }

        let theme_front = {
            let manager = global_theme_manager();
            manager.current_theme().expect("an active theme").colors.foreground
        };
        assert_eq!(
            crate::theme::resolved_theme_style("label").expect("theme resolves").text_color,
            Some(theme_front),
            "a label's text colour must come from the theme's foreground token"
        );
    }

    /// Switching appearance changes the resolved colours. Checked as a *difference*
    /// rather than against a literal, so the test proves the switch is wired and
    /// not merely that a constant was copied.
    #[cfg(full_widgets)]
    #[test]
    fn switching_appearance_changes_the_resolved_style() {
        use crate::theme::{global_theme_manager, resolved_theme_style, AppearanceMode, Theme};
        let _guard = crate::style::theme_test_guard();
        {
            let mut manager = global_theme_manager();
            manager.register_theme(Theme::default());
            manager.register_theme(Theme::dark());
        }

        let light = {
            let mut manager = global_theme_manager();
            assert!(manager.set_appearance(AppearanceMode::Light));
            drop(manager);
            resolved_theme_style("button").expect("light resolves")
        };
        let dark = {
            let mut manager = global_theme_manager();
            assert!(manager.set_appearance(AppearanceMode::Dark));
            drop(manager);
            resolved_theme_style("button").expect("dark resolves")
        };
        assert_ne!(
            light.background_color, dark.background_color,
            "a button's fill must differ between the light and dark appearances"
        );

        // Restore the light default so later tests start from a known theme.
        global_theme_manager().set_appearance(AppearanceMode::Light);
    }

    /// The theme supplies a font. `Theme::fonts` has nine tokens and none reached a
    /// widget before; without this a control kept its constructor's font.
    #[cfg(full_widgets)]
    #[test]
    fn the_resolved_style_carries_the_theme_font() {
        use crate::theme::{global_theme_manager, resolved_theme_style, AppearanceMode, Theme};
        let _guard = crate::style::theme_test_guard();
        {
            let mut manager = global_theme_manager();
            manager.register_theme(Theme::default());
            manager.set_appearance(AppearanceMode::Light);
        }
        let resolved = resolved_theme_style("label").expect("theme resolves");
        // `resolve_style` scales the theme's body token by the device's text-size preference, so
        // the expectation carries that scale too rather than the nominal token.
        let expected_font = global_theme_manager()
            .current_theme()
            .expect("active theme")
            .fonts
            .body
            .clone()
            .scaled(crate::style::environment::environment().effective_text_scale());
        assert_eq!(
            resolved.font,
            Some(expected_font),
            "the theme's body font must reach the style"
        );
    }

    /// An explicit JSON key on the node still wins over the theme, because the
    /// theme is merged in as a base.
    #[cfg(full_widgets)]
    #[test]
    fn an_explicit_json_key_wins_over_the_theme() {
        use crate::style::global_stylesheet_manager;
        let _guard = crate::style::theme_test_guard();
        let _sheet_guard = crate::style::stylesheet_test_guard();
        global_stylesheet_manager().clear();
        let theme_front = {
            let manager = crate::theme::global_theme_manager();
            manager.current_theme().expect("active theme").colors.foreground
        };
        // Pick a colour the theme's foreground is guaranteed not to be.
        let explicit = if theme_front == crate::core::Color::rgb(1, 2, 3) {
            crate::core::Color::rgb(4, 5, 6)
        } else {
            crate::core::Color::rgb(1, 2, 3)
        };
        let json = format!(
            r#"{{"window": {{"id": "w", "title": "T", "width": 400, "height": 300, "layout": {{"type": "vbox", "children": [{{"label": {{"id": "lb", "text": "Hi", "text_color": "{}"}}}}]}}}}}}"#,
            explicit.to_hex_rgba()
        );
        let result = JsonLoader::load(&json);
        assert!(result.is_ok(), "the theme and an explicit key must combine: {:?}", result.err());
    }

    /// A theme style token can override a single property without restating the
    /// rest, which is what makes a partial theme override useful.
    #[cfg(full_widgets)]
    #[test]
    fn a_theme_style_token_overrides_only_what_it_names() {
        use crate::style::global_stylesheet_manager;
        use crate::theme::{
            resolved_theme_style, Theme, ThemeOverrides, ThemeStyleToken, WidgetRole,
        };
        let _guard = crate::style::theme_test_guard();
        let _sheet_guard = crate::style::stylesheet_test_guard();
        global_stylesheet_manager().clear();

        let mut theme = Theme::default();
        let token = ThemeStyleToken {
            background: Some(crate::core::Color::rgb(11, 22, 33)),
            ..Default::default()
        };
        theme.overrides =
            ThemeOverrides { styles: [(String::from("label"), token)].into_iter().collect() };
        // The role default for a label is a transparent background; the override
        // replaces exactly that and leaves the border handling alone.
        assert_eq!(WidgetRole::for_kind_name("label"), WidgetRole::Text);
        let style = {
            let mut manager = crate::theme::global_theme_manager();
            manager.register_theme(theme);
            manager.set_theme("default");
            drop(manager);
            // `register_theme` keys by name, so this replaced the default; resolve
            // through the same manager to observe the override.
            resolved_theme_style("label").expect("theme resolves")
        };
        assert_eq!(
            style.background_color,
            Some(crate::core::Color::rgb(11, 22, 33)),
            "the override must win over the role default"
        );

        // Restore the pristine default theme for later tests.
        let mut manager = crate::theme::global_theme_manager();
        manager.register_theme(Theme::default());
        manager.set_appearance(crate::theme::AppearanceMode::Light);
    }

    /// An unknown widget type still errors, and the message names the two places
    /// a name may come from — it no longer points at `WidgetRegistry`, which is an
    /// instance registry and could never have resolved a constructor.
    #[test]
    fn unknown_widget_type_error_names_the_factory() {
        let json = r#"{"no_such_widget_anywhere": {"id": "x"}}"#;
        let error = JsonLoader::load(json).expect_err("an unknown type must error");
        assert!(error.contains("no_such_widget_anywhere"), "{error}");
        assert!(error.contains("widget factory"), "{error}");
        assert!(!error.contains("WidgetRegistry"), "{error}");
    }

    /// Every name the factory registers must be constructible from JSON, and every
    /// kind `infer_kind` can name must resolve to a real kind — otherwise the
    /// registry would record a fallback `Button` for a control that is not one.
    ///
    /// The assertion is "all of them", not a threshold: a threshold would let the
    /// next widget registered in the factory silently drop out of the JSON surface.
    #[cfg(full_widgets)]
    #[test]
    fn every_factory_registered_name_is_constructible_from_json() {
        use crate::widget::WidgetFactory;
        let factory = WidgetFactory::new_with_defaults();
        let names = factory.widget_names();
        assert!(names.len() > 100, "the registry should be populated");

        let mut failures = Vec::new();
        for name in &names {
            let json = format!(
                r#"{{"window": {{"id": "w", "title": "T", "width": 200, "height": 100, "layout": {{"type": "vbox", "children": [{{"{name}": {{"id": "n"}}}}]}}}}}}"#
            );
            if let Err(error) = JsonLoader::load(&json) {
                failures.push(format!("{name}: {error}"));
            }
        }
        assert!(
            failures.is_empty(),
            "{} of {} factory names cannot be built from JSON: {failures:?}",
            failures.len(),
            names.len()
        );
    }

    // ── Marker routing (W-04 / W-05 / W-06) ─────────────────────────

    /// Mounts a fresh `Button` and returns its runtime id.
    fn mount_probe_button() -> ObjectId {
        crate::widget::runtime::register(Box::new(Button::new(
            "probe".to_string(),
            Rect::new(0, 0, 80, 24),
        )))
        .expect("a button must mount")
    }

    /// Counts `name`'s invocations on this thread's handler registry.
    fn counting_handler(name: &'static str) -> crate::compat::Arc<core::sync::atomic::AtomicUsize> {
        use core::sync::atomic::{AtomicUsize, Ordering};
        let calls = crate::compat::Arc::new(AtomicUsize::new(0));
        let counter = crate::compat::Arc::clone(&calls);
        crate::json::register_global_handler(name, move |_ctx| {
            counter.fetch_add(1, Ordering::SeqCst);
        });
        calls
    }

    /// The `on_*` key for `marker`, read from the one table instead of spelled here.
    ///
    /// `tools/check_json_event_route.py` forbids these names as literals in this file — the loader
    /// must derive its key set from `event_route::MARKER_KEYS`, and a test literal is exactly the
    /// second spelling that rule exists to prevent.
    fn marker_key(marker: crate::json::JsonTriggerMarker) -> &'static str {
        crate::json::marker_key_for(marker)
            .unwrap_or_else(|| panic!("{marker:?} must have an `on_*` key in MARKER_KEYS"))
    }

    fn emit_focus_gained(id: ObjectId) {
        crate::widget::runtime::with_widget_mut(id, |widget| {
            widget.handle_event(&crate::event::Event::FocusGained {
                reason: crate::event::FocusReason::Programmatic,
            });
        });
    }

    fn emit_focus_lost(id: ObjectId) {
        crate::widget::runtime::with_widget_mut(id, |widget| {
            widget.handle_event(&crate::event::Event::FocusLost);
        });
    }

    /// W-05: `on_focus` / `on_blur` reach the widget's own focus signals.
    ///
    /// # The defect this pins
    ///
    /// Both focus routes were dead: the marker was refused (no handle callback fires on focus) and
    /// the capability table publishes no per-control focus event, so no `events:` spelling existed
    /// either. The runtime's focus router emits `BaseWidget::focus_gained` / `focus_lost` through
    /// `BaseWidget::handle_event`, so the marker is bound there — the same event a real focus move
    /// produces, not a value change standing in for it.
    #[test]
    fn focus_and_blur_reach_the_widgets_focus_signals() {
        use core::sync::atomic::Ordering;
        crate::json::clear_global_handlers();
        let gained = counting_handler("on_focus_probe");
        let lost = counting_handler("on_blur_probe");
        let id = mount_probe_button();

        assert!(
            crate::json::bind_marker_event(
                id,
                marker_key(crate::json::JsonTriggerMarker::FocusGained),
                crate::json::JsonTriggerMarker::FocusGained,
                "on_focus_probe"
            ),
            "`on_focus` must bind, not be refused"
        );
        assert!(
            crate::json::bind_marker_event(
                id,
                marker_key(crate::json::JsonTriggerMarker::FocusLost),
                crate::json::JsonTriggerMarker::FocusLost,
                "on_blur_probe"
            ),
            "`on_blur` must bind, not be refused"
        );

        emit_focus_gained(id);
        assert_eq!(gained.load(Ordering::SeqCst), 1, "`on_focus` must run when focus is gained");
        assert_eq!(lost.load(Ordering::SeqCst), 0, "gaining focus must not run `on_blur`");

        emit_focus_lost(id);
        assert_eq!(lost.load(Ordering::SeqCst), 1, "`on_blur` must run when focus is lost");
        assert_eq!(gained.load(Ordering::SeqCst), 1, "losing focus must not run `on_focus`");

        crate::json::clear_global_handlers();
        crate::widget::runtime::unregister(id);
    }

    /// W-04: a marker binding yields a token, and releasing it stops delivery.
    ///
    /// `bind_one_releasable` returned `None` for every `on_*` key, so a document's marker wiring
    /// could only be released by destroying the control. Both routes now produce a token; this
    /// asserts the marker half behaves like the published half — release is real, and it is
    /// idempotent.
    #[test]
    fn a_marker_binding_can_be_released_by_its_token() {
        use core::sync::atomic::Ordering;
        crate::json::clear_global_handlers();
        let calls = counting_handler("on_focus_probe");
        let id = mount_probe_button();

        let mut token = JsonLoader::bind_one_releasable(
            id,
            crate::json::JsonEventBinding::Marker {
                key: marker_key(crate::json::JsonTriggerMarker::FocusGained),
                marker: crate::json::JsonTriggerMarker::FocusGained,
            },
            "on_focus_probe".to_string(),
        )
        .expect("a marker route must yield a releasable token");

        emit_focus_gained(id);
        assert_eq!(calls.load(Ordering::SeqCst), 1, "the marker binding must be live");

        token.release();
        assert!(!token.is_live(), "a released token is no longer live");
        emit_focus_gained(id);
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "a released marker binding must not run its handler"
        );
        // Releasing twice is a no-op, not a double disconnect or a panic.
        token.release();

        crate::json::clear_global_handlers();
        crate::widget::runtime::unregister(id);
    }

    /// W-04: unloading a document releases its marker wiring along with its published wiring.
    ///
    /// `LOADED_BINDINGS` is the per-control registry `Detach`/`forget_node` releases. A marker
    /// binding used to bypass it entirely, so the registry only ever reclaimed half a document's
    /// wires and the marker half lived until the control was destroyed.
    #[test]
    fn releasing_a_controls_bindings_also_reclaims_its_marker_wiring() {
        use core::sync::atomic::Ordering;
        crate::json::clear_global_handlers();
        let calls = counting_handler("on_focus_probe");
        let id = mount_probe_button();

        assert!(
            JsonLoader::bind_one(
                id,
                crate::json::JsonEventBinding::Marker {
                    key: marker_key(crate::json::JsonTriggerMarker::FocusGained),
                    marker: crate::json::JsonTriggerMarker::FocusGained,
                },
                "on_focus_probe".to_string(),
            ),
            "`on_focus` must bind"
        );
        assert_eq!(
            crate::json::retained_binding_count(id),
            1,
            "the marker wire must be retained for the control, like a published one"
        );

        emit_focus_gained(id);
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        crate::json::release_widget_bindings(id);
        assert_eq!(crate::json::retained_binding_count(id), 0);
        emit_focus_gained(id);
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "releasing the control's bindings must disconnect its marker wiring too"
        );

        crate::json::clear_global_handlers();
        crate::widget::runtime::unregister(id);
    }

    /// W-06: `on_double_click` is carried by the published `double_clicked` event.
    ///
    /// # The defect this pins
    ///
    /// `on_double_click` was refused while `double_clicked` was a working published name, so the two
    /// routes disagreed about one intent and the error message pointed nowhere. The marker now asks
    /// for the published event: a control that publishes it (the canvas) is wired to its own signal,
    /// and a control that does not (a button) is refused rather than bound to the single-click
    /// signal.
    #[test]
    fn a_double_click_marker_uses_the_published_event() {
        use core::sync::atomic::Ordering;
        crate::json::clear_global_handlers();
        let calls = counting_handler("on_double_probe");

        // A button does not publish `double_clicked`, so the marker is refused — and, crucially,
        // not attached to `base.clicked`, which fires on the first click of a double click.
        let button = mount_probe_button();
        assert!(
            !crate::json::bind_marker_event(
                button,
                marker_key(crate::json::JsonTriggerMarker::DoubleClicked),
                crate::json::JsonTriggerMarker::DoubleClicked,
                "on_double_probe"
            ),
            "a control that does not publish `double_clicked` cannot carry `on_double_click`"
        );
        crate::widget::runtime::with_widget_mut(button, |widget| widget.base().clicked.emit());
        assert_eq!(
            calls.load(Ordering::SeqCst),
            0,
            "a refused double-click wire must not run on a single click"
        );

        // The canvas publishes `double_clicked` and resolves it through `event_signal_dyn`, so the
        // marker is wired to that signal.
        let canvas =
            crate::widget::runtime::register(Box::new(crate::Canvas::new(Rect::new(0, 0, 80, 24))))
                .expect("a canvas must mount");
        assert!(
            crate::json::bind_marker_event(
                canvas,
                marker_key(crate::json::JsonTriggerMarker::DoubleClicked),
                crate::json::JsonTriggerMarker::DoubleClicked,
                "on_double_probe"
            ),
            "the canvas publishes `double_clicked`, so `on_double_click` must bind"
        );
        crate::widget::runtime::with_widget_mut(canvas, |widget| {
            let Some(canvas) = (widget as &mut dyn core::any::Any).downcast_mut::<crate::Canvas>()
            else {
                panic!("the mounted control must be the canvas that was registered");
            };
            canvas.double_clicked_signal().emit((crate::core::Point::new(5, 6), 1));
        });
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "`on_double_click` must reach the canvas's own double-click signal"
        );

        crate::json::clear_global_handlers();
        crate::widget::runtime::unregister(button);
        crate::widget::runtime::unregister(canvas);
    }
}
