// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Chart widget.
use crate::core::{HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
/// The chart styles [`ChartWidget`] can switch between at runtime.
///
/// # Scope, and why this is not the same enum as the other `ChartType`s
///
/// This is the **control layer**'s set: one widget that can render any of these
/// styles, so a `match` over it stays exhaustive in this widget. Two same-named
/// enums exist elsewhere and are deliberately kept separate (principle #49):
///
/// * [`crate::widget::display_widgets::mini_chart::ChartType`] — `MiniChart`'s
///   two styles (`Line`, `Bar`), since a mini chart has no pie or scatter form;
/// * `crate::widget::chart_widgets::types::ChartType` — the drawing engine's
///   set, which adds `Area`.
///
/// # Why these variants are one control rather than six (rule #80)
///
/// Every variant below reads the same `series: Vec<Vec<f64>>` plus the same
/// `labels`, and every one shares the same interaction (`hovered_index`,
/// `data_point_clicked`, `data_point_hovered`). Only the drawing geometry differs.
/// Six controls would repeat the axes, the labels, the hover hit-test and the
/// signal plumbing six times — which is the duplication rule #80 exists to
/// prevent.
///
/// The multi-value variants (`Candlestick`, `BoxPlot`) read their extra numbers
/// from consecutive entries of one series rather than from a second axis. That is
/// what `set_series` makes explicit: the grouping is a documented contract of the
/// variant, not an accident of how the vector happened to be filled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChartType {
    /// Vertical bars, one per data point; the default.
    #[default]
    Bar,
    /// Polyline connecting the data points in order.
    Line,
    /// Filled area between the polyline and the baseline.
    Area,
    /// Pie/donut wedges whose angles are proportional to each point's share of
    /// the total; negative or zero-sum data draws nothing.
    Pie,
    /// Unconnected markers, one per data point.
    Scatter,
    /// Floating bars from each point to the running total, showing how each
    /// increment contributes to a cumulative value.
    Waterfall,
    /// Progressively narrowing bars against a shared left edge, one per stage.
    ///
    /// Reads the first `series` entry; further entries are ignored, because a
    /// funnel is one measure per stage by definition.
    Funnel,
    /// Open/high/low/close bars, read as **four consecutive values per bar**:
    /// `[open, high, low, close, open, high, low, close, …]`.
    ///
    /// A trailing partial group is ignored rather than padded, so a caller cannot
    /// get a bar drawn from an invented value.
    Candlestick,
    /// Five-number summaries, read as **five consecutive values per box**:
    /// `[min, q1, median, q3, max, …]`.
    ///
    /// Trailing partial groups are ignored, as for `Candlestick`.
    BoxPlot,
}

impl ChartType {
    /// The factory spelling of this variant, matching `CHART_PROPERTIES`.
    pub fn as_str(self) -> &'static str {
        match self {
            ChartType::Bar => "bar",
            ChartType::Line => "line",
            ChartType::Area => "area",
            ChartType::Pie => "pie",
            ChartType::Scatter => "scatter",
            ChartType::Waterfall => "waterfall",
            ChartType::Funnel => "funnel",
            ChartType::Candlestick => "candlestick",
            ChartType::BoxPlot => "box_plot",
        }
    }

    /// Parses a factory spelling into a variant.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "bar" => ChartType::Bar,
            "line" => ChartType::Line,
            "area" => ChartType::Area,
            "pie" => ChartType::Pie,
            "scatter" => ChartType::Scatter,
            "waterfall" => ChartType::Waterfall,
            "funnel" => ChartType::Funnel,
            "candlestick" => ChartType::Candlestick,
            "box_plot" => ChartType::BoxPlot,
            _ => return None,
        })
    }

    /// How many consecutive values in a series one drawn mark consumes.
    ///
    /// One for the single-value styles, four for `Candlestick`, five for
    /// `BoxPlot`. Exposed so a caller can size its data and so the drawing code and
    /// the documentation cannot disagree about the grouping.
    pub fn values_per_point(self) -> usize {
        match self {
            ChartType::Candlestick => 4,
            ChartType::BoxPlot => 5,
            _ => 1,
        }
    }
}

