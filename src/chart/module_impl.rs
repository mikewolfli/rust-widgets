//! Chart widgets and drawing contracts.
//!
//! # Coordinate System
//!
//! Charts use a **Cartesian coordinate system** for data (bottom-left origin, Y increases upward),
//! which is automatically converted to **screen coordinates** (top-left origin, Y increases downward)
//! when rendering.
//!
//! ## Data Coordinates (Cartesian)
//! - Origin: Bottom-left of the plot area
//! - X axis: Increases from left to right
//! - Y axis: Increases from bottom to top
//!
//! ## Screen Coordinates (Output)
//! - Origin: Top-left of the widget
//! - X axis: Increases from left to right
//! - Y axis: Increases from top to bottom
//!
//! The conversion is handled automatically by the chart rendering code, so you can work with
//! natural data coordinates when creating charts.

use crate::core::{Point, Rect, Color};
use std::fs;

/// Chart data point
#[derive(Debug, Clone)]
pub struct DataPoint {
    /// Data-domain x coordinate.
    pub x: f64,
    /// Data-domain y coordinate.
    pub y: f64,
    /// Optional point label for legends/tooltips.
    pub label: Option<String>,
}

/// Chart series
#[derive(Debug, Clone)]
pub struct ChartSeries {
    /// Series display name.
    pub name: String,
    /// Ordered data points.
    pub data: Vec<DataPoint>,
    /// Series draw color.
    pub color: Color,
    /// Visibility flag for filtering/toggling.
    pub visible: bool,
}

/// Chart type
pub enum ChartType {
    /// Polyline chart.
    Line,
    /// Vertical bar chart.
    Bar,
    /// Pie chart.
    Pie,
}

#[derive(Default)]
/// In-memory chart draw-command collector used by tests/demos.
pub struct MemoryChartContext {
    /// Recorded draw commands for tests/demos.
    pub commands: Vec<String>,
}

/// SVG chart context that emits real vector drawing output.
pub struct SvgChartContext {
    width: u32,
    height: u32,
    elements: Vec<String>,
}

impl SvgChartContext {
    /// Create SVG context with explicit viewport size.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width: width.max(1),
            height: height.max(1),
            elements: Vec::new(),
        }
    }

    /// Return SVG XML text.
    pub fn to_svg_string(&self) -> String {
        let mut svg = String::new();
        svg.push_str(&format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\">\n",
            self.width, self.height, self.width, self.height
        ));
        for element in &self.elements {
            svg.push_str(element);
            svg.push('\n');
        }
        svg.push_str("</svg>\n");
        svg
    }

    /// Save SVG XML text to a file path.
    pub fn save(&self, path: &str) -> Result<(), std::io::Error> {
        fs::write(path, self.to_svg_string())
    }
}

impl ChartContext for SvgChartContext {
    fn draw_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, width: f32, color: Color) {
        self.elements.push(format!(
            "<line x1=\"{:.2}\" y1=\"{:.2}\" x2=\"{:.2}\" y2=\"{:.2}\" stroke=\"{}\" stroke-opacity=\"{:.3}\" stroke-width=\"{:.2}\" />",
            x1,
            y1,
            x2,
            y2,
            svg_color_hex(color),
            svg_alpha(color),
            width.max(0.1)
        ));
    }

    fn draw_rect(&mut self, rect: Rect, color: Color) {
        self.elements.push(format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"none\" stroke=\"{}\" stroke-opacity=\"{:.3}\" stroke-width=\"1\" />",
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            svg_color_hex(color),
            svg_alpha(color)
        ));
    }

    fn draw_text(&mut self, text: &str, x: f32, y: f32, font_size: f32, color: Color) {
        self.elements.push(format!(
            "<text x=\"{:.2}\" y=\"{:.2}\" fill=\"{}\" fill-opacity=\"{:.3}\" font-size=\"{:.2}\" font-family=\"sans-serif\">{}</text>",
            x,
            y,
            svg_color_hex(color),
            svg_alpha(color),
            font_size.max(1.0),
            svg_escape_text(text)
        ));
    }

    fn draw_circle(&mut self, center: Point, radius: f32, color: Color) {
        self.elements.push(format!(
            "<circle cx=\"{}\" cy=\"{}\" r=\"{:.2}\" fill=\"none\" stroke=\"{}\" stroke-opacity=\"{:.3}\" stroke-width=\"2\" />",
            center.x,
            center.y,
            radius.max(0.1),
            svg_color_hex(color),
            svg_alpha(color)
        ));
    }
}

