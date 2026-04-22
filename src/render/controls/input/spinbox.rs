//! SpinBox renderer facade.

use crate::core::{Color, Font, Point, Rect};
use crate::render::RenderContext;
use crate::widget::SpinBox;

/// Lightweight spin box renderer.
pub struct SpinBoxRenderer;

impl SpinBoxRenderer {
    /// Draw a minimal spin box representation.
    pub fn draw(context: &mut RenderContext, spin_box: &SpinBox) {
        let rect = spin_box.geometry();
        context.fill_rect(rect, Color::rgba(255, 255, 255, 255));
        context.draw_rect(rect, Color::rgba(160, 160, 160, 255));

        context.draw_text(
            Point::new(rect.x + 6, rect.y + 6),
            &spin_box.value().to_string(),
            &Font::default_ui(),
            Color::rgba(26, 28, 32, 255),
        );

        let control_rect = Rect::new(rect.right() - 18, rect.y, 18, rect.height);
        context.draw_rect(control_rect, Color::rgba(190, 190, 190, 255));
    }
}
