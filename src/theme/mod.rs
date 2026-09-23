// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Theme system and runtime switching.
//!
//! # Where the theme takes effect
//!
//! There are two levels:
//!
//! - `ThemeManager` is an ordinary value: a registry of themes and a selector
//!   over them. A caller may own one for an isolated preview.
//! - `global_theme_manager` is the process-wide registry that makes a theme
//!   *apply*.
//!
//! # How a control picks the theme up
//!
//! Every widget created through the library goes through one of two funnels —
//! `crate::mount_widget_object` (used by the C ABI and the window API) and
//! `CustomPaintControlBackend::mount_widget_of_kind` — and both call
//! `apply::apply_active_theme`, so a control is themed whichever way it was created.
//! The JSON loader additionally merges `resolved_theme_style` per node so a node's
//! `class` can select a role.
//!
//! Precedence everywhere: **explicit style → theme → the widget's own default**.
//! `crate::style::global_stylesheet_manager` layers CSS on top of that, giving
//! theme → stylesheet → explicit style for the declarative path.
//!
//! Applying the theme only in the JSON loader -- which has no production callers --
//! meant a theme switch did nothing for the controls an application actually
//! creates through the C ABI.
//!
//! # Reachability
//!
//! **State:** Exposed over the C ABI (`rw_set_theme`, `rw_theme_names`, `rw_set_high_contrast`). Also applied automatically by both creation funnels.
// The theme **types** are plain data — a palette, an appearance selector, a role table and
// a token enum — with no dependency on the manager, the registry or the platform. The
// widget layer names them (`active.colors.background`, `SemanticColor::Warning`) in builds
// that have a full widget set and no device profile, so the module is compiled wherever a
// colour can be resolved at all. `mini` is the exception: no platform singleton, no palette.
//
// The explicit re-export list at the bottom of this file is the module's public surface;
// it is written out rather than globbed so each name has one documented home.
mod apply;
pub(crate) use apply::apply_active_theme;
mod manager;
mod types;

/// Applies the active theme's style for `widget` to `widget`, as the creation funnels do.
///
/// # Why this is public
///
/// [`apply_active_theme`](crate::theme) is crate-private because the funnels are the only
/// *production* callers, but a tool that renders a control **outside** a funnel needs the
/// same styling or it renders something no user will ever see. The BLUE20 layer 3 SVG
/// exporter was exactly that case: it rendered every control with the theme never applied,
/// so `<name>.svg` and `<name>.light.svg` came out byte-identical apart from their comment
/// — the snapshots existed, looked right in a file listing, and proved nothing.
///
/// This is a single entry point rather than a second implementation: it resolves the
/// style through the same `manager.resolve_style` the funnels use and merges it the same
/// way, so a caller cannot accidentally get a different palette from a control created the
/// normal way. It is `#[cfg]`-gated the same way `apply_active_theme` is, so a profile
/// without a theme module reports `false` (nothing was applied) instead of pretending.
///
/// Returns `true` when a style was applied, `false` when there is no active theme or the
/// build has no theme module — the honest answer in both cases (principle #37: a missing
/// capability is reported, never faked).
///
/// # Gating
///
/// `not(alloc_frugal)` alone is **not** enough. `alloc_frugal` is about the allocator, and
/// it is false for `mini`/`embedded` — but those profiles have no colour model at all, so
/// `crate::theme` does not exist and naming it fails to compile. The condition that
/// actually matches the module is `full_widgets`, which is what `apply_active_theme` itself
/// is gated on and what `build.rs` defines for "a device profile *and* a full widget set".
/// Copying the neighbouring `#[cfg]` without checking it against the module is exactly the
/// drift principle #47 exists to prevent.
#[cfg(full_widgets)]
pub fn apply_theme_to_widget(widget: &mut dyn crate::widget::Widget) -> bool {
    // `apply_active_theme` already handles "no active theme" by returning without styling,
    // so the emptiness check is only to make the return value truthful.
    if global_theme_manager().current_theme().is_none() {
        return false;
    }
    apply_active_theme(widget);
    true
}

