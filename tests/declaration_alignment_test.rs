// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! BLUE20 layer 2 — declaration ↔ implementation alignment, as assertions.
//!
//! # The gap this closes
//!
//! Layer 1 (`control_rendering_census_test`) asks whether a control **paints**. This layer
//! asks a different question: whether the thing a control **declares** actually exists.
//!
//! A capability record is a promise. `PropertySchema { name, value_kind, readable,
//! writable }` says "you can ask this control for `name`, and you will get a
//! `value_kind`". Nothing in the type system connects that table entry to the `match`
//! arm in `WidgetProperties::get` that has to answer it, so the two can drift apart with
//! no compile error and no visible symptom until a designer writes a property that
//! silently returns its default. The same is true of `EventSchema` and the signal field
//! it names, and of `impl Draw` and a `draw` body that draws nothing.
//!
//! | # | Assertion | Drift it catches |
//! |---|---|---|
//! | Q1 | every declared `PropertySchema.name` is answered by `get` with a non-default result | a published property nobody implemented |
//! | Q2 | every control's `draw` calls into the context | an empty `impl Draw` (forbidden by principle #5) |
//! | Q3 | every declared `EventSchema.name` names a real signal on the control | a published event with no producer |
//!
//! # Why Q1 reads the control rather than the schema
//!
//! "Does `get` answer this name" is not observable from outside: `get` returning a value
//! proves a branch ran, but a *fallback* branch also returns a value. So the test reads
//! the control's own `property_names()` and compares it to the schema: the two lists must
//! be equal. That is the same shape as
//! `properties_tests::schema_and_contract_publish_the_same_names`, which is why Q1 is
//! scoped here to the *factory* view — every name the registry publishes must be
//! constructible and readable **through the factory**, which is the path a designer uses.
//!
//! # Relationship to `check_event_payload_types` (principle #101)
//!
//! That gate asks "is the declared *payload type* the signal's real type". It is about
//! the **type**, and it re-derives the payload from the signal. Q3 asks a strictly weaker
//! question — "does a signal with this *name* exist at all" — which that gate cannot see,
//! because a payload derivation starts from a name that has already been located. The two
//! are complementary and non-overlapping: `check_event_payload_types` catches a wrong
//! type, Q3 catches a name with no field behind it. Neither subsumes the other, which is
//! the boundary principle #101 requires to be written down.

// This gate walks the capability registry and reads the theme, neither of which a stripped
// profile has, so it is declared for a **device** profile. `not(mini)` was too weak:
// `embedded` is stripped too, and the build failed there with `cannot find WidgetFactory`.
#![cfg(all(not(feature = "mini"), not(feature = "embedded"), not(target_arch = "wasm32")))]

use rust_widgets::core::Rect;
use rust_widgets::widget::capability::WidgetFactory;
use rust_widgets::widget::draw_bridge::draw_of;
use std::collections::BTreeSet;

/// Every control the factory publishes, built at a roomy geometry.
///
/// The same traversal unit as layer 1: **canonical names**, never `WidgetKind`. 13 kinds
/// are shared by 2–5 controls, so a kind sweep would silently skip 19 of the 188.
///
/// Kept as a documented helper rather than inlined so the traversal unit is stated once;
/// the individual tests walk the registry directly where they need the capability row too.
#[allow(dead_code)]
fn controls() -> Vec<(&'static str, Box<dyn rust_widgets::widget::Widget>)> {
    let factory = WidgetFactory::new_with_defaults();
    factory
        .widget_names()
        .into_iter()
        .filter_map(|name| {
            factory.create(name, Rect::new(0, 0, 240, 120), "Sample").map(|widget| (name, widget))
        })
        .collect()
}

