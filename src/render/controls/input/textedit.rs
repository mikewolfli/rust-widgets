//! TextEdit renderer facade.
use crate::core::{Color, Font, Point};
use crate::render::RenderContext;
use crate::widget::TextEdit;
/// Lightweight text edit renderer.
pub struct TextEditRenderer;
impl TextEditRenderer {
    /// Draw a minimal text edit representation.
    pub fn draw(context: &mut RenderContext, text_edit: &TextEdit) {
        let rect = text_edit.geometry();
        context.fill_rect(Rect::new(rect, Color::rgba(255, 255, 255), 255));
        context.draw_rect(Rect::new(rect, Color::rgba(160, 160, 160), 255));
        let text = text_edit.text();
        if !text.is_empty() {
            context.draw_text(
                Point::new(rect.x + 6, rect.y + 6),
                text,
                &Font::default_ui(),
                Color::rgba(26, 28, 32, 255),
            );
        }
    }
}
