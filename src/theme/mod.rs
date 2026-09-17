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
//!   *apply*. The JSON loader merges `resolved_theme_style` under each node's own
//!   style, and `crate::style::global_stylesheet_manager` layers CSS on top, giving
//!   the documented precedence: theme → stylesheet → explicit style.
//!
//! Without the global accessor the registry was unreachable from the rest of the
//! crate, and every control kept the colours its constructor hardcoded.
mod manager;
mod types;

/// The high-contrast override that [`resolved_theme_style`] honours.
///
/// Re-exported from `crate::style`, where it is defined next to the
/// interaction-state model it interacts with, so a theme consumer has one import
/// path instead of two.
pub use crate::style::HighContrastMode;
/// Serialises tests that switch the process-wide theme; see its own docs.
#[cfg(test)]
pub(crate) use manager::theme_test_guard;
pub use manager::{
    global_high_contrast, global_theme_manager, resolved_theme_style, set_global_high_contrast,
    ThemeManager,
};
pub use types::{
    AppearanceMode, Borders, Colors, Fonts, ShadowOverride, ShadowToken, Spacing, Theme,
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
}
