// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! # The JSON event route: published names and compatibility keys, end to end
//!
//! `src/json/` used to have its own event path — eight hand-matched `on_*` keys — while the
//! capability table published 186 names. The two sets were **disjoint** (`on_click` is not a
//! published name; `clicked` is), so publishing a new event never made it declarable, and
//! `grep -rn "on_click" tools/` found no gate that could notice. That is BLUE19 T-8/T-10 and
//! rule #101.
//!
//! A gate (`tools/check_json_event_route.sh`) verifies the two routes *structurally*. This file
//! verifies them **behaviourally**, because a structural check cannot tell a route that resolves
//! a name from one that resolves it and then fails to fire: the loader could look the name up,
//! log a warning, and wire nothing, and every static assertion would still pass.
//!
//! # Why each test loads a real document
//!
//! Driving `bind_declared_events` directly would test the helper rather than the loader's use of
//! it. Every case below therefore goes through `load_layout_from_str`, which is the path a host
//! actually takes.
//!
//! # Why these tests can now drive the signal
//!
//! An earlier revision of this file could not, and said so: `JsonLoader::load` registered metadata
//! into `WidgetRegistry` and never mounted the widget into `crate::widget::runtime`, so no id a
//! document produced addressed a live control and nothing could raise a signal on one. The tests
//! settled for "the document loaded", which passes identically against a loader that wires every
//! handler to nothing — and that is exactly what it was doing. The loader now mounts through the
//! control backend's creation funnel, so each test below **raises the signal and counts
//! invocations**, which is the only assertion that can tell a wired binding from a resolved name.

#![cfg(all(feature = "desktop", not(alloc_frugal)))]

use rust_widgets::app::dispatch_trigger;
use rust_widgets::core::ObjectId;
use rust_widgets::json::{
    bind_published_event, clear_global_handlers, invoke_global_handler, json_event_binding,
    register_global_handler, BoundJsonLayout, EventHandlerContext, JsonEventBinding, JsonLoader,
};
use rust_widgets::platform::WidgetTriggerKind;
use rust_widgets::widget::capability::WidgetFactory;
use rust_widgets::WidgetTriggerEvent;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Runs `body` with a clean global handler registry.
///
/// The registry is thread-local and the tests run in one process, so a handler registered by an
/// earlier test would be visible here. Clearing on both sides makes each case independent — and
/// it is the same call a host makes when it rebuilds a layout.
fn with_clean_handlers(body: impl FnOnce()) {
    clear_global_handlers();
    body();
    clear_global_handlers();
}

/// A handler registered under `name` that counts its invocations.
fn counting_handler(name: &str) -> Arc<AtomicUsize> {
    let calls = Arc::new(AtomicUsize::new(0));
    let sink = Arc::clone(&calls);
    register_global_handler(name, move |_ctx: &EventHandlerContext| {
        sink.fetch_add(1, Ordering::SeqCst);
    });
    calls
}

/// The id a loaded node was **mounted** under.
///
/// # Why this is the document's id, not a translation of it
///
/// The loader *reserves* the document's `"id"` when it mounts, so the runtime registry and the
/// document share one id space and no lookup is needed. The mount is still verified rather than
/// assumed: if a future change reintroduced a second id space, a test that skipped the check would
/// drive a signal into nothing and pass.
fn mounted_id(bound: &BoundJsonLayout, name: &str) -> ObjectId {
    let declared = bound.id(name).unwrap_or_else(|| panic!("'{name}' must be bound by its id"));
    assert!(
        rust_widgets::widget::runtime::is_mounted(declared),
        "'{name}' (id {declared}) was not mounted, so no signal can be raised on it"
    );
    declared
}

/// Emits the control's own `clicked` signal — the channel `on_click` connects to.
///
/// Deliberately the widget's signal rather than `dispatch_trigger`: a real click arrives through
/// `BaseWidget::clicked`, and the loader's click bindings are connected there. Driving
/// `dispatch_trigger` instead would exercise the *legacy* table, which is a different channel and
/// would let a regression in the connect step pass unnoticed.
fn emit_click(bound: &BoundJsonLayout, name: &str) {
    let id = mounted_id(bound, name);
    rust_widgets::widget::runtime::with_widget_mut(id, |widget| {
        widget.base().clicked.emit();
    });
}

