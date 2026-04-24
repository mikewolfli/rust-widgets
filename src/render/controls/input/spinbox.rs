//! SpinBox renderer.
use crate::core::{Color, Font, Point, Rect};
use crate::render::RenderContext;
use crate::widget::{SpinBox, Widget};
/// SpinBox renderer.
pub struct SpinBoxRenderer;
impl SpinBoxRenderer {
    /// Draw a spin box.
    pub fn draw(context: &mut RenderContext, spin_box: &SpinBox) {
        let rect = spin_box.geometry();
        context.fill_rect(rect, Color::from_rgb(255, 255, 255));
        context.draw_rect(rect, Color::from_rgb(160, 160, 160));
        context.draw_text(
            Point::new(rect.x + 6, rect.y + 6),
            &spin_box.value().to_string(),
            &Font::default(),
            Color::from_rgb(26, 28, 32),
        );
        let control_rect = Rect::new(rect.right() - 18, rect.y, 18, rect.height);
        context.draw_rect(control_rect, Color::from_rgb(190, 190, 190));
    }
}
