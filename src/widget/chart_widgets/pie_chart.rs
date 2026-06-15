//! PieChart widget — a circular statistical chart with colored sectors.
//!
//! The PieChart widget displays data as slices of a circle, with optional
//! labels, percentage annotations, exploded slices, and donut mode.
//! Each slice has a label, numeric value, color, and optional explosion offset.

use crate::core::{HorizontalAlignment, Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// A single slice in the pie chart.
#[derive(Clone, Debug)]
pub struct PieSlice {
    /// Label displayed next to the slice.
    pub label: String,
    /// Numeric value determining the slice arc size.
    pub value: f64,
    /// Fill color for this slice.
    pub color: Color,
    /// Whether this slice is pulled out (exploded) from the center.
    pub exploded: bool,
}

impl PieSlice {
    /// Creates a new pie slice with the given label, value, and color.
    pub fn new(label: impl Into<String>, value: f64, color: Color) -> Self {
        Self { label: label.into(), value, color, exploded: false }
    }

    /// Marks this slice as exploded (pulled out from center).
    pub fn with_exploded(mut self, exploded: bool) -> Self {
        self.exploded = exploded;
        self
    }
}

/// A circular pie chart widget with colored sectors.
///
/// Supports optional labels, percentage display, exploded slices,
/// and donut mode with configurable inner radius ratio.
pub struct PieChart {
    base: BaseWidget,
    slices: Vec<PieSlice>,
    show_labels: bool,
    show_percentages: bool,
    donut: bool,
    donut_ratio: f32,
}

impl PieChart {
    /// Creates a new PieChart widget with the given geometry.
    ///
    /// Defaults: labels shown, percentages shown, donut mode off.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::PieChart, geometry, "PieChart"),
            slices: Vec::new(),
            show_labels: true,
            show_percentages: true,
            donut: false,
            donut_ratio: 0.0,
        }
    }

    /// Sets the slices to display. Clears any previous slices.
    pub fn set_slices(&mut self, entries: Vec<PieSlice>) {
        self.slices = entries;
        self.base.request_redraw();
    }

    /// Adds a single slice to the chart.
    pub fn add_slice(&mut self, slice: PieSlice) {
        self.slices.push(slice);
        self.base.request_redraw();
    }

    /// Removes the slice at the given index.
    /// Returns `true` if the slice was removed, `false` if the index was out of bounds.
    pub fn remove_slice(&mut self, index: usize) -> bool {
        if index < self.slices.len() {
            self.slices.remove(index);
            self.base.request_redraw();
            true
        } else {
            false
        }
    }

    /// Removes all slices from the chart.
    pub fn clear_slices(&mut self) {
        self.slices.clear();
        self.base.request_redraw();
    }

    /// Returns the number of slices.
    pub fn slice_count(&self) -> usize {
        self.slices.len()
    }

    /// Returns a reference to the current slices.
    pub fn slices(&self) -> &[PieSlice] {
        &self.slices
    }

    /// Enables or disables showing labels next to each slice.
    pub fn set_show_labels(&mut self, show: bool) {
        self.show_labels = show;
        self.base.request_redraw();
    }

    /// Returns whether labels are shown next to slices.
    pub fn show_labels(&self) -> bool {
        self.show_labels
    }

    /// Enables or disables showing percentage values on each slice.
    pub fn set_show_percentages(&mut self, show: bool) {
        self.show_percentages = show;
        self.base.request_redraw();
    }

    /// Returns whether percentage values are shown.
    pub fn show_percentages(&self) -> bool {
        self.show_percentages
    }

    /// Enables or disables donut (ring) mode.
    pub fn set_donut_mode(&mut self, donut: bool) {
        self.donut = donut;
        self.base.request_redraw();
    }

    /// Returns whether donut mode is enabled.
    pub fn is_donut(&self) -> bool {
        self.donut
    }

    /// Sets the donut hole ratio (0.0 = full pie, 1.0 = invisible ring).
    /// Clamped to 0.0..=1.0.
    pub fn set_donut_ratio(&mut self, ratio: f32) {
        self.donut_ratio = ratio.clamp(0.0, 1.0);
        self.base.request_redraw();
    }

    /// Returns the donut hole ratio.
    pub fn donut_ratio(&self) -> f32 {
        self.donut_ratio
    }

    /// Returns the total sum of all slice values.
    fn total_value(&self) -> f64 {
        self.slices.iter().map(|s| s.value).sum()
    }

    /// Returns the center point of the chart.
    fn center(&self) -> Option<Point> {
        let rect = self.base.geometry();
        if rect.width == 0 || rect.height == 0 {
            return None;
        }
        Some(Point::new(rect.x + (rect.width as i32) / 2, rect.y + (rect.height as i32) / 2))
    }

    /// Returns the outer radius of the chart.
    fn outer_radius(&self) -> f32 {
        let rect = self.base.geometry();
        rect.width.min(rect.height) as f32 / 2.0 - 4.0
    }

    /// Returns a point on the circle at the given angle (in radians).
    /// Angle 0 is at 3 o'clock (right), angles increase clockwise.
    fn point_on_circle(center: Point, radius: f32, angle: f32) -> Point {
        Point::new(
            center.x + (radius * angle.cos()) as i32,
            center.y + (radius * angle.sin()) as i32,
        )
    }

    /// Fills a pie sector (center to two arc points) using scanline rasterization.
    fn fill_pie_sector(
        context: &mut RenderContext,
        center: Point,
        start_angle: f32,
        end_angle: f32,
        outer_radius: f32,
        inner_radius: f32,
        color: Color,
    ) {
        let segments = 60;
        let total_angle = end_angle - start_angle;
        if total_angle.abs() < 0.001 || outer_radius <= 0.0 {
            return;
        }
        let step = total_angle / segments as f32;

        let mut prev_outer = Self::point_on_circle(center, outer_radius, start_angle);
        let mut prev_inner = Self::point_on_circle(center, inner_radius, start_angle);

        for i in 1..=segments {
            let angle = start_angle + step * i as f32;
            let curr_outer = Self::point_on_circle(center, outer_radius, angle);
            let curr_inner = Self::point_on_circle(center, inner_radius, angle);

            // Fill the quad as two triangles: (prev_outer, curr_outer, prev_inner)
            // and (curr_outer, curr_inner, prev_inner)
            Self::fill_triangle_scanline(context, prev_outer, curr_outer, prev_inner, color);
            Self::fill_triangle_scanline(context, curr_outer, curr_inner, prev_inner, color);

            prev_outer = curr_outer;
            prev_inner = curr_inner;
        }
    }

    /// Fills a triangle using scanline rasterization (each row is a 1px fill_rect).
    fn fill_triangle_scanline(
        context: &mut RenderContext,
        v0: Point,
        v1: Point,
        v2: Point,
        color: Color,
    ) {
        let mut vs = [v0, v1, v2];
        vs.sort_by_key(|v| v.y);
        let [a, b, c] = vs;
        if a.y == c.y {
            let min_x = a.x.min(b.x).min(c.x);
            let max_x = a.x.max(b.x).max(c.x);
            if max_x > min_x {
                context.fill_rect(Rect::new(min_x, a.y, (max_x - min_x) as u32, 1), color);
            }
            return;
        }
        let total_height = c.y - a.y;
        if total_height <= 0 {
            return;
        }
        let lerp_x =
            |p1: Point, p2: Point, t: f32| -> f32 { p1.x as f32 + (p2.x - p1.x) as f32 * t };
        for y in a.y..=c.y {
            let (x1, x2) = if y < b.y {
                let sub_h = b.y - a.y;
                if sub_h == 0 {
                    continue;
                }
                (
                    lerp_x(a, c, (y - a.y) as f32 / total_height as f32),
                    lerp_x(a, b, (y - a.y) as f32 / sub_h as f32),
                )
            } else {
                let sub_h = c.y - b.y;
                if sub_h == 0 {
                    continue;
                }
                (
                    lerp_x(a, c, (y - a.y) as f32 / total_height as f32),
                    lerp_x(b, c, (y - b.y) as f32 / sub_h as f32),
                )
            };
            let x_start = x1.min(x2).round() as i32;
            let x_end = x1.max(x2).round() as i32;
            if x_end > x_start {
                context.fill_rect(Rect::new(x_start, y, (x_end - x_start) as u32, 1), color);
            }
        }
    }
}

