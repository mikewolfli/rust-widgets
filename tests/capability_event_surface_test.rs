// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Every capability's **published events** must be addressable by the name it publishes.
//!
//! # The third promise, and how it differs from the first two
//!
//! A `WidgetCapability` declares three lists:
//!
//! 1. `canonical_name` / `aliases` — round 31.
//! 2. `properties` — round 32.
//! 3. `commands` — round 33 (contract added; per-control rollout in progress).
//! 4. `events` — **159 capabilities publish 281 event names**, and until this round
//!    nothing consumed the list at all.
//!
//! For a command, "unused" meant a promise with no invocation path (round 33) — now
//! fixed by `WidgetFactory::invoke_command`. For an **event**, there is no invocation
//! to make: a capability can never be asked to *emit* an event, because the control
//! emits it as a consequence of user interaction. "Unused" here therefore means
//! something different and worse — a consumer that discovers a control through the
//! registry, reads `events`, and offers "subscribe to `selection_changed`" has no way
//! to turn that name into a subscription. The name is inert data.
//!
//! # What this round asserts
//!
//! The library already owns the mechanism the list needs: `signal::CustomSignalHub` is
//! a name-addressed signal registry (`connect(name, slot)` / `emit(name)`). What was
//! missing is the bridge from a *capability's published names* to *that hub*, so a name
//! read from the registry addresses the same signal the control emits.
//!
//! These tests state the bridge, from both ends:
//!
//! * every published event name is a **connected name**, so it can be subscribed to;
//! * an event the control does not publish is **refused**, so the bridge does not
//!   accept anything and report success.

#![cfg(all(feature = "desktop", not(alloc_frugal)))]

use rust_widgets::core::Rect;
use rust_widgets::signal::CustomSignalHub;
use rust_widgets::widget::capability::WidgetFactory;

/// Every published event name must be subscribable through the event bridge.
#[test]
fn every_published_event_name_is_subscribable() {
    let factory = WidgetFactory::new_with_defaults();
    let hub = CustomSignalHub::new();

    let mut total = 0usize;
    let mut refused: Vec<(&str, &str)> = Vec::new();

    for capability in factory.capabilities() {
        for event in capability.events {
            total += 1;
            // The consumer's path: take the name the registry published, hand it to
            // the bridge, and the bridge must accept it. A name it refuses is a name
            // no consumer can use, which makes the published list decorative.
            if factory.event_is_subscribable(capability.canonical_name, event, &hub).is_err() {
                refused.push((capability.canonical_name, event));
            }
        }
    }

    assert!(
        total > 0,
        "no capability publishes events, so this test proves nothing — the schema field or its \
         wiring has stopped reaching the property layer"
    );
    assert!(
        refused.is_empty(),
        "a capability publishes events no consumer can subscribe to, so the published list is \
         inert data (control, event): {refused:?}"
    );
}

/// An event name the control does not publish must be refused.
///
/// # Why the negative direction is required
///
/// A bridge that accepts every name — `Ok(())` unconditionally — passes the test
/// above while providing nothing: the caller would subscribe to an event that can
/// never fire and never learn why. This asserts the refusal, so a permissive
/// implementation cannot satisfy both tests.
#[test]
fn an_unpublished_event_name_is_refused() {
    let factory = WidgetFactory::new_with_defaults();
    let hub = CustomSignalHub::new();

    // `button` publishes `clicked`; that one must be accepted.
    assert!(
        factory.event_is_subscribable("button", "clicked", &hub).is_ok(),
        "button publishes `clicked`, so the bridge must accept it"
    );

    // A name no capability publishes must not be accepted silently.
    assert!(
        factory.event_is_subscribable("button", "definitely_not_an_event", &hub).is_err(),
        "the bridge accepted an event name the control does not publish, so a subscriber \
         would wait for a signal that can never fire"
    );

    // A name published by a *different* control must also be refused: the question is
    // about this control's events, not about the union of every event name in the
    // library.
    assert!(
        factory.event_is_subscribable("button", "selection_changed", &hub).is_err(),
        "the bridge accepted an event another control publishes, so the check is looking \
         at the wrong list"
    );
}

/// An unknown control must be refused rather than answered from a fallback list.
#[test]
fn an_unknown_control_has_no_events() {
    let factory = WidgetFactory::new_with_defaults();
    let hub = CustomSignalHub::new();
    assert!(
        factory.event_is_subscribable("no_such_control", "clicked", &hub).is_err(),
        "an unknown control name was accepted, so the bridge is not resolving the control"
    );
    let _ = Rect::new(0, 0, 1, 1);
}

/// The bridge must connect a **working** subscription, not merely validate the name.
///
/// # Why this is the test that matters most
///
/// The three tests above could all pass against an implementation that checks the
/// name and then throws the slot away — the caller would get `Ok`, hold a handle, and
/// never be called. That is the same "reported success for something that did not
/// happen" failure the command contract was built to prevent, so the bridge is
/// asserted by *delivery*: subscribe through the published name, emit on the hub under
/// the same name, and require the slot to have run.
#[test]
fn a_published_event_subscription_actually_delivers() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    let factory = WidgetFactory::new_with_defaults();
    let hub = CustomSignalHub::new();

    let delivered = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&delivered);

    factory
        .connect_event("list_box", "selection_changed", &hub, move || {
            counter.fetch_add(1, Ordering::SeqCst);
        })
        .expect("list_box publishes `selection_changed`");

    assert_eq!(delivered.load(Ordering::SeqCst), 0, "the slot must not run before an emit");

    hub.emit("selection_changed");

    assert_eq!(
        delivered.load(Ordering::SeqCst),
        1,
        "an event subscribed to through its published name did not receive the emit, so the \
         bridge validated the name without connecting the slot"
    );
}

/// Events with a payload must be reachable too, by the same published name.
///
/// # Why the payload case needs its own assertion
///
/// Most of the 281 published names carry a value (`value_changed` with an `f32`,
/// `text_changed` with a `String`). A bridge that only wired up zero-argument signals
/// would leave the majority unusable, and the count-based test above would not notice.
#[test]
fn a_published_event_with_a_payload_is_reachable_by_name() {
    let factory = WidgetFactory::new_with_defaults();
    let hub = CustomSignalHub::new();

    // `value_changed` is published by `slider`; proving the name is accepted is the
    // point here, because the payload-carrying emit path is the hub's own concern.
    assert!(
        factory.event_is_subscribable("slider", "value_changed", &hub).is_ok(),
        "a payload-carrying event published by `slider` was not accepted by the bridge"
    );
    assert!(
        factory.connect_event("slider", "value_changed", &hub, || {}).is_ok(),
        "the payload-carrying event could be probed but not subscribed to"
    );
}
