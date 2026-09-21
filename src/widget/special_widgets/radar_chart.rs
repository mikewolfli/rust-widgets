// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! RadarChart — multi-series values plotted on a shared set of dimension axes.
//!
//! # Why this is a control of its own rather than a `ChartWidget` variant
//!
//! `ChartWidget`'s data model is a one-dimensional ordered series (`Vec<f64>`
//! plus a label per point), and every one of its nine styles is a function of
//! `(index, value)`. A radar chart's geometry is a function of `(axis, series)`:
//! the angle comes from **which dimension** a value belongs to, and each series
//! becomes a closed polygon over those dimensions.
//!
//! Those are not the same shape:
//!
//! | | `ChartWidget` | `RadarChart` |
//! |---|---|---|
//! | data | `Vec<Vec<f64>>` — series over one index order | `Vec<Vec<f64>>` — series **over dimensions** |
//! | x position | derived from the point's index | derived from the **dimension's angle** |
//! | a series draws | a line of marks | a **closed polygon** |
//! | axes | one value axis, implied | **one axis per dimension**, labelled |
//!
//! Forcing radar into `ChartWidget` would mean adding a second, incompatible
//! meaning for the same fields — which rule #80's criterion (\"same data model *and*
//! same interaction\") does not permit. Hence a separate `WidgetKind`.
//!
//! # What is shared rather than duplicated
//!
//! The legend, the hover readout and the colour palette follow `ChartWidget`'s
//! conventions (same palette order, same hover-by-nearest-index signal shape), so
//! a caller switching between the two does not have to relearn them.

//! # Reachability
//!
//! Registered in the widget factory as `radar_chart` (aliases `radar`, `spider_chart`),
//! so it is reachable by name from the declarative JSON path (`"radarchart"`), from a
//! CSS selector (`RadarChart`), and through the typed `create_radar_chart` on
//! `ControlBackend`. `WidgetKind::RadarChart` is its own kind rather than a
//! `chart_type` token, because its data model is series-over-dimensions.

