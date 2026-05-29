//! Special widgets: command link, font combo box, LCD number.
//!
//! These were migrated from `render/controls/special/` and adapted to use
//! the pipeline's shared `push_widget_fill_and_border` helper.

use crate::core::{Color, Point, Rect};
use crate::render::{RenderCommand, SceneLayer};
use crate::widget::{LcdNumber, Widget};

/// Append visual commands for an `LCDNumber` baseline representation.
#[allow(dead_code)]
#[deprecated(note = "Pipeline routing is unstable. Use RenderContext directly instead.")]
pub fn append_lcd_number_visual_commands(layer: &mut SceneLayer, lcd_number: &LcdNumber) {
    let rect = lcd_number.geometry();
    if rect.width > 16 && rect.height > 12 {
        layer.push(RenderCommand::DrawText {
            origin: Point::new(rect.x + 8, rect.y + 4),
            text: "LCDNumber".to_string(),
            font: lcd_number.font().cloned().unwrap_or_default(),
            color: lcd_number
                .foreground_color()
                .unwrap_or(Color::from_rgb(0, 0, 0)),
        });
        if rect.height > 30 {
            // Draw LCD display area
            layer.push(RenderCommand::FillRect {
                rect: Rect::new(rect.x + 8, rect.y + 24, rect.width - 16, rect.height - 32),
                color: Color::from_rgb(20, 40, 20),
            });
            // Draw LCD border
            layer.push(RenderCommand::DrawRectStroke {
                rect: Rect::new(rect.x + 8, rect.y + 24, rect.width - 16, rect.height - 32),
                color: Color::from_rgb(80, 100, 80),
                width: 2,
            });
            // Draw sample LCD digits
            layer.push(RenderCommand::DrawText {
                origin: Point::new(rect.x + 24, rect.y + (rect.height as i32) / 2 + 8),
                text: "12:34:56".to_string(),
                font: lcd_number.font().cloned().unwrap_or_default(),
                color: Color::from_rgb(0, 255, 0),
            });
        }
    }
}
