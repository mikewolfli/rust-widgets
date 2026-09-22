// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Sparkline widget — a compact inline chart with no axes or labels.
//!
//! A sparkline is a small, word-sized line chart without axes, typically used to
//! show trends or patterns in a compact space. This widget draws a mini line
//! connecting data values with an optional last-point highlight.
//!
//! # Why this does not use the shared chart engine
//!
//! Unlike [`BarChart`](super::bar_chart) and [`LineChart`](super::line_chart),
//! a sparkline is *defined* by having no chart chrome: no plot margins, no axes,
//! no grid, no tick labels, no legend. Those are exactly the pieces
//! [`crate::widget::chart_widgets`] provides, so routing through it would add an adapter hop and
//! reserve margins the sparkline must not have — cost with no shared logic to
//! show for it.
//!
//! The only overlap is value-range resolution, and even that differs: a
//! sparkline pads its own min/max and always spans the full widget rectangle,
//! whereas the engine fits values into a padded plot area. Keeping the two
//! separate is deliberate, not an oversight.

use crate::core::{Color, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::widget::capability::coercion::expect_f32;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Radius of the highlight disc drawn on the last sample, in logical pixels.
///
/// Named because the plot's inset has to allow for it: a disc of radius `r` centred on the
/// final point reaches `r` px beyond that point, so the point itself must be inset by `r`
/// for the disc to stay inside the control.
const SPARKLINE_DOT_RADIUS: u32 = 3;

/// The edging a sparkline leaves between the control's edge and its plot, in pixels: 4.
///
/// The trace used to be inset by half the stroke alone — the exact amount its ink needs and
/// no more — so the data's extremes were drawn flush against the control's edges
/// (`sparkline.svg` spanned y 13..107 in a 120 px box). A plot that touches its own frame reads
/// as decoration rather than as a series, and a real sparkline (Flutter's, `fl_chart`'s) always
/// leaves a margin.
const SPARKLINE_PLOT_MARGIN: i32 = 4;

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

    fn size_hint(&self) -> Size {
        crate::core::Size::new(200, 40)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `Sparkline`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` / `access_write_other.in.rs` dispatch, including
/// the `f32` → `f64` widening on read.
impl WidgetProperties for Sparkline {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "stroke_width" => Ok(CapabilityValue::Float(self.stroke_width() as f64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "stroke_width" => {
                self.set_stroke_width(expect_f32(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["stroke_width", BASE_PROPERTY_NAMES]
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

        // Map data values to pixel coordinates
        let n = self.data.len();
        // The plot is inset by the chart's own margin **plus** half the stroke (and the
        // highlight dot's radius when one is drawn), so the *painted* extent stays inside the
        // control with a visible gap. A 1 px stroke centred on `rect.x` reaches half a pixel
        // outside it, and the dot is a radius-3 disc centred on the last sample — at the right
        // edge half of it was painted past the control, which the raster backend clipped and
        // the SVG snapshot showed hanging out.
        //
        // The half-stroke inset alone was not enough: it is exactly what the ink needs and no
        // more, so `sparkline.svg` drew its trace from y=13 to y=107 in a 120 px box — the data
        // extremes sat on the frame, with the plot flush against the edges it was given. A
        // sparkline is still a plot, so it takes `SPARKLINE_PLOT_MARGIN` of edging *inside* the
        // half-stroke, which is what makes the trace read as a series on a surface rather than
        // as a border decoration.
        let stroke_w = self.stroke_width as u32;
        let dot_radius = if self.show_last_point { SPARKLINE_DOT_RADIUS } else { 0 };
        let half_stroke = (stroke_w.max(1) as i32 + 1) / 2;
        let inset = half_stroke.max(dot_radius as i32) + SPARKLINE_PLOT_MARGIN;
        let inset = inset.min(rect.width as i32 / 2).min(rect.height as i32 / 2);
        let x0 = rect.x + inset;
        let x1 = rect.x + rect.width as i32 - 1 - inset;
        let y_top = rect.y + inset;
        let y_bottom = rect.y + rect.height as i32 - 1 - inset;
        let plot_height = (y_bottom - y_top).max(1) as f64;
        let mapped: Vec<Point> = self
            .data
            .iter()
            .enumerate()
            .map(|(i, val)| {
                let px = if n > 1 { x0 + (x1 - x0) * i as i32 / (n - 1) as i32 } else { x0 };
                let py = y_bottom - ((val - y_min) / (y_max - y_min) * plot_height) as i32;
                Point::new(px, py.max(y_top))
            })
            .collect();

        // ── Draw line segments ──
        for i in 0..mapped.len().saturating_sub(1) {
            context.draw_line_stroke_aa(mapped[i], mapped[i + 1], line_color, stroke_w.max(1));
        }

        // ── Draw last-point highlight dot ──
        if self.show_last_point && !mapped.is_empty() {
            let dot_color =
                if is_enabled { self.last_point_color } else { Color::DISABLED_FOREGROUND };
            let last = mapped[mapped.len() - 1];
            context.fill_circle(last, dot_radius, dot_color);
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
