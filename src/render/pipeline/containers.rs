//! Container and complex widgets: tab_widget, text_edit, rich_edit, tree_view,
//! table_widget, grid_widget, chart_widget, dock_panel, group_box, splitter, mdi_area,
//! canvas, spin_box, list_view, scroll_area.
//!
//! Also contains the `impl SoftwareSurface` block with all software rendering methods.

use crate::core::{Color, Font, Point, Rect, Size};
use crate::render::{BackBuffer, SceneLayer, RenderCommand, ShapedText, SoftwareRenderConfig, SoftwareSurface, TextCluster, TextMetrics};
use crate::render::{default_software_render_config, is_empty_rect};
use crate::render::pipeline::controls::{
    push_widget_fill_and_border, centered_text_origin,
    normalized_progress_u32, normalized_progress_i32,
};
use crate::render::pipeline::pixel_ops::{fill_pixels, blend_pixel, set_pixel,
    circle_fill_coverage_grid, circle_stroke_coverage_grid, line_stroke_coverage_grid,
    rounded_rect_coverage_grid, draw_bitmap_glyph, glyph_bitmap,
    cluster_ends_with_zwj, is_combining_mark, is_variation_selector, is_wide_scalar,
    estimate_cluster_advance, pixel_bytes_len,
    rounded_rect_effective_radius, inset_rect, point_in_rounded_rect_f32,
    rounded_rect_coverage,
};
use crate::widget::{
    Canvas, ChartWidget, DockPanel, GridWidget, GroupBox, ListView, MdiArea,
    RichEdit, ScrollArea, SpinBox, Splitter, TabWidget, TableWidget, TextEdit, TreeView, Widget,
};