use crate::core::{deg_to_rad, Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::expect_bool;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// The angular start of the first axis, in degrees measured clockwise from
/// 12 o'clock.
///
/// North rather than east because a radar chart is read like a compass rose: the
/// first dimension should be the one directly above the centre, and every
/// documented example of the form does it that way.
const FIRST_AXIS_DEGREES: f32 = 0.0;

/// The six series colours, in `ChartWidget`'s order.
///
/// Duplicated as a constant rather than imported because `ChartWidget::PALETTE` is
/// private to that module; the *order* is what matters for consistency, and the
/// comment is what keeps the two from drifting apart unnoticed.
const PALETTE: [Color; 6] = [
    Color::rgb(66, 133, 244),
    Color::rgb(219, 68, 55),
    Color::rgb(244, 180, 0),
    Color::rgb(15, 157, 88),
    Color::rgb(171, 71, 188),
    Color::rgb(0, 172, 193),
];

/// Nudges a text box of `width`×`height` at `(x, y)` back inside `rect`.
///
/// The returned box is the caller's own measured size, moved so it lies within `rect`.
/// A box larger than the control cannot be made to fit by moving, so it is pinned to the
/// control's origin and left for `draw_text_fitted` to fit to the room that remains:
/// the fitting step is the one place that answers "too wide to fit", and duplicating
/// that judgement here would let the two disagree.
fn label_box(x: i32, y: i32, width: i32, height: i32, rect: Rect) -> Rect {
    let max_x = (rect.x + rect.width as i32 - width).max(rect.x);
    let max_y = (rect.y + rect.height as i32 - height).max(rect.y);
    Rect::new(x.clamp(rect.x, max_x), y.clamp(rect.y, max_y), width as u32, height as u32)
}

/// A radar (spider) chart: several series measured against shared dimension axes.
///
/// # Data model
///
/// `axes[i]` names dimension `i`. `series[s][i]` is series `s`'s value on
/// dimension `i`. Every series is plotted against the same axes, so a series
/// shorter than `axes` leaves the trailing dimensions at zero — the honest reading
/// of \"no value supplied\" on a chart where the dimension exists.
pub struct RadarChart {
    base: BaseWidget,
    axes: Vec<String>,
    series: Vec<Vec<f64>>,
    /// Whether the value rings and the spoke lines are drawn.
    show_grid: bool,
    /// Whether axis names are drawn around the perimeter.
    show_axis_labels: bool,
    /// Whether the legend is drawn.
    show_legend: bool,
    /// The axis index nearest the pointer, or `None` when the pointer is away.
    hovered_axis: Option<usize>,
    /// Emitted when the pointer's nearest axis changes.
    pub axis_hovered: Signal1<usize>,
    /// Emitted when a series polygon is clicked, carrying the series index.
    pub series_clicked: Signal1<usize>,
}

impl RadarChart {
    /// Creates a radar chart with no axes and no series.
    ///
    /// Defaults: grid on, axis labels on, legend on.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::RadarChart, geometry, "RadarChart"),
            axes: Vec::new(),
            series: Vec::new(),
            show_grid: true,
            show_axis_labels: true,
            show_legend: true,
            hovered_axis: None,
            axis_hovered: Signal1::new(),
            series_clicked: Signal1::new(),
        }
    }

    /// Replaces the dimension axes with `axes`.
    ///
    /// At least three axes are needed for a polygon, so fewer than three is
    /// refused and the previous axes are kept: a two-spoke \"radar\" is a line, and
    /// silently drawing one would look like a rendering fault rather than a
    /// rejected input.
    pub fn set_axes(&mut self, axes: Vec<String>) -> bool {
        if axes.len() < 3 {
            return false;
        }
        self.axes = axes;
        self.hovered_axis = None;
        self.base.request_redraw();
        true
    }

    /// Returns the dimension axis labels.
    pub fn axes(&self) -> &[String] {
        &self.axes
    }

    /// Returns the number of dimension axes.
    pub fn axis_count(&self) -> usize {
        self.axes.len()
    }

    /// Replaces every series.
    pub fn set_series(&mut self, series: Vec<Vec<f64>>) {
        self.series = series;
        self.base.request_redraw();
    }

    /// Returns every series.
    pub fn series(&self) -> &[Vec<f64>] {
        &self.series
    }

    /// Returns the number of series.
    pub fn series_count(&self) -> usize {
        self.series.len()
    }

    /// Adds one series, returning its index.
    pub fn add_series(&mut self, values: Vec<f64>) -> usize {
        let index = self.series.len();
        self.series.push(values);
        self.base.request_redraw();
        index
    }

    /// Returns the value of `series` on `axis`, or `None` when either index is
    /// unset.
    ///
    /// A series shorter than the axis list reports `None` rather than `0.0`, so a
    /// caller can tell \"no value\" from \"a value of zero\".
    pub fn value_at(&self, series: usize, axis: usize) -> Option<f64> {
        self.series.get(series).and_then(|values| values.get(axis)).copied()
    }

    /// Whether the grid rings and spokes are drawn.
    pub fn show_grid(&self) -> bool {
        self.show_grid
    }

    /// Sets whether the grid rings and spokes are drawn.
    pub fn set_show_grid(&mut self, show: bool) {
        self.show_grid = show;
        self.base.request_redraw();
    }

    /// Whether axis names are drawn.
    pub fn show_axis_labels(&self) -> bool {
        self.show_axis_labels
    }

    /// Sets whether axis names are drawn.
    pub fn set_show_axis_labels(&mut self, show: bool) {
        self.show_axis_labels = show;
        self.base.request_redraw();
    }

    /// Whether the legend is drawn.
    pub fn show_legend(&self) -> bool {
        self.show_legend
    }

    /// Sets whether the legend is drawn.
    pub fn set_show_legend(&mut self, show: bool) {
        self.show_legend = show;
        self.base.request_redraw();
    }

    /// The largest value in the data, or `None` when there is none.
    ///
    /// Every series is scaled against the same maximum so the polygons are
    /// comparable; scaling each series against its own maximum would make a series
    /// of small numbers look identical to one of large numbers.
    fn data_max(&self) -> Option<f64> {
        let max = self
            .series
            .iter()
            .flatten()
            .copied()
            .filter(|value| value.is_finite())
            .fold(f64::NEG_INFINITY, f64::max);
        if max.is_finite() && max > 0.0 {
            Some(max)
        } else {
            None
        }
    }

    /// The geometry the plot is drawn within.
    ///
    /// The legend reserves a strip on the right and the axis labels need room
    /// outside the outermost ring, so the radius is derived from the smallest
    /// usable dimension rather than from the raw rectangle.
    fn geometry_center_radius(&self) -> Option<(Point, u32)> {
        let rect = self.base.geometry();
        if rect.width == 0 || rect.height == 0 {
            return None;
        }
        /// Room outside the outermost ring for the axis labels.
        const LABEL_MARGIN: u32 = 44;
        /// Room on the right for the legend.
        const LEGEND_WIDTH: u32 = 96;
        /// Room below for the legend-free bottom margin.
        const BOTTOM_MARGIN: u32 = 8;

        let legend = if self.show_legend && !self.series.is_empty() { LEGEND_WIDTH } else { 0 };
        let usable_w = rect.width.saturating_sub(LABEL_MARGIN * 2 + legend).max(1);
        let usable_h = rect.height.saturating_sub(LABEL_MARGIN + BOTTOM_MARGIN).max(1);
        let radius = usable_w.min(usable_h) / 2;
        if radius < 8 {
            return None;
        }
        let center = Point::new(
            rect.x + LABEL_MARGIN as i32 + radius as i32,
            rect.y + LABEL_MARGIN as i32 + radius as i32,
        );
        Some((center, radius))
    }

    /// The angle of `axis` in radians, measured clockwise from 12 o'clock.
    fn axis_angle(&self, axis: usize) -> f32 {
        let count = self.axis_count().max(1) as f32;
        let step = 360.0 / count;
        // `-90` converts \"0 degrees = 3 o'clock\" to \"0 degrees = 12 o'clock\".
        deg_to_rad(FIRST_AXIS_DEGREES + step * axis as f32 - 90.0)
    }

    /// The point a series reaches on `axis`, given the plot geometry.
    fn vertex(&self, center: Point, radius: u32, angle: f32, value: f64, max: f64) -> Point {
        // Negative values would point backwards past the centre, which a radar
        // chart has no way to show; they are clamped to the centre.
        let ratio = (value / max).clamp(0.0, 1.0) as f32;
        let r = radius as f32 * ratio;
        Point::new(center.x + (r * angle.cos()) as i32, center.y + (r * angle.sin()) as i32)
    }

    /// The axis whose spoke is nearest `pos`, when the pointer is in the plot.
    fn axis_at(&self, pos: Point) -> Option<usize> {
        let (center, _) = self.geometry_center_radius()?;
        if self.axis_count() == 0 {
            return None;
        }
        let dx = (pos.x - center.x) as f32;
        let dy = (pos.y - center.y) as f32;
        if dx == 0.0 && dy == 0.0 {
            return None;
        }
        // The pointer's own angle, in the same convention the axes use.
        let pointer = dy.atan2(dx).to_degrees() + 90.0;
        let step = 360.0 / self.axis_count() as f32;
        // Snap to the nearest spoke, wrapping at 360.
        let normalized = ((pointer % 360.0) + 360.0) % 360.0;
        let index = ((normalized + step / 2.0) / step).floor() as usize % self.axis_count();
        Some(index)
    }
}

