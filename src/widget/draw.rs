// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Custom drawing trait for widgets that want to render their own content.

use crate::core::{Color, Rect};
use crate::render::RenderContext;

/// Strokes a control's **surface** border, following the same corner radii as its fill.
///
/// # Why this exists
///
/// A field that fills itself with `border_radius` but strokes a square outline leaves the four
/// corners showing the backdrop between the rounded fill and the square rule — the "corners have no
/// line" defect. Every control that paints a themed surface goes through this one call, so the fill
/// and the outline cannot disagree about the shape again.
///
/// An all-zero radius keeps the historical square call (including its 1 px `draw_rect` form); any
/// non-zero radius strokes a rounded rectangle with the same width — per corner since STY-01, so a
/// non-uniform fill shape gets the matching outline instead of a silently square one.
///
/// A **zero width paints nothing**. Both paths used to force at least one pixel (the square path
/// fell back to `draw_rect` and the rounded path to `width.max(1)`), so `set_border_width(0)` with
/// a border colour still painted a 1 px outline — contradicting the documented "border explicitly
/// drawn with no thickness" and, because `LineEdit` passed `unwrap_or(0)`, painting a border the
/// caller had not configured at all.
///
/// Public because the `rust-widgets-suites` package paints its controls with the same helper: a
/// suite that re-implemented the fill/outline pairing would disagree with this crate about the
/// rounded-corner rule on the first control it moved.
pub fn stroke_surface_border(
    context: &mut RenderContext,
    rect: Rect,
    radii: [u32; 4],
    color: Color,
    width: u32,
) {
    if width == 0 {
        return;
    }
    if radii == [0, 0, 0, 0] {
        if width > 1 {
            context.draw_rect_stroke(rect, color, width);
        } else {
            context.draw_rect(rect, color);
        }
    } else {
        context.draw_rounded_rect_stroke_corners(rect, radii, color, width);
    }
}

/// Resolves the tail of the documented border ladder into the colour a dialog's frame is
/// stroked with.
///
/// The documented precedence is "explicit style → theme style → theme palette → literal"
/// (the "Semantics are unchanged" section of `container_widgets/theme_snapshot.rs`'s module
/// docs). The theme half of the ladder differs per control — most dialogs look up a resolved
/// widget style, the progress dialog reads the palette's `outline` token — so the caller
/// performs its own one-line lookup and passes both resolved rungs in; this helper owns the
/// shared tail. A resolved border equal to the `surface` would paint the frame
/// indistinguishable from its fill, so it is discarded, and the literal rung derives one
/// visible step away from the surface: a 45% blend toward `ink`.
///
/// # Drift
///
/// Hand copies of this ladder have drifted: `find_replace_dialog` blends 0.25, `popover` and
/// the sheet/popup family blend 0.35, and `alert_dialogs`/`file_dialog`/`font_dialog` fall
/// back to a literal without the surface-equality filter. They keep their behaviour until a
/// dedicated pass migrates them; this form is the canonical 0.45 ladder the dialogs agree on.
/// The shared tail of the border-colour ladder (theme_snapshot.rs:23-26).
///
/// `explicit border → themed border → (≠ surface) → surface-blend ink`. The five dialog
/// surfaces that hand-copied this ladder converged here; the blend step is the ladder's
/// 0.45 contrast tone. Callers resolve their own theme half because it differs per site.
/// `full_widgets`-gated: every caller is a dialog, and the stripped profiles compile
/// neither them nor this helper.
#[cfg(full_widgets)]
pub(crate) fn border_ink(
    style_border: Option<Color>,
    themed_border: Option<Color>,
    surface: Color,
    ink: Color,
) -> Color {
    style_border
        .or(themed_border)
        .filter(|resolved| *resolved != surface)
        .unwrap_or_else(|| surface.blend(&ink, 0.45))
}

/// Custom drawing trait for widgets that want to render their own content.
/// Widgets implementing this trait can provide custom drawing logic instead of
/// relying solely on platform rendering.
pub trait Draw {
    /// Draw the widget's content using the provided render context.
    /// This method is called when the widget needs to be repainted.
    fn draw(&mut self, context: &mut RenderContext);
    /// Returns true if this widget uses custom drawing, false otherwise.
    /// This allows the rendering system to choose between the two paths.
    /// Defaults to `false` because most widgets map onto a platform control.
    fn uses_custom_drawing(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Size;
    use crate::render::{PaintBackend, SoftwarePaintBackend};

    /// Counts pixels the helper actually painted onto a transparent surface.
    fn painted_pixels(radii: [u32; 4], width: u32) -> usize {
        let mut backend = SoftwarePaintBackend::new(Size::new(24, 24), 1.0);
        backend.begin_frame(Color::TRANSPARENT);
        {
            let mut context = RenderContext::new(&mut backend);
            stroke_surface_border(
                &mut context,
                Rect::new(2, 2, 20, 20),
                radii,
                Color::BLACK,
                width,
            );
        }
        backend.end_frame();
        let (pixels, _) = backend.frame_rgba().as_chunks::<4>();
        pixels.iter().filter(|pixel| pixel[3] != 0).count()
    }

    /// A border declared with zero thickness paints nothing, on either path.
    ///
    /// # The defect this pins
    ///
    /// The square path fell through to `draw_rect` (a 1 px stroke) and the rounded path forced
    /// `width.max(1)`, so `set_border_width(0)` with a border colour still painted a 1 px outline.
    /// `LineEdit` passes `unwrap_or(0)` for "no width configured", which made the unconfigured
    /// case paint a border too. The assertion that a width of 1 still paints is the control: it
    /// proves the probe can see this helper's output rather than passing vacuously.
    #[test]
    fn a_zero_width_border_paints_nothing() {
        assert_eq!(painted_pixels([0, 0, 0, 0], 0), 0, "the square path must paint nothing");
        assert_eq!(painted_pixels([4, 4, 4, 4], 0), 0, "the rounded path must paint nothing");
        assert!(painted_pixels([0, 0, 0, 0], 1) > 0, "a 1 px square outline still paints");
        assert!(painted_pixels([4, 4, 4, 4], 1) > 0, "a 1 px rounded outline still paints");
    }
}