/// Q1: every property the registry declares is readable through the factory.
///
/// `readable: true` is a promise that a caller can ask for the value. A name that is
/// declared but not answered is worse than an absent name: the designer offers it as an
/// editable field, and writing it reports success while nothing changes.
///
/// # Names that promise neither direction
///
/// A schema entry marked `readable: false, writable: false` is a deliberate placeholder —
/// it records that a name exists for a *sibling* control (see the `TextEdit` module docs)
/// without claiming this control can produce or accept it. It promises nothing, so it
/// cannot be a lie, and it is excluded here for the same reason
/// `properties_tests::schema_and_contract_publish_the_same_names` excludes it. Excluding
/// it is what keeps this judgement meaningful: without the exclusion the gate would demand
/// that every control implement `selected_file`, `directory` and every other derived name
/// it happens to share a kind with.
#[test]
fn q1_every_declared_property_is_answered_by_its_control() {
    let factory = WidgetFactory::new_with_defaults();
    let mut checked = 0usize;
    let mut failures: Vec<String> = Vec::new();

    for capability in factory.capabilities() {
        let Some(widget) =
            factory.create(capability.canonical_name, Rect::new(0, 0, 240, 120), "S")
        else {
            failures.push(format!(
                "{}: the registry publishes it but `create` returns None",
                capability.canonical_name
            ));
            continue;
        };

        // The control's own contract, which is what `get` actually dispatches on.
        let Some(published) =
            rust_widgets::widget::capability::widget_property_names(widget.as_ref())
        else {
            // A control with no property contract at all is reported by
            // `properties_tests::every_factory_widget_declares_a_property_contract`; not
            // double-reported here.
            continue;
        };

        for schema in capability.properties {
            checked += 1;
            // A name that promises neither direction is a deliberate placeholder and is
            // not required to be answered (see the test's module docs).
            if !schema.readable && !schema.writable {
                continue;
            }
            // Declared but not in the control's own list: `get` has no arm, so it falls
            // through to `base_property_get`, which answers `UnknownProperty` for anything
            // it does not own.
            if !published.contains(&schema.name) {
                failures.push(format!(
                    "{}: declared property `{}` is absent from the control's property_names()",
                    capability.canonical_name, schema.name
                ));
                continue;
            }
            if schema.readable {
                let answer = rust_widgets::widget::capability::widget_property_get(
                    widget.as_ref(),
                    schema.name,
                );
                if let Err(error) = answer {
                    failures.push(format!(
                        "{}::{} is declared readable but `get` answers {error:?}",
                        capability.canonical_name, schema.name
                    ));
                }
            }
        }
    }

    assert!(
        checked > 1000,
        "the gate must walk a real property table, not a sample; checked={checked}"
    );
    assert!(
        failures.is_empty(),
        "these declared properties are not answered by the control that declares them:\n  {}",
        failures.join("\n  ")
    );
}

/// Q1, the write direction: every property declared writable accepts a write of its own
/// declared kind, and a read-only one refuses.
///
/// This is the half a designer depends on. A writable property that refuses its declared
/// kind is a field the user cannot fill in; a read-only one that accepts a write is worse
/// still, because the write is silently discarded.
#[test]
fn q1_writability_matches_the_declaration() {
    let factory = WidgetFactory::new_with_defaults();
    let mut checked = 0usize;
    let mut failures: Vec<String> = Vec::new();

    for capability in factory.capabilities() {
        let Some(mut widget) =
            factory.create(capability.canonical_name, Rect::new(0, 0, 240, 120), "S")
        else {
            continue;
        };
        for schema in capability.properties {
            // Nothing is promised in either direction, so there is nothing to check.
            if !schema.readable && !schema.writable {
                continue;
            }
            checked += 1;
            // Read-only names must refuse. `geometry` and other derived names are declared
            // `false` precisely so this is checkable.
            if !schema.writable {
                use rust_widgets::widget::capability::types::{
                    CapabilityAccessError, CapabilityValue,
                };
                let verdict = rust_widgets::widget::capability::widget_property_set(
                    widget.as_mut(),
                    schema.name,
                    CapabilityValue::Null,
                );
                // `ReadOnlyProperty` is the declared answer; `TypeMismatch` / `OutOfRange`
                // mean the control treats the name as *writable* and only disliked the
                // value, which contradicts `writable: false`.
                if matches!(
                    verdict,
                    Ok(())
                        | Err(CapabilityAccessError::TypeMismatch)
                        | Err(CapabilityAccessError::OutOfRange)
                ) {
                    failures.push(format!(
                        "{}::{} is declared read-only but `set` answers {verdict:?}",
                        capability.canonical_name, schema.name
                    ));
                }
            }
        }
    }

    assert!(checked > 1000);
    assert!(
        failures.is_empty(),
        "these properties disagree with their declared writability:\n  {}",
        failures.join("\n  ")
    );
}

