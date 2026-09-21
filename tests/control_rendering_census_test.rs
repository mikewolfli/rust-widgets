// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! BLUE20 layer 1 — the rendering census gate, as assertions.
//!
//! # What this asserts, and why each one can fail
//!
//! The census renders **every** control the factory publishes, twice (light and
//! dark), and this test turns the measurements into four assertions:
//!
//! | # | Assertion | The defect it catches |
//! |---|---|---|
//! | P1 | the control painted ≥1 pixel ≠ its background | laid out off-canvas (round 58 defect A) |
//! | P2 | its dominant colour ≠ its background | painted, but in the window's colour (defect D) |
//! | P3 | light dominant ≠ dark dominant | chrome is hardcoded, so a theme switch does nothing (defect C) |
//! | P4 | each semantic token has a reader, and dark ≠ light | `theme.colors.{error,…}` declared and unread (§1.4) |
//!
//! # Why these are *relative* judgements
//!
//! Nothing here asserts a literal colour. "This control is `#2196f3`" is a fact
//! about today's theme; "this control's fill is not the colour behind it" is the
//! fact the user can see, and it stays true when the palette changes. Literals
//! live only in the baseline file, where they are diffed, not asserted.
//!
//! # Why the widget set is walked by name
//!
//! 13 `WidgetKind`s are shared by 2–5 controls each, so walking kinds would skip
//! 19 controls. The set comes from `WidgetFactory::widget_names()`, which is
//! derived from the same table the factory constructs from — it cannot under-count.
//!
//! # Known-invisible set
//!
//! 23 controls currently paint nothing at the census geometry. They are listed
//! explicitly in `KNOWN_INVISIBLE` so the count cannot quietly grow: a new
//! invisible control fails the test, and removing one from the list is the only
//! way to record a fix. The same shape is used for `KNOWN_THEME_BLIND`.

// The census needs the theme module and the full widget registry, which exist only on a
// **device** profile (`desktop`/`tablet`/`mobile`). `not(mini)` was too weak: `embedded` is
// also stripped of both, so the test failed to compile there with `cannot find theme in
// rust_widgets`. Naming the requirement directly is what keeps this true when a profile is
// added, rather than a `not(...)` list that has to be extended.
#![cfg(all(not(feature = "mini"), not(feature = "embedded"), not(target_arch = "wasm32")))]

use rust_widgets::theme::theme_test_guard;
use rust_widgets::widget::census::{
    census_all_controls, install_preset_appearances, ControlCensus,
};

/// Controls that paint **nothing** at the census geometry.
///
/// Each entry is a control whose `Draw` produces no pixel distinguishable from
/// the background. The list is asserted against the census in both directions:
/// a control here that *does* paint fails, and a control not here that paints
/// nothing fails. So this is a to-do list that can only shrink by fixing, never
/// by silent drift.
const KNOWN_INVISIBLE: &[&str] = &[];

/// Controls whose dominant colour is identical in light and dark, and are **not**
/// exempted as data colours.
///
/// These are hardcoded-chrome defects: a theme switch leaves them unchanged. The
/// list is asserted in both directions like [`KNOWN_INVISIBLE`].
const KNOWN_THEME_BLIND: &[&str] = &[];

/// Controls whose painted body is **defined** to be the surface they sit on.
///
/// Parsed from `tools/control_surface_coincidence_exemptions.txt`, so the reason for
/// each entry lives next to the exemption. Separate from the data-colour table
/// because it exempts a different judgement: that file excuses P3 (invariant across
/// appearances), this one excuses P2 (coincident with the surface).
fn surface_coincidence_exemptions() -> Vec<String> {
    let Ok(text) = std::fs::read_to_string("tools/control_surface_coincidence_exemptions.txt")
    else {
        return Vec::new();
    };
    text.lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .filter_map(|line| line.split_whitespace().next())
        .map(str::to_string)
        .collect()
}

/// Controls whose dominant colour may legitimately not change with appearance.
///
/// Parsed from `tools/control_color_exemptions.txt`, so the reason for each entry
/// lives next to the exemption rather than in this file. A control named in that
/// table is excused from P3; nothing else is.
fn data_color_exemptions() -> Vec<String> {
    let Ok(text) = std::fs::read_to_string("tools/control_color_exemptions.txt") else {
        return Vec::new();
    };
    text.lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .filter_map(|line| line.split_whitespace().next())
        .map(str::to_string)
        .collect()
}

/// Renders the whole registry once and returns it in canonical-name order.
fn census() -> Vec<ControlCensus> {
    let _guard = theme_test_guard();
    install_preset_appearances();
    census_all_controls()
}

#[test]
fn every_published_control_is_measured() {
    let rows = census();
    // The number is the registry's own, not a literal: if the registry grows, this
    // test measures the growth rather than failing on it.
    let expected = rust_widgets::widget::WidgetFactory::new_with_defaults().widget_names().len();
    assert_eq!(
        rows.len(),
        expected,
        "the census must cover every registered control, not a sample"
    );
    let mut names: Vec<&str> = rows.iter().map(|row| row.name).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), rows.len(), "each control must appear exactly once");
}

#[test]
fn p1_every_control_paints_something_unless_known_invisible() {
    let rows = census();
    let mut unexpected = Vec::new();
    for row in &rows {
        let invisible = !row.light.paints_anything();
        let known = KNOWN_INVISIBLE.contains(&row.name);
        if invisible && !known {
            unexpected.push(row.name);
        }
    }
    assert!(
        unexpected.is_empty(),
        "these controls painted nothing and are not recorded as known-invisible: {unexpected:?}"
    );

    // The reverse direction: a control recorded as invisible that now paints must
    // be removed from the list, or the list would keep claiming a defect is open.
    let mut fixed = Vec::new();
    for name in KNOWN_INVISIBLE {
        if let Some(row) = rows.iter().find(|row| row.name == *name) {
            if row.light.paints_anything() {
                fixed.push(*name);
            }
        }
    }
    assert!(
        fixed.is_empty(),
        "these controls now paint but are still listed as invisible — remove them: {fixed:?}"
    );
}

