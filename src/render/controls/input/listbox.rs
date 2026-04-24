//! ListBox renderer.
use crate::core::{Color, Font, Point};
use crate::render::RenderContext;
use crate::widget::{ListBox, Widget};
/// ListBox renderer.
pub struct ListBoxRenderer;
impl ListBoxRenderer {
    /// Draw a list box.
    pub fn draw(context: &mut RenderContext, list_box: &ListBox) {
        let rect = list_box.geometry();
        context.fill_rect(rect, Color::from_rgb(255, 255, 255));
        context.draw_rect(rect, Color::from_rgb(160, 160, 160));
        if let Some(idx) = list_box.current_row() {
            if let Some(item) = list_box.item(idx) {
                context.draw_text(
                    Point::new(rect.x + 6, rect.y + 6),
                    item,
                    &Font::default(),
                    Color::from_rgb(26, 28, 32),
                );
            }
        }
    }
}
