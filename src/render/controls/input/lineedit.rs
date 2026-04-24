//! LineEdit renderer.
use crate::core::{Color, Font, Point};
use crate::render::RenderContext;
use crate::widget::{LineEdit, Widget};
/// LineEdit renderer.
pub struct LineEditRenderer;
impl LineEditRenderer {
    /// Draw a line edit.
    pub fn draw(context: &mut RenderContext, line_edit: &LineEdit) {
        let rect = line_edit.geometry();
        context.fill_rect(rect, Color::from_rgb(255, 255, 255));
        context.draw_rect(rect, Color::from_rgb(160, 160, 160));
        let text = line_edit.text();
        if !text.is_empty() {
            context.draw_text(
                Point::new(rect.x + 6, rect.y + 6),
                &text,
                &Font::default(),
                Color::from_rgb(26, 28, 32),
            );
        }
    }
}
