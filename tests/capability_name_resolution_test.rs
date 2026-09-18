// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Every registered control name must address the same control from **all three**
//! directions the registry answers.
//!
//! # The three answers
//!
//! A control has three independent statements about what it is, and nothing in the
//! library ties them together:
//!
//! 1. **The capability** — `WidgetFactory::capability(name).kind`, what the name means.
//! 2. **The live control** — `widget.kind()`, what the mounted object reports.
//! 3. **The resolved capability** — `capability_for_kind_instance(widget).kind`, the
//!    property/event path's answer for a live control.
//!
//! Round 31 fixed direction 3 going wrong for the *wrong control* (a kind resolving
//! through registration order). This test covers the other failure: a capability that
//! declares a kind no widget ever reports. The library has candidates for exactly
//! that — `divider_capability` declares `WidgetKind::Divider` while `Divider` is
//! three lines of state over a `Line` drawing primitive, `range_slider_capability`
//! declares `RangeSlider`, `sparkline_capability` declares `Sparkline` — so the three
//! answers must be compared rather than assumed equal.
//!
//! # Why the assertion is a comparison and not a read
//!
//! Reading a property through `factory.read_property` answers from whichever
//! capability resolved, so it cannot detect a resolution that picked a sibling; it
//! would compare a source against itself. Every check below therefore compares two
//! *different* sources.

#![cfg(all(feature = "desktop", not(alloc_frugal)))]

use rust_widgets::core::Rect;
use rust_widgets::widget::capability::WidgetFactory;

/// Every registered name must address the same control from both directions.
#[test]
fn every_registered_name_resolves_to_a_capability_in_both_directions() {
    let factory = WidgetFactory::new_with_defaults();

    let mut unconstructible: Vec<&str> = Vec::new();
    let mut unresolvable: Vec<(&str, &str)> = Vec::new();
    let mut kind_disagreement: Vec<(&str, &str, String)> = Vec::new();

    for capability in factory.capabilities() {
        let canonical = capability.canonical_name;
        let declared_kind = format!("{:?}", capability.kind);

        // Canonical name plus every alias it publishes.
        let names = core::iter::once(canonical).chain(capability.aliases.iter().copied());

        for name in names {
            let Some(widget) = factory.create(name, Rect::new(0, 0, 64, 48), "") else {
                // Not constructible by this spelling: the alias addresses nothing.
                unconstructible.push(name);
                continue;
            };

            // Direction 2: what the mounted object itself reports.
            let reported_kind = format!("{:?}", widget.kind());

            // Direction 3: the property path's answer for that live control.
            let resolved = factory.capability_for_kind_instance(widget.as_ref());
            let Some(resolved) = resolved else {
                unresolvable.push((name, canonical));
                continue;
            };

            // Direction 1: what the *name* means to the factory's lookup.
            let Some(by_name) = factory.capability(name) else {
                unconstructible.push(name);
                continue;
            };

            if format!("{:?}", by_name.kind) != declared_kind
                || format!("{:?}", resolved.kind) != declared_kind
                || reported_kind != declared_kind
            {
                kind_disagreement.push((
                    name,
                    canonical,
                    format!(
                        "widget.kind={reported_kind} by_name={:?} resolved={:?}",
                        by_name.kind, resolved.kind
                    ),
                ));
            }
        }
    }

    assert!(
        unconstructible.is_empty(),
        "these registered names cannot be constructed, so the alias addresses nothing: \
         {unconstructible:?}"
    );
    assert!(
        unresolvable.is_empty(),
        "these registered names construct a control the capability layer cannot address, so \
         its own properties report UnknownWidget (name, canonical): {unresolvable:?}"
    );
    assert!(
        kind_disagreement.is_empty(),
        "a name's creation path, its mounted control and its property path disagree about the \
         kind, so the declarative layer, the CSS selector and the accessibility role answer \
         differently for one control (name, canonical, all three answers): {kind_disagreement:?}"
    );
}

/// The capability a live control resolves to must be the one that describes it.
///
/// # Why this compares two sources rather than reading through one
///
/// `read_property` answers from the resolved capability, so it can only ever be
/// self-consistent. The question here is whether the capability the registry hands
/// back is the one whose published property list the **control itself** matches:
/// a live control's `properties_dyn()` is its own contract, independent of any
/// registry entry, so comparing the two catches a resolution that selected a
/// sibling's schema.
#[test]
fn every_resolved_capability_matches_the_contract_the_control_publishes() {
    let factory = WidgetFactory::new_with_defaults();

    let mut missing: Vec<(&str, &str)> = Vec::new();
    let mut undeclared: Vec<(&str, &str)> = Vec::new();

    for capability in factory.capabilities() {
        let Some(widget) = factory.create(capability.canonical_name, Rect::new(0, 0, 64, 48), "")
        else {
            continue;
        };
        let Some(resolved) = factory.capability_for_kind_instance(widget.as_ref()) else {
            // Reported by the test above.
            continue;
        };
        let Some(published) = widget.properties_dyn().map(|props| props.property_names()) else {
            continue;
        };

        // Forward: every name the control publishes is declared by the resolved
        // capability, or is one of the four properties every control shares
        // (`enabled` / `visible` / `tooltip` / `geometry`) that the base contract
        // answers without a per-control schema entry.
        for &name in published {
            let declared = resolved.properties.iter().any(|schema| schema.name == name);
            let is_base = matches!(name, "enabled" | "visible" | "tooltip" | "geometry");
            if !declared && !is_base {
                missing.push((resolved.canonical_name, name));
            }
        }

        // Reverse: every readable schema entry the resolved capability declares is
        // answered by the control. This is the direction that catches a schema
        // promising a name nothing implements.
        for schema in resolved.properties.iter() {
            if !schema.readable && !schema.writable {
                continue;
            }
            if !published.contains(&schema.name) {
                undeclared.push((resolved.canonical_name, schema.name));
            }
        }
    }

    assert!(
        missing.is_empty(),
        "the resolved capability does not declare properties the control publishes, so the \
         registry would reject a name the control answers (control, property): {missing:?}"
    );
    assert!(
        undeclared.is_empty(),
        "the resolved capability declares properties the control does not answer, so the \
         registry advertises names that resolve to UnknownProperty (control, property): \
         {undeclared:?}"
    );
}
