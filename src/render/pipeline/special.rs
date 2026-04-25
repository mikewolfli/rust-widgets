//! Special widgets: command link, font combo box, LCD number.
//!
//! These were migrated from `render/controls/special/` and adapted to use
//! the pipeline's shared `push_widget_fill_and_border` helper.

use crate::core::{Color, Point, Rect};
use crate::render::{RenderCommand, SceneLayer};
use crate::widget::{CommandLink, FontComboBox, LcdNumber, Widget};

use super::controls::push_widget_fill_and_border;

/// Append visual commands for a `CommandLink` baseline representation.
#[allow(dead_code)]
pub fn append_command_link_visual_commands(layer: &mut SceneLayer, command_link: &CommandLink) {
    push_widget_fill_and_border(
        layer,
        command_link,
        Some(Color::BACKGROUND),
        Some((Color::SECONDARY, 1)),
    );
    let rect = command_link.geometry();
    if rect.width > 16 && rect.height > 12 {
        layer.push(RenderCommand::DrawText {
            origin: Point {
                x: rect.x + 8,
                y: rect.y + 4,
            },
            text: "CommandLink".to_string(),
            font: command_link.font().cloned().unwrap_or_default(),
            color: command_link.foreground_color().unwrap_or(Color::FOREGROUND),
        });
        if rect.height > 30 {
            // Draw command link button
            layer.push(RenderCommand::FillRect {
                rect: Rect {
                    x: rect.x + 8,
                    y: rect.y + 24,
                    width: rect.width - 16,
                    height: 40,
                },
                color: Color::PRIMARY,
            });
            // Draw command link text
            layer.push(RenderCommand::DrawText {
                origin: Point {
                    x: rect.x + 24,
                    y: rect.y + 48,
                },
                text: command_link.text().to_string(),
                font: command_link.font().cloned().unwrap_or_default(),
                color: Color::WHITE,
            });
            // Draw description text
            if rect.height > 70 && !command_link.description().is_empty() {
                layer.push(RenderCommand::DrawText {
                    origin: Point {
                        x: rect.x + 24,
                        y: rect.y + 72,
                    },
                    text: command_link.description().to_string(),
                    font: command_link.font().cloned().unwrap_or_default(),
                    color: command_link.foreground_color().unwrap_or(Color::FOREGROUND),
                });
            }
        }
    }
}

/// Append visual commands for a `FontComboBox` baseline representation.
#[allow(dead_code)]
pub fn append_font_combo_box_visual_commands(
    layer: &mut SceneLayer,
    font_combo_box: &FontComboBox,
) {
    push_widget_fill_and_border(
        layer,
        font_combo_box,
        Some(Color::BACKGROUND),
        Some((Color::SECONDARY, 1)),
    );
    let rect = font_combo_box.geometry();
    if rect.width > 16 && rect.height > 12 {
        layer.push(RenderCommand::DrawText {
            origin: Point {
                x: rect.x + 8,
                y: rect.y + 4,
            },
            text: "FontComboBox".to_string(),
            font: font_combo_box.font().cloned().unwrap_or_default(),
            color: font_combo_box
                .foreground_color()
                .unwrap_or(Color::FOREGROUND),
        });
        if rect.height > 30 {
            // Draw combo box field
            layer.push(RenderCommand::FillRect {
                rect: Rect {
                    x: rect.x + 8,
                    y: rect.y + 24,
                    width: rect.width - 16,
                    height: 28,
                },
                color: Color::WHITE,
            });
            layer.push(RenderCommand::DrawRectStroke {
                rect: Rect {
                    x: rect.x + 8,
                    y: rect.y + 24,
                    width: rect.width - 16,
                    height: 28,
                },
                color: Color::SECONDARY,
                width: 1,
            });
            // Draw current font name
            let current_text = font_combo_box.current_text();
            let display_text = if current_text.is_empty() {
                "Select Font...".to_string()
            } else {
                current_text
            };
            layer.push(RenderCommand::DrawText {
                origin: Point {
                    x: rect.x + 16,
                    y: rect.y + 44,
                },
                text: display_text,
                font: font_combo_box.font().cloned().unwrap_or_default(),
                color: font_combo_box
                    .foreground_color()
                    .unwrap_or(Color::FOREGROUND),
            });
            // Draw dropdown button
            let button_width = 24u32;
            layer.push(RenderCommand::FillRect {
                rect: Rect {
                    x: rect.x + rect.width as i32 - button_width as i32 - 8,
                    y: rect.y + 24,
                    width: button_width,
                    height: 28,
                },
                color: Color::BACKGROUND,
            });
            layer.push(RenderCommand::DrawRectStroke {
                rect: Rect {
                    x: rect.x + rect.width as i32 - button_width as i32 - 8,
                    y: rect.y + 24,
                    width: button_width,
                    height: 28,
                },
                color: Color::SECONDARY,
                width: 1,
            });
            // Draw dropdown arrow
            let arrow_center_x = rect.x + rect.width as i32 - button_width as i32 / 2 - 8;
            let arrow_center_y = rect.y + 38;
            let arrow_size = 4;
            layer.push(RenderCommand::DrawLineStroke {
                from: Point {
                    x: arrow_center_x - arrow_size,
                    y: arrow_center_y - arrow_size / 2,
                },
                to: Point {
                    x: arrow_center_x,
                    y: arrow_center_y + arrow_size / 2,
                },
                color: Color::FOREGROUND,
                width: 1,
            });
            layer.push(RenderCommand::DrawLineStroke {
                from: Point {
                    x: arrow_center_x,
                    y: arrow_center_y + arrow_size / 2,
                },
                to: Point {
                    x: arrow_center_x + arrow_size,
                    y: arrow_center_y - arrow_size / 2,
                },
                color: Color::FOREGROUND,
                width: 1,
            });
            // Draw sample font list
            if rect.height > 70 {
                let sample_fonts = vec!["Arial", "Times New Roman", "Courier New", "Helvetica"];
                for (i, font_name) in sample_fonts.iter().enumerate() {
                    let y = rect.y + 60 + (i as i32 * 24);
                    if y + 20 < rect.y + rect.height as i32 {
                        layer.push(RenderCommand::FillRect {
                            rect: Rect {
                                x: rect.x + 8,
                                y,
                                width: rect.width - 16,
                                height: 24,
                            },
                            color: if i % 2 == 0 {
                                Color::BACKGROUND
                            } else {
                                Color::WHITE
                            },
                        });
                        layer.push(RenderCommand::DrawText {
                            origin: Point {
                                x: rect.x + 16,
                                y: y + 16,
                            },
                            text: font_name.to_string(),
                            font: font_combo_box.font().cloned().unwrap_or_default(),
                            color: font_combo_box
                                .foreground_color()
                                .unwrap_or(Color::FOREGROUND),
                        });
                    }
                }
            }
        }
    }
}

/// Append visual commands for an `LCDNumber` baseline representation.
#[allow(dead_code)]
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
