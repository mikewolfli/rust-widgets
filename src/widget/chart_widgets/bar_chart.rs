//! BarChart widget — a vertical bar chart for visualizing categorical data.
//!
//! The BarChart widget draws axes, optional grid lines, vertical bars with
//! optional value labels on top. Each bar can have its own color, or all bars
//! share a default color.

use crate::core::{Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

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

    /// Returns the plot area (inside margins for labels).
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
}

impl Draw for BarChart {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.base.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }

        let plot_area = self.plot_area();
        let (y_min, y_max) = self.resolve_y_range();

        let is_enabled = self.base.is_enabled();
        let disabled_color = Color::DISABLED_FOREGROUND;

        // ── Draw axes ──
        let axis_color = if is_enabled { Color::DARK_GRAY } else { disabled_color };
        // Y-axis (left edge)
        context.draw_line_stroke(
            Point::new(plot_area.x, plot_area.y),
            Point::new(plot_area.x, plot_area.y + plot_area.height as i32),
            axis_color,
            1,
        );
        // X-axis (bottom edge)
        context.draw_line_stroke(
            Point::new(plot_area.x, plot_area.y + plot_area.height as i32),
            Point::new(plot_area.x + plot_area.width as i32, plot_area.y + plot_area.height as i32),
            axis_color,
            1,
        );

        // ── Draw horizontal grid lines ──
        let grid_color = if is_enabled { Color::rgba(200, 200, 200, 120) } else { disabled_color };
        if self.show_grid {
            for i in 0..=4 {
                let t = i as f64 / 4.0;
                let gy = plot_area.y + (plot_area.height as f64 * (1.0 - t)) as i32;
                context.draw_line_aa(
                    Point::new(plot_area.x + 1, gy),
                    Point::new(plot_area.x + plot_area.width as i32 - 1, gy),
                    grid_color,
                );
            }
        }

        // ── Draw bars ──
        if self.bars.is_empty() {
            return;
        }

        let n = self.bars.len();
        let total_slots = n as f32;
        let spacing_pixels = (plot_area.width as f32 * self.bar_spacing) / total_slots;
        let bar_slot_width =
            (plot_area.width as f32 - spacing_pixels * (total_slots + 1.0)) / total_slots;
        let bar_width = bar_slot_width.max(1.0);

        let baseline_y = plot_area.y + plot_area.height as i32;

        for (i, bar) in self.bars.iter().enumerate() {
            let bar_color = bar.color.unwrap_or(self.bar_color);
            let effective_color = if is_enabled { bar_color } else { disabled_color };

            let bar_x = plot_area.x
                + (spacing_pixels * (i as f32 + 1.0) + bar_slot_width * i as f32) as i32;
            let bar_height =
                ((bar.value - y_min) / (y_max - y_min) * plot_area.height as f64) as i32;
            let bar_y = baseline_y - bar_height;

            // Draw the bar
            if bar_height > 0 {
                context.fill_rect(
                    Rect::new(bar_x, bar_y, bar_width as u32, bar_height as u32),
                    effective_color,
                );
            }

            // ── Draw value label on top of bar ──
            if self.show_values && is_enabled {
                let label_font = Font::new("sans-serif", 10.0, false, false);
                let label = format!("{:.1}", bar.value);
                let metrics = context.measure_text(&label, &label_font);
                let label_x = bar_x + (bar_width as i32 - metrics.width as i32) / 2;
                let label_y = bar_y - 4;
                context.draw_text(
                    Point::new(label_x.max(plot_area.x), label_y),
                    &label,
                    &label_font,
                    Color::DARK_GRAY,
                );
            }

            // ── Draw category label below axis ──
            if is_enabled {
                let cat_font = Font::new("sans-serif", 9.0, false, false);
                let metrics = context.measure_text(&bar.label, &cat_font);
                let label_x = bar_x + (bar_width as i32 - metrics.width as i32) / 2;
                let label_y = baseline_y + metrics.height as i32 + 2;
                context.draw_text(
                    Point::new(label_x.max(plot_area.x), label_y),
                    &bar.label,
                    &cat_font,
                    Color::DARK_GRAY,
                );
            }
        }
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
}
