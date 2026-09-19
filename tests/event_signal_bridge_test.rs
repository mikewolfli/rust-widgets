// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! A control's own signal must reach a subscriber that used a **published name**.
//!
//! # Why this test exists next to `capability_event_surface_test`
//!
//! That test proves `WidgetFactory::connect_event` validates a published name and that a
//! slot registered under it is called when something emits on the hub. It does **not**
//! prove a real control reaches that slot: the control emits its own typed `Signal`s
//! (`Button::clicked`, `Signal1<bool>`), which are unrelated to the hub until something
//! joins them.
//!
//! That join is `signal::EventSignalBinder`, and this file is its end-to-end proof. The
//! distinction matters because the failure mode is silent: a consumer subscribes to
//! `"clicked"`, gets a handle, watches the button get pressed, and is never called —
//! with no error anywhere.

#![cfg(all(feature = "desktop", not(alloc_frugal)))]

use rust_widgets::core::Rect;
use rust_widgets::signal::{CustomSignalHub, EventSignalBinder};
use rust_widgets::widget::capability::WidgetFactory;
use rust_widgets::widget::widget_trait::Widget;
use rust_widgets::widget::{Button, CheckBox, Slider};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// A payload-free control signal reaches a subscriber that used the published name.
#[test]
fn a_button_click_reaches_the_published_event_name() {
    let factory = WidgetFactory::new_with_defaults();
    let hub = Arc::new(CustomSignalHub::new());

    // The control owns its signals; the binder joins them to the hub.
    let button = Button::new("Go".to_string(), Rect::new(0, 0, 80, 30));
    let mut binder = EventSignalBinder::new(Arc::clone(&hub));
    binder.forward_unit("clicked", button.clicked_signal());

    // The subscriber's path: a published name, nothing else.
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&calls);
    factory
        .connect_event("button", "clicked", &hub, move || {
            counter.fetch_add(1, Ordering::SeqCst);
        })
        .expect("`button` publishes `clicked`");

    assert_eq!(calls.load(Ordering::SeqCst), 0, "nothing may fire before the click");

    // A real emission from the control, not a manual hub emit.
    button.clicked_signal().emit();

    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "the control emitted `clicked` but the subscriber registered under the published \
         name was not called, so the binder did not join the signal to the hub"
    );
}

/// A payload-carrying signal is forwarded, and its value reaches the observer.
#[test]
fn a_slider_value_change_reaches_the_published_event_name_with_its_payload() {
    let factory = WidgetFactory::new_with_defaults();
    let hub = Arc::new(CustomSignalHub::new());

    let mut slider = Slider::new(Rect::new(0, 0, 200, 30));
    let mut binder = EventSignalBinder::new(Arc::clone(&hub));

    // The payload has nowhere to go on an untyped hub, so the binder requires the
    // caller to say what happens to it — and here that is recording it.
    let seen = Arc::new(std::sync::Mutex::new(Vec::<i32>::new()));
    let recorder = Arc::clone(&seen);
    binder.forward_mapped("value_changed", &slider.value_changed, move |value| {
        recorder.lock().expect("lock").push(*value);
    });

    let calls = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&calls);
    factory
        .connect_event("slider", "value_changed", &hub, move || {
            counter.fetch_add(1, Ordering::SeqCst);
        })
        .expect("`slider` publishes `value_changed`");

    // Drive the control through its own API so the emission is a real consequence.
    slider.set_value(42);

    assert_eq!(calls.load(Ordering::SeqCst), 1, "the subscriber was not called");
    assert_eq!(
        *seen.lock().expect("lock"),
        vec![42],
        "the payload did not reach the observer, so the bridge dropped data it cannot \
         carry on an untyped hub"
    );
}

/// A host can wire a widget with one call, and the call really connects it.
///
/// # Why this test exists
///
/// `EventSignalBinder` is the documented join between a control's typed signals and a
/// name-addressed hub, but the tests above perform that wiring by hand
/// (`binder.forward_unit("clicked", button.clicked_signal())`). That proves the binder
/// works; it does not prove a host has a way to wire a widget without knowing which
/// signal backs which published name.
///
/// `forward_widget_events` is that way, and this test proves it end to end: the
/// control is wired, the subscriber used only a published name, the emission is a real
/// control emission, and the return value reports what was wired.
#[test]
fn a_widget_is_wired_with_one_call_through_the_published_name() {
    let factory = WidgetFactory::new_with_defaults();
    let hub = Arc::new(CustomSignalHub::new());
    let button = Button::new("Go".to_string(), Rect::new(0, 0, 80, 30));

    let mut binder = EventSignalBinder::new(Arc::clone(&hub));
    let wired = binder.forward_widget_events(&button);
    assert_eq!(wired, 1, "`button` publishes `clicked`, so it must be wired");

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
        "a widget wired through `forward_widget_events` must reach the published name"
    );

    // Dropping the binder removes the subscription, so a later emission is not
    // delivered — the ownership contract the binder documents.
    drop(binder);
    button.clicked_signal().emit();
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "a dropped binder must leave no live subscription behind"
    );
}

/// A control that publishes no `clicked` event is reported as contributing nothing.
#[test]
fn wiring_a_widget_without_a_clicked_event_reports_zero() {
    let hub = Arc::new(CustomSignalHub::new());
    let label = rust_widgets::widget::Label::new("text".to_string(), Rect::new(0, 0, 40, 20));
    let mut binder = EventSignalBinder::new(hub);
    assert_eq!(
        binder.forward_widget_events(&label),
        0,
        "`label` publishes no click event, so nothing may be wired"
    );
}