/// Render chart into SVG content and write to file.
pub fn render_chart_to_svg_file(
    chart: &dyn Chart,
    rect: Rect,
    path: &str,
) -> Result<(), std::io::Error> {
    let width = (rect.x.max(0) as u32).saturating_add(rect.width);
    let height = (rect.y.max(0) as u32).saturating_add(rect.height);
    let mut context = SvgChartContext::new(width.max(1), height.max(1));
    chart.draw(rect, &mut context);
    context.save(path)
}

impl ChartContext for MemoryChartContext {
    fn draw_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, width: f32, _color: Color) {
        self.commands.push(format!("line:{x1},{y1}->{x2},{y2}:{width}"));
    }

    fn draw_rect(&mut self, rect: Rect, _color: Color) {
        self.commands.push(format!("rect:{},{},{},{}", rect.x, rect.y, rect.width, rect.height));
    }

    fn draw_text(&mut self, text: &str, x: f32, y: f32, font_size: f32, _color: Color) {
        self.commands.push(format!("text:{text}@{x},{y}:{font_size}"));
    }

    fn draw_circle(&mut self, center: Point, radius: f32, _color: Color) {
        self.commands.push(format!("circle:{},{}:{radius}", center.x, center.y));
    }
}

/// Chart
pub trait Chart {
    /// Add a series
    fn add_series(&mut self, series: ChartSeries);
    
    /// Remove a series
    fn remove_series(&mut self, name: &str);
    
    /// Clear all series
    fn clear_series(&mut self);
    
    /// Set chart title
    fn set_title(&mut self, title: String);
    
    /// Set x-axis label
    fn set_x_axis_label(&mut self, label: String);
    
    /// Set y-axis label
    fn set_y_axis_label(&mut self, label: String);
    
    /// Draw the chart
    fn draw(&self, rect: Rect, context: &mut dyn ChartContext);
}

/// Chart context
pub trait ChartContext {
    /// Draw line
    fn draw_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, width: f32, color: Color);
    
    /// Draw rectangle
    fn draw_rect(&mut self, rect: Rect, color: Color);
    
    /// Draw text
    fn draw_text(&mut self, text: &str, x: f32, y: f32, font_size: f32, color: Color);
    
    /// Draw circle
    fn draw_circle(&mut self, center: Point, radius: f32, color: Color);
}

/// Line chart
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
struct CartesianLayout {
    plot_x: f32,
    plot_y: f32,
    plot_w: f32,
    plot_h: f32,
    legend_x: f32,
    legend_y: f32,
}

