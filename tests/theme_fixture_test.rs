// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Theme file fixtures — the shipped JSON presets, kept in step with the code.
//!
//! # Why these live in `themes/` and are checked in
//!
//! `Theme` is `Serialize`/`Deserialize` and `ThemeManager` can save and load it,
//! but nothing in the repository exercised that: there was no fixture, so a change
//! that broke the JSON shape — a renamed field, a type that stopped round-tripping
//! — would have gone unnoticed until a user hit it. (The fields are the *public*
//! API of the file format, which is exactly the sort of thing that needs a
//! checked-in example.)
//!
//! The files are **generated from the built-in presets**, not hand-written: a
//! hand-written fixture is a second description of the schema and drifts the moment
//! the struct changes. `themes/generate.sh` regenerates them from the same
//! `Theme::default()` / `Theme::dark()` the library uses, and
//! [`fixtures_match_the_built_in_presets`] fails if the checked-in files and the
//! presets disagree.
//!
//! # What is covered
//!
//! * Every preset file deserializes back into a `Theme`.
//! * The deserialized theme equals the preset it was generated from, field by
//!   field — so the round-trip is lossless, not merely parseable.
//! * A theme loaded from a file resolves a style, i.e. it is usable end to end.
//! * `overrides` with a non-empty token table survives the round trip, which is
//!   the part most likely to break silently (nested `Option`s).

#[cfg(all(test, feature = "desktop"))]
mod tests {
    use rust_widgets::theme::{Theme, ThemeManager};

    /// Directory holding the shipped fixtures, relative to the crate root.
    const THEME_DIR: &str = "themes";

    fn fixture_path(name: &str) -> String {
        format!("{THEME_DIR}/{name}.json")
    }

    /// Every built-in preset has a checked-in file, and the file round-trips to the
    /// preset exactly.
    #[test]
    fn fixtures_match_the_built_in_presets() {
        let presets = [Theme::default(), Theme::dark()];

        for preset in &presets {
            let path = fixture_path(&preset.name);
            let text = std::fs::read_to_string(&path).unwrap_or_else(|error| {
                panic!(
                    "the preset '{}' has no checked-in fixture at {path}: {error} (regenerate \
                     with themes/generate.sh)",
                    preset.name
                )
            });

            let parsed: Theme = serde_json::from_str(&text).unwrap_or_else(|error| {
                panic!("{path} does not deserialize into a Theme: {error}")
            });

            assert_eq!(parsed.name, preset.name, "{path} declares the wrong theme name");
            assert_eq!(
                parsed.appearance, preset.appearance,
                "{path} declares the wrong appearance"
            );
            assert_eq!(
                parsed.colors.background, preset.colors.background,
                "{path}: background colour drifted"
            );
            assert_eq!(
                parsed.colors.foreground, preset.colors.foreground,
                "{path}: foreground colour drifted"
            );
            assert_eq!(parsed.colors.primary, preset.colors.primary, "{path}: primary drifted");
            assert_eq!(parsed.colors.accent, preset.colors.accent, "{path}: accent drifted");
            assert_eq!(
                parsed.spacing.medium, preset.spacing.medium,
                "{path}: spacing scale drifted"
            );
            assert_eq!(parsed.borders.width, preset.borders.width, "{path}: border width drifted");
            assert_eq!(
                parsed.borders.radius, preset.borders.radius,
                "{path}: border radius drifted"
            );
            // Fonts are the easiest thing to lose silently, because `Font` has
            // private fields and its own serde representation.
            assert_eq!(
                parsed.fonts.body.family(),
                preset.fonts.body.family(),
                "{path}: body font family drifted"
            );
            assert_eq!(
                parsed.fonts.body.size(),
                preset.fonts.body.size(),
                "{path}: body font size drifted"
            );
            assert_eq!(
                parsed.fonts.monospace.family(),
                preset.fonts.monospace.family(),
                "{path}: monospace font family drifted"
            );
        }
    }

