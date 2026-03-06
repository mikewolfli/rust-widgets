use crate::core::{Color, Point, Rect};
use crate::render::{push_widget_fill_and_border, RenderCommand, SceneLayer};
use crate::widget::{lcd_number::LCDNumber, Widget};

/// Append visual commands for an `LCDNumber` baseline representation.
pub fn append_lcd_number_visual_commands(layer: &mut SceneLayer, lcd_number: &LCDNumber) {
    push_widget_fill_and_border(
        layer,
        lcd_number,
        Some(Color::BLACK),
        Some((Color::SECONDARY, 1)),
    );

    let rect = lcd_number.geometry();
    if rect.width > 16 && rect.height > 12 {
        layer.push(RenderCommand::DrawText {
            origin: Point {
                x: rect.x + 8,
                y: rect.y + 4,
            },
            text: "LCDNumber".to_string(),
            font: lcd_number.font().cloned().unwrap_or_default(),
            color: lcd_number.foreground_color().unwrap_or(Color::FOREGROUND),
        });

        if rect.height > 30 {
            // Draw LCD display area
            layer.push(RenderCommand::FillRect {
                rect: Rect {
                    x: rect.x + 8,
                    y: rect.y + 24,
                    width: rect.width - 16,
                    height: rect.height - 32,
                },
                color: Color::rgba(20, 40, 20, 255),
            });

            // Draw LCD border
            layer.push(RenderCommand::DrawRectStroke {
                rect: Rect {
                    x: rect.x + 8,
                    y: rect.y + 24,
                    width: rect.width - 16,
                    height: rect.height - 32,
                },
                color: Color::rgba(80, 100, 80, 255),
                width: 2,
            });

            // Draw sample LCD digits
            layer.push(RenderCommand::DrawText {
                origin: Point {
                    x: rect.x + 24,
                    y: rect.y + rect.height as i32 / 2 + 8,
                },
                text: "12:34:56".to_string(),
                font: lcd_number.font().cloned().unwrap_or_default(),
                color: Color::rgba(0, 255, 0, 255),
            });
        }
    }
}