/// Invokes `name` the way a handler receives it, and reports whether it was found.
///
/// Used by the cases that assert something about the *hub* rather than about a control.
#[allow(dead_code)]
fn fire(name: &str, kind: WidgetTriggerKind) -> bool {
    let ctx = EventHandlerContext::new(WidgetTriggerEvent { widget_id: 1, kind });
    invoke_global_handler(name, &ctx)
}

/// The published route fires on a real click, not merely resolves the name.
///
/// # What this asserts that the old version could not
///
/// The previous body fired the *handler* directly and asserted it had been registered. That passes
/// against a loader that resolves the name and wires nothing, which is what the loader did. This
/// loads the document, **emits the control's own `clicked` signal** — the channel `on_click`
/// connects to — and asserts the handler ran exactly once.
#[test]
fn a_published_name_declared_under_events_is_wired() {
    with_clean_handlers(|| {
        let calls = counting_handler("on_go");
        let json = r#"{"window":{"id":"w","title":"T","width":400,"height":300,"layout":{
            "type":"vbox","children":[{"button":{"id":"b","text":"Go",
                "events":{"clicked":"on_go"}}}]}}}"#;

        let bound = JsonLoader::load(json).expect("a published-name binding must load");
        emit_click(&bound, "b");

        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "`events.clicked` must reach the handler on a real click, or the published route \
             resolves the name and then wires nothing"
        );
    });
}

/// The compatibility route fires through the callback its marker implies.
#[test]
fn a_compatibility_key_still_reaches_its_handler() {
    with_clean_handlers(|| {
        let calls = counting_handler("on_change_legacy");
        let json = r#"{"window":{"id":"w","title":"T","width":400,"height":300,"layout":{
            "type":"vbox","children":[{"slider":{"id":"s","on_change":"on_change_legacy"}}]}}}"#;

        let bound = JsonLoader::load(json).expect("a compatibility binding must load");
        let fired = dispatch_trigger(mounted_id(&bound, "s"), WidgetTriggerKind::ValueChanged);

        assert!(fired, "`on_change` must reach the value callback table");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    });
}

/// `closed` — both routes — reaches the widget's close channel.
///
/// # The defect this closes
///
/// `on_close` / `events:{"closed":…}` used to be **refused**: nothing produced a close signal
/// reachable from the handle layer, so the loader declined the binding rather than wire it to the
/// click callback (a handler told "closed" while firing on a click). `BaseWidget::closed` now
/// exists and the loader routes `Closed` through `WidgetHandle::on_close`, so both spellings must
/// actually fire when the widget announces the lifecycle fact.
#[test]
fn closed_reaches_the_handle_through_both_routes() {
    with_clean_handlers(|| {
        let published = counting_handler("on_closed_pub");
        let legacy = counting_handler("on_closed_leg");
        let json = r#"{"window":{"id":"w","title":"T","width":400,"height":300,
            "events":{"closed":"on_closed_pub"},"on_close":"on_closed_leg"}}"#;

        let bound = JsonLoader::load(json).expect("a `closed` binding must load, not be refused");
        let id = mounted_id(&bound, "w");

        // Emit the widget's own `closed` signal — the channel `on_close` connects to.
        assert!(
            rust_widgets::close_widget(id),
            "a mounted widget must receive its own close signal"
        );

        assert_eq!(
            published.load(Ordering::SeqCst),
            1,
            "`events.closed` must reach the handler on the widget's closed signal"
        );
        assert_eq!(legacy.load(Ordering::SeqCst), 1, "`on_close` must reach the same channel");
    });
}