    /// A fixture loads through the manager and resolves a real style, so a shipped
    /// theme is usable end to end rather than merely parseable.
    #[test]
    fn a_fixture_loads_and_resolves_a_style() {
        let mut manager = ThemeManager::new();
        let name = manager
            .load_and_activate_theme(&fixture_path("default"))
            .expect("the default fixture must load");
        assert_eq!(name, "default");
        assert_eq!(manager.current_theme_name(), "default");

        let style = manager.resolve_style("button");
        assert!(style.background_color.is_some(), "a loaded theme must resolve a background");
        assert!(style.font.is_some(), "a loaded theme must resolve a font");
        assert_eq!(
            style.background_color,
            Some(manager.current_theme().expect("active").colors.primary),
            "a button's fill must come from the loaded theme's primary token"
        );
    }

    /// The shadow override is a three-way choice, and all three cases survive the
    /// round trip. This caught a real defect: the field used to be
    /// `Option<Option<ShadowToken>>`, and serde maps a JSON `null` to `None` at the
    /// outer level, so "clear the shadow" was unreachable from a file and silently
    /// became "do not mention it".
    #[test]
    fn an_override_token_survives_the_round_trip() {
        use rust_widgets::core::Color;
        use rust_widgets::style::WidgetState;
        use rust_widgets::theme::{ShadowOverride, ShadowToken, ThemeOverrides, ThemeStyleToken};

        let theme = Theme {
            overrides: ThemeOverrides {
                styles: [
                    (
                        "button".to_string(),
                        ThemeStyleToken {
                            background: Some(Color::rgb(1, 2, 3)),
                            foreground: None,
                            border: Some(Color::rgb(4, 5, 6)),
                            border_width: Some(3),
                            radius: None,
                            font: None,
                            opacity: Some(0.5),
                            shadow: ShadowOverride::None,
                            touch_target: Some([44, 44]),
                        },
                    ),
                    (
                        "panel".to_string(),
                        ThemeStyleToken {
                            shadow: ShadowOverride::Set(ShadowToken {
                                x: 1,
                                y: 2,
                                blur: 3,
                                color: Color::rgb(9, 9, 9),
                            }),
                            ..Default::default()
                        },
                    ),
                    (
                        "button:hover".to_string(),
                        ThemeStyleToken {
                            background: Some(Color::rgb(7, 8, 9)),
                            ..Default::default()
                        },
                    ),
                ]
                .into_iter()
                .collect(),
            },
            ..Theme::default()
        };

        let json = serde_json::to_string(&theme).expect("serialize");
        let parsed: Theme = serde_json::from_str(&json).expect("deserialize");

        let token = parsed.overrides.styles.get("button").expect("the override token must survive");
        assert_eq!(token.background, Some(Color::rgb(1, 2, 3)));
        assert_eq!(token.foreground, None, "an unset field must stay unset");
        assert_eq!(token.border, Some(Color::rgb(4, 5, 6)));
        assert_eq!(token.border_width, Some(3));
        assert_eq!(token.opacity, Some(0.5));
        assert_eq!(
            token.shadow,
            ShadowOverride::None,
            "\"clear the shadow\" must not collapse into \"do not mention it\""
        );
        assert_eq!(token.touch_target, Some([44, 44]));

        let set_token =
            parsed.overrides.styles.get("panel").expect("the second token must survive");
        match set_token.shadow {
            ShadowOverride::Set(shadow) => {
                assert_eq!((shadow.x, shadow.y, shadow.blur), (1, 2, 3));
                assert_eq!(shadow.color, Color::rgb(9, 9, 9));
            }
            other => panic!("expected a shadow to survive, got {other:?}"),
        }

        // The state-scoped key survives too, and still selects in that state.
        assert!(parsed.overrides.styles.contains_key("button:hover"));
        let mut manager = ThemeManager::new();
        manager.register_theme(parsed);
        assert!(manager.set_theme("default"));
        let hovered = manager.resolve_style_for_state("button", Some(WidgetState::Hover));
        assert_eq!(hovered.background_color, Some(Color::rgb(7, 8, 9)));

        // And the three cases behave as three cases when resolved, not just when
        // parsed: `None` clears a shadow the base supplied, `Inherit` keeps it.
        let with_shadow = manager.resolve_style_for_state("panel", None);
        assert!(with_shadow.shadow.is_some(), "`Set` must install a shadow");
        let cleared = manager.resolve_style_for_state("button", None);
        assert_eq!(cleared.shadow, None, "`None` must remove the shadow");
    }
}
