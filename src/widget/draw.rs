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
pub(crate) fn stroke_surface_border(
    context: &mut RenderContext,
    rect: Rect,
    radii: [u32; 4],
    color: Color,
    width: u32,
) {
    if radii == [0, 0, 0, 0] {
        if width > 1 {
            context.draw_rect_stroke(rect, color, width);
        } else {
            context.draw_rect(rect, color);
        }
    } else {
        context.draw_rounded_rect_stroke_corners(rect, radii, color, width.max(1));
    }
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
