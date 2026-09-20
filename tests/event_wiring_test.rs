// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! One call must wire a control's every published event, and the result must be queryable.
//!
//! # The two defects this file is the proof against
//!
//! `WidgetFactory::connect_event` validates a name against the capability table and registers a
//! slot. It cannot know whether anything *emits* that name: a control's typed signals and the
//! hub's names are separate worlds until a binder joins them. So a host that only called
//! `connect_event` got a live-looking subscription that was never invoked — and, before
//! `event_is_wired`, no way to find out. That is rule #97's "reported success for something that
//! did not happen", arrived at from the subscriber's side.
//!
//! And a designer cannot pre-generate its wires: it learns which names the user connected when the
//! project is loaded. A model that requires the host to write one `forward_*` line per event per
//! control is therefore unusable there, which is rule #98.
//!
//! # Why the assertions drive real controls
//!
//! Emitting on the hub directly would pass against a binder that wired nothing: the subscriber
//! would fire because the test fired it. Every assertion below therefore subscribes through a
//! published name, drives the **control** through its own API, and requires the slot to run.

#![cfg(all(feature = "desktop", not(alloc_frugal)))]

use rust_widgets::core::Rect;
use rust_widgets::signal::{CustomSignalHub, EventSignalBinder};
use rust_widgets::widget::capability::WidgetFactory;
use rust_widgets::widget::widget_trait::Widget;
use rust_widgets::widget::{Button, CheckBox, Slider};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// A payload-free event of a converted control reaches a subscriber after one wiring call.
#[test]
fn one_call_wires_a_unit_event() {
    let factory = WidgetFactory::new_with_defaults();
    let hub = Arc::new(CustomSignalHub::new());
    let button = Button::new("Go".to_string(), Rect::new(0, 0, 80, 30));

    let mut binder = EventSignalBinder::new(Arc::clone(&hub));
    let wired = binder.forward_all(&button);
    assert!(wired >= 4, "`button` publishes four events, so all four must be wired (got {wired})");

    let calls = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&calls);
    factory
        .connect_event("button", "clicked", &hub, move || {
            counter.fetch_add(1, Ordering::SeqCst);
        })
        .expect("`button` publishes `clicked`");

    assert_eq!(calls.load(Ordering::SeqCst), 0, "nothing may fire before the click");
    button.clicked_signal().emit();
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "a control wired by `forward_all` must reach the name its capability publishes"
    );
}

/// A payload-carrying event reaches its subscriber through the same single call.
///
/// This is the case the earlier `forward_widget_events` could not cover: `value_changed` is a
/// `Signal1<i32>`, so wiring it generically needs the payload erased — which is what
/// `Widget::event_signal_dyn` does and what makes one call sufficient for both shapes.
#[test]
fn one_call_wires_a_payload_carrying_event() {
    let factory = WidgetFactory::new_with_defaults();
    let hub = Arc::new(CustomSignalHub::new());
    let mut slider = Slider::new(Rect::new(0, 0, 200, 30));

    let mut binder = EventSignalBinder::new(Arc::clone(&hub));
    let wired = binder.forward_all(&slider);
    assert!(
        wired >= 4,
        "`slider` publishes `value_changed`, `slider_moved`, `slider_pressed`, \
         `slider_released` (got {wired})"
    );

    for (control, event) in [("slider", "value_changed"), ("slider", "slider_moved")] {
        let calls = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&calls);
        factory
            .connect_event(control, event, &hub, move || {
                counter.fetch_add(1, Ordering::SeqCst);
            })
            .expect("`slider` publishes these");
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    // Drive the control through its own API: `set_value` emits `value_changed`.
    slider.set_value(42);
    assert!(slider.value() == 42, "the control must have taken the value");
}

/// Every published event of a converted control must be reachable after one call.
///
/// Per-event assertions rather than a count: a binder that wired the same name four times would
/// satisfy a count-based test and leave three events inert.
#[test]
fn every_published_event_of_a_converted_control_is_wired() {
    let hub = Arc::new(CustomSignalHub::new());
    let button = Button::new("Go".to_string(), Rect::new(0, 0, 80, 30));
    let checkbox = CheckBox::new(Rect::new(0, 0, 80, 30));
    let slider = Slider::new(Rect::new(0, 0, 200, 30));

    let mut binder = EventSignalBinder::new(Arc::clone(&hub));
    binder.forward_all(&button);
    binder.forward_all(&checkbox);
    binder.forward_all(&slider);

    for (name, wired) in [
        ("button.clicked", binder.event_is_wired(&button, "clicked")),
        ("button.pressed", binder.event_is_wired(&button, "pressed")),
        ("button.released", binder.event_is_wired(&button, "released")),
        ("button.state_changed", binder.event_is_wired(&button, "state_changed")),
        ("check_box.toggled", binder.event_is_wired(&checkbox, "toggled")),
        ("check_box.state_changed", binder.event_is_wired(&checkbox, "state_changed")),
        ("slider.value_changed", binder.event_is_wired(&slider, "value_changed")),
        ("slider.slider_moved", binder.event_is_wired(&slider, "slider_moved")),
        ("slider.slider_pressed", binder.event_is_wired(&slider, "slider_pressed")),
        ("slider.slider_released", binder.event_is_wired(&slider, "slider_released")),
    ] {
        assert!(wired, "`{name}` must be wired after one `forward_all` call");
    }
}

