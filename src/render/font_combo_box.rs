use crate::core::{Color, Point, Rect};
use crate::render::{RenderCommand, SceneLayer};
use crate::widget::{font_combo_box::FontComboBox, Widget};

fn push_widget_fill_and_border(
    layer: &mut SceneLayer,
    widget: &dyn Widget,
    background: Option<Color>,
    border: Option<(Color, u32)>,
) {
    let rect = widget.geometry();
    if let Some(bg_color) = background {
        layer.push(RenderCommand::FillRect {
            rect,
            color: bg_color,
        });
    }
    if let Some((border_color, border_width)) = border {
        layer.push(RenderCommand::DrawRectStroke {
            rect,
            color: border_color,
            width: border_width,
        });
    }
}

/// Append visual commands for a `FontComboBox` baseline representation.
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
