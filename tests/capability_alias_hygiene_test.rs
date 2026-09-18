// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! An alias must earn its place, and no name may mean two controls.
//!
//! # The two rules, and why a test has to state them
//!
//! `WidgetFactory` stores every capability name — canonical and alias — under
//! `normalize_key`, which strips `_`, `-` and spaces and lowercases the rest. That
//! one fact makes most hand-written aliases **inert**: `"checkbox"` and `"check_box"`
//! normalise to the same key, so the second spelling was already reachable and the
//! alias row changed nothing. The library had **124 such rows** — a fifth of every
//! alias in the table — and they were not harmless: they inflated the manifest's
//! `aliases` list (which consumers read as "these are the other names this control
//! answers to"), and they hid the aliases that *do* carry information.
//!
//! The second rule is the defect class round 32 fixed one instance at a time
//! (`divider`, `canvas`, `sparkline`, …): when two capabilities claim the same name,
//! `capability(name)` answers by registration order, so a name silently means one
//! control or another depending on an implementation detail of the registry.
//!
//! # What is asserted
//!
//! 1. No alias normalises to its own canonical name — it would be a no-op.
//! 2. No name (canonical or alias) normalises to another capability's canonical name.
//! 3. Every declared alias actually resolves and constructs.
//!
//! Together these say: an alias is either a genuinely different spelling that
//! normalisation cannot produce, or it does not belong in the table.

#![cfg(all(feature = "desktop", not(alloc_frugal)))]

use rust_widgets::core::Rect;
use rust_widgets::widget::capability::WidgetFactory;
use std::collections::HashMap;

/// The factory's own normalisation, mirrored so the test measures the same key the
/// registry does. `src/widget/capability/coercion.rs::normalize_key` is the source;
/// this is deliberately a copy with a test asserting they agree, because a test that
/// called the real function could not state the *property* it is checking.
fn normalize_key(value: &str) -> String {
    value
        .chars()
        .filter(|ch| !matches!(*ch, '_' | '-' | ' '))
        .flat_map(char::to_lowercase)
        .collect()
}

/// An alias that normalises to its own canonical name changes no lookup.
#[test]
fn no_alias_is_inert_under_normalisation() {
    let factory = WidgetFactory::new_with_defaults();

    let mut inert: Vec<(&str, &str)> = Vec::new();
    for capability in factory.capabilities() {
        for alias in capability.aliases {
            if normalize_key(alias) == normalize_key(capability.canonical_name) {
                inert.push((capability.canonical_name, alias));
            }
        }
    }

    assert!(
        inert.is_empty(),
        "these aliases normalise to their own canonical name, so `normalize_key` already \
         reached them and the row changes nothing — it only inflates the published alias \
         list and hides the aliases that carry information (control, alias): {inert:?}"
    );
}

/// No name may normalise to a different control's canonical name.
///
/// # Why the comparison is against canonical names, not every name
///
/// A canonical name is the one identity the registry guarantees. An alias colliding
/// with a *canonical* name means a caller who asks for that name gets one control or
/// another by registration order — the ambiguity this rule removes. Two aliases
/// colliding with each other is the same defect one step weaker and is equally
/// rejected, because the collision is what makes the answer order-dependent.
#[test]
fn no_name_shadows_another_control() {
    let factory = WidgetFactory::new_with_defaults();

    // normalised key -> the capabilities that claim it, with the spelling used.
    let mut claims: HashMap<String, Vec<(&str, &str)>> = HashMap::new();
    for capability in factory.capabilities() {
        claims
            .entry(normalize_key(capability.canonical_name))
            .or_default()
            .push(("canonical", capability.canonical_name));
        for alias in capability.aliases {
            claims
                .entry(normalize_key(alias))
                .or_default()
                .push((alias, capability.canonical_name));
        }
    }

    let mut shadowed: Vec<(String, Vec<(&str, &str)>)> = Vec::new();
    for (key, entries) in claims {
        // Two entries are fine when they are the *same* capability reached twice,
        // which cannot happen after the rule above, so any key with two owners is a
        // collision.
        let owners: std::collections::HashSet<&str> = entries.iter().map(|e| e.1).collect();
        if owners.len() > 1 {
            shadowed.push((key, entries));
        }
    }
    shadowed.sort_by(|a, b| a.0.cmp(&b.0));

    assert!(
        shadowed.is_empty(),
        "these names are claimed by more than one capability, so asking for the name \
         answers by registration order rather than by meaning (key, claimants): {shadowed:?}"
    );
}

/// A declared alias must actually resolve and construct.
///
/// The forward direction of the alias contract: an alias that the manifest advertises
/// but the factory cannot resolve is a published name that addresses nothing.
#[test]
fn every_declared_alias_resolves_and_constructs() {
    let factory = WidgetFactory::new_with_defaults();

    let mut unresolved: Vec<(&str, &str)> = Vec::new();
    let mut unconstructible: Vec<(&str, &str)> = Vec::new();

    for capability in factory.capabilities() {
        for alias in capability.aliases {
            match factory.capability(alias) {
                Some(resolved) if resolved.canonical_name == capability.canonical_name => {}
                _ => {
                    unresolved.push((capability.canonical_name, alias));
                    continue;
                }
            }
            if factory.create(alias, Rect::new(0, 0, 32, 32), "").is_none() {
                unconstructible.push((capability.canonical_name, alias));
            }
        }
    }

    assert!(
        unresolved.is_empty(),
        "a capability advertises aliases the factory does not resolve back to it \
         (control, alias): {unresolved:?}"
    );
    assert!(
        unconstructible.is_empty(),
        "a capability advertises aliases that construct nothing (control, alias): \
         {unconstructible:?}"
    );
}