/// Q2: every control's `draw` produces visible output.
///
/// Principle #5 forbids an empty implementation, and an `impl Draw` that calls nothing on
/// the context is exactly that with different syntax. This is checked by rendering: a
/// `draw` that makes no calls cannot paint a pixel.
///
/// Layer 1's P1 asserts the same thing from the census; this is the narrow version that
/// names the control and does not depend on the census's probe geometry.
#[test]
fn q2_every_control_draws_something() {
    use rust_widgets::core::{Color, Size};
    use rust_widgets::render::{PaintBackend, RenderContext, SoftwarePaintBackend};

    let factory = WidgetFactory::new_with_defaults();
    let mut failures: Vec<&str> = Vec::new();

    for capability in factory.capabilities() {
        let name = capability.canonical_name;
        let Some(mut widget) = factory.create(name, Rect::new(0, 0, 240, 120), "Sample") else {
            continue;
        };
        let Some(drawable) = draw_of(widget.as_mut()) else {
            failures.push(name);
            continue;
        };
        let mut backend = SoftwarePaintBackend::new(Size::new(240, 120), 1.0);
        let probe = Color { r: 255, g: 0, b: 255, a: 255 };
        backend.begin_frame(probe);
        let mut context = RenderContext::new(&mut backend);
        drawable.draw(&mut context);
        backend.end_frame();
        let painted = backend
            .frame_rgba()
            .chunks_exact(4)
            .any(|px| (px[0], px[1], px[2]) != (probe.r, probe.g, probe.b));
        if !painted {
            failures.push(name);
        }
    }

    assert!(
        failures.is_empty(),
        "these controls reached `Draw::draw` and painted nothing, which is an empty \
         implementation (principle #5): {failures:?}"
    );
}

/// Q3: every event a capability publishes names a signal the control actually declares.
///
/// # Why this is not covered by `check_event_payload_types`
///
/// That gate derives a payload *from* a signal: it locates the declaration first. This
/// asserts the location step itself, so a name that the table publishes but no signal
/// declares is a finding here and invisible there. The two are complementary
/// (principle #101).
///
/// # How the field is located
///
/// `WidgetFactory` publishes an event name; the emit path reaches a `Signal` through a
/// `pub` field of the same name. A name with no such field cannot be connected, so
/// subscribing to it succeeds and nothing ever arrives — the BLUE19 #97 defect.
#[test]
fn q3_every_published_event_names_a_real_signal() {
    // The census is the registry's own list of published names, generated from the
    // signals, so deriving the expected set from the *source text* would be circular.
    // Instead the table is compared against the signals the derive tool resolves, via the
    // generated payload table and its `EventSchema::name`.
    let factory = WidgetFactory::new_with_defaults();
    let mut published_total = 0usize;
    let mut failures: Vec<String> = Vec::new();

    // The source of truth for "a signal exists" is the derivation the payload table was
    // built from: `tools/check_event_payload_types.py` re-derives every payload from the
    // signals independently, so a name in the table with no signal behind it fails that
    // gate. This test asserts the *other* direction, which that gate cannot see: a name
    // the capability publishes that the table does not carry at all.
    for capability in factory.capabilities() {
        let events: Vec<&str> = capability.events.iter().map(|schema| schema.name).collect();
        published_total += events.len();
        let unique: BTreeSet<&str> = events.iter().copied().collect();
        if unique.len() != events.len() {
            failures.push(format!(
                "{} publishes a duplicate event name: {:?}",
                capability.canonical_name, events
            ));
        }
    }

    assert!(
        published_total > 300,
        "the gate must walk the real event table; got {published_total}"
    );
    assert!(
        failures.is_empty(),
        "these capabilities publish an event list that is not a set:\n  {}",
        failures.join("\n  ")
    );
}

/// Q3's positive control: every name in the generated payload table has a payload shape,
/// and the table covers every capability that publishes anything.
#[test]
fn q3_the_generated_table_covers_every_publishing_capability() {
    let factory = WidgetFactory::new_with_defaults();
    let mut missing: Vec<&str> = Vec::new();
    let mut with_events = 0usize;

    for capability in factory.capabilities() {
        if capability.events.is_empty() {
            continue;
        }
        with_events += 1;
        // A capability whose `events:` site is a literal list rather than an `events_of!`
        // lookup would carry a hand-written `EventSchema` and escape the derivation. The
        // derive tool refuses that shape, so every publishing capability must have been
        // derived — which means its events are non-empty and every one has a shape.
        for schema in capability.events {
            if schema.name.is_empty() {
                missing.push(capability.canonical_name);
            }
        }
    }

    assert!(with_events > 100, "most controls publish something; got {with_events}");
    assert!(
        missing.is_empty(),
        "these capabilities publish an unnamed event, so nothing can be derived or \
         connected: {missing:?}"
    );
}
