//! Radio button rendering.
use crate::core::{Color, Point, Rect};
use crate::render::RenderContext;
use crate::widget::RadioButton;
/// Radio button renderer.
pub struct RadioButtonRenderer;
impl RadioButtonRenderer {
    /// Render a radio button.
    pub fn render(context: &mut RenderContext, radio_button: &RadioButton, rect: Rect) {
        let center = Point::new(
            rect.x + (rect.width as i32) / 2,
            rect.y + (rect.height as i32) / 2,
        );
        let radius = (rect.height.min(rect.width) / 2) as u32;
        // Draw outer circle
        context.draw_circle(center, radius, Color::from_rgb(100, 100, 100));
        // Draw inner circle if checked
        if radio_button.is_checked() {
            let inner_radius = if radius > 3 { radius - 3 } else { 1 };
            context.fill_circle(center, inner_radius, Color::from_rgb(0, 120, 215));
        }
    }
}
