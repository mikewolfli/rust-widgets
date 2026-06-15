//! Display widgets: progress bars, sliders, scroll bars, etc.
pub mod arc;
#[cfg(not(feature = "mini"))]
pub mod badge;
#[cfg(not(feature = "mini"))]
pub mod color_history;
#[cfg(not(feature = "mini"))]
pub mod color_well;
#[cfg(not(feature = "mini"))]
pub mod divider;
#[cfg(not(feature = "mini"))]
pub mod empty_state;
#[cfg(not(feature = "mini"))]
pub mod floating_label;
#[cfg(not(feature = "mini"))]
pub mod font_preview;
#[cfg(not(feature = "mini"))]
pub mod icon;
pub mod image_view;
#[cfg(not(feature = "mini"))]
pub mod lcd_number;
pub mod line;
pub mod meter;
pub mod mini_canvas;
pub mod mini_chart;
#[cfg(not(feature = "mini"))]
pub mod progress_circle;
pub mod progressbar;
#[cfg(not(feature = "mini"))]
pub mod rating;
pub mod roller;
pub mod scrollbar;
#[cfg(not(feature = "mini"))]
pub mod skeleton_loader;
pub mod slider;
pub mod spinner;
pub mod switch;

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