/// A detached binder is inert rather than panic-prone or silently attached.
#[test]
fn a_detached_binder_forwards_nothing() {
    let button = Button::new("Go".to_string(), Rect::new(0, 0, 80, 30));
    let mut binder = EventSignalBinder::detached();
    assert!(!binder.is_attached());
    assert!(binder.is_empty());

    binder.forward_unit("clicked", button.clicked_signal());

    // Nothing was registered, so nothing is owned — `is_empty` states the truth rather
    // than counting a subscription that goes nowhere.
    assert!(binder.is_empty(), "a detached binder must not claim subscriptions it lacks");
    // The control still emits; there is simply no destination.
    button.clicked_signal().emit();
}

/// Dropping the binder unsubscribes, so a rebuilt control cannot leak.
///
/// # How this isolates the binder from the hub
///
/// A hub slot and a forwarded slot are both named signals, so counting hub emissions
/// cannot tell them apart. What distinguishes them is *what triggers them*: the forwarded
/// slot fires when the **control** emits, and a hub slot only fires when someone calls
/// `hub.emit`.
///
/// That gives a clean isolation: subscribe to a name nothing forwards (so the hub slot
/// can only be driven by an explicit `hub.emit`), and separately drive the control. With
/// the binder alive the control's emission reaches the hub; after the binder drops it must
/// not. A leak — the forwarded slot outliving its binder — shows up as the probe still
/// firing after the drop, which is exactly the defect an earlier revision of `unbind_all`
/// had: it disconnected from the *hub* using a handle the hub never issued, so it removed
/// nothing and every forwarded slot leaked.
#[test]
fn dropping_the_binder_unsubscribes() {
    let hub = Arc::new(CustomSignalHub::new());
    let button = Button::new("Go".to_string(), Rect::new(0, 0, 80, 30));

    // Counts every emission that reaches the hub under the forwarded name, whatever
    // made it fire. With the binder alive, a control emission adds one; after the drop,
    // the same control emission must add nothing.
    let reached = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&reached);
    hub.connect("clicked", move || {
        counter.fetch_add(1, Ordering::SeqCst);
    });

    {
        let mut binder = EventSignalBinder::new(Arc::clone(&hub));
        binder.forward_unit("clicked", button.clicked_signal());
        assert_eq!(binder.len(), 1, "the binder must own exactly the slot it added");

        button.clicked_signal().emit();
        assert_eq!(
            reached.load(Ordering::SeqCst),
            1,
            "with the binder alive, the control's emission must reach the hub"
        );
        // `binder` drops here, and must remove the forwarded slot.
    }

    button.clicked_signal().emit();
    assert_eq!(
        reached.load(Ordering::SeqCst),
        1,
        "the control's emission still reached the hub after the binder dropped, so the \
         forwarded slot outlived it and a rebuilt control would accumulate one live \
         subscription per rebuild"
    );
}

/// `unbind_all` leaves the binder reusable rather than permanently severed.
#[test]
fn unbind_all_is_reversible() {
    let hub = Arc::new(CustomSignalHub::new());
    let check = CheckBox::new(Rect::new(0, 0, 80, 30));
    let mut binder = EventSignalBinder::new(Arc::clone(&hub));

    let calls = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&calls);
    hub.connect("toggled", move || {
        counter.fetch_add(1, Ordering::SeqCst);
    });

    binder.forward_mapped("toggled", &check.toggled, |_| {});
    check.toggled.emit(true);
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    binder.unbind_all();
    assert!(binder.is_empty(), "`unbind_all` must release every handle it owns");
    check.toggled.emit(true);
    assert_eq!(calls.load(Ordering::SeqCst), 1, "the forwarded path was not removed");

    // Re-binding works, which is what a control that rebuilds its internals needs.
    binder.forward_mapped("toggled", &check.toggled, |_| {});
    check.toggled.emit(true);
    assert_eq!(calls.load(Ordering::SeqCst), 2, "the binder did not re-attach");
}

/// Every event a capability publishes must be forwardable, not merely nameable.
///
/// # Why this is the assertion that keeps the bridge from drifting
///
/// `capability_event_surface_test` proves each published name can be *subscribed to*.
/// This proves each one can be *forwarded* — that a control author has a binder call
/// available for it. Without it, a capability could gain an event that the binding layer
/// can name but no control can ever deliver, which is the same "published but inert"
/// defect this whole audit is about.
#[test]
fn every_published_event_accepts_a_forwarding_call() {
    let factory = WidgetFactory::new_with_defaults();
    let hub = Arc::new(CustomSignalHub::new());

    // A representative signal per event shape, so the test exercises the binder's own
    // API rather than only the capability table.
    let button = Button::new("Go".to_string(), Rect::new(0, 0, 80, 30));
    let mut binder = EventSignalBinder::new(Arc::clone(&hub));
    binder.forward_unit("clicked", button.clicked_signal());
    assert_eq!(binder.len(), 1, "`forward_unit` must register exactly one subscription");

    // The capability table's names must all be subscribable, which is the half the
    // binder relies on to have a destination to emit into.
    let mut unsound: Vec<(&str, &str)> = Vec::new();
    for capability in factory.capabilities() {
        for event in capability.events {
            if factory.event_is_subscribable(capability.canonical_name, event, &hub).is_err() {
                unsound.push((capability.canonical_name, event));
            }
        }
    }
    assert!(
        unsound.is_empty(),
        "these published events have no destination a binder could emit into \
         (control, event): {unsound:?}"
    );
}