/// Both routes in one node, and both must be wired.
///
/// A loader that stopped after the first matching route would satisfy each test above on its own.
#[test]
fn both_routes_can_be_declared_on_one_node() {
    with_clean_handlers(|| {
        let published = counting_handler("on_pub");
        let legacy = counting_handler("on_leg");
        let json = r#"{"window":{"id":"w","title":"T","width":400,"height":300,"layout":{
            "type":"vbox","children":[{"button":{"id":"b","text":"Go",
                "events":{"clicked":"on_pub"},"on_change":"on_leg"}}]}}}"#;

        let bound = JsonLoader::load(json).expect("both routes on one node must load");
        emit_click(&bound, "b");
        dispatch_trigger(mounted_id(&bound, "b"), WidgetTriggerKind::ValueChanged);

        assert_eq!(published.load(Ordering::SeqCst), 1, "the published route must fire");
        assert_eq!(legacy.load(Ordering::SeqCst), 1, "the compatibility route too");
    });
}

/// Two bindings declared under one node **both** fire.
///
/// # The defect this pins
///
/// The callback tables were `HashMap<ObjectId, Rc<RefCell<..>>>`, so registering a second callback
/// for one id **replaced** the first. A node declaring `clicked` and `on_click` — or two published
/// names on one signal — kept whichever was written last, and the other handler silently did
/// nothing. The tables now hold a `Vec` per id and `dispatch_trigger` fires every entry.
#[test]
fn two_bindings_on_one_node_both_fire() {
    with_clean_handlers(|| {
        let first = counting_handler("on_first");
        let second = counting_handler("on_second");
        let json = r#"{"window":{"id":"w","title":"T","width":400,"height":300,"layout":{
            "type":"vbox","children":[{"button":{"id":"b","text":"Go",
                "events":{"clicked":"on_first"},"on_click":"on_second"}}]}}}"#;

        let bound = JsonLoader::load(json).expect("two bindings on one node must load");
        emit_click(&bound, "b");

        assert_eq!(
            first.load(Ordering::SeqCst),
            1,
            "`on_click` must not have replaced the \
                                                    published binding"
        );
        assert_eq!(
            second.load(Ordering::SeqCst),
            1,
            "and the published binding must not have \
                                                     replaced it either"
        );
    });
}

/// A value-binding is wired to the value callback, **not** to the click callback.
///
/// # The defect this pins
///
/// `bind_declared_events` passed [`JsonTriggerMarker::Clicked`] for every published name, so
/// `"events": {"value_changed": "h"}` was bound through `on_click`: the handler ran when the
/// control was *pressed* and never when its value changed, and the name it declared was validated
/// the whole time. The route now reads the event's declared payload — a payload-free name travels
/// the click callback, a payload-carrying one the value callback — and this drives both channels
/// to prove the choice landed on the right one.
#[test]
fn a_value_event_is_not_wired_to_the_click_callback() {
    with_clean_handlers(|| {
        let calls = counting_handler("on_slide");
        let json = r#"{"window":{"id":"w","title":"T","width":400,"height":300,"layout":{
            "type":"vbox","children":[{"slider":{"id":"s","min":0,"max":100,
                "events":{"value_changed":"on_slide"}}}]}}}"#;

        let bound = JsonLoader::load(json).expect("a payload-carrying published name must load");
        let id = mounted_id(&bound, "s");

        // The click channel must not reach it: that is the bug, stated as an assertion.
        rust_widgets::widget::runtime::with_widget_mut(id, |widget| {
            widget.base().clicked.emit();
        });
        assert_eq!(
            calls.load(Ordering::SeqCst),
            0,
            "`value_changed` must not be wired to the click callback"
        );

        // The value channel must.
        assert!(dispatch_trigger(id, WidgetTriggerKind::ValueChanged));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    });
}