impl Widget for PieChart {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}

impl Draw for PieChart {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.base.geometry();
        if rect.width == 0 || rect.height == 0 || self.slices.is_empty() {
            return;
        }

        let is_enabled = self.base.is_enabled();
        let total = self.total_value();
        if total.abs() < f64::EPSILON {
            return;
        }

        let Some(center) = self.center() else {
            return;
        };
        let outer_radius = self.outer_radius();
        let inner_radius = if self.donut { outer_radius * self.donut_ratio } else { 0.0 };
        if outer_radius <= 0.0 {
            return;
        }

        let label_font = Font::new("sans-serif", 10.0, false, false);
        let pct_font = Font::new("sans-serif", 9.0, false, false);

        // Draw sectors
        let mut start_angle = -std::f32::consts::FRAC_PI_2; // Start at 12 o'clock
        for slice in &self.slices {
            let sweep = (slice.value / total * 2.0 * std::f32::consts::PI as f64) as f32;
            let end_angle = start_angle + sweep;

            let slice_color =
                if is_enabled { slice.color } else { Color::rgba(200, 200, 200, 200) };

            // Exploded slice: shift the sector outward
            let explode_offset = if slice.exploded { 6.0 } else { 0.0 };
            let mid_angle = start_angle + sweep * 0.5;
            let offset_x = (explode_offset * mid_angle.cos()) as i32;
            let offset_y = (explode_offset * mid_angle.sin()) as i32;
            let exploded_center = Point::new(center.x + offset_x, center.y + offset_y);

            Self::fill_pie_sector(
                context,
                exploded_center,
                start_angle,
                end_angle,
                outer_radius,
                inner_radius,
                slice_color,
            );

            // Draw labels and percentages
            if is_enabled {
                // Position label at midpoint of the arc, slightly outside
                let label_radius = outer_radius + 12.0;
                let label_pos = Self::point_on_circle(exploded_center, label_radius, mid_angle);

                if self.show_labels {
                    let metrics = context.measure_text(&slice.label, &label_font);
                    let label_x = label_pos.x - metrics.width as i32 / 2;
                    let label_y = label_pos.y + metrics.height as i32 / 2;
                    context.draw_text(
                        Point::new(label_x, label_y),
                        &slice.label,
                        &label_font,
                        Color::DARK_GRAY,
                        HorizontalAlignment::Left,
                    );
                }

                // Draw percentage on the arc (inside, not overlapping label)
                if self.show_percentages && total > 0.0 {
                    let pct = (slice.value / total * 100.0).round() as i32;
                    let pct_text = format!("{}%", pct);
                    // Position percentage inside the sector, halfway between center and edge
                    let pct_radius = if self.donut {
                        (outer_radius + inner_radius) * 0.5
                    } else {
                        outer_radius * 0.6
                    };
                    let pct_pos = Self::point_on_circle(exploded_center, pct_radius, mid_angle);
                    let pct_metrics = context.measure_text(&pct_text, &pct_font);
                    let pct_x = pct_pos.x - pct_metrics.width as i32 / 2;
                    let pct_y = pct_pos.y + pct_metrics.height as i32 / 2;
                    context.draw_text(Point::new(pct_x, pct_y), &pct_text, &pct_font, Color::WHITE, HorizontalAlignment::Left);
                }
            }

            start_angle = end_angle;
        }

