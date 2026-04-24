//! TextEdit renderer.
use crate::core::{Color, Font, Point};
use crate::render::RenderContext;
use crate::widget::{TextEdit, Widget};
/// TextEdit renderer.
pub struct TextEditRenderer;
impl TextEditRenderer {
    /// Draw a text edit.
    pub fn draw(context: &mut RenderContext, text_edit: &TextEdit) {
        let rect = text_edit.geometry();
        context.fill_rect(rect, Color::from_rgb(255, 255, 255));
        context.draw_rect(rect, Color::from_rgb(160, 160, 160));
        let text = text_edit.text();
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
