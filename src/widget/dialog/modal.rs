// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Shared dialog chrome: label resolution, the modal scrim, and the reveal geometry.
//!
//! Kept out of `mod.rs` (principle #8). Six dialogs build an action row from `tr!` sites and
//! every modal dims the page behind it, so these bodies live in one file rather than being
//! repeated — which is how six of them came to differ in the first place.

use crate::core::{Color, Rect};
use crate::render::RenderContext;

/// The built-in English labels for the dialog catalogue keys.
///
/// # Why a table in addition to the catalogue
///
/// The catalogue is the translation resource table: when it is initialised it wins, and the
/// embedded English file is loaded on first use. A build without the `i18n` capability has no
/// catalogue at all, so this table is the resource table for that build — the labels live in
/// one place instead of being restated at each `localized_label` call site, which is how two
/// dialogs came to spell "OK" differently.
const DIALOG_RESOURCE_FALLBACKS: &[(&str, &str)] = &[
    ("common.button.ok", "OK"),
    ("common.button.cancel", "Cancel"),
    ("common.button.open", "Open"),
    ("common.button.save", "Save"),
    ("common.button.close", "Close"),
    ("dialog.ok", "OK"),
    ("dialog.cancel", "Cancel"),
];

/// The English label this module ships for `key`, if it has one.
fn resource_fallback(key: &str) -> Option<&'static str> {
    DIALOG_RESOURCE_FALLBACKS
        .iter()
        .find(|(candidate, _)| *candidate == key)
        .map(|(_, label)| *label)
}

/// Resolves a dialog label for **display**: the catalogue's translation when it has one, the
/// built-in resource label for the key, and the English label otherwise.
///
/// # Why the key is not an acceptable fallback
///
/// `translate_key` returns the key itself when no catalogue is initialised (or the key is
/// absent). Drawing that value is how a message box's OK button came to read `common....`: the
/// row reserved 80px for "OK", and the raw key `common.button.ok` was elided to fit. A dialog's
/// buttons are user-facing copy, so the fallback is the English label the enum already carries —
/// never the catalogue key.
///
/// # Why this is one function rather than each dialog's own check
///
/// Six dialogs build their action row from `tr!` sites, and a per-site `== key` comparison is a
/// rule that has to be repeated exactly. One helper means a new dialog inherits the fallback, and
/// a catalogue that is initialised later still resolves correctly because the lookup happens when
/// the label is drawn.
pub(crate) fn localized_label(key: &str, english: &str) -> crate::compat::String {
    #[cfg(feature = "i18n")]
    {
        let translated = crate::i18n::translate(key);
        if translated.is_empty() || translated == key {
            crate::compat::String::from(resource_fallback(key).unwrap_or(english))
        } else {
            translated
        }
    }
    #[cfg(not(feature = "i18n"))]
    {
        // No catalogue exists in this build, so the built-in resource table and then the
        // caller's English label are the whole answer. Going through `translate` here would
        // log a warning on every frame for a case that is not a failure.
        let _ = key;
        crate::compat::String::from(resource_fallback(key).unwrap_or(english))
    }
}

/// Paints the modal scrim over the control's whole rectangle.
///
/// # Why this is one function and not six copies
///
/// Every modal dialog in this module dims the page it sits on, and six of them were silently
/// missing it: `Dialog` and `MessageBox` were fixed first, then the scan showed
/// `color_dialog` / `file_dialog` / `font_dialog` / `input_dialog` / `progress_dialog` carried a
/// `modal` flag with nothing painted for it. Six copies of the same six lines is how six of them
/// came to diverge in the first place, so the body lives here once.
///
/// # Why the `Scrim` role rather than a blend
///
/// BLUE21 B23 recorded that blending toward the ink **lightens** a dark page — a scrim that
/// brightens what it covers is the defect stated in `Colors::scrim`'s own documentation. The
/// role lets a dark theme dim deliberately while a light theme darkens, and the literal survives
/// only as the fallback for a theme that predates the role.
pub(crate) fn draw_modal_scrim(context: &mut RenderContext, page: Rect) {
    draw_modal_scrim_scaled(context, page, 1.0);
}

/// Paints the modal scrim at `strength` of its resolved opacity, for a backdrop that fades in.
///
/// Scaling the **alpha** rather than blending toward the page is the one form that is right for
/// both a dark scrim (which must get darker as it arrives) and a light one (which must get lighter):
/// a blend toward the window fill would move a scrim in the wrong direction in one of the two
/// appearances. `strength` is clamped, and the whole plane is skipped at zero so a hidden dialog
/// leaves the page untouched.
pub(crate) fn draw_modal_scrim_scaled(context: &mut RenderContext, page: Rect, strength: f32) {
    let strength = strength.clamp(0.0, 1.0);
    if strength <= 0.0 {
        return;
    }
    let scrim = crate::style::layer_color(crate::style::LayerColor::Scrim)
        .unwrap_or_else(|| Color::BLACK.with_alpha(82));
    let scrim = scrim.with_alpha((scrim.a as f32 * strength) as u8);
    context.fill_rect(page, scrim);
}

/// How small a revealing modal is at the instant it appears: 0.92 of its settled size.
///
/// A dialog that grows from nothing reads as a pop rather than an arrival, and one that grows from
/// far away reads as a zoom unrelated to a dialog's purpose. A few percent is the "settle" every
/// platform uses; the value is deliberately *not* zero, so the first visible frame already shows the
/// dialog's shape. Shared by every modal family so a dialog and a message box arrive alike — two
/// nearby literals is exactly how the eight dialogs acquired four title-bar heights (rule #101).
pub(crate) const REVEAL_MIN_SCALE: f32 = 0.92;

/// Scales `rect` about its own centre, rounding outward so a shrinking frame never loses a pixel
/// column to truncation while it is still meant to be visible.
///
/// The frame is grown from its centre rather than from a corner because a dialog that grows from a
/// corner reads as a panel sliding in, which is a different affordance (that is what `bottom_sheet`
/// and `navigation_drawer` do). Kept as a free function so a draw path has one expression for the
/// geometry and a test can compose the same rect without duplicating the arithmetic.
pub(crate) fn scale_about_centre(rect: Rect, scale: f32) -> Rect {
    let scale = scale.clamp(0.0, 1.0);
    let width = ((rect.width as f32 * scale) + 0.5) as u32;
    let height = ((rect.height as f32 * scale) + 0.5) as u32;
    let dx = (rect.width.saturating_sub(width) / 2) as i32;
    let dy = (rect.height.saturating_sub(height) / 2) as i32;
    Rect::new(rect.x + dx, rect.y + dy, width, height)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_known_key_uses_the_shipped_resource_label() {
        // The embedded catalogue is loaded on first use, so this is the resource-table path:
        // an explicitly different English argument must not leak past a known key.
        assert_eq!(localized_label("common.button.ok", "Wrong"), "OK");
    }

    #[test]
    fn an_unknown_key_falls_back_to_the_callers_english() {
        assert_eq!(localized_label("no.such.dialog.key", "Custom"), "Custom");
    }

    #[test]
    fn the_resource_table_covers_every_shipped_key_with_a_label() {
        for (key, label) in DIALOG_RESOURCE_FALLBACKS {
            assert!(!key.is_empty(), "a resource key cannot be empty");
            assert!(!label.is_empty(), "{key} must have a label");
            assert_eq!(resource_fallback(key), Some(*label));
        }
    }
}