fn compute_cartesian_layout(rect: Rect, has_x_label: bool, has_y_label: bool, legend_items: usize) -> CartesianLayout {
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
    let axis_color = Color { r: 90, g: 90, b: 90, a: 255 };
    context.draw_line(
        layout.plot_x,
        layout.plot_y + layout.plot_h,
        layout.plot_x + layout.plot_w,
        layout.plot_y + layout.plot_h,
        1.0,
        axis_color,
    );
    context.draw_line(
        layout.plot_x,
        layout.plot_y,
        layout.plot_x,
        layout.plot_y + layout.plot_h,
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
    let axis_color = Color { r: 150, g: 150, b: 150, a: 255 };
    let label_color = Color { r: 80, g: 80, b: 80, a: 255 };
    let grid_color = Color { r: 210, g: 210, b: 210, a: 255 };
    for tick in 0..=tick_count {
        let t = tick as f32 / tick_count as f32;
        let y = layout.plot_y + layout.plot_h - t * layout.plot_h;
        if draw_grid {
            context.draw_line(layout.plot_x, y, layout.plot_x + layout.plot_w, y, 1.0, grid_color);
        }
        context.draw_line(layout.plot_x - 4.0, y, layout.plot_x, y, 1.0, axis_color);
        let value = min_y + (max_y - min_y) * t as f64;
        context.draw_text(
            &format!("{value:.1}"),
            layout.plot_x - 44.0,
            y + 4.0,
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
    let axis_color = Color { r: 150, g: 150, b: 150, a: 255 };
    let label_color = Color { r: 80, g: 80, b: 80, a: 255 };
    let grid_color = Color { r: 210, g: 210, b: 210, a: 255 };
    for tick in 0..=tick_count {
        let t = tick as f32 / tick_count as f32;
        let x = layout.plot_x + t * layout.plot_w;
        if draw_grid {
            context.draw_line(x, layout.plot_y, x, layout.plot_y + layout.plot_h, 1.0, grid_color);
        }
        context.draw_line(
            x,
            layout.plot_y + layout.plot_h,
            x,
            layout.plot_y + layout.plot_h + 4.0,
            1.0,
            axis_color,
        );
        let value = min_x + (max_x - min_x) * t as f64;
        context.draw_text(
            &format!("{value:.1}"),
            x - 12.0,
            layout.plot_y + layout.plot_h + 16.0,
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
        context.draw_line(layout.legend_x, cursor_y, layout.legend_x + 20.0, cursor_y, 3.0, item.color);
        context.draw_text(
            &truncate_legend_label(&item.name, max_label_chars),
            layout.legend_x + 26.0,
            cursor_y + 4.0,
            11.0,
            Color { r: 40, g: 40, b: 40, a: 255 },
        );
        cursor_y += 18.0;
    }

    let hidden = series.len().saturating_sub(max_items);
    if hidden > 0 {
        context.draw_text(
            &format!("+{hidden} more"),
            layout.legend_x + 26.0,
            cursor_y + 4.0,
            10.0,
            Color { r: 90, g: 90, b: 90, a: 255 },
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
        context.draw_text(&self.title, rect.x as f32 + 8.0, rect.y as f32 + 16.0, 14.0, Color { r: 20, g: 20, b: 20, a: 255 });

        let visible_series: Vec<&ChartSeries> = self
            .series
            .iter()
            .filter(|series| series.visible)
            .collect();
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
                layout.plot_x + layout.plot_w * 0.5 - 28.0,
                layout.plot_y + layout.plot_h + 36.0,
                11.0,
                Color { r: 40, g: 40, b: 40, a: 255 },
            );
        }

        if !self.y_axis_label.is_empty() {
            context.draw_text(
                &self.y_axis_label,
                layout.plot_x - 56.0,
                layout.plot_y - 10.0,
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
                let y1 = layout.plot_y + layout.plot_h - (((p1.y - min_y) / span_y) as f32) * layout.plot_h;
                let x2 = layout.plot_x + (((p2.x - min_x) / span_x) as f32) * layout.plot_w;
                let y2 = layout.plot_y + layout.plot_h - (((p2.y - min_y) / span_y) as f32) * layout.plot_h;
                context.draw_line(
                    x1,
                    y1,
                    x2,
                    y2,
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
        context.draw_text(&self.title, rect.x as f32 + 8.0, rect.y as f32 + 16.0, 14.0, Color { r: 20, g: 20, b: 20, a: 255 });

        let visible_series: Vec<&ChartSeries> = self
            .series
            .iter()
            .filter(|series| series.visible)
            .collect();
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
                layout.plot_x + layout.plot_w * 0.5 - 28.0,
                layout.plot_y + layout.plot_h + 36.0,
                11.0,
                Color { r: 40, g: 40, b: 40, a: 255 },
            );
        }

        if !self.y_axis_label.is_empty() {
            context.draw_text(
                &self.y_axis_label,
                layout.plot_x - 56.0,
                layout.plot_y - 10.0,
                11.0,
                Color { r: 40, g: 40, b: 40, a: 255 },
            );
        }

        let point_slots = points_per_series.max(1) * visible_series.len();
        let bar_width = (layout.plot_w / (point_slots as f32 + 1.0)).clamp(4.0, 24.0);

        for (series_index, series) in visible_series.iter().enumerate() {
            for point in &series.data {
                let base_x = layout.plot_x + (((point.x - min_x) / span_x) as f32) * (layout.plot_w - bar_width);
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
        Self {
            title: String::new(),
            series: Vec::new(),
        }
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
    
    fn set_x_axis_label(&mut self, _label: String) {
        // Not used in pie chart
    }
    
    fn set_y_axis_label(&mut self, _label: String) {
        // Not used in pie chart
    }
    
    fn draw(&self, rect: Rect, context: &mut dyn ChartContext) {
        context.draw_text(&self.title, rect.x as f32 + 8.0, rect.y as f32 + 16.0, 14.0, Color { r: 20, g: 20, b: 20, a: 255 });
        let center = Point {
            x: rect.x + rect.width as i32 / 2,
            y: rect.y + rect.height as i32 / 2,
        };
        let radius = (rect.width.min(rect.height) / 3) as f32;
        for series in &self.series {
            if !series.visible {
                continue;
            }
            context.draw_circle(center, radius, series.color);
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
        context.draw_text(&self.title, rect.x as f32 + 8.0, rect.y as f32 + 16.0, 14.0, Color { r: 20, g: 20, b: 20, a: 255 });

        let visible_series: Vec<&ChartSeries> = self
            .series
            .iter()
            .filter(|series| series.visible)
            .collect();
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
                layout.plot_x + layout.plot_w * 0.5 - 28.0,
                layout.plot_y + layout.plot_h + 36.0,
                11.0,
                Color { r: 40, g: 40, b: 40, a: 255 },
            );
        }

        if !self.y_axis_label.is_empty() {
            context.draw_text(
                &self.y_axis_label,
                layout.plot_x - 56.0,
                layout.plot_y - 10.0,
                11.0,
                Color { r: 40, g: 40, b: 40, a: 255 },
            );
        }

        for series in &visible_series {
            for point in &series.data {
                let x = layout.plot_x + (((point.x - min_x) / span_x) as f32) * layout.plot_w;
                let y = layout.plot_y + layout.plot_h - (((point.y - min_y) / span_y) as f32) * layout.plot_h;
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
        context.draw_text(&self.title, rect.x as f32 + 8.0, rect.y as f32 + 16.0, 14.0, Color { r: 20, g: 20, b: 20, a: 255 });

        let visible_series: Vec<&ChartSeries> = self
            .series
            .iter()
            .filter(|series| series.visible)
            .collect();
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
                layout.plot_x + layout.plot_w * 0.5 - 28.0,
                layout.plot_y + layout.plot_h + 36.0,
                11.0,
                Color { r: 40, g: 40, b: 40, a: 255 },
            );
        }

        if !self.y_axis_label.is_empty() {
            context.draw_text(
                &self.y_axis_label,
                layout.plot_x - 56.0,
                layout.plot_y - 10.0,
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
                let y1 = layout.plot_y + layout.plot_h - (((p1.y - min_y) / span_y) as f32) * layout.plot_h;
                let x2 = layout.plot_x + (((p2.x - min_x) / span_x) as f32) * layout.plot_w;
                let y2 = layout.plot_y + layout.plot_h - (((p2.y - min_y) / span_y) as f32) * layout.plot_h;
                context.draw_line(
                    x1,
                    y1,
                    x2,
                    y2,
                    2.0,
                    series.color,
                );
            }
        }

        draw_legend(context, &layout, &visible_series);
    }
}

fn svg_color_hex(color: Color) -> String {
    format!("#{:02X}{:02X}{:02X}", color.r, color.g, color.b)
}

fn svg_alpha(color: Color) -> f32 {
    color.a as f32 / 255.0
}

fn svg_escape_text(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stable_hash64(input: &str) -> u64 {
        // FNV-1a 64-bit stable hash for deterministic snapshot checks.
        let mut hash: u64 = 0xcbf29ce484222325;
        for byte in input.as_bytes() {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash
    }

    fn count_numeric_text_labels(commands: &[String]) -> usize {
        commands
            .iter()
            .filter_map(|cmd| cmd.strip_prefix("text:"))
            .filter_map(|payload| payload.split_once('@').map(|(text, _)| text))
            .filter(|text| text.parse::<f64>().is_ok())
            .count()
    }

    fn sample_series(name: &str, color: Color, points: &[(f64, f64)]) -> ChartSeries {
        ChartSeries {
            name: name.to_string(),
            data: points
                .iter()
                .map(|(x, y)| DataPoint {
                    x: *x,
                    y: *y,
                    label: None,
                })
                .collect(),
            color,
            visible: true,
        }
    }

    #[test]
    fn line_chart_draws_axis_labels_and_legend() {
        let mut chart = LineChart::new();
        chart.set_title("Latency".to_string());
        chart.set_x_axis_label("Time".to_string());
        chart.set_y_axis_label("ms".to_string());
        chart.add_series(sample_series(
            "p50",
            Color { r: 0, g: 120, b: 255, a: 255 },
            &[(0.0, 10.0), (1.0, 20.0), (2.0, 16.0)],
        ));
        chart.add_series(sample_series(
            "p95",
            Color { r: 255, g: 128, b: 0, a: 255 },
            &[(0.0, 15.0), (1.0, 30.0), (2.0, 24.0)],
        ));

        let mut context = MemoryChartContext::default();
        chart.draw(
            Rect {
                x: 0,
                y: 0,
                width: 640,
                height: 360,
            },
            &mut context,
        );

        assert!(context.commands.iter().any(|cmd| cmd.contains("text:Time@")));
        assert!(context.commands.iter().any(|cmd| cmd.contains("text:ms@")));
        assert!(context.commands.iter().any(|cmd| cmd.contains("text:p50@")));
        assert!(context.commands.iter().any(|cmd| cmd.contains("text:p95@")));
    }

    #[test]
    fn bar_chart_draws_legend_and_axis_ticks() {
        let mut chart = BarChart::new();
        chart.set_title("Throughput".to_string());
        chart.set_x_axis_label("Bucket".to_string());
        chart.set_y_axis_label("req/s".to_string());
        chart.add_series(sample_series(
            "region-a",
            Color { r: 40, g: 180, b: 99, a: 255 },
            &[(0.0, 20.0), (1.0, 40.0), (2.0, 30.0)],
        ));

        let mut context = MemoryChartContext::default();
        chart.draw(
            Rect {
                x: 0,
                y: 0,
                width: 500,
                height: 280,
            },
            &mut context,
        );

        assert!(context.commands.iter().any(|cmd| cmd.contains("text:Bucket@")));
        assert!(context.commands.iter().any(|cmd| cmd.contains("text:req/s@")));
        assert!(context.commands.iter().any(|cmd| cmd.contains("text:region-a@")));
        assert!(context.commands.iter().any(|cmd| cmd.starts_with("line:")));
    }

    #[test]
    fn legend_truncates_long_labels() {
        let mut chart = LineChart::new();
        chart.set_title("Legend".to_string());
        chart.add_series(sample_series(
            "this-is-a-very-long-legend-label-that-should-truncate",
            Color { r: 0, g: 100, b: 220, a: 255 },
            &[(0.0, 1.0), (1.0, 2.0)],
        ));

        let mut context = MemoryChartContext::default();
        chart.draw(
            Rect {
                x: 0,
                y: 0,
                width: 640,
                height: 220,
            },
            &mut context,
        );

        let truncated_entry = context
            .commands
            .iter()
            .find(|cmd| cmd.starts_with("text:this-is-a-very-") && cmd.contains("...@"));
        assert!(truncated_entry.is_some());
        assert!(!context
            .commands
            .iter()
            .any(|cmd| cmd.contains("text:this-is-a-very-long-legend-label-that-should-truncate@")));
    }

    #[test]
    fn legend_shows_overflow_summary() {
        let mut chart = LineChart::new();
        chart.set_title("Overflow".to_string());
        for index in 0..10 {
            chart.add_series(sample_series(
                &format!("s{index}"),
                Color { r: 20, g: 120, b: 200, a: 255 },
                &[(0.0, index as f64 + 1.0), (1.0, index as f64 + 2.0)],
            ));
        }

        let mut context = MemoryChartContext::default();
        chart.draw(
            Rect {
                x: 0,
                y: 0,
                width: 640,
                height: 110,
            },
            &mut context,
        );

        assert!(context.commands.iter().any(|cmd| cmd.contains("text:+")));
        assert!(context.commands.iter().any(|cmd| cmd.contains("more@")));
    }

    #[test]
    fn line_chart_respects_tick_density_configuration() {
        let mut chart = LineChart::new();
        chart.set_title("Ticks".to_string());
        chart.set_x_tick_count(3);
        chart.set_y_tick_count(4);
        chart.add_series(sample_series(
            "s1",
            Color { r: 0, g: 120, b: 255, a: 255 },
            &[(0.0, 0.0), (1.0, 10.0), (2.0, 20.0)],
        ));

        let mut context = MemoryChartContext::default();
        chart.draw(
            Rect {
                x: 0,
                y: 0,
                width: 640,
                height: 320,
            },
            &mut context,
        );

        // x ticks: 4 labels (0..=3), y ticks: 5 labels (0..=4)
        assert_eq!(count_numeric_text_labels(&context.commands), 9);
    }

    #[test]
    fn bar_chart_respects_tick_density_configuration() {
        let mut chart = BarChart::new();
        chart.set_title("Bars".to_string());
        chart.set_x_tick_count(6);
        chart.set_y_tick_count(2);
        chart.add_series(sample_series(
            "s1",
            Color { r: 20, g: 160, b: 100, a: 255 },
            &[(0.0, 5.0), (1.0, 10.0), (2.0, 8.0)],
        ));

        let mut context = MemoryChartContext::default();
        chart.draw(
            Rect {
                x: 0,
                y: 0,
                width: 520,
                height: 280,
            },
            &mut context,
        );

        // x ticks: 7 labels (0..=6), y ticks: 3 labels (0..=2)
        assert_eq!(count_numeric_text_labels(&context.commands), 10);
    }

    #[test]
    fn line_chart_gridline_toggle_changes_line_count() {
        let mut chart = LineChart::new();
        chart.set_title("Grid".to_string());
        chart.set_x_tick_count(3);
        chart.set_y_tick_count(3);
        chart.add_series(sample_series(
            "s1",
            Color { r: 0, g: 120, b: 255, a: 255 },
            &[(0.0, 1.0), (1.0, 2.0), (2.0, 3.0)],
        ));

        let mut without_grid = MemoryChartContext::default();
        chart.draw(
            Rect {
                x: 0,
                y: 0,
                width: 640,
                height: 320,
            },
            &mut without_grid,
        );
        let without_count = without_grid
            .commands
            .iter()
            .filter(|cmd| cmd.starts_with("line:"))
            .count();

        chart.set_grid_enabled(true);
        let mut with_grid = MemoryChartContext::default();
        chart.draw(
            Rect {
                x: 0,
                y: 0,
                width: 640,
                height: 320,
            },
            &mut with_grid,
        );
        let with_count = with_grid
            .commands
            .iter()
            .filter(|cmd| cmd.starts_with("line:"))
            .count();

        assert!(with_count > without_count);
    }

    #[test]
    fn bar_chart_gridline_toggle_changes_line_count() {
        let mut chart = BarChart::new();
        chart.set_title("GridBar".to_string());
        chart.set_x_tick_count(4);
        chart.set_y_tick_count(2);
        chart.add_series(sample_series(
            "s1",
            Color { r: 20, g: 160, b: 100, a: 255 },
            &[(0.0, 5.0), (1.0, 10.0), (2.0, 8.0)],
        ));

        let mut without_grid = MemoryChartContext::default();
        chart.draw(
            Rect {
                x: 0,
                y: 0,
                width: 520,
                height: 280,
            },
            &mut without_grid,
        );
        let without_count = without_grid
            .commands
            .iter()
            .filter(|cmd| cmd.starts_with("line:"))
            .count();

        chart.set_grid_enabled(true);
        let mut with_grid = MemoryChartContext::default();
        chart.draw(
            Rect {
                x: 0,
                y: 0,
                width: 520,
                height: 280,
            },
            &mut with_grid,
        );
        let with_count = with_grid
            .commands
            .iter()
            .filter(|cmd| cmd.starts_with("line:"))
            .count();

        assert!(with_count > without_count);
    }

    #[test]
    fn svg_snapshot_line_chart_stable() {
        let mut chart = LineChart::new();
        chart.set_title("SnapshotLine".to_string());
        chart.set_x_axis_label("X".to_string());
        chart.set_y_axis_label("Y".to_string());
        chart.set_grid_enabled(true);
        chart.set_x_tick_count(4);
        chart.set_y_tick_count(3);
        chart.add_series(sample_series(
            "line-a",
            Color { r: 15, g: 120, b: 240, a: 255 },
            &[(0.0, 1.0), (1.0, 4.0), (2.0, 2.0), (3.0, 5.0)],
        ));

        let mut context = SvgChartContext::new(640, 360);
        chart.draw(
            Rect {
                x: 0,
                y: 0,
                width: 640,
                height: 360,
            },
            &mut context,
        );
        let svg = context.to_svg_string();
        let got = stable_hash64(&svg);
        let expected = 11261639005027384391u64;
        assert_eq!(got, expected, "line snapshot hash changed: {got}");
    }

    #[test]
    fn svg_snapshot_bar_chart_stable() {
        let mut chart = BarChart::new();
        chart.set_title("SnapshotBar".to_string());
        chart.set_x_axis_label("Bucket".to_string());
        chart.set_y_axis_label("Value".to_string());
        chart.set_grid_enabled(true);
        chart.set_x_tick_count(5);
        chart.set_y_tick_count(4);
        chart.add_series(sample_series(
            "bar-a",
            Color { r: 20, g: 170, b: 100, a: 255 },
            &[(0.0, 2.0), (1.0, 5.0), (2.0, 3.0)],
        ));

        let mut context = SvgChartContext::new(640, 360);
        chart.draw(
            Rect {
                x: 0,
                y: 0,
                width: 640,
                height: 360,
            },
            &mut context,
        );
        let svg = context.to_svg_string();
        let got = stable_hash64(&svg);
        let expected = 13616823873602107208u64;
        assert_eq!(got, expected, "bar snapshot hash changed: {got}");
    }
}

