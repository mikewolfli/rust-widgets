// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Shared drawing helpers used by display widgets.

/// Shared line drawing helper used by both `Line` and `Divider` widgets.
///
/// Draws a horizontal or vertical line centred within the given rectangle.
///
/// # Why the chord is inset by half the thickness
///
/// A stroke is centred on the chord it follows, so a `thickness` px line reaches
/// `thickness / 2` px on **each** side of it. A thickness-2 line laid on `rect.y` therefore
/// painted one of its two rows at `rect.y - 1`, outside the widget: `Line` showed as a row
/// straddling its own top edge, and the raster backend hid the defect by clipping it. The
/// chord is inset so the painted band — not the chord — is what fits.
///
/// The length is inset for the same reason at the ends: `Rect`'s far edge is exclusive
/// (`width` px spans `x .. x + width - 1`), so a chord running to `rect.x + width - 1`
/// already reaches the last pixel, and half a thickness beyond it would leave the box.
pub(crate) fn draw_line(
    context: &mut crate::render::RenderContext,
    rect: crate::core::Rect,
    vertical: bool,
    thickness: u32,
    color: crate::core::Color,
) {
    if rect.width == 0 || rect.height == 0 {
        return;
    }
    let thickness = thickness.max(1);
    let half = (thickness / 2) as i32;
    if vertical {
        let x =
            rect.x + (rect.width as i32 / 2).clamp(half, (rect.width as i32 - 1 - half).max(half));
        let top = rect.y + half;
        let bottom = (rect.y + rect.height as i32 - 1 - half).max(top);
        context.draw_line_stroke(
            crate::core::Point::new(x, top),
            crate::core::Point::new(x, bottom),
            color,
            thickness,
        );
    } else {
        let y = rect.y
            + (rect.height as i32 / 2).clamp(half, (rect.height as i32 - 1 - half).max(half));
        let left = rect.x + half;
        let right = (rect.x + rect.width as i32 - 1 - half).max(left);
        context.draw_line_stroke(
            crate::core::Point::new(left, y),
            crate::core::Point::new(right, y),
            color,
            thickness,
        );
    }
}

/// Paints a two-line sunken/raised separator.
///
/// Both `Line` and `Divider` publish the same three-state shading vocabulary, so the treatment
/// has one implementation: two adjacent 1 px bands centred in `rect`, with `sunken` deciding
/// which edge is dark. The derived highlight and shadow come from `color`, so a themed line and a
/// themed divider move together. A control that is one pixel thick in the cross axis has no room
/// for two rows and paints the leading band, so the style still has visible ink.
pub(crate) fn draw_shaded_line(
    context: &mut crate::render::RenderContext,
    rect: crate::core::Rect,
    vertical: bool,
    color: crate::core::Color,
    sunken: bool,
) {
    if rect.width == 0 || rect.height == 0 {
        return;
    }
    let highlight = color.blend(&crate::core::Color::WHITE, 0.45);
    let shadow = color.blend(&crate::core::Color::BLACK, 0.35);
    let (first, second) = if sunken { (shadow, highlight) } else { (highlight, shadow) };

    let extent = if vertical { rect.width } else { rect.height };
    if extent < 2 {
        draw_line(context, rect, vertical, 1, first);
        return;
    }

    if vertical {
        let middle = rect.x + rect.width as i32 / 2;
        draw_line(
            context,
            crate::core::Rect::new(middle - 1, rect.y, 1, rect.height),
            true,
            1,
            first,
        );
        draw_line(context, crate::core::Rect::new(middle, rect.y, 1, rect.height), true, 1, second);
    } else {
        let middle = rect.y + rect.height as i32 / 2;
        draw_line(
            context,
            crate::core::Rect::new(rect.x, middle - 1, rect.width, 1),
            false,
            1,
            first,
        );
        draw_line(context, crate::core::Rect::new(rect.x, middle, rect.width, 1), false, 1, second);
    }
}
