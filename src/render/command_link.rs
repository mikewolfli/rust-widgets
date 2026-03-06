use crate::core::{Color, Point, Rect};
use crate::render::{RenderCommand, SceneLayer};
use crate::widget::{command_link::CommandLink, Widget};

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

/// Append visual commands for a `CommandLink` baseline representation.
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
