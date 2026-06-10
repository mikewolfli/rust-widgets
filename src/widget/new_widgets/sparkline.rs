//! Sparkline widget — a compact inline sparkline chart.
//!
//! A sparkline is a small, word-sized line chart without axes, typically used
//! to show trends or patterns in a compact space. This widget draws a mini
//! line connecting data values with an optional last-point highlight.

use crate::core::{Color, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// A compact inline sparkline chart widget.
///
/// Draws a mini line chart without axes. Only the line itself and an optional
/// last-point highlight dot are shown. Suitable for embedding in tables, labels,
/// or small card UIs.
pub struct Sparkline {
    base: BaseWidget,
    data: Vec<f64>,
    line_color: Color,
    stroke_width: f32,
    show_last_point: bool,
    last_point_color: Color,
    min: Option<f64>,
    max: Option<f64>,
}

impl Sparkline {
    /// Creates a new Sparkline widget with the given geometry.
    ///
    /// Defaults: green line, stroke width 1.5, last point highlight enabled with
    /// a darker green dot.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Sparkline, geometry, "Sparkline"),
            data: Vec::new(),
            line_color: Color::SUCCESS,
            stroke_width: 1.5,
            show_last_point: true,
            last_point_color: Color::rgba(34, 139, 34, 255),
            min: None,
            max: None,
        }
    }

    /// Sets the data values to display. Clears any previous values.
    pub fn set_data(&mut self, values: Vec<f64>) {
        self.data = values;
        self.base.request_redraw();
    }

    /// Returns a reference to the current data values.
    pub fn data(&self) -> &[f64] {
        &self.data
    }

    /// Sets the line color.
    pub fn set_color(&mut self, color: Color) {
        self.line_color = color;
        self.last_point_color = color;
        self.base.request_redraw();
    }

    /// Returns the current line color.
    pub fn line_color(&self) -> Color {
        self.line_color
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

    /// Sets whether the last point is highlighted with a colored dot.
    pub fn set_show_last_point(&mut self, show: bool) {
        self.show_last_point = show;
        self.base.request_redraw();
    }

    /// Returns whether the last point highlight is enabled.
    pub fn show_last_point(&self) -> bool {
        self.show_last_point
    }

    /// Sets the color used for the last-point highlight dot.
    pub fn set_last_point_color(&mut self, color: Color) {
        self.last_point_color = color;
        self.base.request_redraw();
    }

    /// Returns the current last-point highlight color.
    pub fn last_point_color(&self) -> Color {
        self.last_point_color
    }

    /// Clears all data values.
    pub fn clear(&mut self) {
        self.data.clear();
        self.base.request_redraw();
    }

    /// Adds a single value to the data series.
    pub fn add_value(&mut self, value: f64) {
        self.data.push(value);
        self.base.request_redraw();
    }

    /// Sets manual min/max values for the Y range.
    /// Pass `None` for auto-compute.
    pub fn set_range(&mut self, min: Option<f64>, max: Option<f64>) {
        self.min = min;
        self.max = max;
        self.base.request_redraw();
    }

    /// Returns the configured Y-min (if manually set).
    pub fn min(&self) -> Option<f64> {
        self.min
    }

    /// Returns the configured Y-max (if manually set).
    pub fn max(&self) -> Option<f64> {
        self.max
    }

    /// Resolves the Y range: uses manual if set, otherwise auto-computes.
    fn resolve_y_range(&self) -> (f64, f64) {
        match (self.min, self.max) {
            (Some(min), Some(max)) => (min, max),
            _ => {
                let data = &self.data;
                if data.is_empty() {
                    return (0.0, 1.0);
                }
                let min = data.iter().cloned().fold(f64::INFINITY, f64::min);
                let max = data.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                if (max - min).abs() < f64::EPSILON {
                    return (min - 1.0, min + 1.0);
                }
                let padding = (max - min) * 0.1;
                (min - padding, max + padding)
            }
        }
    }
}

impl Widget for Sparkline {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}

impl Draw for Sparkline {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.base.geometry();
        if rect.width == 0 || rect.height == 0 || self.data.len() < 2 {
            return;
        }

        let is_enabled = self.base.is_enabled();
        let line_color = if is_enabled { self.line_color } else { Color::DISABLED_FOREGROUND };

