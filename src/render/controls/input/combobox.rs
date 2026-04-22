//! ComboBox renderer facade.

use crate::core::{Color, Font, Point, Rect};
use crate::render::RenderContext;
use crate::widget::ComboBox;

/// Lightweight combobox renderer.
pub struct ComboBoxRenderer;

impl ComboBoxRenderer {
    /// Draw a minimal combobox representation.
    pub fn draw(context: &mut RenderContext, combo_box: &ComboBox) {
        let rect = combo_box.geometry();
        context.fill_rect(rect, Color::rgba(255, 255, 255, 255));
        context.draw_rect(rect, Color::rgba(160, 160, 160, 255));

        let text = combo_box.current_text();
        if !text.is_empty() {
            context.draw_text(
                Point::new(rect.x + 6, rect.y + 6),
                text,
                &Font::default_ui(),
                Color::rgba(26, 28, 32, 255),
            );
        }

        let arrow_rect = Rect::new(rect.right() - 18, rect.y, 18, rect.height);
        context.draw_rect(arrow_rect, Color::rgba(190, 190, 190, 255));
    }
}
