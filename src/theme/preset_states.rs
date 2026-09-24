// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Interaction-state overrides baked into the built-in theme presets.
//!
//! # Why this module exists
//!
//! The state channel is end to end: a control reports its [`crate::style::WidgetState`],
//! `apply_active_theme` looks the control up by `"<kind>:<state>"`, and
//! `ThemeManager::resolve_style_for_state` merges the override. Every one of those
//! pieces was implemented and tested — and every preset shipped with `overrides.styles`
//! **empty**, so a hovered button resolved to the same style as a resting one and the
//! channel had nothing to carry.
//!
//! The values are derived from a preset's own [`Colors`] rather than hard-coded, so the
//! light and dark presets cannot drift apart and a preset that changes its palette gets
//! matching state colours for free. The derivations follow the published opacity
//! convention every toolkit uses for the same purpose: a hover is a light step toward
//! the content colour, a press is a firmer one, and a disabled control fades toward its
//! own background.

use crate::compat::BTreeMap;
use crate::theme::types::{Colors, ThemeStyleToken};

/// Blend factor for a hovered control's fill, toward `foreground`.
///
/// The value the major toolkits agree on for "the pointer is over me": a visible but
/// non-committal step. Written once so every state key below uses the same number
/// rather than each spelling its own.
const HOVER_BLEND: f32 = 0.08;

/// Blend factor for a pressed control's fill, toward `foreground`.
///
/// Firmer than [`HOVER_BLEND`], because a press is a committed gesture.
const PRESSED_BLEND: f32 = 0.12;

/// Blend factor for taking a control's fill toward its own background when disabled.
const DISABLED_FADE: f32 = 0.55;

/// Builds the `"<kind>:<state>"` override table for a preset.
///
/// # Which kinds get which states
///
/// Only states that change *what is painted* are listed, and only for the kinds where a
/// state is genuinely reachable:
///
/// * `button` / `toggle_button` / `tool_button` are the daily interactive fills, so they
///   get hover, pressed and disabled.
/// * `check_box` / `radio_button` / `switch` latch a value, so they get `checked`.
/// * The editable kinds get `:error`, which is the one state the `error` colour role had
///   no consumer for.
///
/// A resting control is deliberately absent: `resolve_style` already answers it, and
/// restating it here would be a second description of the base appearance that could
/// drift.
pub(super) fn preset_state_overrides(colors: &Colors) -> BTreeMap<String, ThemeStyleToken> {
    let mut styles = BTreeMap::new();
    let ink = colors.foreground;

    // The three fills a push button moves through. A hover and a press are the same
    // derivation at two strengths, so a theme change moves both together.
    for kind in ["button", "toggle_button", "tool_button", "split_button"] {
        styles.insert(
            format!("{kind}:hover"),
            ThemeStyleToken {
                background: Some(colors.background.blend(&ink, HOVER_BLEND)),
                ..ThemeStyleToken::default()
            },
        );
        styles.insert(
            format!("{kind}:pressed"),
            ThemeStyleToken {
                background: Some(colors.background.blend(&ink, PRESSED_BLEND)),
                ..ThemeStyleToken::default()
            },
        );
        styles.insert(
            format!("{kind}:disabled"),
            ThemeStyleToken {
                background: Some(colors.disabled),
                foreground: Some(colors.background.blend(&ink, DISABLED_FADE)),
                ..ThemeStyleToken::default()
            },
        );
    }

    // Latching controls: the checked fill is the brand colour, so a checked box reads as
    // "on" at a glance rather than as a slightly darker box.
    //
    // # Why the controls report these states themselves
    //
    // Every key below is looked up as `"<kind>:<state>"`, so a key is only reachable if some
    // control actually *reports* that state. The trait's default `widget_state` knows the four
    // primitive flags (`disabled`/`pressed`/`hovered`/`focused`) and **none of them means
    // "latched"** — so before the latching controls overrode it, all four of these keys per
    // preset resolved to `Normal` and never painted. `CheckBox::widget_state` and its siblings are
    // the consuming half.
    for kind in ["check_box", "radio_button", "switch"] {
        styles.insert(
            format!("{kind}:checked"),
            ThemeStyleToken {
                background: Some(colors.primary),
                foreground: Some(colors.primary.contrast_color()),
                ..ThemeStyleToken::default()
            },
        );
    }
    // A chip list is a collection, not a latch, so it reports `Selected` rather than `Checked`.
    // The key used to be `chip:selected`'s predecessor (`chip:checked`), which the control never
    // reported; see `Chip::widget_state`.
    styles.insert(
        "chip:selected".to_string(),
        ThemeStyleToken {
            background: Some(colors.primary),
            foreground: Some(colors.primary.contrast_color()),
            ..ThemeStyleToken::default()
        },
    );

    // Validation. This is the one place the `error` role reaches a control's own style,
    // which is why it is a border rather than a fill: an invalid field keeps its
    // editable surface and marks itself out.
    for kind in [
        "line_edit",
        "text_edit",
        "text_area",
        "combo_box",
        "editable_combo_box",
        "spin_box",
        "date_edit",
        "time_edit",
        "date_time_edit",
        "masked_edit",
    ] {
        styles.insert(
            format!("{kind}:error"),
            ThemeStyleToken {
                border: Some(colors.error),
                border_width: Some(2),
                ..ThemeStyleToken::default()
            },
        );
    }

    styles
}

/// The number of state keys a preset is expected to carry.
///
/// Exposed so the gate that guards "the presets really have state overrides" and the
/// test that pins the count read the same number rather than each hard-coding it. A
/// preset with fewer keys than this means a state group was dropped.
pub fn preset_state_key_count() -> usize {
    // 4 push-button kinds x 3 states, 3 latching kinds x 1 `checked` + 1 chip `selected`,
    // 10 editable kinds x 1.
    4 * 3 + 3 + 1 + 10
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_preset_family_carries_its_states() {
        for colors in [Colors::default(), crate::theme::Theme::dark().colors] {
            let styles = preset_state_overrides(&colors);
            assert_eq!(
                styles.len(),
                preset_state_key_count(),
                "a preset must carry the full state table, not a subset"
            );
            // Spot-check one key from each family so a whole group going missing is caught
            // by name, not only by count.
            assert!(styles.contains_key("button:hover"));
            assert!(styles.contains_key("button:pressed"));
            assert!(styles.contains_key("button:disabled"));
            assert!(styles.contains_key("check_box:checked"));
            assert!(styles.contains_key("chip:selected"));
            // The key that named a state no control reported: it must be gone, not merely
            // unreferenced, or the table still claims a state the crate cannot reach.
            assert!(
                !styles.contains_key("chip:checked"),
                "`chip` is a collection and reports `Selected`, never `Checked`"
            );
            assert!(styles.contains_key("line_edit:error"));
        }
    }

    /// A hover must differ from the resting fill, or the theme change is invisible.
    ///
    /// This is the state channel's *point*: if the derived hover happened to equal the
    /// background the override would be a no-op and the control would look identical
    /// hovered and at rest.
    #[test]
    fn a_hover_fill_differs_from_the_background() {
        let colors = Colors::default();
        let styles = preset_state_overrides(&colors);
        let hover = styles["button:hover"].background.expect("hover sets a background");
        assert_ne!(hover, colors.background, "hover must move the fill");
        let pressed = styles["button:pressed"].background.expect("pressed sets a background");
        assert_ne!(pressed, hover, "a press must be a firmer step than a hover");
    }
}
