//! Menu and toolbar widgets: menu_bar, menu, context_menu, tool_bar, status_bar.
use crate::core::{Color, Point, Rect};
use crate::render::{SceneLayer, RenderCommand};
use crate::render::is_empty_rect;
use crate::render::pipeline::controls::{push_widget_fill_and_border, centered_text_origin};
use crate::widget::{ContextMenu, Menu, MenuBar, StatusBar, ToolBar, Widget};

#[deprecated(note = "Pipeline routing is unstable. Use RenderContext directly instead.")]
pub fn append_menu_bar_visual_commands(layer: &mut SceneLayer, menu_bar: &MenuBar) {
    push_widget_fill_and_border(
        layer,
        menu_bar,
        Some(Color::rgba(238, 242, 248, 255)),
        Some((Color::rgba(128, 134, 144, 255), 1)),
    );
    let rect = menu_bar.geometry();
    if is_empty_rect(&rect) {
        return;
    }
    let menu_count = menu_bar.entries().len().max(1);
    let slot_width = (rect.width / menu_count as u32).max(20);
    for index in 0..menu_count {
        let slot_rect = Rect {
            x: rect.x + (index as u32 * slot_width) as i32,
            y: rect.y,
            width: slot_width.min(rect.width),
            height: rect.height,
        };
        if menu_bar.active_index().is_some() && index == 0 {
            layer.push(RenderCommand::FillRect {
                rect: slot_rect,
                color: Color::rgba(208, 224, 249, 255),
            });
        }
        let label = menu_bar
            .entries()
            .get(index)
            .map(|entry| entry.title.clone())
            .unwrap_or_else(|| format!("Menu{}", index + 1));
        layer.push(RenderCommand::DrawText {
            origin: centered_text_origin(slot_rect),
            text: label,
            font: menu_bar.font().cloned().unwrap_or_default(),
            color: menu_bar
                .foreground_color()
                .unwrap_or(Color::rgba(32, 34, 38, 255)),
        });
    }
}
/// Append visual commands for a `Menu` host representation.
#[deprecated(note = "Pipeline routing is unstable. Use RenderContext directly instead.")]
pub fn append_menu_visual_commands(layer: &mut SceneLayer, menu: &Menu) {
    push_widget_fill_and_border(
        layer,
        menu,
        Some(Color::rgba(250, 250, 251, 255)),
        Some((Color::rgba(124, 130, 140, 255), 1)),
    );
    let rect = menu.geometry();
    if is_empty_rect(&rect) {
        return;
    }
    // Draw title if present
    let mut content_offset = 0i32;
    if !menu.title().is_empty() {
        layer.push(RenderCommand::DrawText {
            origin: Point {
                x: rect.x + 8,
                y: rect.y + 4,
            },
            text: menu.title().to_string(),
            font: menu.font().cloned().unwrap_or_default(),
            color: menu
                .foreground_color()
                .unwrap_or(Color::rgba(22, 24, 30, 255)),
        });
        content_offset = 20;
    }
    let row_height = 24u32;
    let icon_width = 0i32;
    let shortcut_width = 60;
    for (index, item) in menu.items().iter().enumerate() {
        let row_y = rect.y + content_offset + (index as u32 * row_height) as i32;
        let row_rect = Rect {
            x: rect.x + 2,
            y: row_y,
            width: rect.width.saturating_sub(4),
            height: row_height,
        };
        // Draw selection highlight
        if Some(index) == menu.hovered_index() {
            layer.push(RenderCommand::FillRect {
                rect: row_rect,
                color: Color::rgba(208, 224, 249, 255),
            });
        }
        // Handle different item types
        if item.separator {
            // Draw separator line
            layer.push(RenderCommand::DrawLine {
                from: Point {
                    x: rect.x + 8,
                    y: row_y + (row_height / 2) as i32,
                },
                to: Point {
                    x: rect.x + rect.width as f32 as i32 - 8,
                    y: row_y + (row_height / 2) as i32,
                },
                color: Color::rgba(180, 186, 196, 255),
            });
        } else {
            // Draw checkmark for checkable items
            let mut text_offset_x = rect.x + 8;
            if item.checkable {
                if item.checked {
                    layer.push(RenderCommand::DrawText {
                        origin: Point {
                            x: text_offset_x,
                            y: row_y + 4,
                        },
                        text: "✓".to_string(),
                        font: menu.font().cloned().unwrap_or_default(),
                        color: Color::rgba(32, 34, 38, 255),
                    });
                }
                text_offset_x += 16;
            }
            text_offset_x += icon_width;
            // Draw item text
            let text_color = if item.enabled {
                menu.foreground_color()
                    .unwrap_or(Color::rgba(32, 34, 38, 255))
            } else {
                Color::rgba(128, 128, 128, 255)
            };
            layer.push(RenderCommand::DrawText {
                origin: Point {
                    x: text_offset_x,
                    y: row_y + 4,
                },
                text: item.text.clone(),
                font: menu.font().cloned().unwrap_or_default(),
                color: text_color,
            });
            // Draw shortcut if present
            if !item.shortcut.is_empty() {
                layer.push(RenderCommand::DrawText {
                    origin: Point {
                        x: rect.x + rect.width as f32 as i32 - shortcut_width,
                        y: row_y + 4,
                    },
                    text: item.shortcut.clone(),
                    font: menu.font().cloned().unwrap_or_default(),
                    color: Color::rgba(100, 100, 100, 255),
                });
            }
            // Draw submenu arrow
            if item.has_submenu {
                layer.push(RenderCommand::DrawText {
                    origin: Point {
                        x: rect.x + rect.width as f32 as i32 - 16,
                        y: row_y + 4,
                    },
                    text: "▶".to_string(),
                    font: menu.font().cloned().unwrap_or_default(),
                    color: Color::rgba(100, 100, 100, 255),
                });
            }
        }
    }
}
/// Append visual commands for a `ContextMenu` host representation.
/// Reuses the same rendering logic as Menu for consistency.
#[deprecated(note = "Pipeline routing is unstable. Use RenderContext directly instead.")]
pub fn append_context_menu_visual_commands(layer: &mut SceneLayer, context_menu: &ContextMenu) {
    push_widget_fill_and_border(
        layer,
        context_menu,
        Some(Color::rgba(250, 250, 251, 255)),
        Some((Color::rgba(124, 130, 140, 255), 1)),
    );
    let rect = context_menu.geometry();
    if is_empty_rect(&rect) {
        return;
    }
    let row_height = 24u32;
    let icon_width = 0i32;
    let shortcut_width = 60;
    for (index, item) in context_menu.items().iter().enumerate() {
        let row_y = rect.y + (index as u32 * row_height) as i32;
        let row_rect = Rect {
            x: rect.x + 2,
            y: row_y,
            width: rect.width.saturating_sub(4),
            height: row_height,
        };
        // Draw selection highlight
        if let Some(selected_idx) = context_menu.hovered_index() {
            if selected_idx == index {
                layer.push(RenderCommand::FillRect {
                    rect: row_rect,
                    color: Color::rgba(208, 224, 249, 255),
                });
            }
        }
        // Handle different item types
        if item.separator {
            // Draw separator line
            layer.push(RenderCommand::DrawLine {
                from: Point {
                    x: rect.x + 8,
                    y: row_y + (row_height / 2) as i32,
                },
                to: Point {
                    x: rect.x + rect.width as f32 as i32 - 8,
                    y: row_y + (row_height / 2) as i32,
                },
                color: Color::rgba(180, 186, 196, 255),
            });
        } else {
            // Draw checkmark for checkable items
            let mut text_offset_x = rect.x + 8;
            if item.checkable {
                if item.checked {
                    layer.push(RenderCommand::DrawText {
                        origin: Point {
                            x: text_offset_x,
                            y: row_y + 4,
                        },
                        text: "✓".to_string(),
                        font: context_menu.font().cloned().unwrap_or_default(),
                        color: Color::rgba(32, 34, 38, 255),
                    });
                }
                text_offset_x += 16;
            }
            text_offset_x += icon_width;
            // Draw item text
            let text_color = if item.enabled {
                context_menu
                    .foreground_color()
                    .unwrap_or(Color::rgba(32, 34, 38, 255))
            } else {
                Color::rgba(128, 128, 128, 255)
            };
            layer.push(RenderCommand::DrawText {
                origin: Point {
                    x: text_offset_x,
                    y: row_y + 4,
                },
                text: item.text.clone(),
                font: context_menu.font().cloned().unwrap_or_default(),
                color: text_color,
            });
            // Draw shortcut if present
            if !item.shortcut.is_empty() {
                layer.push(RenderCommand::DrawText {
                    origin: Point {
                        x: rect.x + rect.width as f32 as i32 - shortcut_width,
                        y: row_y + 4,
                    },
                    text: item.shortcut.clone(),
                    font: context_menu.font().cloned().unwrap_or_default(),
                    color: Color::rgba(100, 100, 100, 255),
                });
            }
            // Draw submenu arrow
            if item.has_submenu {
                layer.push(RenderCommand::DrawText {
                    origin: Point {
                        x: rect.x + rect.width as f32 as i32 - 16,
                        y: row_y + 4,
                    },
                    text: "▶".to_string(),
                    font: context_menu.font().cloned().unwrap_or_default(),
                    color: Color::rgba(100, 100, 100, 255),
                });
            }
        }
    }
}
/// Append visual commands for a `ToolBar` host representation.
#[deprecated(note = "Pipeline routing is unstable. Use RenderContext directly instead.")]
pub fn append_tool_bar_visual_commands(layer: &mut SceneLayer, tool_bar: &ToolBar) {
    push_widget_fill_and_border(
        layer,
        tool_bar,
        Some(Color::rgba(236, 240, 246, 255)),
        Some((Color::rgba(126, 132, 142, 255), 1)),
    );
    let rect = tool_bar.geometry();
    if is_empty_rect(&rect) {
        return;
    }
    let mut cursor_x = rect.x + 4;
    let button_width = 32u32;
    let separator_width = 4u32;
    for (_index, item) in tool_bar.items().iter().enumerate() {
        // Draw separator
        if item.separator {
            let separator_rect = Rect {
                x: cursor_x,
                y: rect.y + 4,
                width: separator_width,
                height: rect.height.saturating_sub(8),
            };
            layer.push(RenderCommand::FillRect {
                rect: separator_rect,
                color: Color::rgba(180, 186, 196, 255),
            });
            cursor_x += separator_width as i32 + 4;
            continue;
        }
        // Draw action item
        let action_rect = Rect {
            x: cursor_x,
            y: rect.y + 2,
            width: button_width,
            height: rect.height.saturating_sub(4),
        };
        // Draw selection highlight
        if item.checked {
            layer.push(RenderCommand::FillRoundedRect {
                rect: action_rect,
                radius: 3,
                color: Color::rgba(208, 224, 249, 255),
            });
        }
        // Draw button background
        layer.push(RenderCommand::FillRoundedRect {
            rect: action_rect,
            radius: 3,
            color: Color::rgba(216, 225, 238, 255),
        });
        // Draw item text (if no icon or as tooltip)
        let text_color = if item.enabled {
            tool_bar
                .foreground_color()
                .unwrap_or(Color::rgba(30, 32, 36, 255))
        } else {
            Color::rgba(128, 128, 128, 255)
        };
        // Show first character as compact button text
        if !item.text.is_empty() {
            layer.push(RenderCommand::DrawText {
                origin: centered_text_origin(action_rect),
                text: item.text.chars().take(1).collect::<String>(),
                font: tool_bar.font().cloned().unwrap_or_default(),
                color: text_color,
            });
        }
        cursor_x += button_width as i32 + 4;
        if cursor_x >= rect.x + rect.width as f32 as i32 {
            break;
        }
    }
}
/// Append visual commands for a `StatusBar` host representation.
#[deprecated(note = "Pipeline routing is unstable. Use RenderContext directly instead.")]
pub fn append_status_bar_visual_commands(layer: &mut SceneLayer, status_bar: &StatusBar) {
    push_widget_fill_and_border(
        layer,
        status_bar,
        Some(Color::rgba(232, 236, 243, 255)),
        Some((Color::rgba(124, 130, 140, 255), 1)),
    );
    if !status_bar.message().is_empty() {
        let rect = status_bar.geometry();
        layer.push(RenderCommand::DrawText {
            origin: centered_text_origin(rect),
            text: status_bar.message().to_string(),
            font: status_bar.font().cloned().unwrap_or_default(),
            color: status_bar
                .foreground_color()
                .unwrap_or(Color::rgba(34, 36, 40, 255)),
        });
    }
}
