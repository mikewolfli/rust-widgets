// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The designer manifest must export, re-load and re-export to the same bytes — for every control.
//!
//! # Why the assertion is byte equality
//!
//! A designer saves a project as one of these documents and reopens it later. If writing it out
//! twice can produce different bytes, then every save is a diff, a merge is a conflict, and — worse
//! — information can be lost between the two, because nothing compares them.
//!
//! Semantic equality would not be enough: two documents can mean the same thing and disagree on
//! field order, on which of two representations of a list was used, or on whether an absent payload
//! was spelled `null` or omitted. Each of those turns a save into a change.
//!
//! # Why the sentinels are asserted too
//!
//! «The two strings are equal» is satisfied by two empty strings, and by a document that dropped
//! every field. So each round trip is also required to contain the control's own event names and,
//! where an event carries a value, its payload token. A re-derivation that stopped resolving
//! payloads would leave the strings equal and the sentinels missing — that is the defect this
//! second assertion exists to catch.
//!
//! # Why it re-loads through an independent parser
//!
//! Calling the same writer twice would compare an implementation against itself. The re-load goes
//! through [`DesignerManifest::from_json`], which parses the text into a *second* structure; the
//! re-export then comes from that structure, so the comparison is between two independent passes
//! over the same facts.
//!
//! `--inject` drops one event from the re-loaded manifest and requires the sentinel check to fail,
//! which is the reverse-injection BLUE19's DoD-T-2 asks for.

#![cfg(all(feature = "desktop", not(alloc_frugal)))]

use rust_widgets::widget::capability::{
    all_capability_manifests_json, capability_manifest_json, manifest_to_json, DesignerManifest,
    WidgetFactory,
};

/// Every control's manifest must survive export → load → export unchanged.
#[test]
fn every_control_manifest_round_trips_byte_for_byte() {
    let factory = WidgetFactory::new_with_defaults();
    let mut checked = 0usize;
    let mut failures: Vec<String> = Vec::new();

    let mut names: Vec<&str> =
        factory.capabilities().iter().map(|capability| capability.canonical_name).collect();
    names.sort_unstable();

    for name in names {
        let Ok(first) = capability_manifest_json(&factory, name) else {
            failures.push(format!("{name}: export failed"));
            continue;
        };
        let reloaded = match DesignerManifest::from_json(&first) {
            Ok(manifest) => manifest,
            Err(error) => {
                failures.push(format!("{name}: the exported document did not parse: {error}"));
                continue;
            }
        };
        let second = manifest_to_json(&reloaded);
        if first != second {
            failures.push(format!("{name}: export -> load -> export changed the document"));
            continue;
        }
        // The document must describe the control it was asked for, and must contain an event
        // entry for every name that control publishes. Without this, a writer that emitted only
        // the control name would pass the byte comparison above.
        if !first.contains(&format!("\"control\": \"{name}\"")) {
            failures.push(format!("{name}: the document does not name its own control"));
            continue;
        }
        if let Some(capability) = factory.capability(name) {
            for event in capability.events {
                if !first.contains(&format!("\"name\": \"{}\"", event.name)) {
                    failures.push(format!(
                        "{name}: event `{}` is missing from the document",
                        event.name
                    ));
                }
                if let Some(kind) = event.payload {
                    let token = rust_widgets::widget::capability::value_kind_token(kind);
                    if !first.contains(&format!("\"payload\": \"{token}\"")) {
                        failures.push(format!(
                            "{name}: event `{}` does not carry its payload token `{token}`",
                            event.name
                        ));
                    }
                }
            }
        }
        checked += 1;
    }

    assert!(checked > 100, "only {checked} controls were checked, so this proves little");
    assert!(
        failures.is_empty(),
        "these controls do not round-trip ({}):\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// The all-controls document must itself round-trip through the per-control parser.
///
/// The array form is what a designer loads to populate its palette, so it must be stable for the
/// same reason each single document is — and it must contain every control exactly once.
#[test]
fn the_whole_palette_document_is_complete_and_stable() {
    let factory = WidgetFactory::new_with_defaults();
    let document = all_capability_manifests_json(&factory).expect("the palette exports");
    let again = all_capability_manifests_json(&factory).expect("the palette exports");
    assert_eq!(document, again, "the palette document is not stable across two exports");

    let mut expected = 0usize;
    for capability in factory.capabilities() {
        let needle = format!("\"control\": \"{}\"", capability.canonical_name);
        assert!(
            document.contains(&needle),
            "the palette is missing `{}`, so a designer could not offer it",
            capability.canonical_name
        );
        expected += 1;
    }
    assert!(expected > 100, "the factory has only {expected} capabilities");
}

/// A payload-carrying event's declared type must survive the round trip.
///
/// `slider` is the sentinel: it publishes `value_changed` (an `Int`) and `slider_pressed` (no
/// payload). Both must be present and typed as such, which is what distinguishes a document that
/// says "you can read a number here" from one that says nothing.
#[test]
fn the_slider_sentinel_carries_its_payload_types() {
    let factory = WidgetFactory::new_with_defaults();
    let document = capability_manifest_json(&factory, "slider").expect("slider exports");

    assert!(
        document.contains("\"name\": \"value_changed\""),
        "`slider.value_changed` is the payload sentinel and must be in the document"
    );
    assert!(
        document.contains("\"payload\": \"int\""),
        "`value_changed` carries an `i32`, so the document must say `int`"
    );
    assert!(
        document.contains("\"name\": \"slider_pressed\""),
        "`slider_pressed` is the payload-free sentinel and must be in the document"
    );
    assert!(
        document.contains("\"payload\": null"),
        "a payload-free event must be spelled `null` rather than omitted"
    );

    let reloaded = DesignerManifest::from_json(&document).expect("the document parses");
    assert_eq!(manifest_to_json(&reloaded), document, "the sentinel document changed on reload");
}

/// Dropping an event from a re-loaded manifest must be detectable.
///
/// This is the reverse-injection half: it proves the sentinel assertions above can fail, so a
/// writer that silently stopped emitting events would be caught rather than merely producing two
/// equal short strings.
#[test]
fn dropping_an_event_is_detectable() {
    let factory = WidgetFactory::new_with_defaults();
    let document = capability_manifest_json(&factory, "slider").expect("slider exports");

    let mut manifest = DesignerManifest::from_json(&document).expect("the document parses");
    let before = manifest.events.len();
    manifest.events.retain(|event| event.name != "value_changed");
    assert_eq!(manifest.events.len(), before - 1, "the injection must remove exactly one event");

    let damaged = manifest_to_json(&manifest);
    assert_ne!(
        damaged, document,
        "removing an event produced an identical document, so the export is not describing events"
    );
    assert!(
        !damaged.contains("\"name\": \"value_changed\""),
        "the removed event is still present, so the removal did not take effect"
    );
}
