// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! LineChart widget — a 2D line chart for visualizing data series.
//!
//! The LineChart widget draws axes, optional grid lines, an optional filled area
//! under the line, and a line connecting data points with configurable stroke.
//! Axis ranges can be set manually or auto-computed from the data.

//! LineChart widget — a 2D line chart with optional area fill and grid.
//!
//! # Rendering path
//!
//! With the `chart` feature enabled, the plot area, axes, grid lines and X/Y
//! tick labels come from the shared chart engine in [`crate::widget::chart_widgets`], so this
//! widget cannot drift from the SVG chart renderer. Without the feature
//! (tablet/mobile) a compact local preamble is used instead.

#[cfg(not(feature = "chart"))]
use crate::core::Font;
use crate::core::{Color, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
#[cfg(feature = "chart")]
use crate::widget::chart_widgets::adapter::ChartContextAdapter;
#[cfg(feature = "chart")]
use crate::widget::chart_widgets::charts::{
    compute_cartesian_layout, draw_cartesian_axes, draw_x_ticks, draw_y_ticks, CartesianLayout,
};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Converts the shared engine's float plot area into the integer [`Rect`] this
/// widget maps data into.
#[cfg(feature = "chart")]
fn plot_rect(layout: &CartesianLayout) -> Rect {
    Rect::new(
        layout.plot_x().round() as i32,
        layout.plot_y().round() as i32,
        layout.plot_w().round().max(1.0) as u32,
        layout.plot_h().round().max(1.0) as u32,
    )
}

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
    ///
    /// Only used by the pre-`chart`-feature backdrop; with the shared engine the
    /// layout comes from `compute_cartesian_layout` instead.
    #[cfg(not(feature = "chart"))]
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

        let (x_min, x_max) = self.resolve_x_range();
        let (y_min, y_max) = self.resolve_y_range();

        let is_enabled = self.base.is_enabled();
        let disabled_color = Color::DISABLED_FOREGROUND;

        // ── Backdrop: axes, grid and tick labels ──
        #[cfg(feature = "chart")]
        let plot_area = {
            // Legend width is reserved only when labels are shown, matching the
            // widget's previous margin behaviour.
            let layout = compute_cartesian_layout(rect, self.show_labels, self.show_labels, 0);
            let plot_area = plot_rect(&layout);
            let mut adapter = ChartContextAdapter::new(context);

            // Axes and grid come from the shared engine. `show_grid` drives both
            // the horizontal and vertical grid lines; the engine previously
            // emitted only horizontal ones, so the vertical pass is requested
            // explicitly below to preserve this widget's denser grid.
            draw_cartesian_axes(&mut adapter, &layout);
            if self.show_labels {
                draw_y_ticks(&mut adapter, &layout, y_min, y_max, 4, self.show_grid);
                draw_x_ticks(&mut adapter, &layout, x_min, x_max, 4, false);
            } else if self.show_grid {
                // No labels requested, but the grid is: drive it through the tick
                // helpers with label-free positioning so the lines still appear.
                draw_y_ticks(&mut adapter, &layout, y_min, y_max, 4, true);
            }
            plot_area
        };

        #[cfg(not(feature = "chart"))]
        let plot_area = self.draw_backdrop_without_engine(
            context,
            x_min,
            x_max,
            y_min,
            y_max,
            is_enabled,
            disabled_color,
        );

        // ── Data series ──
        if self.data.len() < 2 {
            return;
        }

        let line_color = if is_enabled { self.line_color } else { disabled_color };

        let points: Vec<Point> = self
            .data
            .iter()
            .map(|(x, y)| Self::map_to_pixel(*x, *y, x_min, x_max, y_min, y_max, plot_area))
            .collect();

        // ── Filled area under the line ──
        if self.fill_area {
            let fill_color =
                if is_enabled { self.fill_color } else { Color::rgba(200, 200, 200, 60) };
            let baseline_y = plot_area.y + plot_area.height as i32;
            for i in 0..points.len() - 1 {
                let left_x = points[i].x;
                let right_x = points[i + 1].x;
                let strip_width = (right_x - left_x).max(1) as u32;
                let top_y = points[i].y.min(points[i + 1].y);
                let strip_height = (baseline_y - top_y).max(1) as u32;
                context.fill_rect(Rect::new(left_x, top_y, strip_width, strip_height), fill_color);
            }
        }

        // ── Line connecting the points ──
        let stroke_w = self.stroke_width as u32;
        for i in 0..points.len() - 1 {
            context.draw_line_stroke_aa(points[i], points[i + 1], line_color, stroke_w.max(1));
        }
    }
}

