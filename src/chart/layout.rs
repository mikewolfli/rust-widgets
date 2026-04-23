//! Cartesian layout computation and axis/legend drawing.

use crate::chart::types::*;
use crate::chart::svg::*;
use crate::core::{Color, Point, Rect};

struct CartesianLayout {
    plot_x: f32,
    plot_y: f32,
    plot_w: f32,
    plot_h: f32,
    legend_x: f32,
    legend_y: f32,
}
fn compute_cartesian_layout(
    rect: Rect,
    has_x_label: bool,
    has_y_label: bool,
    legend_items: usize,
) -> CartesianLayout {
    let left_margin = if has_y_label { 64.0 } else { 48.0 };
    let top_margin = 32.0;
    let bottom_margin = if has_x_label { 52.0 } else { 36.0 };
    let right_margin = if legend_items > 0 { 170.0 } else { 24.0 };
    let plot_x = rect.x as f32 + left_margin;
    let plot_y = rect.y as f32 + top_margin;
    let plot_w = (rect.width as f32 - left_margin - right_margin).max(1.0);
    let plot_h = (rect.height as f32 - top_margin - bottom_margin).max(1.0);
    CartesianLayout {
        plot_x,
        plot_y,
        plot_w,
        plot_h,
        legend_x: plot_x + plot_w + 16.0,
        legend_y: plot_y + 8.0,
    }
}
fn draw_cartesian_axes(context: &mut dyn ChartContext, layout: &CartesianLayout) {
    let axis_color = Color {
        r: 90,
        g: 90,
        b: 90,
        a: 255,
    };
    context.draw_line(
        Point::from_f32(layout.plot_x, layout.plot_y + layout.plot_h),
        Point::from_f32(layout.plot_x + layout.plot_w, layout.plot_y + layout.plot_h),
        1.0,
        axis_color,
    );
    context.draw_line(
        Point::from_f32(layout.plot_x, layout.plot_y),
        Point::from_f32(layout.plot_x, layout.plot_y + layout.plot_h),
        1.0,
        axis_color,
    );
}
fn draw_y_ticks(
    context: &mut dyn ChartContext,
    layout: &CartesianLayout,
    min_y: f64,
    max_y: f64,
    tick_count: usize,
    draw_grid: bool,
) {
    let tick_count = tick_count.max(2);
    let axis_color = Color {
        r: 150,
        g: 150,
        b: 150,
        a: 255,
    };
    let label_color = Color {
        r: 80,
        g: 80,
        b: 80,
        a: 255,
    };
    let grid_color = Color {
        r: 210,
        g: 210,
        b: 210,
        a: 255,
    };
    for tick in 0..=tick_count {
        let t = tick as f32 / tick_count as f32;
        let y = layout.plot_y + layout.plot_h - t * layout.plot_h;
        if draw_grid {
            context.draw_line(
                Point::from_f32(layout.plot_x, y),
                Point::from_f32(layout.plot_x + layout.plot_w, y),
                1.0,
                grid_color,
            );
        }
        context.draw_line(
            Point::from_f32(layout.plot_x - 4.0, y),
            Point::from_f32(layout.plot_x, y),
            1.0,
            axis_color,
        );
        let value = min_y + (max_y - min_y) * t as f64;
        context.draw_text(
            &format!("{value:.1}"),
            Point::from_f32(layout.plot_x - 44.0, y + 4.0),
            10.0,
            label_color,
        );
    }
}
fn draw_x_ticks(
    context: &mut dyn ChartContext,
    layout: &CartesianLayout,
    min_x: f64,
    max_x: f64,
    tick_count: usize,
    draw_grid: bool,
) {
    let tick_count = tick_count.max(2);
    let axis_color = Color {
        r: 150,
        g: 150,
        b: 150,
        a: 255,
    };
    let label_color = Color {
        r: 80,
        g: 80,
        b: 80,
        a: 255,
    };
    let grid_color = Color {
        r: 210,
        g: 210,
        b: 210,
        a: 255,
    };
    for tick in 0..=tick_count {
        let t = tick as f32 / tick_count as f32;
        let x = layout.plot_x + t * layout.plot_w;
        if draw_grid {
            context.draw_line(
                Point::from_f32(x, layout.plot_y),
                Point::from_f32(x, layout.plot_y + layout.plot_h),
                1.0,
                grid_color,
            );
        }
        context.draw_line(
            Point::from_f32(x, layout.plot_y + layout.plot_h),
            Point::from_f32(x, layout.plot_y + layout.plot_h + 4.0),
            1.0,
            axis_color,
        );
        let value = min_x + (max_x - min_x) * t as f64;
        context.draw_text(
            &format!("{value:.1}"),
            Point::from_f32(x - 12.0, layout.plot_y + layout.plot_h + 16.0),
            10.0,
            label_color,
        );
    }
}
fn draw_legend(context: &mut dyn ChartContext, layout: &CartesianLayout, series: &[&ChartSeries]) {
    if series.is_empty() {
        return;
    }
    // Keep one legend row roughly every 18 px and reserve one row for overflow summary.
    let max_rows = ((layout.plot_h / 18.0).floor() as usize).max(1);
    let max_items = max_rows.saturating_sub(1).max(1);
    let max_label_chars = 20usize;
    let mut cursor_y = layout.legend_y;
    for item in series.iter().take(max_items) {
        context.draw_line(
            Point::from_f32(layout.legend_x, cursor_y),
            Point::from_f32(layout.legend_x + 20.0, cursor_y),
            3.0,
            item.color,
        );
        context.draw_text(
            &truncate_legend_label(&item.name, max_label_chars),
            Point::from_f32(layout.legend_x + 26.0, cursor_y + 4.0),
            11.0,
            Color {
                r: 40,
                g: 40,
                b: 40,
                a: 255,
            },
        );
        cursor_y += 18.0;
    }
    let hidden = series.len().saturating_sub(max_items);
    if hidden > 0 {
        context.draw_text(
            &format!("+{hidden} more"),
            Point::from_f32(layout.legend_x + 26.0, cursor_y + 4.0),
            10.0,
            Color {
                r: 90,
                g: 90,
                b: 90,
                a: 255,
            },
        );
    }
}
fn truncate_legend_label(label: &str, max_chars: usize) -> String {
    let char_count = label.chars().count();
    if char_count <= max_chars {
        return label.to_string();
    }
    if max_chars <= 3 {
        return "...".to_string();
    }
    let kept = max_chars - 3;
    let prefix = label.chars().take(kept).collect::<String>();
    format!("{prefix}...")
}