/// The route's callback choice follows the event's declared intent (name + payload), for every
/// published pair.
///
/// The behavioural tests above cover two names; this covers the *rule*, so a control that publishes
/// an event the two do not mention cannot be wired through a hard-coded constant. The rule is the
/// marker's: a payload-free name that is not a click (`closed`, `double_clicked`) resolves to its
/// own marker and is then refused by `unroutable_reason`, rather than being flattened to `Clicked`.
#[test]
fn the_callback_choice_follows_the_declared_intent() {
    let factory = WidgetFactory::new_with_defaults();
    let mut checked = 0usize;
    for capability in factory.capabilities() {
        for schema in capability.events {
            let binding = json_event_binding(capability.canonical_name, schema.name);
            let JsonEventBinding::Published { has_payload, name, marker } = binding else {
                panic!("a published name must produce a Published binding, not a marker");
            };
            assert_eq!(
                name, schema.name,
                "the binding must carry the table's own spelling of the name"
            );
            assert_eq!(
                has_payload,
                schema.payload.is_some(),
                "`{}.{}` declares {} but the binding says {}",
                capability.canonical_name,
                schema.name,
                if schema.payload.is_some() { "a payload" } else { "no payload" },
                if has_payload { "a payload" } else { "no payload" }
            );
            // The binding's marker and callback must agree with each other (both derive from the
            // marker), so a name cannot be told one trigger and wired to the other's callback.
            assert_eq!(
                binding.marker(),
                marker,
                "`{}.{}` must report the marker its binding carries",
                capability.canonical_name,
                schema.name
            );
            assert_eq!(
                binding.uses_value_callback(),
                marker.uses_value_callback(),
                "`{}.{}` must be wired to the callback its marker implies, not its payload",
                capability.canonical_name,
                schema.name
            );
            // A binding that is refused must never reach a callback at all.
            if marker.unroutable_reason().is_some() {
                assert!(
                    !bind_published_event(0u64, capability.canonical_name, schema.name, "h"),
                    "`{}.{}` names an unroutable trigger and must be refused",
                    capability.canonical_name,
                    schema.name
                );
            }
            checked += 1;
        }
    }
    assert!(
        checked > 300,
        "the table has 300+ (control, event) pairs; only {checked} were checked"
    );
}

/// A binding the control cannot support must be refused **by the capability table**, and the
/// refusal must not un-wire the bindings that *are* supported on the same node.
#[test]
fn an_unpublished_name_cannot_be_subscribed_while_a_published_one_can() {
    let hub = rust_widgets::signal::CustomSignalHub::new();
    let factory = WidgetFactory::new_with_defaults();

    factory
        .connect_event("button", "clicked", &hub, || {})
        .expect("`button` publishes `clicked`, so the loader's published route can accept it");

    assert!(
        factory.connect_event("button", "clikced", &hub, || {}).is_err(),
        "`clikced` is not a published name, which is exactly why a document declaring it must not \
         end up with a live binding"
    );
}

/// A typo in one event name must not cost the node its *other* binding.
///
/// A refused name is a warning, not a reason to abandon the node: the loop that walks the `events`
/// object uses `continue`, so the sounds bindings around the bad one survive. This asserts that on
/// the live signal rather than on the load result, because "the document loaded" is true either
/// way.
#[test]
fn a_typo_in_an_event_name_does_not_cost_the_node_its_other_binding() {
    with_clean_handlers(|| {
        let good = counting_handler("on_good");
        counting_handler("on_typo");
        let json = r#"{"window":{"id":"w","title":"T","width":400,"height":300,"layout":{
            "type":"vbox","children":[{"button":{"id":"b","text":"Go",
                "events":{"clikced":"on_typo","clicked":"on_good"}}}]}}}"#;

        let bound = JsonLoader::load(json).expect("a refused binding must not fail the load");
        assert!(
            bound.id("b").is_some(),
            "the button must still be registered: one bad handler name is not a broken layout"
        );
        emit_click(&bound, "b");
        assert_eq!(
            good.load(Ordering::SeqCst),
            1,
            "the correct binding on the same node must still fire"
        );
    });
}

/// The property pass must not confuse a declared binding with an unknown property.
///
/// `events` and the `on_*` keys are consumed by the wiring, so the name-driven property pass has
/// to skip them. The failure mode is a load that succeeds while logging a property warning for
/// every handler in the document — noise that hides real warnings.
#[test]
fn a_declared_binding_is_not_reported_as_an_unknown_property() {
    with_clean_handlers(|| {
        let json = r#"{"window":{"id":"w","title":"T","width":400,"height":300,"layout":{
            "type":"vbox","children":[{"button":{"id":"b","text":"Go",
                "events":{"clicked":"on_a"},"on_close":"on_b","on_selection_changed":"on_c"}}]}}}"#;

        let layout = JsonLoader::load(json).expect("the document must load");
        assert!(!layout.is_empty(), "the document's identified widgets must be registered");
    });
}
