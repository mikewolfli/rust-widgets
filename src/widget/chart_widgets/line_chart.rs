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

// The fallback backdrop is the only `write!` user; cfg'd to match it.
#[cfg(not(feature = "chart"))]
use core::fmt::Write as _;

#[cfg(feature = "chart")]
use crate::core::Font;
use crate::core::{Color, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::style::{MotionSlot, PropertyDriver};
use crate::widget::capability::coercion::expect_f32;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
#[cfg(feature = "chart")]
use crate::widget::chart_widgets::adapter::ChartContextAdapter;
#[cfg(feature = "chart")]
use crate::widget::chart_widgets::charts::{
    compute_cartesian_layout_measured, draw_cartesian_axes, draw_legend_entries_iter, draw_x_ticks,
    draw_x_ticks_configured, draw_y_ticks, draw_y_ticks_configured, CartesianLayout,
};
use crate::widget::chart_widgets::charts::{AxisConfig, AxisScale};
use crate::widget::chart_widgets::{finite_samples, is_finite_sample};
#[cfg(not(feature = "chart"))]
use crate::widget::metrics::role_font;
// Shared with the engine-backed path so the `not(feature = "chart")` fallback cannot
// reintroduce the light-chart literals the engine path moved off.
#[cfg(not(feature = "chart"))]
use crate::widget::chart_widgets::charts::axis_chrome;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

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

/// One named line in a [`LineChart`].
///
/// Points are `(x, y)` pairs in data space. A series without a name is not shown
/// in the legend; an explicit `color` overrides the chart's palette entry.
#[derive(Clone, Debug)]
pub struct LineSeries {
    /// The label shown in the legend. Empty hides the entry.
    pub name: String,
    /// The ordered `(x, y)` samples.
    pub points: Vec<(f64, f64)>,
    /// The caller's stroke colour, or `None` to follow the chart's palette.
    pub color: Option<Color>,
    /// Whether this series fills the area under its line.
    ///
    /// Per series, because a multi-line chart usually wants one emphasised series and several
    /// plain ones. The chart-level [`LineChart::set_fill_area`] remains the default for series
    /// that do not state their own choice.
    pub fill: Option<bool>,
    /// Whether the series is drawn and contributes to the resolved axis range.
    ///
    /// A legend click flips this flag; the default is `true`, so a series list that never
    /// touches it renders exactly as before.
    pub visible: bool,
}

impl LineSeries {
    /// Creates a named series from `points`.
    pub fn new(name: impl Into<String>, points: Vec<(f64, f64)>) -> Self {
        Self { name: name.into(), points, color: None, fill: None, visible: true }
    }

    /// Creates a series whose initial visibility is stated explicitly.
    pub fn with_visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }

    /// Sets the series colour, replacing the palette entry.
    pub fn with_color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    /// Enables or disables the area fill for this series only.
    pub fn with_fill(mut self, fill: bool) -> Self {
        self.fill = Some(fill);
        self
    }
}

/// Cached data bounds across every series (F-P12).
#[derive(Clone, Copy, Default)]
struct SeriesExtents {
    x_min: f64,
    x_max: f64,
    y_min: f64,
    y_max: f64,
    valid: bool,
}

/// A 2D line chart widget for visualizing (x, y) data series.
///
/// The chart stores one or more [`LineSeries`]. The single-series API
/// (`set_data`, `add_point`, `data`) operates on series zero, so existing code
/// keeps working while `set_series` / `add_series` enable several lines and a
/// legend.
pub struct LineChart {
    base: BaseWidget,
    /// Every series; index zero is the one the single-series API reads and writes.
    series: Vec<LineSeries>,
    line_color: Color,
    fill_area: bool,
    fill_color: Color,
    stroke_width: f32,
    x_min: Option<f64>,
    x_max: Option<f64>,
    y_min: Option<f64>,
    y_max: Option<f64>,
    show_grid: bool,
    /// The caller's grid colour, or `None` while the grid follows the chart chrome.
    ///
    /// The field used to be a plain `Color` seeded with `rgba(200, 200, 200, 120)`, which
    /// is the *light-theme* grid painted unbounded on every appearance: near-invisible on a
    /// light surface and too bright on a dark one. `axis_chrome` already derives the grid
    /// strength for every other chart, so an unset grid now resolves to that same value and
    /// [`LineChart::set_grid_color`] still wins when the caller states one.
    grid_color: Option<Color>,
    show_labels: bool,
    /// Whether the legend column is reserved and drawn.
    show_legend: bool,
    /// The series and point index under the pointer.
    hovered: Option<(usize, usize)>,
    /// Whether a tooltip is painted beside the hovered point.
    show_tooltip: bool,
    /// Whether wheel zoom and drag panning are accepted.
    zoom_enabled: bool,
    /// Minimum/maximum bounds of every series, rebuilt only when the data changes (F-P12).
    flat_extents: std::cell::Cell<SeriesExtents>,
    /// Per-series point indices sorted by x, rebuilt with the extents (F-P12).
    sorted_x: std::cell::RefCell<Option<Vec<Vec<usize>>>>,
    /// How each numeric axis maps values to the plot and chooses its ticks.
    x_axis_config: AxisConfig,
    y_axis_config: AxisConfig,
    /// Whether data changes ease from the samples currently on screen.
    animate: bool,
    /// Progress of the running data transition, in `0.0..=1.0`.
    transition: PropertyDriver,
    /// The start samples of the running transition, one list per series.
    from_series: Vec<Vec<(f64, f64)>>,
    /// Reused scratch for interpolated samples while a transition is running.
    animated_points: Vec<(f64, f64)>,
    /// Key of the cached projection; `None` until the first paint.
    projection_key: Option<ProjectionKey>,
    /// Pixel-space points per series, rebuilt only when the key changes.
    projection_series: Vec<Vec<Point>>,
    /// Incremented on every projection rebuild; read by the cache test.
    #[cfg(test)]
    projection_builds: std::cell::Cell<u64>,
    /// The legend column's clickable rows from the last paint.
    last_legend_area: Option<LegendArea>,
    /// Whether the hover read-out is delegated to an external tooltip host.
    hover_readout_external: bool,
    /// Bumped on every data or visibility change so the projection key cannot collide.
    data_revision: u64,
    /// The pointer position where a press began, for the click-vs-pan threshold (CH-06).
    press_start: Option<Point>,
    /// The point under the press, reported on release when the gesture was a click (CH-06).
    pressed_point: Option<(usize, usize)>,
    /// Whether the current drag actually panned (CH-06).
    pan_actual: bool,
    /// The chart title, painted centred at the top (CH-07).
    title: Option<String>,
    /// The x-axis title, painted under the plot (CH-07).
    x_axis_label: Option<String>,
    /// The y-axis title, painted above the plot's left edge (CH-07).
    y_axis_label: Option<String>,
    /// An attached reference-line companion (COMP-F01).
    crosshair: Option<crate::widget::chart_widgets::chart_crosshair::ChartCrosshair>,
    /// The X window set by zooming or panning, or `None` for the resolved data range.
    view_x: Option<(f64, f64)>,
    /// The pointer position where a pan drag began, while one is in progress.
    pan_origin: Option<(Point, (f64, f64))>,
    /// The plot area from the last paint, used to hit-test pointer events without measuring
    /// the axis text a second time.
    last_plot_area: Option<Rect>,
    /// The last pointer position seen, used to anchor wheel zoom (the wheel event carries
    /// no position of its own).
    last_pointer: Option<Point>,
    /// Cached y-axis label width; `None` until the first measured paint.
    #[cfg(feature = "chart")]
    y_label_cache: Option<YAxisLabelCache>,
    /// Emitted with `(series, point)` under the pointer when hover enters a point.
    pub point_hovered: Signal1<(usize, usize)>,
    /// Emitted with `(series, point)` when the pointer leaves the last hovered point.
    pub point_unhovered: Signal1<(usize, usize)>,
    /// Emitted with the `(series, point)` of a clicked point.
    pub point_clicked: Signal1<(usize, usize)>,
    /// Emitted with `(series, visible)` after a legend click or [`Self::set_series_visible`].
    pub series_toggled: Signal1<(usize, bool)>,
}

/// The key that makes the projection cache valid for exactly one view.
#[derive(Clone, Copy, PartialEq)]
struct ProjectionKey {
    x_min_bits: u64,
    x_max_bits: u64,
    y_min_bits: u64,
    y_max_bits: u64,
    plot: Rect,
    x_axis: AxisConfig,
    y_axis: AxisConfig,
    data_revision: u64,
    /// Whether a transition is contributing interpolated samples.
    animating: bool,
    /// Bits of the transition progress, or zero when no transition is running.
    animation_bits: u32,
}

/// The legend column's geometry and clickable row count.
#[derive(Clone, Copy)]
struct LegendArea {
    x: i32,
    y: i32,
    row_height: i32,
    rows: usize,
}

/// Cached width of the widest y-axis tick label.
///
/// The measurement depends only on the tick values (which the y range fixes), the
/// control width, the font, and the backend's DPI scale, so those form the key.
#[cfg(feature = "chart")]
struct YAxisLabelCache {
    y_min_bits: u64,
    y_max_bits: u64,
    width: u32,
    dpi_bits: u32,
    font: Font,
    widest: u32,
}

#[cfg(feature = "chart")]
impl YAxisLabelCache {
    /// Whether this cache still describes the requested measurement.
    fn matches(&self, y_min: f64, y_max: f64, width: u32, dpi_scale: f32, font: &Font) -> bool {
        self.y_min_bits == y_min.to_bits()
            && self.y_max_bits == y_max.to_bits()
            && self.width == width
            && self.dpi_bits == dpi_scale.to_bits()
            && &self.font == font
    }
}