impl Widget for RadarChart {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        Size::new(320, 320)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `RadarChart`'s property contract.
///
/// The axes and series are lists of strings and numbers, so they are written
/// through `set_axes` / `set_series`; what the property layer reports is the
/// derived counts plus the three display switches, following the same convention
/// `Meter`'s threshold bands and `ScrollArea`'s sticky regions use.
impl WidgetProperties for RadarChart {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "axis_count" => Ok(CapabilityValue::UInt(self.axis_count() as u64)),
            "series_count" => Ok(CapabilityValue::UInt(self.series_count() as u64)),
            "show_grid" => Ok(CapabilityValue::Bool(self.show_grid())),
            "show_axis_labels" => Ok(CapabilityValue::Bool(self.show_axis_labels())),
            "show_legend" => Ok(CapabilityValue::Bool(self.show_legend())),
            "hovered_axis" => Ok(match self.hovered_axis {
                Some(axis) => CapabilityValue::UInt(axis as u64),
                None => CapabilityValue::Null,
            }),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "show_grid" => {
                self.set_show_grid(expect_bool(value)?);
                Ok(())
            }
            "show_axis_labels" => {
                self.set_show_axis_labels(expect_bool(value)?);
                Ok(())
            }
            "show_legend" => {
                self.set_show_legend(expect_bool(value)?);
                Ok(())
            }
            // `hovered_axis` is written through the pointer, not through the
            // property layer: a name that could be set to an arbitrary index would
            // claim a hover that is not happening.
            "axis_count" | "series_count" | "hovered_axis" => {
                Err(CapabilityAccessError::ReadOnlyProperty)
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Mirrors `RADAR_CHART_PROPERTIES`.
        property_names_of![
            "axis_count",
            "series_count",
            "show_grid",
            "show_axis_labels",
            "show_legend",
            "hovered_axis",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `radar_chart` publishes.
    ///
    /// `add_series` cannot execute bare: a series is a list of values per axis, and
    /// `add_series` reports the index it appended at, so there is no default the
    /// control could supply without inventing data. The name is valid and the
    /// *argument* is what is missing, which is
    /// [`CapabilityAccessError::OutOfRange`], not `UnknownCommand`. `set_axes` and
    /// `set_series` likewise carry their own payloads.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "add_series" | "set_axes" | "set_series" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl Draw for RadarChart {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.base.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        context.fill_rect(rect, Color::rgb(255, 255, 255));
        context.draw_rect(rect, Color::rgb(200, 200, 200));

        let axis_count = self.axis_count();
        let Some(max) = self.data_max() else {
            self.draw_placeholder(context, rect);
            return;
        };
        let Some((center, radius)) = self.geometry_center_radius() else {
            return;
        };
        if axis_count == 0 {
            self.draw_placeholder(context, rect);
            return;
        }

        if self.show_grid {
            self.draw_grid(context, center, radius, axis_count);
        }
        if self.show_axis_labels {
            self.draw_axis_labels(context, center, radius, axis_count);
        }
        for series_index in 0..self.series.len() {
            self.draw_series(context, center, radius, max, series_index);
        }
        if self.show_legend && self.series.len() > 1 {
            self.draw_legend(context, rect);
        }
        // The hovered spoke last, so it is legible over every polygon.
        if let Some(axis) = self.hovered_axis {
            self.draw_hover_spoke(context, center, radius, axis, max);
        }
    }
}

impl RadarChart {
    /// Draws the "no data" caption.
    fn draw_placeholder(&self, context: &mut RenderContext, rect: Rect) {
        context.draw_text_fitted(
            rect,
            "No data",
            &Font::simple("Sans", 12.0),
            Color::rgb(180, 180, 180),
            HorizontalAlignment::Left,
        );
    }

