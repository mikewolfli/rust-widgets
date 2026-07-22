//! LineChart widget — a 2D line chart for visualizing data series.
//!
//! The LineChart widget draws axes, optional grid lines, an optional filled area
//! under the line, and a line connecting data points with configurable stroke.
//! Axis ranges can be set manually or auto-computed from the data.

use crate::core::{HorizontalAlignment, Color, Font, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// A 2D line chart widget for visualizing (x, y) data series.
pub struct LineChart {
    base: BaseWidget,
    data: Vec<(f64, f64)>,
    line_color: Color,
    fill_area: bool,
    fill_color: Color,
    stroke_width: f32,
    x_min: Option<f64>,
    x_max: Option<f64>,
    y_min: Option<f64>,
    y_max: Option<f64>,
    show_grid: bool,
    grid_color: Color,
    show_labels: bool,
}

impl LineChart {
    /// Creates a new LineChart widget with the given geometry.
    ///
    /// Defaults: blue line, no fill, stroke width 2, grid enabled with light gray,
    /// labels enabled.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::LineChart, geometry, "LineChart"),
            data: Vec::new(),
            line_color: Color::PRIMARY,
            fill_area: false,
            fill_color: Color::rgba(66, 133, 244, 60),
            stroke_width: 2.0,
            x_min: None,
            x_max: None,
            y_min: None,
            y_max: None,
            show_grid: true,
            grid_color: Color::rgba(200, 200, 200, 120),
            show_labels: true,
        }
    }

    /// Sets the data points to display. Clears any previous data.
    pub fn set_data(&mut self, points: Vec<(f64, f64)>) {
        self.data = points;
        self.base.request_redraw();
    }

    /// Sets the line color.
    pub fn set_line_color(&mut self, color: Color) {
        self.line_color = color;
        self.base.request_redraw();
    }

    /// Returns the current line color.
    pub fn line_color(&self) -> Color {
        self.line_color
    }

    /// Enables or disables the filled area under the line.
    pub fn set_fill_area(&mut self, fill: bool) {
        self.fill_area = fill;
        self.base.request_redraw();
    }

    /// Returns whether area fill is enabled.
    pub fn is_fill_area(&self) -> bool {
        self.fill_area
    }

    /// Sets the fill color used for the area under the line.
    pub fn set_fill_color(&mut self, color: Color) {
        self.fill_color = color;
        self.base.request_redraw();
    }

    /// Returns the current fill color.
    pub fn fill_color(&self) -> Color {
        self.fill_color
    }

    /// Sets the stroke width of the line.
    pub fn set_stroke_width(&mut self, width: f32) {
        self.stroke_width = width.max(0.5);
        self.base.request_redraw();
    }

    /// Returns the current stroke width.
    pub fn stroke_width(&self) -> f32 {
        self.stroke_width
    }

    /// Sets manual axis ranges. Pass `None` for any axis to auto-compute.
    pub fn set_axis_range(
        &mut self,
        x_min: Option<f64>,
        x_max: Option<f64>,
        y_min: Option<f64>,
        y_max: Option<f64>,
    ) {
        self.x_min = x_min;
        self.x_max = x_max;
        self.y_min = y_min;
        self.y_max = y_max;
        self.base.request_redraw();
    }

    /// Returns the current X-axis minimum (manual or auto-computed).
    pub fn x_min(&self) -> Option<f64> {
        self.x_min
    }

    /// Returns the current X-axis maximum (manual or auto-computed).
    pub fn x_max(&self) -> Option<f64> {
        self.x_max
    }

    /// Returns the current Y-axis minimum (manual or auto-computed).
    pub fn y_min(&self) -> Option<f64> {
        self.y_min
    }

    /// Returns the current Y-axis maximum (manual or auto-computed).
    pub fn y_max(&self) -> Option<f64> {
        self.y_max
    }

    /// Adds a single data point to the chart.
    pub fn add_point(&mut self, x: f64, y: f64) {
        self.data.push((x, y));
        self.base.request_redraw();
    }

    /// Removes all data points from the chart.
    pub fn clear_data(&mut self) {
        self.data.clear();
        self.base.request_redraw();
    }

    /// Returns a reference to the current data points.
    pub fn data(&self) -> &[(f64, f64)] {
        &self.data
    }

    /// Enables or disables the grid lines.
    pub fn set_show_grid(&mut self, show: bool) {
        self.show_grid = show;
        self.base.request_redraw();
    }

    /// Returns whether grid lines are shown.
    pub fn show_grid(&self) -> bool {
        self.show_grid
    }

    /// Sets the grid line color.
    pub fn set_grid_color(&mut self, color: Color) {
        self.grid_color = color;
        self.base.request_redraw();
    }

    /// Returns the current grid color.
    pub fn grid_color(&self) -> Color {
        self.grid_color
    }

    /// Enables or disables axis labels.
    pub fn set_show_labels(&mut self, show: bool) {
        self.show_labels = show;
        self.base.request_redraw();
    }

    /// Returns whether axis labels are shown.
    pub fn show_labels(&self) -> bool {
        self.show_labels
    }

    /// Resolves X-axis range.
    fn resolve_x_range(&self) -> (f64, f64) {
        match (self.x_min, self.x_max) {
            (Some(min), Some(max)) => (min, max),
            _ => {
                let data = &self.data;
                if data.is_empty() {
                    return (0.0, 10.0);
                }
                let min = data.iter().map(|p| p.0).fold(f64::INFINITY, f64::min);
                let max = data.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max);
                if (max - min).abs() < f64::EPSILON {
                    return (min - 1.0, min + 1.0);
                }
                let padding = (max - min) * 0.1;
                (min - padding, max + padding)
            }
        }
    }

    /// Resolves Y-axis range.
    fn resolve_y_range(&self) -> (f64, f64) {
        match (self.y_min, self.y_max) {
            (Some(min), Some(max)) => (min, max),
            _ => {
                let data = &self.data;
                if data.is_empty() {
                    return (0.0, 10.0);
                }
                let min = data.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
                let max = data.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max);
                if (max - min).abs() < f64::EPSILON {
                    return (min - 1.0, min + 1.0);
                }
                let padding = (max - min) * 0.1;
                (min - padding, max + padding)
            }
        }
    }

    /// Maps a data coordinate to a pixel position on screen.
    fn map_to_pixel(
        x: f64,
        y: f64,
        x_min: f64,
        x_max: f64,
        y_min: f64,
        y_max: f64,
        plot_rect: Rect,
    ) -> Point {
        let px = plot_rect.x + (plot_rect.width as f64 * ((x - x_min) / (x_max - x_min))) as i32;
        let py =
            plot_rect.y + (plot_rect.height as f64 * (1.0 - (y - y_min) / (y_max - y_min))) as i32;
        Point::new(px, py)
    }

    /// Returns the plot area (inside margins for labels).
    fn plot_area(&self) -> Rect {
        let rect = self.base.geometry();
        let margin_left = if self.show_labels { 50 } else { 10 };
        let margin_right = 10;
        let margin_top = 10;
        let margin_bottom = if self.show_labels { 30 } else { 10 };
        let x = rect.x + margin_left;
        let y = rect.y + margin_top;
        let w = (rect.width as i32 - margin_left - margin_right).max(10) as u32;
        let h = (rect.height as i32 - margin_top - margin_bottom).max(10) as u32;
        Rect::new(x, y, w, h)
    }
}

