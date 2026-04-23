use crate::core::{Color, Point, Rect};
use crate::render::{RenderCommand, SceneLayer};
use crate::widget::{
    web_engine::WebEngineView, WebEngineContextMenuRequest, WebEngineCookieStore,
    WebEngineDownloadItem, WebEngineFindTextResult, WebEngineNotification, WebEnginePage,
    WebEngineScriptDialog, WebEngineSettings, WebEngineWebChannel, Widget,
};
pub(crate) fn push_widget_fill_and_border(
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
/// Append visual commands for a `WebEngineView` baseline representation.
pub fn append_web_engine_view_visual_commands(
    layer: &mut SceneLayer,
    web_engine_view: &WebEngineView,
) {
    push_widget_fill_and_border(
        layer,
        web_engine_view,
        Some(Color::BACKGROUND),
        Some((Color::SECONDARY, 1)),
    );
    let rect = web_engine_view.geometry();
    if rect.width > 16 && rect.height > 12 {
        layer.push(RenderCommand::DrawText {
            origin: Point {
                x: rect.x + 8,
                y: rect.y + 4,
            },
            text: "WebEngineView".to_string(),
            font: web_engine_view.font().cloned().unwrap_or_default(),
            color: web_engine_view
                .foreground_color()
                .unwrap_or(Color::FOREGROUND),
        });
        if rect.height > 30 {
            // Draw navigation bar
            layer.push(RenderCommand::FillRect {
                rect: Rect {
                    x: rect.x,
                    y: rect.y + 24,
                    width: rect.width,
                    height: 40,
                },
                color: Color::SECONDARY,
            });
            // Draw back button
            let button_size = 28u32;
            layer.push(RenderCommand::FillRect {
                rect: Rect {
                    x: rect.x + 8,
                    y: rect.y + 30,
                    width: button_size,
                    height: button_size,
                },
                color: if web_engine_view.can_go_back() {
                    Color::PRIMARY
                } else {
                    Color::BACKGROUND
                },
            });
            layer.push(RenderCommand::DrawText {
                origin: Point {
                    x: rect.x + 16,
                    y: rect.y + 48,
                },
                text: "<".to_string(),
                font: web_engine_view.font().cloned().unwrap_or_default(),
                color: if web_engine_view.can_go_back() {
                    Color::WHITE
                } else {
                    Color::FOREGROUND
                },
            });
            // Draw forward button
            layer.push(RenderCommand::FillRect {
                rect: Rect {
                    x: rect.x + 44,
                    y: rect.y + 30,
                    width: button_size,
                    height: button_size,
                },
                color: if web_engine_view.can_go_forward() {
                    Color::PRIMARY
                } else {
                    Color::BACKGROUND
                },
            });
            layer.push(RenderCommand::DrawText {
                origin: Point {
                    x: rect.x + 52,
                    y: rect.y + 48,
                },
                text: ">".to_string(),
                font: web_engine_view.font().cloned().unwrap_or_default(),
                color: if web_engine_view.can_go_forward() {
                    Color::WHITE
                } else {
                    Color::FOREGROUND
                },
            });
            // Draw reload button
            layer.push(RenderCommand::FillRect {
                rect: Rect {
                    x: rect.x + 80,
                    y: rect.y + 30,
                    width: button_size,
                    height: button_size,
                },
                color: Color::PRIMARY,
            });
            layer.push(RenderCommand::DrawText {
                origin: Point {
                    x: rect.x + 88,
                    y: rect.y + 48,
                },
                text: "R".to_string(),
                font: web_engine_view.font().cloned().unwrap_or_default(),
                color: Color::WHITE,
            });
            // Draw address bar
            layer.push(RenderCommand::FillRect {
                rect: Rect {
                    x: rect.x + 116,
                    y: rect.y + 30,
                    width: rect.width - 124,
                    height: button_size,
                },
                color: Color::WHITE,
            });
            layer.push(RenderCommand::DrawRectStroke {
                rect: Rect {
                    x: rect.x + 116,
                    y: rect.y + 30,
                    width: rect.width - 124,
                    height: button_size,
                },
                color: Color::rgba(122, 128, 138, 255),
                width: 1,
            });
            // Draw URL text
            let url_text = if web_engine_view.url().is_empty() {
                "about:blank"
            } else {
                web_engine_view.url()
            };
            layer.push(RenderCommand::DrawText {
                origin: Point {
                    x: rect.x + 124,
                    y: rect.y + 48,
                },
                text: url_text.to_string(),
                font: web_engine_view.font().cloned().unwrap_or_default(),
                color: Color::FOREGROUND,
            });
            // Draw web content area
            if rect.height > 80 {
                layer.push(RenderCommand::FillRect {
                    rect: Rect {
                        x: rect.x + 8,
                        y: rect.y + 72,
                        width: rect.width - 16,
                        height: rect.height - 80,
                    },
                    color: Color::WHITE,
                });
                layer.push(RenderCommand::DrawRectStroke {
                    rect: Rect {
                        x: rect.x + 8,
                        y: rect.y + 72,
                        width: rect.width - 16,
                        height: rect.height - 80,
                    },
                    color: Color::rgba(122, 128, 138, 255),
                    width: 1,
                });
                // Draw sample web content
                let title = if web_engine_view.title().is_empty() {
                    "Web Page"
                } else {
                    web_engine_view.title()
                };
                layer.push(RenderCommand::DrawText {
                    origin: Point {
                        x: rect.x + 24,
                        y: rect.y + 96,
                    },
                    text: title.to_string(),
                    font: web_engine_view.font().cloned().unwrap_or_default(),
                    color: Color::BLACK,
                });
                layer.push(RenderCommand::DrawText {
                    origin: Point {
                        x: rect.x + 24,
                        y: rect.y + 120,
                    },
                    text: "This is a web engine view widget.".to_string(),
                    font: web_engine_view.font().cloned().unwrap_or_default(),
                    color: Color::rgba(100, 100, 100, 255),
                });
                // Draw loading indicator
                if web_engine_view.is_loading() {
                    layer.push(RenderCommand::FillRect {
                        rect: Rect {
                            x: rect.x + 24,
                            y: rect.y + 144,
                            width: 100,
                            height: 4,
                        },
                        color: Color::PRIMARY,
                    });
                }
            }
        }
    }
}
/// Append visual commands for a `WebEnginePage` baseline representation.
pub fn append_web_engine_page_visual_commands(
    layer: &mut SceneLayer,
    web_engine_page: &WebEnginePage,
) {
    push_widget_fill_and_border(
        layer,
        web_engine_page,
        Some(Color::BACKGROUND),
        Some((Color::SECONDARY, 1)),
    );
    let rect = web_engine_page.geometry();
    if rect.width > 16 && rect.height > 12 {
        layer.push(RenderCommand::DrawText {
            origin: Point {
                x: rect.x + 8,
                y: rect.y + 4,
            },
            text: "WebEnginePage".to_string(),
            font: web_engine_page.font().cloned().unwrap_or_default(),
            color: web_engine_page
                .foreground_color()
                .unwrap_or(Color::FOREGROUND),
        });
        if rect.height > 40 {
            // Draw page info
            layer.push(RenderCommand::DrawText {
                origin: Point {
                    x: rect.x + 16,
                    y: rect.y + 24,
                },
                text: format!(
                    "URL: {}",
                    if web_engine_page.url().is_empty() {
                        "about:blank"
                    } else {
                        web_engine_page.url()
                    }
                ),
                font: web_engine_page.font().cloned().unwrap_or_default(),
                color: web_engine_page
                    .foreground_color()
                    .unwrap_or(Color::FOREGROUND),
            });
            layer.push(RenderCommand::DrawText {
                origin: Point {
                    x: rect.x + 16,
                    y: rect.y + 48,
                },
                text: format!(
                    "Title: {}",
                    if web_engine_page.title().is_empty() {
                        "(No title)"
                    } else {
                        web_engine_page.title()
                    }
                ),
                font: web_engine_page.font().cloned().unwrap_or_default(),
                color: web_engine_page
                    .foreground_color()
                    .unwrap_or(Color::FOREGROUND),
            });
            // Draw status indicators
            let status_text = format!(
                "Loading: {}, Back: {}, Forward: {}",
                web_engine_page.is_loading(),
                web_engine_page.can_go_back(),
                web_engine_page.can_go_forward()
            );
            layer.push(RenderCommand::DrawText {
                origin: Point {
                    x: rect.x + 16,
                    y: rect.y + 72,
                },
                text: status_text,
                font: web_engine_page.font().cloned().unwrap_or_default(),
                color: web_engine_page
                    .foreground_color()
                    .unwrap_or(Color::FOREGROUND),
            });
        }
    }
}
/// Append visual commands for a `WebEngineSettings` baseline representation.
pub fn append_web_engine_settings_visual_commands(
    layer: &mut SceneLayer,
    web_engine_settings: &WebEngineSettings,
) {
    push_widget_fill_and_border(
        layer,
        web_engine_settings,
        Some(Color::BACKGROUND),
        Some((Color::SECONDARY, 1)),
    );
    let rect = web_engine_settings.geometry();
    if rect.width > 16 && rect.height > 12 {
        layer.push(RenderCommand::DrawText {
            origin: Point {
                x: rect.x + 8,
                y: rect.y + 4,
            },
            text: "WebEngineSettings".to_string(),
            font: web_engine_settings.font().cloned().unwrap_or_default(),
            color: web_engine_settings
                .foreground_color()
                .unwrap_or(Color::FOREGROUND),
        });
        if rect.height > 60 {
            // Draw settings items
            let settings_items = [
                "JavaScript: Enabled",
                "Plugins: Disabled",
                "Private Browsing: Disabled",
                "Local Storage: Enabled",
                "Cookies: Enabled",
            ];
            for (i, item) in settings_items.iter().enumerate() {
                layer.push(RenderCommand::DrawText {
                    origin: Point {
                        x: rect.x + 16,
                        y: rect.y + 24 + (i as i32) * 20,
                    },
                    text: item.to_string(),
                    font: web_engine_settings.font().cloned().unwrap_or_default(),
                    color: web_engine_settings
                        .foreground_color()
                        .unwrap_or(Color::FOREGROUND),
                });
            }
        }
    }
}
/// Append visual commands for a `WebEngineDownloadItem` baseline representation.
pub fn append_web_engine_download_item_visual_commands(
    layer: &mut SceneLayer,
    download_item: &WebEngineDownloadItem,
) {
    push_widget_fill_and_border(
        layer,
        download_item,
        Some(Color::BACKGROUND),
        Some((Color::SECONDARY, 1)),
    );
    let rect = download_item.geometry();
    if rect.width > 16 && rect.height > 12 {
        layer.push(RenderCommand::DrawText {
            origin: Point {
                x: rect.x + 8,
                y: rect.y + 4,
            },
            text: "WebEngineDownloadItem".to_string(),
            font: download_item.font().cloned().unwrap_or_default(),
            color: download_item
                .foreground_color()
                .unwrap_or(Color::FOREGROUND),
        });
        if rect.height > 60 {
            // Draw download info
            layer.push(RenderCommand::DrawText {
                origin: Point {
                    x: rect.x + 16,
                    y: rect.y + 24,
                },
                text: "File: download.bin".to_string(),
                font: download_item.font().cloned().unwrap_or_default(),
                color: download_item
                    .foreground_color()
                    .unwrap_or(Color::FOREGROUND),
            });
            layer.push(RenderCommand::DrawText {
                origin: Point {
                    x: rect.x + 16,
                    y: rect.y + 48,
                },
                text: "Progress: 50%".to_string(),
                font: download_item.font().cloned().unwrap_or_default(),
                color: download_item
                    .foreground_color()
                    .unwrap_or(Color::FOREGROUND),
            });
            // Draw progress bar
            layer.push(RenderCommand::FillRect {
                rect: Rect {
                    x: rect.x + 16,
                    y: rect.y + 64,
                    width: rect.width - 32,
                    height: 8,
                },
                color: Color::SECONDARY,
            });
            layer.push(RenderCommand::FillRect {
                rect: Rect {
                    x: rect.x + 16,
                    y: rect.y + 64,
                    width: (rect.width - 32) / 2,
                    height: 8,
                },
                color: Color::PRIMARY,
            });
        }
    }
}
/// Append visual commands for a `WebEngineCookieStore` baseline representation.
pub fn append_web_engine_cookie_store_visual_commands(
    layer: &mut SceneLayer,
    cookie_store: &WebEngineCookieStore,
) {
    push_widget_fill_and_border(
        layer,
        cookie_store,
        Some(Color::BACKGROUND),
        Some((Color::SECONDARY, 1)),
    );
    let rect = cookie_store.geometry();
    if rect.width > 16 && rect.height > 12 {
        layer.push(RenderCommand::DrawText {
            origin: Point {
                x: rect.x + 8,
                y: rect.y + 4,
            },
            text: "WebEngineCookieStore".to_string(),
            font: cookie_store.font().cloned().unwrap_or_default(),
            color: cookie_store.foreground_color().unwrap_or(Color::FOREGROUND),
        });
        if rect.height > 60 {
            // Draw cookie info
            layer.push(RenderCommand::DrawText {
                origin: Point {
                    x: rect.x + 16,
                    y: rect.y + 24,
                },
                text: "Cookies: 5".to_string(),
                font: cookie_store.font().cloned().unwrap_or_default(),
                color: cookie_store.foreground_color().unwrap_or(Color::FOREGROUND),
            });
            layer.push(RenderCommand::DrawText {
                origin: Point {
                    x: rect.x + 16,
                    y: rect.y + 48,
                },
                text: "Session: 2, Persistent: 3".to_string(),
                font: cookie_store.font().cloned().unwrap_or_default(),
                color: cookie_store.foreground_color().unwrap_or(Color::FOREGROUND),
            });
        }
    }
}
/// Append visual commands for a `WebEngineWebChannel` baseline representation.
pub fn append_web_engine_web_channel_visual_commands(
    layer: &mut SceneLayer,
    web_channel: &WebEngineWebChannel,
) {
    push_widget_fill_and_border(
        layer,
        web_channel,
        Some(Color::BACKGROUND),
        Some((Color::SECONDARY, 1)),
    );
    let rect = web_channel.geometry();
    if rect.width > 16 && rect.height > 12 {
        layer.push(RenderCommand::DrawText {
            origin: Point {
                x: rect.x + 8,
                y: rect.y + 4,
            },
            text: "WebEngineWebChannel".to_string(),
            font: web_channel.font().cloned().unwrap_or_default(),
            color: web_channel.foreground_color().unwrap_or(Color::FOREGROUND),
        });
        if rect.height > 40 {
            // Draw channel info
            layer.push(RenderCommand::DrawText {
                origin: Point {
                    x: rect.x + 16,
                    y: rect.y + 24,
                },
                text: "JavaScript Bridge".to_string(),
                font: web_channel.font().cloned().unwrap_or_default(),
                color: web_channel.foreground_color().unwrap_or(Color::FOREGROUND),
            });
            layer.push(RenderCommand::DrawText {
                origin: Point {
                    x: rect.x + 16,
                    y: rect.y + 48,
                },
                text: "Connected: Yes".to_string(),
                font: web_channel.font().cloned().unwrap_or_default(),
                color: web_channel.foreground_color().unwrap_or(Color::FOREGROUND),
            });
        }
    }
}
/// Append visual commands for a `WebEngineFindTextResult` baseline representation.
pub fn append_web_engine_find_text_result_visual_commands(
    layer: &mut SceneLayer,
    find_result: &WebEngineFindTextResult,
) {
    push_widget_fill_and_border(
        layer,
        find_result,
        Some(Color::BACKGROUND),
        Some((Color::SECONDARY, 1)),
    );
    let rect = find_result.geometry();
    if rect.width > 16 && rect.height > 12 {
        layer.push(RenderCommand::DrawText {
            origin: Point {
                x: rect.x + 8,
                y: rect.y + 4,
            },
            text: "WebEngineFindTextResult".to_string(),
            font: find_result.font().cloned().unwrap_or_default(),
            color: find_result.foreground_color().unwrap_or(Color::FOREGROUND),
        });
        if rect.height > 40 {
            // Draw find info
            layer.push(RenderCommand::DrawText {
                origin: Point {
                    x: rect.x + 16,
                    y: rect.y + 24,
                },
                text: "Found: 5 matches".to_string(),
                font: find_result.font().cloned().unwrap_or_default(),
                color: find_result.foreground_color().unwrap_or(Color::FOREGROUND),
            });
            layer.push(RenderCommand::DrawText {
                origin: Point {
                    x: rect.x + 16,
                    y: rect.y + 48,
                },
                text: "Current: Match 3/5".to_string(),
                font: find_result.font().cloned().unwrap_or_default(),
                color: find_result.foreground_color().unwrap_or(Color::FOREGROUND),
            });
        }
    }
}
/// Append visual commands for a `WebEngineNotification` baseline representation.
pub fn append_web_engine_notification_visual_commands(
    layer: &mut SceneLayer,
    notification: &WebEngineNotification,
) {
    push_widget_fill_and_border(
        layer,
        notification,
        Some(Color::BACKGROUND),
        Some((Color::SECONDARY, 1)),
    );
    let rect = notification.geometry();
    if rect.width > 16 && rect.height > 12 {
        layer.push(RenderCommand::DrawText {
            origin: Point {
                x: rect.x + 8,
                y: rect.y + 4,
            },
            text: "WebEngineNotification".to_string(),
            font: notification.font().cloned().unwrap_or_default(),
            color: notification.foreground_color().unwrap_or(Color::FOREGROUND),
        });
        if rect.height > 60 {
            // Draw notification content
            layer.push(RenderCommand::DrawText {
                origin: Point {
                    x: rect.x + 16,
                    y: rect.y + 24,
                },
                text: "Notification Title".to_string(),
                font: notification.font().cloned().unwrap_or_default(),
                color: notification.foreground_color().unwrap_or(Color::FOREGROUND),
            });
            layer.push(RenderCommand::DrawText {
                origin: Point {
                    x: rect.x + 16,
                    y: rect.y + 48,
                },
                text: "This is a web notification".to_string(),
                font: notification.font().cloned().unwrap_or_default(),
                color: notification.foreground_color().unwrap_or(Color::FOREGROUND),
            });
        }
    }
}
/// Append visual commands for a `WebEngineScriptDialog` baseline representation.
pub fn append_web_engine_script_dialog_visual_commands(
    layer: &mut SceneLayer,
    script_dialog: &WebEngineScriptDialog,
) {
    push_widget_fill_and_border(
        layer,
        script_dialog,
        Some(Color::BACKGROUND),
        Some((Color::SECONDARY, 1)),
    );
    let rect = script_dialog.geometry();
    if rect.width > 16 && rect.height > 12 {
        layer.push(RenderCommand::DrawText {
            origin: Point {
                x: rect.x + 8,
                y: rect.y + 4,
            },
            text: "WebEngineScriptDialog".to_string(),
            font: script_dialog.font().cloned().unwrap_or_default(),
            color: script_dialog
                .foreground_color()
                .unwrap_or(Color::FOREGROUND),
        });
        if rect.height > 80 {
            // Draw dialog content
            layer.push(RenderCommand::DrawText {
                origin: Point {
                    x: rect.x + 16,
                    y: rect.y + 24,
                },
                text: "JavaScript Alert".to_string(),
                font: script_dialog.font().cloned().unwrap_or_default(),
                color: script_dialog
                    .foreground_color()
                    .unwrap_or(Color::FOREGROUND),
            });
            layer.push(RenderCommand::DrawText {
                origin: Point {
                    x: rect.x + 16,
                    y: rect.y + 48,
                },
                text: "Hello, world!".to_string(),
                font: script_dialog.font().cloned().unwrap_or_default(),
                color: script_dialog
                    .foreground_color()
                    .unwrap_or(Color::FOREGROUND),
            });
            // Draw OK button
            layer.push(RenderCommand::FillRect {
                rect: Rect {
                    x: rect.x + (rect.width as i32 - 80) / 2,
                    y: rect.y + 60,
                    width: 80,
                    height: 24,
                },
                color: Color::PRIMARY,
            });
            layer.push(RenderCommand::DrawText {
                origin: Point {
                    x: rect.x + (rect.width as i32 - 80) / 2 + 30,
                    y: rect.y + 76,
                },
                text: "OK".to_string(),
                font: script_dialog.font().cloned().unwrap_or_default(),
                color: Color::WHITE,
            });
        }
    }
}
/// Append visual commands for a `WebEngineContextMenuRequest` baseline representation.
pub fn append_web_engine_context_menu_request_visual_commands(
    layer: &mut SceneLayer,
    context_menu: &WebEngineContextMenuRequest,
) {
    push_widget_fill_and_border(
        layer,
        context_menu,
        Some(Color::BACKGROUND),
        Some((Color::SECONDARY, 1)),
    );
    let rect = context_menu.geometry();
    if rect.width > 16 && rect.height > 12 {
        layer.push(RenderCommand::DrawText {
            origin: Point {
                x: rect.x + 8,
                y: rect.y + 4,
            },
            text: "WebEngineContextMenuRequest".to_string(),
            font: context_menu.font().cloned().unwrap_or_default(),
            color: context_menu.foreground_color().unwrap_or(Color::FOREGROUND),
        });
        if rect.height > 100 {
            // Draw context menu items
            let menu_items = [
                "Open in new tab",
                "Save link as...",
                "Copy link address",
                "Inspect element",
                "Save image as...",
            ];
            for (i, item) in menu_items.iter().enumerate() {
                layer.push(RenderCommand::DrawText {
                    origin: Point {
                        x: rect.x + 16,
                        y: rect.y + 24 + (i as i32) * 20,
                    },
                    text: item.to_string(),
                    font: context_menu.font().cloned().unwrap_or_default(),
                    color: context_menu.foreground_color().unwrap_or(Color::FOREGROUND),
                });
            }
        }
    }
}