/// The no-theme arm, for the profiles that compile this module out.
///
/// A separate definition rather than a `cfg` inside one body: the module is not compiled in
/// these profiles, so a caller in the widget layer can still link against the name and get
/// the truthful "nothing was applied" answer instead of an unresolved symbol.
#[cfg(not(full_widgets))]
pub fn apply_theme_to_widget(widget: &mut dyn crate::widget::Widget) -> bool {
    let _ = widget;
    false
}

/// The high-contrast override that [`resolved_theme_style`] honours.
///
/// Re-exported from `crate::style`, where it is defined next to the
/// interaction-state model it interacts with, so a theme consumer has one import
/// path instead of two.
pub use crate::style::HighContrastMode;
/// Serialises tests that switch the process-wide theme; see its own docs.
///
/// Exported (not `#[cfg(test)]`) because integration tests live in **separate
/// crates** and therefore cannot see a crate-test-only item. Those tests exercise
/// the same process-wide registry, so they need the same guard — hiding it behind
/// `cfg(test)` left them with no way to serialise at all, which is how one test's
/// theme leaked into another's assertion.
///
/// Not intended for production use: an application does not have "other tests" to
/// race against, and holding this would only couple unrelated code.
pub use manager::theme_test_guard;
pub use manager::{
    global_high_contrast, global_theme_manager, resolved_theme_style, resolved_theme_style_for,
    resolved_theme_style_for_state, semantic_color, set_global_high_contrast, SemanticColor,
    ThemeManager,
};
pub use types::{
    AppearanceMode, Borders, Colors, Fonts, Motion, ShadowOverride, ShadowToken, Spacing, Theme,
    ThemeOverrides, ThemeStyleToken, WidgetRole,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Color;
    use crate::style::WidgetState;

    /// Builds a theme whose override table holds exactly the given entries.
    ///
    /// A helper rather than a literal in each test: assigning `theme.overrides`
    /// after `Theme::default()` trips `clippy::field_reassign_with_default`, and a
    /// constructor argument would change the public API for a test's convenience.
    fn theme_with_overrides(entries: &[(&str, ThemeStyleToken)]) -> Theme {
        Theme {
            overrides: ThemeOverrides {
                styles: entries
                    .iter()
                    .map(|(name, token)| ((*name).to_string(), token.clone()))
                    .collect(),
            },
            ..Theme::default()
        }
    }

    #[test]
    fn theme_default() {
        let manager = ThemeManager::default();
        let theme = manager.current_theme();
        assert!(theme.is_some());
        assert_eq!(theme.unwrap().name, "default");
    }

    #[test]
    fn theme_dark_exists() {
        let dark = Theme::dark();
        assert_eq!(dark.name, "dark");
        // Background should be very dark (near black)
        assert!(dark.colors.background.r < 30);
        assert!(dark.colors.background.g < 30);
        assert!(dark.colors.background.b < 30);
        // Foreground should be very light (near white)
        assert!(dark.colors.foreground.r > 200);
        assert!(dark.colors.foreground.g > 200);
        assert!(dark.colors.foreground.b > 200);
    }

    /// Each preset declares its own appearance, so the light/dark switch can find
    /// it without guessing from the background colour.
    #[test]
    fn presets_declare_their_appearance() {
        assert_eq!(Theme::default().appearance, AppearanceMode::Light);
        assert_eq!(Theme::dark().appearance, AppearanceMode::Dark);
    }

    // ── Role classification ───────────────────────────────────────────────

    /// The role table classifies the controls the resolver is expected to style,
    /// and folds separators/case the way the rest of the crate does.
    #[test]
    fn roles_are_classified_by_kind_name() {
        assert_eq!(WidgetRole::for_kind_name("Button"), WidgetRole::Primary);
        assert_eq!(WidgetRole::for_kind_name("button"), WidgetRole::Primary);
        assert_eq!(WidgetRole::for_kind_name("LineEdit"), WidgetRole::Input);
        assert_eq!(WidgetRole::for_kind_name("line_edit"), WidgetRole::Input);
        assert_eq!(WidgetRole::for_kind_name("CheckBox"), WidgetRole::Choice);
        assert_eq!(WidgetRole::for_kind_name("ProgressBar"), WidgetRole::Accent);
        assert_eq!(WidgetRole::for_kind_name("Label"), WidgetRole::Text);
    }

    /// A name the table does not know falls back to `Surface`, which is the same
    /// treatment it received from the previous `_` arm — so the classification is
    /// a superset of the old behaviour, not a change.
    #[test]
    fn unknown_kind_names_fall_back_to_surface() {
        assert_eq!(WidgetRole::for_kind_name("SomeThirdPartyWidget"), WidgetRole::Surface);
        assert_eq!(WidgetRole::for_kind_name(""), WidgetRole::Surface);
    }

    /// A selectable field is an `Input`, not a bare `Surface`.
    ///
    /// # The defect this pins
    ///
    /// `list_box`, `list_view`, `table_view`, `tree_view`, `scroll_area`,
    /// `text_browser` and `plaintext_edit` all fell through to the `_` arm, which maps to
    /// [`WidgetRole::Surface`]. `Surface` resolves to `theme.colors.background` — the very
    /// colour a window paints — so each of those controls was filled with the window's own
    /// background and had **no visible extent**: geometrically correct, styled "correctly",
    /// and invisible on screen.
    ///
    /// The assertion is stated as "differs from the window" rather than as a literal
    /// colour, because the property that has to hold is contrast against what is beneath,
    /// and the literal is the theme's business.
    #[test]
    fn selectable_fields_are_inputs_not_bare_surfaces() {
        let manager = ThemeManager::new();
        let window_fill = manager.resolve_style("window").background_color;

        for kind in
            ["list_box", "list_view", "table_view", "tree_view", "scroll_area", "text_browser"]
        {
            assert_eq!(
                WidgetRole::for_kind_name(kind),
                WidgetRole::Input,
                "{kind} is a field a cursor can enter; classifying it as a plain surface
                 makes it the window's colour and therefore invisible"
            );
            let fill = manager.resolve_style(kind).background_color;
            assert_ne!(
                fill, window_fill,
                "{kind} must not be filled with the window's own background {window_fill:?}"
            );
        }
    }

    // ── Resolution ────────────────────────────────────────────────────────

    /// The font token set reaches the resolved style. `Theme::fonts` has nine
    /// tokens and none of them used to be read, so every control kept its
    /// constructor's font.
    #[test]
    fn resolved_style_carries_the_theme_font() {
        let manager = ThemeManager::new();
        let style = manager.resolve_style("label");
        let expected = manager.current_theme().unwrap().fonts.body.clone();
        assert_eq!(style.font, Some(expected));
    }

    /// A filled action's foreground is chosen for legibility against its own fill,
    /// rather than hardcoded white. With a light primary colour the text must go
    /// dark, which the old `Color::rgba(255, 255, 255, 255)` could not express.
    #[test]
    fn a_light_primary_fill_gets_dark_text() {
        let mut theme = Theme::default();
        theme.colors.primary = Color::rgba(250, 250, 250, 255);
        let mut manager = ThemeManager::new();
        manager.register_theme(theme);
        assert!(manager.set_theme("default"));

        let style = manager.resolve_style("button");
        let text = style.text_color.expect("a button resolves a text colour");
        assert!(text.r < 60 && text.g < 60 && text.b < 60, "expected dark text, got {text:?}");
    }

    /// The mirrored case: a dark fill gets light text.
    #[test]
    fn a_dark_primary_fill_gets_light_text() {
        let mut theme = Theme::default();
        theme.colors.primary = Color::rgba(10, 10, 10, 255);
        let mut manager = ThemeManager::new();
        manager.register_theme(theme);
        assert!(manager.set_theme("default"));

        let style = manager.resolve_style("button");
        let text = style.text_color.expect("a button resolves a text colour");
        assert!(text.r > 200 && text.g > 200 && text.b > 200, "expected light text, got {text:?}");
    }

    /// An input's interior is derived from the theme's own background, so a dark
    /// theme gets a dark field rather than the forced-white one it used to get.
    #[test]
    fn input_background_follows_the_theme() {
        let light = Theme::default();
        let dark = Theme::dark();
        let light_input = light.colors.input_background();
        let dark_input = dark.colors.input_background();
        assert!(
            light_input.r > dark_input.r,
            "the light theme's input ({light_input:?}) must be lighter than the dark theme's \
             ({dark_input:?})"
        );
    }

    /// A state override applies only in that state, and only to the field it names.
    #[test]
    fn a_state_override_applies_only_in_that_state() {
        let theme = theme_with_overrides(&[(
            "button:hover",
            ThemeStyleToken { background: Some(Color::rgba(1, 2, 3, 255)), ..Default::default() },
        )]);
        let mut manager = ThemeManager::new();
        manager.register_theme(theme);
        assert!(manager.set_theme("default"));

        let resting = manager.resolve_style_for_state("button", None);
        let hovered = manager.resolve_style_for_state("button", Some(WidgetState::Hover));
        let pressed = manager.resolve_style_for_state("button", Some(WidgetState::Pressed));

        assert_eq!(hovered.background_color, Some(Color::rgba(1, 2, 3, 255)));
        assert_ne!(resting.background_color, Some(Color::rgba(1, 2, 3, 255)));
        assert_ne!(pressed.background_color, Some(Color::rgba(1, 2, 3, 255)));
    }

    /// A class-scoped token overrides the role default while leaving the fields it
    /// does not name at the role's values.
    #[test]
    fn a_partial_token_overrides_only_what_it_names() {
        let theme = theme_with_overrides(&[(
            "label",
            ThemeStyleToken { border_width: Some(9), ..Default::default() },
        )]);
        let expected_radius = theme.borders.radius;
        let mut manager = ThemeManager::new();
        manager.register_theme(theme);
        assert!(manager.set_theme("default"));

        let style = manager.resolve_style("label");
        assert_eq!(style.border_width, Some(9), "the named field is overridden");
        // A field the token does not name keeps the role default.
        assert_eq!(style.border_radius, Some(expected_radius));
    }

    /// `set_appearance` selects a registered theme of that appearance, and reports
    /// `false` rather than silently keeping the current one when none matches.
    #[test]
    fn set_appearance_selects_and_reports_honestly() {
        let mut manager = ThemeManager::new();
        manager.register_theme(Theme::dark());

        assert!(manager.set_appearance(AppearanceMode::Dark));
        assert_eq!(manager.current_theme_name(), "dark");
        assert!(manager.set_appearance(AppearanceMode::Light));
        assert_eq!(manager.current_theme_name(), "default");
    }

    /// A fresh manager holds only the light default, so asking for dark must answer
    /// `false` and leave the light theme active — not silently succeed.
    #[test]
    fn set_appearance_without_a_matching_theme_reports_false() {
        let mut manager = ThemeManager::new();
        // Only the light default is registered on a fresh manager.
        assert!(!manager.set_appearance(AppearanceMode::Dark));
        assert_eq!(manager.current_theme_name(), "default");
    }

    /// The global manager is seeded with both appearances, so the previously
    /// unreachable `Theme::dark` preset is usable without prior registration.
    #[test]
    fn the_global_manager_offers_both_appearances() {
        let _guard = theme_test_guard();
        let manager = global_theme_manager();
        assert!(manager.get_theme("default").is_some());
        assert!(manager.get_theme("dark").is_some());
        assert!(
            manager.theme_names().len() >= 2,
            "both presets must be registered: {:?}",
            manager.theme_names()
        );
    }

    /// A high-contrast override replaces the resolved background and text colour
    /// with the forced pair, on every role.
    #[test]
    fn a_high_contrast_override_forces_the_pair() {
        let mut manager = ThemeManager::new();
        assert_eq!(manager.high_contrast(), HighContrastMode::None);

        manager.set_high_contrast(HighContrastMode::WhiteOnBlack);
        for class in ["button", "label", "lineedit", "slider", "checkbox", "panel", ""] {
            let style = manager.resolve_style(class);
            assert_eq!(
                style.background_color,
                Some(Color::BLACK),
                "{class}: the forced background must apply"
            );
            assert_eq!(
                style.text_color,
                Some(Color::WHITE),
                "{class}: the forced foreground must apply"
            );
        }
    }

    /// The override outranks a theme's own class token and a state variant, because
    /// a user who asked for maximum contrast must not have it undone by a palette.
    #[test]
    fn a_high_contrast_override_outranks_tokens_and_states() {
        let theme = theme_with_overrides(&[
            (
                "button",
                ThemeStyleToken {
                    background: Some(Color::rgb(1, 2, 3)),
                    foreground: Some(Color::rgb(4, 5, 6)),
                    ..Default::default()
                },
            ),
            (
                "button:hover",
                ThemeStyleToken { background: Some(Color::rgb(7, 8, 9)), ..Default::default() },
            ),
        ]);
        let mut manager = ThemeManager::new();
        manager.register_theme(theme);
        assert!(manager.set_theme("default"));
        manager.set_high_contrast(HighContrastMode::BlackOnWhite);

        let resting = manager.resolve_style_for_state("button", None);
        assert_eq!(resting.background_color, Some(Color::WHITE));
        assert_eq!(resting.text_color, Some(Color::BLACK));

        // The state token's background must not leak through either.
        let hovered = manager.resolve_style_for_state("button", Some(WidgetState::Hover));
        assert_eq!(hovered.background_color, Some(Color::WHITE));
        assert_eq!(hovered.text_color, Some(Color::BLACK));
    }

    /// A gradient would defeat a flat forced pair, so the override clears it.
    #[test]
    fn a_high_contrast_override_clears_a_gradient() {
        let theme = theme_with_overrides(&[(
            "button",
            ThemeStyleToken { background: Some(Color::RED), ..Default::default() },
        )]);
        let mut manager = ThemeManager::new();
        manager.register_theme(theme);
        assert!(manager.set_theme("default"));

        // A theme override may itself set a gradient; whether it does or not, the
        // forced pair must leave the result flat.
        manager.set_high_contrast(HighContrastMode::BlackOnWhite);
        let after = manager.resolve_style("button");
        assert_eq!(after.background_gradient, None, "a forced pair must be flat");
        assert_eq!(after.background_color, Some(Color::WHITE));
        assert_eq!(after.text_color, Some(Color::BLACK));
    }

    /// Returning to `None` restores the theme's own colours.
    #[test]
    fn clearing_the_high_contrast_override_restores_the_theme() {
        let mut manager = ThemeManager::new();
        let before = manager.resolve_style("button");

        manager.set_high_contrast(HighContrastMode::WhiteOnBlack);
        assert_ne!(manager.resolve_style("button").background_color, before.background_color);

        manager.set_high_contrast(HighContrastMode::None);
        assert_eq!(manager.resolve_style("button"), before);
    }

    /// The override survives a theme switch: it is a user preference, not a
    /// property of one theme.
    #[test]
    fn the_high_contrast_override_survives_a_theme_switch() {
        let mut manager = ThemeManager::new();
        manager.register_theme(Theme::dark());
        manager.set_high_contrast(HighContrastMode::WhiteOnBlack);

        assert!(manager.set_theme("dark"));
        let style = manager.resolve_style("button");
        assert_eq!(style.background_color, Some(Color::BLACK));
        assert_eq!(style.text_color, Some(Color::WHITE));
        assert_eq!(manager.high_contrast(), HighContrastMode::WhiteOnBlack);
    }

    // ── Theme file round-trip ────────────────────────────────────────────

    /// A theme survives a save → load round-trip with its tokens intact. Nothing
    /// tested this before, so a broken `Serialize`/`Deserialize` pair would have
    /// gone unnoticed.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn a_theme_round_trips_through_json() {
        let mut manager = ThemeManager::new();
        manager.set_theme("default");

        let path =
            std::env::temp_dir().join(format!("rw-theme-roundtrip-{}.json", std::process::id()));
        let path_str = path.to_str().expect("the temp path is UTF-8");
        manager.save_theme(path_str).expect("save must succeed");

        let mut reloaded = ThemeManager::new();
        let name =
            reloaded.load_and_activate_theme(path_str).expect("the file we just wrote must load");
        assert_eq!(name, "default", "activation is by the name in the file");
        assert_eq!(reloaded.current_theme_name(), "default");

        // The tokens match the originals, field by field.
        let original = manager.current_theme().expect("active").clone();
        let restored = reloaded.current_theme().expect("active");
        assert_eq!(restored.name, original.name);
        assert_eq!(restored.appearance, original.appearance);
        assert_eq!(restored.colors.background, original.colors.background);
        assert_eq!(restored.colors.primary, original.colors.primary);
        assert_eq!(restored.spacing.medium, original.spacing.medium);
        assert_eq!(restored.borders.radius, original.borders.radius);
        assert_eq!(restored.fonts.body.size(), original.fonts.body.size());

        let _ = std::fs::remove_file(&path);
    }

    /// `load_theme` registers without activating (the pre-existing behaviour),
    /// while `load_and_activate_theme` activates. Both are pinned so the two cannot
    /// drift into each other.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn load_registers_and_load_and_activate_activates() {
        let theme = Theme {
            name: "round-trip-fixture".to_string(),
            colors: Colors { primary: Color::rgb(9, 8, 7), ..Theme::default().colors },
            ..Theme::default()
        };

        let path =
            std::env::temp_dir().join(format!("rw-theme-activate-{}.json", std::process::id()));
        let path_str = path.to_str().expect("the temp path is UTF-8");
        std::fs::write(path_str, serde_json::to_string(&theme).expect("serialize"))
            .expect("write fixture");

        // Register-only.
        let mut manager = ThemeManager::new();
        let before = manager.current_theme_name().to_string();
        manager.load_theme(path_str).expect("load");
        assert_eq!(manager.current_theme_name(), before, "load_theme must not activate");
        assert!(manager.get_theme("round-trip-fixture").is_some(), "but it is registered");

        // Register and activate.
        let mut manager = ThemeManager::new();
        let activated = manager.load_and_activate_theme(path_str).expect("load and activate");
        assert_eq!(activated, "round-trip-fixture");
        assert_eq!(manager.current_theme_name(), "round-trip-fixture");
        assert_eq!(
            manager.current_theme().expect("active").colors.primary,
            Color::rgb(9, 8, 7),
            "the loaded tokens must be in effect"
        );

        let _ = std::fs::remove_file(&path);
    }

    /// A file that is not a theme is reported, not accepted silently.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn a_malformed_theme_file_is_reported() {
        let path =
            std::env::temp_dir().join(format!("rw-theme-malformed-{}.json", std::process::id()));
        let path_str = path.to_str().expect("the temp path is UTF-8");
        std::fs::write(path_str, "{ not json at all").expect("write fixture");

        let mut manager = ThemeManager::new();
        assert!(manager.load_theme(path_str).is_err(), "a malformed file must error");
        assert!(manager.load_and_activate_theme(path_str).is_err());

        let _ = std::fs::remove_file(&path);
    }

    /// The active theme can be changed through the global manager, which is what
    /// makes a theme switch take effect for the whole process.
    #[test]
    fn the_global_appearance_can_be_switched_and_restored() {
        let _guard = theme_test_guard();
        let before = resolved_theme_style("button").expect("a theme is active");

        {
            let mut manager = global_theme_manager();
            assert!(manager.set_appearance(AppearanceMode::Dark));
        }
        let dark = resolved_theme_style("button").expect("a theme is active");
        assert_ne!(
            before.background_color, dark.background_color,
            "the switch must change the fill"
        );

        // Restore so no later test observes the dark theme.
        {
            let mut manager = global_theme_manager();
            assert!(manager.set_appearance(AppearanceMode::Light));
        }
        let restored = resolved_theme_style("button").expect("a theme is active");
        assert_eq!(restored.background_color, before.background_color);
    }
    /// A node's `class` must not replace the widget kind when the role is resolved.
    ///
    /// `resolve_style` classifies its argument with `WidgetRole::for_kind_name`, whose table is
    /// keyed on **control kinds** (`button`, `label`, `line_edit`, ...). The JSON loader passed
    /// a node's `class` in place of its kind, so `<button class="primary">` was classified from
    /// the string `"primary"` — not a control kind — and fell through to `WidgetRole::Surface`.
    /// The button was painted as a grey panel instead of a filled brand-coloured control, i.e.
    /// the class silently discarded the role it was there to select.
    ///
    /// `resolve_style_for` keeps the two vocabularies apart: the kind picks the role, the class
    /// only selects an override.
    #[test]
    fn a_class_does_not_replace_the_kind_when_resolving_a_role() {
        let _guard = theme_test_guard();
        let manager = global_theme_manager();

        let by_kind = manager.resolve_style_for("Button", None, None);
        let with_class = manager.resolve_style_for("Button", Some("primary"), None);

        // The role default is what the kind gives; the class must not change it into Surface.
        assert_eq!(
            with_class.background_color, by_kind.background_color,
            "a class must not reclassify the widget's role"
        );
        assert_ne!(
            with_class.background_color,
            Some(Color::rgb(240, 240, 240)),
            "the class must not collapse the button to the Surface role"
        );

        // And the misclassification this guards against is still what a bare class name does:
        // a name that is not a control kind resolves as Surface when it is used *as* the kind.
        //
        // The expected value is read from the live theme rather than written as the literal the
        // test used to carry (`rgb(240,240,240)`): what the assertion is *about* is that `"primary"`
        // lands on the **same role as any unknown kind**, not about which colour that role happens
        // to resolve to. Hardcoding the colour made the test fail the moment the role's fill was
        // corrected — a false alarm about the wrong thing.
        let as_kind = manager.resolve_style_for("primary", None, None);
        let unknown_kind = manager.resolve_style_for("SomeThirdPartyWidget", None, None);
        assert_eq!(
            as_kind.background_color, unknown_kind.background_color,
            "the regression this test pins: 'primary' is not a control kind, so it resolves as \
             the same role an unknown kind does"
        );
        assert_ne!(
            as_kind.background_color, by_kind.background_color,
            "and that role must not be the one a button gets"
        );
    }

    /// A class-level override is still consulted, now through the class argument.
    #[test]
    fn a_class_override_still_applies() {
        use crate::compat::HashMap;
        use crate::theme::types::{ThemeOverrides, ThemeStyleToken};

        let _guard = theme_test_guard();
        let mut manager = ThemeManager::new();
        let Some(mut theme) = manager.get_theme("default").cloned() else {
            panic!("the default theme must be registered");
        };
        let mut overrides = ThemeOverrides { styles: HashMap::new() };
        overrides.styles.insert(
            "primary".to_string(),
            ThemeStyleToken { background: Some(Color::rgb(1, 2, 3)), ..Default::default() },
        );
        theme.overrides = overrides;
        manager.register_theme(theme);
        assert!(manager.set_theme("default"));

        let styled = manager.resolve_style_for("Button", Some("primary"), None);
        assert_eq!(
            styled.background_color,
            Some(Color::rgb(1, 2, 3)),
            "a theme override keyed by the class must still win over the role default"
        );
    }

    /// A container's role resolves to the container token, not to the window's own fill.
    ///
    /// # The defect this closes (BLUE22 · F-9)
    ///
    /// `Colors::surface_container` was declared with the documentation "the container colour for
    /// cards and panels sitting on `background`" and had **no consumer at all**: the `Surface` role —
    /// the arm every card, panel, group box, container and unnamed third-party kind falls through to
    /// — resolved to `theme.colors.background`, i.e. the very colour a window paints. Every such
    /// control was therefore filled with the colour behind it, so its extent was invisible: the
    /// frame rendered correctly and showed nothing where the control was. Measured on the committed
    /// snapshots, `adaptive_scaffold` and `carousel` both carried `rgba(18,18,18)` fills identical to
    /// their own backdrop.
    ///
    /// The assertion is the *distinction*, not a literal colour: a container must not be
    /// byte-identical to the surface it sits on, because a control whose fill equals its backdrop has
    /// no extent at all. That is the property the role exists for, and it is what makes this a
    /// statement about the theme rather than about one preset's numbers.
    #[test]
    fn a_surface_role_is_not_byte_identical_to_the_window_fill() {
        let _guard = theme_test_guard();
        let manager = global_theme_manager();
        // A kind the role table does not name: the pure `Surface` case, which is what a card, a
        // panel and a container all are.
        let container = manager.resolve_style_for("SomeThirdPartyContainer", None, None);
        let Some(fill) = container.background_color else {
            panic!("a surface role must resolve a fill; a container with none cannot be seen")
        };
        let Some(active) = manager.current_theme() else {
            panic!("a theme must be active for this test to mean anything")
        };
        assert_ne!(
            fill, active.colors.background,
            "a container painted in the window's own fill has no visible extent"
        );
        assert_eq!(
            fill, active.colors.surface_container,
            "and the fill it does use is the container token that exists for it"
        );
    }

    /// Every token the crate adds for a role has a consumer in `src/`.
    ///
    /// # Why this is a test and not a comment
    ///
    /// BLUE22 · F-9 found **six of seven** new `Colors` roles with zero consumers: `outline_variant`,
    /// `scrim`, `surface_container`, `surface_container_high`, `inverse_surface` and
    /// `on_inverse_surface` were declared, documented with the control each was for, and read by
    /// nobody. Principle #4 forbids exactly this ("zero-consumer mechanisms must not exist"), and the
    /// way it happens is that the token is added in one round and its consumer is deferred — so the
    /// check belongs next to the declaration, where the next addition will see it.
    ///
    /// The source scan is deliberately textual and narrow: it looks for `colors.<token>` outside this
    /// file, which is the only shape a consumer can take (`Colors` is a plain struct). A token added
    /// without a consumer therefore fails here rather than shipping as decoration.
    #[test]
    fn every_new_colour_role_has_a_consumer_outside_the_theme_module() {
        // The roles added by P0-10. Each names the control that reads it, in its own doc comment.
        let tokens = [
            "outline",
            "outline_variant",
            "scrim",
            "surface_container",
            "surface_container_high",
            "inverse_surface",
            "on_inverse_surface",
        ];
        let mut sources = String::new();
        let mut stack = vec![std::path::PathBuf::from("src")];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else { continue };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().is_some_and(|ext| ext == "rs") {
                    // This file *declares* the roles, so it can never be a consumer.
                    if path.ends_with("src/theme/types.rs") {
                        continue;
                    }
                    if let Ok(text) = std::fs::read_to_string(&path) {
                        sources.push_str(&text);
                    }
                }
            }
        }
        let mut unconsumed = Vec::new();
        for token in tokens {
            let needle = format!("colors.{token}");
            if !sources.contains(&needle) {
                unconsumed.push(token);
            }
        }
        assert!(
            unconsumed.is_empty(),
            "these colour roles are declared and documented but read by nothing, so a theme author \
             can set them and see no effect: {unconsumed:?}. Either give each one its consumer or \
             remove the role (principle #4)."
        );
    }
}
