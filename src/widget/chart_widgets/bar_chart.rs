// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! BarChart widget — a vertical bar chart for visualizing categorical data.
//!
//! The BarChart widget draws axes, optional grid lines, vertical bars with
//! optional value labels on top. Each bar can have its own color, or all bars
//! share a default color.
//!
//! # Rendering path
//!
//! With the `chart` feature enabled, the plot area, axes, grid and tick labels
//! are produced by the shared chart engine in [`crate::widget::chart_widgets`], so this widget
//! and the SVG chart renderer share one implementation (see `plot_rect` below).
//!
//! Without that feature — `tablet` and `mobile` do not enable it — the widget
//! falls back to its own compact plot-area and grid loop. That path is kept
//! deliberately small so the two never diverge in the parts that matter: both
//! use the same bar geometry, labels and colors.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
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
    compute_cartesian_layout_measured, draw_legend_entries_iter, draw_y_ticks, CartesianLayout,
};
use crate::widget::metrics::role_font;
// The chrome derivation is shared with the engine-backed path deliberately: the
// `not(feature = "chart")` backdrop used to write its own light-chart literals, so a
// tablet/mobile build in the dark appearance drew a near-invisible chart. One
// derivation, both paths.
use crate::widget::chart_widgets::charts::{axis_chrome, axis_chrome_color};
#[cfg(feature = "chart")]
use crate::widget::chart_widgets::types::ChartContext;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Converts the shared engine's float plot area into the integer [`Rect`] this
/// widget positions bars in.
///
/// `CartesianLayout` stores `f32` geometry because it is shared with the SVG
/// backend; bar placement is pixel-exact, so the conversion happens once here
/// rather than at every use site.
#[cfg(feature = "chart")]
fn plot_rect(layout: &CartesianLayout) -> Rect {
    Rect::new(
        layout.plot_x().round() as i32,
        layout.plot_y().round() as i32,
        layout.plot_w().round().max(1.0) as u32,
        layout.plot_h().round().max(1.0) as u32,
    )
}

/// A single bar entry in the bar chart.
#[derive(Clone, Debug)]
pub struct BarEntry {
    /// Label displayed below the bar (X-axis category).
    pub label: String,
    /// Numeric value determining the bar height.
    pub value: f64,
    /// Optional per-bar color. Falls back to the chart's bar_color if None.
    pub color: Option<Color>,
}

impl BarEntry {
    /// Creates a new bar entry with the given label and value.
    pub fn new(label: impl Into<String>, value: f64) -> Self {
        Self { label: label.into(), value, color: None }
    }

    /// Sets a custom color for this bar entry.
    pub fn with_color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }
}

/// Which way the bars grow from the axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BarOrientation {
    /// Bars grow upward from the horizontal axis; the default.
    #[default]
    Vertical,
    /// Bars grow rightward from the vertical axis, one category per row.
    Horizontal,
}

/// How several [`BarSeries`] share one category slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BarGrouping {
    /// One bar per series, side by side inside the category slot; the default.
    #[default]
    Grouped,
    /// One bar per category, split by the series' contributions.
    Stacked,
}

/// One named series of category values in a [`BarChart`].
///
/// `values` is indexed by category: entry `n` is the bar for the chart's category label
/// `n`. A `color` overrides the palette entry for the whole series.
#[derive(Clone, Debug)]
pub struct BarSeries {
    /// The label shown in the legend. Empty hides the entry.
    pub name: String,
    /// One value per category.
    pub values: Vec<f64>,
    /// The caller's fill colour, or `None` to follow the chart's palette.
    pub color: Option<Color>,
}

impl BarSeries {
    /// Creates a named series from its per-category values.
    pub fn new(name: impl Into<String>, values: Vec<f64>) -> Self {
        Self { name: name.into(), values, color: None }
    }

    /// Sets the series colour, replacing the palette entry.
    pub fn with_color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }
}

/// A vertical bar chart widget for categorical data.
pub struct BarChart {
    base: BaseWidget,
    bars: Vec<BarEntry>,
    bar_color: Color,
    bar_spacing: f32,
    show_values: bool,
    show_grid: bool,
    min_value: Option<f64>,
    max_value: Option<f64>,
    /// Multi-series data. When empty, `bars` is the single series.
    series: Vec<BarSeries>,
    /// Category labels for multi-series mode.
    category_labels: Vec<String>,
    orientation: BarOrientation,
    grouping: BarGrouping,
    show_legend: bool,
    show_tooltip: bool,
    hovered: Option<usize>,
    /// The bar growth fraction, used only while `animate_bars` is set.
    reveal: PropertyDriver,
    animate_bars: bool,
    /// Whether data updates ease from the values currently on screen.
    animate_value_changes: bool,
    /// Progress of the running value-change transition, in `0.0..=1.0`.
    ///
    /// Only consulted while `animate_value_changes` is set; the start values live in
    /// `value_from_bars` / `value_from_series`.
    value_transition: PropertyDriver,
    /// The starting value of each bar for the running transition (single-series model).
    value_from_bars: Vec<f64>,
    /// The starting values of each series for the running transition (multi-series model).
    value_from_series: Vec<Vec<f64>>,
    /// The plot area from the last paint, reused for hit testing.
    last_plot_area: Option<Rect>,
    /// Cached y-axis label width; `None` until the first measured paint.
    #[cfg(feature = "chart")]
    y_label_cache: Option<YAxisLabelCache>,
    /// Emitted with the category index under the pointer when hover enters a bar.
    pub bar_hovered: Signal1<usize>,
    /// Emitted with the category index when the pointer leaves a bar.
    pub bar_unhovered: Signal1<usize>,
    /// Emitted with the category index of a clicked bar.
    pub bar_clicked: Signal1<usize>,
    /// Emitted with `(series, category)` when hover enters a bar (CH-04).
    ///
    /// The single-index signals above keep their historical payload for compatibility; a
    /// grouped chart needs the series dimension, which this signal carries.
    pub bar_hovered_with_series: Signal1<(usize, usize)>,
    /// Emitted with `(series, category)` when the pointer leaves a bar (CH-04).
    pub bar_unhovered_with_series: Signal1<(usize, usize)>,
    /// Emitted with `(series, category)` of a clicked bar (CH-04).
    pub bar_clicked_with_series: Signal1<(usize, usize)>,
    /// The chart title, painted centred at the top (CH-07).
    title: Option<String>,
    /// The x-axis title, painted under the plot (CH-07).
    x_axis_label: Option<String>,
    /// The y-axis title, painted above the plot's leading edge (CH-07).
    y_axis_label: Option<String>,
    /// An attached reference-line companion (COMP-F01).
    crosshair: Option<crate::widget::chart_widgets::chart_crosshair::ChartCrosshair>,
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

/// Font size of the value label drawn above each bar, in points.
const LABEL_FONT_SIZE: f32 = 10.0;

/// Font size of the category label drawn below the axis, in points.
const CATEGORY_FONT_SIZE: f32 = 9.0;

/// Distance between a bar's top edge and the bottom edge of its value label.
const LABEL_GAP: i32 = 4;

/// Distance from the plot baseline down to the top of the category label row.
const LABEL_ROW_TOP: i32 = 12;

impl BarChart {
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