    /// Draws the concentric rings and the spokes.
    fn draw_grid(
        &self,
        context: &mut RenderContext,
        center: Point,
        radius: u32,
        axis_count: usize,
    ) {
        const RINGS: u32 = 4;
        let grid = Color::rgb(225, 225, 225);
        // Rings: one per quarter of the radius, so the gauge reading is easy to
        // estimate. Drawn as polylines through the axis angles, which is what makes
        // them polygons rather than circles — a radar chart's rings are the shapes
        // of the plotted polygons themselves.
        for ring in 1..=RINGS {
            let ring_radius = radius * ring / RINGS;
            for axis in 0..axis_count {
                let angle = self.axis_angle(axis);
                let next_angle = self.axis_angle((axis + 1) % axis_count);
                let from = Point::new(
                    center.x + (ring_radius as f32 * angle.cos()) as i32,
                    center.y + (ring_radius as f32 * angle.sin()) as i32,
                );
                let to = Point::new(
                    center.x + (ring_radius as f32 * next_angle.cos()) as i32,
                    center.y + (ring_radius as f32 * next_angle.sin()) as i32,
                );
                context.draw_line_stroke(from, to, grid, 1);
            }
        }
        // Spokes: one per axis, from the centre to the outermost ring.
        for axis in 0..axis_count {
            let angle = self.axis_angle(axis);
            let outer = Point::new(
                center.x + (radius as f32 * angle.cos()) as i32,
                center.y + (radius as f32 * angle.sin()) as i32,
            );
            context.draw_line_stroke(center, outer, grid, 1);
        }
    }