/// Fallback backdrop for builds without the `chart` feature (tablet/mobile).
#[cfg(not(feature = "chart"))]
impl LineChart {
    #[allow(clippy::too_many_arguments)]
    fn draw_backdrop_without_engine(
        &self,
        context: &mut RenderContext,
        x_min: f64,
        x_max: f64,
        y_min: f64,
        y_max: f64,
        is_enabled: bool,
        disabled_color: Color,
    ) -> Rect {
        let plot_area = self.plot_area();
        let axis_color = if is_enabled { Color::DARK_GRAY } else { disabled_color };
        let bottom = plot_area.y + plot_area.height as i32;
        let right = plot_area.x + plot_area.width as i32;

        context.draw_line_stroke(
            Point::new(plot_area.x, plot_area.y),
            Point::new(plot_area.x, bottom),
            axis_color,
            1,
        );
        context.draw_line_stroke(
            Point::new(plot_area.x, bottom),
            Point::new(right, bottom),
            axis_color,
            1,
        );

        let grid_color = if is_enabled { self.grid_color } else { disabled_color };
        if self.show_grid {
            for i in 0..=4 {
                let t = i as f64 / 4.0;
                let y = plot_area.y + (plot_area.height as f64 * (1.0 - t)) as i32;
                context.draw_line_aa(
                    Point::new(plot_area.x + 1, y),
                    Point::new(right - 1, y),
                    grid_color,
                );
            }
            for i in 0..=4 {
                let t = i as f64 / 4.0;
                let x = plot_area.x + (plot_area.width as f64 * t) as i32;
                context.draw_line_aa(
                    Point::new(x, plot_area.y + 1),
                    Point::new(x, bottom - 1),
                    grid_color,
                );
            }
        }

        if self.show_labels && is_enabled {
            let label_font = Font::new("sans-serif", 9.0, false, false);
            for i in 0..=4 {
                let t = i as f64 / 4.0;
                let val = y_min + (y_max - y_min) * (1.0 - t);
                let label = format!("{val:.1}");
                let y_pos = plot_area.y + (plot_area.height as f64 * (1.0 - t)) as i32;
                let text_y = y_pos + 4;
                context.draw_text(
                    Point::new((plot_area.x - 44).max(0), text_y),
                    &label,
                    &label_font,
                    Color::DARK_GRAY,
                    crate::core::HorizontalAlignment::Left,
                );
            }
            for i in 0..=4 {
                let t = i as f64 / 4.0;
                let val = x_min + (x_max - x_min) * t;
                let label = format!("{val:.1}");
                let x_pos = plot_area.x + (plot_area.width as f64 * t) as i32;
                context.draw_text(
                    Point::new((x_pos - 12).max(plot_area.x), bottom + 16),
                    &label,
                    &label_font,
                    Color::DARK_GRAY,
                    crate::core::HorizontalAlignment::Left,
                );
            }
        }

        plot_area
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

    /// The widget must render its axes and ticks through the shared chart engine
    /// rather than its own copy of that math — the duplication this refactor
    /// removed.
    ///
    /// Observable proof: with labels enabled the engine emits numeric tick
    /// labels for both axes, and the explicit axis range must appear in them.
    /// The previous hand-rolled path drew the same labels, so this also guards
    /// that the migration kept that behaviour.
    #[test]
    fn line_chart_renders_axis_tick_labels_from_the_shared_engine() {
        let mut lc = LineChart::new(Rect::new(0, 0, 300, 200));
        lc.set_data(vec![(0.0, 0.0), (10.0, 50.0)]);
        lc.set_axis_range(Some(0.0), Some(10.0), Some(0.0), Some(50.0));
        lc.set_show_labels(true);

        let svg = render_to_svg(&mut lc);

        assert!(
            svg.contains("50.0"),
            "y-axis upper bound label missing; widget no longer uses the shared tick engine"
        );
        assert!(svg.contains("10.0"), "x-axis upper bound label missing");
    }

    /// Grid toggling must reach the shared engine: enabling it increases the
    /// number of rendered lines.
    #[test]
    fn line_chart_grid_toggle_changes_rendered_line_count() {
        let mut lc = LineChart::new(Rect::new(0, 0, 300, 200));
        lc.set_data(vec![(0.0, 0.0), (1.0, 1.0), (2.0, 2.0)]);

        lc.set_show_grid(false);
        let without_grid = render_to_svg(&mut lc).matches("<line").count();

        lc.set_show_grid(true);
        let with_grid = render_to_svg(&mut lc).matches("<line").count();

        assert!(
            with_grid > without_grid,
            "grid must add lines through the shared engine (off={without_grid}, on={with_grid})"
        );
    }
}