    /// Creates a new BarChart widget with the given geometry.
    ///
    /// Defaults: blue bars, spacing 0.2 (20% of bar slot), values shown, grid enabled.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::BarChart, geometry, "BarChart"),
            bars: Vec::new(),
            bar_color: Color::PRIMARY,
            bar_spacing: 0.2,
            show_values: true,
            show_grid: true,
            min_value: None,
            max_value: None,
            series: Vec::new(),
            category_labels: Vec::new(),
            orientation: BarOrientation::Vertical,
            grouping: BarGrouping::Grouped,
            show_legend: false,
            show_tooltip: false,
            hovered: None,
            reveal: PropertyDriver::at(1.0, MotionSlot::Normal),
            animate_bars: false,
            animate_value_changes: false,
            value_transition: PropertyDriver::at(1.0, MotionSlot::Normal),
            value_from_bars: Vec::new(),
            value_from_series: Vec::new(),
            last_plot_area: None,
            #[cfg(feature = "chart")]
            y_label_cache: None,
            bar_hovered: Signal1::new(),
            bar_unhovered: Signal1::new(),
            bar_clicked: Signal1::new(),
            bar_hovered_with_series: Signal1::new(),
            bar_unhovered_with_series: Signal1::new(),
            bar_clicked_with_series: Signal1::new(),
            title: None,
            x_axis_label: None,
            y_axis_label: None,
            crosshair: None,
        }
    }

    /// Replaces the multi-series data and its category labels at once.
    ///
    /// Non-finite values are dropped. When `series` is non-empty the chart draws in
    /// multi-series mode and ignores [`Self::bars`]; setting an empty list returns it to the
    /// single-series model.
    pub fn set_series(&mut self, series: Vec<BarSeries>, category_labels: Vec<String>) {
        let next: Vec<BarSeries> = series
            .into_iter()
            .map(|mut item| {
                item.values.retain(|value| value.is_finite());
                item
            })
            .collect();
        if self.animate_value_changes && self.series_shape_matches(&next, &category_labels) {
            let from: Vec<Vec<f64>> = (0..self.series.len())
                .map(|series_index| {
                    (0..self.series[series_index].values.len())
                        .filter_map(|category| self.rendered_series_value(series_index, category))
                        .collect()
                })
                .collect();
            self.series = next;
            self.category_labels = category_labels;
            let unchanged = from.iter().zip(&self.series).all(|(start, series)| {
                start.iter().zip(&series.values).all(|(old, new)| *old == *new)
            });
            if unchanged {
                self.clear_value_transition();
            } else {
                self.value_from_series = from;
                self.value_from_bars.clear();
                self.start_value_transition();
            }
        } else {
            self.clear_value_transition();
            self.series = next;
            self.category_labels = category_labels;
            self.restart_growth();
        }
        self.revalidate_hover();
        self.base.request_redraw();
    }

    /// Returns the multi-series data.
    pub fn series(&self) -> &[BarSeries] {
        &self.series
    }

    /// Returns the category labels used in multi-series mode.
    pub fn category_labels(&self) -> &[String] {
        &self.category_labels
    }

    /// Sets the bar direction.
    pub fn set_orientation(&mut self, orientation: BarOrientation) {
        if self.orientation == orientation {
            return;
        }
        self.orientation = orientation;
        self.base.request_redraw();
    }

    /// Returns the bar direction.
    pub fn orientation(&self) -> BarOrientation {
        self.orientation
    }

    /// Sets how several series share a category slot.
    pub fn set_grouping(&mut self, grouping: BarGrouping) {
        if self.grouping == grouping {
            return;
        }
        self.grouping = grouping;
        self.base.request_redraw();
    }

    /// Returns the grouping mode.
    pub fn grouping(&self) -> BarGrouping {
        self.grouping
    }

    /// Shows or hides the multi-series legend.
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

    /// Enables the bar growth animation.
    ///
    /// The animation is opt-in: a chart that never asks for it paints at its values on the
    /// first frame, exactly as before.
    pub fn set_animate_bars(&mut self, animate: bool) {
        if self.animate_bars == animate {
            return;
        }
        self.animate_bars = animate;
        if !animate {
            self.reveal.jump_to(1.0);
        }
        self.base.request_redraw();
    }

    /// Returns whether the bar growth animation is enabled.
    pub fn animate_bars(&self) -> bool {
        self.animate_bars
    }

    /// Enables easing of value changes made through [`Self::set_bars`] and [`Self::set_series`].
    ///
    /// # What eases and what jumps
    ///
    /// With this on, an update whose **shape** is unchanged keeps the values currently on screen
    /// as the start and eases to the new ones through the crate's motion system. For the
    /// single-series model the shape is the bar count; for the multi-series model it is the
    /// series count, each series' value count, and the drawn category count.
    ///
    /// A shape change — a category added or removed, a series added or removed, or switching
    /// between the single-series and multi-series models — has no compatible previous frame to
    /// interpolate from, so it jumps to the new data instead. A pure value change eases.
    ///
    /// The default is off: a data update paints the new values on the next frame, exactly as
    /// before. Turning it off during a transition lands the bars on their targets immediately.
    ///
    /// This flag is independent of [`Self::set_animate_bars`], which is the one-shot
    /// grow-from-zero reveal. A value change that eases does not restart that reveal.
    pub fn set_animate_value_changes(&mut self, animate: bool) {
        if self.animate_value_changes == animate {
            return;
        }
        self.animate_value_changes = animate;
        if !animate {
            self.clear_value_transition();
        }
        self.base.request_redraw();
    }

    /// Returns whether data updates ease from the previously drawn values.
    pub fn animate_value_changes(&self) -> bool {
        self.animate_value_changes
    }

    /// How far the bars have grown, in `0.0..=1.0`.
    pub fn growth_progress(&self) -> f32 {
        if self.animate_bars {
            self.reveal.value()
        } else {
            1.0
        }
    }

    /// The hovered category index, if any.
    pub fn hovered_bar(&self) -> Option<usize> {
        self.hovered
    }

    /// Restarts the growth animation when it is enabled.
    fn restart_growth(&mut self) {
        if self.animate_bars {
            self.reveal.jump_to(0.0);
            self.reveal.set_target(1.0);
        }
    }

    /// Lands the running value transition on its targets and drops its start values.
    fn clear_value_transition(&mut self) {
        self.value_transition.jump_to(1.0);
        self.value_from_bars.clear();
        self.value_from_series.clear();
    }

    /// Starts the progress driver; the caller has already installed the start values.
    fn start_value_transition(&mut self) {
        self.value_transition.jump_to(0.0);
        self.value_transition.set_target(1.0);
    }

    /// The value the painter draws for `category` in the single-series model.
    ///
    /// This is the one place values are interpolated: the draw loop, the value labels and the
    /// axis range all read through it, so a bar and the label above it can never disagree.
    fn rendered_bar_value(&self, category: usize) -> Option<f64> {
        let target = self.bars.get(category)?.value;
        Some(self.eased_value(self.value_from_bars.get(category).copied(), target))
    }

    /// The value the painter draws for `category` in `series[series_index]`.
    fn rendered_series_value(&self, series_index: usize, category: usize) -> Option<f64> {
        let target = *self.series.get(series_index)?.values.get(category)?;
        Some(
            self.eased_value(
                self.value_from_series
                    .get(series_index)
                    .and_then(|values| values.get(category))
                    .copied(),
                target,
            ),
        )
    }

    /// Interpolates a value between the point a transition started at and its target.
    ///
    /// `from` is `None` when no transition covers this value, in which case the target is drawn
    /// as-is. That is also the answer whenever the easing flag is off, so a stale start value can
    /// never move a bar the caller expects to be static.
    fn eased_value(&self, from: Option<f64>, target: f64) -> f64 {
        match from {
            Some(from) if self.animate_value_changes => {
                let progress = f64::from(self.value_transition.value().clamp(0.0, 1.0));
                from + (target - from) * progress
            }
            _ => target,
        }
    }

    /// Whether `next` has the same drawn shape as the current single-series model.
    fn bar_shape_matches(&self, next: &[BarEntry]) -> bool {
        self.series.is_empty() && self.bars.len() == next.len()
    }

    /// Whether `next` has the same drawn shape as the current multi-series model.
    ///
    /// Labels are compared by their contribution to the drawn category count rather than by
    /// text: renaming a label keeps the slot layout and may ease, while adding or removing a
    /// category slot may not.
    fn series_shape_matches(&self, next: &[BarSeries], labels: &[String]) -> bool {
        if self.series.is_empty() || self.series.len() != next.len() {
            return false;
        }
        if !self.series.iter().zip(next).all(|(old, new)| old.values.len() == new.values.len()) {
            return false;
        }
        let next_categories =
            next.iter().map(|series| series.values.len()).max().unwrap_or(0).max(labels.len());
        self.category_count() == next_categories
    }

    /// Drops a hover index the new data no longer has a bar for.
    fn revalidate_hover(&mut self) {
        let categories = self.category_count();
        if self.hovered.is_some_and(|index| index >= categories) {
            if let Some(index) = self.hovered.take() {
                self.bar_unhovered.emit(index);
                let series = self.series_at(Point::new(0, 0), Rect::new(0, 0, 1, 1), index);
                self.bar_unhovered_with_series.emit((series, index));
            }
        }
    }

    /// The number of categories the chart draws.
    fn category_count(&self) -> usize {
        if self.series.is_empty() {
            self.bars.len()
        } else {
            self.series
                .iter()
                .map(|series| series.values.len())
                .max()
                .unwrap_or(0)
                .max(self.category_labels.len())
        }
    }

    /// The chart title, when one is set (CH-07).
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    /// Attaches a crosshair companion (COMP-F01).
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

    /// Sets the x-axis title.
    pub fn set_x_axis_label(&mut self, label: impl Into<String>) {
        self.x_axis_label = Some(label.into());
        self.base.request_redraw();
    }

    /// The y-axis title, when one is set.
    pub fn y_axis_label(&self) -> Option<&str> {
        self.y_axis_label.as_deref()
    }

    /// Sets the y-axis title.
    pub fn set_y_axis_label(&mut self, label: impl Into<String>) {
        self.y_axis_label = Some(label.into());
        self.base.request_redraw();
    }

    /// The series a pointer position names inside `category` (CH-04).
    ///
    /// Grouped mode divides the category slot into one strip per series; stacked mode cannot
    /// attribute a height to a series without its stack offset, so it reports series 0, which is
    /// the same compatibility mapping the single-index signals keep.
    fn series_at(&self, pos: Point, plot_area: Rect, category: usize) -> usize {
        let series_count = self.series.len();
        if series_count <= 1 || self.grouping == BarGrouping::Stacked {
            return 0;
        }
        let categories = self.category_count().max(1);
        let fraction = match self.orientation {
            BarOrientation::Vertical => {
                let slot = (plot_area.width as f32 / categories as f32).max(1.0);
                let start = plot_area.x as f32 + slot * category as f32;
                ((pos.x as f32 - start) / slot).clamp(0.0, 0.999)
            }
            BarOrientation::Horizontal => {
                let slot = (plot_area.height as f32 / categories as f32).max(1.0);
                let start = plot_area.y as f32 + slot * category as f32;
                ((pos.y as f32 - start) / slot).clamp(0.0, 0.999)
            }
        };
        ((fraction * series_count as f32) as usize).min(series_count - 1)
    }

    /// The colour of series `index`.
    fn series_color(&self, index: usize) -> Color {
        match self.series.get(index).and_then(|series| series.color) {
            Some(color) => color,
            None if index == 0 => self.bar_color,
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

    /// Sets the bars to display. Clears any previous bars.
    ///
    /// A bar whose value is non-finite cannot be mapped to a height, so it is dropped rather than
    /// drawn at a fabricated position (see [`is_finite_sample`](crate::widget::chart_widgets::is_finite_sample)).
    pub fn set_bars(&mut self, entries: Vec<BarEntry>) {
        let next: Vec<BarEntry> = entries.into_iter().filter(|bar| bar.value.is_finite()).collect();
        if self.animate_value_changes && self.bar_shape_matches(&next) {
            let from: Vec<f64> =
                (0..self.bars.len()).filter_map(|index| self.rendered_bar_value(index)).collect();
            self.bars = next;
            let unchanged = from.iter().zip(&self.bars).all(|(start, bar)| *start == bar.value);
            if unchanged {
                self.clear_value_transition();
            } else {
                self.value_from_bars = from;
                self.value_from_series.clear();
                self.start_value_transition();
            }
        } else {
            self.clear_value_transition();
            self.bars = next;
            self.restart_growth();
        }
        self.revalidate_hover();
        self.base.request_redraw();
    }

    /// Adds a single bar entry to the chart.
    ///
    /// A non-finite value is refused; see [`Self::set_bars`].
    pub fn add_bar(&mut self, entry: BarEntry) {
        if !entry.value.is_finite() {
            log::warn!(
                "[bar_chart] ignoring the bar '{}' with non-finite value {}",
                entry.label,
                entry.value
            );
            return;
        }
        self.clear_value_transition();
        self.bars.push(entry);
        self.base.request_redraw();
    }

    /// Removes the bar at the given index.
    /// Returns `true` if the bar was removed, `false` if the index was out of bounds.
    pub fn remove_bar(&mut self, index: usize) -> bool {
        if index < self.bars.len() {
            self.clear_value_transition();
            self.bars.remove(index);
            self.base.request_redraw();
            true
        } else {
            false
        }
    }

    /// Removes all bars from the chart.
    pub fn clear_bars(&mut self) {
        self.clear_value_transition();
        self.bars.clear();
        self.base.request_redraw();
    }

    /// Returns the number of bars.
    pub fn bar_count(&self) -> usize {
        self.bars.len()
    }

    /// Returns a reference to the current bar entries.
    pub fn bars(&self) -> &[BarEntry] {
        &self.bars
    }

    /// Sets the default bar color for all bars (used when BarEntry.color is None).
    pub fn set_bar_color(&mut self, color: Color) {
        if self.bar_color == color {
            return;
        }
        self.bar_color = color;
        self.base.request_redraw();
    }

    /// Returns the current default bar color.
    pub fn bar_color(&self) -> Color {
        self.bar_color
    }

    /// Sets the spacing between bars as a fraction of the bar slot width.
    pub fn set_bar_spacing(&mut self, spacing: f32) {
        let spacing = spacing.clamp(0.0, 0.8);
        if self.bar_spacing == spacing {
            return;
        }
        self.bar_spacing = spacing;
        self.base.request_redraw();
    }

    /// Returns the current bar spacing fraction.
    pub fn bar_spacing(&self) -> f32 {
        self.bar_spacing
    }

    /// Enables or disables showing value labels on top of bars.
    pub fn set_show_values(&mut self, show: bool) {
        if self.show_values == show {
            return;
        }
        self.show_values = show;
        self.base.request_redraw();
    }

    /// Returns whether value labels are shown on top of bars.
    pub fn show_values(&self) -> bool {
        self.show_values
    }

    /// Enables or disables grid lines.
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

    /// Sets manual min/max value range. Pass `None` for auto-compute.
    pub fn set_value_range(&mut self, min: Option<f64>, max: Option<f64>) {
        if self.min_value == min && self.max_value == max {
            return;
        }
        self.min_value = min;
        self.max_value = max;
        self.base.request_redraw();
    }

    /// Returns the configured value minimum (if manually set).
    pub fn min_value(&self) -> Option<f64> {
        self.min_value
    }

    /// Returns the configured value maximum (if manually set).
    pub fn max_value(&self) -> Option<f64> {
        self.max_value
    }

    /// Resolves the Y range for the chart.
    fn resolve_y_range(&self) -> (f64, f64) {
        let (mut min, mut max) = match (self.min_value, self.max_value) {
            (Some(min), Some(max)) => (min, max),
            _ => {
                if self.category_count() == 0 {
                    return (0.0, 10.0);
                }
                let (low, high) = self.value_extent();
                (low.min(0.0), high)
            }
        };
        if !min.is_finite() || !max.is_finite() {
            return (0.0, 10.0);
        }
        if (max - min).abs() < f64::EPSILON {
            min = 0.0;
            max = max.max(1.0) + 1.0;
        }
        if self.min_value.is_none() || self.max_value.is_none() {
            let padding = (max - min) * 0.1;
            min -= padding;
            max += padding;
        }
        (min, max)
    }

    /// The lowest and highest drawn value, across the active data model.
    fn value_extent(&self) -> (f64, f64) {
        let mut min = f64::INFINITY;
        let mut max = f64::NEG_INFINITY;
        if self.series.is_empty() {
            for category in 0..self.bars.len() {
                if let Some(value) = self.rendered_bar_value(category) {
                    min = min.min(value);
                    max = max.max(value);
                }
            }
        } else if self.grouping == BarGrouping::Stacked {
            let categories = self.category_count();
            for category in 0..categories {
                let (mut positive, mut negative) = (0.0f64, 0.0f64);
                for series_index in 0..self.series.len() {
                    if let Some(value) = self.rendered_series_value(series_index, category) {
                        if value >= 0.0 {
                            positive += value;
                        } else {
                            negative += value;
                        }
                    }
                }
                min = min.min(negative);
                max = max.max(positive);
            }
        } else {
            for series_index in 0..self.series.len() {
                for category in 0..self.series[series_index].values.len() {
                    if let Some(value) = self.rendered_series_value(series_index, category) {
                        min = min.min(value);
                        max = max.max(value);
                    }
                }
            }
        }
        if !min.is_finite() || !max.is_finite() {
            (0.0, 10.0)
        } else {
            (min, max)
        }
    }

    /// Plot area used when the `chart` feature is unavailable.
    ///
    /// `tablet` and `mobile` do not enable the shared chart engine, so they keep
    /// this compact margin calculation. It mirrors the engine's left margin (for
    /// Y labels) and bottom margin (for X categories) so a chart looks the same
    /// across profiles.
    #[cfg(not(feature = "chart"))]
    fn plot_area(&self) -> Rect {
        let rect = self.base.geometry();
        let margin_left = 50;
        let margin_right = 10;
        let margin_top = if self.show_values { 30 } else { 10 };
        let margin_bottom = if !self.bars.is_empty() { 30 } else { 10 };
        let x = rect.x + margin_left;
        let y = rect.y + margin_top;
        let w = (rect.width as i32 - margin_left - margin_right).max(10) as u32;
        let h = (rect.height as i32 - margin_top - margin_bottom).max(10) as u32;
        Rect::new(x, y, w, h)
    }
}

impl Widget for BarChart {
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

    /// Advances the bar growth reveal and the value transition by `delta_ms`.
    fn tick(&mut self, delta_ms: u32) -> bool {
        let mut moving = false;
        if self.animate_bars {
            moving |= self.reveal.tick(delta_ms);
        }
        if self.animate_value_changes {
            moving |= self.value_transition.tick(delta_ms);
            if !self.value_transition.is_moving() {
                self.value_from_bars.clear();
                self.value_from_series.clear();
            }
        }
        moving
    }

    /// Whether either animation still owes frames.
    fn is_animating(&self) -> bool {
        (self.animate_bars && self.reveal.is_moving())
            || (self.animate_value_changes && self.value_transition.is_moving())
    }

    /// Resolves a published event name to the signal this chart actually emits.
    ///
    /// The capability publishes all three names with a `UInt` payload, and the
    /// control carries the category index as a `usize`, so the conversion is the
    /// widening that keeps the subscriber's number identical to the emitted one.
    /// Without these arms the names would validate in `connect_event` and then
    /// never reach a signal.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "bar_clicked" => Some(EventSignalRef::mapped("bar_clicked", &self.bar_clicked, |v| {
                CapabilityValue::UInt(*v as u64)
            })),
            "bar_hovered" => Some(EventSignalRef::mapped("bar_hovered", &self.bar_hovered, |v| {
                CapabilityValue::UInt(*v as u64)
            })),
            "bar_unhovered" => {
                Some(EventSignalRef::mapped("bar_unhovered", &self.bar_unhovered, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
            _ => None,
        }
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `BarChart`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` / `access_write_other.in.rs` dispatch, including
/// the `f32` → `f64` widening on read.
impl WidgetProperties for BarChart {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "bar_spacing" => Ok(CapabilityValue::Float(self.bar_spacing() as f64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "bar_spacing" => {
                self.set_bar_spacing(expect_f32(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["bar_spacing", BASE_PROPERTY_NAMES]
    }
}

impl Draw for BarChart {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.base.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }

        let (y_min, y_max) = self.resolve_y_range();
        let is_enabled = self.base.is_enabled();
        let disabled_color = Color::DISABLED_FOREGROUND;

        // ── Backdrop: axes, grid and tick labels ──
        //
        // With the `chart` feature this is the shared engine, so the widget and
        // the SVG chart renderer cannot drift apart. Without it (tablet/mobile)
        // a compact local preamble runs instead — see `draw_backdrop_without_engine`.
        #[cfg(feature = "chart")]
        let plot_area = {
            let widest_y_label = {
                let font = role_font(self.base.style(), 10.0);
                self.widest_y_label(context, y_min, y_max, rect.width, font)
            };
            let legend_items = if self.show_legend { self.series.len() } else { 0 };
            let layout =
                compute_cartesian_layout_measured(rect, true, true, legend_items, widest_y_label);
            let plot_area = plot_rect(&layout);
            let mut adapter = ChartContextAdapter::new(context);

            let axis_color = if is_enabled { axis_chrome().0 } else { disabled_color };
            let bottom = layout.plot_y() + layout.plot_h();
            adapter.draw_line(
                Point::new(layout.plot_x() as i32, layout.plot_y() as i32),
                Point::new(layout.plot_x() as i32, bottom as i32),
                1.0,
                axis_color,
            );
            adapter.draw_line(
                Point::new(layout.plot_x() as i32, bottom as i32),
                Point::new((layout.plot_x() + layout.plot_w()) as i32, bottom as i32),
                1.0,
                axis_color,
            );

            // `draw_y_ticks` emits the grid lines *and* their value labels, so the
            // tick density is no longer hard-coded in this widget.
            draw_y_ticks(&mut adapter, &layout, y_min, y_max, 4, self.show_grid);
            if self.show_legend {
                draw_legend_entries_iter(
                    &mut adapter,
                    &layout,
                    self.series
                        .iter()
                        .enumerate()
                        .filter(|(_, series)| !series.name.is_empty())
                        .map(|(index, series)| (series.name.as_str(), self.series_color(index))),
                );
            }
            plot_area
        };

        #[cfg(not(feature = "chart"))]
        let plot_area = self.draw_backdrop_without_engine(context, is_enabled, disabled_color);
        self.last_plot_area = Some(plot_area);

        // ── Title and axis titles (CH-07) ──
        if self.title.is_some() || self.x_axis_label.is_some() || self.y_axis_label.is_some() {
            let chrome = axis_chrome_color(0.85);
            let centred = crate::core::HorizontalAlignment::Center;
            if let Some(title) = self.title.as_deref() {
                let font = role_font(self.base.style(), 12.0).with_bold(true);
                context.draw_text_fitted_cow(
                    Rect::new(rect.x, rect.y, rect.width, 16),
                    title,
                    &font,
                    chrome,
                    centred,
                );
            }
            if let Some(label) = self.x_axis_label.as_deref() {
                let font = role_font(self.base.style(), 10.0);
                context.draw_text_fitted_cow(
                    Rect::new(
                        plot_area.x,
                        plot_area.y + plot_area.height as i32 + 14,
                        plot_area.width,
                        12,
                    ),
                    label,
                    &font,
                    chrome,
                    centred,
                );
            }
            if let Some(label) = self.y_axis_label.as_deref() {
                let font = role_font(self.base.style(), 10.0);
                context.draw_text_fitted_cow(
                    Rect::new(rect.x + 2, rect.y + 16, rect.width.min(120), 12),
                    label,
                    &font,
                    chrome,
                    crate::core::HorizontalAlignment::Left,
                );
            }
        }

        // ── Bars ──
        if !self.series.is_empty() {
            self.draw_series_bars(context, plot_area, y_min, y_max, is_enabled);
            self.draw_hover_overlay(context, plot_area);
            if let Some(crosshair) = self.crosshair.as_mut() {
                crosshair.base_mut().set_geometry(plot_area);
                crosshair.draw(context);
            }
            return;
        }
        if self.bars.is_empty() {
            return;
        }

        let plot_width = plot_area.width as f32;
        let n = self.bars.len();
        let total_slots = n as f32;
        let spacing_pixels = (plot_width * self.bar_spacing) / total_slots;
        let bar_slot_width = (plot_width - spacing_pixels * (total_slots + 1.0)) / total_slots;
        let bar_width = bar_slot_width.max(1.0);

        let baseline_y = plot_area.y + plot_area.height as i32;
        let height_range = plot_area.height as f64;
        let value_span = (y_max - y_min).max(f64::EPSILON);
        let growth = f64::from(self.growth_progress());

        // # Why the label's face and its line height are resolved once
        //
        // Both are properties of the control, not of a bar, yet they sat inside the per-bar loop: a
        // 40-bar chart resolved the theme font 40 times and measured `"M"` 40 times, per frame. The
        // label text itself still has to be formatted per bar (each bar shows its own value), but
        // the appearance it is drawn with does not.
        let label_font = role_font(self.base.style(), LABEL_FONT_SIZE);
        let label_line_height = context.measure_text("M", &label_font).height.max(1) as i32;
        // The category label's face is also a property of the control, not of a bar. It used
        // to be resolved (and `"M"` measured) inside the per-bar loop, which the value-label
        // path above had already been fixed to avoid.
        let category_font = role_font(self.base.style(), CATEGORY_FONT_SIZE);
        let category_line_height = context.measure_text("M", &category_font).height.max(1) as i32;
        // Reused across bars so a value label costs no heap allocation per frame.
        let mut label = String::with_capacity(16);

        for (i, bar) in self.bars.iter().enumerate() {
            let Some(value) = self.rendered_bar_value(i) else {
                continue;
            };
            let bar_color = bar.color.unwrap_or(self.bar_color);
            let effective_color = if is_enabled { bar_color } else { disabled_color };

            let bar_x = plot_area.x
                + (spacing_pixels * (i as f32 + 1.0) + bar_slot_width * i as f32) as i32;
            let bar_height = ((value - y_min) * growth / value_span * height_range) as i32;
            let bar_y = baseline_y - bar_height;

            if bar_height > 0 {
                context.fill_rect(
                    Rect::new(bar_x, bar_y, bar_width as u32, bar_height as u32),
                    effective_color,
                );
            }

            // ── Value label on top of the bar ──
            //
            // Placed as a **line box sitting above the bar**, not at `bar_y - 4`. The literal
            // gap had no relation to the label's own height, so a 10 px line began 4 px above
            // the bar's top with its glyph box still overlapping the bar, which the vertical
            // audit reads as PUSHED-DOWN. Building the box from the measured line height and
            // anchoring its bottom edge to the bar's top is what makes the gap actually four
            // pixels, and it scales with the label font.
            if self.show_values && is_enabled {
                label.clear();
                let _ = core::fmt::Write::write_fmt(&mut label, format_args!("{:.1}", value));
                let label_x = bar_x.max(plot_area.x) + (bar_width as i32 / 2).min(12);
                let label_band = Rect {
                    x: label_x,
                    y: (bar_y - LABEL_GAP - label_line_height).max(plot_area.y),
                    width: bar_width as u32,
                    height: label_line_height as u32,
                };
                draw_label(context, &label, label_band, &label_font);
            }

            // ── Category label below the axis ──
            if is_enabled {
                let label_x = bar_x.max(plot_area.x) + (bar_width as i32 / 2).min(12);
                // Same treatment, anchored below the baseline: the line box starts where the
                // axis label row starts, so the gap under the axis is `LABEL_ROW_TOP`, not a
                // second hand-tuned offset that happened to equal it.
                let label_band = Rect {
                    x: label_x,
                    y: baseline_y + LABEL_ROW_TOP,
                    width: (plot_area.width as i32 - (label_x - plot_area.x)).max(0) as u32,
                    height: category_line_height as u32,
                };
                draw_label(context, &bar.label, label_band, &category_font);
            }
        }
        self.draw_hover_overlay(context, plot_area);
        if let Some(crosshair) = self.crosshair.as_mut() {
            crosshair.base_mut().set_geometry(plot_area);
            crosshair.draw(context);
        }
    }
}

impl BarChart {
    /// Draws the multi-series bars in the configured orientation and grouping.
    fn draw_series_bars(
        &self,
        context: &mut RenderContext,
        plot_area: Rect,
        y_min: f64,
        y_max: f64,
        is_enabled: bool,
    ) {
        let categories = self.category_count();
        if categories == 0 || self.series.is_empty() {
            return;
        }
        let disabled = Color::DISABLED_FOREGROUND;
        let growth = f64::from(self.growth_progress());
        let span = (y_max - y_min).max(f64::EPSILON);
        let font = role_font(self.base.style(), CATEGORY_FONT_SIZE);
        let line_height = context.measure_text("M", &font).height.max(1) as i32;
        let series_count = self.series.len().max(1);

        match self.orientation {
            BarOrientation::Vertical => {
                let slot = (plot_area.width as f32 / categories as f32).max(1.0);
                let baseline = plot_area.y + plot_area.height as i32;
                for category in 0..categories {
                    let slot_x = plot_area.x + (slot * category as f32) as i32;
                    let (mut positive, mut negative) = (0.0f64, 0.0f64);
                    for series_index in 0..self.series.len() {
                        let Some(value) = self.rendered_series_value(series_index, category) else {
                            continue;
                        };
                        let color =
                            if is_enabled { self.series_color(series_index) } else { disabled };
                        let (x, bar_width) = match self.grouping {
                            BarGrouping::Grouped => {
                                let width = (slot / series_count as f32).max(1.0);
                                (slot_x as f32 + width * series_index as f32, width - 1.0)
                            }
                            BarGrouping::Stacked => (slot_x as f32, (slot - 2.0).max(1.0)),
                        };
                        let start = match self.grouping {
                            BarGrouping::Grouped => y_min,
                            BarGrouping::Stacked if value >= 0.0 => positive,
                            BarGrouping::Stacked => negative,
                        };
                        let end = start + value * growth;
                        let top = Self::value_pixel(end.max(start), y_min, span, plot_area);
                        let bottom = Self::value_pixel(end.min(start), y_min, span, plot_area);
                        let height = (bottom - top).max(1) as u32;
                        if is_enabled {
                            context.fill_rect(
                                Rect::new(x as i32, top, bar_width.max(1.0) as u32, height),
                                color,
                            );
                        }
                        if value >= 0.0 {
                            positive = end;
                        } else {
                            negative = end;
                        }
                    }
                    if let Some(label) = self.category_labels.get(category) {
                        let band = Rect::new(
                            slot_x,
                            baseline + LABEL_ROW_TOP,
                            slot.max(1.0) as u32,
                            line_height as u32,
                        );
                        draw_label(context, label, band, &font);
                    }
                }
            }
            BarOrientation::Horizontal => {
                let slot = (plot_area.height as f32 / categories as f32).max(1.0);
                let axis_x = plot_area.x;
                for category in 0..categories {
                    let slot_y = plot_area.y + (slot * category as f32) as i32;
                    let (mut positive, mut negative) = (0.0f64, 0.0f64);
                    for series_index in 0..self.series.len() {
                        let Some(value) = self.rendered_series_value(series_index, category) else {
                            continue;
                        };
                        let color =
                            if is_enabled { self.series_color(series_index) } else { disabled };
                        let (y, bar_height) = match self.grouping {
                            BarGrouping::Grouped => {
                                let height = (slot / series_count as f32).max(1.0);
                                (slot_y as f32 + height * series_index as f32, height - 1.0)
                            }
                            BarGrouping::Stacked => (slot_y as f32, (slot - 2.0).max(1.0)),
                        };
                        let start = match self.grouping {
                            BarGrouping::Grouped => 0.0,
                            BarGrouping::Stacked if value >= 0.0 => positive,
                            BarGrouping::Stacked => negative,
                        };
                        let end = start + value * growth;
                        let x0 = axis_x + ((start - 0.0) / span * plot_area.width as f64) as i32;
                        let x1 = axis_x + ((end - 0.0) / span * plot_area.width as f64) as i32;
                        let left = x0.min(x1);
                        let width = (x0 - x1).unsigned_abs().max(1);
                        if is_enabled {
                            context.fill_rect(
                                Rect::new(left, y as i32, width, bar_height.max(1.0) as u32),
                                color,
                            );
                        }
                        if value >= 0.0 {
                            positive = end;
                        } else {
                            negative = end;
                        }
                    }
                    if let Some(label) = self.category_labels.get(category) {
                        let band = Rect::new(
                            plot_area.x.saturating_sub(60),
                            slot_y,
                            58,
                            line_height as u32,
                        );
                        draw_label(context, label, band, &font);
                    }
                }
            }
        }
    }

    /// The y pixel for a value in a vertical plot.
    fn value_pixel(value: f64, y_min: f64, span: f64, plot_area: Rect) -> i32 {
        let baseline = plot_area.y + plot_area.height as i32;
        let height = plot_area.height as f64;
        baseline - (((value - y_min) / span) * height) as i32
    }

    /// Paints the hover highlight and, when enabled, its tooltip.
    fn draw_hover_overlay(&self, context: &mut RenderContext, plot_area: Rect) {
        let Some(index) = self.hovered else {
            return;
        };
        let categories = self.category_count();
        if categories == 0 {
            return;
        }
        let slot = match self.orientation {
            BarOrientation::Vertical => plot_area.width as f32 / categories as f32,
            BarOrientation::Horizontal => plot_area.height as f32 / categories as f32,
        };
        let highlight = axis_chrome_color(0.10);
        let band = match self.orientation {
            BarOrientation::Vertical => Rect::new(
                plot_area.x + (slot * index as f32) as i32,
                plot_area.y,
                slot.max(1.0) as u32,
                plot_area.height,
            ),
            BarOrientation::Horizontal => Rect::new(
                plot_area.x,
                plot_area.y + (slot * index as f32) as i32,
                plot_area.width,
                slot.max(1.0) as u32,
            ),
        };
        context.fill_rect(band, highlight);
        if !self.show_tooltip {
            return;
        }
        let category_label = self
            .category_labels
            .get(index)
            .cloned()
            .or_else(|| self.bars.get(index).map(|bar| bar.label.clone()))
            .unwrap_or_else(|| index.to_string());
        let font = role_font(self.base.style(), 10.0);
        let surface = self.base.style().background_color.unwrap_or_else(|| axis_chrome_color(0.96));
        let ink = surface.contrast_color();
        // Multi-series charts show one line per series, each in its series colour (CH-03); a
        // single-series chart keeps the historical one-line label.
        let mut lines: Vec<(String, Color)> = Vec::new();
        if self.series.is_empty() {
            lines.push((category_label, ink));
        } else {
            lines.push((category_label, ink));
            for (series_index, series) in self.series.iter().enumerate() {
                let value = series.values.get(index).copied().unwrap_or(0.0);
                let name = if series.name.is_empty() {
                    std::format!("Series {}", series_index + 1)
                } else {
                    series.name.clone()
                };
                lines.push((std::format!("{name}: {value}"), self.series_color(series_index)));
            }
        }
        let line_height = context.measure_text("0", &font).height.max(1) as i32 + 2;
        let widest = lines
            .iter()
            .map(|(line, _)| context.measure_text(line, &font).width as i32)
            .max()
            .unwrap_or(0);
        let width = widest + 8;
        let height = line_height * lines.len() as i32 + 8;
        let mut x = band.x + 6;
        let mut y = band.y + 6;
        if x + width > plot_area.x + plot_area.width as i32 {
            x = (plot_area.x + plot_area.width as i32 - width).max(plot_area.x);
        }
        if y + height > plot_area.y + plot_area.height as i32 {
            y = (plot_area.y + plot_area.height as i32 - height).max(plot_area.y);
        }
        let box_rect = Rect::new(x, y, width.max(1) as u32, height.max(1) as u32);
        context.fill_rect(box_rect, surface);
        context.draw_rect(box_rect, ink.with_alpha_f32(0.35));
        for (line_index, (line, color)) in lines.iter().enumerate() {
            let line_rect = Rect::new(
                box_rect.x + 4,
                box_rect.y + 4 + line_index as i32 * line_height,
                box_rect.width.saturating_sub(8),
                line_height.max(1) as u32,
            );
            context.draw_text_fitted_cow(line_rect, line, &font, *color, HorizontalAlignment::Left);
        }
    }

    /// Maps a pointer position to the category slot under it.
    fn category_at(&self, pos: Point, plot_area: Rect) -> Option<usize> {
        let categories = self.category_count();
        if categories == 0 || !plot_area.contains_point(pos) {
            return None;
        }
        let index = match self.orientation {
            BarOrientation::Vertical => {
                let slot = (plot_area.width as f32 / categories as f32).max(1.0);
                ((pos.x - plot_area.x) as f32 / slot) as usize
            }
            BarOrientation::Horizontal => {
                let slot = (plot_area.height as f32 / categories as f32).max(1.0);
                ((pos.y - plot_area.y) as f32 / slot) as usize
            }
        };
        (index < categories).then_some(index)
    }
}

/// Draws a single-line label with the chart's shared styling.
///
/// The colour is derived from the active surface rather than written as `DARK_GRAY`:
/// that literal is a *light* chart's ink, so on the dark appearance the value labels
/// above the bars and the category labels below the axis rendered at **1.8:1** against
/// their own background — present in the pixel census, unreadable to a person. The
/// series colours are unchanged; only the framing text moves (rule #108 ③).
fn draw_label(context: &mut RenderContext, text: &str, band: Rect, font: &Font) {
    // Fitted to the band, so a long category name cannot run into the bar beside it — the
    // caller already knows each label's column, which is why the band is passed rather than a
    // point.
    context.draw_text_fitted_cow(
        band,
        text,
        font,
        axis_chrome_color(0.70),
        HorizontalAlignment::Center,
    );
}

/// Fallback backdrop for builds without the `chart` feature (tablet/mobile).
///
/// Kept intentionally minimal and structurally identical to what the shared
/// engine produces: same axes, same grid spacing, so a chart is recognisable
/// across profiles rather than looking like a different widget.
#[cfg(not(feature = "chart"))]
impl BarChart {
    fn draw_backdrop_without_engine(
        &self,
        context: &mut RenderContext,
        is_enabled: bool,
        disabled_color: Color,
    ) -> Rect {
        let plot_area = self.plot_area();
        let (axis_color, _, grid_color) = axis_chrome();
        let axis_color = if is_enabled { axis_color } else { disabled_color };
        let bottom = plot_area.y + plot_area.height as i32;

        context.draw_line_stroke(
            Point::new(plot_area.x, plot_area.y),
            Point::new(plot_area.x, bottom),
            axis_color,
            1,
        );
        context.draw_line_stroke(
            Point::new(plot_area.x, bottom),
            Point::new(plot_area.x + plot_area.width as i32, bottom),
            axis_color,
            1,
        );

        if self.show_grid {
            let grid_color = if is_enabled { grid_color } else { disabled_color };
            for tick in 0..=4 {
                let t = tick as f64 / 4.0;
                let gy = plot_area.y + (plot_area.height as f64 * (1.0 - t)) as i32;
                context.draw_line_aa(
                    Point::new(plot_area.x + 1, gy),
                    Point::new(plot_area.x + plot_area.width as i32 - 1, gy),
                    grid_color,
                );
            }
        }

        plot_area
    }
}

impl EventHandler for BarChart {
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
                let next = self.category_at(*pos, plot_area);
                if next != self.hovered {
                    if let Some(index) = self.hovered {
                        self.bar_unhovered.emit(index);
                        let series = self.series_at(*pos, plot_area, index);
                        self.bar_unhovered_with_series.emit((series, index));
                    }
                    self.hovered = next;
                    if let Some(index) = next {
                        self.bar_hovered.emit(index);
                        let series = self.series_at(*pos, plot_area, index);
                        self.bar_hovered_with_series.emit((series, index));
                    }
                    self.base.request_redraw();
                }
                // Drive the attached crosshair from the hovered category (COMP-F01).
                let crosshair_point = self.hovered.map(|index| {
                    let value = if self.series.is_empty() {
                        self.bars.get(index).map(|bar| bar.value).unwrap_or(0.0)
                    } else {
                        self.series
                            .iter()
                            .filter_map(|series| series.values.get(index).copied())
                            .fold(f64::NEG_INFINITY, f64::max)
                    };
                    let categories = self.category_count().max(1);
                    let slot = plot_area.width as f32 / categories as f32;
                    (
                        Point::new(
                            plot_area.x + (slot * (index as f32 + 0.5)) as i32,
                            plot_area.y + plot_area.height as i32 / 2,
                        ),
                        (index as f64, value),
                    )
                });
                if let Some(crosshair) = self.crosshair.as_mut() {
                    match crosshair_point {
                        Some((pixel, data)) => crosshair.show_at_data(pixel, data),
                        None => crosshair.hide(),
                    }
                }
            }
            Event::MouseLeave { .. } => {
                if let Some(crosshair) = self.crosshair.as_mut() {
                    crosshair.hide();
                }
                if let Some(index) = self.hovered.take() {
                    self.bar_unhovered.emit(index);
                    let series = self.series_at(Point::new(0, 0), plot_area, index);
                    self.bar_unhovered_with_series.emit((series, index));
                    self.base.request_redraw();
                }
            }
            Event::MousePress { pos, button, .. } if *button == 1 => {
                self.base.set_mouse_pressed(true);
                if let Some(index) = self.category_at(*pos, plot_area) {
                    self.bar_clicked.emit(index);
                    let series = self.series_at(*pos, plot_area, index);
                    self.bar_clicked_with_series.emit((series, index));
                }
            }
            Event::MouseRelease { button, .. } if *button == 1 => {
                self.base.set_mouse_pressed(false);
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
    fn published_bar_events_resolve_to_signals() {
        let chart = BarChart::new(Rect::new(0, 0, 300, 200));
        for name in ["bar_clicked", "bar_hovered", "bar_unhovered"] {
            assert!(chart.event_signal_dyn(name).is_some(), "{name} must resolve");
        }
        assert!(chart.event_signal_dyn("no_such_event").is_none());
    }

    #[test]
    fn bar_chart_default_creation() {
        let bc = BarChart::new(Rect::new(0, 0, 300, 200));
        assert_eq!(bc.kind(), WidgetKind::BarChart);
        assert_eq!(bc.bar_count(), 0);
        assert_eq!(bc.bar_color(), Color::PRIMARY);
        assert!((bc.bar_spacing() - 0.2).abs() < f32::EPSILON);
        assert!(bc.show_values());
        assert!(bc.show_grid());
    }

    #[test]
    fn bar_chart_set_bars() {
        let mut bc = BarChart::new(Rect::new(0, 0, 300, 200));
        let entries =
            vec![BarEntry::new("A", 10.0), BarEntry::new("B", 20.0), BarEntry::new("C", 15.0)];
        bc.set_bars(entries);
        assert_eq!(bc.bar_count(), 3);
        assert_eq!(bc.bars()[0].label, "A");
        assert!((bc.bars()[1].value - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn bar_chart_add_and_remove() {
        let mut bc = BarChart::new(Rect::new(0, 0, 300, 200));
        bc.add_bar(BarEntry::new("X", 5.0));
        bc.add_bar(BarEntry::new("Y", 10.0));
        bc.add_bar(BarEntry::new("Z", 15.0));
        assert_eq!(bc.bar_count(), 3);

        assert!(bc.remove_bar(1)); // Remove "Y"
        assert_eq!(bc.bar_count(), 2);
        assert_eq!(bc.bars()[1].label, "Z");

        assert!(!bc.remove_bar(5)); // Out of bounds
        assert_eq!(bc.bar_count(), 2);
    }

    #[test]
    fn bar_chart_clear_bars() {
        let mut bc = BarChart::new(Rect::new(0, 0, 300, 200));
        bc.add_bar(BarEntry::new("A", 1.0));
        bc.add_bar(BarEntry::new("B", 2.0));
        bc.clear_bars();
        assert_eq!(bc.bar_count(), 0);
    }

    #[test]
    fn bar_chart_bar_color_and_spacing() {
        let mut bc = BarChart::new(Rect::new(0, 0, 300, 200));
        bc.set_bar_color(Color::WARNING);
        assert_eq!(bc.bar_color(), Color::WARNING);
        bc.set_bar_spacing(0.5);
        assert!((bc.bar_spacing() - 0.5).abs() < f32::EPSILON);
        bc.set_bar_spacing(1.5); // clamp
        assert!((bc.bar_spacing() - 0.8).abs() < f32::EPSILON);
    }

    #[test]
    fn bar_chart_show_values_and_grid() {
        let mut bc = BarChart::new(Rect::new(0, 0, 300, 200));
        assert!(bc.show_values());
        bc.set_show_values(false);
        assert!(!bc.show_values());
        assert!(bc.show_grid());
        bc.set_show_grid(false);
        assert!(!bc.show_grid());
    }

    #[test]
    fn bar_chart_with_custom_colors() {
        let mut bc = BarChart::new(Rect::new(0, 0, 300, 200));
        bc.add_bar(BarEntry::new("A", 10.0).with_color(Color::ERROR));
        bc.add_bar(BarEntry::new("B", 20.0)); // Uses default bar_color
        assert_eq!(bc.bar_count(), 2);
        assert_eq!(bc.bars()[0].color, Some(Color::ERROR));
        assert!(bc.bars()[1].color.is_none());
    }

    #[test]
    fn bar_chart_svg_output() {
        let mut bc = BarChart::new(Rect::new(0, 0, 300, 200));
        bc.add_bar(BarEntry::new("A", 10.0));
        bc.add_bar(BarEntry::new("B", 20.0));
        bc.add_bar(BarEntry::new("C", 15.0));
        let svg = render_to_svg(&mut bc);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }

    #[test]
    fn bar_chart_empty_bars_no_crash() {
        let mut bc = BarChart::new(Rect::new(0, 0, 300, 200));
        let svg = render_to_svg(&mut bc);
        assert!(svg.starts_with("<svg"));
    }

    #[test]
    fn bar_chart_event_forwarding() {
        let mut bc = BarChart::new(Rect::new(0, 0, 300, 200));
        bc.handle_event(&Event::MouseMove { pos: Point::new(10, 10) });
        bc.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1, modifiers: 0 });
    }

    /// One ink box per text `<path>`, in document order.
    ///
    /// # Why the geometry and not the string
    ///
    /// Text leaves the backend as the `font8x8` rectangles the software rasteriser fills — one
    /// axis-aligned `<path>` subpath per set bitmap bit — so the rendered string is **not in the
    /// document in any form** and `svg.contains("100.0")` can never be true. A run can only be
    /// located by *where it is*: the union of one `<path>`'s subpaths is its ink box.
    ///
    /// # Why subpaths are not de-duplicated
    ///
    /// When a glyph box is wider than 8 pixels two bitmap columns map onto the same pixel via
    /// integer division, so the same rectangle is emitted twice. That is the rasteriser's own
    /// geometry — it fills that pixel twice — so collapsing it here would make this disagree with
    /// the drawing.
    ///
    /// # Why this delegates instead of parsing `d` itself
    ///
    /// The first form read the `font8x8` rectangle spelling directly: split `d` on `M` and take the
    /// four integers of `M{x} {y}h{w}v{h}`. That is the *bitmap* path's grammar and only its grammar.
    /// An outline face writes `M21.45 30.24L...Z` with fractional vertices, where the same split
    /// reads `21` as `x`, `45` as `y`, `30` as `w` — geometry that is not ink and not where the glyph
    /// is. The crate already has the correct reader, which knows both grammars (and the `data-text`
    /// provenance tag that distinguishes a run from a drawn shape); duplicating it here was the
    /// actual bug. One reader, so the two cannot disagree.
    #[cfg(feature = "chart")]
    fn ink_paths(svg: &str) -> Vec<(i32, i32, i32, i32)> {
        crate::widget::svg::text_ink_boxes(svg)
    }

    /// The bitmap face fills its whole glyph box (`font8x8` rectangles span the box), so a run's
    /// ink left *is* the box left, exactly. An **outline** face draws a real glyph whose ink is
    /// inset — a digit's stroke does not occupy the box's first column — so the ink's left edge sits
    /// one or two pixels right of the box edge at these sizes. A lookup that required
    /// `ink.left == box.left`, or an assertion that pinned the two equal, encoded a property of the
    /// bitmap face rather than of the layout, and is false the moment an outline is drawn.
    ///
    /// The bound is deliberately tight: it has to absorb the outline inset (≤ 2 px) and nothing
    /// more, so a run anchored a whole entry away still fails.
    ///
    /// Gated with its only consumers: the two assertions below are `#[cfg(feature = "chart")]`,
    /// because they exercise the shared tick engine, which does not exist without that feature. An
    /// ungated constant therefore became dead code in `tablet`/`mobile`/`mini` — where `chart` is
    /// off — and `clippy -D warnings` reported it there while `desktop` stayed green.
    #[cfg(feature = "chart")]
    const INK_INSET_TOLERANCE: i32 = 3;

    /// The widget must render through the shared chart engine rather than its
    /// own copy of the axis/tick math — that duplication is what this refactor
    /// removed.
    ///
    /// Observable proof: `draw_y_ticks` emits a numeric value label for every
    /// tick, which the widget's previous hand-rolled grid loop did not draw. The labels are not
    /// searchable as strings — they are `font8x8` rectangles now — so the check is on where the
    /// ink landed. `draw_text` anchors every y-tick label's glyph box at `plot_x - 44 = 20`, and
    /// the value decides how much ink follows: `100.0` is 17 px wide, `0.0` is 29 px because a
    /// three-character label is ellipsised to `0…` in the 29 px-wide `bar_width` band. A widget
    /// drawing only bars produces no run on that anchor at all.
    ///
    /// Gated on the feature, like `ink_paths` and the module's other `chart`-dependent items:
    /// a build without `chart` compiles this test module and the engine it exercises is absent.
    #[cfg(feature = "chart")]
    #[test]
    fn bar_chart_renders_axis_value_labels_from_the_shared_engine() {
        // Holds the crate-wide theme guard: this test renders, and a concurrent
        // test that switches the appearance would otherwise change a later frame.
        let _theme_guard = crate::style::theme_test_guard();
        let mut bc = BarChart::new(Rect::new(0, 0, 300, 200));
        bc.set_bars(vec![BarEntry::new("A", 0.0), BarEntry::new("B", 100.0)]);
        bc.set_value_range(Some(0.0), Some(100.0));

        let svg = render_to_svg(&mut bc);
        let runs = ink_paths(&svg);

        // `draw_y_ticks` walks `tick_count = 4` plus the inclusive upper bound, so five labels
        // are painted. The bars' own `{:.1}` value labels are here too, which is why the
        // assertion picks the runs out by geometry rather than by count.
        assert!(
            runs.len() >= 8,
            "the shared tick engine must label every tick (bars included), got {} runs",
            runs.len()
        );
        let (left, _, right, _) = runs[0];
        // The engine's anchor is where the glyph **box** starts, at `plot_x - 44`; under the bitmap
        // face the ink's left edge *is* that box edge, but an outline glyph is inset a pixel or two
        // (its leftmost column carries no set bit), so the ink starts `INK_INSET_TOLERANCE` within it.
        // Pinning the two equal encoded a property of the bitmap face, not of the layout.
        assert!(
            (left - 20).abs() <= INK_INSET_TOLERANCE,
            "the topmost y-tick label starts at the engine's `plot_x - 44`"
        );
        // An outline's ink is the glyph's **real** width, which is narrower than the advance, so the
        // band is an upper bound rather than an equality: `100.0` is five characters and cannot
        // exceed the five advances they occupy.
        assert!(
            right - left <= 17,
            "...and `100.0` is at most five characters of ink, not the bar label"
        );
        let (short_left, _, short_right, _) = runs[4];
        // `0.0` is three characters ellipsised to `0…`, whose bitmap ink spans the whole 29 px bar
        // band; the outline face draws the real, narrower glyphs, so the band is an upper bound.
        assert!(
            (short_left - 20).abs() <= INK_INSET_TOLERANCE,
            "the bottom y-tick label starts on the same anchor"
        );
        assert!(
            short_right - short_left <= 29,
            "the bottom y-tick label is `0.0`, which the 29 px bar band ellipsises to `0…`"
        );
        assert!(
            runs.iter().any(|run| (run.0 - 20).abs() <= INK_INSET_TOLERANCE && run.2 - run.0 <= 17),
            "at least one five-character label sits on the y-axis anchor"
        );
        // The two runs differ, so the engine drew a *value* label rather than repeating one
        // string at every tick: a widget that lost the tick loop's `min_y + span * t` would
        // paint five identical boxes.
        assert_ne!(runs[0], runs[4], "the ticks carry different values, so different ink");
    }

    /// Grid toggling must reach the shared engine: `draw_y_ticks` adds grid lines
    /// when asked, so enabling the grid must increase the line count.
    #[test]
    fn bar_chart_grid_toggle_changes_rendered_line_count() {
        let mut bc = BarChart::new(Rect::new(0, 0, 300, 200));
        bc.set_bars(vec![BarEntry::new("A", 10.0), BarEntry::new("B", 20.0)]);

        bc.set_show_grid(false);
        let without_grid = render_to_svg(&mut bc).matches("<line").count();

        bc.set_show_grid(true);
        let with_grid = render_to_svg(&mut bc).matches("<line").count();

        assert!(
            with_grid > without_grid,
            "grid must add lines through the shared engine (off={without_grid}, on={with_grid})"
        );
    }

    /// N-CH-04: non-finite bar values are dropped at the data entry.
    #[test]
    fn non_finite_bars_are_dropped_at_entry() {
        let mut bc = BarChart::new(Rect::new(0, 0, 200, 100));
        bc.set_bars(vec![
            BarEntry::new("a", 1.0),
            BarEntry::new("bad", f64::NAN),
            BarEntry::new("inf", f64::INFINITY),
            BarEntry::new("b", 2.0),
        ]);
        let labels: Vec<&str> = bc.bars().iter().map(|b| b.label.as_str()).collect();
        assert_eq!(labels, vec!["a", "b"], "only finite bars survive");

        let before = bc.bars().len();
        bc.add_bar(BarEntry::new("nan", f64::NAN));
        assert_eq!(bc.bars().len(), before, "add_bar must refuse a non-finite value");
        bc.add_bar(BarEntry::new("c", 3.0));
        assert_eq!(bc.bars().len(), before + 1);
    }

    // ── D09-STYLE-01: the axis/category label consumes the resolved theme/caller font ──

    /// A caller-authored `style.font` must change the ink the category axis labels paint.
    ///
    /// # The defect this pins
    ///
    /// The value and category labels built a fixed `sans-serif` face by hand, so the theme body
    /// font and the user's text scale never reached the chart chrome. They are now derived from
    /// `metrics::effective_font` at their role sizes.
    #[test]
    fn a_custom_font_changes_the_category_axis_label_ink() {
        let render = |font: Option<crate::core::Font>| {
            let mut bc = BarChart::new(Rect::new(0, 0, 400, 300));
            bc.set_bars(vec![BarEntry::new("Category", 5.0), BarEntry::new("Second", 3.0)]);
            if let Some(font) = font {
                bc.set_style(crate::style::WidgetStyle::default().with_font(font));
            }
            render_to_svg(&mut bc)
        };

        let base = render(None);
        let big = render(Some(crate::core::Font::simple("Test", 30.0)));
        // The control must actually carry the caller font in its resolved style; if this fails the
        // defect is upstream of the chart's own measurement.
        let mut probe = BarChart::new(Rect::new(0, 0, 400, 300));
        probe.set_style(
            crate::style::WidgetStyle::default().with_font(crate::core::Font::simple("Test", 30.0)),
        );
        assert_eq!(
            crate::widget::metrics::effective_font(probe.style()).size(),
            30.0,
            "the caller font must reach the chart's resolved style"
        );
        assert!(!base.is_empty(), "the chart must paint something for this test to mean anything");
        assert_ne!(base, big, "a caller-authored larger font must change what the chart paints");
    }

    /// Multi-series grouped and stacked modes both paint, and the model round-trips.
    #[test]
    fn multi_series_grouped_and_stacked_render() {
        let mut bc = BarChart::new(Rect::new(0, 0, 400, 260));
        bc.set_series(
            vec![
                BarSeries::new("revenue", vec![3.0, 5.0, 2.0]),
                BarSeries::new("cost", vec![1.0, 2.0, 4.0]),
            ],
            vec!["Q1".to_string(), "Q2".to_string(), "Q3".to_string()],
        );
        assert_eq!(bc.series().len(), 2);
        assert_eq!(bc.category_count(), 3);
        let grouped = render_to_svg(&mut bc);
        bc.set_grouping(BarGrouping::Stacked);
        let stacked = render_to_svg(&mut bc);
        assert!(grouped.starts_with("<svg") && stacked.starts_with("<svg"));
        assert_ne!(grouped, stacked, "stacking changes the painted bars");
        bc.set_orientation(BarOrientation::Horizontal);
        assert_eq!(bc.orientation(), BarOrientation::Horizontal);
        assert!(render_to_svg(&mut bc).starts_with("<svg"));
    }

    /// The growth animation is opt-in and advances toward its target.
    #[test]
    fn bar_growth_animation_advances_when_enabled() {
        let mut bc = BarChart::new(Rect::new(0, 0, 300, 200));
        bc.set_bars(vec![BarEntry::new("A", 5.0)]);
        assert_eq!(bc.growth_progress(), 1.0, "the default is already grown");
        bc.set_animate_bars(true);
        bc.set_bars(vec![BarEntry::new("A", 5.0)]);
        assert_eq!(bc.growth_progress(), 0.0);
        assert!(bc.is_animating());
        let _ = bc.tick(16);
        assert!(bc.growth_progress() > 0.0, "a frame advances the reveal");
    }

    /// Hover reports the category under the pointer and leaves cleanly.
    #[test]
    fn hovering_a_category_reports_it() {
        let mut bc = BarChart::new(Rect::new(0, 0, 300, 200));
        bc.set_bars(vec![BarEntry::new("A", 1.0), BarEntry::new("B", 2.0)]);
        let _ = render_to_svg(&mut bc);
        let plot = bc.last_plot_area.expect("the paint records the plot area");
        bc.handle_event(&Event::MouseMove {
            pos: Point::new(plot.x + plot.width as i32 / 4, plot.y + 4),
        });
        assert_eq!(bc.hovered_bar(), Some(0));
        bc.handle_event(&Event::MouseLeave { pos: Point::new(0, 0) });
        assert_eq!(bc.hovered_bar(), None);
    }

    // ── Value-change easing (issue-20261023, BarChart) ──

    /// The easing is opt-in: with the flag off, an update is painted on the next frame and the
    /// document is identical to a chart built directly from the new data.
    #[test]
    fn value_changes_are_immediate_and_identical_when_the_flag_is_off() {
        let _theme_guard = crate::style::theme_test_guard();
        let mut chart = BarChart::new(Rect::new(0, 0, 300, 200));
        chart.set_bars(vec![BarEntry::new("A", 10.0), BarEntry::new("B", 20.0)]);
        let before = render_to_svg(&mut chart);

        chart.set_bars(vec![BarEntry::new("A", 4.0), BarEntry::new("B", 8.0)]);
        assert!(!chart.animate_value_changes(), "the easing is opt-in");
        assert!(!chart.is_animating(), "default-off updates do not owe frames");
        let after = render_to_svg(&mut chart);

        let mut fresh = BarChart::new(Rect::new(0, 0, 300, 200));
        fresh.set_bars(vec![BarEntry::new("A", 4.0), BarEntry::new("B", 8.0)]);
        assert_eq!(
            after,
            render_to_svg(&mut fresh),
            "default-off must paint the new data immediately"
        );
        assert_ne!(before, after, "the update must still change the painted heights");
    }

    /// One frame into a transition draws a value strictly between the old and the new one.
    #[test]
    fn a_value_change_eases_between_the_old_and_new_heights() {
        let mut chart = BarChart::new(Rect::new(0, 0, 300, 200));
        chart.set_bars(vec![BarEntry::new("A", 0.0), BarEntry::new("B", 10.0)]);
        chart.set_animate_value_changes(true);

        chart.set_bars(vec![BarEntry::new("A", 100.0), BarEntry::new("B", -30.0)]);
        assert!(chart.is_animating(), "the update starts a transition");
        assert_eq!(chart.rendered_bar_value(0), Some(0.0), "the first frame is the old value");
        assert_eq!(chart.rendered_bar_value(1), Some(10.0));

        assert!(chart.tick(16), "a short frame leaves frames owing");
        let a = chart.rendered_bar_value(0).expect("bar A exists");
        let b = chart.rendered_bar_value(1).expect("bar B exists");
        assert!(a > 0.0 && a < 100.0, "bar A must be strictly inside its ends, got {a}");
        assert!(b < 10.0 && b > -30.0, "a negative target eases too, got {b}");
    }

    /// The transition lands exactly on the new values and stops owing frames.
    #[test]
    fn a_value_transition_settles_exactly_on_its_target() {
        let mut chart = BarChart::new(Rect::new(0, 0, 300, 200));
        chart.set_bars(vec![BarEntry::new("A", 0.0), BarEntry::new("B", 10.0)]);
        chart.set_animate_value_changes(true);
        chart.set_bars(vec![BarEntry::new("A", 42.0), BarEntry::new("B", -7.0)]);

        let mut frames = 0;
        while chart.tick(16) {
            frames += 1;
            assert!(frames < 200, "the transition must terminate rather than tick forever");
        }
        assert!(frames > 1, "easing must span more than one frame, or it is a cut");
        assert_eq!(chart.rendered_bar_value(0), Some(42.0));
        assert_eq!(chart.rendered_bar_value(1), Some(-7.0));
        assert!(!chart.is_animating());
        assert!(!chart.tick(16), "a settled transition stays settled");
    }

    /// A count change has no compatible previous frame, so it jumps instead of easing.
    #[test]
    fn a_shape_change_jumps_instead_of_easing() {
        let mut chart = BarChart::new(Rect::new(0, 0, 300, 200));
        chart.set_bars(vec![BarEntry::new("A", 0.0), BarEntry::new("B", 0.0)]);
        chart.set_animate_value_changes(true);
        chart.set_bars(vec![BarEntry::new("A", 10.0), BarEntry::new("B", 20.0)]);
        assert!(chart.is_animating());

        chart.set_bars(vec![
            BarEntry::new("A", 1.0),
            BarEntry::new("B", 2.0),
            BarEntry::new("C", 3.0),
        ]);
        assert!(!chart.is_animating(), "a third category must jump, not interpolate");
        assert_eq!(chart.rendered_bar_value(0), Some(1.0));
        assert_eq!(chart.rendered_bar_value(2), Some(3.0));

        chart.set_series(
            vec![BarSeries::new("s", vec![5.0, 6.0])],
            vec!["x".to_string(), "y".to_string()],
        );
        assert!(!chart.is_animating(), "switching models must jump, not interpolate");
        assert_eq!(chart.rendered_series_value(0, 1), Some(6.0));
    }

    /// Turning the flag off mid-flight lands on the new values at once.
    #[test]
    fn disabling_value_easing_mid_transition_lands_on_the_target() {
        let mut chart = BarChart::new(Rect::new(0, 0, 300, 200));
        chart.set_bars(vec![BarEntry::new("A", 0.0)]);
        chart.set_animate_value_changes(true);
        chart.set_bars(vec![BarEntry::new("A", 50.0)]);
        assert!(chart.tick(16));
        let mid = chart.rendered_bar_value(0).expect("bar A exists");
        assert!(mid > 0.0 && mid < 50.0, "the fixture must be mid-flight: {mid}");

        chart.set_animate_value_changes(false);
        assert!(!chart.animate_value_changes());
        assert!(!chart.is_animating());
        assert_eq!(chart.rendered_bar_value(0), Some(50.0), "the target is drawn at once");
        assert!(!chart.tick(16), "the dropped transition owes no frames");
    }

    /// Every series and category eases from its own previous value, and a per-series count
    /// change is a shape change that jumps.
    #[test]
    fn multi_series_value_changes_ease_per_series_and_category() {
        let mut chart = BarChart::new(Rect::new(0, 0, 400, 260));
        chart.set_series(
            vec![
                BarSeries::new("revenue", vec![0.0, 10.0]),
                BarSeries::new("cost", vec![5.0, 0.0]),
            ],
            vec!["Q1".to_string(), "Q2".to_string()],
        );
        chart.set_animate_value_changes(true);
        chart.set_series(
            vec![
                BarSeries::new("revenue", vec![100.0, 50.0]),
                BarSeries::new("cost", vec![25.0, 40.0]),
            ],
            vec!["Q1".to_string(), "Q2".to_string()],
        );
        assert!(chart.is_animating());
        assert_eq!(chart.rendered_series_value(0, 0), Some(0.0));
        assert_eq!(chart.rendered_series_value(1, 1), Some(0.0));

        assert!(chart.tick(16));
        let revenue = chart.rendered_series_value(0, 0).expect("revenue Q1 exists");
        let cost = chart.rendered_series_value(1, 0).expect("cost Q1 exists");
        let revenue_q2 = chart.rendered_series_value(0, 1).expect("revenue Q2 exists");
        assert!(revenue > 0.0 && revenue < 100.0, "revenue Q1 eased, got {revenue}");
        assert!(cost > 5.0 && cost < 25.0, "cost Q1 eased, got {cost}");
        assert!(revenue_q2 > 10.0 && revenue_q2 < 50.0, "revenue Q2 eased, got {revenue_q2}");

        chart.set_series(
            vec![BarSeries::new("revenue", vec![1.0]), BarSeries::new("cost", vec![2.0])],
            vec!["Q1".to_string()],
        );
        assert!(!chart.is_animating(), "a per-series count change must jump");
        assert_eq!(chart.rendered_series_value(0, 0), Some(1.0));
        assert_eq!(chart.rendered_series_value(1, 0), Some(2.0));
    }

    /// Re-reporting the same values is not a movement, and negative targets ease through zero.
    #[test]
    fn zero_and_negative_value_changes_behave() {
        let mut chart = BarChart::new(Rect::new(0, 0, 300, 200));
        chart.set_bars(vec![BarEntry::new("A", 5.0), BarEntry::new("B", 0.0)]);
        chart.set_animate_value_changes(true);

        chart.set_bars(vec![BarEntry::new("A", 5.0), BarEntry::new("B", 0.0)]);
        assert!(!chart.is_animating(), "an unchanged value has nothing to ease");
        assert_eq!(chart.rendered_bar_value(0), Some(5.0));

        chart.set_bars(vec![BarEntry::new("A", 5.0), BarEntry::new("B", -20.0)]);
        assert!(chart.is_animating());
        assert!(chart.tick(16));
        let value = chart.rendered_bar_value(1).expect("bar B exists");
        assert!(value < 0.0 && value > -20.0, "the negative value eased, got {value}");
        assert_eq!(
            chart.rendered_bar_value(0),
            Some(5.0),
            "a bar whose value did not change stays put"
        );

        while chart.tick(16) {}
        assert_eq!(chart.rendered_bar_value(1), Some(-20.0));
        assert!(!chart.is_animating());
    }

    /// CH-03/CH-04/CH-07: the tooltip lists every series, the signals name the series, and the
    /// title is painted.
    #[test]
    fn series_tooltip_signals_and_title_are_wired() {
        let mut chart = BarChart::new(Rect::new(0, 0, 400, 300));
        chart.set_series(
            vec![BarSeries::new("Revenue", vec![10.0]), BarSeries::new("Cost", vec![5.0])],
            vec!["Q1".to_string()],
        );
        chart.set_show_tooltip(true);
        chart.set_show_legend(true);
        chart.set_title("Totals");
        chart.set_x_axis_label("Quarter");
        chart.set_y_axis_label("USD");
        let baseline = crate::widget::svg::render_to_svg(&mut chart);
        let runs_without_tooltip = crate::widget::svg::text_ink_boxes(&baseline).len();
        let plot = chart.last_plot_area.expect("plot area");

        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = std::sync::Arc::clone(&seen);
        chart.bar_hovered_with_series.connect(move |pair| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*pair);
            }
        });

        // The first eighth of the single category is inside the first series' strip.
        let pos = Point::new(plot.x + plot.width as i32 / 8, plot.y + plot.height as i32 / 2);
        chart.handle_event(&Event::MouseMove { pos });
        assert_eq!(*seen.lock().unwrap(), vec![(0, 0)], "the signal names (series, category)");

        let svg = crate::widget::svg::render_to_svg(&mut chart);
        assert!(
            crate::widget::svg::text_ink_boxes(&svg).len() > runs_without_tooltip,
            "the tooltip adds text runs"
        );
    }
}
