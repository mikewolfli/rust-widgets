//! Button rendering implementation.
use crate::core::{Color, Font, Point};
use crate::render::RenderContext;
use crate::widget::{Button, ButtonState, Widget};
/// Button renderer.
pub struct ButtonRenderer;
impl ButtonRenderer {
    /// Render a button.
    pub fn draw(context: &mut RenderContext, button: &Button) {
        let rect = button.geometry();
        let state = button.state();
        let text = button.text();
        // Background
        let bg = match state {
            ButtonState::Normal => Color::from_rgb(240, 240, 240),
            ButtonState::Pressed => Color::from_rgb(200, 200, 200),
            ButtonState::Disabled => Color::from_rgb(220, 220, 220),
        };
        context.fill_rect(rect, bg);
        // Border
        let border = match state {
            ButtonState::Normal => Color::from_rgb(180, 180, 180),
            ButtonState::Pressed => Color::from_rgb(150, 150, 150),
            ButtonState::Disabled => Color::from_rgb(200, 200, 200),
        };
        context.draw_rect(rect, border);
        // Text
        if !text.is_empty() {
            let text_color = match state {
                ButtonState::Disabled => Color::from_rgb(150, 150, 150),
                _ => Color::from_rgb(0, 0, 0),
            };
            let origin = Point::new(rect.x + 8, rect.y + (rect.height as i32) / 2 + 4);
            context.draw_text(origin, text, &Font::default(), text_color);
        }
    }
}
