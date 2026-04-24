//! Checkbox rendering implementation.
use crate::core::{Color, Font, Point, Rect};
use crate::render::RenderContext;
use crate::widget::CheckState;
/// Checkbox renderer.
pub struct CheckBoxRenderer;
impl CheckBoxRenderer {
    /// Render a checkbox.
    pub fn draw(
        context: &mut RenderContext,
        rect: Rect,
        state: CheckState,
        enabled: bool,
        label: &str,
    ) {
        let checkbox_size = 16u32;
        let cb_x = rect.x;
        let cb_y = rect.y + ((rect.height as i32) - checkbox_size as i32) / 2;
        // Background
        let bg = if !enabled {
            Color::from_rgb(240, 240, 240)
        } else {
            Color::from_rgb(255, 255, 255)
        };
        context.fill_rect(Rect::new(cb_x, cb_y, checkbox_size, checkbox_size), bg);
        // Border
        let border = if !enabled {
            Color::from_rgb(180, 180, 180)
        } else {
            Color::from_rgb(100, 100, 100)
        };
        context.draw_rect(Rect::new(cb_x, cb_y, checkbox_size, checkbox_size), border);
        // Checkmark
        let check_color = if !enabled {
            Color::from_rgb(150, 150, 150)
        } else {
            Color::from_rgb(0, 0, 0)
        };
        match state {
            CheckState::Checked => {
                // Simple checkmark
                let cx = cb_x + (checkbox_size as i32) / 2;
                let cy = cb_y + (checkbox_size as i32) / 2;
                let s = 3i32;
                let from = Point::new(cx - s, cy);
                let default_font = Font::default();
                context.draw_text(from, "✓", &default_font, check_color);
            }
            CheckState::PartiallyChecked => {
                // Horizontal line
                let cy = cb_y + (checkbox_size as i32) / 2;
                context.draw_rect(
                    Rect::new(cb_x + 2, cy - 1, checkbox_size - 4, 2),
                    check_color,
                );
            }
            CheckState::Unchecked => {}
        }
        // Label
        if !label.is_empty() {
            let label_x = cb_x + checkbox_size as i32 + 6;
            let label_y = cb_y + (checkbox_size as i32) / 2 + 4;
            let label_color = if !enabled {
                Color::from_rgb(150, 150, 150)
            } else {
                Color::from_rgb(0, 0, 0)
            };
            context.draw_text(
                Point::new(label_x, label_y),
                label,
                &Font::default(),
                label_color,
            );
        }
    }
}
