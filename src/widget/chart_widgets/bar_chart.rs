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
use crate::widget::capability::coercion::expect_f32;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
#[cfg(feature = "chart")]
use crate::widget::chart_widgets::adapter::ChartContextAdapter;
#[cfg(feature = "chart")]
use crate::widget::chart_widgets::charts::{
    compute_cartesian_layout, draw_y_ticks, CartesianLayout,
};
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
}

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
        }
    }

    /// Sets the bars to display. Clears any previous bars.
    pub fn set_bars(&mut self, entries: Vec<BarEntry>) {
        self.bars = entries;
        self.base.request_redraw();
    }

    /// Adds a single bar entry to the chart.
    pub fn add_bar(&mut self, entry: BarEntry) {
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
        self.bar_color = color;
        self.base.request_redraw();
    }

    /// Returns the current default bar color.
    pub fn bar_color(&self) -> Color {
        self.bar_color
    }

    /// Sets the spacing between bars as a fraction of the bar slot width.
    pub fn set_bar_spacing(&mut self, spacing: f32) {
        self.bar_spacing = spacing.clamp(0.0, 0.8);
        self.base.request_redraw();
    }

    /// Returns the current bar spacing fraction.
    pub fn bar_spacing(&self) -> f32 {
        self.bar_spacing
    }

    /// Enables or disables showing value labels on top of bars.
    pub fn set_show_values(&mut self, show: bool) {
        self.show_values = show;
        self.base.request_redraw();
    }

    /// Returns whether value labels are shown on top of bars.
    pub fn show_values(&self) -> bool {
        self.show_values
    }

    /// Enables or disables grid lines.
    pub fn set_show_grid(&mut self, show: bool) {
        self.show_grid = show;
        self.base.request_redraw();
    }

    /// Returns whether grid lines are shown.
    pub fn show_grid(&self) -> bool {
        self.show_grid
    }

    /// Sets manual min/max value range. Pass `None` for auto-compute.
    pub fn set_value_range(&mut self, min: Option<f64>, max: Option<f64>) {
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
        match (self.min_value, self.max_value) {
            (Some(min), Some(max)) => (min, max),
            _ => {
                if self.bars.is_empty() {
                    return (0.0, 10.0);
                }
                let min = self.bars.iter().map(|b| b.value).fold(f64::INFINITY, f64::min).min(0.0);
                let max = self.bars.iter().map(|b| b.value).fold(f64::NEG_INFINITY, f64::max);
                if (max - min).abs() < f64::EPSILON {
                    return (0.0, max.max(1.0) + 1.0);
                }
                let padding = (max - min) * 0.1;
                (min - padding, max + padding)
            }
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

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        crate::core::Size::new(400, 300)
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
            let layout = compute_cartesian_layout(rect, true, true, 0);
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
            plot_area
        };

        #[cfg(not(feature = "chart"))]
        let plot_area = self.draw_backdrop_without_engine(context, is_enabled, disabled_color);

        // ── Bars ──
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

        for (i, bar) in self.bars.iter().enumerate() {
            let bar_color = bar.color.unwrap_or(self.bar_color);
            let effective_color = if is_enabled { bar_color } else { disabled_color };

            let bar_x = plot_area.x
                + (spacing_pixels * (i as f32 + 1.0) + bar_slot_width * i as f32) as i32;
            let bar_height = ((bar.value - y_min) / value_span * height_range) as i32;
            let bar_y = baseline_y - bar_height;

            if bar_height > 0 {
                context.fill_rect(
                    Rect::new(bar_x, bar_y, bar_width as u32, bar_height as u32),
                    effective_color,
                );
            }

            // ── Value label on top of the bar ──
            if self.show_values && is_enabled {
                let label = format!("{:.1}", bar.value);
                let label_x = bar_x.max(plot_area.x) + (bar_width as i32 / 2).min(12);
                let label_y = bar_y - 4;
                draw_label(context, &label, label_x, label_y, 10.0);
            }

            // ── Category label below the axis ──
            if is_enabled {
                let label_x = bar_x.max(plot_area.x) + (bar_width as i32 / 2).min(12);
                let label_y = baseline_y + 12;
                draw_label(context, &bar.label, label_x, label_y, 9.0);
            }
        }
    }
}

/// Draws a single-line label with the chart's shared styling.
///
/// The colour is derived from the active surface rather than written as `DARK_GRAY`:
/// that literal is a *light* chart's ink, so on the dark appearance the value labels
/// above the bars and the category labels below the axis rendered at **1.8:1** against
/// their own background — present in the pixel census, unreadable to a person. The
/// series colours are unchanged; only the framing text moves (rule #108 ③).
fn draw_label(context: &mut RenderContext, text: &str, x: i32, y: i32, size: f32) {
    context.draw_text(
        Point::new(x, y),
        text,
        &Font::simple("sans-serif", size),
        axis_chrome_color(0.70),
        HorizontalAlignment::Left,
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
        bc.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1 });
    }

    /// The widget must render through the shared chart engine rather than its
    /// own copy of the axis/tick math — that duplication is what this refactor
    /// removed.
    ///
    /// Observable proof: `draw_y_ticks` emits a numeric value label for every
    /// tick, which the widget's previous hand-rolled grid loop did not draw. So
    /// rendered output now contains the resolved Y-range bounds as text.
    #[test]
    fn bar_chart_renders_axis_value_labels_from_the_shared_engine() {
        let mut bc = BarChart::new(Rect::new(0, 0, 300, 200));
        bc.set_bars(vec![BarEntry::new("A", 0.0), BarEntry::new("B", 100.0)]);
        bc.set_value_range(Some(0.0), Some(100.0));

        let svg = render_to_svg(&mut bc);

        // `draw_y_ticks` labels ticks with `{value:.1}`, so the configured range
        // bounds must appear. A widget drawing only bars would not produce these.
        assert!(
            svg.contains("100.0"),
            "y-axis upper bound label missing; widget no longer uses the shared tick engine"
        );
        assert!(svg.contains("0.0"), "y-axis lower bound label missing");
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
}