        let (y_min, y_max) = self.resolve_y_range();

        let x0 = rect.x;
        let x1 = rect.x + rect.width as i32 - 1;
        let y_bottom = rect.y + rect.height as i32 - 1;

        // Map data values to pixel coordinates
        let n = self.data.len();
        let mapped: Vec<Point> = self
            .data
            .iter()
            .enumerate()
            .map(|(i, val)| {
                let px = if n > 1 { x0 + (x1 - x0) * i as i32 / (n - 1) as i32 } else { x0 };
                let py = y_bottom - ((val - y_min) / (y_max - y_min) * rect.height as f64) as i32;
                Point::new(px, py)
            })
            .collect();

        // ── Draw line segments ──
        let stroke_w = self.stroke_width as u32;
        for i in 0..mapped.len() - 1 {
            context.draw_line_stroke_aa(mapped[i], mapped[i + 1], line_color, stroke_w.max(1));
        }

        // ── Draw last-point highlight dot ──
        if self.show_last_point && !mapped.is_empty() {
            let dot_color =
                if is_enabled { self.last_point_color } else { Color::DISABLED_FOREGROUND };
            let last = mapped[mapped.len() - 1];
            context.fill_circle(last, 3, dot_color);
        }
    }
}

impl EventHandler for Sparkline {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::svg::render_to_svg;

    #[test]
    fn sparkline_default_creation() {
        let sl = Sparkline::new(Rect::new(0, 0, 80, 24));
        assert_eq!(sl.kind(), WidgetKind::Sparkline);
        assert!(sl.data().is_empty());
        assert_eq!(sl.line_color(), Color::SUCCESS);
        assert!((sl.stroke_width() - 1.5).abs() < f32::EPSILON);
        assert!(sl.show_last_point());
    }

    #[test]
    fn sparkline_set_data_and_clear() {
        let mut sl = Sparkline::new(Rect::new(0, 0, 80, 24));
        sl.set_data(vec![1.0, 2.0, 3.0, 4.0, 5.0]);
        assert_eq!(sl.data().len(), 5);
        sl.clear();
        assert!(sl.data().is_empty());
    }

    #[test]
    fn sparkline_add_value() {
        let mut sl = Sparkline::new(Rect::new(0, 0, 80, 24));
        sl.add_value(10.0);
        sl.add_value(20.0);
        assert_eq!(sl.data().len(), 2);
    }

    #[test]
    fn sparkline_set_color() {
        let mut sl = Sparkline::new(Rect::new(0, 0, 80, 24));
        sl.set_color(Color::WARNING);
        assert_eq!(sl.line_color(), Color::WARNING);
        assert_eq!(sl.last_point_color(), Color::WARNING);
    }

    #[test]
    fn sparkline_last_point_toggle() {
        let mut sl = Sparkline::new(Rect::new(0, 0, 80, 24));
        assert!(sl.show_last_point());
        sl.set_show_last_point(false);
        assert!(!sl.show_last_point());
        sl.set_last_point_color(Color::ERROR);
        assert_eq!(sl.last_point_color(), Color::ERROR);
    }

    #[test]
    fn sparkline_svg_output() {
        let mut sl = Sparkline::new(Rect::new(0, 0, 80, 24));
        sl.set_data(vec![1.0, 3.0, 2.0, 5.0, 4.0, 6.0]);
        let svg = render_to_svg(&mut sl);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }

    #[test]
    fn sparkline_empty_data_no_crash() {
        let mut sl = Sparkline::new(Rect::new(0, 0, 80, 24));
        let svg = render_to_svg(&mut sl);
        assert!(svg.starts_with("<svg"));
    }

    #[test]
    fn sparkline_single_point_no_crash() {
        let mut sl = Sparkline::new(Rect::new(0, 0, 80, 24));
        sl.add_value(42.0);
        let svg = render_to_svg(&mut sl);
        assert!(svg.starts_with("<svg"));
    }

    #[test]
    fn sparkline_event_forwarding() {
        let mut sl = Sparkline::new(Rect::new(0, 0, 80, 24));
        sl.handle_event(&Event::MouseMove { pos: Point::new(5, 5) });
        sl.handle_event(&Event::MousePress { pos: Point::new(5, 5), button: 1 });
    }
}
