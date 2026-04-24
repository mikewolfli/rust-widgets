//! Miscellaneous widgets: activity_indicator, toggle_button, check_list_box,
//! double_spin_box, dial, wizard.
use crate::core::{Color, Font, Point, Rect};
use crate::render::is_empty_rect;
use crate::render::pipeline::controls::{centered_text_origin, push_widget_fill_and_border};
use crate::render::{RenderCommand, SceneLayer};
use crate::widget::{ActivityIndicator, Widget};

pub fn append_activity_indicator_visual_commands(
    layer: &mut SceneLayer,
    activity_indicator: &ActivityIndicator,
) {
    push_widget_fill_and_border(
        layer,
        activity_indicator,
        Some(Color::BACKGROUND),
        Some((Color::SECONDARY, 1)),
    );
    let rect = activity_indicator.geometry();
    let center = Point {
        x: rect.x + (rect.width / 2) as i32,
        y: rect.y + (rect.height / 2) as i32,
    };
    let radius = (rect.width.min(rect.height) / 2 - 4) as f32;
    // Draw activity indicator
    for i in 0..8 {
        let angle = (i as f32) * std::f32::consts::PI / 4.0;
        let alpha = (i as f32 / 8.0) * 255.0;
        let color = Color {
            r: 0,
            g: 128,
            b: 255,
            a: alpha as u8,
        };
        let x = center.x + (angle.cos() * radius) as i32;
        let y = center.y + (angle.sin() * radius) as i32;
        layer.push(RenderCommand::DrawCircle {
            center: Point { x, y },
            radius: 3,
            color,
        });
    }
}
/// Append visual commands for a `ToggleButton` baseline representation.
pub fn append_toggle_button_visual_commands(
    layer: &mut SceneLayer,
    toggle_button: &crate::widget::ToggleButton,
) {
    push_widget_fill_and_border(
        layer,
        toggle_button,
        Some(if toggle_button.is_checked() {
            Color::PRIMARY
        } else {
            Color::BACKGROUND
        }),
        Some((Color::rgba(122, 128, 138, 255), 1)),
    );
    let rect = toggle_button.geometry();
    if is_empty_rect(&rect) {
        return;
    }
    layer.push(RenderCommand::DrawText {
        origin: centered_text_origin(rect),
        text: toggle_button.text().to_string(),
        font: toggle_button.font().cloned().unwrap_or_default(),
        color: if toggle_button.is_checked() {
            Color::WHITE
        } else {
            toggle_button
                .foreground_color()
                .unwrap_or(Color::rgba(30, 32, 36, 255))
        },
    });
}
/// Append visual commands for a `CheckListBox` baseline representation.
pub fn append_check_list_box_visual_commands(
    layer: &mut SceneLayer,
    check_list_box: &crate::widget::CheckListBox,
) {
    push_widget_fill_and_border(
        layer,
        check_list_box,
        Some(Color::WHITE),
        Some((Color::rgba(160, 168, 180, 255), 1)),
    );
    let rect = check_list_box.geometry();
    if is_empty_rect(&rect) {
        return;
    }
    let row_height = 24u32;
    let padding = 8i32;
    let text_color = Color::rgba(40, 44, 52, 255);
    let font = Font::default_ui();
    let visible_rows = (rect.height / row_height) as usize;
    let row_count = check_list_box.count().min(visible_rows);
    for row in 0..row_count {
        let row_y = rect.y + (row as u32 * row_height) as i32;
        // Draw checkbox
        let checkbox_rect = Rect {
            x: rect.x + padding,
            y: row_y + (row_height as i32 - 16) / 2,
            width: 16,
            height: 16,
        };
        layer.push(RenderCommand::DrawRectStroke {
            rect: checkbox_rect,
            color: Color::rgba(122, 128, 138, 255),
            width: 1,
        });
        if check_list_box.is_selected(row) {
            layer.push(RenderCommand::FillRect {
                rect: Rect {
                    x: checkbox_rect.x + 3,
                    y: checkbox_rect.y + 3,
                    width: 10,
                    height: 10,
                },
                color: Color::PRIMARY,
            });
        }
        // Draw item text
        if let Some(item) = check_list_box.item(row) {
            layer.push(RenderCommand::DrawText {
                text: item.to_string(),
                origin: Point {
                    x: rect.x + padding + 24,
                    y: row_y + row_height as i32 / 2,
                },
                font: font.clone(),
                color: text_color,
            });
        }
    }
}
/// Append visual commands for a `DoubleSpinBox` baseline representation.
pub fn append_double_spin_box_visual_commands(
    layer: &mut SceneLayer,
    double_spin_box: &crate::widget::DoubleSpinBox,
) {
    push_widget_fill_and_border(
        layer,
        double_spin_box,
        Some(Color::WHITE),
        Some((Color::rgba(160, 168, 180, 255), 1)),
    );
    let rect = double_spin_box.geometry();
    if is_empty_rect(&rect) {
        return;
    }
    // Draw value text
    let text_rect = Rect {
        x: rect.x + 8,
        y: rect.y,
        width: rect.width - 32,
        height: rect.height,
    };
    layer.push(RenderCommand::DrawText {
        origin: centered_text_origin(text_rect),
        text: format!("{:.2}", double_spin_box.value()),
        font: double_spin_box.font().cloned().unwrap_or_default(),
        color: double_spin_box
            .foreground_color()
            .unwrap_or(Color::rgba(30, 32, 36, 255)),
    });
    // Draw up/down buttons
    let button_width = 32u32;
    let button_height = rect.height / 2;
    // Up button
    layer.push(RenderCommand::DrawRectStroke {
        rect: Rect {
            x: rect.x + rect.width as f32 as i32 - button_width as i32,
            y: rect.y,
            width: button_width,
            height: button_height,
        },
        color: Color::rgba(160, 168, 180, 255),
        width: 1,
    });
    // Down button
    layer.push(RenderCommand::DrawRectStroke {
        rect: Rect {
            x: rect.x + rect.width as f32 as i32 - button_width as i32,
            y: rect.y + button_height as i32,
            width: button_width,
            height: button_height,
        },
        color: Color::rgba(160, 168, 180, 255),
        width: 1,
    });
}
/// Append visual commands for a `Dial` baseline representation.
pub fn append_dial_visual_commands(layer: &mut SceneLayer, dial: &crate::widget::Dial) {
    push_widget_fill_and_border(
        layer,
        dial,
        Some(Color::BACKGROUND),
        Some((Color::rgba(122, 128, 138, 255), 1)),
    );
    let rect = dial.geometry();
    if is_empty_rect(&rect) {
        return;
    }
    let center = Point {
        x: rect.x + rect.width as f32 as i32 / 2,
        y: rect.y + rect.height as f32 as i32 / 2,
    };
    let radius = (rect.width.min(rect.height) / 2 - 4) as u32;
    // Draw dial background
    layer.push(RenderCommand::DrawCircleStroke {
        center,
        radius,
        color: Color::rgba(160, 168, 180, 255),
        width: 2,
    });
    // Draw dial needle
    let value = dial.value() as f64;
    let min = 0.0;
    let max = 100.0;
    let angle =
        (value - min) / (max - min) * std::f64::consts::PI * 2.0 - std::f64::consts::PI / 2.0;
    let needle_end = Point {
        x: center.x + (angle.cos() * radius as f64) as i32,
        y: center.y + (angle.sin() * radius as f64) as i32,
    };
    layer.push(RenderCommand::DrawLine {
        from: center,
        to: needle_end,
        color: Color::PRIMARY,
    });
    // Draw center point
    layer.push(RenderCommand::FillCircle {
        center,
        radius: 4,
        color: Color::PRIMARY,
    });
}
/// Append visual commands for a `Wizard` baseline representation.
pub fn append_wizard_visual_commands(layer: &mut SceneLayer, wizard: &crate::widget::Wizard) {
    push_widget_fill_and_border(
        layer,
        wizard,
        Some(Color::BACKGROUND),
        Some((Color::SECONDARY, 1)),
    );
    let rect = wizard.geometry();
    if rect.width > 16 && rect.height > 12 {
        layer.push(RenderCommand::DrawText {
            origin: Point {
                x: rect.x + 8,
                y: rect.y + 4,
            },
            text: "Wizard".to_string(),
            font: wizard.font().cloned().unwrap_or_default(),
            color: wizard.foreground_color().unwrap_or(Color::FOREGROUND),
        });
        if rect.height > 30 {
            // Draw header
            layer.push(RenderCommand::FillRect {
                rect: Rect {
                    x: rect.x,
                    y: rect.y + 24,
                    width: rect.width,
                    height: 40,
                },
                color: Color::PRIMARY,
            });
            layer.push(RenderCommand::DrawText {
                origin: Point {
                    x: rect.x + 16,
                    y: rect.y + 40,
                },
                text: "Wizard Step 1 of 3".to_string(),
                font: wizard.font().cloned().unwrap_or_default(),
                color: Color::WHITE,
            });
            // Draw content area
            layer.push(RenderCommand::FillRect {
                rect: Rect {
                    x: rect.x + 8,
                    y: rect.y + 72,
                    width: rect.width - 16,
                    height: rect.height - 120,
                },
                color: Color::WHITE,
            });
            layer.push(RenderCommand::DrawRectStroke {
                rect: Rect {
                    x: rect.x + 8,
                    y: rect.y + 72,
                    width: rect.width - 16,
                    height: rect.height - 120,
                },
                color: Color::rgba(122, 128, 138, 255),
                width: 1,
            });
            // Draw buttons
            let button_width = 80u32;
            let button_height = 28u32;
            // Back button
            layer.push(RenderCommand::FillRect {
                rect: Rect {
                    x: rect.x + rect.width as f32 as i32 - button_width as i32 * 2 - 16,
                    y: rect.y + rect.height as f32 as i32 - button_height as i32 - 8,
                    width: button_width,
                    height: button_height,
                },
                color: Color::BACKGROUND,
            });
            layer.push(RenderCommand::DrawRectStroke {
                rect: Rect {
                    x: rect.x + rect.width as f32 as i32 - button_width as i32 * 2 - 16,
                    y: rect.y + rect.height as f32 as i32 - button_height as i32 - 8,
                    width: button_width,
                    height: button_height,
                },
                color: Color::rgba(122, 128, 138, 255),
                width: 1,
            });
            layer.push(RenderCommand::DrawText {
                origin: centered_text_origin(Rect {
                    x: rect.x + rect.width as f32 as i32 - button_width as i32 * 2 - 16,
                    y: rect.y + rect.height as f32 as i32 - button_height as i32 - 8,
                    width: button_width,
                    height: button_height,
                }),
                text: "Back".to_string(),
                font: wizard.font().cloned().unwrap_or_default(),
                color: wizard
                    .foreground_color()
                    .unwrap_or(Color::rgba(30, 32, 36, 255)),
            });
            // Next button
            layer.push(RenderCommand::FillRect {
                rect: Rect {
                    x: rect.x + rect.width as f32 as i32 - button_width as i32 - 8,
                    y: rect.y + rect.height as f32 as i32 - button_height as i32 - 8,
                    width: button_width,
                    height: button_height,
                },
                color: Color::PRIMARY,
            });
            layer.push(RenderCommand::DrawRectStroke {
                rect: Rect {
                    x: rect.x + rect.width as f32 as i32 - button_width as i32 - 8,
                    y: rect.y + rect.height as f32 as i32 - button_height as i32 - 8,
                    width: button_width,
                    height: button_height,
                },
                color: Color::rgba(52, 122, 226, 255),
                width: 1,
            });
            layer.push(RenderCommand::DrawText {
                origin: centered_text_origin(Rect {
                    x: rect.x + rect.width as f32 as i32 - button_width as i32 - 8,
                    y: rect.y + rect.height as f32 as i32 - button_height as i32 - 8,
                    width: button_width,
                    height: button_height,
                }),
                text: "Next".to_string(),
                font: wizard.font().cloned().unwrap_or_default(),
                color: Color::WHITE,
            });
        }
    }
}