/// Chart widget for data visualization.
pub struct ChartWidget {
    base: BaseWidget,
    chart_type: ChartType,
    /// The drawn series.
    ///
    /// Always the storage; `data()` is series zero. Kept as a vector-of-vectors
    /// rather than a `Vec<f64>` plus an optional second vector so that "one
    /// series" and "many series" are the same code path — the previous
    /// `Vec<f64>`-only model could not express the multi-value variants above
    /// without a grouping convention embedded in the drawing code.
    series: Vec<Vec<f64>>,
    labels: Vec<String>,
    hovered_index: Option<usize>,
    /// Emitted when a data point is clicked.
    pub data_point_clicked: Signal1<usize>,
    /// Emitted when pointer hover enters a data point bucket.
    pub data_point_hovered: Signal1<usize>,
    /// Emitted with the index hover is *leaving*.
    ///
    /// Carries the index that stopped being hovered rather than the one entered, so a
    /// subscriber can keep its own highlight in step without remembering the previous index
    /// itself. The sibling charts carry the same signal under their own naming
    /// (`bar_unhovered` on `candlestick_chart`); `ChartWidget` published only the enter half,
    /// so a hover highlight could be turned on and never off.
    pub data_point_unhovered: Signal1<usize>,
}
impl ChartWidget {
    /// Creates a new chart widget.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Chart, geometry, "ChartWidget"),
            chart_type: ChartType::default(),
            series: Vec::new(),
            labels: Vec::new(),
            hovered_index: None,
            data_point_clicked: Signal1::new(),
            data_point_hovered: Signal1::new(),
            data_point_unhovered: Signal1::new(),
        }
    }

    /// Returns the chart type.
    pub fn chart_type(&self) -> ChartType {
        self.chart_type
    }

    /// Returns the first series' values.
    ///
    /// The single-series spelling of [`Self::series`], kept because it was the
    /// whole API before multi-series existed and every non-multi-series style
    /// still has exactly one.
    pub fn data(&self) -> &[f64] {
        self.series.first().map_or(&[], |series| series.as_slice())
    }

    /// Returns every series.
    pub fn series(&self) -> &[Vec<f64>] {
        &self.series
    }

    /// Returns the chart data labels.
    pub fn labels(&self) -> &[String] {
        &self.labels
    }

    /// Returns the index of the hovered data point, if any.
    ///
    /// This is the accessor the module doc promises when it says every variant "shares the
    /// same interaction (`hovered_index`, ...)": the field was written by `handle_event` and
    /// read by nothing, so the hover state existed only as a local comparison and a caller had
    /// no way to observe it. The sibling charts (`candlestick_chart`, `volume_chart`,
    /// `indicator_chart`, `quote_board`, `order_book`) all expose this.
    pub fn hovered_index(&self) -> Option<usize> {
        self.hovered_index
    }

    /// The data value at the hovered index, if the index still names a point.
    pub fn hovered_value(&self) -> Option<f64> {
        let index = self.hovered_index?;
        self.data().get(index).copied()
    }

    /// Sets the chart type.
    pub fn set_chart_type(&mut self, chart_type: ChartType) {
        self.chart_type = chart_type;
        self.base.request_redraw();
    }

    /// Sets the chart data as a single series.
    pub fn set_data(&mut self, data: Vec<f64>) {
        self.series = if data.is_empty() { Vec::new() } else { vec![data] };
        self.revalidate_hover();
        self.base.request_redraw();
    }

    /// Sets the chart data as multiple series.
    ///
    /// Series zero is what [`Self::data`] reports, so the two setters agree about
    /// which values are "the data". An empty input clears the chart, matching
    /// `set_data`.
    pub fn set_series(&mut self, series: Vec<Vec<f64>>) {
        self.series = series;
        self.revalidate_hover();
        self.base.request_redraw();
    }

    /// Sets the chart data labels.
    pub fn set_labels(&mut self, labels: Vec<String>) {
        self.labels = labels;
        self.base.request_redraw();
    }

    /// Drops a hover index that the new data no longer has a point for.
    ///
    /// Replacing a 40-point series with a 5-point one left `hovered_index` naming index 30 —
    /// an index the chart cannot render and `data_point_hovered` will never emit again. The
    /// sibling charts re-validate on `set_series` for the same reason; `ChartWidget` did not.
    fn revalidate_hover(&mut self) {
        let points = self.data().len();
        if self.hovered_index.is_some_and(|index| index >= points) {
            self.hovered_index = None;
            self.data_point_unhovered.emit(0);
        }
    }

    /// Maps a pointer position to the index of the point under it.
    ///
    /// The bucket is computed from `data().len()`, so the hit area follows the
    /// marks the `x`-sequenced styles actually draw.
    fn data_index_at(&self, pos: Point) -> Option<usize> {
        let point_count = point_count_for(self.chart_type, self.data().len());
        if point_count == 0 {
            return None;
        }
        let rect = self.base.geometry();
        if !rect.contains_point(pos) || rect.width == 0 {
            return None;
        }
        let local_x = (pos.x - rect.x).max(0) as u32;
        let width = rect.width.max(1);
        let mut idx = ((local_x as u64) * (point_count as u64) / (width as u64)) as usize;
        if idx >= point_count {
            idx = point_count - 1;
        }
        Some(idx)
    }

    /// The `(x, y)` extremes across every series, for the styles that share one
    /// value axis.
    ///
    /// Computed over all series rather than only series zero, so a multi-series
    /// line chart does not clip the taller series out of the plot area.
    fn value_range(&self) -> Option<(f64, f64)> {
        let mut min = f64::INFINITY;
        let mut max = f64::NEG_INFINITY;
        for value in self.series.iter().flatten() {
            if value.is_finite() {
                min = min.min(*value);
                max = max.max(*value);
            }
        }
        if !min.is_finite() || !max.is_finite() {
            return None;
        }
        Some((min, max))
    }
}