pub fn append_tab_widget_visual_commands(layer: &mut SceneLayer, tab_widget: &TabWidget) {
    push_widget_fill_and_border(
        layer,
        tab_widget,
        Some(Color::rgba(245, 247, 252, 255)),
        Some((Color::rgba(126, 132, 142, 255), 1)),
    );
    let rect = tab_widget.geometry();
    if is_empty_rect(&rect) {
        return;
    }
    let count = tab_widget.count().max(1);
    let tab_height = rect.height.min(26);
    let tab_width = (rect.width / count as u32).max(24);
    for index in 0..count {
        let tab_rect = Rect {
            x: rect.x + (index as u32 * tab_width) as i32,
            y: rect.y,
            width: tab_width.min(rect.width),
            height: tab_height,
        };
        let is_current = tab_widget.current_index() == index;
        layer.push(RenderCommand::FillRect {
            rect: tab_rect,
            color: if is_current {
                Color::rgba(210, 224, 248, 255)
            } else {
                Color::rgba(229, 234, 242, 255)
            },
        });
        layer.push(RenderCommand::DrawText {
            origin: centered_text_origin(tab_rect),
            text: format!("Tab{}", index + 1),
            font: tab_widget.font().cloned().unwrap_or_default(),
            color: tab_widget
                .foreground_color()
                .unwrap_or(Color::rgba(30, 32, 36, 255)),
        });
    }
}
/// Append visual commands for a `TextEdit` multi-line text editor representation.
pub fn append_text_edit_visual_commands(layer: &mut SceneLayer, text_edit: &TextEdit) {
    push_widget_fill_and_border(
        layer,
        text_edit,
        Some(Color::WHITE),
        Some((Color::rgba(122, 128, 138, 255), 1)),
    );
    let text = text_edit.text();
    if !text.is_empty() {
        let rect = text_edit.geometry();
        let padding = 4i32;
        let text_rect = Rect {
            x: rect.x + padding,
            y: rect.y + padding,
            width: rect.width.saturating_sub(padding as u32 * 2),
            height: rect.height.saturating_sub(padding as u32 * 2),
        };
        layer.push(RenderCommand::DrawText {
            origin: Point {
                x: text_rect.x,
                y: text_rect.y + text_rect.height as i32 / 2,
            },
            text: text.to_string(),
            font: text_edit.font().cloned().unwrap_or_default(),
            color: text_edit
                .foreground_color()
                .unwrap_or(Color::rgba(26, 26, 26, 255)),
        });
    }
}
/// Append visual commands for a `RichEdit` rich text editor representation.
pub fn append_rich_edit_visual_commands(layer: &mut SceneLayer, rich_edit: &RichEdit) {
    let bg_color = if rich_edit.is_read_only() {
        Color::rgba(245, 245, 245, 255)
    } else {
        Color::WHITE
    };
    push_widget_fill_and_border(
        layer,
        rich_edit,
        Some(bg_color),
        Some((Color::rgba(122, 128, 138, 255), 1)),
    );
    let text = rich_edit.text();
    if !text.is_empty() {
        let rect = rich_edit.geometry();
        let padding = 4i32;
        let text_rect = Rect {
            x: rect.x + padding,
            y: rect.y + padding,
            width: rect.width.saturating_sub(padding as u32 * 2),
            height: rect.height.saturating_sub(padding as u32 * 2),
        };
        layer.push(RenderCommand::DrawText {
            origin: Point {
                x: text_rect.x,
                y: text_rect.y + text_rect.height as i32 / 2,
            },
            text: text.to_string(),
            font: rich_edit.font().cloned().unwrap_or_default(),
            color: rich_edit
                .foreground_color()
                .unwrap_or(Color::rgba(26, 26, 26, 255)),
        });
    }
    // Draw selection highlight if present
    if let Some((start, end)) = rich_edit.selection() {
        if start != end {
            let rect = rich_edit.geometry();
            let padding = 4i32;
            let selection_width = ((end - start) as u32).min(20);
            layer.push(RenderCommand::FillRect {
                rect: Rect {
                    x: rect.x + padding,
                    y: rect.y + padding,
                    width: selection_width,
                    height: rect.height.saturating_sub(padding as u32 * 2),
                },
                color: Color::rgba(128, 192, 255, 128),
            });
        }
    }
}
/// Append visual commands for a `TreeView` hierarchical data display representation.
pub fn append_tree_view_visual_commands(layer: &mut SceneLayer, tree_view: &TreeView) {
    push_widget_fill_and_border(
        layer,
        tree_view,
        Some(Color::WHITE),
        Some((Color::rgba(122, 128, 138, 255), 1)),
    );
    let rect = tree_view.geometry();
    if is_empty_rect(&rect) {
        return;
    }
    // Draw header area
    let header_height = 20u32.min(rect.height);
    let header_rect = Rect {
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: header_height,
    };
    layer.push(RenderCommand::FillRect {
        rect: header_rect,
        color: Color::rgba(235, 238, 243, 255),
    });
    layer.push(RenderCommand::DrawRectStroke {
        rect: header_rect,
        color: Color::rgba(200, 205, 215, 255),
        width: 1,
    });
    // Draw tree icon placeholder
    let icon_size = 12u32.min(header_height);
    if icon_size > 0 {
        layer.push(RenderCommand::DrawRectStroke {
            rect: Rect {
                x: rect.x + 4,
                y: rect.y + 4,
                width: icon_size,
                height: icon_size,
            },
            color: Color::rgba(100, 100, 100, 255),
            width: 1,
        });
    }
    // Draw placeholder text for tree structure
    layer.push(RenderCommand::DrawText {
        origin: Point {
            x: rect.x + icon_size as i32 + 12,
            y: rect.y + header_height as i32 / 2,
        },
        text: "Tree".to_string(),
        font: tree_view.font().cloned().unwrap_or_default(),
        color: tree_view
            .foreground_color()
            .unwrap_or(Color::rgba(26, 26, 26, 255)),
    });
}
/// Append visual commands for a `TableWidget` data grid representation.
pub fn append_table_widget_visual_commands(layer: &mut SceneLayer, table_widget: &TableWidget) {
    push_widget_fill_and_border(
        layer,
        table_widget,
        Some(Color::WHITE),
        Some((Color::rgba(122, 128, 138, 255), 1)),
    );
    let rect = table_widget.geometry();
    if is_empty_rect(&rect) {
        return;
    }
    // Draw header row
    let header_height = 20u32.min(rect.height / 4).max(16);
    let header_rect = Rect {
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: header_height,
    };
    layer.push(RenderCommand::FillRect {
        rect: header_rect,
        color: Color::rgba(235, 238, 243, 255),
    });
    layer.push(RenderCommand::DrawRectStroke {
        rect: header_rect,
        color: Color::rgba(200, 205, 215, 255),
        width: 1,
    });
    // Draw column dividers
    let column_count = 3u32;
    let column_width = rect.width / column_count;
    for i in 1..column_count {
        let x = rect.x + (i * column_width) as i32;
        layer.push(RenderCommand::DrawLineStroke {
            from: Point { x, y: rect.y },
            to: Point {
                x,
                y: rect.y + header_height as i32,
            },
            color: Color::rgba(200, 205, 215, 255),
            width: 1,
        });
    }
    // Draw data rows placeholder
    let row_height = 18u32;
    let data_height = rect.height.saturating_sub(header_height);
    let visible_rows = data_height / row_height;
    for row in 0..visible_rows.min(10) {
        let y = rect.y + header_height as i32 + (row * row_height) as i32;
        if y + row_height as i32 > rect.y + rect.height as f32 as i32 {
            break;
        }
        // Row background (alternating)
        if row % 2 == 1 {
            layer.push(RenderCommand::FillRect {
                rect: Rect {
                    x: rect.x,
                    y,
                    width: rect.width,
                    height: row_height,
                },
                color: Color::rgba(250, 250, 252, 255),
            });
        }
        // Row divider
        layer.push(RenderCommand::DrawLineStroke {
            from: Point { x: rect.x, y },
            to: Point {
                x: rect.x + rect.width as f32 as i32,
                y,
            },
            color: Color::rgba(230, 232, 238, 255),
            width: 1,
        });
    }
}
/// Append visual commands for a `GridWidget` layout container representation.
pub fn append_grid_widget_visual_commands(layer: &mut SceneLayer, grid_widget: &GridWidget) {
    push_widget_fill_and_border(
        layer,
        grid_widget,
        Some(Color::rgba(250, 250, 252, 255)),
        Some((Color::rgba(180, 185, 195, 255), 1)),
    );
    let rect = grid_widget.geometry();
    if is_empty_rect(&rect) {
        return;
    }
    // Draw grid lines
    let rows = 4u32;
    let cols = 4u32;
    let cell_width = rect.width / cols;
    let cell_height = rect.height / rows;
    // Horizontal lines
    for i in 1..rows {
        let y = rect.y + (i * cell_height) as i32;
        layer.push(RenderCommand::DrawLineStroke {
            from: Point { x: rect.x, y },
            to: Point {
                x: rect.x + rect.width as f32 as i32,
                y,
            },
            color: Color::rgba(210, 215, 225, 255),
            width: 1,
        });
    }
    // Vertical lines
    for i in 1..cols {
        let x = rect.x + (i * cell_width) as i32;
        layer.push(RenderCommand::DrawLineStroke {
            from: Point { x, y: rect.y },
            to: Point {
                x,
                y: rect.y + rect.height as f32 as i32,
            },
            color: Color::rgba(210, 215, 225, 255),
            width: 1,
        });
    }
}
/// Append visual commands for a `ChartWidget` data visualization representation.
pub fn append_chart_widget_visual_commands(layer: &mut SceneLayer, chart_widget: &ChartWidget) {
    push_widget_fill_and_border(
        layer,
        chart_widget,
        Some(Color::WHITE),
        Some((Color::rgba(122, 128, 138, 255), 1)),
    );
    let rect = chart_widget.geometry();
    if is_empty_rect(&rect) {
        return;
    }
    let padding = 20i32;
    let chart_rect = Rect {
        x: rect.x + padding,
        y: rect.y + padding,
        width: rect.width.saturating_sub(padding as u32 * 2),
        height: rect.height.saturating_sub(padding as u32 * 2),
    };
    if chart_rect.width == 0 || chart_rect.height == 0 {
        return;
    }
    // Draw chart background
    layer.push(RenderCommand::FillRect {
        rect: chart_rect,
        color: Color::rgba(248, 249, 250, 255),
    });
    // Draw axis lines
    layer.push(RenderCommand::DrawLineStroke {
        from: Point {
            x: chart_rect.x,
            y: chart_rect.y + chart_rect.height as i32,
        },
        to: Point {
            x: chart_rect.x + chart_rect.width as i32,
            y: chart_rect.y + chart_rect.height as i32,
        },
        color: Color::rgba(100, 100, 100, 255),
        width: 2,
    });
    layer.push(RenderCommand::DrawLineStroke {
        from: Point {
            x: chart_rect.x,
            y: chart_rect.y,
        },
        to: Point {
            x: chart_rect.x,
            y: chart_rect.y + chart_rect.height as i32,
        },
        color: Color::rgba(100, 100, 100, 255),
        width: 2,
    });
    // Draw sample bar chart bars
    let bar_count = 5u32;
    let bar_width = chart_rect.width / (bar_count * 2);
    let max_bar_height = chart_rect.height.saturating_sub(10);
    for i in 0..bar_count {
        let bar_height = max_bar_height * (i + 1) / bar_count;
        let x = chart_rect.x + (i * bar_width * 2) as i32 + bar_width as i32 / 2;
        let y = chart_rect.y + chart_rect.height as i32 - bar_height as i32;
        layer.push(RenderCommand::FillRect {
            rect: Rect {
                x,
                y,
                width: bar_width,
                height: bar_height,
            },
            color: Color::rgba(66, 133, 244, 200),
        });
        layer.push(RenderCommand::DrawRectStroke {
            rect: Rect {
                x,
                y,
                width: bar_width,
                height: bar_height,
            },
            color: Color::rgba(66, 133, 244, 255),
            width: 1,
        });
    }
}
/// Append visual commands for a `DockPanel` docking container representation.
pub fn append_dock_panel_visual_commands(layer: &mut SceneLayer, dock_panel: &DockPanel) {
    push_widget_fill_and_border(
        layer,
        dock_panel,
        Some(Color::BACKGROUND),
        Some((Color::rgba(160, 168, 180, 255), 1)),
    );
    let rect = dock_panel.geometry();
    if is_empty_rect(&rect) {
        return;
    }
    // Draw dock area dividers
    let center_x = rect.x + rect.width as f32 as i32 / 2;
    let center_y = rect.y + rect.height as f32 as i32 / 2;
    // Vertical center divider
    layer.push(RenderCommand::DrawLineStroke {
        from: Point {
            x: center_x,
            y: rect.y + 4,
        },
        to: Point {
            x: center_x,
            y: rect.y + rect.height as f32 as i32 - 4,
        },
        color: Color::rgba(200, 205, 215, 255),
        width: 2,
    });
    // Horizontal center divider
    layer.push(RenderCommand::DrawLineStroke {
        from: Point {
            x: rect.x + 4,
            y: center_y,
        },
        to: Point {
            x: rect.x + rect.width as f32 as i32 - 4,
            y: center_y,
        },
        color: Color::rgba(200, 205, 215, 255),
        width: 2,
    });
}
/// Append visual commands for a `GroupBox` titled container representation.
pub fn append_group_box_visual_commands(layer: &mut SceneLayer, group_box: &GroupBox) {
    let rect = group_box.geometry();
    // Draw the main border with title area
    let title_height = 16i32;
    layer.push(RenderCommand::DrawRectStroke {
        rect: Rect {
            x: rect.x,
            y: rect.y + title_height / 2,
            width: rect.width,
            height: rect.height.saturating_sub(title_height as u32 / 2),
        },
        color: Color::rgba(140, 145, 155, 255),
        width: 1,
    });
    // Fill title background
    layer.push(RenderCommand::FillRect {
        rect: Rect {
            x: rect.x + 8,
            y: rect.y,
            width: 60,
            height: title_height as u32,
        },
        color: Color::BACKGROUND,
    });
    // Draw title text
    layer.push(RenderCommand::DrawText {
        origin: Point {
            x: rect.x + 12,
            y: rect.y + title_height / 2,
        },
        text: "Group".to_string(),
        font: group_box.font().cloned().unwrap_or_default(),
        color: group_box
            .foreground_color()
            .unwrap_or(Color::rgba(50, 52, 56, 255)),
    });
}
/// Append visual commands for a `Splitter` resizable divider representation.
pub fn append_splitter_visual_commands(layer: &mut SceneLayer, splitter: &Splitter) {
    push_widget_fill_and_border(
        layer,
        splitter,
        Some(Color::rgba(235, 238, 243, 255)),
        Some((Color::rgba(180, 185, 195, 255), 1)),
    );
    let rect = splitter.geometry();
    if is_empty_rect(&rect) {
        return;
    }
    // Draw gripper dots/lines
    let is_horizontal = rect.width > rect.height;
    if is_horizontal {
        // Horizontal splitter - vertical gripper line
        let center_x = rect.x + rect.width as f32 as i32 / 2;
        layer.push(RenderCommand::DrawLineStroke {
            from: Point {
                x: center_x,
                y: rect.y + 4,
            },
            to: Point {
                x: center_x,
                y: rect.y + rect.height as f32 as i32 - 4,
            },
            color: Color::rgba(160, 165, 175, 255),
            width: 2,
        });
    } else {
        // Vertical splitter - horizontal gripper line
        let center_y = rect.y + rect.height as f32 as i32 / 2;
        layer.push(RenderCommand::DrawLineStroke {
            from: Point {
                x: rect.x + 4,
                y: center_y,
            },
            to: Point {
                x: rect.x + rect.width as f32 as i32 - 4,
                y: center_y,
            },
            color: Color::rgba(160, 165, 175, 255),
            width: 2,
        });
    }
}
/// Append visual commands for an `MdiArea` multiple document interface representation.
pub fn append_mdi_area_visual_commands(layer: &mut SceneLayer, mdi_area: &MdiArea) {
    push_widget_fill_and_border(
        layer,
        mdi_area,
        Some(Color::rgba(220, 225, 232, 255)),
        Some((Color::rgba(140, 148, 160, 255), 1)),
    );
    let rect = mdi_area.geometry();
    if is_empty_rect(&rect) {
        return;
    }
    // Draw placeholder child window frames
    let child_rect = Rect {
        x: rect.x + 10,
        y: rect.y + 10,
        width: (rect.width / 2).saturating_sub(15),
        height: (rect.height / 2).saturating_sub(15),
    };
    if child_rect.width > 0 && child_rect.height > 0 {
        // Child window background
        layer.push(RenderCommand::FillRect {
            rect: child_rect,
            color: Color::WHITE,
        });
        // Child window title bar
        layer.push(RenderCommand::FillRect {
            rect: Rect {
                x: child_rect.x,
                y: child_rect.y,
                width: child_rect.width,
                height: 20u32.min(child_rect.height),
            },
            color: Color::rgba(66, 133, 244, 255),
        });
        // Child window border
        layer.push(RenderCommand::DrawRectStroke {
            rect: child_rect,
            color: Color::rgba(120, 128, 140, 255),
            width: 1,
        });
    }
}
/// Append visual commands for a `Canvas` drawing surface representation.
pub fn append_canvas_visual_commands(layer: &mut SceneLayer, canvas: &Canvas) {
    push_widget_fill_and_border(
        layer,
        canvas,
        Some(Color::WHITE),
        Some((Color::rgba(100, 108, 120, 255), 1)),
    );
    let rect = canvas.geometry();
    if is_empty_rect(&rect) {
        return;
    }
    // Draw canvas grid pattern
    let grid_size = 20u32;
    let cols = rect.width / grid_size;
    let rows = rect.height / grid_size;
    // Light grid dots
    for row in 0..rows {
        for col in 0..cols {
            let x = rect.x + (col * grid_size) as i32 + grid_size as i32 / 2;
            let y = rect.y + (row * grid_size) as i32 + grid_size as i32 / 2;
            layer.push(RenderCommand::FillRect {
                rect: Rect {
                    x,
                    y,
                    width: 1,
                    height: 1,
                },
                color: Color::rgba(220, 225, 235, 255),
            });
        }
    }
}
/// Append visual commands for a `SpinBox` numeric input control.
pub fn append_spin_box_visual_commands(layer: &mut SceneLayer, spin_box: &crate::widget::SpinBox) {
    push_widget_fill_and_border(
        layer,
        spin_box,
        Some(Color::WHITE),
        Some((Color::rgba(160, 168, 180, 255), 1)),
    );
    let rect = spin_box.geometry();
    if is_empty_rect(&rect) {
        return;
    }
    // Up/down button width
    let button_width = (rect.width / 5).clamp(16, 24);
    let value_area_width = rect.width.saturating_sub(button_width);
    // Draw value text
    let value_text = spin_box.value().to_string();
    let text_color = spin_box
        .foreground_color()
        .unwrap_or(Color::rgba(40, 44, 52, 255));
    let padding = 4i32;
    layer.push(RenderCommand::DrawText {
        text: value_text,
        origin: Point {
            x: rect.x + padding,
            y: rect.y + rect.height as f32 as i32 / 2,
        },
        font: spin_box.font().cloned().unwrap_or_default(),
        color: text_color,
    });
    // Draw up button (top half of right side)
    let button_x = rect.x + value_area_width as i32;
    layer.push(RenderCommand::FillRect {
        rect: Rect {
            x: button_x,
            y: rect.y,
            width: button_width,
            height: rect.height / 2,
        },
        color: Color::rgba(240, 242, 245, 255),
    });
    // Draw up arrow
    let arrow_center_y = rect.y + rect.height as f32 as i32 / 4;
    let arrow_color = Color::rgba(80, 84, 92, 255);
    layer.push(RenderCommand::DrawLineStroke {
        from: Point {
            x: button_x + button_width as i32 / 2 - 3,
            y: arrow_center_y + 2,
        },
        to: Point {
            x: button_x + button_width as i32 / 2,
            y: arrow_center_y - 2,
        },
        color: arrow_color,
        width: 1,
    });
    layer.push(RenderCommand::DrawLineStroke {
        from: Point {
            x: button_x + button_width as i32 / 2,
            y: arrow_center_y - 2,
        },
        to: Point {
            x: button_x + button_width as i32 / 2 + 3,
            y: arrow_center_y + 2,
        },
        color: arrow_color,
        width: 1,
    });
    // Draw down button (bottom half of right side)
    layer.push(RenderCommand::FillRect {
        rect: Rect {
            x: button_x,
            y: rect.y + rect.height as f32 as i32 / 2,
            width: button_width,
            height: rect.height / 2,
        },
        color: Color::rgba(240, 242, 245, 255),
    });
    // Draw down arrow
    let arrow_center_y2 = rect.y + rect.height as f32 as i32 * 3 / 4;
    layer.push(RenderCommand::DrawLineStroke {
        from: Point {
            x: button_x + button_width as i32 / 2 - 3,
            y: arrow_center_y2 - 2,
        },
        to: Point {
            x: button_x + button_width as i32 / 2,
            y: arrow_center_y2 + 2,
        },
        color: arrow_color,
        width: 1,
    });
    layer.push(RenderCommand::DrawLineStroke {
        from: Point {
            x: button_x + button_width as i32 / 2,
            y: arrow_center_y2 + 2,
        },
        to: Point {
            x: button_x + button_width as i32 / 2 + 3,
            y: arrow_center_y2 - 2,
        },
        color: arrow_color,
        width: 1,
    });
    // Button separator lines
    layer.push(RenderCommand::DrawLineStroke {
        from: Point {
            x: button_x,
            y: rect.y,
        },
        to: Point {
            x: button_x,
            y: rect.y + rect.height as f32 as i32,
        },
        color: Color::rgba(160, 168, 180, 255),
        width: 1,
    });
    layer.push(RenderCommand::DrawLineStroke {
        from: Point {
            x: button_x,
            y: rect.y + rect.height as f32 as i32 / 2,
        },
        to: Point {
            x: button_x + button_width as i32,
            y: rect.y + rect.height as f32 as i32 / 2,
        },
        color: Color::rgba(160, 168, 180, 255),
        width: 1,
    });
}
/// Append visual commands for a `ListView` widget representation.
pub fn append_list_view_visual_commands(
    layer: &mut SceneLayer,
    list_view: &crate::widget::ListView,
) {
    push_widget_fill_and_border(
        layer,
        list_view,
        Some(Color::WHITE),
        Some((Color::rgba(160, 168, 180, 255), 1)),
    );
    let rect = list_view.geometry();
    if is_empty_rect(&rect) {
        return;
    }
    let row_height = 24u32;
    let padding = 8i32;
    let text_color = Color::rgba(40, 44, 52, 255);
    let selected_bg = Color::PRIMARY;
    let selected_text = Color::WHITE;
    let font = Font::default_ui();
    let visible_rows = (rect.height / row_height) as usize;
    let row_count = list_view.row_count().min(visible_rows);
    for row in 0..row_count {
        let row_y = rect.y + (row as u32 * row_height) as i32;
        let is_selected = list_view.selected_row() == Some(row);
        let is_focused = list_view.focused_row() == Some(row);
        // Draw selection background
        if is_selected {
            layer.push(RenderCommand::FillRect {
                rect: Rect {
                    x: rect.x,
                    y: row_y,
                    width: rect.width,
                    height: row_height,
                },
                color: selected_bg,
            });
        }
        // Draw focus indicator
        if is_focused && !is_selected {
            layer.push(RenderCommand::DrawRectStroke {
                rect: Rect {
                    x: rect.x + 1,
                    y: row_y + 1,
                    width: rect.width.saturating_sub(2),
                    height: row_height.saturating_sub(2),
                },
                color: Color::PRIMARY,
                width: 1,
            });
        }
        // Draw item text
        if let Some(text) = list_view.item(row) {
            layer.push(RenderCommand::DrawText {
                text,
                origin: Point {
                    x: rect.x + padding,
                    y: row_y + row_height as i32 / 2,
                },
                font: font.clone(),
                color: if is_selected {
                    selected_text
                } else {
                    text_color
                },
            });
        }
    }
}
/// Append visual commands for a `ScrollArea` scrollable container.
pub fn append_scroll_area_visual_commands(
    layer: &mut SceneLayer,
    scroll_area: &crate::widget::ScrollArea,
) {
    push_widget_fill_and_border(
        layer,
        scroll_area,
        Some(Color::rgba(250, 250, 252, 255)),
        Some((Color::rgba(180, 188, 200, 255), 1)),
    );
    let rect = scroll_area.geometry();
    if is_empty_rect(&rect) {
        return;
    }
    let viewport = scroll_area.viewport();
    let scroll_offset = viewport.position();
    let viewport_size = viewport.size();
    // ScrollArea does not currently expose content geometry; use viewport as a safe baseline.
    let content_size = viewport.size();
    // Calculate scrollbar visibility and sizes
    let needs_h_scroll = content_size.width > viewport_size.width;
    let needs_v_scroll = content_size.height > viewport_size.height;
    let scrollbar_size = 12u32;
    // Horizontal scrollbar
    if needs_h_scroll {
        let h_track_y = rect.y + rect.height as f32 as i32 - scrollbar_size as i32;
        let h_track_width = if needs_v_scroll {
            rect.width.saturating_sub(scrollbar_size)
        } else {
            rect.width
        };
        // Track
        layer.push(RenderCommand::FillRect {
            rect: Rect {
                x: rect.x,
                y: h_track_y,
                width: h_track_width,
                height: scrollbar_size,
            },
            color: Color::rgba(232, 234, 238, 255),
        });
        // Thumb
        let h_ratio = viewport_size.width as f32 / content_size.width as f32;
        let h_thumb_width = (h_track_width as f32 * h_ratio).max(20.0) as u32;
        let h_max_offset = content_size.width.saturating_sub(viewport_size.width) as i32;
        let h_thumb_offset = if h_max_offset > 0 {
            (scroll_offset.x as f32 / h_max_offset as f32
                * (h_track_width.saturating_sub(h_thumb_width)) as f32) as i32
        } else {
            0
        };
        layer.push(RenderCommand::FillRect {
            rect: Rect {
                x: rect.x + h_thumb_offset,
                y: h_track_y + 2,
                width: h_thumb_width,
                height: scrollbar_size.saturating_sub(4),
            },
            color: Color::rgba(172, 178, 188, 255),
        });
    }
    // Vertical scrollbar
    if needs_v_scroll {
        let v_track_x = rect.x + rect.width as f32 as i32 - scrollbar_size as i32;
        let v_track_height = if needs_h_scroll {
            rect.height.saturating_sub(scrollbar_size)
        } else {
            rect.height
        };
        // Track
        layer.push(RenderCommand::FillRect {
            rect: Rect {
                x: v_track_x,
                y: rect.y,
                width: scrollbar_size,
                height: v_track_height,
            },
            color: Color::rgba(232, 234, 238, 255),
        });
        // Thumb
        let v_ratio = viewport_size.height as f32 / content_size.height as f32;
        let v_thumb_height = (v_track_height as f32 * v_ratio).max(20.0) as u32;
        let v_max_offset = content_size.height.saturating_sub(viewport_size.height) as i32;
        let v_thumb_offset = if v_max_offset > 0 {
            (scroll_offset.y as f32 / v_max_offset as f32
                * (v_track_height.saturating_sub(v_thumb_height)) as f32) as i32
        } else {
            0
        };
        layer.push(RenderCommand::FillRect {
            rect: Rect {
                x: v_track_x + 2,
                y: rect.y + v_thumb_offset,
                width: scrollbar_size.saturating_sub(4),
                height: v_thumb_height,
            },
            color: Color::rgba(172, 178, 188, 255),
        });
    }
    // Corner square (when both scrollbars are visible)
    if needs_h_scroll && needs_v_scroll {
        layer.push(RenderCommand::FillRect {
            rect: Rect {
                x: rect.x + rect.width as f32 as i32 - scrollbar_size as i32,
                y: rect.y + rect.height as f32 as i32 - scrollbar_size as i32,
                width: scrollbar_size,
                height: scrollbar_size,
            },
            color: Color::rgba(232, 234, 238, 255),
        });
    }
}
impl SoftwareSurface {
    /// Creates a software surface with size and DPI scale.
    pub fn new(size: Size, dpi_scale: f32) -> Self {
        let config = default_software_render_config();
        Self {
            buffer: BackBuffer::new(size, dpi_scale),
            aa_samples_per_axis: config.aa_samples_per_axis,
        }
    }
    /// Get current software render configuration.
    pub fn render_config(&self) -> SoftwareRenderConfig {
        SoftwareRenderConfig {
            aa_samples_per_axis: self.aa_samples_per_axis,
        }
    }
    /// Apply software render configuration.
    pub fn apply_render_config(&mut self, config: SoftwareRenderConfig) {
        let normalized = config.normalized();
        self.aa_samples_per_axis = normalized.aa_samples_per_axis;
    }
    /// Set anti-aliasing sample grid size per axis for high-sample raster paths.
    pub fn set_aa_samples_per_axis(&mut self, samples: u8) {
        self.apply_render_config(SoftwareRenderConfig {
            aa_samples_per_axis: samples,
        });
    }
    /// Get anti-aliasing sample grid size per axis.
    pub fn aa_samples_per_axis(&self) -> u8 {
        self.aa_samples_per_axis
    }
    /// Clears the current back buffer with a solid color.
    pub fn begin_frame(&mut self, clear: Color) {
        fill_pixels(self.buffer.back_mut(), clear);
    }
    /// Presents the back buffer as the current frame.
    pub fn end_frame(&mut self) {
        self.buffer.present();
    }
    /// Returns logical surface size.
    pub fn size(&self) -> Size {
        self.buffer.size()
    }
    /// Resizes the surface buffers.
    pub fn resize(&mut self, size: Size) {
        self.buffer.resize(size);
    }
    /// Sets logical DPI scale for text and geometry.
    pub fn set_dpi_scale(&mut self, dpi_scale: f32) {
        self.buffer.set_dpi_scale(dpi_scale);
    }
    /// Returns logical DPI scale.
    pub fn dpi_scale(&self) -> f32 {
        self.buffer.dpi_scale()
    }
    /// Returns RGBA bytes of the presented frame.
    pub fn frame_rgba(&self) -> &[u8] {
        self.buffer.front()
    }
    /// Measures text bounds and baseline metrics.
    pub fn measure_text(&self, text: &str, font: &Font) -> TextMetrics {
        let scale = self.buffer.dpi_scale();
        let line_height = (font.size * scale).max(1.0);
        let ascent = (line_height * 0.8) as u32;
        let descent = (line_height - ascent as f32).max(0.0) as u32;
        let shaped = self.shape_text(text, font);
        let width = shaped.advance().round() as u32;
        TextMetrics {
            width,
            height: line_height.round() as u32,
            ascent,
            descent,
        }
    }
    /// Shape text into unicode-aware clusters with logical advances.
    pub fn shape_text(&self, text: &str, font: &Font) -> ShapedText {
        let scale = self.buffer.dpi_scale();
        let mut clusters: Vec<TextCluster> = Vec::new();
        for scalar in text.chars() {
            let should_merge = clusters
                .last()
                .map(|cluster| {
                    cluster_ends_with_zwj(cluster)
                        || scalar == '\u{200D}'
                        || is_combining_mark(scalar)
                        || is_variation_selector(scalar)
                })
                .unwrap_or(false);
            if should_merge {
                if let Some(last) = clusters.last_mut() {
                    last.text.push(scalar);
                }
            } else {
                clusters.push(TextCluster {
                    text: scalar.to_string(),
                    advance: 0.0,
                });
            }
        }
        let mut total_advance = 0.0f32;
        for cluster in &mut clusters {
            cluster.advance = estimate_cluster_advance(&cluster.text, font.size, scale);
            total_advance += cluster.advance;
        }
        ShapedText {
            clusters,
            advance: total_advance,
        }
    }
    /// Fills a rectangle with a solid color.
    pub fn fill_rect(&mut self, rect: Rect, color: Color) {
        let size = self.buffer.size();
        let x0 = rect.x.max(0) as u32;
        let y0 = rect.y.max(0) as u32;
        let x1 = (rect.x + rect.width as f32 as i32).max(0) as u32;
        let y1 = (rect.y + rect.height as f32 as i32).max(0) as u32;
        let x1 = x1.min(size.width);
        let y1 = y1.min(size.height);
        let frame = self.buffer.back_mut();
        for y in y0..y1 {
            for x in x0..x1 {
                set_pixel(frame, size.width, x, y, color);
            }
        }
    }
    /// Draws a 1px rectangle stroke.
    pub fn draw_rect(&mut self, rect: Rect, color: Color) {
        self.draw_rect_with_width(rect, color, 1);
    }
    /// Draws a rectangle stroke with explicit width.
    pub fn draw_rect_with_width(&mut self, rect: Rect, color: Color, stroke_width: u32) {
        if stroke_width == 0 {
            return;
        }
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let x0 = rect.x;
        let y0 = rect.y;
        let x1 = rect.x + rect.width as f32 as i32 - 1;
        let y1 = rect.y + rect.height as f32 as i32 - 1;
        self.draw_line_with_width(
            Point { x: x0, y: y0 },
            Point { x: x1, y: y0 },
            color,
            stroke_width,
        );
        self.draw_line_with_width(
            Point { x: x0, y: y1 },
            Point { x: x1, y: y1 },
            color,
            stroke_width,
        );
        self.draw_line_with_width(
            Point { x: x0, y: y0 },
            Point { x: x0, y: y1 },
            color,
            stroke_width,
        );
        self.draw_line_with_width(
            Point { x: x1, y: y0 },
            Point { x: x1, y: y1 },
            color,
            stroke_width,
        );
    }
    /// Fills a rounded rectangle using coverage blending.
    pub fn fill_rounded_rect(&mut self, rect: Rect, radius: u32, color: Color) {
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let size = self.buffer.size();
        let width = size.width as i32;
        let height = size.height as i32;
        let frame = self.buffer.back_mut();
        let x0 = rect.x.max(0);
        let y0 = rect.y.max(0);
        let x1 = (rect.x + rect.width as f32 as i32 - 1).min(width - 1);
        let y1 = (rect.y + rect.height as f32 as i32 - 1).min(height - 1);
        let effective_radius = rounded_rect_effective_radius(rect, radius);
        for py in y0..=y1 {
            for px in x0..=x1 {
                let coverage = rounded_rect_coverage(px, py, rect, effective_radius);
                if coverage > 0.0 {
                    blend_pixel(frame, size.width, px as u32, py as u32, color, coverage);
                }
            }
        }
    }
    /// Fill rounded-rectangle with stronger anti-aliasing sampling.
    pub fn fill_rounded_rect_aa(&mut self, rect: Rect, radius: u32, color: Color) {
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let sample_grid = self.aa_samples_per_axis;
        let size = self.buffer.size();
        let width = size.width as i32;
        let height = size.height as i32;
        let frame = self.buffer.back_mut();
        let x0 = rect.x.max(0);
        let y0 = rect.y.max(0);
        let x1 = (rect.x + rect.width as f32 as i32 - 1).min(width - 1);
        let y1 = (rect.y + rect.height as f32 as i32 - 1).min(height - 1);
        let effective_radius = rounded_rect_effective_radius(rect, radius);
        for py in y0..=y1 {
            for px in x0..=x1 {
                let coverage =
                    rounded_rect_coverage_grid(px, py, rect, effective_radius, sample_grid);
                if coverage > 0.0 {
                    blend_pixel(frame, size.width, px as u32, py as u32, color, coverage);
                }
            }
        }
    }
    /// Draws a rounded rectangle stroke with explicit width.
    pub fn draw_rounded_rect_with_width(
        &mut self,
        rect: Rect,
        radius: u32,
        color: Color,
        stroke_width: u32,
    ) {
        if stroke_width == 0 || rect.width == 0 || rect.height == 0 {
            return;
        }
        let size = self.buffer.size();
        let width = size.width as i32;
        let height = size.height as i32;
        let frame = self.buffer.back_mut();
        let x0 = rect.x.max(0);
        let y0 = rect.y.max(0);
        let x1 = (rect.x + rect.width as f32 as i32 - 1).min(width - 1);
        let y1 = (rect.y + rect.height as f32 as i32 - 1).min(height - 1);
        let effective_radius = rounded_rect_effective_radius(rect, radius);
        let inner = inset_rect(rect, stroke_width as i32);
        let has_inner = inner.width > 0 && inner.height > 0;
        let inner_radius = effective_radius.saturating_sub(stroke_width);
        for py in y0..=y1 {
            for px in x0..=x1 {
                let outer_coverage = rounded_rect_coverage(px, py, rect, effective_radius);
                if outer_coverage <= 0.0 {
                    continue;
                }
                let inner_coverage = if has_inner {
                    rounded_rect_coverage(px, py, inner, inner_radius)
                } else {
                    0.0
                };
                let stroke_coverage = (outer_coverage - inner_coverage).clamp(0.0, 1.0);
                if stroke_coverage > 0.0 {
                    blend_pixel(
                        frame,
                        size.width,
                        px as u32,
                        py as u32,
                        color,
                        stroke_coverage,
                    );
                }
            }
        }
    }
    /// Draw rounded-rectangle stroke with stronger anti-aliasing sampling.
    pub fn draw_rounded_rect_aa_with_width(
        &mut self,
        rect: Rect,
        radius: u32,
        color: Color,
        stroke_width: u32,
    ) {
        if stroke_width == 0 || rect.width == 0 || rect.height == 0 {
            return;
        }
        let sample_grid = self.aa_samples_per_axis;
        let size = self.buffer.size();
        let width = size.width as i32;
        let height = size.height as i32;
        let frame = self.buffer.back_mut();
        let x0 = rect.x.max(0);
        let y0 = rect.y.max(0);
        let x1 = (rect.x + rect.width as f32 as i32 - 1).min(width - 1);
        let y1 = (rect.y + rect.height as f32 as i32 - 1).min(height - 1);
        let effective_radius = rounded_rect_effective_radius(rect, radius);
        let inner = inset_rect(rect, stroke_width as i32);
        let has_inner = inner.width > 0 && inner.height > 0;
        let inner_radius = effective_radius.saturating_sub(stroke_width);
        for py in y0..=y1 {
            for px in x0..=x1 {
                let outer_coverage =
                    rounded_rect_coverage_grid(px, py, rect, effective_radius, sample_grid);
                if outer_coverage <= 0.0 {
                    continue;
                }
                let inner_coverage = if has_inner {
                    rounded_rect_coverage_grid(px, py, inner, inner_radius, sample_grid)
                } else {
                    0.0
                };
                let stroke_coverage = (outer_coverage - inner_coverage).clamp(0.0, 1.0);
                if stroke_coverage > 0.0 {
                    blend_pixel(
                        frame,
                        size.width,
                        px as u32,
                        py as u32,
                        color,
                        stroke_coverage,
                    );
                }
            }
        }
    }
    /// Draws a 1px line segment.
    pub fn draw_line(&mut self, from: Point, to: Point, color: Color) {
        self.draw_line_with_width(from, to, color, 1);
    }
    /// Draws a line segment with explicit stroke width.
    pub fn draw_line_with_width(
        &mut self,
        from: Point,
        to: Point,
        color: Color,
        stroke_width: u32,
    ) {
        if stroke_width == 0 {
            return;
        }
        let size = self.buffer.size();
        let width = size.width;
        let height = size.height;
        let frame = self.buffer.back_mut();
        let brush_start = -(stroke_width as i32 / 2);
        let brush_end = brush_start + stroke_width as i32 - 1;
        let mut x0 = from.x;
        let mut y0 = from.y;
        let x1 = to.x;
        let y1 = to.y;
        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            for oy in brush_start..=brush_end {
                for ox in brush_start..=brush_end {
                    let px = x0 + ox;
                    let py = y0 + oy;
                    if px >= 0 && py >= 0 && (px as u32) < width && (py as u32) < height {
                        set_pixel(frame, width, px as u32, py as u32, color);
                    }
                }
            }
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = err * 2;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
    }
    /// Draw anti-aliased line using configurable sample-grid coverage.
    pub fn draw_line_aa(&mut self, from: Point, to: Point, color: Color) {
        self.draw_line_aa_with_width(from, to, color, 1);
    }
    /// Draw anti-aliased line with configurable stroke width.
    pub fn draw_line_aa_with_width(
        &mut self,
        from: Point,
        to: Point,
        color: Color,
        stroke_width: u32,
    ) {
        if stroke_width == 0 {
            return;
        }
        let sample_grid = self.aa_samples_per_axis;
        let size = self.buffer.size();
        let width = size.width as i32;
        let height = size.height as i32;
        let frame = self.buffer.back_mut();
        let half = stroke_width as f32 / 2.0;
        let pad = half.ceil() as i32 + 1;
        let min_x = from.x.min(to.x).saturating_sub(pad).max(0);
        let max_x = (from.x.max(to.x) + pad).min(width - 1);
        let min_y = from.y.min(to.y).saturating_sub(pad).max(0);
        let max_y = (from.y.max(to.y) + pad).min(height - 1);
        let ax = from.x as f32;
        let ay = from.y as f32;
        let bx = to.x as f32;
        let by = to.y as f32;
        for py in min_y..=max_y {
            for px in min_x..=max_x {
                let coverage = line_stroke_coverage_grid(px, py, ax, ay, bx, by, half, sample_grid);
                if coverage > 0.0 {
                    blend_pixel(frame, size.width, px as u32, py as u32, color, coverage);
                }
            }
        }
    }
    /// Fills a circle with a solid color.
    pub fn fill_circle(&mut self, center: Point, radius: u32, color: Color) {
        if radius == 0 {
            return;
        }
        let size = self.buffer.size();
        let width = size.width as i32;
        let height = size.height as i32;
        let frame = self.buffer.back_mut();
        let r = radius as i32;
        for y in -r..=r {
            let y2 = y * y;
            if y2 > r * r {
                continue;
            }
            let span = ((r * r - y2) as f32).sqrt() as i32;
            let py = center.y + y;
            if py < 0 || py >= height {
                continue;
            }
            for x in -span..=span {
                let px = center.x + x;
                if px < 0 || px >= width {
                    continue;
                }
                set_pixel(frame, size.width, px as u32, py as u32, color);
            }
        }
    }
    /// Fills a circle using anti-aliased coverage.
    pub fn fill_circle_aa(&mut self, center: Point, radius: u32, color: Color) {
        if radius == 0 {
            return;
        }
        let sample_grid = self.aa_samples_per_axis;
        let size = self.buffer.size();
        let width = size.width as i32;
        let height = size.height as i32;
        let frame = self.buffer.back_mut();
        let r = radius as f32;
        let x0 = (center.x - radius as i32 - 1).max(0);
        let y0 = (center.y - radius as i32 - 1).max(0);
        let x1 = (center.x + radius as i32 + 1).min(width - 1);
        let y1 = (center.y + radius as i32 + 1).min(height - 1);
        for py in y0..=y1 {
            for px in x0..=x1 {
                let coverage = circle_fill_coverage_grid(px, py, center, r, sample_grid);
                if coverage > 0.0 {
                    blend_pixel(frame, size.width, px as u32, py as u32, color, coverage);
                }
            }
        }
    }
    /// Draws a 1px circle stroke.
    pub fn draw_circle(&mut self, center: Point, radius: u32, color: Color) {
        self.draw_circle_with_width(center, radius, color, 1);
    }
    /// Draws a circle stroke with explicit width.
    pub fn draw_circle_with_width(
        &mut self,
        center: Point,
        radius: u32,
        color: Color,
        stroke_width: u32,
    ) {
        if radius == 0 {
            return;
        }
        if stroke_width == 0 {
            return;
        }
        let sample_grid = self.aa_samples_per_axis;
        let size = self.buffer.size();
        let width = size.width as i32;
        let height = size.height as i32;
        let frame = self.buffer.back_mut();
        let ring_radius = radius as f32;
        // let ring_half_width = stroke_width as f32 / 2.0; // unused
        let x0 = (center.x - radius as i32 - 1).max(0);
        let y0 = (center.y - radius as i32 - 1).max(0);
        let x1 = (center.x + radius as i32 + 1).min(width - 1);
        let y1 = (center.y + radius as i32 + 1).min(height - 1);
        for py in y0..=y1 {
            for px in x0..=x1 {
                let stroke_coverage = circle_stroke_coverage_grid(
                    px,
                    py,
                    center,
                    ring_radius,
                    stroke_width as f32,
                    sample_grid,
                );
                if stroke_coverage > 0.0 {
                    blend_pixel(
                        frame,
                        size.width,
                        px as u32,
                        py as u32,
                        color,
                        stroke_coverage,
                    );
                }
            }
        }
    }
    /// Draws text using the current text raster fallback path.
    pub fn draw_text(&mut self, origin: Point, text: &str, font: &Font, color: Color) {
        let metrics = self.measure_text(text, font);
        if metrics.width == 0 || metrics.height == 0 {
            return;
        }
        let shaped = self.shape_text(text, font);
        let mut pen_x = origin.x as f32;
        let glyph_height = metrics.height.max(1) as i32;
        let size = self.buffer.size();
        let frame = self.buffer.back_mut();
        for cluster in shaped.clusters() {
            let glyph_width = cluster.advance.max(1.0).round() as i32;
            let display_char = cluster
                .text
                .chars()
                .find(|ch| !is_combining_mark(*ch) && !is_variation_selector(*ch));
            if let Some(ch) = display_char {
                draw_bitmap_glyph(
                    frame,
                    size.width,
                    size.height,
                    ch,
                    pen_x.round() as i32,
                    origin.y,
                    glyph_width,
                    glyph_height,
                    color,
                );
            }
            pen_x += cluster.advance;
        }
    }
}
