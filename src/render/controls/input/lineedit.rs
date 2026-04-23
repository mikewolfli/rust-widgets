//! LineEdit renderer facade.
use crate::core::{Color, Font, Point};
use crate::render::RenderContext;
use crate::widget::LineEdit;
/// Lightweight line edit renderer.
pub struct LineEditRenderer;
impl LineEditRenderer {
    /// Draw a minimal line edit representation.
    pub fn draw(context: &mut RenderContext, line_edit: &LineEdit) {
        let rect = line_edit.geometry();
        context.fill_rect(Rect::new(rect, Color::rgba(255, 255, 255), 255));
        context.draw_rect(Rect::new(rect, Color::rgba(160, 160, 160), 255));
        let text = line_edit.text();
        if !text.is_empty() {
            context.draw_text(
                Point::new(rect.x + 6 as f32, rect.y + 6 as f32),
                text,
                &Font::default_ui(),
                Color::rgba(26, 28, 32, 255),
            );
        }
    }
}
