//! Basic widget rendering: helpers + window, panel, label, button, checkbox,
//! radiobutton, line_edit, combo_box, list_box, progress_bar, slider, scroll_bar.
use crate::core::{Color, Point, Rect};
use crate::render::{SceneLayer, RenderCommand};
use crate::render::is_empty_rect;
use crate::widget::{
    Button, ButtonState, CheckBox, CheckState, ComboBox,
    Label, LineEdit, ListBox, Panel,
    ProgressBar, RadioButton, ScrollBar, Slider, Widget,
};
use crate::window::Window;

pub(crate) fn push_widget_fill_and_border<W: Widget>(
    layer: &mut SceneLayer,
    widget: &W,
    fallback_background: Option<Color>,
    fallback_border: Option<(Color, u32)>,
) {
    let rect = widget.geometry();
    if is_empty_rect(&rect) {
        return;
    }
    if let Some(background) = widget.background_color().or(fallback_background) {
        if widget.border_radius() > 0 {
            layer.push(RenderCommand::FillRoundedRect {
                rect,
                radius: widget.border_radius(),
                color: background,
            });
        } else {
            layer.push(RenderCommand::FillRect {
                rect,
                color: background,
            });
        }
    }
    let border_color = widget
        .border_color()
        .or_else(|| fallback_border.map(|value| value.0));
    let border_width = if widget.border_width() > 0 {
        widget.border_width()
    } else {
        fallback_border.map(|value| value.1).unwrap_or(0)
    };
    if let Some(color) = border_color {
        if border_width > 0 {
            if widget.border_radius() > 0 {
                layer.push(RenderCommand::DrawRoundedRectStroke {
                    rect,
                    radius: widget.border_radius(),
                    color,
                    width: border_width,
                });
            } else {
                layer.push(RenderCommand::DrawRectStroke {
                    rect,
                    color,
                    width: border_width,
                });
            }
        }
    }
}
pub(crate) fn centered_text_origin(rect: Rect) -> Point {
    Point {
        x: rect.x + 6,
        y: rect.y + (rect.height as i32 / 2) - 4,
    }
}
#[allow(dead_code)]
pub(crate) fn normalized_progress_u32(value: u32, min: u32, max: u32) -> f32 {
    if max <= min {
        return 0.0;
    }
    ((value.saturating_sub(min)) as f32 / (max - min) as f32).clamp(0.0, 1.0)
}
pub(crate) fn normalized_progress_i32(value: i32, min: i32, max: i32) -> f32 {
    if max <= min {
        return 0.0;
    }
    ((value - min) as f32 / (max - min) as f32).clamp(0.0, 1.0)
}
/// Append visual commands for a `Window` baseline representation.
#[deprecated(note = "Pipeline routing is unstable. Use RenderContext directly instead.")]
pub fn append_window_visual_commands(layer: &mut SceneLayer, window: &Window) {
    push_widget_fill_and_border(
        layer,
        window,
        Some(Color::BACKGROUND),
        Some((Color::SECONDARY, 1)),
    );
    let rect = window.geometry();
    if rect.width > 16 && rect.height > 12 {
        layer.push(RenderCommand::DrawText {
            origin: Point {
                x: rect.x + 8,
                y: rect.y + 4,
            },
            text: window.title().to_string(),
            font: window.font().cloned().unwrap_or_default(),
            color: window.foreground_color().unwrap_or(Color::FOREGROUND),
        });
    }
}
/// Append visual commands for a `Panel` baseline representation.
#[deprecated(note = "Pipeline routing is unstable. Use RenderContext directly instead.")]
pub fn append_panel_visual_commands(layer: &mut SceneLayer, panel: &Panel) {
    push_widget_fill_and_border(
        layer,
        panel,
        Some(Color::rgba(232, 235, 240, 255)),
        Some((Color::rgba(146, 152, 165, 255), 1)),
    );
}
/// Append visual commands for a `Label` baseline representation.
#[deprecated(note = "Pipeline routing is unstable. Use RenderContext directly instead.")]
pub fn append_label_visual_commands(layer: &mut SceneLayer, label: &Label) {
    push_widget_fill_and_border(layer, label, None, None);
    if !label.text().is_empty() {
        layer.push(RenderCommand::DrawText {
            origin: centered_text_origin(label.geometry()),
            text: label.text().to_string(),
            font: label.font().cloned().unwrap_or_default(),
            color: label
                .foreground_color()
                .unwrap_or(Color::rgba(30, 30, 30, 255)),
        });
    }
}
/// Append visual commands for a `Button` baseline representation.
#[deprecated(note = "Pipeline routing is unstable. Use RenderContext directly instead.")]
pub fn append_button_visual_commands(layer: &mut SceneLayer, button: &Button) {
    let fallback_bg = match button.state() {
        ButtonState::Pressed => Color::PRIMARY,
        ButtonState::Disabled => Color::LIGHT_GRAY,
        ButtonState::Normal => Color::PRIMARY,
    };
    let fallback_fg = if matches!(button.state(), ButtonState::Disabled) {
        Color::GRAY
    } else {
        Color::WHITE
    };
    push_widget_fill_and_border(
        layer,
        button,
        Some(fallback_bg),
        Some((Color::DARK_GRAY, 1)),
    );
    if !button.text().is_empty() {
        layer.push(RenderCommand::DrawText {
            origin: centered_text_origin(button.geometry()),
            text: button.text().to_string(),
            font: button.font().cloned().unwrap_or_default(),
            color: button.foreground_color().unwrap_or(fallback_fg),
        });
    }
}
/// Append visual commands for a `CheckBox` baseline representation.
#[deprecated(note = "Pipeline routing is unstable. Use RenderContext directly instead.")]
pub fn append_checkbox_visual_commands(layer: &mut SceneLayer, checkbox: &CheckBox) {
    let rect = checkbox.geometry();
    if is_empty_rect(&rect) {
        return;
    }
    let box_side = rect.width.min(rect.height).clamp(8, 16);
    let indicator = Rect {
        x: rect.x + 2,
        y: rect.y + ((rect.height as i32 - box_side as i32) / 2),
        width: box_side,
        height: box_side,
    };
    layer.push(RenderCommand::FillRect {
        rect: indicator,
        color: checkbox
            .background_color()
            .unwrap_or(Color::rgba(250, 250, 250, 255)),
    });
    layer.push(RenderCommand::DrawRectStroke {
        rect: indicator,
        color: checkbox
            .border_color()
            .unwrap_or(Color::rgba(90, 98, 108, 255)),
        width: checkbox.border_width().max(1),
    });
    match checkbox.state() {
        CheckState::Checked => {
            layer.push(RenderCommand::FillRect {
                rect: Rect {
                    x: indicator.x + 3,
                    y: indicator.y + 3,
                    width: indicator.width.saturating_sub(6),
                    height: indicator.height.saturating_sub(6),
                },
                color: checkbox
                    .foreground_color()
                    .unwrap_or(Color::rgba(40, 120, 230, 255)),
            });
        }
        CheckState::PartiallyChecked => {
            layer.push(RenderCommand::FillRect {
                rect: Rect {
                    x: indicator.x + 2,
                    y: indicator.y + (indicator.height as i32 / 2) - 1,
                    width: indicator.width.saturating_sub(4),
                    height: 2,
                },
                color: checkbox
                    .foreground_color()
                    .unwrap_or(Color::rgba(40, 120, 230, 255)),
            });
        }
        CheckState::Unchecked => {}
    }
}
/// Append visual commands for a `RadioButton` baseline representation.
#[deprecated(note = "Pipeline routing is unstable. Use RenderContext directly instead.")]
pub fn append_radiobutton_visual_commands(layer: &mut SceneLayer, radio: &RadioButton) {
    let rect = radio.geometry();
    if is_empty_rect(&rect) {
        return;
    }
    let radius = (rect.width.min(rect.height).min(16) / 2).max(4);
    let center = Point {
        x: rect.x + 2 + radius as i32,
        y: rect.y + (rect.height as i32 / 2),
    };
    layer.push(RenderCommand::DrawCircleStroke {
        center,
        radius,
        color: radio
            .border_color()
            .unwrap_or(Color::rgba(92, 98, 108, 255)),
        width: radio.border_width().max(1),
    });
    if radio.is_checked() {
        layer.push(RenderCommand::FillCircle {
            center,
            radius: radius.saturating_sub(3).max(1),
            color: radio
                .foreground_color()
                .unwrap_or(Color::rgba(45, 122, 235, 255)),
        });
    }
}
/// Append visual commands for a `LineEdit` baseline representation.
#[deprecated(note = "Pipeline routing is unstable. Use RenderContext directly instead.")]
pub fn append_line_edit_visual_commands(layer: &mut SceneLayer, line_edit: &LineEdit) {
    push_widget_fill_and_border(
        layer,
        line_edit,
        Some(Color::WHITE),
        Some((Color::rgba(122, 128, 138, 255), 1)),
    );
    let text = line_edit.text().to_string();
    if !text.is_empty() {
        layer.push(RenderCommand::DrawText {
            origin: centered_text_origin(line_edit.geometry()),
            text,
            font: line_edit.font().cloned().unwrap_or_default(),
            color: line_edit
                .foreground_color()
                .unwrap_or(Color::rgba(26, 26, 26, 255)),
        });
    }
}
/// Append visual commands for a `ComboBox` baseline representation.
#[deprecated(note = "Pipeline routing is unstable. Use RenderContext directly instead.")]
pub fn append_combo_box_visual_commands(layer: &mut SceneLayer, combo_box: &ComboBox) {
    let rect = combo_box.geometry();
    if is_empty_rect(&rect) {
        return;
    }
    // Render main background
    layer.push(RenderCommand::FillRect {
        rect,
        color: Color::WHITE,
    });
    layer.push(RenderCommand::DrawRectStroke {
        rect,
        color: Color::rgba(122, 128, 138, 255),
        width: 1,
    });
    let arrow_width = 14u32.min(rect.width);
    let arrow_rect = Rect {
        x: rect.x + rect.width as f32 as i32 - arrow_width as i32,
        y: rect.y,
        width: arrow_width,
        height: rect.height,
    };
    layer.push(RenderCommand::FillRect {
        rect: arrow_rect,
        color: Color::rgba(235, 238, 243, 255),
    });
    layer.push(RenderCommand::DrawRectStroke {
        rect: arrow_rect,
        color: Color::rgba(122, 128, 138, 255),
        width: 1,
    });
    let text = combo_box.current_text();
    if !text.is_empty() {
        layer.push(RenderCommand::DrawText {
            origin: centered_text_origin(rect),
            text,
            font: combo_box.font().cloned().unwrap_or_default(),
            color: combo_box
                .foreground_color()
                .unwrap_or(Color::rgba(28, 30, 34, 255)),
        });
    }
    if combo_box.count() > 0 {
        let popup_rows = combo_box.count().min(4) as u32;
        let row_height = rect.height.max(16);
        let popup_rect = Rect {
            x: rect.x,
            y: rect.y + rect.height as f32 as i32,
            width: rect.width,
            height: row_height.saturating_mul(popup_rows),
        };
        layer.push(RenderCommand::FillRect {
            rect: popup_rect,
            color: Color::rgba(250, 250, 252, 255),
        });
        layer.push(RenderCommand::DrawRectStroke {
            rect: popup_rect,
            color: Color::rgba(122, 128, 138, 255),
            width: 1,
        });
        for row in 0..popup_rows as usize {
            let item_rect = Rect {
                x: popup_rect.x + 2,
                y: popup_rect.y + row as i32 * row_height as i32,
                width: popup_rect.width.saturating_sub(4),
                height: row_height,
            };
            if combo_box.current_index() == Some(row) {
                layer.push(RenderCommand::FillRect {
                    rect: item_rect,
                    color: Color::rgba(206, 226, 255, 255),
                });
            }
            if let Some(item) = combo_box.item(row) {
                layer.push(RenderCommand::DrawText {
                    origin: centered_text_origin(item_rect),
                    text: item.to_string(),
                    font: combo_box.font().cloned().unwrap_or_default(),
                    color: Color::rgba(28, 30, 34, 255),
                });
            }
        }
    }
}
/// Append visual commands for a `ListBox` baseline representation.
#[deprecated(note = "Pipeline routing is unstable. Use RenderContext directly instead.")]
pub fn append_list_box_visual_commands(layer: &mut SceneLayer, list_box: &ListBox) {
    push_widget_fill_and_border(
        layer,
        list_box,
        Some(Color::rgba(252, 252, 253, 255)),
        Some((Color::rgba(122, 128, 138, 255), 1)),
    );
    let rect = list_box.geometry();
    if is_empty_rect(&rect) {
        return;
    }
    let row_height = 16u32;
    let max_rows = (rect.height / row_height).clamp(1, 4) as usize;
    for row in 0..list_box.count().min(max_rows) {
        let item_rect = Rect {
            x: rect.x + 2,
            y: rect.y + 2 + row as i32 * row_height as i32,
            width: rect.width.saturating_sub(4),
            height: row_height,
        };
        if let Some(item) = list_box.item(row) {
            layer.push(RenderCommand::DrawText {
                origin: centered_text_origin(item_rect),
                text: item.to_string(),
                font: list_box.font().cloned().unwrap_or_default(),
                color: list_box
                    .foreground_color()
                    .unwrap_or(Color::rgba(30, 32, 36, 255)),
            });
        }
    }
}
/// Append visual commands for a `ProgressBar` value representation.
#[deprecated(note = "Pipeline routing is unstable. Use RenderContext directly instead.")]
pub fn append_progress_bar_visual_commands(layer: &mut SceneLayer, progress_bar: &ProgressBar) {
    push_widget_fill_and_border(
        layer,
        progress_bar,
        Some(Color::rgba(232, 236, 243, 255)),
        Some((Color::rgba(122, 128, 138, 255), 1)),
    );
    let rect = progress_bar.geometry();
    if is_empty_rect(&rect) {
        return;
    }
    let ratio = normalized_progress_i32(
        progress_bar.value(),
        progress_bar.minimum(),
        progress_bar.maximum(),
    );
    let filled_width = ((rect.width as f32) * ratio).round() as u32;
    if filled_width > 0 {
        layer.push(RenderCommand::FillRect {
            rect: Rect {
                x: rect.x,
                y: rect.y,
                width: filled_width.min(rect.width),
                height: rect.height,
            },
            color: progress_bar.foreground_color().unwrap_or(Color::PRIMARY),
        });
    }
}
/// Append visual commands for a `Slider` value representation.
#[deprecated(note = "Pipeline routing is unstable. Use RenderContext directly instead.")]
pub fn append_slider_visual_commands(layer: &mut SceneLayer, slider: &Slider) {
    let rect = slider.geometry();
    if is_empty_rect(&rect) {
        return;
    }
    layer.push(RenderCommand::FillRect {
        rect,
        color: slider
            .background_color()
            .unwrap_or(Color::rgba(238, 241, 246, 255)),
    });
    let ratio = normalized_progress_i32(slider.value(), slider.minimum(), slider.maximum());
    if rect.width >= rect.height {
        let track_y = rect.y + rect.height as f32 as i32 / 2;
        layer.push(RenderCommand::DrawLineStroke {
            from: Point {
                x: rect.x + 4,
                y: track_y,
            },
            to: Point {
                x: rect.x + rect.width as f32 as i32 - 4,
                y: track_y,
            },
            color: slider
                .border_color()
                .unwrap_or(Color::rgba(126, 132, 142, 255)),
            width: 2,
        });
        let thumb_x = rect.x + ((rect.width.saturating_sub(1) as f32) * ratio).round() as i32;
        layer.push(RenderCommand::FillCircle {
            center: Point {
                x: thumb_x,
                y: track_y,
            },
            radius: (rect.height / 3).max(3),
            color: slider
                .foreground_color()
                .unwrap_or(Color::rgba(70, 140, 248, 255)),
        });
    } else {
        let track_x = rect.x + rect.width as f32 as i32 / 2;
        layer.push(RenderCommand::DrawLineStroke {
            from: Point {
                x: track_x,
                y: rect.y + 4,
            },
            to: Point {
                x: track_x,
                y: rect.y + rect.height as f32 as i32 - 4,
            },
            color: slider
                .border_color()
                .unwrap_or(Color::rgba(126, 132, 142, 255)),
            width: 2,
        });
        let thumb_y = rect.y + ((rect.height.saturating_sub(1) as f32) * ratio).round() as i32;
        layer.push(RenderCommand::FillCircle {
            center: Point {
                x: track_x,
                y: thumb_y,
            },
            radius: (rect.width / 3).max(3),
            color: slider
                .foreground_color()
                .unwrap_or(Color::rgba(70, 140, 248, 255)),
        });
    }
}
/// Append visual commands for a `ScrollBar` value representation.
#[deprecated(note = "Pipeline routing is unstable. Use RenderContext directly instead.")]
pub fn append_scroll_bar_visual_commands(layer: &mut SceneLayer, scroll_bar: &ScrollBar) {
    let rect = scroll_bar.geometry();
    if is_empty_rect(&rect) {
        return;
    }
    layer.push(RenderCommand::FillRect {
        rect,
        color: scroll_bar
            .background_color()
            .unwrap_or(Color::rgba(229, 233, 239, 255)),
    });
    let ratio = normalized_progress_i32(
        scroll_bar.value(),
        scroll_bar.minimum(),
        scroll_bar.maximum(),
    );
    let denom = (scroll_bar.maximum() - scroll_bar.minimum()).max(1) as f32;
    let page_ratio = (scroll_bar.page_step().max(1) as f32
        / (denom + scroll_bar.page_step().max(1) as f32))
        .clamp(0.1, 1.0);
    if rect.width >= rect.height {
        let thumb_width = ((rect.width as f32) * page_ratio).round() as u32;
        let travel = rect.width.saturating_sub(thumb_width);
        let thumb_x = rect.x + ((travel as f32) * ratio).round() as i32;
        layer.push(RenderCommand::FillRect {
            rect: Rect {
                x: thumb_x,
                y: rect.y,
                width: thumb_width.max(6).min(rect.width),
                height: rect.height,
            },
            color: scroll_bar
                .foreground_color()
                .unwrap_or(Color::rgba(144, 151, 164, 255)),
        });
    } else {
        let thumb_height = ((rect.height as f32) * page_ratio).round() as u32;
        let travel = rect.height.saturating_sub(thumb_height);
        let thumb_y = rect.y + ((travel as f32) * ratio).round() as i32;
        layer.push(RenderCommand::FillRect {
            rect: Rect {
                x: rect.x,
                y: thumb_y,
                width: rect.width,
                height: thumb_height.max(6).min(rect.height),
            },
            color: scroll_bar
                .foreground_color()
                .unwrap_or(Color::rgba(144, 151, 164, 255)),
        });
    }
}
