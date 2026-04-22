//! ListBox renderer facade.

use crate::core::{Color, Font, Point};
use crate::render::RenderContext;
use crate::widget::ListBox;

/// Lightweight list box renderer.
pub struct ListBoxRenderer;

impl ListBoxRenderer {
    /// Draw a minimal list box representation.
    pub fn draw(context: &mut RenderContext, list_box: &ListBox) {
        let rect = list_box.geometry();
        context.fill_rect(rect, Color::rgba(255, 255, 255, 255));
        context.draw_rect(rect, Color::rgba(160, 160, 160, 255));

        if let Some(item) = list_box.current_item() {
            context.draw_text(
                Point::new(rect.x + 6, rect.y + 6),
                item,
                &Font::default_ui(),
                Color::rgba(26, 28, 32, 255),
            );
        }
    }
}