impl LineChart {
    /// Creates a new LineChart widget with the given geometry.
    ///
    /// Defaults: blue line, no fill, stroke width 2, grid enabled with light gray,
    /// labels enabled, legend hidden, tooltip hidden, zoom disabled.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::LineChart, geometry, "LineChart"),
            series: Vec::new(),
            line_color: Color::PRIMARY,
            fill_area: false,
            fill_color: Color::rgba(66, 133, 244, 60),
            stroke_width: 2.0,
            x_min: None,
            x_max: None,
            y_min: None,
            y_max: None,
            show_grid: true,
            grid_color: None,
            show_labels: true,
            show_legend: false,
            hovered: None,
            show_tooltip: false,
            zoom_enabled: false,
            flat_extents: std::cell::Cell::new(SeriesExtents::default()),
            sorted_x: std::cell::RefCell::new(None),
            x_axis_config: AxisConfig::default(),
            y_axis_config: AxisConfig::default(),
            animate: true,
            transition: PropertyDriver::at(1.0, MotionSlot::Normal),
            from_series: Vec::new(),
            animated_points: Vec::new(),
            projection_key: None,
            projection_series: Vec::new(),
            #[cfg(test)]
            projection_builds: std::cell::Cell::new(0),
            last_legend_area: None,
            hover_readout_external: false,
            data_revision: 0,
            press_start: None,
            pressed_point: None,
            pan_actual: false,
            title: None,
            x_axis_label: None,
            y_axis_label: None,
            crosshair: None,
            view_x: None,
            pan_origin: None,
            last_plot_area: None,
            last_pointer: None,
            #[cfg(feature = "chart")]
            y_label_cache: None,
            point_hovered: Signal1::new(),
            point_unhovered: Signal1::new(),
            point_clicked: Signal1::new(),
            series_toggled: Signal1::new(),
        }
    }

    /// Sets the primary series' data points, replacing any previous primary data.
    ///
    /// Non-finite samples (`NaN`, `±Inf`) have no honest pixel position and are dropped; see
    /// `is_finite_sample`. A lone bad sample would
    /// otherwise stretch the auto-range and squash every real point.
    pub fn set_data(&mut self, points: Vec<(f64, f64)>) {
        self.ensure_primary();
        let next = finite_samples(points);
        let from = if self.animate && self.series[0].points.len() == next.len() {
            Some(vec![self.displayed_points(0)])
        } else {
            None
        };
        self.series[0].points = next;
        self.install_transition(from);
        self.invalidate_data_cache();
        self.revalidate_hover();
        self.base.request_redraw();
    }

    /// Replaces every series at once.
    ///
    /// Non-finite samples are dropped per series. An empty list clears the chart.
    pub fn set_series(&mut self, series: Vec<LineSeries>) {
        let next: Vec<LineSeries> = series
            .into_iter()
            .map(|mut item| {
                item.points = finite_samples(item.points);
                item
            })
            .collect();
        let same_shape = self.series.len() == next.len()
            && self.series.iter().zip(&next).all(|(old, new)| old.points.len() == new.points.len());
        let from = if self.animate && same_shape {
            Some((0..self.series.len()).map(|index| self.displayed_points(index)).collect())
        } else {
            None
        };
        self.series = next;
        self.install_transition(from);
        self.invalidate_data_cache();
        self.revalidate_hover();
        self.base.request_redraw();
    }

    /// Appends one series. The primary series is created first when the chart is empty.
    pub fn add_series(&mut self, mut series: LineSeries) {
        series.points = finite_samples(series.points);
        self.ensure_primary();
        self.series.push(series);
        self.clear_transition();
        self.invalidate_data_cache();
        self.base.request_redraw();
    }

    /// The samples currently on screen for `series_index`, including any running transition.
    fn displayed_points(&self, series_index: usize) -> Vec<(f64, f64)> {
        let Some(series) = self.series.get(series_index) else {
            return Vec::new();
        };
        let progress = self.animation_progress().map(f64::from);
        series
            .points
            .iter()
            .enumerate()
            .map(|(point_index, target)| {
                match progress.and_then(|progress| {
                    self.from_series
                        .get(series_index)
                        .and_then(|points| points.get(point_index))
                        .map(|from| (*from, progress))
                }) {
                    Some((from, progress)) => (
                        from.0 + (target.0 - from.0) * progress,
                        from.1 + (target.1 - from.1) * progress,
                    ),
                    None => *target,
                }
            })
            .collect()
    }

    /// Starts a transition from `from`, or lands immediately when there is nothing to ease.
    fn install_transition(&mut self, from: Option<Vec<Vec<(f64, f64)>>>) {
        let from = match from {
            Some(from) if !from.is_empty() => from,
            _ => {
                self.clear_transition();
                return;
            }
        };
        let unchanged = from.len() == self.series.len()
            && from.iter().zip(&self.series).all(|(old, series)| *old == series.points);
        if unchanged {
            self.clear_transition();
        } else {
            self.from_series = from;
            self.transition.jump_to(0.0);
            self.transition.set_target(1.0);
        }
    }

    /// Lands the running transition on its targets and drops the start samples.
    fn clear_transition(&mut self) {
        self.transition.jump_to(1.0);
        self.from_series.clear();
    }

    /// The transition progress, or `None` when no transition is running.
    fn animation_progress(&self) -> Option<f32> {
        (!self.from_series.is_empty()).then(|| self.transition.value().clamp(0.0, 1.0))
    }

    /// Returns every series, primary first.
    pub fn series(&self) -> &[LineSeries] {
        &self.series
    }

    /// Ensures series zero exists so the single-series setters have a home.
    fn ensure_primary(&mut self) {
        if self.series.is_empty() {
            self.series.push(LineSeries::new("", Vec::new()));
        }
    }

    /// The colour a series is drawn in.
    fn series_color(&self, index: usize) -> Color {
        match self.series.get(index).and_then(|series| series.color) {
            Some(color) => color,
            None if index == 0 => self.line_color,
            None => {
                const PALETTE: [Color; 5] = [
                    Color::rgb(219, 68, 55),
                    Color::rgb(244, 180, 0),
                    Color::rgb(15, 157, 88),
                    Color::rgb(171, 71, 188),
                    Color::rgb(0, 172, 193),
                ];
                PALETTE[(index - 1) % PALETTE.len()]
            }
        }
    }

    /// Drops a hover index the current data no longer has a point for.
    fn revalidate_hover(&mut self) {
        let still_valid = self.hovered.is_some_and(|(series, index)| {
            self.series.get(series).is_some_and(|item| item.visible && index < item.points.len())
        });
        if !still_valid {
            if let Some((series, index)) = self.hovered.take() {
                self.point_unhovered.emit((series, index));
            }
        }
    }

    /// Shows or hides the legend column.
    pub fn set_show_legend(&mut self, show: bool) {
        if self.show_legend == show {
            return;
        }
        self.show_legend = show;
        self.base.request_redraw();
    }

    /// Returns whether the legend is shown.
    pub fn show_legend(&self) -> bool {
        self.show_legend
    }

    /// Enables the hover tooltip.
    pub fn set_show_tooltip(&mut self, show: bool) {
        if self.show_tooltip == show {
            return;
        }
        self.show_tooltip = show;
        self.base.request_redraw();
    }

    /// Returns whether the hover tooltip is enabled.
    pub fn show_tooltip(&self) -> bool {
        self.show_tooltip
    }

    /// Enables wheel zoom and drag panning.
    pub fn set_zoom_enabled(&mut self, enabled: bool) {
        if self.zoom_enabled == enabled {
            return;
        }
        self.zoom_enabled = enabled;
        if !enabled {
            self.view_x = None;
            self.pan_origin = None;
        }
        self.base.request_redraw();
    }

    /// Returns whether zoom and pan are enabled.
    pub fn is_zoom_enabled(&self) -> bool {
        self.zoom_enabled
    }

    /// Attaches a crosshair companion; the chart drives its position (COMP-F01).
    pub fn set_crosshair(
        &mut self,
        crosshair: Option<crate::widget::chart_widgets::chart_crosshair::ChartCrosshair>,
    ) {
        self.crosshair = crosshair;
        self.base.request_redraw();
    }

    /// The attached crosshair, when one is set.
    pub fn crosshair(
        &self,
    ) -> Option<&crate::widget::chart_widgets::chart_crosshair::ChartCrosshair> {
        self.crosshair.as_ref()
    }

    /// The attached crosshair mutably, when one is set.
    pub fn crosshair_mut(
        &mut self,
    ) -> Option<&mut crate::widget::chart_widgets::chart_crosshair::ChartCrosshair> {
        self.crosshair.as_mut()
    }

    /// The chart title, when one is set (CH-07).
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    /// Sets the chart title, painted centred at the top.
    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = Some(title.into());
        self.base.request_redraw();
    }

    /// Removes the chart title.
    pub fn clear_title(&mut self) {
        self.title = None;
        self.base.request_redraw();
    }

    /// The x-axis title, when one is set.
    pub fn x_axis_label(&self) -> Option<&str> {
        self.x_axis_label.as_deref()
    }

    /// Sets the x-axis title, painted under the plot.
    pub fn set_x_axis_label(&mut self, label: impl Into<String>) {
        self.x_axis_label = Some(label.into());
        self.base.request_redraw();
    }

    /// The y-axis title, when one is set.
    pub fn y_axis_label(&self) -> Option<&str> {
        self.y_axis_label.as_deref()
    }

    /// Sets the y-axis title, painted above the plot's leading edge.
    pub fn set_y_axis_label(&mut self, label: impl Into<String>) {
        self.y_axis_label = Some(label.into());
        self.base.request_redraw();
    }

    /// The X window set by zooming, or `None` while the resolved data range is used.
    pub fn view_range(&self) -> Option<(f64, f64)> {
        self.view_x
    }

    /// Sets an explicit X window. The range is normalised and must be finite and non-degenerate.
    pub fn set_view_range(&mut self, range: Option<(f64, f64)>) {
        self.view_x = range.map(|(low, high)| if low <= high { (low, high) } else { (high, low) });
        self.base.request_redraw();
    }

    /// Restores the auto-computed axis range.
    pub fn reset_view(&mut self) {
        if self.view_x.is_none() {
            return;
        }
        self.view_x = None;
        self.pan_origin = None;
        self.base.request_redraw();
    }

    /// The hovered `(series, point)` index, if the pointer is over a point.
    pub fn hovered_point(&self) -> Option<(usize, usize)> {
        self.hovered
    }

    /// The hovered point's data coordinates, when one is hovered.
    pub fn hovered_value(&self) -> Option<(f64, f64)> {
        let (series, index) = self.hovered?;
        self.series.get(series)?.points.get(index).copied()
    }

    /// Sets the line color.
    pub fn set_line_color(&mut self, color: Color) {
        if self.line_color == color {
            return;
        }
        self.line_color = color;
        self.base.request_redraw();
    }

    /// Returns the current line color.
    pub fn line_color(&self) -> Color {
        self.line_color
    }

    /// Enables or disables the filled area under the line.
    pub fn set_fill_area(&mut self, fill: bool) {
        if self.fill_area == fill {
            return;
        }
        self.fill_area = fill;
        self.base.request_redraw();
    }

    /// Returns whether area fill is enabled.
    pub fn is_fill_area(&self) -> bool {
        self.fill_area
    }

    /// Sets the fill color used for the area under the line.
    pub fn set_fill_color(&mut self, color: Color) {
        if self.fill_color == color {
            return;
        }
        self.fill_color = color;
        self.base.request_redraw();
    }

    /// Returns the current fill color.
    pub fn fill_color(&self) -> Color {
        self.fill_color
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

    /// Adds a single data point to the primary series.
    ///
    /// A non-finite point is refused (see [`Self::set_data`]) so the chart never holds a sample it
    /// cannot place on the plot.
    pub fn add_point(&mut self, x: f64, y: f64) {
        if !is_finite_sample((x, y)) {
            log::warn!("[line_chart] ignoring the non-finite point ({x}, {y})");
            return;
        }
        self.ensure_primary();
        self.series[0].points.push((x, y));
        self.invalidate_data_cache();
        self.base.request_redraw();
    }

    /// Removes all data points and series from the chart.
    pub fn clear_data(&mut self) {
        self.series.clear();
        self.invalidate_data_cache();
        self.revalidate_hover();
        self.base.request_redraw();
    }

    /// Returns the primary series' data points.
    pub fn data(&self) -> &[(f64, f64)] {
        self.series.first().map_or(&[], |series| series.points.as_slice())
    }

    /// Enables or disables the grid lines.
    pub fn set_show_grid(&mut self, show: bool) {
        if self.show_grid == show {
            return;
        }
        self.show_grid = show;
        self.base.request_redraw();
    }

    /// Returns whether grid lines are shown.
    pub fn show_grid(&self) -> bool {
        self.show_grid
    }

    /// Sets the grid line color, overriding the theme-derived default.
    pub fn set_grid_color(&mut self, color: Color) {
        if self.grid_color == Some(color) {
            return;
        }
        self.grid_color = Some(color);
        self.base.request_redraw();
    }

    /// Returns the colour the grid is drawn in: the caller's override, or the chart
    /// chrome's grid strength when none was set.
    ///
    /// `axis_chrome_color(0.16)` is the third member of `axis_chrome`'s triple — the grid
    /// derivation every chart shares — named directly because the triple itself is only
    /// compiled in the engine-less build.
    pub fn grid_color(&self) -> Color {
        self.grid_color
            .unwrap_or_else(|| crate::widget::chart_widgets::charts::axis_chrome_color(0.16))
    }

    /// Enables or disables axis labels.
    pub fn set_show_labels(&mut self, show: bool) {
        if self.show_labels == show {
            return;
        }
        self.show_labels = show;
        self.base.request_redraw();
    }

    /// Returns whether axis labels are shown.
    pub fn show_labels(&self) -> bool {
        self.show_labels
    }

    /// Resolves X-axis range, honouring a zoom window when one is set.
    fn resolve_x_range(&self) -> (f64, f64) {
        if let Some((min, max)) = self.view_x {
            return (min, max);
        }
        match (self.x_min, self.x_max) {
            (Some(min), Some(max)) => (min, max),
            _ => {
                let extents = self.extents();
                if !extents.valid {
                    return (0.0, 10.0);
                }
                let range = Self::padded_range(extents.x_min, extents.x_max);
                if self.x_axis_config.scale == AxisScale::Logarithmic {
                    Self::log_domain_range(range, extents.x_min)
                } else {
                    range
                }
            }
        }
    }

    /// The cached data bounds, computed once per data change (F-P12).
    fn extents(&self) -> SeriesExtents {
        let cached = self.flat_extents.get();
        if cached.valid {
            return cached;
        }
        let mut extents = SeriesExtents {
            x_min: f64::INFINITY,
            x_max: f64::NEG_INFINITY,
            y_min: f64::INFINITY,
            y_max: f64::NEG_INFINITY,
            valid: false,
        };
        for point in self
            .series
            .iter()
            .filter(|series| series.visible)
            .flat_map(|series| series.points.iter())
        {
            extents.x_min = extents.x_min.min(point.0);
            extents.x_max = extents.x_max.max(point.0);
            extents.y_min = extents.y_min.min(point.1);
            extents.y_max = extents.y_max.max(point.1);
        }
        if extents.x_min.is_finite()
            && extents.x_max.is_finite()
            && extents.y_min.is_finite()
            && extents.y_max.is_finite()
        {
            extents.valid = true;
            self.flat_extents.set(extents);
        }
        extents
    }

    /// Drops the extents and the sorted-x index after a data mutation.
    fn invalidate_data_cache(&mut self) {
        self.flat_extents.set(SeriesExtents::default());
        *self.sorted_x.borrow_mut() = None;
        self.data_revision = self.data_revision.wrapping_add(1);
        self.projection_key = None;
    }

    /// The axis presentation used for the x axis.
    pub fn x_axis_config(&self) -> AxisConfig {
        self.x_axis_config
    }

    /// The axis presentation used for the y axis.
    pub fn y_axis_config(&self) -> AxisConfig {
        self.y_axis_config
    }

    /// Replaces the x-axis presentation (scale, direction, tick step).
    pub fn set_x_axis_config(&mut self, config: AxisConfig) {
        if self.x_axis_config == config {
            return;
        }
        self.x_axis_config = config;
        self.projection_key = None;
        self.base.request_redraw();
    }

    /// Replaces the y-axis presentation (scale, direction, tick step).
    pub fn set_y_axis_config(&mut self, config: AxisConfig) {
        if self.y_axis_config == config {
            return;
        }
        self.y_axis_config = config;
        self.projection_key = None;
        self.base.request_redraw();
    }

    /// Sets the x-axis scale. [`AxisScale::Linear`] is the default.
    pub fn set_x_scale(&mut self, scale: AxisScale) {
        let mut config = self.x_axis_config;
        config.scale = scale;
        self.set_x_axis_config(config);
    }

    /// Sets the y-axis scale. [`AxisScale::Linear`] is the default.
    pub fn set_y_scale(&mut self, scale: AxisScale) {
        let mut config = self.y_axis_config;
        config.scale = scale;
        self.set_y_axis_config(config);
    }

    /// Reverses the x axis (maximum at the left) when `reversed` is set.
    pub fn set_x_reversed(&mut self, reversed: bool) {
        let mut config = self.x_axis_config;
        config.reversed = reversed;
        self.set_x_axis_config(config);
    }

    /// Reverses the y axis (minimum at the top) when `reversed` is set.
    pub fn set_y_reversed(&mut self, reversed: bool) {
        let mut config = self.y_axis_config;
        config.reversed = reversed;
        self.set_y_axis_config(config);
    }

    /// Enables 1-2-5 automatic tick steps when no manual step is set.
    pub fn set_auto_tick_step(&mut self, auto: bool) {
        let mut x = self.x_axis_config;
        x.auto_step = auto;
        let mut y = self.y_axis_config;
        y.auto_step = auto;
        self.set_x_axis_config(x);
        self.set_y_axis_config(y);
    }

    /// Fixes the x-axis tick step, or restores the derived step with `None`.
    pub fn set_x_tick_step(&mut self, step: Option<f64>) {
        let mut config = self.x_axis_config;
        config.step = step.filter(|value| value.is_finite() && *value > 0.0);
        self.set_x_axis_config(config);
    }

    /// Fixes the y-axis tick step, or restores the derived step with `None`.
    pub fn set_y_tick_step(&mut self, step: Option<f64>) {
        let mut config = self.y_axis_config;
        config.step = step.filter(|value| value.is_finite() && *value > 0.0);
        self.set_y_axis_config(config);
    }

    /// Enables or disables easing of data changes. On by default.
    pub fn set_animate(&mut self, animate: bool) {
        if self.animate == animate {
            return;
        }
        self.animate = animate;
        if !animate {
            self.clear_transition();
        }
        self.base.request_redraw();
    }

    /// Whether data changes ease into place.
    pub fn is_animated(&self) -> bool {
        self.animate
    }

    /// Whether series `index` is drawn and contributes to the axis range.
    pub fn is_series_visible(&self, index: usize) -> bool {
        self.series.get(index).is_some_and(|series| series.visible)
    }

    /// Shows or hides series `index`, reporting the change through [`Self::series_toggled`].
    ///
    /// Returns `false` when `index` is out of bounds.
    pub fn set_series_visible(&mut self, index: usize, visible: bool) -> bool {
        let Some(series) = self.series.get_mut(index) else {
            return false;
        };
        if series.visible == visible {
            return true;
        }
        series.visible = visible;
        self.invalidate_data_cache();
        self.revalidate_hover();
        self.series_toggled.emit((index, visible));
        self.base.request_redraw();
        true
    }

    /// Delegates the hover read-out to a host-owned tooltip when `external` is set.
    ///
    /// The inline bubble remains the default. A host that owns one
    /// [`ChartTooltip`](crate::widget::companions::ChartTooltip) for several charts flips this
    /// so there is a single presentation, and reads [`Self::hover_anchor`] to fill it.
    pub fn set_hover_readout_external(&mut self, external: bool) {
        if self.hover_readout_external == external {
            return;
        }
        self.hover_readout_external = external;
        self.base.request_redraw();
    }

    /// Whether the hover read-out is delegated to an external tooltip host.
    pub fn hover_readout_external(&self) -> bool {
        self.hover_readout_external
    }

    /// The hovered point's anchor and suggested tooltip content, for an external read-out.
    ///
    /// Returns `None` when nothing is hovered or the chart has not been painted yet.
    pub fn hover_anchor(
        &self,
    ) -> Option<(Point, crate::compat::Vec<crate::widget::companions::TooltipLine>)> {
        use crate::widget::companions::TooltipLine;
        let (series_index, point_index) = self.hovered?;
        let point = *self.series.get(series_index)?.points.get(point_index)?;
        let plot = self.last_plot_area?;
        let (x_min, x_max) = self.resolve_x_range();
        let (y_min, y_max) = self.resolve_y_range();
        let pixel = Self::map_to_pixel_axes(
            point.0,
            point.1,
            self.x_axis_config,
            self.y_axis_config,
            x_min,
            x_max,
            y_min,
            y_max,
            plot,
        );
        let name = self
            .series
            .get(series_index)
            .map(|series| series.name.as_str())
            .filter(|name| !name.is_empty());
        let text = match name {
            Some(name) => format!("{name}  {:.2}, {:.2}", point.0, point.1),
            None => format!("{:.2}, {:.2}", point.0, point.1),
        };
        Some((
            pixel,
            crate::compat::Vec::from([TooltipLine::colored(text, self.series_color(series_index))]),
        ))
    }

    /// The series index of the legend row under `pos`, when the legend is shown.
    fn legend_entry_at(&self, pos: Point) -> Option<usize> {
        if !self.show_legend {
            return None;
        }
        let area = self.last_legend_area?;
        if pos.x < area.x || pos.y < area.y {
            return None;
        }
        let row = ((pos.y - area.y) / area.row_height.max(1)) as usize;
        if row >= area.rows {
            return None;
        }
        self.series
            .iter()
            .enumerate()
            .filter(|(_, series)| !series.name.is_empty())
            .nth(row)
            .map(|(index, _)| index)
    }

    /// Width of the widest y-axis tick label.
    ///
    /// Measures only when the y range, the control width, the font or the DPI scale
    /// changes; every other paint reuses the width the previous one measured.
    #[cfg(feature = "chart")]
    fn widest_y_label(
        &mut self,
        context: &RenderContext,
        y_min: f64,
        y_max: f64,
        width: u32,
        font: Font,
    ) -> f32 {
        let dpi_scale = context.dpi_scale();
        if let Some(cache) = self.y_label_cache.as_ref() {
            if cache.matches(y_min, y_max, width, dpi_scale, &font) {
                return cache.widest as f32;
            }
        }
        let mut widest = 0u32;
        for step in 0..=4 {
            let value = y_min + (y_max - y_min) * f64::from(step) / 4.0;
            let label = format!("{value:.1}");
            widest = widest.max(context.measure_text(&label, &font).width);
        }
        self.y_label_cache = Some(YAxisLabelCache {
            y_min_bits: y_min.to_bits(),
            y_max_bits: y_max.to_bits(),
            width,
            dpi_bits: dpi_scale.to_bits(),
            font,
            widest,
        });
        widest as f32
    }

    /// Resolves Y-axis range across every series.
    fn resolve_y_range(&self) -> (f64, f64) {
        match (self.y_min, self.y_max) {
            (Some(min), Some(max)) => (min, max),
            _ => {
                let extents = self.extents();
                if !extents.valid {
                    return (0.0, 10.0);
                }
                let range = Self::padded_range(extents.y_min, extents.y_max);
                if self.y_axis_config.scale == AxisScale::Logarithmic {
                    Self::log_domain_range(range, extents.y_min)
                } else {
                    range
                }
            }
        }
    }

    /// Clamps an auto-resolved range into the positive domain a logarithmic axis needs.
    ///
    /// The linear padding can push the lower bound to or below zero, where the logarithm
    /// is undefined and [`AxisConfig::fraction`] silently falls back to the linear
    /// mapping. When the data itself reaches into positive territory the lower bound is
    /// raised to the smallest positive data value, so the requested logarithmic scale
    /// actually reaches the projection; data with no positive values keeps that linear
    /// fallback.
    fn log_domain_range(range: (f64, f64), data_min: f64) -> (f64, f64) {
        let (min, max) = range;
        if min > 0.0 || max <= 0.0 || !data_min.is_finite() || data_min <= 0.0 {
            return range;
        }
        (min.max(data_min), max)
    }

    /// Expands a raw data range by the historical 10 % padding.
    fn padded_range(min: f64, max: f64) -> (f64, f64) {
        if (max - min).abs() < f64::EPSILON {
            return (min - 1.0, min + 1.0);
        }
        let padding = (max - min) * 0.1;
        (min - padding, max + padding)
    }

    /// Maps a window-relative pointer position to the nearest plotted point.
    ///
    /// Points are searched through a per-series x-sorted index (F-P12): the pointer's ±24 px
    /// hit window is converted to a data interval and binary-searched, so a mouse move costs
    /// `O(log n + k)` instead of scanning every point of every series.
    fn nearest_point(&self, pos: Point, plot_area: Rect) -> Option<(usize, usize)> {
        if !plot_area.contains_point(pos) {
            return None;
        }
        let (x_min, x_max) = self.resolve_x_range();
        let (y_min, y_max) = self.resolve_y_range();
        let span = x_max - x_min;
        if !span.is_finite() || span == 0.0 || plot_area.width == 0 {
            return None;
        }
        let to_data = |px: i32| -> f64 {
            let mut fraction = (px - plot_area.x) as f64 / plot_area.width as f64;
            if self.x_axis_config.reversed {
                fraction = 1.0 - fraction;
            }
            if self.x_axis_config.scale == AxisScale::Logarithmic && x_min > 0.0 {
                x_min * 10f64.powf(fraction * (x_max / x_min).log10())
            } else {
                x_min + fraction * span
            }
        };
        let left_bound = to_data(pos.x.saturating_sub(24));
        let right_bound = to_data(pos.x.saturating_add(24));
        let (left_bound, right_bound) = if left_bound <= right_bound {
            (left_bound, right_bound)
        } else {
            (right_bound, left_bound)
        };

        let mut cache = self.sorted_x.borrow_mut();
        if cache.is_none() {
            *cache = Some(
                self.series
                    .iter()
                    .map(|series| {
                        let mut order: Vec<usize> = (0..series.points.len()).collect();
                        order.sort_by(|a, b| series.points[*a].0.total_cmp(&series.points[*b].0));
                        order
                    })
                    .collect(),
            );
        }
        let sorted = cache.as_ref().expect("the x index was just built");
        let mut best: Option<(usize, usize, i64)> = None;
        for (series_index, series) in self.series.iter().enumerate() {
            if !series.visible {
                continue;
            }
            let Some(order) = sorted.get(series_index) else { continue };
            let start = order.partition_point(|&index| series.points[index].0 < left_bound);
            for &index in &order[start..] {
                let point = &series.points[index];
                if point.0 > right_bound {
                    break;
                }
                let pixel = Self::map_to_pixel_axes(
                    point.0,
                    point.1,
                    self.x_axis_config,
                    self.y_axis_config,
                    x_min,
                    x_max,
                    y_min,
                    y_max,
                    plot_area,
                );
                let distance =
                    i64::from((pixel.x - pos.x).abs()) + i64::from((pixel.y - pos.y).abs());
                if distance <= 24 && best.is_none_or(|(_, _, near)| distance < near) {
                    best = Some((series_index, index, distance));
                }
            }
        }
        best.map(|(series, index, _)| (series, index))
    }

    /// Maps a data coordinate to a pixel position on screen using the given axis settings.
    #[allow(clippy::too_many_arguments)]
    fn map_to_pixel_axes(
        x: f64,
        y: f64,
        x_axis: AxisConfig,
        y_axis: AxisConfig,
        x_min: f64,
        x_max: f64,
        y_min: f64,
        y_max: f64,
        plot_rect: Rect,
    ) -> Point {
        let fx = x_axis.fraction(x, x_min, x_max);
        let fy = y_axis.fraction(y, y_min, y_max);
        Point::new(
            plot_rect.x + (plot_rect.width as f64 * fx) as i32,
            plot_rect.y + (plot_rect.height as f64 * (1.0 - fy)) as i32,
        )
    }

    /// Maps a data coordinate to a pixel position on screen.
    #[cfg(test)]
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
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        crate::core::Size::new(400, 300)
    }

    /// Advances the data transition by `delta_ms`.
    fn tick(&mut self, delta_ms: u32) -> bool {
        if !self.animate || self.from_series.is_empty() {
            return false;
        }
        let moving = self.transition.tick(delta_ms);
        if !self.transition.is_moving() {
            self.from_series.clear();
            self.transition.jump_to(1.0);
        }
        moving
    }

    /// Whether the data transition still owes frames.
    fn is_animating(&self) -> bool {
        self.animate && !self.from_series.is_empty() && self.transition.is_moving()
    }

    /// Resolves a published event name to the signal this chart actually emits.
    ///
    /// Each name carries the data index as a `UInt`, which is the payload kind the
    /// capability declares; the control stores that index as a `usize`, so the
    /// conversion is the widening that preserves the emitted value. The `point_*`
    /// names would otherwise be accepted by `connect_event` and dropped silently.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "point_clicked" => {
                Some(EventSignalRef::mapped("point_clicked", &self.point_clicked, |v| {
                    CapabilityValue::Tuple(crate::compat::Vec::from([
                        CapabilityValue::UInt(v.0 as u64),
                        CapabilityValue::UInt(v.1 as u64),
                    ]))
                }))
            }
            "point_hovered" => {
                Some(EventSignalRef::mapped("point_hovered", &self.point_hovered, |v| {
                    CapabilityValue::Tuple(crate::compat::Vec::from([
                        CapabilityValue::UInt(v.0 as u64),
                        CapabilityValue::UInt(v.1 as u64),
                    ]))
                }))
            }
            "point_unhovered" => {
                Some(EventSignalRef::mapped("point_unhovered", &self.point_unhovered, |v| {
                    CapabilityValue::Tuple(crate::compat::Vec::from([
                        CapabilityValue::UInt(v.0 as u64),
                        CapabilityValue::UInt(v.1 as u64),
                    ]))
                }))
            }
            "series_toggled" => {
                Some(EventSignalRef::mapped("series_toggled", &self.series_toggled, |v| {
                    CapabilityValue::Tuple(crate::compat::Vec::from([
                        CapabilityValue::UInt(v.0 as u64),
                        CapabilityValue::Bool(v.1),
                    ]))
                }))
            }
            _ => None,
        }
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `LineChart`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` / `access_write_other.in.rs` dispatch, including
/// the `f32` → `f64` widening on read.
impl WidgetProperties for LineChart {
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
            // The y labels are measured rather than assumed: the fixed 64 px column was
            // chosen for a four-character label, so a wider value ran out of the plot's
            // own margin. The column only grows past that reservation, which keeps a
            // normal chart's geometry byte-identical.
            let widest_y_label = if self.show_labels {
                let font = crate::widget::metrics::role_font(self.base.style(), 10.0);
                self.widest_y_label(context, y_min, y_max, rect.width, font)
            } else {
                0.0
            };
            let legend_items = if self.show_legend { self.series.len() } else { 0 };
            let layout = compute_cartesian_layout_measured(
                rect,
                self.show_labels || self.x_axis_label.is_some(),
                self.show_labels || self.y_axis_label.is_some(),
                legend_items,
                widest_y_label,
            );
            let plot_area = plot_rect(&layout);
            let mut adapter = ChartContextAdapter::new(context);

            // Axes and grid come from the shared engine. `show_grid` drives both
            // the horizontal and vertical grid lines; the engine previously
            // emitted only horizontal ones, so the vertical pass is requested
            // explicitly below to preserve this widget's denser grid.
            draw_cartesian_axes(&mut adapter, &layout);
            if self.show_labels {
                if self.y_axis_config.is_default() {
                    draw_y_ticks(&mut adapter, &layout, y_min, y_max, 4, self.show_grid);
                } else {
                    draw_y_ticks_configured(
                        &mut adapter,
                        &layout,
                        y_min,
                        y_max,
                        4,
                        self.y_axis_config,
                        self.show_grid,
                    );
                }
                if self.x_axis_config.is_default() {
                    draw_x_ticks(&mut adapter, &layout, x_min, x_max, 4, false);
                } else {
                    draw_x_ticks_configured(
                        &mut adapter,
                        &layout,
                        x_min,
                        x_max,
                        4,
                        self.x_axis_config,
                        false,
                    );
                }
            } else if self.show_grid {
                // No labels requested, but the grid is: drive it through the tick
                // helpers with label-free positioning so the lines still appear.
                if self.y_axis_config.is_default() {
                    draw_y_ticks(&mut adapter, &layout, y_min, y_max, 4, true);
                } else {
                    draw_y_ticks_configured(
                        &mut adapter,
                        &layout,
                        y_min,
                        y_max,
                        4,
                        self.y_axis_config,
                        true,
                    );
                }
            }
            if self.show_legend {
                draw_legend_entries_iter(
                    &mut adapter,
                    &layout,
                    self.series
                        .iter()
                        .enumerate()
                        .filter(|(_, series)| !series.name.is_empty())
                        .map(|(index, series)| {
                            let color = self.series_color(index);
                            let color = if series.visible { color } else { color.with_alpha(70) };
                            (series.name.as_str(), color)
                        }),
                );
                // The engine thins its rows to those that fit and reserves the last one for
                // its `+N more` summary. Recording the same budget here is what makes a click
                // land on the row the user actually sees.
                let row_height = 18i32;
                let max_rows = ((layout.plot_h() / row_height as f32).floor() as usize).max(1);
                let max_items = max_rows.saturating_sub(1).max(1);
                let named = self.series.iter().filter(|series| !series.name.is_empty()).count();
                self.last_legend_area = Some(LegendArea {
                    x: layout.legend_x() as i32,
                    y: layout.legend_y() as i32,
                    row_height,
                    rows: max_items.min(named),
                });
            } else {
                self.last_legend_area = None;
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
        self.last_plot_area = Some(plot_area);

        // ── Title and axis titles (CH-07) ──
        if self.title.is_some() || self.x_axis_label.is_some() || self.y_axis_label.is_some() {
            let chrome = crate::widget::chart_widgets::charts::axis_chrome_color(0.85);
            if let Some(title) = self.title.as_deref() {
                let font =
                    crate::widget::metrics::role_font(self.base.style(), 12.0).with_bold(true);
                context.draw_text_fitted_cow(
                    Rect::new(rect.x, rect.y, rect.width, 16),
                    title,
                    &font,
                    chrome,
                    crate::core::HorizontalAlignment::Center,
                );
            }
            if let Some(label) = self.x_axis_label.as_deref() {
                let font = crate::widget::metrics::role_font(self.base.style(), 10.0);
                let band = Rect::new(
                    plot_area.x,
                    plot_area.y + plot_area.height as i32 + 14,
                    plot_area.width,
                    12,
                );
                context.draw_text_fitted_cow(
                    band,
                    label,
                    &font,
                    chrome,
                    crate::core::HorizontalAlignment::Center,
                );
            }
            if let Some(label) = self.y_axis_label.as_deref() {
                let font = crate::widget::metrics::role_font(self.base.style(), 10.0);
                let band = Rect::new(rect.x + 2, rect.y + 16, rect.width.min(120), 12);
                context.draw_text_fitted_cow(
                    band,
                    label,
                    &font,
                    chrome,
                    crate::core::HorizontalAlignment::Left,
                );
            }
        }

        // ── Data series ──
        if self.series.iter().all(|series| !series.visible || series.points.len() < 2) {
            return;
        }

        // The pixel positions are a pure function of the data and the view, so they are
        // cached per view and only rebuilt when either changes (CH-008).
        self.ensure_projection(x_min, x_max, y_min, y_max, plot_area);

        for series_index in 0..self.series.len() {
            if !self.series[series_index].visible || self.series[series_index].points.len() < 2 {
                continue;
            }
            let series_color =
                if is_enabled { self.series_color(series_index) } else { disabled_color };
            let mapped = &self.projection_series[series_index];

            // ── Filled area under the line ──
            if self.series[series_index].fill.unwrap_or(self.fill_area) {
                // A per-series fill takes the series' own colour so two filled series stay
                // distinguishable; the chart-level colour remains the first series' default.
                let fill_color = if series_index == 0 && self.series[0].fill.is_none() {
                    self.fill_color
                } else {
                    series_color.with_alpha(60)
                };
                let fill_color =
                    if is_enabled { fill_color } else { disabled_color.with_alpha(60) };
                let baseline_y = plot_area.y + plot_area.height as i32;
                for pair in mapped.windows(2) {
                    let strip_width = (pair[1].x - pair[0].x).max(1) as u32;
                    let top_y = pair[0].y.min(pair[1].y);
                    let strip_height = (baseline_y - top_y).max(1) as u32;
                    context.fill_rect(
                        Rect::new(pair[0].x, top_y, strip_width, strip_height),
                        fill_color,
                    );
                }
            }

            // ── Line connecting the points ──
            let stroke_w = self.stroke_width as u32;
            let right = plot_area.x + plot_area.width as i32;
            for pair in mapped.windows(2) {
                // A segment wholly outside the plot is clipped away rather than handed to
                // the rasteriser: a zoomed-in view keeps most of a long series off screen.
                if (pair[0].x < plot_area.x && pair[1].x < plot_area.x)
                    || (pair[0].x > right && pair[1].x > right)
                {
                    continue;
                }
                context.draw_line_stroke_aa(pair[0], pair[1], series_color, stroke_w.max(1));
            }
        }

        // ── Hover marker and tooltip ──
        if let Some((series_index, index)) = self.hovered {
            if let Some(point) =
                self.series.get(series_index).and_then(|series| series.points.get(index)).copied()
            {
                let pixel = Self::map_to_pixel_axes(
                    point.0,
                    point.1,
                    self.x_axis_config,
                    self.y_axis_config,
                    x_min,
                    x_max,
                    y_min,
                    y_max,
                    plot_area,
                );
                if is_enabled {
                    context.fill_circle(pixel, 3, self.series_color(series_index));
                }
                if self.show_tooltip && !self.hover_readout_external && is_enabled {
                    self.draw_tooltip(context, pixel, point, series_index, plot_area);
                }
            }
        }

        // ── Attached crosshair (COMP-F01) ──
        if let Some(crosshair) = self.crosshair.as_mut() {
            crosshair.base_mut().set_geometry(plot_area);
            crosshair.draw(context);
        }
    }
}

impl LineChart {
    /// Writes the series' points in pixel space into `out`, decimated to at most two samples per
    /// pixel column.
    ///
    /// A series shorter than twice the plot width is mapped one-to-one. A denser one keeps
    /// the first, lowest, highest and last point of every column, which preserves the drawn
    /// envelope while bounding the work per frame — the previous path emitted one stroke per
    /// sample, so ten thousand points meant ten thousand primitives for a plot 400 px wide.
    ///
    /// The buffer is the caller's: the draw path reuses one `Vec` for every series and frame,
    /// where this used to answer with an owned `Vec` per series per frame.
    #[cfg(test)]
    fn write_mapped_points(
        points: &[(f64, f64)],
        x_min: f64,
        x_max: f64,
        y_min: f64,
        y_max: f64,
        plot_area: Rect,
        out: &mut Vec<Point>,
    ) {
        out.clear();
        let map = |x: f64, y: f64| Self::map_to_pixel(x, y, x_min, x_max, y_min, y_max, plot_area);
        let budget = (plot_area.width as usize).saturating_mul(2).max(2);
        if points.len() <= budget {
            out.extend(points.iter().map(|point| map(point.0, point.1)));
            return;
        }

        let chunk = points.len().div_ceil(budget);
        for group in points.chunks(chunk) {
            let first = group[0];
            let last = group[group.len() - 1];
            let (mut lowest, mut highest) = (first, first);
            for point in group {
                if point.1 < lowest.1 {
                    lowest = *point;
                }
                if point.1 > highest.1 {
                    highest = *point;
                }
            }
            let mut candidates = [first, lowest, highest, last];
            candidates.sort_by(|a, b| a.0.total_cmp(&b.0));
            out.extend(candidates.iter().map(|point| map(point.0, point.1)));
        }
    }

    /// Rebuilds the cached pixel projection when the data, view or transition changed.
    fn ensure_projection(
        &mut self,
        x_min: f64,
        x_max: f64,
        y_min: f64,
        y_max: f64,
        plot_area: Rect,
    ) {
        let progress = self.animation_progress();
        let key = ProjectionKey {
            x_min_bits: x_min.to_bits(),
            x_max_bits: x_max.to_bits(),
            y_min_bits: y_min.to_bits(),
            y_max_bits: y_max.to_bits(),
            plot: plot_area,
            x_axis: self.x_axis_config,
            y_axis: self.y_axis_config,
            data_revision: self.data_revision,
            animating: progress.is_some(),
            animation_bits: progress.map(f32::to_bits).unwrap_or(0),
        };
        if self.projection_key == Some(key) {
            return;
        }
        self.projection_key = Some(key);
        self.projection_series.resize_with(self.series.len(), Vec::new);
        for series_index in 0..self.series.len() {
            let visible = self.series[series_index].visible;
            let out = &mut self.projection_series[series_index];
            out.clear();
            if !visible {
                continue;
            }
            if let Some(progress) = progress {
                let progress = f64::from(progress);
                self.animated_points.clear();
                for (point_index, target) in self.series[series_index].points.iter().enumerate() {
                    let from = self
                        .from_series
                        .get(series_index)
                        .and_then(|points| points.get(point_index));
                    self.animated_points.push(match from {
                        Some(from) => (
                            from.0 + (target.0 - from.0) * progress,
                            from.1 + (target.1 - from.1) * progress,
                        ),
                        None => *target,
                    });
                }
                Self::write_mapped_points_axes(
                    &self.animated_points,
                    self.x_axis_config,
                    self.y_axis_config,
                    x_min,
                    x_max,
                    y_min,
                    y_max,
                    plot_area,
                    out,
                );
            } else {
                Self::write_mapped_points_axes(
                    &self.series[series_index].points,
                    self.x_axis_config,
                    self.y_axis_config,
                    x_min,
                    x_max,
                    y_min,
                    y_max,
                    plot_area,
                    out,
                );
            }
        }
        #[cfg(test)]
        self.projection_builds.set(self.projection_builds.get().wrapping_add(1));
    }

    /// [`Self::write_mapped_points`] with explicit axis settings.
    #[allow(clippy::too_many_arguments)]
    fn write_mapped_points_axes(
        points: &[(f64, f64)],
        x_axis: AxisConfig,
        y_axis: AxisConfig,
        x_min: f64,
        x_max: f64,
        y_min: f64,
        y_max: f64,
        plot_area: Rect,
        out: &mut Vec<Point>,
    ) {
        out.clear();
        let map = |x: f64, y: f64| {
            Self::map_to_pixel_axes(x, y, x_axis, y_axis, x_min, x_max, y_min, y_max, plot_area)
        };
        let budget = (plot_area.width as usize).saturating_mul(2).max(2);
        if points.len() <= budget {
            out.extend(points.iter().map(|point| map(point.0, point.1)));
            return;
        }
        let chunk = points.len().div_ceil(budget);
        for group in points.chunks(chunk) {
            let first = group[0];
            let last = group[group.len() - 1];
            let (mut lowest, mut highest) = (first, first);
            for point in group {
                if point.1 < lowest.1 {
                    lowest = *point;
                }
                if point.1 > highest.1 {
                    highest = *point;
                }
            }
            let mut candidates = [first, lowest, highest, last];
            candidates.sort_by(|a, b| a.0.total_cmp(&b.0));
            out.extend(candidates.iter().map(|point| map(point.0, point.1)));
        }
    }

    /// Paints a small readout beside the hovered point.
    ///
    /// The box is clamped into the plot so a point at the edge does not paint its label
    /// outside the control, and it is filled from the control's resolved surface so the text
    /// keeps its contrast on either appearance.
    fn draw_tooltip(
        &self,
        context: &mut RenderContext,
        pixel: Point,
        value: (f64, f64),
        series_index: usize,
        plot: Rect,
    ) {
        use crate::core::HorizontalAlignment;
        let font = crate::widget::metrics::role_font(self.base.style(), 10.0);
        let series_name = self
            .series
            .get(series_index)
            .map(|series| series.name.as_str())
            .filter(|name| !name.is_empty());
        let text = match series_name {
            Some(name) => format!("{name}  {:.2}, {:.2}", value.0, value.1),
            None => format!("{:.2}, {:.2}", value.0, value.1),
        };
        let metrics = context.measure_text(&text, &font);
        let padding = 4i32;
        let width = metrics.width as i32 + padding * 2;
        let height = metrics.height.max(1) as i32 + padding * 2;
        let mut x = pixel.x + 8;
        let mut y = pixel.y - height - 6;
        if x + width > plot.x + plot.width as i32 {
            x = pixel.x - width - 8;
        }
        if y < plot.y {
            y = pixel.y + 6;
        }
        x = x.max(plot.x);
        y = y.max(plot.y).min(plot.y + plot.height as i32 - height);
        let box_rect = Rect::new(x, y, width.max(1) as u32, height.max(1) as u32);
        let (surface, ink) = {
            let style = self.base.style();
            let surface = style
                .background_color
                .or_else(|| {
                    crate::style::resolved_theme_style_shared("line_chart")
                        .and_then(|resolved| resolved.background_color)
                })
                .unwrap_or(Color::WHITE);
            (surface, surface.contrast_color())
        };
        context.fill_rect(box_rect, surface);
        context.draw_rect(box_rect, ink.with_alpha_f32(0.35));
        context.draw_text_line_cow(box_rect, &text, &font, ink, HorizontalAlignment::Center);
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
        // Same derivation as the engine-backed path (`charts::axis_chrome`): this fallback
        // runs on tablet/mobile, where the dark appearance used to get a `DARK_GRAY` axis
        // and near-black labels on a near-black surface.
        let (axis_chrome_color, axis_label, _) = axis_chrome();
        let axis_color = if is_enabled { axis_chrome_color } else { disabled_color };
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

        let grid_color = if is_enabled { self.grid_color() } else { disabled_color };
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
            // The axis labels take the **effective font** at their role size, so the theme family
            // and the user's text scale reach the chart chrome (D09-STYLE-01).
            let label_font = role_font(self.base.style(), 9.0);
            // One buffer reused across all ten labels: `format!` per label per frame was
            // a fresh allocation for a string a `write!` renders in place.
            let mut label = String::new();
            for i in 0..=4 {
                let t = i as f64 / 4.0;
                let val = y_min + (y_max - y_min) * (1.0 - t);
                label.clear();
                let _ = write!(label, "{val:.1}");
                let y_pos = plot_area.y + (plot_area.height as f64 * (1.0 - t)) as i32;
                let text_y = y_pos + 4;
                context.draw_text(
                    Point::new((plot_area.x - 44).max(0), text_y),
                    &label,
                    &label_font,
                    axis_label,
                    crate::core::HorizontalAlignment::Left,
                );
            }
            for i in 0..=4 {
                let t = i as f64 / 4.0;
                let val = x_min + (x_max - x_min) * t;
                label.clear();
                let _ = write!(label, "{val:.1}");
                let x_pos = plot_area.x + (plot_area.width as f64 * t) as i32;
                context.draw_text(
                    Point::new((x_pos - 12).max(plot_area.x), bottom + 16),
                    &label,
                    &label_font,
                    axis_label,
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
        if !self.base.is_enabled() {
            return;
        }
        let plot_area = match self.last_plot_area {
            Some(area) => area,
            None => return,
        };
        match event {
            Event::MouseMove { pos } => {
                self.last_pointer = Some(*pos);
                if let Some((origin, range)) = self.pan_origin {
                    // A drag only becomes a pan once it exceeds the click threshold; below it
                    // the gesture is still a click on the point under the press (CH-06).
                    let moved = self
                        .press_start
                        .is_some_and(|start| (pos.x - start.x).abs() + (pos.y - start.y).abs() > 4);
                    if self.pan_actual || moved {
                        self.pan_actual = true;
                        let span = range.1 - range.0;
                        let width = plot_area.width.max(1) as f64;
                        let shift = (origin.x - pos.x) as f64 / width * span;
                        self.view_x = Some((range.0 + shift, range.1 + shift));
                        self.base.request_redraw();
                        return;
                    }
                }
                let next = self.nearest_point(*pos, plot_area);
                if next != self.hovered {
                    if let Some((series, index)) = self.hovered {
                        self.point_unhovered.emit((series, index));
                    }
                    self.hovered = next;
                    if let Some((series, index)) = next {
                        self.point_hovered.emit((series, index));
                    }
                    self.base.request_redraw();
                }
                // Drive the attached crosshair from the same nearest point (COMP-F01).
                let crosshair_point = self.hovered.and_then(|(series, index)| {
                    self.series.get(series).and_then(|item| item.points.get(index)).copied()
                });
                let crosshair_pixel = crosshair_point.map(|point| {
                    let (x_min, x_max) = self.resolve_x_range();
                    let (y_min, y_max) = self.resolve_y_range();
                    (
                        Self::map_to_pixel_axes(
                            point.0,
                            point.1,
                            self.x_axis_config,
                            self.y_axis_config,
                            x_min,
                            x_max,
                            y_min,
                            y_max,
                            plot_area,
                        ),
                        point,
                    )
                });
                if let Some(crosshair) = self.crosshair.as_mut() {
                    match crosshair_pixel {
                        Some((pixel, point)) => crosshair.show_at_data(pixel, point),
                        None => crosshair.hide(),
                    }
                }
            }
            Event::MouseLeave { .. } => {
                self.pan_origin = None;
                if let Some(crosshair) = self.crosshair.as_mut() {
                    crosshair.hide();
                }
                if let Some((series, index)) = self.hovered.take() {
                    self.point_unhovered.emit((series, index));
                    self.base.request_redraw();
                }
            }
            Event::MousePress { pos, button, .. } if *button == 1 => {
                // A legend row is a toggle, not a plot gesture: consume the press before
                // the click/pan machinery can interpret it as a point interaction.
                if let Some(index) = self.legend_entry_at(*pos) {
                    let visible = self.is_series_visible(index);
                    self.set_series_visible(index, !visible);
                    return;
                }
                let hit = self.nearest_point(*pos, plot_area);
                self.press_start = Some(*pos);
                self.pressed_point = hit;
                self.pan_actual = false;
                if self.zoom_enabled {
                    self.pan_origin = Some((*pos, self.resolve_x_range()));
                }
                self.base.set_mouse_pressed(true);
            }
            Event::MouseRelease { button, .. } if *button == 1 => {
                // A press that never moved is a click on the point under it, whether or not
                // zoom is enabled (CH-06).
                if !self.pan_actual {
                    if let Some((series, index)) = self.pressed_point {
                        self.point_clicked.emit((series, index));
                    }
                }
                self.pan_origin = None;
                self.press_start = None;
                self.pressed_point = None;
                self.pan_actual = false;
                self.base.set_mouse_pressed(false);
            }
            Event::Wheel { delta, .. } if self.zoom_enabled && delta.y != 0 => {
                let (low, high) = self.resolve_x_range();
                let span = (high - low).max(f64::EPSILON);
                // The anchor is the pointer's fraction across the plot, so a wheel turn
                // zooms around what the pointer is pointing at rather than the centre.
                let anchor_x = self
                    .last_pointer
                    .map_or(plot_area.x + (plot_area.width as i32 / 2), |point| point.x);
                let fraction = if plot_area.width == 0 {
                    0.5
                } else {
                    ((anchor_x - plot_area.x) as f64 / plot_area.width as f64).clamp(0.0, 1.0)
                };
                let factor = if delta.y > 0 { 1.25 } else { 0.8 };
                let anchor = low + span * fraction;
                let new_low = anchor - (anchor - low) * factor;
                let new_high = anchor + (high - anchor) * factor;
                if new_high - new_low > f64::EPSILON {
                    self.view_x = Some((new_low, new_high));
                    self.base.request_redraw();
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

    /// Every name the capability publishes must resolve, and a name it does not
    /// publish must stay unresolved — a dead arm reads as support it does not have.
    #[test]
    fn published_point_events_resolve_to_signals() {
        let chart = LineChart::new(Rect::new(0, 0, 300, 200));
        for name in ["point_clicked", "point_hovered", "point_unhovered"] {
            assert!(chart.event_signal_dyn(name).is_some(), "{name} must resolve");
        }
        assert!(chart.event_signal_dyn("no_such_event").is_none());
    }

    /// One ink box per text run, in document order.
    ///
    /// Text leaves the backend as the `font8x8` rectangles the software rasteriser fills — one
    /// axis-aligned `<path>` subpath per set bitmap bit — so the rendered string is **not in the
    /// document in any form** and `svg.contains("50.0")` can never be true. A run can only be
    /// located by *where it is*: the union of one run's ink is its box.
    ///
    /// The first form read that union by hand: it split `d` on `M` and took the four integers of
    /// `M{x} {y}h{w}v{h}`. That is the *bitmap* path's grammar and only its grammar. An outline
    /// face writes `M17.00 23.84L...Z` with fractional vertices, where the same split reads `17`
    /// as `x`, `0` as `y`, `23` as `w` — geometry that is not ink and not where the glyph is. The
    /// crate already has the correct reader, which knows both grammars (and the `data-text`
    /// provenance tag that distinguishes a run from a drawn shape); duplicating it here was the
    /// actual bug. One reader, so the two cannot disagree.
    #[cfg(feature = "chart")]
    fn ink_paths(svg: &str) -> Vec<(i32, i32, i32, i32)> {
        crate::widget::svg::text_ink_boxes(svg)
    }

    /// The bitmap face fills its whole glyph box (`font8x8` rectangles span the box), so a run's
    /// ink left *is* the box left, exactly. An **outline** face draws a real glyph whose ink is
    /// inset — a digit's stroke does not occupy the box's first column — so the ink's left edge
    /// sits one or two pixels right of the box edge at these sizes. A lookup that required
    /// `ink.left == box.left`, or an assertion that pinned the two equal, encoded a property of
    /// the bitmap face rather than of the layout, and is false the moment an outline is drawn.
    ///
    /// The bound is deliberately tight: it has to absorb the outline inset (≤ 2 px) and nothing
    /// more, so a label anchored a whole tick away still fails.
    ///
    /// Gated with its only consumers: the assertions below are `#[cfg(feature = "chart")]`, because
    /// they exercise the shared tick engine. An ungated constant became dead code in
    /// `tablet`/`mobile`/`mini` — where `chart` is off — and `clippy -D warnings` reported it there
    /// while `desktop` stayed green.
    #[cfg(feature = "chart")]
    const INK_INSET_TOLERANCE: i32 = 3;

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

    /// An unset grid follows the shared chart chrome; an explicit colour wins.
    #[test]
    fn an_unset_grid_follows_the_chart_chrome() {
        let chart = LineChart::new(Rect::new(0, 0, 300, 200));
        assert_eq!(
            chart.grid_color(),
            crate::widget::chart_widgets::charts::axis_chrome_color(0.16),
            "the default grid must be the same derivation every other chart uses"
        );
        let mut overridden = chart;
        overridden.set_grid_color(Color::LIGHT_GRAY);
        assert_eq!(overridden.grid_color(), Color::LIGHT_GRAY);
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
        lc.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1, modifiers: 0 });
    }

    /// The widget must render its axes and ticks through the shared chart engine
    /// rather than its own copy of that math — the duplication this refactor
    /// removed.
    ///
    /// Observable proof: with labels enabled the engine emits numeric tick
    /// labels for both axes. They are `font8x8` glyph geometry now, so the string is not in the
    /// document and the check is on where the ink landed. `draw_y_ticks` anchors every label at
    /// `plot_x - 44 = 20` and `draw_x_ticks` spreads five labels across the plot, each clamped
    /// inside it — so the y-axis column and the x-axis row are two different bands, and a widget
    /// that drew neither (or drew one axis only) cannot produce both. The previous hand-rolled
    /// path drew them too, so this also guards that the migration kept that behaviour.
    ///
    /// Gated on the feature, like `ink_paths` and the module's other `chart`-dependent items:
    /// a build without `chart` compiles this test module and the engine it exercises is absent.
    #[cfg(feature = "chart")]
    #[test]
    fn line_chart_renders_axis_tick_labels_from_the_shared_engine() {
        // Holds the crate-wide theme guard: this test renders, and a concurrent
        // test that switches the appearance would otherwise change a later frame.
        let _theme_guard = crate::style::theme_test_guard();
        let mut lc = LineChart::new(Rect::new(0, 0, 300, 200));
        lc.set_data(vec![(0.0, 0.0), (10.0, 50.0)]);
        lc.set_axis_range(Some(0.0), Some(10.0), Some(0.0), Some(50.0));
        lc.set_show_labels(true);

        let svg = render_to_svg(&mut lc);
        let runs = ink_paths(&svg);

        // The Y axis: five labels anchored at box x = 20, and `0.0`/`50.0` are two characters of
        // ink wide where `25.0` is three. `draw_y_ticks` labels `min_y + span * t`, so the ticks
        // carry different values and therefore different ink. The anchor is where the glyph **box**
        // starts; the bitmap face's ink left *is* that edge, but an outline glyph is inset a pixel
        // or two, so the locator allows `INK_INSET_TOLERANCE` and nothing more.
        let y_axis: Vec<_> = runs
            .iter()
            .filter(|run| (run.0 - 20).abs() <= INK_INSET_TOLERANCE && run.2 > 20)
            .collect();
        assert_eq!(y_axis.len(), 5, "one y-tick label per tick: {y_axis:?}");
        // An outline's ink is the glyph's **real** width, which is narrower than the advance, so the
        // band is an upper bound rather than an equality: `25.0` / `50.0` are three characters wide
        // and cannot exceed the three advances they occupy.
        assert!(
            y_axis.iter().any(|run| run.2 - run.0 <= 17),
            "a `25.0` / `50.0` label is at most three characters of ink wide: {y_axis:?}"
        );
        assert!(
            y_axis.iter().all(|run| run.1 < 160),
            "every y label sits above the x-tick row, so the two axes are distinct: {y_axis:?}"
        );
        // The bounds differ, so the engine drew real *values* rather than one string repeated:
        // a tick loop that lost its `min_y + span * t` would paint five identical boxes.
        assert_ne!(y_axis[0], y_axis[4], "the y ticks carry different values: {y_axis:?}");

        // The X axis: five labels in a row below the plot, which is a line box top of 160 for this
        // geometry, and they reach past `plot_x` into the plot's own horizontal band. Under the
        // bitmap face the ink top *is* the line box top; under an outline face it is inset a pixel
        // or two, so the band is `160 ± INK_INSET_TOLERANCE` — tight enough that a label on the
        // y-axis row (160 px away) still fails.
        let x_axis: Vec<_> =
            runs.iter().filter(|run| (run.1 - 160).abs() <= INK_INSET_TOLERANCE).collect();
        assert_eq!(x_axis.len(), 5, "one x-tick label per tick: {x_axis:?}");
        assert!(
            x_axis.iter().all(|run| run.0 >= 64),
            "every x label is clamped inside the plot area: {x_axis:?}"
        );
        assert!(
            x_axis.iter().any(|run| run.0 > 200),
            "the labels are distributed along the axis, not stacked on one tick: {x_axis:?}"
        );
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

    /// N-CH-04: non-finite samples are dropped at the data entry.
    ///
    /// A `NaN` y-coordinate used to map through `f64 as i32` to pixel `0`, landing on the plot edge
    /// as if it were a real minimum, and a lone `+Inf` stretched the auto-range so every genuine
    /// point collapsed onto one pixel. Both are dropped instead.
    #[test]
    fn non_finite_points_are_dropped_at_entry() {
        let mut lc = LineChart::new(Rect::new(0, 0, 200, 100));
        lc.set_data(vec![
            (0.0, 1.0),
            (1.0, f64::NAN),
            (2.0, f64::INFINITY),
            (3.0, f64::NEG_INFINITY),
            (4.0, 2.0),
        ]);
        assert_eq!(lc.data().len(), 2, "only the two finite points survive");
        assert_eq!(lc.data()[0], (0.0, 1.0));
        assert_eq!(lc.data()[1], (4.0, 2.0));

        // `add_point` refuses a non-finite point too.
        let before = lc.data().len();
        lc.add_point(f64::NAN, 3.0);
        lc.add_point(3.0, f64::NAN);
        assert_eq!(lc.data().len(), before, "add_point must refuse non-finite input");
        // And a finite one is still accepted.
        lc.add_point(5.0, 5.0);
        assert_eq!(lc.data().len(), before + 1);
    }

    /// Several series are drawn and the legend reserves a column for their names.
    #[test]
    fn multi_series_draws_every_line_and_the_legend() {
        let mut lc = LineChart::new(Rect::new(0, 0, 400, 240));
        lc.set_series(vec![
            LineSeries::new("alpha", vec![(0.0, 1.0), (1.0, 2.0), (2.0, 3.0)]),
            LineSeries::new("beta", vec![(0.0, 3.0), (1.0, 1.0), (2.0, 2.0)])
                .with_color(Color::ERROR),
        ]);
        assert_eq!(lc.series().len(), 2);
        assert_eq!(lc.data().len(), 3, "series zero remains the primary data");
        lc.set_show_legend(true);
        let svg = crate::widget::svg::render_to_svg(&mut lc);
        let with_legend = crate::widget::svg::text_ink_boxes(&svg).len();
        lc.set_show_legend(false);
        let without_legend = crate::widget::svg::text_ink_boxes(&render_to_svg(&mut lc)).len();
        assert!(
            with_legend > without_legend,
            "the legend labels add text runs ({with_legend} vs {without_legend})"
        );
    }

    /// Zooming narrows the resolved x window and resetting restores the data range.
    #[test]
    fn zoom_narrows_the_window_and_reset_restores_it() {
        let mut lc = LineChart::new(Rect::new(0, 0, 300, 200));
        lc.set_data(vec![(0.0, 0.0), (10.0, 1.0)]);
        let (full_low, full_high) = lc.resolve_x_range();
        lc.set_zoom_enabled(true);
        assert!(lc.is_zoom_enabled());
        lc.set_view_range(Some((2.0, 4.0)));
        assert_eq!(lc.view_range(), Some((2.0, 4.0)));
        let (zoomed_low, zoomed_high) = lc.resolve_x_range();
        assert!(zoomed_high - zoomed_low < full_high - full_low);
        lc.reset_view();
        assert_eq!(lc.resolve_x_range(), (full_low, full_high));
    }

    /// Hovering over a plotted point is reported through the signals and the accessor.
    #[test]
    fn hovering_a_point_emits_the_hover_signals() {
        use std::sync::{Arc, Mutex};
        let mut lc = LineChart::new(Rect::new(0, 0, 300, 200));
        lc.set_data(vec![(0.0, 0.0), (1.0, 1.0), (2.0, 2.0)]);
        // Paint once so the hit-test uses the measured plot area.
        let _ = render_to_svg(&mut lc);
        let seen = Arc::new(Mutex::new(Vec::<(usize, usize)>::new()));
        let sink = Arc::clone(&seen);
        lc.point_hovered.connect(move |index| sink.lock().unwrap().push(*index));
        let mut points = Vec::new();
        LineChart::write_mapped_points(
            &lc.series[0].points,
            -0.2,
            2.2,
            -0.2,
            2.2,
            lc.last_plot_area.unwrap(),
            &mut points,
        );
        lc.handle_event(&Event::MouseMove { pos: points[2] });
        assert_eq!(lc.hovered_point(), Some((0, 2)));
        assert_eq!(*seen.lock().unwrap(), vec![(0, 2)]);
        lc.handle_event(&Event::MouseLeave { pos: Point::new(0, 0) });
        assert!(lc.hovered_point().is_none());
    }

    /// F-P12: the extents cache follows data mutations and the padded range is intact.
    #[test]
    fn extents_are_cached_and_follow_data_changes() {
        let mut lc = LineChart::new(Rect::new(0, 0, 400, 300));
        lc.set_data(vec![(0.0, 0.0), (10.0, 20.0)]);
        let (low, high) = lc.resolve_x_range();
        assert!((low - -1.0).abs() < 1e-9 && (high - 11.0).abs() < 1e-9, "0..10 padded by 10%");
        let (y_low, y_high) = lc.resolve_y_range();
        assert!((y_low - -2.0).abs() < 1e-9 && (y_high - 22.0).abs() < 1e-9);

        lc.add_point(20.0, 40.0);
        let (low, high) = lc.resolve_x_range();
        assert!(low < -1.0 && high > 11.0, "the cached extents were invalidated");
    }

    /// CH-06: with zoom on, a press that does not move is still a click on the point.
    #[test]
    fn a_click_without_movement_reaches_point_clicked_while_zoom_is_on() {
        let mut lc = LineChart::new(Rect::new(0, 0, 400, 300));
        lc.set_data(vec![(0.0, 0.0), (10.0, 10.0)]);
        lc.set_zoom_enabled(true);
        let _ = crate::widget::svg::render_to_svg(&mut lc);
        let plot = lc.last_plot_area.expect("the paint stored the plot area");
        let (x_min, x_max) = lc.resolve_x_range();
        let (y_min, y_max) = lc.resolve_y_range();
        let pixel = LineChart::map_to_pixel(10.0, 10.0, x_min, x_max, y_min, y_max, plot);

        let clicked = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = std::sync::Arc::clone(&clicked);
        lc.point_clicked.connect(move |point| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*point);
            }
        });
        lc.handle_event(&Event::MousePress { pos: pixel, button: 1, modifiers: 0 });
        lc.handle_event(&Event::MouseRelease { pos: pixel, button: 1 });
        assert_eq!(*clicked.lock().unwrap(), vec![(0, 1)], "the click was not swallowed by pan");
    }

    /// CH-05/CH-07: the tooltip adds a text run and the title/axis titles are painted.
    #[test]
    fn tooltip_series_name_and_titles_are_painted() {
        let mut lc = LineChart::new(Rect::new(0, 0, 400, 300));
        lc.set_series(vec![LineSeries::new("Revenue", vec![(0.0, 0.0), (10.0, 10.0)])]);
        lc.set_show_tooltip(true);
        lc.set_title("Quarterly");
        lc.set_x_axis_label("Month");
        lc.set_y_axis_label("USD");
        let before = crate::widget::svg::render_to_svg(&mut lc);
        let runs_without_tooltip = crate::widget::svg::text_ink_boxes(&before).len();
        assert!(runs_without_tooltip >= 3, "title and both axis titles are painted");

        let plot = lc.last_plot_area.expect("plot area");
        let (x_min, x_max) = lc.resolve_x_range();
        let (y_min, y_max) = lc.resolve_y_range();
        let pixel = LineChart::map_to_pixel(10.0, 10.0, x_min, x_max, y_min, y_max, plot);
        lc.handle_event(&Event::MouseMove { pos: pixel });
        let after = crate::widget::svg::render_to_svg(&mut lc);
        assert!(
            crate::widget::svg::text_ink_boxes(&after).len() > runs_without_tooltip,
            "the tooltip adds a text run"
        );
    }

    /// CH-001: clicking a legend row toggles the series and reports it.
    #[test]
    fn a_legend_click_toggles_the_series_it_names() {
        use std::sync::{Arc, Mutex};
        let mut lc = LineChart::new(Rect::new(0, 0, 500, 300));
        lc.set_series(vec![
            LineSeries::new("alpha", vec![(0.0, 1.0), (1.0, 2.0)]),
            LineSeries::new("beta", vec![(0.0, 2.0), (1.0, 1.0)]),
        ]);
        lc.set_show_legend(true);
        let _ = crate::widget::svg::render_to_svg(&mut lc);
        let area = lc.last_legend_area.expect("the legend has a hit area");
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&seen);
        lc.series_toggled.connect(move |value| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*value);
            }
        });
        lc.handle_event(&Event::mouse_press(area.x + 4, area.y + 4, 1));
        assert!(!lc.is_series_visible(0), "the clicked series is hidden");
        assert_eq!(*seen.lock().unwrap(), vec![(0, false)]);
        // The same row toggles it back.
        lc.handle_event(&Event::mouse_press(area.x + 4, area.y + 4, 1));
        assert!(lc.is_series_visible(0));
    }

    /// CH-008: the projection is rebuilt only when the view or the data changes.
    #[test]
    fn the_projection_cache_follows_the_view_revision() {
        let mut lc = LineChart::new(Rect::new(0, 0, 320, 220));
        lc.set_data(vec![(0.0, 1.0), (1.0, 2.0), (2.0, 3.0)]);
        let _ = crate::widget::svg::render_to_svg(&mut lc);
        let after_first = lc.projection_builds.get();
        let _ = crate::widget::svg::render_to_svg(&mut lc);
        assert_eq!(
            lc.projection_builds.get(),
            after_first,
            "an unchanged view reuses the cached projection"
        );
        lc.set_x_reversed(true);
        let _ = crate::widget::svg::render_to_svg(&mut lc);
        assert!(
            lc.projection_builds.get() > after_first,
            "a viewport revision invalidates the projection"
        );
    }

    /// CH-005: line updates ease by default and can be made immediate.
    #[test]
    fn line_value_changes_ease_by_default_and_can_be_disabled() {
        let mut lc = LineChart::new(Rect::new(0, 0, 320, 220));
        lc.set_data(vec![(0.0, 0.0), (1.0, 1.0)]);
        lc.set_data(vec![(0.0, 10.0), (1.0, 10.0)]);
        assert!(lc.is_animated());
        assert!(lc.displayed_points(0)[0].1 < 10.0, "the new value starts behind the target");
        assert!(lc.is_animating());
        let mut frames = 0;
        while lc.tick(16) && frames < 200 {
            frames += 1;
        }
        assert!((lc.displayed_points(0)[0].1 - 10.0).abs() < 1e-6);
        assert!(!lc.is_animating());

        lc.set_animate(false);
        lc.set_data(vec![(0.0, 20.0), (1.0, 20.0)]);
        assert!((lc.displayed_points(0)[0].1 - 20.0).abs() < 1e-9);
        assert!(!lc.is_animating());
    }

    /// CH-003/CH-004: logarithmic and reversed axes change the painted projection.
    #[test]
    fn logarithmic_and_reversed_axes_reach_the_projection() {
        let mut lc = LineChart::new(Rect::new(0, 0, 320, 220));
        lc.set_data(vec![(1.0, 1.0), (2.0, 100.0)]);
        let _ = crate::widget::svg::render_to_svg(&mut lc);
        let linear_y = lc.projection_series[0][1].y;
        lc.set_y_scale(AxisScale::Logarithmic);
        let _ = crate::widget::svg::render_to_svg(&mut lc);
        let log_y = lc.projection_series[0][1].y;
        assert_ne!(linear_y, log_y, "the log axis maps the same datum differently");
        lc.set_y_reversed(true);
        let _ = crate::widget::svg::render_to_svg(&mut lc);
        assert_ne!(lc.projection_series[0][1].y, log_y, "the reversed axis flips the mapping");
    }

    /// CH-010: an external read-out replaces the inline bubble and exposes its content.
    #[test]
    fn an_external_readout_owns_the_tooltip_presentation() {
        let mut lc = LineChart::new(Rect::new(0, 0, 400, 300));
        lc.set_series(vec![LineSeries::new("Revenue", vec![(0.0, 0.0), (10.0, 10.0)])]);
        lc.set_show_tooltip(true);
        lc.set_hover_readout_external(true);
        let before = crate::widget::svg::render_to_svg(&mut lc);
        let runs_without_tooltip = crate::widget::svg::text_ink_boxes(&before).len();
        let plot = lc.last_plot_area.expect("plot area");
        let (x_min, x_max) = lc.resolve_x_range();
        let (y_min, y_max) = lc.resolve_y_range();
        let pixel = LineChart::map_to_pixel(10.0, 10.0, x_min, x_max, y_min, y_max, plot);
        lc.handle_event(&Event::MouseMove { pos: pixel });
        let (anchor, content) = lc.hover_anchor().expect("the external host gets the read-out");
        assert_eq!(anchor, pixel);
        assert!(content[0].text.contains("Revenue"));
        let after = crate::widget::svg::render_to_svg(&mut lc);
        assert_eq!(
            crate::widget::svg::text_ink_boxes(&after).len(),
            runs_without_tooltip,
            "the chart no longer paints its own tooltip"
        );
    }
}
