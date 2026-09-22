// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! PieChart widget — a circular statistical chart with colored sectors.
//!
//! The PieChart widget displays data as slices of a circle, with optional
//! labels, percentage annotations, exploded slices, and donut mode.
//! Each slice has a label, numeric value, color, and optional explosion offset.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::widget::capability::coercion::expect_bool;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

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

    /// Fills a pie/donut sector.
    ///
    /// Geometry comes from the shared `sector_polygon` builder in the chart
    /// engine, so this widget and the SVG chart renderer approximate arcs with
    /// the same vertex ring instead of maintaining two rasterizers.
    ///
    /// This widget rasterizes by scanline because it targets `RenderContext`
    /// directly; the engine hands back the polygon, and the triangle fan below
    /// only converts that ring into fills — it no longer computes angles.
    ///
    /// Without the `chart` feature (tablet/mobile) the ring is built locally from
    /// the same angular stepping, so the visual result is identical.
    fn fill_pie_sector(
        context: &mut RenderContext,
        center: Point,
        start_angle: f32,
        end_angle: f32,
        outer_radius: f32,
        inner_radius: f32,
        color: Color,
    ) {
        let sweep = end_angle - start_angle;
        if sweep.abs() < 0.001 || outer_radius <= 0.0 {
            return;
        }

        // Angular resolution scales with the swept arc so a wide slice stays
        // smooth and a sliver stays cheap.
        let steps = (sweep.abs() * outer_radius / 6.0).ceil() as u32;
        #[cfg(feature = "chart")]
        let ring = crate::widget::chart_widgets::charts::sector_polygon(
            center,
            outer_radius,
            inner_radius,
            start_angle,
            end_angle,
            steps,
        );
        #[cfg(not(feature = "chart"))]
        let ring = Self::sector_polygon_local(
            center,
            outer_radius,
            inner_radius,
            start_angle,
            end_angle,
            steps,
        );

        if ring.len() < 3 {
            return;
        }

        if inner_radius > 0.0 {
            // Donut: the ring is outer-arc then inner-arc (reversed). Fill it as
            // a strip of quads spanning the two arcs.
            let half = ring.len() / 2;
            let (outer, inner) = ring.split_at(half);
            // `inner` is already reversed, so index i of each pairs up.
            for i in 0..outer.len().saturating_sub(1) {
                let (o0, o1) = (outer[i], outer[i + 1]);
                let (i0, i1) = (
                    inner.get(i).copied().unwrap_or(inner[0]),
                    inner.get(i + 1).copied().unwrap_or(*inner.last().unwrap_or(&inner[0])),
                );
                Self::fill_triangle_scanline(context, o0, o1, i0, color);
                Self::fill_triangle_scanline(context, o1, i1, i0, color);
            }
        } else {
            // Solid wedge: fan the arc against the apex (the final vertex).
            let apex = *ring.last().unwrap_or(&center);
            for pair in ring[..ring.len() - 1].windows(2) {
                Self::fill_triangle_scanline(context, pair[0], pair[1], apex, color);
            }
        }
    }

    /// Builds the sector vertex ring for builds without the `chart` feature.
    ///
    /// Mirrors `chart::charts::sector_polygon` exactly (outer arc, then inner arc
    /// reversed, or a single apex for a solid wedge) so tablet/mobile render the
    /// same shape as profiles that use the shared engine.
    #[cfg(not(feature = "chart"))]
    fn sector_polygon_local(
        center: Point,
        outer_radius: f32,
        inner_radius: f32,
        start_angle: f32,
        end_angle: f32,
        steps: u32,
    ) -> Vec<Point> {
        let sweep = end_angle - start_angle;
        if sweep.abs() < f32::EPSILON || outer_radius <= 0.0 {
            return Vec::new();
        }
        let steps = steps.clamp(3, 180);
        let mut vertices = Vec::with_capacity((steps as usize + 1) * 2);
        for step in 0..=steps {
            let angle = start_angle + sweep * (step as f32 / steps as f32);
            vertices.push(Self::point_on_circle(center, outer_radius, angle));
        }
        if inner_radius > 0.0 {
            for step in (0..=steps).rev() {
                let angle = start_angle + sweep * (step as f32 / steps as f32);
                vertices.push(Self::point_on_circle(center, inner_radius, angle));
            }
        } else {
            vertices.push(center);
        }
        vertices
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

/// Nudges a text box of `width`×`height` at `(x, y)` back inside `rect`.
///
/// The returned box is the caller's own measured size, moved so it lies within `rect`.
/// A box larger than the control cannot be made to fit by moving, so it is pinned to the
/// control's origin and left for `draw_text_fitted` to fit to the room that remains: the
/// fitting step is the one place that answers "too wide to fit", and duplicating that
/// judgement here would let the two disagree.
///
/// The labels are anchored outside the pie's ring, so on the downward slices the anchor
/// lands below the control; clamping is what keeps the glyph box inside the rectangle the
/// raster backends clip to.
/// Puts a label's box back inside `rect` when it does not fit, reporting whether it had to.
///
/// The flag matters to the *caller*: a label placed outside the pie sits on the chart's own
/// panel and takes the panel's ink, but one that had to be pulled inside the control lands on
/// a **slice**, where the panel ink is the wrong colour. Returning the flag lets the caller
/// pick the ink for the surface the label actually ended up on, instead of assuming the
/// placement it asked for.
fn label_box(x: i32, y: i32, width: i32, height: i32, rect: Rect) -> (Rect, bool) {
    // The gutter is the margin a clamped label keeps from the control's own edge, so a
    // clamped box sits *inside* the picture rather than on its border: `pie_chart.svg` drew
    // its top and bottom slice labels flush against the frame when the box was clamped to
    // `rect`'s raw extremes. One pixel is enough to read as inset and never loses a glyph.
    const EDGE_GUARD: i32 = 1;
    let min_x = rect.x + EDGE_GUARD;
    let min_y = rect.y + EDGE_GUARD;
    let max_x = (rect.x + rect.width as i32 - width - EDGE_GUARD).max(min_x);
    let max_y = (rect.y + rect.height as i32 - height - EDGE_GUARD).max(min_y);
    let clamped_x = x.clamp(min_x, max_x);
    let clamped_y = y.clamp(min_y, max_y);
    let moved = clamped_x != x || clamped_y != y;
    (Rect::new(clamped_x, clamped_y, width.max(1) as u32, height.max(1) as u32), moved)
}

impl Widget for PieChart {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        crate::core::Size::new(300, 300)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `PieChart`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` / `access_write_other.in.rs` dispatch.
impl WidgetProperties for PieChart {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "donut" => Ok(CapabilityValue::Bool(self.is_donut())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "donut" => {
                self.set_donut_mode(expect_bool(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["donut", BASE_PROPERTY_NAMES]
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
        // Chart chrome, derived from the active surface: `DARK_GRAY` was a light-chart literal
        // and rendered at 1.8:1 on the dark appearance's surface. The *slice* colours are
        // untouched — those identify the data (rule #108 ③). Shared with `bar_chart` and the
        // cartesian axes so the three cannot drift apart again.
        let slice_label_color = crate::widget::chart_widgets::charts::axis_chrome_color(0.75);

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
                // Position label at midpoint of the arc, slightly outside.
                //
                // The label box is the room between the arc and the control's edge, and
                // the text is fitted into it. The anchor sits a fixed distance outside the
                // ring, so on the downward slices it lands near the bottom edge: centring
                // an unfitted label there put half its glyph box below the control, which
                // the census caught as `[96,129..102,139]` on a 120 px-high box.
                //
                // The anchor is also *pulled inside* the control by the gutter the label
                // needs. A fixed `outer_radius + 12` put the anchor at y = 0 for the top
                // slice and y = 110 for the bottom one on a 120 px box — the label was then
                // only kept in the picture by `label_box` clamping it flush against the
                // border, so the first and last labels sat on the frame. Deriving the anchor
                // from the room that is actually left leaves a margin and lets `label_box`
                // stay a genuine fallback rather than the mechanism.
                let label_gutter = context.measure_text("0", &label_font).height as f32 + 4.0;
                let label_radius = (outer_radius + 12.0)
                    .min((rect.width.min(rect.height) as f32 / 2.0) - label_gutter);
                let label_pos = Self::point_on_circle(exploded_center, label_radius, mid_angle);

                if self.show_labels {
                    let metrics = context.measure_text(&slice.label, &label_font);
                    let label_x = label_pos.x - metrics.width as i32 / 2;
                    let label_y = label_pos.y - metrics.height as i32 / 2;
                    let (bounds, clamped) = label_box(
                        label_x,
                        label_y,
                        metrics.width as i32,
                        metrics.height as i32,
                        rect,
                    );
                    // The anchor is deliberately outside the pie, so the panel's chrome ink is
                    // the right colour — unless the control is too small for that and the box was
                    // pulled back inside, which lands the label on a slice. There the ink has to
                    // come from the slice, or the label is drawn panel-on-slice (measured 1.91:1
                    // on the dark appearance and 2.57:1 on the light one).
                    let ink =
                        if clamped { slice_color.contrast_color() } else { slice_label_color };
                    context.draw_text_fitted(
                        bounds,
                        &slice.label,
                        &label_font,
                        ink,
                        HorizontalAlignment::Left,
                    );
                }

                // Draw percentage on the arc (inside, not overlapping label)
                if self.show_percentages && total > 0.0 {
                    let pct = (slice.value / total * 100.0).round() as i32;
                    let pct_text = format!("{pct}%");
                    // Position percentage inside the sector, halfway between center and edge
                    let pct_radius = if self.donut {
                        (outer_radius + inner_radius) * 0.5
                    } else {
                        outer_radius * 0.6
                    };
                    let pct_pos = Self::point_on_circle(exploded_center, pct_radius, mid_angle);
                    let pct_metrics = context.measure_text(&pct_text, &pct_font);
                    let pct_x = pct_pos.x - pct_metrics.width as i32 / 2;
                    let pct_y = pct_pos.y - pct_metrics.height as i32 / 2;
                    let (bounds, _clamped) = label_box(
                        pct_x,
                        pct_y,
                        pct_metrics.width as i32,
                        pct_metrics.height as i32,
                        rect,
                    );
                    // The percentage is drawn *inside* the sector, so its ink is chosen from
                    // the slice's own fill rather than being a fixed white. A literal white is
                    // only legible on the darker half of a palette: on the light appearance's
                    // `rgb(244,180,0)` slice it measured 1.85:1, so the number that explains
                    // the slice was the least readable thing on it. The slice colour is still
                    // data and is untouched — only the ink over it is decided per slice, which
                    // is how a chart keeps both the encoding and the label.
                    context.draw_text_fitted(
                        bounds,
                        &pct_text,
                        &pct_font,
                        slice_color.contrast_color(),
                        HorizontalAlignment::Left,
                    );
                }
            }

            start_angle = end_angle;
        }

        // Draw outer circle stroke (border). Derived from the surface like the labels:
        // `DARK_GRAY` is a light chart's outline and vanished into the dark appearance's
        // surface, so the pie lost its outer edge exactly when the slices needed framing.
        let border_color = if is_enabled {
            crate::widget::chart_widgets::charts::axis_chrome_color(0.45)
        } else {
            Color::DISABLED_FOREGROUND
        };
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

    /// Sector geometry must come from the shared engine, and the fill must still
    /// produce real pixels — not just avoid panicking.
    ///
    /// The rendered output must contain filled rows (`<rect>` scanlines) for both
    /// slices, proving the polygon ring is being converted into coverage rather
    /// than silently dropped by the refactor.
    #[test]
    fn pie_chart_sector_fill_produces_pixels() {
        let mut pc = PieChart::new(Rect::new(0, 0, 200, 200));
        pc.add_slice(PieSlice::new("A", 50.0, Color::RED));
        pc.add_slice(PieSlice::new("B", 50.0, Color::BLUE));
        pc.set_show_labels(false);
        pc.set_show_percentages(false);

        let svg = render_to_svg(&mut pc);

        // Scanline fills are emitted as `<rect>` elements; a dropped sector would
        // leave none.
        let scanlines = svg.matches("<rect").count();
        assert!(
            scanlines > 50,
            "expected the two sectors to be rasterized into many scanlines, got {scanlines}"
        );
    }

    /// Donut mode must fill the ring between the two radii, which exercises the
    /// inner-arc branch of the shared polygon builder.
    #[test]
    fn pie_chart_donut_mode_produces_ring_pixels() {
        let mut pc = PieChart::new(Rect::new(0, 0, 200, 200));
        pc.add_slice(PieSlice::new("A", 100.0, Color::RED));
        pc.set_donut_mode(true);
        pc.set_donut_ratio(0.5);
        pc.set_show_labels(false);
        pc.set_show_percentages(false);

        let svg = render_to_svg(&mut pc);
        let scanlines = svg.matches("<rect").count();
        assert!(scanlines > 50, "donut ring should still rasterize, got {scanlines}");
    }
}
