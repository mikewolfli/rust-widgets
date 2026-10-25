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
use crate::signal::Signal1;
use crate::widget::capability::coercion::{
    expect_bool, expect_f32, expect_number_list, expect_u32,
};
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
/// as decoration rather than as a series, and a real sparkline always
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
    /// The caller's trace colour, or `None` while the trace follows the theme's
    /// success token.
    ///
    /// The field used to be a plain `Color` seeded with `Color::SUCCESS`, which is a
    /// fixed green painted identically on every appearance — the dark and light
    /// snapshots were byte-for-byte the same, so a theme author restyling success had
    /// no way to reach the widget. An unset colour now resolves through
    /// [`crate::style::semantic_color`] (falling back to `Color::SUCCESS` only when no
    /// theme is active) and [`Sparkline::set_color`] still wins when the caller states
    /// one.
    line_color: Option<Color>,
    stroke_width: f32,
    show_last_point: bool,
    /// The caller's highlight-dot colour, or `None` while the dot is derived from the
    /// resolved trace colour (the trace darkened, preserving the documented "darker
    /// dot" default).
    last_point_color: Option<Color>,
    min: Option<f64>,
    max: Option<f64>,
    /// Whether the final sample's value is printed beside the trace.
    show_end_value: bool,
    /// Whether the area under the trace is filled.
    fill: bool,
    /// Whether every sample gets a small disc.
    show_points: bool,
    /// Maximum number of retained samples; `0` means unbounded.
    capacity: usize,
    /// The sample index under the pointer, while one is hovered.
    hovered_index: Option<usize>,
    /// Emitted with `(index, value)` when the pointer enters a different sample.
    pub value_hovered: Signal1<(usize, f64)>,
    /// Emitted with the final sample's value when the last-point marker is clicked.
    pub last_value_clicked: Signal1<f64>,
    /// Smallest and largest finite sample, maintained incrementally as the series grows.
    ///
    /// `None` while the series is empty. Kept in step by `set_data`, `add_value` and
    /// `clear`; `resolve_y_range` reads it instead of scanning the whole series on
    /// every paint.
    series_min: Option<f64>,
    series_max: Option<f64>,
}

