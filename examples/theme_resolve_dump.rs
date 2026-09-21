// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! What the active theme resolves to for the control demo's widgets.
//!
//! # Why this exists
//!
//! The demo's controls looked uniformly grey — a button's normal background resolved to
//! the same colour as the window, so only its text was visible. That could be a theme
//! that does not describe buttons, a classification that puts buttons in the wrong role,
//! or the control ignoring what it was given. This prints the resolved style for each
//! kind, so the question is answered by the values rather than by reading the merger.
//!
//! # Why both appearances
//!
//! The first version printed only whatever appearance happened to be active, which is a
//! **light** theme by default. The control demo runs dark, so the table answered a
//! question the demo does not ask: it reported `combo_box` background as
//! `rgb(180,180,180)` (a light grey) while the defect on screen was a combo box painted
//! the same colour as the window. Printing both appearances puts the dark values, and
//! the dark window background they must contrast with, in one run.
//!
//! # Why it also prints the window background
//!
//! "This colour is invisible" is a *relative* judgement: a colour is invisible when it
//! equals what is underneath. Hard-coding the window background in the output (as this
//! probe first did) lets the claim and the value drift apart silently. Reading it from
//! the theme's own `background` token keeps both sides of the comparison together.

#![cfg(all(
    any(feature = "desktop", feature = "tablet", feature = "mobile"),
    not(any(feature = "mini", feature = "embedded"))
))]

use rust_widgets::core::Color;
use rust_widgets::theme::{global_theme_manager, resolved_theme_style, AppearanceMode};

/// The kinds the control demo places, by factory name.
const KINDS: &[&str] = &[
    "button",
    "check_box",
    "radio_button",
    "spin_box",
    "combo_box",
    "line_edit",
    "list_box",
    "slider",
    "progress_bar",
    "label",
    "panel",
    "frame",
    "scroll_area",
    "message_box",
    // Window-level chrome. These are the kinds the control demo's toolbar and status
    // bar resolve through, and a chrome surface that reads as `Background` in a dark
    // theme is a light band across the top of the window.
    "tool_bar",
    "status_bar",
    "menu_bar",
    "group_box",
    "scroll_bar",
    "list_view",
];

fn rgb(color: Option<Color>) -> String {
    color.map(|c| format!("rgb({},{},{})", c.r, c.g, c.b)).unwrap_or_else(|| "(none)".to_string())
}

/// Prints the resolved styles for whichever appearance is active, then one row per kind.
fn dump(label: &str, window_bg: Option<Color>) {
    println!("── {label} ──  window background: {}", rgb(window_bg));
    println!("{:<16} {:<28} {:<28} border", "kind", "background", "text");
    println!("{}", "-".repeat(96));
    for kind in KINDS {
        match resolved_theme_style(kind) {
            Some(style) => {
                let border = style
                    .border_color
                    .map(|c| format!("rgb({},{},{}) w={:?}", c.r, c.g, c.b, style.border_width))
                    .unwrap_or_else(|| "(none)".to_string());
                // Mark the rows that would be painted invisibly: a control whose fill
                // equals the window background has no visible extent at all.
                let invisible = match (style.background_color, window_bg) {
                    (Some(fill), Some(bg))
                        if fill.r == bg.r && fill.g == bg.g && fill.b == bg.b =>
                    {
                        "  <-- same as window background"
                    }
                    _ => "",
                };
                println!(
                    "{kind:<16} {:<28} {:<28} {border}{invisible}",
                    rgb(style.background_color),
                    rgb(style.text_color)
                );
            }
            None => println!("{kind:<16} (no theme style resolved)"),
        }
    }
    println!();
}

fn main() {
    for (mode, label) in [(AppearanceMode::Light, "light"), (AppearanceMode::Dark, "dark")] {
        if !global_theme_manager().set_appearance(mode) {
            println!("── {label} ──  (this appearance is not registered)\n");
            continue;
        }
        let window_bg = global_theme_manager().current_theme().map(|t| t.colors.background);
        dump(label, window_bg);
    }
}
