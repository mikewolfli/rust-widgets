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
    compute_cartesian_layout_measured, draw_legend_entries, draw_y_ticks, CartesianLayout,
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
    /// The plot area from the last paint, reused for hit testing.
    last_plot_area: Option<Rect>,
    /// Emitted with the category index under the pointer when hover enters a bar.
    pub bar_hovered: Signal1<usize>,
    /// Emitted with the category index when the pointer leaves a bar.
    pub bar_unhovered: Signal1<usize>,
    /// Emitted with the category index of a clicked bar.
    pub bar_clicked: Signal1<usize>,
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
            last_plot_area: None,
            bar_hovered: Signal1::new(),
            bar_unhovered: Signal1::new(),
            bar_clicked: Signal1::new(),
        }
    }

    /// Replaces the multi-series data and its category labels at once.
    ///
    /// Non-finite values are dropped. When `series` is non-empty the chart draws in
    /// multi-series mode and ignores [`Self::bars`]; setting an empty list returns it to the
    /// single-series model.
    pub fn set_series(&mut self, series: Vec<BarSeries>, category_labels: Vec<String>) {
        self.series = series
            .into_iter()
            .map(|mut item| {
                item.values.retain(|value| value.is_finite());
                item
            })
            .collect();
        self.category_labels = category_labels;
        self.restart_growth();
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

    /// Drops a hover index the new data no longer has a bar for.
    fn revalidate_hover(&mut self) {
        let categories = self.category_count();
        if self.hovered.is_some_and(|index| index >= categories) {
            if let Some(index) = self.hovered.take() {
                self.bar_unhovered.emit(index);
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
        self.bars = entries.into_iter().filter(|bar| bar.value.is_finite()).collect();
        self.restart_growth();
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
        self.bars.push(entry);
        self.base.request_redraw();
    }

    /// Removes the bar at the given index.
    /// Returns `true` if the bar was removed, `false` if the index was out of bounds.
    pub fn remove_bar(&mut self, index: usize) -> bool {
        if index < self.bars.len() {
            self.bars.remove(index);
            self.base.request_redraw();
            true
        } else {
            false
        }
    }

    /// Removes all bars from the chart.
    pub fn clear_bars(&mut self) {
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
            for bar in &self.bars {
                min = min.min(bar.value);
                max = max.max(bar.value);
            }
        } else if self.grouping == BarGrouping::Stacked {
            let categories = self.category_count();
            for category in 0..categories {
                let (mut positive, mut negative) = (0.0f64, 0.0f64);
                for series in &self.series {
                    if let Some(value) = series.values.get(category) {
                        if *value >= 0.0 {
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
            for series in &self.series {
                for value in &series.values {
                    min = min.min(*value);
                    max = max.max(*value);
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

    /// Advances the bar growth animation by `delta_ms`.
    fn tick(&mut self, delta_ms: u32) -> bool {
        if !self.animate_bars {
            return false;
        }
        self.reveal.tick(delta_ms)
    }

    /// Whether the bar growth animation still owes frames.
    fn is_animating(&self) -> bool {
        self.animate_bars && self.reveal.is_moving()
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
                let mut widest = 0u32;
                for step in 0..=4 {
                    let value = y_min + (y_max - y_min) * f64::from(step) / 4.0;
                    widest = widest.max(context.measure_text(&format!("{value:.1}"), &font).width);
                }
                widest as f32
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
                let entries: Vec<(&str, Color)> = self
                    .series
                    .iter()
                    .enumerate()
                    .filter(|(_, series)| !series.name.is_empty())
                    .map(|(index, series)| (series.name.as_str(), self.series_color(index)))
                    .collect();
                draw_legend_entries(&mut adapter, &layout, &entries);
            }
            plot_area
        };

        #[cfg(not(feature = "chart"))]
        let plot_area = self.draw_backdrop_without_engine(context, is_enabled, disabled_color);
        self.last_plot_area = Some(plot_area);

        // ── Bars ──
        if !self.series.is_empty() {
            self.draw_series_bars(context, plot_area, y_min, y_max, is_enabled);
            self.draw_hover_overlay(context, plot_area);
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
            let bar_color = bar.color.unwrap_or(self.bar_color);
            let effective_color = if is_enabled { bar_color } else { disabled_color };

            let bar_x = plot_area.x
                + (spacing_pixels * (i as f32 + 1.0) + bar_slot_width * i as f32) as i32;
            let bar_height = ((bar.value - y_min) * growth / value_span * height_range) as i32;
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
                let _ = core::fmt::Write::write_fmt(&mut label, format_args!("{:.1}", bar.value));
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
                    for (series_index, series) in self.series.iter().enumerate() {
                        let Some(value) = series.values.get(category).copied() else {
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
                    for (series_index, series) in self.series.iter().enumerate() {
                        let Some(value) = series.values.get(category).copied() else {
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
        let label = self
            .category_labels
            .get(index)
            .cloned()
            .or_else(|| self.bars.get(index).map(|bar| bar.label.clone()))
            .unwrap_or_else(|| index.to_string());
        let font = role_font(self.base.style(), 10.0);
        let metrics = context.measure_text(&label, &font);
        let width = metrics.width as i32 + 8;
        let height = metrics.height.max(1) as i32 + 8;
        let mut x = band.x + 6;
        let mut y = band.y + 6;
        if x + width > plot_area.x + plot_area.width as i32 {
            x = (plot_area.x + plot_area.width as i32 - width).max(plot_area.x);
        }
        if y + height > plot_area.y + plot_area.height as i32 {
            y = (plot_area.y + plot_area.height as i32 - height).max(plot_area.y);
        }
        let surface = self.base.style().background_color.unwrap_or_else(|| axis_chrome_color(0.96));
        let ink = surface.contrast_color();
        let box_rect = Rect::new(x, y, width.max(1) as u32, height.max(1) as u32);
        context.fill_rect(box_rect, surface);
        context.draw_rect(box_rect, ink.with_alpha_f32(0.35));
        context.draw_text_line(box_rect, &label, &font, ink, HorizontalAlignment::Center);
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
                    }
                    self.hovered = next;
                    if let Some(index) = next {
                        self.bar_hovered.emit(index);
                    }
                    self.base.request_redraw();
                }
            }
            Event::MouseLeave { .. } => {
                if let Some(index) = self.hovered.take() {
                    self.bar_unhovered.emit(index);
                    self.base.request_redraw();
                }
            }
            Event::MousePress { pos, button, .. } if *button == 1 => {
                self.base.set_mouse_pressed(true);
                if let Some(index) = self.category_at(*pos, plot_area) {
                    self.bar_clicked.emit(index);
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
}