        // Draw outer circle stroke (border)
        let border_color = if is_enabled { Color::DARK_GRAY } else { Color::DISABLED_FOREGROUND };
        context.draw_circle_stroke(center, outer_radius as u32, border_color, 1);

        // Draw inner circle stroke for donut mode
        if self.donut && inner_radius > 0.0 {
            context.draw_circle_stroke(center, inner_radius as u32, border_color, 1);
        }
    }
}

impl EventHandler for PieChart {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::svg::render_to_svg;

    #[test]
    fn pie_chart_default_creation() {
        let pc = PieChart::new(Rect::new(0, 0, 200, 200));
        assert_eq!(pc.kind(), WidgetKind::PieChart);
        assert_eq!(pc.slice_count(), 0);
        assert!(pc.show_labels());
        assert!(pc.show_percentages());
        assert!(!pc.is_donut());
        assert!((pc.donut_ratio() - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn pie_chart_set_slices() {
        let mut pc = PieChart::new(Rect::new(0, 0, 200, 200));
        let slices = vec![
            PieSlice::new("A", 30.0, Color::RED),
            PieSlice::new("B", 50.0, Color::BLUE),
            PieSlice::new("C", 20.0, Color::GREEN),
        ];
        pc.set_slices(slices);
        assert_eq!(pc.slice_count(), 3);
        assert_eq!(pc.slices()[0].label, "A");
        assert!((pc.slices()[1].value - 50.0).abs() < f64::EPSILON);
        assert_eq!(pc.slices()[2].color, Color::GREEN);
    }

    #[test]
    fn pie_chart_add_and_remove() {
        let mut pc = PieChart::new(Rect::new(0, 0, 200, 200));
        pc.add_slice(PieSlice::new("X", 10.0, Color::RED));
        pc.add_slice(PieSlice::new("Y", 20.0, Color::BLUE));
        pc.add_slice(PieSlice::new("Z", 30.0, Color::GREEN));
        assert_eq!(pc.slice_count(), 3);

        assert!(pc.remove_slice(1)); // Remove "Y"
        assert_eq!(pc.slice_count(), 2);
        assert_eq!(pc.slices()[1].label, "Z");

        assert!(!pc.remove_slice(5)); // Out of bounds
        assert_eq!(pc.slice_count(), 2);
    }

    #[test]
    fn pie_chart_clear_slices() {
        let mut pc = PieChart::new(Rect::new(0, 0, 200, 200));
        pc.add_slice(PieSlice::new("A", 1.0, Color::RED));
        pc.add_slice(PieSlice::new("B", 2.0, Color::BLUE));
        pc.clear_slices();
        assert_eq!(pc.slice_count(), 0);
    }

    #[test]
    fn pie_chart_show_labels_and_percentages() {
        let mut pc = PieChart::new(Rect::new(0, 0, 200, 200));
        assert!(pc.show_labels());
        pc.set_show_labels(false);
        assert!(!pc.show_labels());
        assert!(pc.show_percentages());
        pc.set_show_percentages(false);
        assert!(!pc.show_percentages());
    }

    #[test]
    fn pie_chart_donut_mode() {
        let mut pc = PieChart::new(Rect::new(0, 0, 200, 200));
        assert!(!pc.is_donut());
        pc.set_donut_mode(true);
        assert!(pc.is_donut());
        pc.set_donut_ratio(0.5);
        assert!((pc.donut_ratio() - 0.5).abs() < f32::EPSILON);
        pc.set_donut_ratio(1.5); // Should clamp to 1.0
        assert!((pc.donut_ratio() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn pie_chart_exploded_slice() {
        let mut pc = PieChart::new(Rect::new(0, 0, 200, 200));
        pc.add_slice(PieSlice::new("A", 30.0, Color::RED));
        pc.add_slice(PieSlice::new("B", 70.0, Color::BLUE).with_exploded(true));
        assert_eq!(pc.slice_count(), 2);
        assert!(!pc.slices()[0].exploded);
        assert!(pc.slices()[1].exploded);
    }

    #[test]
    fn pie_chart_svg_output() {
        let mut pc = PieChart::new(Rect::new(0, 0, 200, 200));
        pc.add_slice(PieSlice::new("A", 30.0, Color::RED));
        pc.add_slice(PieSlice::new("B", 50.0, Color::BLUE));
        pc.add_slice(PieSlice::new("C", 20.0, Color::GREEN));
        let svg = render_to_svg(&mut pc);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }

    #[test]
    fn pie_chart_empty_slices_no_crash() {
        let mut pc = PieChart::new(Rect::new(0, 0, 200, 200));
        let svg = render_to_svg(&mut pc);
        assert!(svg.starts_with("<svg"));
    }

    #[test]
    fn pie_chart_event_forwarding() {
        let mut pc = PieChart::new(Rect::new(0, 0, 200, 200));
        pc.handle_event(&Event::MouseMove { pos: Point::new(10, 10) });
        pc.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1 });
    }
}