impl Widget for LineChart {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        crate::core::Size::new(400, 300)
    }
}

impl Draw for LineChart {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.base.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }

        let plot_area = self.plot_area();
        let (x_min, x_max) = self.resolve_x_range();
        let (y_min, y_max) = self.resolve_y_range();

        let is_enabled = self.base.is_enabled();
        let disabled_color = Color::DISABLED_FOREGROUND;

        // ── Draw axes ──
        let axis_color = if is_enabled { Color::DARK_GRAY } else { disabled_color };
        // Y-axis (left edge of plot area)
        context.draw_line_stroke(
            Point::new(plot_area.x, plot_area.y),
            Point::new(plot_area.x, plot_area.y + plot_area.height as i32),
            axis_color,
            1,
        );
        // X-axis (bottom edge of plot area)
        context.draw_line_stroke(
            Point::new(plot_area.x, plot_area.y + plot_area.height as i32),
            Point::new(plot_area.x + plot_area.width as i32, plot_area.y + plot_area.height as i32),
            axis_color,
            1,
        );

        // ── Draw grid lines ──
        let grid_color = if is_enabled { self.grid_color } else { disabled_color };
        if self.show_grid {
            // Horizontal grid lines (5 lines)
            for i in 0..=4 {
                let t = i as f64 / 4.0;
                let y = plot_area.y + (plot_area.height as f64 * (1.0 - t)) as i32;
                context.draw_line_aa(
                    Point::new(plot_area.x + 1, y),
                    Point::new(plot_area.x + plot_area.width as i32 - 1, y),
                    grid_color,
                );
            }
            // Vertical grid lines (5 lines)
            for i in 0..=4 {
                let t = i as f64 / 4.0;
                let x = plot_area.x + (plot_area.width as f64 * t) as i32;
                context.draw_line_aa(
                    Point::new(x, plot_area.y + 1),
                    Point::new(x, plot_area.y + plot_area.height as i32 - 1),
                    grid_color,
                );
            }
        }

        // ── Draw labels ──
        if self.show_labels && is_enabled {
            let label_font = Font::new("sans-serif", 9.0, false, false);
            // Y-axis labels
            for i in 0..=4 {
                let t = i as f64 / 4.0;
                let val = y_min + (y_max - y_min) * (1.0 - t);
                let label = format!("{:.1}", val);
                let y_pos = plot_area.y + (plot_area.height as f64 * (1.0 - t)) as i32;
                let metrics = context.measure_text(&label, &label_font);
                let text_x = (plot_area.x - metrics.width as i32 - 4).max(0);
                let text_y = y_pos + (metrics.ascent as i32 / 2);
                context.draw_text(
                    Point::new(text_x, text_y),
                    &label,
                    &label_font,
                    Color::DARK_GRAY,
                    HorizontalAlignment::Left,
                );
            }
            // X-axis labels
            for i in 0..=4 {
                let t = i as f64 / 4.0;
                let val = x_min + (x_max - x_min) * t;
                let label = format!("{:.1}", val);
                let x_pos = plot_area.x + (plot_area.width as f64 * t) as i32;
                let metrics = context.measure_text(&label, &label_font);
                let text_x = x_pos - metrics.width as i32 / 2;
                let text_y = plot_area.y + plot_area.height as i32 + metrics.height as i32 + 2;
                context.draw_text(
                    Point::new(text_x.max(plot_area.x), text_y),
                    &label,
                    &label_font,
                    Color::DARK_GRAY,
                    HorizontalAlignment::Left,
                );
            }
        }

        // ── Draw data ──
        if self.data.len() < 2 {
            return;
        }

        let line_color = if is_enabled { self.line_color } else { disabled_color };

        // Map data points to pixel coordinates
        let points: Vec<Point> = self
            .data
            .iter()
            .map(|(x, y)| Self::map_to_pixel(*x, *y, x_min, x_max, y_min, y_max, plot_area))
            .collect();

        // ── Draw filled area under the line (if enabled) ──
        if self.fill_area {
            let fill_color =
                if is_enabled { self.fill_color } else { Color::rgba(200, 200, 200, 60) };
            let baseline_y = plot_area.y + plot_area.height as i32;
            // Draw vertical strips between consecutive points
            for i in 0..points.len() - 1 {
                let left_x = points[i].x;
                let right_x = points[i + 1].x;
                let strip_width = (right_x - left_x).max(1) as u32;
                let top_y = points[i].y.min(points[i + 1].y);
                let strip_height = (baseline_y - top_y).max(1) as u32;
                context.fill_rect(Rect::new(left_x, top_y, strip_width, strip_height), fill_color);
            }
        }

        // ── Draw line connecting points ──
        let stroke_w = self.stroke_width as u32;
        for i in 0..points.len() - 1 {
            context.draw_line_stroke_aa(points[i], points[i + 1], line_color, stroke_w.max(1));
        }
    }
}