#[test]
fn p2_every_visible_control_differs_from_its_background() {
    let rows = census();
    let surface_exemptions = surface_coincidence_exemptions();
    let mut failures = Vec::new();
    let mut unexpectedly_exempt = Vec::new();
    for row in &rows {
        // P1 governs "did it paint at all" (probe render); P2 governs "can the user
        // see it where it sits" (surface render). A control that paints nothing is
        // P1's finding, so it is not double-reported here.
        if !row.light.paints_anything() {
            continue;
        }
        let exempt = surface_exemptions.iter().any(|name| name == row.name);
        let visible = row.visible_against_its_surface();
        if !visible && !exempt {
            failures.push(row.name);
        }
        // The reverse direction: an exemption that is no longer needed must be
        // removed, or the list would keep excusing a defect that has been fixed.
        if visible && exempt {
            unexpectedly_exempt.push(row.name);
        }
    }
    assert!(
        failures.is_empty(),
        "these controls paint, but only in the colour of the surface they sit on, \
         so the user cannot see them: {failures:?}"
    );
    assert!(
        unexpectedly_exempt.is_empty(),
        "these controls are visible against their surface and no longer need a \
         P2 exemption — remove them from \
         tools/control_surface_coincidence_exemptions.txt: {unexpectedly_exempt:?}"
    );
}

#[test]
fn p3_chrome_follows_the_appearance_unless_exempted_as_data() {
    let rows = census();
    let exemptions = data_color_exemptions();
    assert!(
        !exemptions.is_empty(),
        "the exemption table must be readable; an empty table would silently \
         exempt nothing and every data-coloured chart would fail"
    );

    // The four controls that carry *semantic* colours must never be exempted: if
    // they were, "the theme declares four tokens and nobody reads them" would be
    // permanently legalised.
    for forbidden in ["banner", "calendar", "progress_dialog", "message_box"] {
        assert!(
            !exemptions.iter().any(|name| name == forbidden),
            "{forbidden} carries a semantic colour and must not be data-exempt"
        );
    }

    let mut unexpected = Vec::new();
    for row in &rows {
        if exemptions.iter().any(|name| name == row.name) {
            continue;
        }
        if !row.differs_between_appearances() && !KNOWN_THEME_BLIND.contains(&row.name) {
            unexpected.push(row.name);
        }
    }
    assert!(
        unexpected.is_empty(),
        "these controls render identically in light and dark and are not recorded \
         as theme-blind nor exempted as data colours: {unexpected:?}"
    );
}

#[test]
fn p4_every_semantic_token_is_consumed_and_moves_with_the_appearance() {
    let rows = census();

    // A banner is the control that reads all four tokens, so its four severities
    // are the probe. Each must paint, and each must differ between appearances —
    // which is only true if the token it reads differs between appearances.
    let banner = rows
        .iter()
        .find(|row| row.name == "banner")
        .expect("the banner control must be registered");
    assert!(
        banner.light.paints_anything(),
        "the banner must paint, or the semantic tokens have no visible consumer"
    );
    assert!(
        banner.differs_between_appearances(),
        "the banner must change with the appearance, which proves it reads a token \
         rather than a literal"
    );

    // Each token must resolve to a colour under the active theme, and the light and
    // dark presets must give different colours — otherwise "dark ≠ light" for a
    // control reading it would be impossible.
    let _guard = theme_test_guard();
    install_preset_appearances();
    let mut manager = rust_widgets::theme::global_theme_manager();
    manager.set_appearance(rust_widgets::theme::AppearanceMode::Light);
    let light: Vec<_> = rust_widgets::theme::SemanticColor::ALL
        .iter()
        .map(|token| token.of(manager.current_theme().expect("a theme is active")))
        .collect();
    manager.set_appearance(rust_widgets::theme::AppearanceMode::Dark);
    let dark: Vec<_> = rust_widgets::theme::SemanticColor::ALL
        .iter()
        .map(|token| token.of(manager.current_theme().expect("a theme is active")))
        .collect();

    for (index, token) in rust_widgets::theme::SemanticColor::ALL.iter().enumerate() {
        assert_ne!(
            light[index],
            dark[index],
            "semantic token `{}` resolves to the same colour in light and dark, so a \
             control reading it could not respond to a theme switch",
            token.token()
        );
    }
}

#[test]
fn known_theme_blind_matches_the_census() {
    let rows = census();
    let exemptions = data_color_exemptions();

    let mut fixed = Vec::new();
    for name in KNOWN_THEME_BLIND {
        let Some(row) = rows.iter().find(|row| row.name == *name) else {
            continue;
        };
        if row.differs_between_appearances() {
            fixed.push(*name);
        }
    }
    assert!(
        fixed.is_empty(),
        "these controls now follow the appearance but are still listed as \
         theme-blind — remove them: {fixed:?}"
    );

    // And the reverse: a control that stopped following the appearance without
    // being recorded fails, so a regression cannot hide behind the list.
    let mut regressed = Vec::new();
    for row in &rows {
        if exemptions.iter().any(|name| name == row.name) || KNOWN_THEME_BLIND.contains(&row.name) {
            continue;
        }
        if row.light.paints_anything() && !row.differs_between_appearances() {
            regressed.push(row.name);
        }
    }
    assert!(regressed.is_empty(), "these controls stopped following the appearance: {regressed:?}");
}