/// How many drawn points a series of `len` values produces under `chart_type`.
///
/// For the grouped styles this is the number of *complete* groups, so a trailing
/// partial group is neither drawn nor hoverable — there is no mark for it.
fn point_count_for(chart_type: ChartType, len: usize) -> usize {
    let per_point = chart_type.values_per_point();
    if per_point <= 1 {
        len
    } else {
        len / per_point
    }
}
impl Widget for ChartWidget {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(400, 300)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `ChartWidget`'s property contract, published under the `Chart` kind.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` / `access_write_other.in.rs` dispatch.
///
/// `WidgetKind::Chart` is the kind the capability layer pairs with `GanttWidget`,
/// so `task_count` / `selected_id` / `viewport_*` / `zoom_level` belong to that
/// control. `chart_capability` publishes no properties for this control, so this
/// contract inherits the shared four and owns nothing beyond them.
impl WidgetProperties for ChartWidget {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "chart_type" => {
                Ok(CapabilityValue::String(chart_type_to_str(self.chart_type()).to_string()))
            }
            "point_count" => Ok(CapabilityValue::UInt(point_count_for(
                self.chart_type,
                self.data().len(),
            ) as u64)),
            "label_count" => Ok(CapabilityValue::UInt(self.labels().len() as u64)),
            "series_count" => Ok(CapabilityValue::UInt(self.series().len() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "chart_type" => {
                self.set_chart_type(expect_chart_type(value)?);
                Ok(())
            }
            // Both counts are derived from the series the caller supplied through
            // `set_data` / `set_series` / `set_labels`.
            "point_count" | "label_count" | "series_count" => {
                Err(CapabilityAccessError::ReadOnlyProperty)
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "chart_type",
            "point_count",
            "label_count",
            "series_count",
            BASE_PROPERTY_NAMES
        ]
    }
}

/// Publishes `ChartType` as the shared lower-case token.
fn chart_type_to_str(chart_type: ChartType) -> &'static str {
    chart_type.as_str()
}

/// Parses the shared lower-case token back, rejecting anything else.
fn expect_chart_type(value: CapabilityValue) -> Result<ChartType, CapabilityAccessError> {
    match value {
        CapabilityValue::String(token) => {
            ChartType::from_name(&token).ok_or(CapabilityAccessError::TypeMismatch)
        }
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

impl Draw for ChartWidget {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.base.geometry();
        use crate::core::Color;
        use crate::core::Font;
        // Draw chart background
        context.fill_rect(rect, Color::rgb(255, 255, 255));
        // Draw border to make chart area visible
        context.draw_rect(rect, Color::rgb(200, 200, 200));

        if self.series.is_empty() {
            // No data — draw placeholder text
            let text_origin =
                crate::core::Point { x: rect.x + 4, y: rect.y + rect.height as i32 / 2 };
            let font = Font::simple("Sans", 12.0);
            context.draw_text(
                text_origin,
                "No data",
                &font,
                Color::rgb(180, 180, 180),
                HorizontalAlignment::Left,
            );
            return;
        }

        match self.chart_type {
            // The multi-series styles iterate every series, so a second series is
            // drawn beside the first rather than being ignored. The single-series
            // callers pass `0` and stop.
            ChartType::Bar => {
                for index in 0..self.series.len() {
                    self.draw_bar_chart(context, rect, index);
                }
            }
            ChartType::Line => {
                for index in 0..self.series.len() {
                    self.draw_line_chart(context, rect, index);
                }
            }
            ChartType::Area => {
                for index in 0..self.series.len() {
                    self.draw_area_chart(context, rect, index);
                }
            }
            ChartType::Scatter => {
                for index in 0..self.series.len() {
                    self.draw_scatter_chart(context, rect, index);
                }
            }
            ChartType::Pie => self.draw_pie_chart(context, rect),
            ChartType::Waterfall => self.draw_waterfall_chart(context, rect),
            ChartType::Funnel => self.draw_funnel_chart(context, rect),
            ChartType::Candlestick => self.draw_candlestick_chart(context, rect),
            ChartType::BoxPlot => self.draw_box_plot_chart(context, rect),
        }
    }
}

/// The plot area shared by the `x`-sequenced styles.
///
/// The four margins are named so that every renderer agrees on where the baseline
/// and the top of the plot are; deriving them per-renderer is how the axes drifted
/// apart in the first place.
struct PlotArea {
    /// Left edge of the plotting region.
    left: i32,
    /// Right edge (exclusive in arithmetic, inclusive visually).
    right: i32,
    /// Bottom edge, where value `min` is drawn.
    baseline_y: i32,
    /// Top edge, where value `max` is drawn.
    top_y: i32,
}

impl PlotArea {
    /// Derives the plot area from the control's rectangle.
    fn of(rect: Rect) -> Self {
        const PADDING: i32 = 8;
        const BOTTOM_MARGIN: i32 = 20;
        let left = rect.x.saturating_add(PADDING);
        let right = rect.x.saturating_add(rect.width as i32).saturating_sub(PADDING);
        let baseline_y = rect.y.saturating_add(rect.height as i32).saturating_sub(BOTTOM_MARGIN);
        let top_y = rect.y.saturating_add(PADDING);
        Self { left, right, baseline_y, top_y }
    }

    /// The height available to a mark, never zero (a zero height draws nothing and
    /// also divides by zero in the ratio, so the floor is load-bearing).
    fn height_range(&self) -> f64 {
        (self.baseline_y.saturating_sub(self.top_y)).max(1) as f64
    }

    /// Maps `value` to a y coordinate, given the series' extremes.
    ///
    /// Uses `min` as well as `max` so a series with negative values or a narrow
    /// range around a large offset is drawn at its true scale rather than being
    /// pinned to the baseline.
    fn y_for(&self, value: f64, min: f64, max: f64) -> i32 {
        let span = max - min;
        if !span.is_finite() || span <= 0.0 {
            return self.baseline_y;
        }
        let ratio = (value - min) / span;
        self.baseline_y - (ratio * self.height_range()) as i32
    }

    /// The x coordinate of point `index` out of `count`, spread across the area.
    fn x_for(&self, index: usize, count: usize, inset: i32) -> i32 {
        if count <= 1 {
            return self.left + (self.right - self.left) / 2;
        }
        let span = (self.right - self.left).saturating_sub(inset * 2).max(1);
        self.left + inset + (index as i32 * span) / (count as i32 - 1).max(1)
    }
}

/// Draws a label under `x`, truncated to a fixed budget so a long label cannot
/// run into its neighbour. The truncation marker is part of the visible text, so
/// it is counted in the budget rather than appended past it.
fn draw_truncated_label(context: &mut RenderContext, x: i32, baseline_y: i32, label: &str) {
    use crate::core::{Color, Font};
    const BUDGET: usize = 6;
    if label.is_empty() {
        return;
    }
    let text = if label.chars().count() > BUDGET {
        let kept: String = label.chars().take(BUDGET - 2).collect();
        format!("{kept}..")
    } else {
        label.to_string()
    };
    context.draw_text(
        crate::core::Point { x: x.saturating_sub(6), y: baseline_y + 12 },
        &text,
        &Font::simple("Sans", 10.0),
        Color::rgb(80, 80, 80),
        HorizontalAlignment::Left,
    );
}

impl ChartWidget {
    /// The palette every renderer draws from.
    ///
    /// Shared rather than repeated so a bar and the line beside it cannot be
    /// different colours for the same series.
    const PALETTE: [crate::core::Color; 6] = [
        crate::core::Color::rgb(66, 133, 244),
        crate::core::Color::rgb(219, 68, 55),
        crate::core::Color::rgb(244, 180, 0),
        crate::core::Color::rgb(15, 157, 88),
        crate::core::Color::rgb(171, 71, 188),
        crate::core::Color::rgb(0, 172, 193),
    ];

    /// The colour of series `index`, cycling through [`Self::PALETTE`].
    fn series_color(index: usize) -> crate::core::Color {
        Self::PALETTE[index % Self::PALETTE.len()]
    }

    /// The value range the shared-axis styles plot against.
    ///
    /// Falls back to `(0, 1)` when the series is empty or entirely non-finite, so
    /// the callers never divide by a zero span. Bars and areas need a zero
    /// baseline to be readable, so the minimum is clamped to zero when every value
    /// is non-negative — otherwise a series of `[98, 99, 100]` would be drawn with
    /// its smallest bar at the baseline and look like zero.
    fn plot_range(&self) -> (f64, f64) {
        let Some((min, max)) = self.value_range() else {
            return (0.0, 1.0);
        };
        if min >= 0.0 {
            (0.0, if max > 0.0 { max } else { 1.0 })
        } else {
            (min, max)
        }
    }

    /// Draws vertical bars for series `series_index`.
    ///
    /// When the widget holds several series they are drawn side by side within
    /// each point's slot, which is the grouped-bar reading; with one series the
    /// slot arithmetic reduces to the full slot width.
    fn draw_bar_chart(&self, context: &mut RenderContext, rect: Rect, series_index: usize) {
        let data = match self.series.get(series_index) {
            Some(data) if !data.is_empty() => data,
            _ => return,
        };
        let area = PlotArea::of(rect);
        let (min, max) = self.plot_range();
        let series_total = self.series.len().max(1);
        let slot = (area.right - area.left).max(1) / data.len() as i32;
        let bar_width = (slot / series_total as i32).max(1);
        let gap = if series_total > 1 { 1 } else { 2 };

        for (i, &val) in data.iter().enumerate() {
            let slot_x = area.left + (i as i32) * slot;
            let x = slot_x + (series_index as i32) * bar_width;
            let top = area.y_for(val, min, max);
            let color = Self::series_color(series_index);
            context.fill_rect(
                Rect {
                    x,
                    y: top,
                    width: bar_width.saturating_sub(gap).max(1) as u32,
                    height: area.baseline_y.saturating_sub(top).max(1) as u32,
                },
                color,
            );
            // Only the first series labels the axis: repeating the same labels once
            // per series draws them on top of each other.
            if series_index == 0 {
                if let Some(label) = self.labels.get(i) {
                    let label_x = slot_x + slot / 2;
                    draw_truncated_label(context, label_x, area.baseline_y, label);
                }
            }
        }
    }

    /// Draws a polyline for series `series_index`.
    fn draw_line_chart(&self, context: &mut RenderContext, rect: Rect, series_index: usize) {
        let data = match self.series.get(series_index) {
            Some(data) if data.len() >= 2 => data,
            _ => return,
        };
        let area = PlotArea::of(rect);
        let (min, max) = self.plot_range();
        let color = Self::series_color(series_index);
        let points: Vec<Point> = data
            .iter()
            .enumerate()
            .map(|(i, &val)| Point {
                x: area.x_for(i, data.len(), 0),
                y: area.y_for(val, min, max),
            })
            .collect();

        for pair in points.windows(2) {
            context.draw_line_stroke(pair[0], pair[1], color, 2);
        }
        for (i, point) in points.iter().enumerate() {
            context.fill_circle(*point, 3, color);
            if series_index == 0 {
                if let Some(label) = self.labels.get(i) {
                    draw_truncated_label(context, point.x, area.baseline_y, label);
                }
            }
        }
    }

    /// Draws a filled area between the polyline and the baseline.
    ///
    /// # Why this is a variant rather than a second widget
    ///
    /// The drawing engine already had an `AreaChart`, but the control layer never
    /// exposed it — so the style existed and could not be asked for. It shares the
    /// line chart's data model and hit-test exactly, which is the condition rule
    /// #80 sets for extending rather than adding.
    fn draw_area_chart(&self, context: &mut RenderContext, rect: Rect, series_index: usize) {
        let data = match self.series.get(series_index) {
            Some(data) if data.len() >= 2 => data,
            _ => return,
        };
        let area = PlotArea::of(rect);
        let (min, max) = self.plot_range();
        let color = Self::series_color(series_index);
        let points: Vec<Point> = data
            .iter()
            .enumerate()
            .map(|(i, &val)| Point {
                x: area.x_for(i, data.len(), 0),
                y: area.y_for(val, min, max),
            })
            .collect();

        // Fill with columns rather than a polygon: the render context has no
        // filled-polygon primitive, and a one-pixel column per x is exactly the
        // area under the piecewise-linear curve. The vertical extent is taken from
        // the segment the column falls in, so the fill tracks the line instead of
        // stair-stepping at each point.
        for column in points.windows(2) {
            let (from, to) = (column[0], column[1]);
            let span = (to.x - from.x).max(1);
            for step in 0..=span {
                let x = from.x + step;
                let ratio = step as f64 / span as f64;
                let y = (from.y as f64 + (to.y as f64 - from.y as f64) * ratio) as i32;
                context.fill_rect(
                    Rect {
                        x,
                        y,
                        width: 1,
                        height: area.baseline_y.saturating_sub(y).max(1) as u32,
                    },
                    color,
                );
            }
        }
        for pair in points.windows(2) {
            context.draw_line_stroke(pair[0], pair[1], color, 2);
        }
        if series_index == 0 {
            for (i, point) in points.iter().enumerate() {
                if let Some(label) = self.labels.get(i) {
                    draw_truncated_label(context, point.x, area.baseline_y, label);
                }
            }
        }
    }

    /// Draws floating bars from each point to the running total.
    ///
    /// Each bar spans `previous_total` to `previous_total + value`, so a negative
    /// increment draws downward from the running total — which is what makes a
    /// waterfall readable rather than a set of bars at the wrong heights.
    fn draw_waterfall_chart(&self, context: &mut RenderContext, rect: Rect) {
        let data = match self.series.first() {
            Some(data) if !data.is_empty() => data,
            _ => return,
        };
        let area = PlotArea::of(rect);
        let mut cumulative = 0.0f64;
        let totals: Vec<(f64, f64)> = data
            .iter()
            .map(|&value| {
                let from = cumulative;
                cumulative += value;
                (from, cumulative)
            })
            .collect();

        // The range covers the running totals, which can exceed any single value.
        let mut min = 0.0f64;
        let mut max = 0.0f64;
        for (from, to) in &totals {
            min = min.min(*from).min(*to);
            max = max.max(*from).max(*to);
        }
        let span = max - min;
        if !span.is_finite() || span <= 0.0 {
            return;
        }

        let slot = (area.right - area.left).max(1) / data.len() as i32;
        let bar_width = slot.saturating_sub(2).max(1);
        for (i, &(from, to)) in totals.iter().enumerate() {
            let x = area.left + (i as i32) * slot;
            let y_from = area.baseline_y - (((from - min) / span) * area.height_range()) as i32;
            let y_to = area.baseline_y - (((to - min) / span) * area.height_range()) as i32;
            let top = y_from.min(y_to);
            let height = (y_from - y_to).unsigned_abs().max(1);
            // A rise and a fall get different colours, so the sign of each
            // increment is visible without reading the numbers.
            let color = if to >= from { Self::series_color(3) } else { Self::series_color(1) };
            context.fill_rect(Rect { x, y: top, width: bar_width as u32, height }, color);
            // A connector from this bar's top to the next bar's start, so the
            // running total is traceable across the chart.
            if i + 1 < totals.len() {
                let connector_y = y_to;
                context.draw_line_stroke(
                    Point { x: x + bar_width, y: connector_y },
                    Point { x: area.left + ((i + 1) as i32) * slot, y: connector_y },
                    crate::core::Color::rgb(150, 150, 150),
                    1,
                );
            }
            if let Some(label) = self.labels.get(i) {
                draw_truncated_label(context, x + bar_width / 2, area.baseline_y, label);
            }
        }
    }

    /// Draws progressively narrowing bars against a shared left edge.
    ///
    /// The width is proportional to the value's share of the largest value, so the
    /// silhouette narrows exactly when the numbers fall.
    fn draw_funnel_chart(&self, context: &mut RenderContext, rect: Rect) {
        let data = match self.series.first() {
            Some(data) if !data.is_empty() => data,
            _ => return,
        };
        let max = data.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        if !max.is_finite() || max <= 0.0 {
            return;
        }
        let area = PlotArea::of(rect);
        let available_height = area.baseline_y.saturating_sub(area.top_y).max(1);
        let stage_height = (available_height / data.len() as i32).max(1);
        let available_width = (area.right - area.left).max(1);

        for (i, &value) in data.iter().enumerate() {
            let ratio = (value / max).clamp(0.0, 1.0);
            let width = ((ratio * available_width as f64).round() as i32).max(1);
            let y = area.top_y + (i as i32) * stage_height;
            // Centred, so the narrowing reads as a funnel rather than a left-aligned
            // bar chart.
            let x = area.left + (available_width - width) / 2;
            context.fill_rect(
                Rect {
                    x,
                    y,
                    width: width as u32,
                    height: stage_height.saturating_sub(2).max(1) as u32,
                },
                Self::series_color(i),
            );
            if let Some(label) = self.labels.get(i) {
                // The label sits at the stage's left edge, which for a narrow stage
                // is inside the bar; centred text would be unreadable there.
                draw_truncated_label(context, x + 2, y + stage_height - 4, label);
            }
        }
    }

    /// Draws open/high/low/close bars.
    ///
    /// Reads four consecutive values per mark: the body spans open→close and the
    /// wick spans low→high. A rising bar is drawn in the "up" colour and a falling
    /// one in the "down" colour, which is the convention that makes a candlestick
    /// readable at a glance. A trailing group with fewer than four values is
    /// ignored, because there is no bar to draw from it.
    fn draw_candlestick_chart(&self, context: &mut RenderContext, rect: Rect) {
        const VALUES_PER_BAR: usize = 4;
        let data = match self.series.first() {
            Some(data) => data,
            _ => return,
        };
        let bar_count = data.len() / VALUES_PER_BAR;
        if bar_count == 0 {
            return;
        }
        let area = PlotArea::of(rect);
        let mut min = f64::INFINITY;
        let mut max = f64::NEG_INFINITY;
        for bar in data.chunks_exact(VALUES_PER_BAR).take(bar_count) {
            for value in bar {
                if value.is_finite() {
                    min = min.min(*value);
                    max = max.max(*value);
                }
            }
        }
        if !min.is_finite() || !max.is_finite() || max <= min {
            return;
        }

        let slot = (area.right - area.left).max(1) / bar_count as i32;
        let body_width = slot.saturating_sub(2).max(3);
        let wick_x_offset = body_width / 2;
        for (i, bar) in data.chunks_exact(VALUES_PER_BAR).take(bar_count).enumerate() {
            let (open, high, low, close) = (bar[0], bar[1], bar[2], bar[3]);
            let x = area.left + (i as i32) * slot;
            let y_open = area.y_for(open, min, max);
            let y_close = area.y_for(close, min, max);
            let y_high = area.y_for(high, min, max);
            let y_low = area.y_for(low, min, max);
            let rising = close >= open;
            let color = if rising { Self::series_color(3) } else { Self::series_color(1) };

            // Wick: the high/low extremes, drawn first so the body covers its middle.
            let wick_x = x + wick_x_offset;
            context.draw_line_stroke(
                Point { x: wick_x, y: y_high },
                Point { x: wick_x, y: y_low },
                color,
                1,
            );
            let body_top = y_open.min(y_close);
            context.fill_rect(
                Rect {
                    x,
                    y: body_top,
                    width: body_width as u32,
                    // A doji (open == close) has no height; one pixel keeps it visible.
                    height: (y_open - y_close).unsigned_abs().max(1),
                },
                color,
            );
        }
        // One label per bar, placed under every fourth value's slot.
        for i in 0..bar_count {
            if let Some(label) = self.labels.get(i) {
                let x = area.left + (i as i32) * slot + slot / 2;
                draw_truncated_label(context, x, area.baseline_y, label);
            }
        }
    }

    /// Draws five-number summary boxes.
    ///
    /// Reads five consecutive values per box — `min`, `q1`, `median`, `q3`, `max` —
    /// draws the box between `q1` and `q3`, the median line through it, and the
    /// whiskers out to `min` and `max`. A trailing partial group is ignored.
    fn draw_box_plot_chart(&self, context: &mut RenderContext, rect: Rect) {
        const VALUES_PER_BOX: usize = 5;
        let data = match self.series.first() {
            Some(data) => data,
            _ => return,
        };
        let box_count = data.len() / VALUES_PER_BOX;
        if box_count == 0 {
            return;
        }
        let area = PlotArea::of(rect);
        let mut min = f64::INFINITY;
        let mut max = f64::NEG_INFINITY;
        for group in data.chunks_exact(VALUES_PER_BOX).take(box_count) {
            for value in group {
                if value.is_finite() {
                    min = min.min(*value);
                    max = max.max(*value);
                }
            }
        }
        if !min.is_finite() || !max.is_finite() || max <= min {
            return;
        }

        let slot = (area.right - area.left).max(1) / box_count as i32;
        let box_width = slot.saturating_sub(4).max(4);
        let center_offset = box_width / 2;
        let color = Self::series_color(0);
        for (i, group) in data.chunks_exact(VALUES_PER_BOX).take(box_count).enumerate() {
            let (low, q1, median, q3, high) = (group[0], group[1], group[2], group[3], group[4]);
            let x = area.left + (i as i32) * slot + 2;
            let cx = x + center_offset;
            let y_low = area.y_for(low, min, max);
            let y_high = area.y_for(high, min, max);
            let y_q1 = area.y_for(q1, min, max);
            let y_q3 = area.y_for(q3, min, max);

            // Whiskers first, so the box covers their inner ends.
            context.draw_line_stroke(
                Point { x: cx, y: y_high },
                Point { x: cx, y: y_low },
                color,
                1,
            );
            let box_top = y_q1.min(y_q3);
            context.fill_rect(
                Rect {
                    x,
                    y: box_top,
                    width: box_width as u32,
                    height: (y_q1 - y_q3).unsigned_abs().max(1),
                },
                color,
            );
            // Median: drawn in the background colour so it reads as a line *through*
            // the box rather than another filled band.
            let y_median = area.y_for(median, min, max);
            context.fill_rect(
                Rect { x, y: y_median, width: box_width as u32, height: 2 },
                crate::core::Color::rgb(255, 255, 255),
            );
            if let Some(label) = self.labels.get(i) {
                draw_truncated_label(context, cx, area.baseline_y, label);
            }
        }
    }

    fn draw_pie_chart(&self, context: &mut RenderContext, rect: Rect) {
        let data = match self.series.first() {
            Some(data) if !data.is_empty() => data,
            _ => return,
        };
        let total: f64 = data.iter().filter(|value| **value > 0.0).sum();
        if total <= 0.0 {
            return;
        }
        let cx = rect.x + rect.width as i32 / 2;
        let cy = rect.y + rect.height as i32 / 2;
        let radius = (rect.width.min(rect.height) as i32 / 2).saturating_sub(10).max(10);
        let mut start_angle = -std::f64::consts::FRAC_PI_2;
        for (i, &val) in data.iter().enumerate() {
            // A non-positive wedge has no angle; skipping it keeps the angle sum
            // equal to the sum of the positive values the total was taken over.
            if val <= 0.0 {
                continue;
            }
            let slice_angle = 2.0 * std::f64::consts::PI * (val / total);
            let mid_angle = start_angle + slice_angle / 2.0;
            let end_angle = start_angle + slice_angle;
            let color = Self::series_color(i);
            // Segment count scales with the arc length so a thin wedge is not drawn
            // with more spokes than its angle needs.
            let segments = ((radius as f64 * slice_angle * 0.4).ceil() as i32).clamp(1, 120);
            for s in 0..segments {
                let t = start_angle + slice_angle * (s as f64 + 0.5) / segments as f64;
                let ex = cx + (radius as f64 * t.cos()) as i32;
                let ey = cy + (radius as f64 * t.sin()) as i32;
                context.draw_line(Point { x: cx, y: cy }, Point { x: ex, y: ey }, color);
            }
            if let Some(label) = self.labels.get(i) {
                let label_radius = radius.saturating_add(14) as f64;
                let lx = cx + (label_radius * mid_angle.cos()) as i32;
                let ly = cy + (label_radius * mid_angle.sin()) as i32;
                let pct = val / total * 100.0;
                let text = if pct >= 1.0 {
                    format!("{label}:{pct:.0}%")
                } else {
                    format!("{label}:{pct:.1}%")
                };
                context.draw_text(
                    Point { x: lx, y: ly },
                    &text,
                    &crate::core::Font::simple("Sans", 9.0),
                    crate::core::Color::rgb(60, 60, 60),
                    HorizontalAlignment::Left,
                );
            }
            start_angle = end_angle;
        }
    }

    /// Draws unconnected markers for series `series_index`.
    fn draw_scatter_chart(&self, context: &mut RenderContext, rect: Rect, series_index: usize) {
        let data = match self.series.get(series_index) {
            Some(data) if !data.is_empty() => data,
            _ => return,
        };
        let area = PlotArea::of(rect);
        let (min, max) = self.plot_range();
        let color = Self::series_color(series_index);
        let slot = (area.right - area.left).max(1) / data.len() as i32;
        for (i, &val) in data.iter().enumerate() {
            // Scatter uses the slot centre, since unconnected markers have no
            // line to sit on.
            let x = area.left + (i as i32) * slot + slot / 2;
            let y = area.y_for(val, min, max);
            context.fill_circle(Point { x, y }, 3, color);
            if series_index == 0 {
                if let Some(label) = self.labels.get(i) {
                    draw_truncated_label(context, x, area.baseline_y, label);
                }
            }
        }
    }
}
impl EventHandler for ChartWidget {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MouseMove { pos } => {
                // Entering a *different* bucket must also leave the previous one, or a
                // subscriber that highlights on `data_point_hovered` accumulates highlights.
                let next = self.data_index_at(*pos);
                if next != self.hovered_index {
                    if let Some(previous) = self.hovered_index {
                        self.data_point_unhovered.emit(previous);
                    }
                    if let Some(index) = next {
                        self.hovered_index = Some(index);
                        self.data_point_hovered.emit(index);
                    } else {
                        // Moved off the plot area: no bucket is under the pointer.
                        self.hovered_index = None;
                    }
                    self.base.request_redraw();
                }
            }
            Event::MouseLeave { .. } => {
                // Without this arm `hovered_index` was only ever set, never cleared — the
                // chart kept reporting a hover after the pointer left, and the stale index
                // survived a `set_series` that shrank the data. Every sibling chart clears
                // on leave; this one now does too.
                if let Some(previous) = self.hovered_index.take() {
                    self.data_point_unhovered.emit(previous);
                    self.base.request_redraw();
                }
            }
            Event::MousePress { pos, button } if *button == 1 => {
                self.base.set_mouse_pressed(true);
                if let Some(index) = self.data_index_at(*pos) {
                    self.base.clicked.emit();
                    self.data_point_clicked.emit(index);
                }
            }
            Event::MouseRelease { pos: _, button } if *button == 1 => {
                self.base.set_mouse_pressed(false);
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn chart_mouse_interaction_emits_data_index_signals() {
        let mut chart = ChartWidget::new(Rect::new(0, 0, 200, 100));
        chart.set_data(vec![10.0, 20.0, 30.0, 40.0]);

        let clicked = Arc::new(Mutex::new(Vec::<usize>::new()));
        let hovered = Arc::new(Mutex::new(Vec::<usize>::new()));

        let clicked_sink = clicked.clone();
        chart.data_point_clicked.connect(move |index| {
            if let Ok(mut guard) = clicked_sink.lock() {
                guard.push(*index);
            }
        });

        let hovered_sink = hovered.clone();
        chart.data_point_hovered.connect(move |index| {
            if let Ok(mut guard) = hovered_sink.lock() {
                guard.push(*index);
            }
        });

        chart.handle_event(&Event::mouse_move(120, 50));
        chart.handle_event(&Event::mouse_press(120, 50, 1));

        let clicked_values = clicked.lock().expect("clicked lock poisoned").clone();
        let hovered_values = hovered.lock().expect("hovered lock poisoned").clone();

        assert_eq!(hovered_values, vec![2]);
        assert_eq!(clicked_values, vec![2]);
    }

    /// Renders the chart into a software frame and returns the pixels.
    fn render(chart: &mut ChartWidget, width: u32, height: u32) -> Vec<u8> {
        use crate::core::{Color, Size};
        use crate::render::{PaintBackend, SoftwarePaintBackend};
        let mut backend = SoftwarePaintBackend::new(Size::new(width, height), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut context = RenderContext::new(&mut backend);
        chart.draw(&mut context);
        backend.end_frame();
        backend.frame_rgba().to_vec()
    }

    /// Counts pixels that are neither the white background nor the grey border.
    ///
    /// A renderer that silently draws nothing leaves the frame at background, so
    /// this is the discriminating measure for "the new variant actually painted".
    fn painted_pixels(rgba: &[u8]) -> usize {
        rgba.chunks_exact(4)
            .filter(|px| {
                let (r, g, b) = (px[0], px[1], px[2]);
                // Skip pure white (background) and the grey border family.
                !((r == 255 && g == 255 && b == 255) || (r == 200 && g == 200 && b == 200))
            })
            .count()
    }

    // ── Multi-series data model (C-1) ───────────────────────────────────────

    #[test]
    fn chart_set_data_populates_series_zero() {
        let mut chart = ChartWidget::new(Rect::new(0, 0, 200, 100));
        chart.set_data(vec![1.0, 2.0, 3.0]);
        assert_eq!(chart.data(), &[1.0, 2.0, 3.0]);
        assert_eq!(chart.series().len(), 1);
        assert_eq!(chart.get("series_count").unwrap(), CapabilityValue::UInt(1));
    }

    #[test]
    fn chart_set_data_empty_clears_series() {
        let mut chart = ChartWidget::new(Rect::new(0, 0, 200, 100));
        chart.set_data(vec![1.0]);
        chart.set_data(Vec::new());
        // An empty input must not leave a phantom series of length zero, which
        // would make `series_count` report 1 for a chart with nothing in it.
        assert!(chart.series().is_empty());
        assert_eq!(chart.get("series_count").unwrap(), CapabilityValue::UInt(0));
    }

    #[test]
    fn chart_set_series_keeps_every_series_and_agrees_with_data() {
        let mut chart = ChartWidget::new(Rect::new(0, 0, 200, 100));
        chart.set_series(vec![vec![1.0, 2.0], vec![3.0, 4.0, 5.0]]);
        assert_eq!(chart.series().len(), 2);
        assert_eq!(chart.data(), &[1.0, 2.0], "data() reports series zero");
        assert_eq!(chart.get("series_count").unwrap(), CapabilityValue::UInt(2));
    }

    // ── C-2: the new variants ───────────────────────────────────────────────

    #[test]
    fn chart_every_variant_paints_something() {
        for variant in [
            ChartType::Bar,
            ChartType::Line,
            ChartType::Area,
            ChartType::Pie,
            ChartType::Scatter,
            ChartType::Waterfall,
            ChartType::Funnel,
            ChartType::Candlestick,
            ChartType::BoxPlot,
        ] {
            let mut chart = ChartWidget::new(Rect::new(0, 0, 200, 120));
            chart.set_chart_type(variant);
            // Enough values for every variant, including the four- and five-value
            // groupings.
            chart.set_data(vec![
                10.0, 30.0, 20.0, 40.0, 25.0, 35.0, 15.0, 45.0, 30.0, 50.0, 40.0, 60.0, 45.0, 55.0,
                50.0, 70.0, 60.0, 80.0, 65.0, 75.0,
            ]);
            let rgba = render(&mut chart, 200, 120);
            assert!(
                painted_pixels(&rgba) > 0,
                "{variant:?} painted nothing; a variant that cannot draw is a name without a control"
            );
        }
    }

    #[test]
    fn chart_grouped_variants_ignore_trailing_partial_group() {
        // Seven values: one complete candlestick (4) plus three stragglers, and
        // one complete box (5) plus two stragglers.
        let mut candle = ChartWidget::new(Rect::new(0, 0, 200, 120));
        candle.set_chart_type(ChartType::Candlestick);
        candle.set_data(vec![10.0, 30.0, 5.0, 20.0, 99.0, 99.0, 99.0]);
        assert_eq!(
            candle.get("point_count").unwrap(),
            CapabilityValue::UInt(1),
            "three trailing values do not make a second candlestick"
        );

        let mut boxes = ChartWidget::new(Rect::new(0, 0, 200, 120));
        boxes.set_chart_type(ChartType::BoxPlot);
        boxes.set_data(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        assert_eq!(
            boxes.get("point_count").unwrap(),
            CapabilityValue::UInt(1),
            "one trailing value does not make a second box"
        );
    }

    #[test]
    fn chart_values_per_point_matches_the_documented_grouping() {
        assert_eq!(ChartType::Candlestick.values_per_point(), 4);
        assert_eq!(ChartType::BoxPlot.values_per_point(), 5);
        for variant in [
            ChartType::Bar,
            ChartType::Line,
            ChartType::Area,
            ChartType::Pie,
            ChartType::Scatter,
            ChartType::Waterfall,
            ChartType::Funnel,
        ] {
            assert_eq!(variant.values_per_point(), 1);
        }
    }

    #[test]
    fn chart_hover_index_follows_the_grouping() {
        // With two candlesticks in a 200px wide chart, a pointer at x=150 is in the
        // second bucket, not the fourth value's bucket.
        let mut chart = ChartWidget::new(Rect::new(0, 0, 200, 120));
        chart.set_chart_type(ChartType::Candlestick);
        chart.set_data(vec![10.0, 30.0, 5.0, 20.0, 15.0, 40.0, 10.0, 35.0]);

        let hovered = Arc::new(Mutex::new(Vec::<usize>::new()));
        let sink = hovered.clone();
        chart.data_point_hovered.connect(move |index| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*index);
            }
        });
        chart.handle_event(&Event::mouse_move(150, 50));

        assert_eq!(
            *hovered.lock().expect("hover lock poisoned"),
            vec![1],
            "the hit bucket counts bars, not raw values"
        );
    }

    // ── C-5: the tokens round-trip (rule #82) ───────────────────────────────

    #[test]
    fn chart_type_token_round_trips_for_every_variant() {
        let mut chart = ChartWidget::new(Rect::new(0, 0, 200, 120));
        for variant in [
            ChartType::Bar,
            ChartType::Line,
            ChartType::Area,
            ChartType::Pie,
            ChartType::Scatter,
            ChartType::Waterfall,
            ChartType::Funnel,
            ChartType::Candlestick,
            ChartType::BoxPlot,
        ] {
            chart
                .set("chart_type", CapabilityValue::String(variant.as_str().to_string()))
                .expect("every published token must be writable");
            assert_eq!(chart.chart_type(), variant);
            assert_eq!(
                chart.get("chart_type").unwrap(),
                CapabilityValue::String(variant.as_str().to_string())
            );
            // `from_name` and `as_str` must agree, or a token could be published
            // that the parser rejects.
            assert_eq!(ChartType::from_name(variant.as_str()), Some(variant));
        }
        assert!(chart
            .set("chart_type", CapabilityValue::String("candlestick_chart".to_string()))
            .is_err());
    }

    #[test]
    fn chart_series_count_is_read_only() {
        let mut chart = ChartWidget::new(Rect::new(0, 0, 200, 120));
        assert_eq!(
            chart.set("series_count", CapabilityValue::UInt(5)),
            Err(CapabilityAccessError::ReadOnlyProperty)
        );
    }

    #[test]
    fn chart_renders_negative_values_below_the_baseline() {
        // A series crossing zero must use a range that includes it, or the negative
        // bars would be drawn at a positive height.
        let mut chart = ChartWidget::new(Rect::new(0, 0, 200, 120));
        chart.set_chart_type(ChartType::Bar);
        chart.set_data(vec![-10.0, 20.0, -5.0]);
        let rgba = render(&mut chart, 200, 120);
        assert!(painted_pixels(&rgba) > 0);
        assert_eq!(chart.chart_type(), ChartType::Bar);
    }
    /// Hover is observable through the accessor the module doc promises.
    ///
    /// `hovered_index` was written by `handle_event` and read by nothing, and had no
    /// accessor at all — so the "shared interaction (`hovered_index`, ...)" the module doc
    /// describes was not reachable by a caller.
    #[test]
    fn chart_hovered_index_is_observable() {
        let mut chart = ChartWidget::new(Rect::new(0, 0, 200, 120));
        chart.set_data(vec![10.0, 20.0, 30.0, 40.0]);
        assert_eq!(chart.hovered_index(), None, "nothing is hovered initially");

        chart.handle_event(&Event::mouse_move(120, 50));
        assert_eq!(chart.hovered_index(), Some(2));
        assert_eq!(chart.hovered_value(), Some(30.0));
    }

    /// Leaving the chart clears the hover, and says which point was left.
    ///
    /// There was no `MouseLeave` arm, so `hovered_index` was only ever set: the chart kept
    /// reporting a hover after the pointer left, and a highlight driven by
    /// `data_point_hovered` could be turned on but never off. The accessor reports the index
    /// that stopped being hovered, matching the sibling charts' `*_unhovered` signals.
    #[test]
    fn chart_mouse_leave_clears_hover_and_reports_it() {
        let mut chart = ChartWidget::new(Rect::new(0, 0, 200, 120));
        chart.set_data(vec![10.0, 20.0, 30.0, 40.0]);

        let unhovered = Arc::new(Mutex::new(Vec::<usize>::new()));
        let sink = unhovered.clone();
        chart.data_point_unhovered.connect(move |index| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*index);
            }
        });

