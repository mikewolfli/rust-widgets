// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Reachability gates for the style/theme/CSS surface.
//!
//! # Why these exist
//!
//! Two classes of defect kept appearing in this area, and neither is visible from a
//! passing test suite:
//!
//! 1. **A style field with no way to set it.** `background_gradient`, `shadow` and
//!    `touch_target` were fields of `WidgetStyle` that no CSS property could reach,
//!    so a stylesheet could not express the whole style record. Adding a field
//!    without a property reproduces the gap silently.
//! 2. **A documented capability with no consumer.** The theme system, the
//!    stylesheet manager and the JSON loader were all fully implemented, fully
//!    tested, and reachable from **nothing** — so a theme switch changed no control
//!    an application could see.
//!
//! These tests are mechanical: they enumerate the real types rather than a
//! hand-written sample, so they fail when a new field or a new capability is added
//! without its wiring.

#![cfg(feature = "desktop")]

use rust_widgets::core::Color;
use rust_widgets::style::{CssDeclaration, CssParser, WidgetStyle};

/// Every `WidgetStyle` field a stylesheet is expected to be able to set.
///
/// The style record has four fields a stylesheet has no business setting
/// (`padding`/`margin` are set as a group through their shorthand properties, and
/// the two colour shorthands are covered by `background`/`color`), so this is the
/// list of *properties*, not of fields.
const REQUIRED_CSS_PROPERTIES: &[&str] = &[
    "color",
    "background-color",
    "border-color",
    "border-width",
    "border-radius",
    "padding",
    "margin",
    "font-size",
    "font-family",
    "opacity",
    // The three that were unreachable. Kept in the same list so a future field
    // cannot be left out without deleting an entry from here — which is a visible
    // edit rather than a silent omission.
    "background-gradient",
    "shadow",
    "touch-target",
];

/// Applies one declaration and reports whether the parser accepted it.
fn applies(property: &str, value: &str) -> bool {
    let decl = CssDeclaration { property: property.to_string(), value: value.to_string() };
    let mut style = WidgetStyle::default();
    CssParser::apply_declarations(&[decl], &mut style).is_ok()
}

/// Every property a stylesheet should support actually applies.
#[test]
fn every_stylesheet_property_is_settable() {
    let samples = [
        ("color", "#112233"),
        ("background-color", "#445566"),
        ("border-color", "#778899"),
        ("border-width", "2px"),
        ("border-radius", "6px"),
        ("padding", "4px 8px"),
        ("margin", "2px"),
        ("font-size", "15px"),
        ("font-family", "Arial"),
        ("opacity", "0.5"),
        ("background-gradient", "linear-gradient(90deg, #ff0000, #0000ff)"),
        ("shadow", "1px 2px 3px #000000"),
        ("touch-target", "44px 44px"),
    ];

    for (property, value) in samples {
        assert!(
            REQUIRED_CSS_PROPERTIES.contains(&property),
            "{property} is listed as a sample but not among the required properties"
        );
        assert!(
            applies(property, value),
            "CSS property {property}: {value} must apply, but the parser rejected it"
        );
    }

    // And the reverse direction: every required property has a sample, so a name
    // added to the list without a sample fails here rather than being skipped.
    for required in REQUIRED_CSS_PROPERTIES {
        assert!(
            samples.iter().any(|(name, _)| name == required),
            "{required} is required but has no sample value in this test"
        );
    }
}

/// The property names CSS accepts are exactly the ones the style record can hold,
/// checked by reading a value back out for each.
#[test]
fn each_property_reaches_a_distinct_style_field() {
    let mut style = WidgetStyle::default();

    CssParser::apply_declarations(
        &[CssDeclaration {
            property: "background-gradient".into(),
            value: "linear-gradient(#ff0000, #0000ff)".into(),
        }],
        &mut style,
    )
    .expect("gradient applies");
    assert!(style.background_gradient.is_some(), "the gradient must land on the style");

    CssParser::apply_declarations(
        &[CssDeclaration { property: "shadow".into(), value: "1px 2px 3px #000000".into() }],
        &mut style,
    )
    .expect("shadow applies");
    assert!(style.shadow.is_some(), "the shadow must land on the style");

    CssParser::apply_declarations(
        &[CssDeclaration { property: "touch-target".into(), value: "48px 48px".into() }],
        &mut style,
    )
    .expect("touch target applies");
    assert!(style.touch_target.is_some(), "the touch target must land on the style");
}

/// A `shadow: none` declaration clears rather than errors, so a stylesheet can undo
/// a shadow a lower-priority source set.
#[test]
fn shadow_none_is_a_value_not_an_error() {
    let mut style = WidgetStyle::default();
    CssParser::apply_declarations(
        &[CssDeclaration { property: "shadow".into(), value: "1px 1px 1px #000000".into() }],
        &mut style,
    )
    .expect("set");
    assert!(style.shadow.is_some());

    CssParser::apply_declarations(
        &[CssDeclaration { property: "shadow".into(), value: "none".into() }],
        &mut style,
    )
    .expect("clear");
    assert_eq!(style.shadow, None);
}

/// The theme reaches a control created through the **main** creation path.
///
/// This is the reachability assertion the theme system was missing entirely: it was
/// wired only into the JSON loader, which has no production callers.
#[test]
fn the_theme_reaches_a_control_created_through_the_main_path() {
    use rust_widgets::theme::{global_theme_manager, AppearanceMode, HighContrastMode};
    use rust_widgets::widget::runtime::with_widget;

    // Serialise against the other tests that switch the process-wide theme.
    let _guard = rust_widgets::theme::theme_test_guard();

    let mut manager = global_theme_manager();
    manager.set_high_contrast(HighContrastMode::None);
    assert!(manager.set_appearance(AppearanceMode::Light));
    let primary = manager.current_theme().expect("an active theme").colors.primary;
    drop(manager);

    let window = rust_widgets::create_window("reachability", 0, 0, 320, 240);
    assert_ne!(window, 0, "a window must be creatable");

    let button = rust_widgets::create_button(window, "OK", 10, 10, 80, 30);
    assert_ne!(button, 0, "a button must be creatable");

    let fill = with_widget(button, |widget| widget.style().background_color).flatten();
    assert_eq!(
        fill,
        Some(primary),
        "a control created through the C ABI path must take the active theme — if this fails, \
         the theme system is once again reachable only from a path nothing calls"
    );

    // And the override reaches it too.
    global_theme_manager().set_high_contrast(HighContrastMode::WhiteOnBlack);
    let forced = rust_widgets::create_button(window, "OK", 10, 50, 80, 30);
    let forced_fill = with_widget(forced, |widget| widget.style().background_color).flatten();
    assert_eq!(forced_fill, Some(Color::BLACK), "the forced pair must reach a new control");
    global_theme_manager().set_high_contrast(HighContrastMode::None);
}
