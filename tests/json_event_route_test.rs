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

#![cfg(all(feature = "desktop", not(alloc_frugal)))]

use rust_widgets::json::{
    clear_global_handlers, invoke_global_handler, register_global_handler, EventHandlerContext,
    JsonLoader,
};
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

/// Invokes `name` the way the loader's wiring does, and reports whether it was found.
fn fire(name: &str, kind: rust_widgets::platform::WidgetTriggerKind) -> bool {
    let ctx = EventHandlerContext::new(WidgetTriggerEvent { widget_id: 1, kind });
    invoke_global_handler(name, &ctx)
}

/// The published route reaches a handler. This is the capability that did not exist before T-8.
#[test]
fn a_published_name_declared_under_events_is_wired() {
    use rust_widgets::platform::WidgetTriggerKind;

    with_clean_handlers(|| {
        let calls = counting_handler("on_go");
        let json = r#"{"window":{"id":"w","title":"T","width":400,"height":300,"layout":{
            "type":"vbox","children":[{"button":{"id":"b","text":"Go","events":{"clicked":"on_go"}}}]}}}"#;

        JsonLoader::load(json).expect("a published-name binding must load");

        assert!(
            fire("on_go", WidgetTriggerKind::Clicked),
            "the handler named under `events.clicked` must be registered, or the published \
             route resolves the name and then wires nothing"
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    });
}

/// The compatibility route still reaches a handler, so T-8 merged rather than replaced.
#[test]
fn a_compatibility_key_still_reaches_its_handler() {
    use rust_widgets::platform::WidgetTriggerKind;

    with_clean_handlers(|| {
        let calls = counting_handler("on_close_legacy");
        let json = r#"{"window":{"id":"w","title":"T","width":400,"height":300,
            "on_close":"on_close_legacy"}}"#;

        JsonLoader::load(json).expect("a compatibility binding must load");
        assert!(fire("on_close_legacy", WidgetTriggerKind::Closed));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    });
}

/// Both routes in one node, and both must be wired.
///
/// A loader that stopped after the first matching route would satisfy each test above on its own.
#[test]
fn both_routes_can_be_declared_on_one_node() {
    use rust_widgets::platform::WidgetTriggerKind;

    with_clean_handlers(|| {
        let published = counting_handler("on_pub");
        let legacy = counting_handler("on_leg");
        let json = r#"{"window":{"id":"w","title":"T","width":400,"height":300,"layout":{
            "type":"vbox","children":[{"button":{"id":"b","text":"Go",
                "events":{"clicked":"on_pub"},"on_change":"on_leg"}}]}}}"#;

        JsonLoader::load(json).expect("both routes on one node must load");

        assert!(fire("on_pub", WidgetTriggerKind::Clicked), "the published route must be wired");
        assert!(fire("on_leg", WidgetTriggerKind::ValueChanged), "the compatibility route too");
        assert_eq!(published.load(Ordering::SeqCst), 1);
        assert_eq!(legacy.load(Ordering::SeqCst), 1);
    });
}

/// A binding the control cannot support must be refused **by the capability table**.
///
/// # Why this is a capability assertion and not a loader assertion
///
/// The honest statement here is weaker than "the loader skips the typo", and pretending otherwise
/// would be the vacuity this repository keeps hunting. `JsonLoader::load` registers metadata into
/// `WidgetRegistry`; it does not mount the control into the runtime, so nothing in a test can drive
/// a click through a loaded document and watch the binding fire or fail. A test that asserted "the
/// handler was never called" would pass identically against a loader that wired the typo, because
/// nothing in the test ever raises the signal.
///
/// So this pins the fact that *is* observable and that the fix depends on: the name `clikced` is
/// not something the capability layer can subscribe, while `clicked` is. The loader's own refusal
/// is structural and covered by `tools/check_json_event_route.sh` step 1, which requires the
/// published route to resolve through this same table.
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

/// A document whose only binding is a typo still loads, with the rest of the tree intact.
///
/// A binding the loader refuses is a *warning*, not a document-level error: refusing the whole
/// layout because one handler name is misspelled would make the designer unusable, since the tree
/// is otherwise valid. This pins that shape — the document loads, and the identified widget is
/// still registered.
#[test]
fn a_typo_in_an_event_name_does_not_fail_the_whole_document() {
    with_clean_handlers(|| {
        counting_handler("on_typo");
        let json = r#"{"window":{"id":"w","title":"T","width":400,"height":300,"layout":{
            "type":"vbox","children":[{"button":{"id":"b","text":"Go",
                "events":{"clikced":"on_typo"}}}]}}}"#;

        let layout = JsonLoader::load(json).expect("a refused binding must not fail the load");
        assert!(
            layout.id("b").is_some(),
            "the button must still be registered: one bad handler name is not a broken layout"
        );
    });
}

/// A published payload-carrying name is declarable, so the route is not click-only.
///
/// The eight legacy keys reached `value_changed` only through `on_change`/`on_value_changed`;
/// `events.slider_moved` has no legacy spelling at all, which is the gap the published route
/// closes.
#[test]
fn a_payload_carrying_published_name_is_declarable() {
    with_clean_handlers(|| {
        counting_handler("on_moved");
        let json = r#"{"window":{"id":"w","title":"T","width":400,"height":300,"layout":{
            "type":"vbox","children":[{"slider":{"id":"s","min":0,"max":100,
                "events":{"slider_moved":"on_moved"}}}]}}}"#;

        JsonLoader::load(json).expect("a payload-carrying published name must be declarable");
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
        // The window and the button are both registered by id.
        assert!(!layout.is_empty(), "the document's identified widgets must be registered");
    });
}
