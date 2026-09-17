// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! End-to-end closure: `Binding::set` → `View::build` → `diff` → `apply` → live control.
//!
//! # Why this test exists (BLUE18 gap G5)
//!
//! `src/data_binding/` has been complete for several rounds, and so has
//! `src/view/`. Each was tested on its own. What was never tested is the **join
//! between them** — that a value change on a `Binding` actually reaches a control
//! through the declarative layer, with that control being one the widget factory
//! really created rather than a stub id.
//!
//! The gap is not a formality. Every part can be individually correct while the
//! join is wrong: the listener could fire while the view was rebuilt from a stale
//! copy; the diff could be right while `apply` wrote to a path the engine had
//! already re-indexed. A test at either end would still pass, which is why the
//! assertion here is on the live control's own property.
//!
//! # Why the assertion is not staged
//!
//! BLUE18 Phase F-2 asks for a single non-staged assertion, deliberately: a test
//! that asserts "the listener fired", then "a patch was produced", then "the
//! property changed" spends its strength on intermediate steps and can be satisfied
//! by them individually — for instance by checking the listener fired while the
//! write was still being refused. The assertion is on the end state; the chain that
//! produced it appears in the failure message, so a regression still says where it
//! broke.

use rust_widgets::core::{ObjectId, Rect};
use rust_widgets::data_binding::{Binding, FnListener};
use rust_widgets::view::{Node, View, ViewEngine};
use rust_widgets::widget::capability::properties_trait::widget_property_get;
use rust_widgets::widget::capability::CapabilityValue;
use rust_widgets::widget::{runtime, Widget, WidgetFactory};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// A view of one label whose text comes from shared state.
///
/// The binding is **borrowed**, not cloned. `Binding` is `Arc`-backed and
/// deliberately not `Clone`: two clones would be two handles to one value with no
/// way to tell them apart. A view that owned a *copy* of the value would be stale
/// by construction, so borrowing is not a test convenience — it is what makes
/// `build` re-read the current value on every call, which is the property a
/// reactive view depends on.
struct LabelState<'a> {
    text: &'a Binding<String>,
}

impl View for LabelState<'_> {
    fn build(&self) -> Node {
        Node::new("panel").key("root").child(
            Node::new("label")
                .key("greeting")
                .prop("text", CapabilityValue::String(self.text.get())),
        )
    }
}

/// A creator that builds **real** controls through the widget factory.
///
/// The engine takes creation as an injected closure, so this is where a test
/// chooses between stubs and the real thing. This one chooses the real thing: a
/// stub id would let the test pass while `apply`'s property write was being
/// refused, because nothing would be there to hold the value.
struct RealCreator {
    factory: WidgetFactory,
}

impl RealCreator {
    fn new() -> Self {
        Self { factory: WidgetFactory::new_with_defaults() }
    }

    fn creator(&self) -> impl Fn(&Node) -> Option<ObjectId> + '_ {
        move |node: &Node| {
            // Return `None` rather than a stub when the factory does not know the
            // name: an unknown name is a defect in this test's own view, and `None`
            // makes the engine report `UnknownWidgetType` rather than mount a
            // fabricated tree that would make the assertions below meaningless.
            let widget: Box<dyn Widget> = self.factory.create(
                &node.widget,
                Rect::new(0, 0, 120, 32),
                node.key_str().unwrap_or("anon"),
            )?;
            runtime::register(widget)
        }
    }
}

/// The `text` property of a live control, or `None` when it is not a string.
fn live_text(id: ObjectId) -> Option<String> {
    runtime::with_widget(id, |widget| {
        widget_property_get(widget, "text").ok().and_then(|value| match value {
            CapabilityValue::String(text) => Some(text),
            _ => None,
        })
    })
    .flatten()
}

/// `Binding::set(value)` must reach the live control's `text` property.
///
/// # Why the listener is a counter and the UI work happens after it
///
/// `BindingListener` requires `Send` (a binding may be set from any thread), while
/// `ViewEngine` and every control are `!Send` — `runtime.rs`'s registry is
/// thread-local. A listener therefore **cannot** hold the engine. That is the
/// contract, not an obstacle, and the shape it forces — a thread-safe notification,
/// then work on the UI thread — is how a real host uses the pair. This test drives
/// exactly that shape and keeps its assertion on the live control.
#[test]
fn binding_set_reaches_the_live_control_through_the_view_engine() {
    let text = Binding::new(String::from("first"));
    let creator = RealCreator::new();
    let mut engine = ViewEngine::new();

    // Mount: the control starts at the binding's initial value.
    let mounted = engine.mount(&LabelState { text: &text }, &creator.creator());
    assert!(
        mounted.widgets_created >= 2 && mounted.errors.is_empty(),
        "mount must create a real panel and label with no refused writes: {mounted:?}"
    );

    let label_id = engine.id_at(&[0]).expect("the label is the first child of the root");
    assert_eq!(live_text(label_id).as_deref(), Some("first"));

    // A `Send` notification the UI thread can observe.
    let notifications = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&notifications);
    text.subscribe(
        "engine",
        Box::new(FnListener::new(move |_key, _operation| {
            counter.fetch_add(1, Ordering::SeqCst);
        })),
    );

    // ── The state change, then the UI thread's response to it. ──
    text.set(String::from("second"));
    engine.update(&LabelState { text: &text }, &creator.creator());

    assert_eq!(
        live_text(label_id).as_deref(),
        Some("second"),
        "after Binding::set the live control must hold the new value \
         (notifications delivered={}, listeners={})",
        notifications.load(Ordering::SeqCst),
        text.listener_count()
    );
}

/// The hybrid architecture's actual promise, asserted end to end: an update must not
/// rebuild the controls that did not change.
///
/// This is what separates "declarative layered on retained" from "rebuild everything
/// each frame". Without it, the test above would also pass on an implementation that
/// tore the tree down and recreated it on every update — the architecture this work
/// exists to avoid (BLUE18 rule #90).
#[test]
fn a_binding_driven_update_keeps_the_unchanged_control_identity() {
    let text = Binding::new(String::from("v1"));
    let creator = RealCreator::new();
    let mut engine = ViewEngine::new();
    engine.mount(&LabelState { text: &text }, &creator.creator());

    let root_before = engine.id_at(&[]).expect("root mounted");
    let label_before = engine.id_at(&[0]).expect("label mounted");

    // Two updates, each changing only the label's text.
    for value in ["v2", "v3"] {
        text.set(String::from(value));
        let report = engine.update(&LabelState { text: &text }, &creator.creator());
        assert!(
            report.replaced_subtrees == 0,
            "a text-only change must not replace any subtree: {report:?}"
        );
    }

    let root_after = engine.id_at(&[]);
    let label_after = engine.id_at(&[0]);
    assert_eq!(
        (root_after, label_after),
        (Some(root_before), Some(label_before)),
        "a text-only update must leave every control's identity alone — new ids here \
         mean the tree was rebuilt rather than patched"
    );
    assert_eq!(
        live_text(label_before).as_deref(),
        Some("v3"),
        "and the surviving control must hold the last value"
    );
}