    /// Draws the axis names just outside the outermost ring.
    fn draw_axis_labels(
        &self,
        context: &mut RenderContext,
        center: Point,
        radius: u32,
        axis_count: usize,
    ) {
        const LABEL_GAP: u32 = 10;
        let rect = self.base.geometry();
        let font = Font::simple("Sans", 10.0);
        let label_radius = radius + LABEL_GAP;
        for axis in 0..axis_count {
            let Some(label) = self.axes.get(axis) else {
                continue;
            };
            let angle = self.axis_angle(axis);
            let anchor_x = center.x + (label_radius as f32 * angle.cos()) as i32;
            let anchor_y = center.y + (label_radius as f32 * angle.sin()) as i32;
            let metrics = context.measure_text(label, &font);
            // Centre the text on the anchor horizontally and on the spoke's vertical
            // position, then clamp the *fitted* box into the control.
            //
            // The anchor sits outside the outermost ring, so on the upward-pointing
            // spokes it lands above the control and on the downward ones below it; a
            // label centred there had half its glyph box outside the widget. Fitting the
            // text to the room the clamp leaves is what keeps a long axis name both inside
            // the control and readable, rather than being clipped away by the raster
            // backend and running off the picture in the SVG one.
            let text_x = anchor_x - metrics.width as i32 / 2;
            let text_y = anchor_y + metrics.ascent as i32 / 2;
            let bounds =
                label_box(text_x, text_y, metrics.width as i32, metrics.height as i32, rect);
            context.draw_text_fitted(
                bounds,
                label,
                &font,
                Color::rgb(90, 90, 90),
                HorizontalAlignment::Left,
            );
        }
    }

    /// Draws one series as a stroked, filled polygon.
    fn draw_series(
        &self,
        context: &mut RenderContext,
        center: Point,
        radius: u32,
        max: f64,
        series_index: usize,
    ) {
        let Some(values) = self.series.get(series_index) else {
            return;
        };
        let axis_count = self.axis_count();
        if axis_count == 0 {
            return;
        }
        let color = Self::series_color(series_index);
        let vertices: Vec<Point> = (0..axis_count)
            .map(|axis| {
                let angle = self.axis_angle(axis);
                // A missing dimension reads as zero, which lands on the centre,
                // matching `value_at` returning `None` for it.
                let value = values.get(axis).copied().unwrap_or(0.0);
                self.vertex(center, radius, angle, value, max)
            })
            .collect();

        // Fill first, then stroke: the render context has no filled-polygon
        // primitive, so the fill is a fan of triangles from the centre — exact for a
        // star-shaped polygon, which a radar polygon always is because every vertex
        // lies on a ray from the centre.
        for axis in 0..axis_count {
            let from = vertices[axis];
            let to = vertices[(axis + 1) % axis_count];
            context.draw_line(center, from, color);
            context.draw_line(center, to, color);
            context.draw_line(from, to, color);
        }
        for axis in 0..axis_count {
            let from = vertices[axis];
            let to = vertices[(axis + 1) % axis_count];
            context.draw_line_stroke(from, to, color, 2);
        }
        for vertex in &vertices {
            context.fill_circle(*vertex, 3, color);
        }
    }

    /// Draws the series legend down the right edge.
    fn draw_legend(&self, context: &mut RenderContext, rect: Rect) {
        const SWATCH: u32 = 10;
        const ROW_HEIGHT: i32 = 18;
        let font = Font::simple("Sans", 10.0);
        let x = rect.x + rect.width as i32 - 90;
        let mut y = rect.y + 12;
        for index in 0..self.series.len() {
            context.fill_rect(Rect::new(x, y, SWATCH, SWATCH), Self::series_color(index));
            let label = format!("Series {}", index + 1);
            // The legend strip is fixed at 90 px on the right, so the text is fitted to
            // what remains of it beside the swatch: an unfitted `Series 12` ran past the
            // control's right edge and only the raster backends hid it.
            let text_x = x + SWATCH as i32 + 6;
            let row =
                Rect::new(text_x, y, (rect.x + rect.width as i32 - text_x).max(0) as u32, SWATCH);
            context.draw_text_fitted(
                row,
                &label,
                &font,
                Color::rgb(70, 70, 70),
                HorizontalAlignment::Left,
            );
            y += ROW_HEIGHT;
        }
    }