impl EventHandler for LineChart {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::svg::render_to_svg;

    #[test]
    fn line_chart_default_creation() {
        let lc = LineChart::new(Rect::new(0, 0, 300, 200));
        assert_eq!(lc.kind(), WidgetKind::LineChart);
        assert!(lc.data().is_empty());
        assert_eq!(lc.line_color(), Color::PRIMARY);
        assert!(!lc.is_fill_area());
        assert!((lc.stroke_width() - 2.0).abs() < f32::EPSILON);
        assert!(lc.show_grid());
        assert!(lc.show_labels());
    }

    #[test]
    fn line_chart_set_data() {
        let mut lc = LineChart::new(Rect::new(0, 0, 300, 200));
        let points = vec![(0.0, 0.0), (1.0, 2.0), (2.0, 4.0), (3.0, 6.0)];
        lc.set_data(points.clone());
        assert_eq!(lc.data().len(), 4);
        assert!((lc.data()[1].1 - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn line_chart_add_point_and_clear() {
        let mut lc = LineChart::new(Rect::new(0, 0, 300, 200));
        lc.add_point(0.0, 1.0);
        lc.add_point(1.0, 3.0);
        assert_eq!(lc.data().len(), 2);
        lc.clear_data();
        assert!(lc.data().is_empty());
    }

    #[test]
    fn line_chart_line_color_and_stroke() {
        let mut lc = LineChart::new(Rect::new(0, 0, 300, 200));
        lc.set_line_color(Color::SUCCESS);
        assert_eq!(lc.line_color(), Color::SUCCESS);
        lc.set_stroke_width(4.0);
        assert!((lc.stroke_width() - 4.0).abs() < f32::EPSILON);
        lc.set_stroke_width(-1.0); // clamp
        assert!((lc.stroke_width() - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn line_chart_fill_area() {
        let mut lc = LineChart::new(Rect::new(0, 0, 300, 200));
        assert!(!lc.is_fill_area());
        lc.set_fill_area(true);
        assert!(lc.is_fill_area());
        lc.set_fill_color(Color::rgba(255, 0, 0, 100));
        assert_eq!(lc.fill_color(), Color::rgba(255, 0, 0, 100));
    }

    #[test]
    fn line_chart_axis_range() {
        let mut lc = LineChart::new(Rect::new(0, 0, 300, 200));
        assert!(lc.x_min().is_none());
        assert!(lc.x_max().is_none());
        lc.set_axis_range(Some(0.0), Some(10.0), Some(-1.0), Some(1.0));
        assert!((lc.x_min().unwrap() - 0.0).abs() < f64::EPSILON);
        assert!((lc.x_max().unwrap() - 10.0).abs() < f64::EPSILON);
        assert!((lc.y_min().unwrap() - (-1.0)).abs() < f64::EPSILON);
        assert!((lc.y_max().unwrap() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn line_chart_grid_and_labels() {
        let mut lc = LineChart::new(Rect::new(0, 0, 300, 200));
        assert!(lc.show_grid());
        lc.set_show_grid(false);
        assert!(!lc.show_grid());
        assert!(lc.show_labels());
        lc.set_show_labels(false);
        assert!(!lc.show_labels());
        lc.set_grid_color(Color::LIGHT_GRAY);
        assert_eq!(lc.grid_color(), Color::LIGHT_GRAY);
    }

    #[test]
    fn line_chart_svg_output() {
        let mut lc = LineChart::new(Rect::new(0, 0, 300, 200));
        lc.set_data(vec![(0.0, 0.0), (1.0, 2.0), (2.0, 4.0), (3.0, 6.0)]);
        let svg = render_to_svg(&mut lc);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }

    #[test]
    fn line_chart_empty_data_no_crash() {
        let mut lc = LineChart::new(Rect::new(0, 0, 300, 200));
        let svg = render_to_svg(&mut lc);
        assert!(svg.starts_with("<svg"));
    }

    #[test]
    fn line_chart_fill_area_svg_output() {
        let mut lc = LineChart::new(Rect::new(0, 0, 300, 200));
        lc.set_data(vec![(0.0, 1.0), (1.0, 3.0), (2.0, 2.0), (3.0, 5.0)]);
        lc.set_fill_area(true);
        let svg = render_to_svg(&mut lc);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }

    #[test]
    fn line_chart_event_forwarding() {
        let mut lc = LineChart::new(Rect::new(0, 0, 300, 200));
        lc.handle_event(&Event::MouseMove { pos: Point::new(10, 10) });
        lc.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1 });
    }
}
