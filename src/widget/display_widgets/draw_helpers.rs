//! Shared drawing helpers used by display widgets.

/// Shared line drawing helper used by both `Line` and `Divider` widgets.
/// Draws a horizontal or vertical line centered within the given rectangle.
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
    if vertical {
        let x = rect.x + (rect.width as i32 / 2);
        let top = rect.y;
        let bottom = rect.y + (rect.height as i32).saturating_sub(1);
        context.draw_line_stroke(
            crate::core::Point::new(x, top),
            crate::core::Point::new(x, bottom),
            color,
            thickness,
        );
    } else {
        let y = rect.y + (rect.height as i32 / 2);
        let left = rect.x;
        let right = rect.x + (rect.width as i32).saturating_sub(1);
        context.draw_line_stroke(
            crate::core::Point::new(left, y),
            crate::core::Point::new(right, y),
            color,
            thickness,
        );
    }
}