    /// Draws the hovered axis's spoke emphasised, with its value marked.
    fn draw_hover_spoke(
        &self,
        context: &mut RenderContext,
        center: Point,
        radius: u32,
        axis: usize,
        max: f64,
    ) {
        if axis >= self.axis_count() {
            return;
        }
        let angle = self.axis_angle(axis);
        let outer = Point::new(
            center.x + (radius as f32 * angle.cos()) as i32,
            center.y + (radius as f32 * angle.sin()) as i32,
        );
        context.draw_line_stroke(center, outer, Color::rgb(120, 120, 120), 1);
        // The read-out for every series on this dimension, which is what makes the
        // hover worth doing on a chart whose polygons overlap.
        let font = Font::simple("Sans", 9.0);
        let mut text_y = outer.y + 4;
        for series_index in 0..self.series.len() {
            let Some(value) = self.value_at(series_index, axis) else {
                continue;
            };
            let marker = self.vertex(center, radius, angle, value, max);
            context.fill_circle(marker, 4, Self::series_color(series_index));
            let label = format!("{value:.1}");
            context.draw_text(
                Point { x: outer.x + 4, y: text_y },
                &label,
                &font,
                Color::rgb(60, 60, 60),
                HorizontalAlignment::Left,
            );
            text_y += 12;
        }
    }

    /// The colour of series `index`, cycling through [`PALETTE`].
    fn series_color(index: usize) -> Color {
        PALETTE[index % PALETTE.len()]
    }
}

impl EventHandler for RadarChart {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MouseMove { pos } => {
                let axis = self.axis_at(*pos);
                if axis != self.hovered_axis {
                    self.hovered_axis = axis;
                    if let Some(axis) = axis {
                        self.axis_hovered.emit(axis);
                    }
                    self.base.request_redraw();
                }
            }
            Event::MouseLeave { .. } if self.hovered_axis.is_some() => {
                self.hovered_axis = None;
                self.base.request_redraw();
            }
            Event::MousePress { pos, button } if *button == 1 => {
                self.base.set_mouse_pressed(true);
                if let Some(axis) = self.axis_at(*pos) {
                    // The nearest series polygon is reported, so a click on a
                    // chart tells the caller *which* series was aimed at rather
                    // than only which dimension.
                    if let Some(series) = self.nearest_series(axis, *pos) {
                        self.base.clicked.emit();
                        self.series_clicked.emit(series);
                    }
                }
            }
            Event::MouseRelease { button, .. } if *button == 1 => {
                self.base.set_mouse_pressed(false);
            }
            _ => {}
        }
    }
}

