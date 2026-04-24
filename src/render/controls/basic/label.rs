//! Label rendering implementation.
use crate::core::{Alignment, Color, Font, Point};
use crate::render::RenderContext;
use crate::widget::{Label, Widget};
/// Label renderer.
pub struct LabelRenderer;
impl LabelRenderer {
    /// Render a label.
    pub fn draw(context: &mut RenderContext, label: &Label) {
        let rect = label.geometry();
        let text = label.text();
        let alignment = label.alignment();
        if text.is_empty() {
            return;
        }
        let font = Font::default();
        let text_color = Color::from_rgb(0, 0, 0);
        let origin = match alignment {
            Alignment::Left => Point::new(rect.x + 2, rect.y + (rect.height as i32) / 2 + 4),
            Alignment::Center => {
                let metrics = context.measure_text(text, &font);
                let x = rect.x + ((rect.width as i32) - metrics.width as i32) / 2;
                Point::new(x, rect.y + (rect.height as i32) / 2 + 4)
            }
            Alignment::Right => {
                let metrics = context.measure_text(text, &font);
                let x = rect.x + (rect.width as i32) - metrics.width as i32 - 2;
                Point::new(x, rect.y + (rect.height as i32) / 2 + 4)
            }
            Alignment::Top => Point::new(rect.x + 2, rect.y + 4),
            Alignment::Bottom => Point::new(rect.x + 2, rect.y + (rect.height as i32) - 4),
        };
        context.draw_text(origin, text, &font, text_color);
    }
}
