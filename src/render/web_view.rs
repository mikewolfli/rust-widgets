use crate::core::{Color, Point, Rect};
use crate::render::{push_widget_fill_and_border, RenderCommand, SceneLayer};
use crate::widget::{web_view::WebView, Widget};
/// Append visual commands for a `WebView` baseline representation.
pub fn append_web_view_visual_commands(layer: &mut SceneLayer, web_view: &WebView) {
    push_widget_fill_and_border(
        layer,
        web_view,
        Some(Color::rgba(240, 240, 240, 255)),
        Some((Color::SECONDARY, 1)),
    );
    let rect = web_view.geometry();
    if rect.width > 16 && rect.height > 12 {
        // Draw address bar
        layer.push(RenderCommand::FillRect {
            rect: Rect {
                x: rect.x + 8,
                y: rect.y + 8,
                width: rect.width - 16,
                height: 24,
            },
            color: Color::rgba(220, 220, 220, 255),
        });
        layer.push(RenderCommand::DrawRectStroke {
            rect: Rect {
                x: rect.x + 8,
                y: rect.y + 8,
                width: rect.width - 16,
                height: 24,
            },
            color: Color::SECONDARY,
            width: 1,
        });
        // Draw URL text
        layer.push(RenderCommand::DrawText {
            origin: Point {
                x: rect.x + 16,
                y: rect.y + 24,
            },
            text: web_view.url().to_string(),
            font: web_view.font().cloned().unwrap_or_default(),
            color: Color::FOREGROUND,
        });
        // Draw web content area
        layer.push(RenderCommand::FillRect {
            rect: Rect {
                x: rect.x + 8,
                y: rect.y + 40,
                width: rect.width - 16,
                height: rect.height - 48,
            },
            color: Color::WHITE,
        });
        layer.push(RenderCommand::DrawRectStroke {
            rect: Rect {
                x: rect.x + 8,
                y: rect.y + 40,
                width: rect.width - 16,
                height: rect.height - 48,
            },
            color: Color::SECONDARY,
            width: 1,
        });
        // Draw sample web content
        let content_text = if web_view.title().is_empty() {
            "WebView Content".to_string()
        } else {
            web_view.title().to_string()
        };
        layer.push(RenderCommand::DrawText {
            origin: Point {
                x: rect.x + 24,
                y: rect.y + 60,
            },
            text: content_text,
            font: web_view.font().cloned().unwrap_or_default(),
            color: Color::FOREGROUND,
        });
        layer.push(RenderCommand::DrawText {
            origin: Point {
                x: rect.x + 24,
                y: rect.y + 80,
            },
            text: "URL: ".to_string() + web_view.url(),
            font: web_view.font().cloned().unwrap_or_default(),
            color: Color::rgba(0, 0, 128, 255),
        });
        // Draw loading indicator if loading
        if web_view.is_loading() {
            layer.push(RenderCommand::DrawText {
                origin: Point {
                    x: rect.x + rect.width as i32 as i32 - 80,
                    y: rect.y + 24,
                },
                text: "Loading...".to_string(),
                font: web_view.font().cloned().unwrap_or_default(),
                color: Color::rgba(0, 128, 0, 255),
            });
        }
    }
}
