//! ComboBox renderer.
use crate::core::{Color, Font, Point, Rect};
use crate::render::RenderContext;
use crate::widget::{ComboBox, Widget};
/// ComboBox renderer.
pub struct ComboBoxRenderer;
impl ComboBoxRenderer {
    /// Draw a combobox.
    pub fn draw(context: &mut RenderContext, combo_box: &ComboBox) {
        let rect = combo_box.geometry();
        context.fill_rect(rect, Color::from_rgb(255, 255, 255));
        context.draw_rect(rect, Color::from_rgb(160, 160, 160));
        let text = combo_box.current_text();
        if !text.is_empty() {
            context.draw_text(
                Point::new(rect.x + 6, rect.y + 6),
                &text,
                &Font::default(),
                Color::from_rgb(26, 28, 32),
            );
        }
        let arrow_rect = Rect::new(rect.right() - 18, rect.y, 18, rect.height);
        context.draw_rect(arrow_rect, Color::from_rgb(190, 190, 190));
    }
}