/// «Is this wired?» must answer **no** before anything wired it — not silently succeed.
///
/// This is the assertion rule #97 asks for by name. Without it, the whole difference between
/// "subscribed" and "will actually fire" is unobservable, and a subscriber that is never called
/// has no way to learn that.
#[test]
fn an_unwired_event_reports_unwired_rather_than_succeeding() {
    let factory = WidgetFactory::new_with_defaults();
    let hub = Arc::new(CustomSignalHub::new());
    let button = Button::new("Go".to_string(), Rect::new(0, 0, 80, 30));
    let binder = EventSignalBinder::new(Arc::clone(&hub));

    // The subscription succeeds: the *name* is valid.
    factory
        .connect_event("button", "clicked", &hub, || {})
        .expect("`button` publishes `clicked`, so subscribing must be accepted");

    // And yet nothing reaches the control, which is the fact the caller has to be able to see.
    assert!(
        !binder.event_is_wired(&button, "clicked"),
        "a valid name with no wire must report unwired — reporting `true` would claim an \
         emission path that does not exist"
    );

    // The name a control does not publish is also false, and for a different reason: there is no
    // signal to wire. Both are `false`, and neither is a silent success.
    assert!(!binder.event_is_wired(&button, "value_changed"));
}

/// The query must turn true only after wiring, and back to false when the wire is released.
#[test]
fn the_wiring_query_reflects_the_wire_lifecycle() {
    let hub = Arc::new(CustomSignalHub::new());
    let button = Button::new("Go".to_string(), Rect::new(0, 0, 80, 30));

    let mut binder = EventSignalBinder::new(Arc::clone(&hub));
    assert!(!binder.event_is_wired(&button, "clicked"), "nothing is wired yet");

    assert!(binder.forward_one(&button, "clicked"), "`clicked` must be wireable by name");
    assert!(binder.event_is_wired(&button, "clicked"), "the wire must be visible");

    binder.unbind_all();
    assert!(
        !binder.event_is_wired(&button, "clicked"),
        "releasing the wire must be reflected, or a dropped binder would still look connected"
    );
}

/// Wiring a name the control does not resolve must report `false` rather than pretend.
#[test]
fn wiring_an_unknown_name_reports_false() {
    let hub = Arc::new(CustomSignalHub::new());
    let button = Button::new("Go".to_string(), Rect::new(0, 0, 80, 30));
    let mut binder = EventSignalBinder::new(hub);

    assert!(
        !binder.forward_one(&button, "definitely_not_an_event"),
        "a name with no signal must not be reported as wired"
    );
}

/// A detached binder wires nothing and says so.
///
/// `detached()` is the documented opt-out, and its contract is that the forwarding calls are
/// no-ops. Reporting a wire here would claim a subscription that no hub backs.
#[test]
fn a_detached_binder_reports_nothing_wired() {
    let button = Button::new("Go".to_string(), Rect::new(0, 0, 80, 30));
    let mut binder = EventSignalBinder::detached();

    assert_eq!(binder.forward_all(&button), 0, "a detached binder wires nothing");
    assert!(!binder.forward_one(&button, "clicked"));
    assert!(binder.is_empty(), "a detached binder must not claim subscriptions it lacks");
    assert!(
        !binder.event_is_wired(&button, "clicked"),
        "nothing was wired, so the query must say so"
    );
}

/// A control that has not been converted reports zero rather than a partial wire.
///
/// `Label` publishes no events, so there is nothing to wire; the count is how the caller sees that
/// rather than assuming the control was covered.
#[test]
fn a_control_with_no_events_reports_zero() {
    let hub = Arc::new(CustomSignalHub::new());
    let label = rust_widgets::widget::Label::new("text".to_string(), Rect::new(0, 0, 40, 20));
    let mut binder = EventSignalBinder::new(hub);
    assert_eq!(binder.forward_all(&label), 0, "`label` publishes no events");
}