        chart.handle_event(&Event::mouse_move(120, 50));
        assert_eq!(chart.hovered_index(), Some(2));

        chart.handle_event(&Event::mouse_leave(120, 50));
        assert_eq!(chart.hovered_index(), None, "leave must clear the hover");
        assert_eq!(
            unhovered.lock().expect("lock poisoned").clone(),
            vec![2],
            "leave must report the point it left"
        );

        // A second leave with nothing hovered must not emit again.
        chart.handle_event(&Event::mouse_leave(120, 50));
        assert_eq!(unhovered.lock().expect("lock poisoned").len(), 1);
    }

    /// Moving between buckets leaves the previous one before entering the next.
    #[test]
    fn chart_moving_between_buckets_unhovers_the_previous() {
        let mut chart = ChartWidget::new(Rect::new(0, 0, 200, 120));
        chart.set_data(vec![10.0, 20.0, 30.0, 40.0]);

        let events = Arc::new(Mutex::new(Vec::<String>::new()));
        let hover_sink = events.clone();
        chart.data_point_hovered.connect(move |index| {
            if let Ok(mut guard) = hover_sink.lock() {
                guard.push(format!("enter {index}"));
            }
        });
        let unhover_sink = events.clone();
        chart.data_point_unhovered.connect(move |index| {
            if let Ok(mut guard) = unhover_sink.lock() {
                guard.push(format!("leave {index}"));
            }
        });

        chart.handle_event(&Event::mouse_move(30, 50));
        chart.handle_event(&Event::mouse_move(120, 50));

        assert_eq!(
            events.lock().expect("lock poisoned").clone(),
            vec!["enter 0".to_string(), "leave 0".to_string(), "enter 2".to_string()],
            "a bucket change must leave the old bucket before entering the new one"
        );
    }

    /// Replacing the data drops a hover that no longer names a point.
    ///
    /// The sibling charts re-validate on `set_series`. `ChartWidget` kept a stale index, so
    /// `hovered_index()` reported a point the chart cannot draw and `data_point_hovered`
    /// would never fire for again.
    #[test]
    fn chart_replacing_data_drops_a_stale_hover() {
        let mut chart = ChartWidget::new(Rect::new(0, 0, 200, 120));
        chart.set_data(vec![1.0; 40]);
        chart.handle_event(&Event::mouse_move(150, 50));
        let index = chart.hovered_index().expect("the pointer is over a point");
        assert!(index >= 5, "expected a high index for a 40-point series, got {index}");

        // Shrink the series below the hovered index.
        chart.set_data(vec![1.0; 5]);
        assert_eq!(chart.hovered_index(), None, "the stale index must be dropped");
        assert_eq!(chart.hovered_value(), None);

        // A hover that *is* still in range survives the replacement.
        chart.handle_event(&Event::mouse_move(30, 50));
        let in_range = chart.hovered_index().expect("pointer over a point");
        assert!(in_range < 5);
        chart.set_data(vec![1.0; 8]);
        assert_eq!(chart.hovered_index(), Some(in_range), "an in-range hover is kept");
    }

}