impl Sparkline {
    /// Creates a new Sparkline widget with the given geometry.
    ///
    /// Defaults: a trace in the theme's success colour, stroke width 1.5, last point
    /// highlight enabled with a dot derived from (darker than) the trace.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Sparkline, geometry, "Sparkline"),
            data: Vec::new(),
            line_color: None,
            stroke_width: 1.5,
            show_last_point: true,
            last_point_color: None,
            min: None,
            max: None,
            show_end_value: false,
            fill: false,
            show_points: false,
            capacity: 0,
            hovered_index: None,
            value_hovered: Signal1::new(),
            last_value_clicked: Signal1::new(),
            series_min: None,
            series_max: None,
        }
    }

    /// Sets the data values to display. Clears any previous values.
    ///
    /// Non-finite values are dropped: a `NaN` sample has no honest baseline position and an infinite
    /// one stretches the auto-range so every real sample collapses onto one pixel.
    pub fn set_data(&mut self, values: Vec<f64>) {
        let mut data = Vec::with_capacity(values.len());
        let mut min = f64::INFINITY;
        let mut max = f64::NEG_INFINITY;
        for value in values {
            if value.is_finite() {
                min = min.min(value);
                max = max.max(value);
                data.push(value);
            }
        }
        self.data = data;
        let removed = self.apply_capacity();
        if self.data.is_empty() {
            self.series_min = None;
            self.series_max = None;
        } else {
            if removed {
                self.recompute_extents();
            } else {
                self.series_min = Some(min);
                self.series_max = Some(max);
            }
        }
        self.base.request_redraw();
    }

    /// Returns a reference to the current data values.
    pub fn data(&self) -> &[f64] {
        &self.data
    }

    /// Sets the line color, overriding the theme-derived default. The last-point dot
    /// follows the same explicit colour, as it always has.
    pub fn set_color(&mut self, color: Color) {
        if self.line_color == Some(color) && self.last_point_color == Some(color) {
            return;
        }
        self.line_color = Some(color);
        self.last_point_color = Some(color);
        self.base.request_redraw();
    }

    /// Returns the colour the trace is drawn in: the caller's override, or the theme's
    /// success token when none was set.
    pub fn line_color(&self) -> Color {
        self.line_color.unwrap_or_else(|| {
            crate::style::semantic_color(crate::style::SemanticColor::Success)
                .unwrap_or(Color::SUCCESS)
        })
    }

    /// Sets the stroke width of the line.
    pub fn set_stroke_width(&mut self, width: f32) {
        let width = width.max(0.5);
        if self.stroke_width == width {
            return;
        }
        self.stroke_width = width;
        self.base.request_redraw();
    }

    /// Returns the current stroke width.
    pub fn stroke_width(&self) -> f32 {
        self.stroke_width
    }

    /// Sets whether the last point is highlighted with a colored dot.
    pub fn set_show_last_point(&mut self, show: bool) {
        if self.show_last_point == show {
            return;
        }
        self.show_last_point = show;
        self.base.request_redraw();
    }

    /// Returns whether the last point highlight is enabled.
    pub fn show_last_point(&self) -> bool {
        self.show_last_point
    }

    /// Sets the color used for the last-point highlight dot, overriding the
    /// trace-derived default.
    pub fn set_last_point_color(&mut self, color: Color) {
        if self.last_point_color == Some(color) {
            return;
        }
        self.last_point_color = Some(color);
        self.base.request_redraw();
    }

    /// Returns the colour the highlight dot is drawn in: the caller's override, or the
    /// resolved trace colour darkened, keeping the dot visibly part of the same series.
    pub fn last_point_color(&self) -> Color {
        self.last_point_color.unwrap_or_else(|| self.line_color().blend(&Color::BLACK, 0.3))
    }

    /// Clears all data values.
    pub fn clear(&mut self) {
        self.data.clear();
        self.series_min = None;
        self.series_max = None;
        self.base.request_redraw();
    }

    /// Adds a single value to the data series.
    ///
    /// A non-finite value is refused; see [`Self::set_data`].
    pub fn add_value(&mut self, value: f64) {
        if !value.is_finite() {
            log::warn!("[sparkline] ignoring the non-finite value {value}");
            return;
        }
        self.data.push(value);
        let evicted = self.apply_capacity();
        if evicted {
            self.recompute_extents();
        } else {
            self.series_min = Some(self.series_min.map_or(value, |min| min.min(value)));
            self.series_max = Some(self.series_max.map_or(value, |max| max.max(value)));
        }
        self.base.request_redraw();
    }

    /// Truncates the series to [`Self::capacity`], keeping the newest samples.
    ///
    /// Returns whether anything was dropped, so the caller knows the cached extents are stale.
    fn apply_capacity(&mut self) -> bool {
        if self.capacity == 0 || self.data.len() <= self.capacity {
            return false;
        }
        let excess = self.data.len() - self.capacity;
        self.data.drain(0..excess);
        true
    }

    /// Recomputes the cached extents from the whole series.
    fn recompute_extents(&mut self) {
        if self.data.is_empty() {
            self.series_min = None;
            self.series_max = None;
            return;
        }
        let mut min = f64::INFINITY;
        let mut max = f64::NEG_INFINITY;
        for value in &self.data {
            min = min.min(*value);
            max = max.max(*value);
        }
        self.series_min = Some(min);
        self.series_max = Some(max);
    }

    /// Whether the area under the trace is filled.
    pub fn fill(&self) -> bool {
        self.fill
    }

    /// Enables or disables the area fill.
    pub fn set_fill(&mut self, fill: bool) {
        if self.fill == fill {
            return;
        }
        self.fill = fill;
        self.base.request_redraw();
    }

    /// Whether every sample is marked with a disc.
    pub fn show_points(&self) -> bool {
        self.show_points
    }

    /// Shows or hides a disc on every sample.
    pub fn set_show_points(&mut self, show: bool) {
        if self.show_points == show {
            return;
        }
        self.show_points = show;
        self.base.request_redraw();
    }

    /// The retained-sample limit; `0` means unbounded.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Caps the number of retained samples, dropping the oldest beyond the limit.
    ///
    /// `0` removes the cap. A cap that is smaller than the current series truncates it
    /// immediately, so the limit is a fact about the control rather than a future promise.
    pub fn set_capacity(&mut self, capacity: usize) {
        if self.capacity == capacity {
            return;
        }
        self.capacity = capacity;
        if self.apply_capacity() {
            self.recompute_extents();
            self.hovered_index = None;
        }
        self.base.request_redraw();
    }

    /// The last-point marker's visibility, the documented `show_last_marker` spelling.
    pub fn show_last_marker(&self) -> bool {
        self.show_last_point
    }

    /// Shows or hides the last-point marker.
    pub fn set_show_last_marker(&mut self, show: bool) {
        self.set_show_last_point(show);
    }

    /// Sets manual min/max values for the Y range.
    /// Pass `None` for auto-compute.
    pub fn set_range(&mut self, min: Option<f64>, max: Option<f64>) {
        if self.min == min && self.max == max {
            return;
        }
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

    /// Shows or hides the last sample's value beside the trace.
    ///
    /// The annotation is opt-in: a sparkline is often embedded in a table cell, where a
    /// number beside every row is noise rather than information.
    pub fn set_show_end_value(&mut self, show: bool) {
        if self.show_end_value == show {
            return;
        }
        self.show_end_value = show;
        self.base.request_redraw();
    }

    /// Returns whether the last sample's value is annotated.
    pub fn show_end_value(&self) -> bool {
        self.show_end_value
    }

    /// The formatted last-sample annotation, if there is a sample.
    pub fn end_value_text(&self) -> Option<String> {
        self.data.last().map(|value| format_end_value(*value))
    }

    /// The plot's pixel extents as `(x0, x1, y_top, y_bottom)`.
    ///
    /// Shared by the paint pass and the hover hit test, so the sample a pointer reports is the
    /// sample whose ink is under it.
    fn plot_extents(&self) -> (i32, i32, i32, i32) {
        let rect = self.base.geometry();
        let stroke_w = self.stroke_width as u32;
        let dot_radius =
            if self.show_last_point || self.show_points { SPARKLINE_DOT_RADIUS } else { 0 };
        let half_stroke = (stroke_w.max(1) as i32 + 1) / 2;
        let inset = half_stroke.max(dot_radius as i32) + SPARKLINE_PLOT_MARGIN;
        let inset = inset.min(rect.width as i32 / 2).min(rect.height as i32 / 2);
        (
            rect.x + inset,
            rect.x + rect.width as i32 - 1 - inset,
            rect.y + inset,
            rect.y + rect.height as i32 - 1 - inset,
        )
    }

    /// Nearest sample to `pos`, when the pointer is inside the plot.
    fn sample_at(&self, pos: Point) -> Option<usize> {
        if self.data.is_empty() {
            return None;
        }
        let (x0, x1, y_top, y_bottom) = self.plot_extents();
        if pos.x < x0 || pos.x > x1 || pos.y < y_top || pos.y > y_bottom {
            return None;
        }
        let n = self.data.len();
        if n == 1 {
            return Some(0);
        }
        let span = (x1 - x0).max(1);
        let index =
            ((pos.x - x0) as i64 * (n as i64 - 1) / span as i64).clamp(0, n as i64 - 1) as usize;
        Some(index)
    }

    /// Resolves the Y range: uses manual if set, otherwise auto-computes.
    fn resolve_y_range(&self) -> (f64, f64) {
        match (self.min, self.max) {
            (Some(min), Some(max)) => (min, max),
            _ => {
                let (min, max) = match (self.series_min, self.series_max) {
                    (Some(min), Some(max)) => (min, max),
                    _ => return (0.0, 1.0),
                };
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
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        crate::core::Size::new(200, 40)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();

    /// Resolves the sparkline's published events to its signals.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "value_hovered" => {
                Some(EventSignalRef::mapped("value_hovered", &self.value_hovered, |v| {
                    CapabilityValue::Tuple(crate::compat::Vec::from([
                        CapabilityValue::UInt(v.0 as u64),
                        CapabilityValue::Float(v.1),
                    ]))
                }))
            }
            "last_value_clicked" => {
                Some(EventSignalRef::mapped("last_value_clicked", &self.last_value_clicked, |v| {
                    CapabilityValue::Float(*v)
                }))
            }
            _ => None,
        }
    }
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
            "show_end_value" => Ok(CapabilityValue::Bool(self.show_end_value)),
            "fill" => Ok(CapabilityValue::Bool(self.fill)),
            "show_points" => Ok(CapabilityValue::Bool(self.show_points)),
            "show_last_marker" => Ok(CapabilityValue::Bool(self.show_last_marker())),
            "capacity" => Ok(CapabilityValue::UInt(self.capacity as u64)),
            // The series is a *property*, not only the `set_data` command, so a document can spell
            // the samples and a designer can read them back. It is a `NumberList`: order is the
            // value and samples are not identifiable rows.
            "data" => Ok(CapabilityValue::Tuple(
                self.data().iter().map(|value| CapabilityValue::Float(*value)).collect(),
            )),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "stroke_width" => {
                self.set_stroke_width(expect_f32(value)?);
                Ok(())
            }
            "show_end_value" => {
                let show = match value {
                    CapabilityValue::Bool(flag) => flag,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_show_end_value(show);
                Ok(())
            }
            "fill" => {
                self.set_fill(expect_bool(value)?);
                Ok(())
            }
            "show_points" => {
                self.set_show_points(expect_bool(value)?);
                Ok(())
            }
            "show_last_marker" => {
                self.set_show_last_marker(expect_bool(value)?);
                Ok(())
            }
            "capacity" => {
                self.set_capacity(expect_u32(value)? as usize);
                Ok(())
            }
            "data" => {
                self.set_data(expect_number_list(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "data",
            "stroke_width",
            "show_end_value",
            "fill",
            "show_points",
            "show_last_marker",
            "capacity",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl Draw for Sparkline {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.base.geometry();
        if rect.width == 0 || rect.height == 0 || self.data.len() < 2 {
            return;
        }

        let is_enabled = self.base.is_enabled();
        // Resolve both colours before any painting: `semantic_color` takes the theme lock
        // itself and returns an owned `Color`, so nothing holds a lock across the draw calls.
        let line_color = if is_enabled { self.line_color() } else { Color::DISABLED_FOREGROUND };

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
        let dot_radius =
            if self.show_last_point || self.show_points { SPARKLINE_DOT_RADIUS } else { 0 };
        let (x0, x1, y_top, y_bottom) = self.plot_extents();
        let plot_height = (y_bottom - y_top).max(1) as f64;
        // Pixel mapping is pure, so it is recomputed per segment instead of materialising a
        // `Vec<Point>` once per frame. The highlight dot maps the last sample directly.
        let map_at = |i: usize, val: f64| -> Point {
            let px = if n > 1 { x0 + (x1 - x0) * i as i32 / (n - 1) as i32 } else { x0 };
            let py = y_bottom - ((val - y_min) / (y_max - y_min) * plot_height) as i32;
            Point::new(px, py.max(y_top))
        };

        // ── Area fill ──
        //
        // One strip per segment down to the plot's baseline, the same construction the line chart
        // uses. Drawn before the trace so the line stays the strongest mark.
        if self.fill && is_enabled {
            let fill_color = line_color.with_alpha(40);
            for (i, pair) in self.data.windows(2).enumerate() {
                let from = map_at(i, pair[0]);
                let to = map_at(i + 1, pair[1]);
                let strip_width = (to.x - from.x).max(1) as u32;
                let top = from.y.min(to.y);
                let height = (y_bottom - top).max(1) as u32;
                context.fill_rect(Rect::new(from.x, top, strip_width, height), fill_color);
            }
        }

        // ── Draw line segments ──
        for (i, pair) in self.data.windows(2).enumerate() {
            context.draw_line_stroke_aa(
                map_at(i, pair[0]),
                map_at(i + 1, pair[1]),
                line_color,
                stroke_w.max(1),
            );
        }

        // ── Sample discs ──
        if self.show_points && is_enabled {
            let point_color = line_color;
            for (i, value) in self.data.iter().enumerate() {
                context.fill_circle(map_at(i, *value), dot_radius, point_color);
            }
        }

        // ── Hovered sample marker ──
        if let Some(index) = self.hovered_index {
            if is_enabled {
                if let Some(value) = self.data.get(index) {
                    context.fill_circle(
                        map_at(index, *value),
                        dot_radius + 1,
                        line_color.with_alpha(140),
                    );
                }
            }
        }

        // ── Draw last-point highlight dot ──
        if self.show_last_point {
            let dot_color =
                if is_enabled { self.last_point_color() } else { Color::DISABLED_FOREGROUND };
            let last = map_at(n - 1, self.data[n - 1]);
            context.fill_circle(last, dot_radius, dot_color);
        }

        // ── End-value annotation ──
        //
        // Drawn last so it sits above the trace, and anchored to the right edge of the
        // control. The line box is the measured font height, so the text never reaches past
        // the bottom of a short sparkline.
        if self.show_end_value && is_enabled {
            if let Some(text) = self.end_value_text() {
                let font = crate::widget::metrics::role_font(self.base.style(), 9.0);
                let metrics = context.measure_text(&text, &font);
                let width = metrics.width as i32;
                let height = metrics.height.max(1) as i32;
                let x = (rect.x + rect.width as i32 - width - 2).max(rect.x);
                let y = (rect.y + rect.height as i32 - height - 1).max(rect.y);
                let ink = if is_enabled { self.line_color() } else { Color::DISABLED_FOREGROUND };
                context.draw_text(
                    Point::new(x, y),
                    &text,
                    &font,
                    ink,
                    crate::core::HorizontalAlignment::Left,
                );
            }
        }
    }
}

/// Formats a sparkline's end value for its annotation.
///
/// Two decimals at most, with trailing zeros dropped: a sparkline is a compact reading, and
/// `1.2000000476837158` (the raw `f64`) is not one. Large magnitudes keep one decimal so the
/// text still fits a narrow control.
fn format_end_value(value: f64) -> String {
    if !value.is_finite() {
        return String::from("—");
    }
    if value.abs() >= 10_000.0 {
        return format!("{:.1}", value);
    }
    let rounded = (value * 100.0).round() / 100.0;
    if (rounded - rounded.round()).abs() < f64::EPSILON {
        format!("{}", rounded.round() as i64)
    } else {
        let mut text = format!("{rounded:.2}");
        while text.ends_with('0') {
            text.pop();
        }
        text
    }
}

impl EventHandler for Sparkline {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MouseMove { pos } => {
                let next = self.sample_at(*pos);
                if next != self.hovered_index {
                    self.hovered_index = next;
                    if let Some(index) = next {
                        if let Some(value) = self.data.get(index) {
                            self.value_hovered.emit((index, *value));
                        }
                    }
                    self.base.request_redraw();
                }
            }
            Event::MouseLeave { .. } => {
                if self.hovered_index.take().is_some() {
                    self.base.request_redraw();
                }
            }
            Event::MousePress { pos, button, .. } if *button == 1 => {
                if let (Some(last), Some(index)) = (self.data.last(), self.sample_at(*pos)) {
                    if index + 1 == self.data.len() {
                        self.last_value_clicked.emit(*last);
                    }
                }
            }
            _ => { /* Other events are not relevant */ }
        }
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
        assert_eq!(
            sl.line_color(),
            crate::style::semantic_color(crate::style::SemanticColor::Success)
                .unwrap_or(Color::SUCCESS),
            "the default trace must be the theme's success token"
        );
        assert!((sl.stroke_width() - 1.5).abs() < f32::EPSILON);
        assert!(sl.show_last_point());
    }

    /// An unset sparkline follows the theme's success token; an explicit colour wins.
    #[test]
    fn an_unset_sparkline_follows_the_success_token() {
        let sl = Sparkline::new(Rect::new(0, 0, 80, 24));
        assert_eq!(
            sl.line_color(),
            crate::style::semantic_color(crate::style::SemanticColor::Success)
                .unwrap_or(Color::SUCCESS),
            "the default trace must be the same derivation the theme defines"
        );
        let mut overridden = Sparkline::new(Rect::new(0, 0, 80, 24));
        overridden.set_color(Color::WARNING);
        assert_eq!(overridden.line_color(), Color::WARNING);
        overridden.set_color(Color::INFO);
        assert_eq!(overridden.line_color(), Color::INFO);
        assert_eq!(overridden.last_point_color(), Color::INFO);
    }

    /// The unset dot is derived from the resolved trace (darker), so it tracks the
    /// theme together with the line instead of carrying its own fixed literal.
    #[test]
    fn an_unset_dot_is_derived_from_the_resolved_trace() {
        let sl = Sparkline::new(Rect::new(0, 0, 80, 24));
        assert_eq!(sl.last_point_color(), sl.line_color().blend(&Color::BLACK, 0.3));
        let mut overridden = Sparkline::new(Rect::new(0, 0, 80, 24));
        overridden.set_last_point_color(Color::ERROR);
        assert_eq!(overridden.last_point_color(), Color::ERROR);
    }

    #[test]
    fn data_property_round_trips_the_samples() {
        let mut sl = Sparkline::new(Rect::new(0, 0, 80, 24));
        let written = CapabilityValue::Tuple(vec![
            CapabilityValue::Float(1.0),
            // An integer spelling is a legitimate way to write a sample.
            CapabilityValue::Int(2),
            CapabilityValue::Float(3.5),
        ]);
        sl.set("data", written).expect("data is writable");
        assert_eq!(sl.data(), &[1.0, 2.0, 3.5]);
        match sl.get("data").expect("data is readable") {
            CapabilityValue::Tuple(values) => {
                let numbers: Vec<f64> = values
                    .iter()
                    .map(|value| match value {
                        CapabilityValue::Float(number) => *number,
                        other => panic!("data elements should read back as Float, got {other:?}"),
                    })
                    .collect();
                assert_eq!(numbers, vec![1.0, 2.0, 3.5]);
            }
            other => panic!("data should read back as a Tuple, got {other:?}"),
        }
        // A non-numeric element refuses the whole list rather than dropping the sample.
        assert_eq!(
            sl.set(
                "data",
                CapabilityValue::Tuple(vec![
                    CapabilityValue::Float(1.0),
                    CapabilityValue::String("oops".to_string()),
                ])
            ),
            Err(CapabilityAccessError::TypeMismatch)
        );
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
        sl.handle_event(&Event::MousePress { pos: Point::new(5, 5), button: 1, modifiers: 0 });
    }

    /// N-CH-04: non-finite values are dropped at the data entry.
    #[test]
    fn non_finite_sparkline_values_are_dropped_at_entry() {
        let mut sl = Sparkline::new(Rect::new(0, 0, 80, 24));
        sl.set_data(vec![1.0, f64::NAN, 2.0, f64::INFINITY, 3.0]);
        assert_eq!(sl.data(), &[1.0, 2.0, 3.0], "only finite values survive");

        let before = sl.data().len();
        sl.add_value(f64::NAN);
        sl.add_value(f64::NEG_INFINITY);
        assert_eq!(sl.data().len(), before, "add_value must refuse non-finite input");
        sl.add_value(4.0);
        assert_eq!(sl.data().len(), before + 1);
    }

    #[test]
    fn the_end_value_annotation_is_opt_in() {
        let mut sl = Sparkline::new(Rect::new(0, 0, 80, 24));
        sl.set_data(vec![1.0, 2.5]);
        assert!(!sl.show_end_value());
        sl.set_show_end_value(true);
        assert_eq!(sl.end_value_text().as_deref(), Some("2.5"));
        let svg = render_to_svg(&mut sl);
        assert!(!crate::widget::svg::text_ink_boxes(&svg).is_empty(), "the value is painted");
        assert_eq!(format_end_value(3.0), "3");
        assert_eq!(format_end_value(12_500.0), "12500.0");
    }

    #[test]
    fn capacity_keeps_the_newest_samples_and_updates_the_range() {
        let mut sl = Sparkline::new(Rect::new(0, 0, 80, 24));
        sl.set_capacity(3);
        for value in [5.0, 1.0, 9.0, 2.0, 3.0] {
            sl.add_value(value);
        }
        assert_eq!(sl.data(), &[9.0, 2.0, 3.0]);
        assert_eq!(sl.capacity(), 3);
        sl.set_capacity(0);
        sl.add_value(4.0);
        assert_eq!(sl.data().len(), 4, "zero clears the cap");
    }

    #[test]
    fn fill_points_and_last_marker_are_painted_when_enabled() {
        let mut sl = Sparkline::new(Rect::new(0, 0, 80, 24));
        sl.set_data(vec![1.0, 3.0, 2.0]);
        let plain = render_to_svg(&mut sl);
        sl.set_fill(true);
        sl.set_show_points(true);
        let richer = render_to_svg(&mut sl);
        assert!(richer.len() > plain.len(), "fill and point marks add ink");
        assert!(sl.fill());
        assert!(sl.show_points());
        assert!(sl.show_last_marker());
    }

    #[test]
    fn pointer_events_resolve_and_emit() {
        use crate::event::{Event, EventHandler};
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        let mut sl = Sparkline::new(Rect::new(0, 0, 80, 24));
        sl.set_data(vec![1.0, 2.0, 3.0]);
        let _ = render_to_svg(&mut sl);
        assert!(sl.event_signal_dyn("value_hovered").is_some());
        assert!(sl.event_signal_dyn("last_value_clicked").is_some());
        let hit = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&hit);
        sl.last_value_clicked.connect(move |_| flag.store(true, Ordering::SeqCst));
        let (_, x1, _, _) = sl.plot_extents();
        sl.handle_event(&Event::MousePress { pos: Point::new(x1, 12), button: 1, modifiers: 0 });
        assert!(hit.load(Ordering::SeqCst), "a click on the final sample reports it");
    }
}
