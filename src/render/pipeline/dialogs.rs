//! Dialog widgets: dialog, message_box, file_dialog, color_dialog, font_dialog,
//! popup_window, directory_dialog.
use crate::core::{Color, Point, Rect};

use crate::render::pipeline::controls::push_widget_fill_and_border;
use crate::render::{RenderCommand, SceneLayer};
use crate::widget::{
    ColorDialog, Dialog, DirectoryDialog, FileDialog, FontDialog, MessageBox, PopupWindow, Widget,
};

pub fn append_dialog_visual_commands(layer: &mut SceneLayer, dialog: &Dialog) {
    push_widget_fill_and_border(
        layer,
        dialog,
        Some(Color::BACKGROUND),
        Some((Color::SECONDARY, 1)),
    );
    let rect = dialog.geometry();
    if rect.width > 16 && rect.height > 12 {
        layer.push(RenderCommand::DrawText {
            origin: Point {
                x: rect.x + 8,
                y: rect.y + 4,
            },
            text: "Dialog".to_string(),
            font: dialog.font().cloned().unwrap_or_default(),
            color: dialog.foreground_color().unwrap_or(Color::FOREGROUND),
        });
    }
}
/// Append visual commands for a `MessageBox` baseline representation.
pub fn append_message_box_visual_commands(layer: &mut SceneLayer, message_box: &MessageBox) {
    push_widget_fill_and_border(
        layer,
        message_box,
        Some(Color::BACKGROUND),
        Some((Color::SECONDARY, 1)),
    );
    let rect = message_box.geometry();
    if rect.width > 16 && rect.height > 12 {
        layer.push(RenderCommand::DrawText {
            origin: Point {
                x: rect.x + 8,
                y: rect.y + 4,
            },
            text: message_box.title().to_string(),
            font: message_box.font().cloned().unwrap_or_default(),
            color: message_box.foreground_color().unwrap_or(Color::FOREGROUND),
        });
        if rect.height > 30 {
            layer.push(RenderCommand::DrawText {
                origin: Point {
                    x: rect.x + 8,
                    y: rect.y + 24,
                },
                text: "Message content".to_string(),
                font: message_box.font().cloned().unwrap_or_default(),
                color: message_box.foreground_color().unwrap_or(Color::FOREGROUND),
            });
        }
    }
}
/// Append visual commands for a `FileDialog` baseline representation.
pub fn append_file_dialog_visual_commands(layer: &mut SceneLayer, file_dialog: &FileDialog) {
    push_widget_fill_and_border(
        layer,
        file_dialog,
        Some(Color::BACKGROUND),
        Some((Color::SECONDARY, 1)),
    );
    let rect = file_dialog.geometry();
    if rect.width > 16 && rect.height > 12 {
        layer.push(RenderCommand::DrawText {
            origin: Point {
                x: rect.x + 8,
                y: rect.y + 4,
            },
            text: file_dialog.title().to_string(),
            font: file_dialog.font().cloned().unwrap_or_default(),
            color: file_dialog.foreground_color().unwrap_or(Color::FOREGROUND),
        });
        if rect.height > 30 {
            layer.push(RenderCommand::FillRect {
                rect: Rect {
                    x: rect.x + 8,
                    y: rect.y + 24,
                    width: rect.width - 16,
                    height: rect.height - 40,
                },
                color: Color::WHITE,
            });
            layer.push(RenderCommand::DrawRectStroke {
                rect: Rect {
                    x: rect.x + 8,
                    y: rect.y + 24,
                    width: rect.width - 16,
                    height: rect.height - 40,
                },
                color: Color::rgba(122, 128, 138, 255),
                width: 1,
            });
            layer.push(RenderCommand::DrawText {
                origin: Point {
                    x: rect.x + 16,
                    y: rect.y + 32,
                },
                text: "File browser".to_string(),
                font: file_dialog.font().cloned().unwrap_or_default(),
                color: file_dialog.foreground_color().unwrap_or(Color::FOREGROUND),
            });
        }
    }
}
/// Append visual commands for a `ColorDialog` baseline representation.
pub fn append_color_dialog_visual_commands(layer: &mut SceneLayer, color_dialog: &ColorDialog) {
    push_widget_fill_and_border(
        layer,
        color_dialog,
        Some(Color::BACKGROUND),
        Some((Color::SECONDARY, 1)),
    );
    let rect = color_dialog.geometry();
    if rect.width > 16 && rect.height > 12 {
        layer.push(RenderCommand::DrawText {
            origin: Point {
                x: rect.x + 8,
                y: rect.y + 4,
            },
            text: "Color Dialog".to_string(),
            font: color_dialog.font().cloned().unwrap_or_default(),
            color: color_dialog.foreground_color().unwrap_or(Color::FOREGROUND),
        });
        if rect.width > 40 && rect.height > 40 {
            layer.push(RenderCommand::FillRect {
                rect: Rect {
                    x: rect.x + 16,
                    y: rect.y + 32,
                    width: 80,
                    height: 80,
                },
                color: Color::RED,
            });
            layer.push(RenderCommand::DrawRectStroke {
                rect: Rect {
                    x: rect.x + 16,
                    y: rect.y + 32,
                    width: 80,
                    height: 80,
                },
                color: Color::rgba(122, 128, 138, 255),
                width: 1,
            });
        }
    }
}
/// Append visual commands for a `FontDialog` baseline representation.
pub fn append_font_dialog_visual_commands(layer: &mut SceneLayer, font_dialog: &FontDialog) {
    push_widget_fill_and_border(
        layer,
        font_dialog,
        Some(Color::BACKGROUND),
        Some((Color::SECONDARY, 1)),
    );
    let rect = font_dialog.geometry();
    if rect.width > 16 && rect.height > 12 {
        layer.push(RenderCommand::DrawText {
            origin: Point {
                x: rect.x + 8,
                y: rect.y + 4,
            },
            text: "Font Dialog".to_string(),
            font: font_dialog.font().cloned().unwrap_or_default(),
            color: font_dialog.foreground_color().unwrap_or(Color::FOREGROUND),
        });
        if rect.height > 30 {
            layer.push(RenderCommand::DrawText {
                origin: Point {
                    x: rect.x + 16,
                    y: rect.y + 32,
                },
                text: "Font preview: ABCabc123".to_string(),
                font: font_dialog.font().cloned().unwrap_or_default(),
                color: font_dialog.foreground_color().unwrap_or(Color::FOREGROUND),
            });
        }
    }
}
/// Append visual commands for a `PopupWindow` baseline representation.
pub fn append_popup_window_visual_commands(layer: &mut SceneLayer, popup_window: &PopupWindow) {
    push_widget_fill_and_border(
        layer,
        popup_window,
        Some(Color::rgba(250, 250, 252, 255)),
        Some((Color::SECONDARY, 1)),
    );
    let rect = popup_window.geometry();
    if rect.width > 16 && rect.height > 12 {
        layer.push(RenderCommand::DrawText {
            origin: Point {
                x: rect.x + 8,
                y: rect.y + 4,
            },
            text: "Popup Window".to_string(),
            font: popup_window.font().cloned().unwrap_or_default(),
            color: popup_window.foreground_color().unwrap_or(Color::FOREGROUND),
        });
    }
}
/// Append visual commands for a `DirectoryDialog` baseline representation.
pub fn append_directory_dialog_visual_commands(
    layer: &mut SceneLayer,
    directory_dialog: &DirectoryDialog,
) {
    push_widget_fill_and_border(
        layer,
        directory_dialog,
        Some(Color::BACKGROUND),
        Some((Color::SECONDARY, 1)),
    );
    let rect = directory_dialog.geometry();
    if rect.width > 16 && rect.height > 12 {
        layer.push(RenderCommand::DrawText {
            origin: Point {
                x: rect.x + 8,
                y: rect.y + 4,
            },
            text: "Directory Dialog".to_string(),
            font: directory_dialog.font().cloned().unwrap_or_default(),
            color: directory_dialog
                .foreground_color()
                .unwrap_or(Color::FOREGROUND),
        });
        if rect.height > 30 {
            layer.push(RenderCommand::FillRect {
                rect: Rect {
                    x: rect.x + 8,
                    y: rect.y + 24,
                    width: rect.width - 16,
                    height: rect.height - 40,
                },
                color: Color::WHITE,
            });
            layer.push(RenderCommand::DrawRectStroke {
                rect: Rect {
                    x: rect.x + 8,
                    y: rect.y + 24,
                    width: rect.width - 16,
                    height: rect.height - 40,
                },
                color: Color::rgba(122, 128, 138, 255),
                width: 1,
            });
            layer.push(RenderCommand::DrawText {
                origin: Point {
                    x: rect.x + 16,
                    y: rect.y + 32,
                },
                text: "Directory browser".to_string(),
                font: directory_dialog.font().cloned().unwrap_or_default(),
                color: directory_dialog
                    .foreground_color()
                    .unwrap_or(Color::FOREGROUND),
            });
        }
    }
}
