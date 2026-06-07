//! Chart implementations: Line, Bar, Pie, Scatter, Area.

use crate::chart::types::*;
// svg used for chart rendering
use crate::core::{Color, Point, Rect};

pub struct LineChart {
    title: String,
    x_axis_label: String,
    y_axis_label: String,
    series: Vec<ChartSeries>,
    x_tick_count: usize,
    y_tick_count: usize,
    show_grid: bool,
}
/// Common cartesian layout for line/bar style charts.
pub struct CartesianLayout {
    plot_x: f32,
    plot_y: f32,
    plot_w: f32,
    plot_h: f32,
    legend_x: f32,
    legend_y: f32,
}
pub fn compute_cartesian_layout(
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
pub fn draw_cartesian_axes(context: &mut dyn ChartContext, layout: &CartesianLayout) {
    let axis_color = Color { r: 90, g: 90, b: 90, a: 255 };
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
pub fn draw_y_ticks(
    context: &mut dyn ChartContext,
    layout: &CartesianLayout,
    min_y: f64,
    max_y: f64,
    tick_count: usize,
    draw_grid: bool,
) {
    let tick_count = tick_count.max(2);
    let axis_color = Color { r: 150, g: 150, b: 150, a: 255 };
    let label_color = Color { r: 80, g: 80, b: 80, a: 255 };
    let grid_color = Color { r: 210, g: 210, b: 210, a: 255 };
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
pub fn draw_x_ticks(
    context: &mut dyn ChartContext,
    layout: &CartesianLayout,
    min_x: f64,
    max_x: f64,
    tick_count: usize,
    draw_grid: bool,
) {
    let tick_count = tick_count.max(2);
    let axis_color = Color { r: 150, g: 150, b: 150, a: 255 };
    let label_color = Color { r: 80, g: 80, b: 80, a: 255 };
    let grid_color = Color { r: 210, g: 210, b: 210, a: 255 };
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
pub fn draw_legend(
    context: &mut dyn ChartContext,
    layout: &CartesianLayout,
    series: &[&ChartSeries],
) {
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
            Color { r: 40, g: 40, b: 40, a: 255 },
        );
        cursor_y += 18.0;
    }
    let hidden = series.len().saturating_sub(max_items);
    if hidden > 0 {
        context.draw_text(
            &format!("+{hidden} more"),
            Point::from_f32(layout.legend_x + 26.0, cursor_y + 4.0),
            10.0,
            Color { r: 90, g: 90, b: 90, a: 255 },
        );
    }
}
pub fn truncate_legend_label(label: &str, max_chars: usize) -> String {
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
impl LineChart {
    /// Create a new line chart
    pub fn new() -> Self {
        Self {
            title: String::new(),
            x_axis_label: String::new(),
            y_axis_label: String::new(),
            series: Vec::new(),
            x_tick_count: 5,
            y_tick_count: 5,
            show_grid: false,
        }
    }
    /// Configure x-axis tick density for line chart rendering.
    pub fn set_x_tick_count(&mut self, tick_count: usize) {
        self.x_tick_count = tick_count.clamp(2, 16);
    }
    /// Configure y-axis tick density for line chart rendering.
    pub fn set_y_tick_count(&mut self, tick_count: usize) {
        self.y_tick_count = tick_count.clamp(2, 16);
    }
    /// Enable or disable cartesian gridline rendering.
    pub fn set_grid_enabled(&mut self, enabled: bool) {
        self.show_grid = enabled;
    }
}
impl Default for LineChart {
    fn default() -> Self {
        Self::new()
    }
}
impl Chart for LineChart {
    fn add_series(&mut self, series: ChartSeries) {
        self.series.push(series);
    }
    fn remove_series(&mut self, name: &str) {
        self.series.retain(|s| s.name != name);
    }
    fn clear_series(&mut self) {
        self.series.clear();
    }
    fn set_title(&mut self, title: String) {
        self.title = title;
    }
    fn set_x_axis_label(&mut self, label: String) {
        self.x_axis_label = label;
    }
    fn set_y_axis_label(&mut self, label: String) {
        self.y_axis_label = label;
    }
    fn draw(&self, rect: Rect, context: &mut dyn ChartContext) {
        context.draw_rect(rect, Color { r: 230, g: 230, b: 230, a: 255 });
        context.draw_text(
            &self.title,
            Point::new(rect.x + 8, rect.y + 16),
            14.0,
            Color { r: 20, g: 20, b: 20, a: 255 },
        );
        let visible_series: Vec<&ChartSeries> =
            self.series.iter().filter(|series| series.visible).collect();
        let layout = compute_cartesian_layout(
            rect,
            !self.x_axis_label.is_empty(),
            !self.y_axis_label.is_empty(),
            visible_series.len(),
        );
        draw_cartesian_axes(context, &layout);
        let mut min_x = f64::MAX;
        let mut max_x = f64::MIN;
        let mut min_y = f64::MAX;
        let mut max_y = f64::MIN;
        for series in &visible_series {
            for point in &series.data {
                min_x = min_x.min(point.x);
                max_x = max_x.max(point.x);
                min_y = min_y.min(point.y);
                max_y = max_y.max(point.y);
            }
        }
        if min_x == f64::MAX || min_y == f64::MAX {
            return;
        }
        let span_x = (max_x - min_x).max(1.0);
        let span_y = (max_y - min_y).max(1.0);
        draw_x_ticks(context, &layout, min_x, max_x, self.x_tick_count, self.show_grid);
        draw_y_ticks(context, &layout, min_y, max_y, self.y_tick_count, self.show_grid);
        if !self.x_axis_label.is_empty() {
            context.draw_text(
                &self.x_axis_label,
                Point::from_f32(
                    layout.plot_x + layout.plot_w * 0.5 - 28.0,
                    layout.plot_y + layout.plot_h + 36.0,
                ),
                11.0,
                Color { r: 40, g: 40, b: 40, a: 255 },
            );
        }
        if !self.y_axis_label.is_empty() {
            context.draw_text(
                &self.y_axis_label,
                Point::from_f32(layout.plot_x - 56.0, layout.plot_y - 10.0),
                11.0,
                Color { r: 40, g: 40, b: 40, a: 255 },
            );
        }
        for series in &visible_series {
            if series.data.len() < 2 {
                continue;
            }
            for i in 1..series.data.len() {
                let p1 = &series.data[i - 1];
                let p2 = &series.data[i];
                let x1 = layout.plot_x + (((p1.x - min_x) / span_x) as f32) * layout.plot_w;
                let y1 = layout.plot_y + layout.plot_h
                    - (((p1.y - min_y) / span_y) as f32) * layout.plot_h;
                let x2 = layout.plot_x + (((p2.x - min_x) / span_x) as f32) * layout.plot_w;
                let y2 = layout.plot_y + layout.plot_h
                    - (((p2.y - min_y) / span_y) as f32) * layout.plot_h;
                context.draw_line(
                    Point::from_f32(x1, y1),
                    Point::from_f32(x2, y2),
                    2.0,
                    series.color,
                );
            }
        }
        draw_legend(context, &layout, &visible_series);
    }
}
/// Bar chart
pub struct BarChart {
    title: String,
    x_axis_label: String,
    y_axis_label: String,
    series: Vec<ChartSeries>,
    x_tick_count: usize,
    y_tick_count: usize,
    show_grid: bool,
}
impl BarChart {
    /// Create a new bar chart
    pub fn new() -> Self {
        Self {
            title: String::new(),
            x_axis_label: String::new(),
            y_axis_label: String::new(),
            series: Vec::new(),
            x_tick_count: 5,
            y_tick_count: 5,
            show_grid: false,
        }
    }
    /// Configure x-axis tick density for bar chart rendering.
    pub fn set_x_tick_count(&mut self, tick_count: usize) {
        self.x_tick_count = tick_count.clamp(2, 16);
    }
    /// Configure y-axis tick density for bar chart rendering.
    pub fn set_y_tick_count(&mut self, tick_count: usize) {
        self.y_tick_count = tick_count.clamp(2, 16);
    }
    /// Enable or disable cartesian gridline rendering.
    pub fn set_grid_enabled(&mut self, enabled: bool) {
        self.show_grid = enabled;
    }
}
impl Default for BarChart {
    fn default() -> Self {
        Self::new()
    }
}
impl Chart for BarChart {
    fn add_series(&mut self, series: ChartSeries) {
        self.series.push(series);
    }
    fn remove_series(&mut self, name: &str) {
        self.series.retain(|s| s.name != name);
    }
    fn clear_series(&mut self) {
        self.series.clear();
    }
    fn set_title(&mut self, title: String) {
        self.title = title;
    }
    fn set_x_axis_label(&mut self, label: String) {
        self.x_axis_label = label;
    }
    fn set_y_axis_label(&mut self, label: String) {
        self.y_axis_label = label;
    }
    fn draw(&self, rect: Rect, context: &mut dyn ChartContext) {
        context.draw_rect(rect, Color { r: 240, g: 240, b: 240, a: 255 });
        context.draw_text(
            &self.title,
            Point::new(rect.x + 8, rect.y + 16),
            14.0,
            Color { r: 20, g: 20, b: 20, a: 255 },
        );
        let visible_series: Vec<&ChartSeries> =
            self.series.iter().filter(|series| series.visible).collect();
        if visible_series.is_empty() {
            return;
        }
        let layout = compute_cartesian_layout(
            rect,
            !self.x_axis_label.is_empty(),
            !self.y_axis_label.is_empty(),
            visible_series.len(),
        );
        draw_cartesian_axes(context, &layout);
        let mut max_y = 1.0f64;
        let mut min_x = f64::MAX;
        let mut max_x = f64::MIN;
        let mut points_per_series = 0usize;
        for series in &visible_series {
            points_per_series = points_per_series.max(series.data.len());
            for point in &series.data {
                max_y = max_y.max(point.y.max(1.0));
                min_x = min_x.min(point.x);
                max_x = max_x.max(point.x);
            }
        }
        if min_x == f64::MAX {
            return;
        }
        let span_x = (max_x - min_x).max(1.0);
        draw_x_ticks(context, &layout, min_x, max_x, self.x_tick_count, self.show_grid);
        draw_y_ticks(context, &layout, 0.0, max_y, self.y_tick_count, self.show_grid);
        if !self.x_axis_label.is_empty() {
            context.draw_text(
                &self.x_axis_label,
                Point::from_f32(
                    layout.plot_x + layout.plot_w * 0.5 - 28.0,
                    layout.plot_y + layout.plot_h + 36.0,
                ),
                11.0,
                Color { r: 40, g: 40, b: 40, a: 255 },
            );
        }
        if !self.y_axis_label.is_empty() {
            context.draw_text(
                &self.y_axis_label,
                Point::from_f32(layout.plot_x - 56.0, layout.plot_y - 10.0),
                11.0,
                Color { r: 40, g: 40, b: 40, a: 255 },
            );
        }
        let point_slots = points_per_series.max(1) * visible_series.len();
        let bar_width = (layout.plot_w / (point_slots as f32 + 1.0)).clamp(4.0, 24.0);
        for (series_index, series) in visible_series.iter().enumerate() {
            for point in &series.data {
                let base_x = layout.plot_x
                    + (((point.x - min_x) / span_x) as f32) * (layout.plot_w - bar_width);
                let x = base_x + series_index as f32 * bar_width;
                let bar_h = ((point.y / max_y) * layout.plot_h as f64) as u32;
                let bar = Rect {
                    x: x as i32,
                    y: (layout.plot_y + layout.plot_h) as i32 - bar_h as i32,
                    width: bar_width.max(1.0) as u32,
                    height: bar_h,
                };
                context.draw_rect(bar, series.color);
            }
        }
        draw_legend(context, &layout, &visible_series);
    }
}
/// Pie chart
pub struct PieChart {
    title: String,
    series: Vec<ChartSeries>,
}
impl PieChart {
    /// Create a new pie chart
    pub fn new() -> Self {
        Self { title: String::new(), series: Vec::new() }
    }
    /// Return a color from a preset palette based on index.
    fn palette_color(index: usize) -> Color {
        const PALETTE: &[Color] = &[
            Color { r: 255, g: 99, b: 132, a: 255 },  // rose
            Color { r: 54, g: 162, b: 235, a: 255 },  // blue
            Color { r: 255, g: 206, b: 86, a: 255 },  // yellow
            Color { r: 75, g: 192, b: 192, a: 255 },  // teal
            Color { r: 153, g: 102, b: 255, a: 255 }, // purple
            Color { r: 255, g: 159, b: 64, a: 255 },  // orange
            Color { r: 46, g: 204, b: 113, a: 255 },  // green
            Color { r: 231, g: 76, b: 60, a: 255 },   // red
        ];
        PALETTE[index % PALETTE.len()]
    }
}
impl Default for PieChart {
    fn default() -> Self {
        Self::new()
    }
}
impl Chart for PieChart {
    fn add_series(&mut self, series: ChartSeries) {
        self.series.push(series);
    }
    fn remove_series(&mut self, name: &str) {
        self.series.retain(|s| s.name != name);
    }
    fn clear_series(&mut self) {
        self.series.clear();
    }
    fn set_title(&mut self, title: String) {
        self.title = title;
    }
    fn set_x_axis_label(&mut self, label: String) {
        log::debug!(
            "[chart] PieChart.set_x_axis_label ignored: pie charts have no x-axis (label='{}')",
            label
        );
    }
    fn set_y_axis_label(&mut self, label: String) {
        log::debug!(
            "[chart] PieChart.set_y_axis_label ignored: pie charts have no y-axis (label='{}')",
            label
        );
    }
    fn draw(&self, rect: Rect, context: &mut dyn ChartContext) {
        // Draw title at the top
        context.draw_text(
            &self.title,
            Point::new(rect.x + 8, rect.y + 16),
            14.0,
            Color { r: 20, g: 20, b: 20, a: 255 },
        );
        let center =
            Point { x: rect.x + rect.width as i32 / 2, y: rect.y + rect.height as i32 / 2 };
        let radius = (rect.width.min(rect.height) as f64 / 2.5) as u32;
        // Compute total sum across all visible series
        let total: f64 =
            self.series.iter().filter(|s| s.visible).flat_map(|s| &s.data).map(|d| d.y).sum();
        if total <= 0.0 {
            return;
        }
        let mut start_angle: f64 = -90.0; // Start from top (12 o'clock)
        let mut sector_index: usize = 0;
        for series in &self.series {
            if !series.visible {
                continue;
            }
            for point in &series.data {
                let sweep = (point.y / total) * 360.0;
                if sweep <= 0.0 {
                    continue;
                }
                // Pick a distinct color per sector from the palette
                let sector_color = Self::palette_color(sector_index);
                sector_index += 1;
                // Draw pie arc segment as a filled polygon approximating the arc
                let cx = center.x as f64;
                let cy = center.y as f64;
                let r = radius as f64;
                let steps = (sweep.abs().ceil() as u32).clamp(3, 90);
                let mut vertices = Vec::with_capacity((steps + 2) as usize);
                // Center point
                vertices.push(Point { x: cx as i32, y: cy as i32 });
                // Arc boundary points
                for i in 0..=steps {
                    let angle = start_angle + sweep * (i as f64 / steps as f64);
                    let rad = angle.to_radians();
                    vertices.push(Point {
                        x: (cx + rad.cos() * r) as i32,
                        y: (cy + rad.sin() * r) as i32,
                    });
                }
                context.draw_polygon(&vertices, sector_color);
                start_angle += sweep;
            }
        }
    }
}
/// Scatter plot chart
pub struct ScatterChart {
    title: String,
    x_axis_label: String,
    y_axis_label: String,
    series: Vec<ChartSeries>,
    x_tick_count: usize,
    y_tick_count: usize,
    show_grid: bool,
    point_radius: f32,
}
impl ScatterChart {
    /// Create a new scatter plot chart
    pub fn new() -> Self {
        Self {
            title: String::new(),
            x_axis_label: String::new(),
            y_axis_label: String::new(),
            series: Vec::new(),
            x_tick_count: 5,
            y_tick_count: 5,
            show_grid: false,
            point_radius: 4.0,
        }
    }
    /// Configure x-axis tick density
    pub fn set_x_tick_count(&mut self, tick_count: usize) {
        self.x_tick_count = tick_count.clamp(2, 16);
    }
    /// Configure y-axis tick density
    pub fn set_y_tick_count(&mut self, tick_count: usize) {
        self.y_tick_count = tick_count.clamp(2, 16);
    }
    /// Enable or disable grid rendering
    pub fn set_grid_enabled(&mut self, enabled: bool) {
        self.show_grid = enabled;
    }
    /// Set point radius
    pub fn set_point_radius(&mut self, radius: f32) {
        self.point_radius = radius.max(1.0);
    }
}
impl Default for ScatterChart {
    fn default() -> Self {
        Self::new()
    }
}
impl Chart for ScatterChart {
    fn add_series(&mut self, series: ChartSeries) {
        self.series.push(series);
    }
    fn remove_series(&mut self, name: &str) {
        self.series.retain(|s| s.name != name);
    }
    fn clear_series(&mut self) {
        self.series.clear();
    }
    fn set_title(&mut self, title: String) {
        self.title = title;
    }
    fn set_x_axis_label(&mut self, label: String) {
        self.x_axis_label = label;
    }
    fn set_y_axis_label(&mut self, label: String) {
        self.y_axis_label = label;
    }
    fn draw(&self, rect: Rect, context: &mut dyn ChartContext) {
        context.draw_rect(rect, Color { r: 240, g: 240, b: 240, a: 255 });
        context.draw_text(
            &self.title,
            Point::new(rect.x + 8, rect.y + 16),
            14.0,
            Color { r: 20, g: 20, b: 20, a: 255 },
        );
        let visible_series: Vec<&ChartSeries> =
            self.series.iter().filter(|series| series.visible).collect();
        let layout = compute_cartesian_layout(
            rect,
            !self.x_axis_label.is_empty(),
            !self.y_axis_label.is_empty(),
            visible_series.len(),
        );
        draw_cartesian_axes(context, &layout);
        let mut min_x = f64::MAX;
        let mut max_x = f64::MIN;
        let mut min_y = f64::MAX;
        let mut max_y = f64::MIN;
        for series in &visible_series {
            for point in &series.data {
                min_x = min_x.min(point.x);
                max_x = max_x.max(point.x);
                min_y = min_y.min(point.y);
                max_y = max_y.max(point.y);
            }
        }
        if min_x == f64::MAX || min_y == f64::MAX {
            return;
        }
        let span_x = (max_x - min_x).max(1.0);
        let span_y = (max_y - min_y).max(1.0);
        draw_x_ticks(context, &layout, min_x, max_x, self.x_tick_count, self.show_grid);
        draw_y_ticks(context, &layout, min_y, max_y, self.y_tick_count, self.show_grid);
        if !self.x_axis_label.is_empty() {
            context.draw_text(
                &self.x_axis_label,
                Point::from_f32(
                    layout.plot_x + layout.plot_w * 0.5 - 28.0,
                    layout.plot_y + layout.plot_h + 36.0,
                ),
                11.0,
                Color { r: 40, g: 40, b: 40, a: 255 },
            );
        }
        if !self.y_axis_label.is_empty() {
            context.draw_text(
                &self.y_axis_label,
                Point::from_f32(layout.plot_x - 56.0, layout.plot_y - 10.0),
                11.0,
                Color { r: 40, g: 40, b: 40, a: 255 },
            );
        }
        for series in &visible_series {
            for point in &series.data {
                let x = layout.plot_x + (((point.x - min_x) / span_x) as f32) * layout.plot_w;
                let y = layout.plot_y + layout.plot_h
                    - (((point.y - min_y) / span_y) as f32) * layout.plot_h;
                context.draw_circle(
                    Point { x: x as i32, y: y as i32 },
                    self.point_radius,
                    series.color,
                );
            }
        }
        draw_legend(context, &layout, &visible_series);
    }
}
/// Area chart
pub struct AreaChart {
    title: String,
    x_axis_label: String,
    y_axis_label: String,
    series: Vec<ChartSeries>,
    x_tick_count: usize,
    y_tick_count: usize,
    show_grid: bool,
    stacked: bool,
}
impl AreaChart {
    /// Create a new area chart
    pub fn new() -> Self {
        Self {
            title: String::new(),
            x_axis_label: String::new(),
            y_axis_label: String::new(),
            series: Vec::new(),
            x_tick_count: 5,
            y_tick_count: 5,
            show_grid: false,
            stacked: false,
        }
    }
    /// Configure x-axis tick density
    pub fn set_x_tick_count(&mut self, tick_count: usize) {
        self.x_tick_count = tick_count.clamp(2, 16);
    }
    /// Configure y-axis tick density
    pub fn set_y_tick_count(&mut self, tick_count: usize) {
        self.y_tick_count = tick_count.clamp(2, 16);
    }
    /// Enable or disable grid rendering
    pub fn set_grid_enabled(&mut self, enabled: bool) {
        self.show_grid = enabled;
    }
    /// Set stacked mode
    pub fn set_stacked(&mut self, stacked: bool) {
        self.stacked = stacked;
    }
}
impl Default for AreaChart {
    fn default() -> Self {
        Self::new()
    }
}
impl Chart for AreaChart {
    fn add_series(&mut self, series: ChartSeries) {
        self.series.push(series);
    }
    fn remove_series(&mut self, name: &str) {
        self.series.retain(|s| s.name != name);
    }
    fn clear_series(&mut self) {
        self.series.clear();
    }
    fn set_title(&mut self, title: String) {
        self.title = title;
    }
    fn set_x_axis_label(&mut self, label: String) {
        self.x_axis_label = label;
    }
    fn set_y_axis_label(&mut self, label: String) {
        self.y_axis_label = label;
    }
    fn draw(&self, rect: Rect, context: &mut dyn ChartContext) {
        context.draw_rect(rect, Color { r: 240, g: 240, b: 240, a: 255 });
        context.draw_text(
            &self.title,
            Point::new(rect.x + 8, rect.y + 16),
            14.0,
            Color { r: 20, g: 20, b: 20, a: 255 },
        );
        let visible_series: Vec<&ChartSeries> =
            self.series.iter().filter(|series| series.visible).collect();
        let layout = compute_cartesian_layout(
            rect,
            !self.x_axis_label.is_empty(),
            !self.y_axis_label.is_empty(),
            visible_series.len(),
        );
        draw_cartesian_axes(context, &layout);
        let mut min_x = f64::MAX;
        let mut max_x = f64::MIN;
        let mut min_y = f64::MAX;
        let mut max_y = f64::MIN;
        for series in &visible_series {
            for point in &series.data {
                min_x = min_x.min(point.x);
                max_x = max_x.max(point.x);
                min_y = min_y.min(point.y);
                max_y = max_y.max(point.y);
            }
        }
        if min_x == f64::MAX || min_y == f64::MAX {
            return;
        }
        let span_x = (max_x - min_x).max(1.0);
        let span_y = (max_y - min_y).max(1.0);
        draw_x_ticks(context, &layout, min_x, max_x, self.x_tick_count, self.show_grid);
        draw_y_ticks(context, &layout, min_y, max_y, self.y_tick_count, self.show_grid);
        if !self.x_axis_label.is_empty() {
            context.draw_text(
                &self.x_axis_label,
                Point::from_f32(
                    layout.plot_x + layout.plot_w * 0.5 - 28.0,
                    layout.plot_y + layout.plot_h + 36.0,
                ),
                11.0,
                Color { r: 40, g: 40, b: 40, a: 255 },
            );
        }
        if !self.y_axis_label.is_empty() {
            context.draw_text(
                &self.y_axis_label,
                Point::from_f32(layout.plot_x - 56.0, layout.plot_y - 10.0),
                11.0,
                Color { r: 40, g: 40, b: 40, a: 255 },
            );
        }
        let baseline = layout.plot_y + layout.plot_h;
        // Track accumulated y-values for stacking mode
        let mut accum: Option<Vec<f64>> = None;
        for series in &visible_series {
            if series.data.len() < 2 {
                continue;
            }
            // Determine effective y-values for this series (stacked or raw)
            let effective_y: Vec<f64> = if self.stacked {
                // Ensure accumulator is sized to match this series
                if accum.as_ref().map(|a| a.len() != series.data.len()).unwrap_or(true) {
                    accum = Some(vec![0.0_f64; series.data.len()]);
                }
                let acc = accum.as_mut().unwrap();
                series
                    .data
                    .iter()
                    .enumerate()
                    .map(|(i, p)| {
                        let stacked_y = acc[i] + p.y;
                        acc[i] = stacked_y;
                        stacked_y
                    })
                    .collect()
            } else {
                series.data.iter().map(|p| p.y).collect()
            };
            // Build polygon from baseline up to data points, across, and back down
            let mut pts = Vec::with_capacity(series.data.len() * 2 + 2);
            let p0 = &series.data[0];
            let x0 = layout.plot_x + (((p0.x - min_x) / span_x) as f32) * layout.plot_w;
            let y0 = baseline - (((effective_y[0] - min_y) / span_y) as f32) * layout.plot_h;
            // Start at baseline below first point, go up to first point
            pts.push(Point::from_f32(x0, baseline));
            pts.push(Point::from_f32(x0, y0));
            // Walk through intermediate data points
            for (p, ey) in series.data.iter().zip(effective_y.iter()).skip(1) {
                let x = layout.plot_x + (((p.x - min_x) / span_x) as f32) * layout.plot_w;
                let y = baseline - (((ey - min_y) / span_y) as f32) * layout.plot_h;
                pts.push(Point::from_f32(x, y));
            }
            // Drop back down to baseline from last point
            let last = &series.data[series.data.len() - 1];
            let last_x = layout.plot_x + (((last.x - min_x) / span_x) as f32) * layout.plot_w;
            pts.push(Point::from_f32(last_x, baseline));
            // Fill the enclosed area with semi-transparent color
            context.draw_polygon(&pts, series.color.with_alpha_f32(0.3));
            // Overdraw the top line for visual clarity
            for i in 1..series.data.len() {
                let p1 = &series.data[i - 1];
                let p2 = &series.data[i];
                let x1 = layout.plot_x + (((p1.x - min_x) / span_x) as f32) * layout.plot_w;
                let y1 =
                    baseline - (((effective_y[i - 1] - min_y) / span_y) as f32) * layout.plot_h;
                let x2 = layout.plot_x + (((p2.x - min_x) / span_x) as f32) * layout.plot_w;
                let y2 = baseline - (((effective_y[i] - min_y) / span_y) as f32) * layout.plot_h;
                context.draw_line(
                    Point::from_f32(x1, y1),
                    Point::from_f32(x2, y2),
                    2.0,
                    series.color,
                );
            }
        }
        draw_legend(context, &layout, &visible_series);
    }
}