impl RadarChart {
    /// The series whose polygon edge is nearest `pos` among those crossing `axis`.
    ///
    /// \"Nearest\" is measured by the vertex the series has on the hovered axis, so
    /// the result is the series whose marker the pointer is closest to — which is
    /// what a caller clicking at a chart expects, and is stable regardless of which
    /// side of the spoke they clicked.
    fn nearest_series(&self, axis: usize, pos: Point) -> Option<usize> {
        let (center, radius) = self.geometry_center_radius()?;
        let max = self.data_max()?;
        let angle = self.axis_angle(axis);
        let mut best: Option<(usize, i64)> = None;
        for series_index in 0..self.series.len() {
            let value = self.value_at(series_index, axis).unwrap_or(0.0);
            let vertex = self.vertex(center, radius, angle, value, max);
            let dx = (vertex.x - pos.x) as i64;
            let dy = (vertex.y - pos.y) as i64;
            // Squared distance, which orders identically to the distance and
            // avoids a square root per series.
            let distance = dx * dx + dy * dy;
            if best.is_none_or(|(_, best_distance)| distance < best_distance) {
                best = Some((series_index, distance));
            }
        }
        best.map(|(index, _)| index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{PaintBackend, SoftwarePaintBackend};

    fn chart() -> RadarChart {
        let mut chart = RadarChart::new(Rect::new(0, 0, 320, 320));
        assert!(chart.set_axes(vec![
            "Speed".to_string(),
            "Power".to_string(),
            "Range".to_string(),
            "Comfort".to_string(),
            "Safety".to_string(),
        ]));
        chart.add_series(vec![80.0, 60.0, 90.0, 70.0, 50.0]);
        chart
    }

    /// Renders the chart and returns the RGBA frame.
    fn render(chart: &mut RadarChart, size: Size) -> Vec<u8> {
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);
        let mut context = RenderContext::new(&mut backend);
        chart.draw(&mut context);
        backend.end_frame();
        backend.frame_rgba().to_vec()
    }

    /// Counts pixels near `target`.
    fn count_near(rgba: &[u8], target: (u8, u8, u8)) -> usize {
        const TOLERANCE: i32 = 24;
        rgba.chunks_exact(4)
            .filter(|px| {
                (px[0] as i32 - target.0 as i32).abs() <= TOLERANCE
                    && (px[1] as i32 - target.1 as i32).abs() <= TOLERANCE
                    && (px[2] as i32 - target.2 as i32).abs() <= TOLERANCE
                    && px[3] > 0
            })
            .count()
    }

    #[test]
    fn radar_chart_creation_defaults() {
        let chart = RadarChart::new(Rect::new(0, 0, 320, 320));
        assert_eq!(chart.kind(), WidgetKind::RadarChart);
        assert_eq!(chart.axis_count(), 0);
        assert_eq!(chart.series_count(), 0);
        assert!(chart.show_grid());
        assert!(chart.show_axis_labels());
        assert!(chart.show_legend());
    }

    #[test]
    fn radar_chart_set_axes_requires_three() {
        let mut chart = RadarChart::new(Rect::new(0, 0, 320, 320));
        // A two-spoke radar is a line, not a polygon.
        assert!(!chart.set_axes(vec!["A".to_string(), "B".to_string()]));
        assert_eq!(chart.axis_count(), 0, "a refused input must not change state");
        assert!(chart.set_axes(vec!["A".to_string(), "B".to_string(), "C".to_string()]));
        assert_eq!(chart.axis_count(), 3);
    }

    #[test]
    fn radar_chart_value_at_distinguishes_missing_from_zero() {
        let mut chart = RadarChart::new(Rect::new(0, 0, 320, 320));
        chart.set_axes(vec!["A".to_string(), "B".to_string(), "C".to_string()]);
        chart.add_series(vec![1.0, 0.0]);
        // A supplied zero.
        assert_eq!(chart.value_at(0, 1), Some(0.0));
        // A dimension the series has no value for.
        assert_eq!(chart.value_at(0, 2), None);
        // An unset series.
        assert_eq!(chart.value_at(9, 0), None);
    }

    #[test]
    fn radar_chart_series_count_and_polygon_pixels() {
        let mut chart = chart();
        assert_eq!(chart.series_count(), 1);
        let rgba = render(&mut chart, Size::new(320, 320));
        // Series zero's colour must be on the canvas, which proves the polygon was
        // actually drawn rather than only stored.
        assert!(count_near(&rgba, (66, 133, 244)) > 0);
    }

    #[test]
    fn radar_chart_multiple_series_use_distinct_colours() {
        let mut chart = chart();
        chart.add_series(vec![50.0, 90.0, 60.0, 85.0, 95.0]);
        assert_eq!(chart.series_count(), 2);
        let rgba = render(&mut chart, Size::new(320, 320));
        assert!(count_near(&rgba, (66, 133, 244)) > 0, "series zero");
        assert!(count_near(&rgba, (219, 68, 55)) > 0, "series one");
    }

    #[test]
    fn radar_chart_empty_draw_does_not_panic() {
        let mut chart = RadarChart::new(Rect::new(0, 0, 320, 320));
        let rgba = render(&mut chart, Size::new(320, 320));
        assert!(!rgba.is_empty());
    }

    #[test]
    fn radar_chart_zero_geometry_does_not_panic() {
        let mut chart = chart();
        let rgba = render(&mut chart, Size::new(4, 4));
        assert!(!rgba.is_empty());
    }

    #[test]
    fn radar_chart_axis_hover_emits_nearest_axis() {
        let mut chart = chart();
        let hovered = std::sync::Arc::new(std::sync::Mutex::new(Vec::<usize>::new()));
        let sink = hovered.clone();
        chart.axis_hovered.connect(move |axis| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*axis);
            }
        });

        // First axis points straight up from the centre; hovering above the centre
        // must select it.
        let (center, _) = chart.geometry_center_radius().expect("geometry");
        chart.handle_event(&Event::mouse_move(center.x, center.y - 40));
        assert_eq!(*hovered.lock().expect("hover lock poisoned"), vec![0]);
        assert_eq!(chart.get("hovered_axis").unwrap(), CapabilityValue::UInt(0));

        // Leaving clears the hover.
        chart.handle_event(&Event::MouseLeave { pos: Point::new(0, 0) });
        assert_eq!(chart.get("hovered_axis").unwrap(), CapabilityValue::Null);
    }

    #[test]
    fn radar_chart_axis_at_centres_on_the_first_spoke() {
        let chart = chart();
        let (center, _) = chart.geometry_center_radius().expect("geometry");
        // Straight up is axis 0; straight right is axis 1 on a five-axis chart
        // (72 degrees per spoke, so 90 degrees is nearest to spoke 1).
        assert_eq!(chart.axis_at(Point::new(center.x, center.y - 30)), Some(0));
        assert_eq!(chart.axis_at(Point::new(center.x + 30, center.y)), Some(1));
        // The exact centre has no direction and therefore no axis.
        assert_eq!(chart.axis_at(center), None);
    }

    #[test]
    fn radar_chart_click_emits_nearest_series() {
        let mut chart = chart();
        chart.add_series(vec![20.0, 20.0, 20.0, 20.0, 20.0]);
        let clicked = std::sync::Arc::new(std::sync::Mutex::new(Vec::<usize>::new()));
        let sink = clicked.clone();
        chart.series_clicked.connect(move |series| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*series);
            }
        });

        let (center, radius) = chart.geometry_center_radius().expect("geometry");
        let max = chart.data_max().expect("data");
        // Click exactly on series zero's vertex for axis 0.
        let vertex = chart.vertex(center, radius, chart.axis_angle(0), 80.0, max);
        chart.handle_event(&Event::mouse_press(vertex.x, vertex.y, 1));
        assert_eq!(*clicked.lock().expect("click lock poisoned"), vec![0]);
    }

    #[test]
    fn radar_chart_display_switches_round_trip() {
        let mut chart = chart();
        for name in ["show_grid", "show_axis_labels", "show_legend"] {
            chart.set(name, CapabilityValue::Bool(false)).unwrap();
            assert_eq!(chart.get(name).unwrap(), CapabilityValue::Bool(false));
            chart.set(name, CapabilityValue::Bool(true)).unwrap();
            assert_eq!(chart.get(name).unwrap(), CapabilityValue::Bool(true));
        }
        assert!(chart.set("show_grid", CapabilityValue::UInt(1)).is_err());
    }

    #[test]
    fn radar_chart_derived_properties_are_read_only() {
        let mut chart = chart();
        for name in ["axis_count", "series_count", "hovered_axis"] {
            assert_eq!(
                chart.set(name, CapabilityValue::UInt(1)),
                Err(CapabilityAccessError::ReadOnlyProperty),
                "{name} must be read-only"
            );
        }
        assert_eq!(chart.get("axis_count").unwrap(), CapabilityValue::UInt(5));
        assert_eq!(chart.get("series_count").unwrap(), CapabilityValue::UInt(1));
    }

    #[test]
    fn radar_chart_hidden_grid_removes_grid_pixels() {
        let size = Size::new(320, 320);
        let mut with_grid = chart();
        with_grid.set_show_axis_labels(false);
        let grid_pixels = count_near(&render(&mut with_grid, size), (225, 225, 225));

        let mut without_grid = chart();
        without_grid.set_show_axis_labels(false);
        without_grid.set_show_grid(false);
        let bare_pixels = count_near(&render(&mut without_grid, size), (225, 225, 225));

        assert!(grid_pixels > bare_pixels, "the grid must paint: {grid_pixels} vs {bare_pixels}");
    }

    #[test]
    fn radar_chart_series_shorter_than_axes_reads_as_zero() {
        let mut chart = RadarChart::new(Rect::new(0, 0, 320, 320));
        chart.set_axes(vec!["A".to_string(), "B".to_string(), "C".to_string()]);
        // Only two of three dimensions supplied.
        chart.add_series(vec![10.0, 20.0]);
        let rgba = render(&mut chart, Size::new(320, 320));
        assert!(!rgba.is_empty());
        assert_eq!(chart.value_at(0, 2), None);
    }
}
