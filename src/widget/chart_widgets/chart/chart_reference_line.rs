// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `ChartReferenceLine` — fixed value/threshold lines a cartesian chart mounts.
//!
//! A target line, an average line or an alert threshold is a *datum*, not a pointer
//! reading: it sits at one value for as long as the chart shows that value, and it needs
//! a label the reader can compare the series against. `ChartCrosshair` cannot carry it
//! (its lines follow the pointer), and the gauge's internal zone bands only exist for a
//! radial dial, so every cartesian chart in the family needs the same companion (COMP-A1).
//!
//! # Wiring
//!
//! A chart paints into a plot rectangle and maps data over a resolved range. Rather than
//! guessing either, [`ChartReferenceLine::attach_line_chart`] and
//! [`ChartReferenceLine::attach_bar_chart`] copy both from public accessors on the chart
//! after its first paint. The companion then owns the pixel mapping and the hit test, so
//! a host only has to call the attach method again after the chart's geometry, data or
//! zoom window changes.
//!
//! # Axes
//!
//! [`ReferenceAxis::Y`] draws horizontal lines at values on the value axis — the common
//! threshold case. [`ReferenceAxis::X`] draws vertical lines at positions on the index /
//! data axis: a line-chart x value, or a bar-chart category index.

use core::cell::{Ref, RefCell};

use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::{expect_bool, expect_u32};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::chart_widgets::{BarChart, LineChart};
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// The axis a [`ReferenceLineSpec::value`] is measured on.
///
/// The issue named this `AxisEdge`, but that type is the shared engine's enum
/// (`types::AxisEdge`, re-exported at the module root) and it states which *edge* of a
/// chart an axis sits on. This companion's axis is a much smaller question — which
/// direction a line runs — so it states that directly instead of borrowing an engine enum
/// whose semantic is edge placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReferenceAxis {
    /// Horizontal lines at values on the Y (value) axis.
    #[default]
    Y,
    /// Vertical lines at positions on the X (index / data) axis.
    X,
}

/// One reference line: a value, an optional label and its own colour/dash overrides.
#[derive(Debug, Clone, PartialEq)]
pub struct ReferenceLineSpec {
    /// The value to draw: a Y value for [`ReferenceAxis::Y`], or an X index/value for
    /// [`ReferenceAxis::X`]. A non-finite value is stored but never painted or hit-tested,
    /// because it has no honest pixel position.
    pub value: f64,
    /// The label painted beside the line; `None` uses the formatted value.
    pub label: Option<String>,
    /// The line colour; `None` inherits the companion's resolved theme colour.
    pub color: Option<Color>,
    /// Whether the line is painted with the companion's dash pattern.
    pub dashed: bool,
}

impl ReferenceLineSpec {
    /// Creates an undashed, unlabelled line at `value`.
    pub fn new(value: f64) -> Self {
        Self { value, label: None, color: None, dashed: false }
    }

    /// Sets the painted label (builder form).
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Sets the line colour (builder form).
    pub fn with_color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    /// Sets whether the line is dashed (builder form).
    pub fn with_dashed(mut self, dashed: bool) -> Self {
        self.dashed = dashed;
        self
    }
}

/// A set of fixed reference/threshold lines over a chart's plot area.
///
/// The control owns no chart: [`Self::attach_line_chart`] / [`Self::attach_bar_chart`]
/// copy the chart's painted plot rectangle and axis ranges, so the lines land on the
/// pixels the chart actually drew even though the chart's geometry stays private.
pub struct ChartReferenceLine {
    base: BaseWidget,
    lines: Vec<ReferenceLineSpec>,
    axis: ReferenceAxis,
    label_visible: bool,
    /// Alternating inked/blank run lengths in pixels. Empty means the default period.
    dash_style: Vec<u32>,
    hit_target_px: u32,
    /// The plot rectangle from the attached chart's last paint.
    plot_area: Option<Rect>,
    /// The value (Y) range from the attached chart's last paint.
    value_range: Option<(f64, f64)>,
    /// The x-axis range from the attached chart's last paint.
    x_range: Option<(f64, f64)>,
    /// The line index the pointer currently sits on, so a crossing emits once.
    hovered: Option<usize>,
    /// The labels painted for the current line set, keyed by `lines_revision`.
    ///
    /// `label_text` formats on every call, and the paint used to call it per line per
    /// frame; a still frame now only indexes this list.
    label_cache: RefCell<Option<(u64, Vec<String>)>>,
    /// Bumped whenever the line set or label visibility changes, which keys `label_cache`.
    lines_revision: u64,
    /// Reused run list for dashed lines, so the dashed path allocates nothing in steady state.
    dash_spans_scratch: RefCell<Vec<(i32, i32)>>,
    /// Emitted with `(line index, value)` when the pointer enters a line's hit target.
    pub on_line_crossed: Signal1<(usize, f64)>,
}

impl ChartReferenceLine {
    /// Creates an empty reference-line set covering `geometry`.
    ///
    /// The kind is [`WidgetKind::LineChart`] because a reference line is a cartesian
    /// chart overlay; companions reuse the nearest existing kind rather than adding one.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::LineChart, geometry, "ChartReferenceLine"),
            lines: Vec::new(),
            axis: ReferenceAxis::Y,
            label_visible: true,
            dash_style: Vec::new(),
            hit_target_px: 8,
            plot_area: None,
            value_range: None,
            x_range: None,
            hovered: None,
            label_cache: RefCell::new(None),
            lines_revision: 0,
            dash_spans_scratch: RefCell::new(Vec::new()),
            on_line_crossed: Signal1::new(),
        }
    }

    /// Replaces the line set.
    pub fn set_lines(&mut self, lines: Vec<ReferenceLineSpec>) {
        self.lines = lines;
        self.lines_revision = self.lines_revision.wrapping_add(1);
        self.hovered = None;
        self.base.request_redraw();
    }

    /// The current line set.
    pub fn lines(&self) -> &[ReferenceLineSpec] {
        &self.lines
    }

    /// Appends one line.
    pub fn add_line(&mut self, spec: ReferenceLineSpec) {
        self.lines.push(spec);
        self.lines_revision = self.lines_revision.wrapping_add(1);
        self.base.request_redraw();
    }

    /// Removes the line at `index`, reporting whether the index existed.
    ///
    /// A removal shifts every later line's index, which is why the hovered line is
    /// forgotten: an index carried over from before the removal would name a different
    /// line than the one the pointer was on.
    pub fn remove_line(&mut self, index: usize) -> bool {
        if index >= self.lines.len() {
            return false;
        }
        self.lines.remove(index);
        self.lines_revision = self.lines_revision.wrapping_add(1);
        self.hovered = None;
        self.base.request_redraw();
        true
    }

    /// Removes every line.
    pub fn clear_lines(&mut self) {
        if self.lines.is_empty() {
            return;
        }
        self.lines.clear();
        self.lines_revision = self.lines_revision.wrapping_add(1);
        self.hovered = None;
        self.base.request_redraw();
    }

    /// The axis the lines are measured on.
    pub fn axis(&self) -> ReferenceAxis {
        self.axis
    }

    /// Sets the axis the lines are measured on.
    pub fn set_axis(&mut self, axis: ReferenceAxis) {
        if self.axis == axis {
            return;
        }
        self.axis = axis;
        self.hovered = None;
        self.base.request_redraw();
    }

    /// Whether labels are painted.
    pub fn label_visible(&self) -> bool {
        self.label_visible
    }

    /// Enables or disables labels.
    pub fn set_label_visible(&mut self, visible: bool) {
        if self.label_visible == visible {
            return;
        }
        self.label_visible = visible;
        self.lines_revision = self.lines_revision.wrapping_add(1);
        self.base.request_redraw();
    }

    /// The dash pattern: alternating inked and blank run lengths in pixels.
    pub fn dash_style(&self) -> &[u32] {
        &self.dash_style
    }

    /// Sets the dash pattern used by [`ReferenceLineSpec::dashed`] lines.
    ///
    /// The vector is read as alternating inked and blank run lengths in pixels and cycles
    /// when the line is longer than the pattern. An empty (or single-entry) vector means
    /// the default period, `3 × width` inked and `2 × width` blank — the same period the
    /// toolkit's dashed borders use. The render layer's own dash kernel carries exactly
    /// one mark/gap pair, so this companion walks a longer pattern itself.
    pub fn set_dash_style(&mut self, pattern: Vec<u32>) {
        if self.dash_style == pattern {
            return;
        }
        self.dash_style = pattern;
        self.base.request_redraw();
    }

    /// The pointer distance, in pixels, that counts as touching a line.
    pub fn hit_target_px(&self) -> u32 {
        self.hit_target_px
    }

    /// Sets the pointer distance that counts as touching a line.
    pub fn set_hit_target_px(&mut self, px: u32) {
        if self.hit_target_px == px {
            return;
        }
        self.hit_target_px = px;
        self.hovered = None;
        self.base.request_redraw();
    }

    /// Copies the plot rectangle and axis ranges from a line chart's last paint.
    ///
    /// Call this after the chart has painted (a layout change, new data or a zoom needs
    /// another call). A chart that has not painted yet has no rectangle to copy, so the
    /// companion keeps its own geometry and reports no value range.
    pub fn attach_line_chart(&mut self, chart: &LineChart) {
        self.plot_area = chart.last_plot_area();
        self.value_range = chart.value_range();
        self.x_range = chart.x_range();
        self.hovered = None;
        self.base.request_redraw();
    }

    /// Copies the plot rectangle and axis ranges from a bar chart's last paint.
    ///
    /// As with [`Self::attach_line_chart`], re-attach after the chart changes. For a bar
    /// chart the X axis is the category axis; an [`ReferenceAxis::X`] value is a category
    /// index, and [`BarChart::category_index_range`] supplies the half-slot margins that
    /// put an index on the centre of its bar slot.
    pub fn attach_bar_chart(&mut self, chart: &BarChart) {
        self.plot_area = chart.last_plot_area();
        self.value_range = chart.value_range();
        self.x_range = chart.category_index_range();
        self.hovered = None;
        self.base.request_redraw();
    }

    /// The rectangle the lines are painted in.
    ///
    /// Falls back to the control's own geometry while no chart has been attached, so a
    /// standalone reference-line overlay still draws where it was placed.
    fn plot(&self) -> Rect {
        match self.plot_area {
            Some(rect) if rect.width > 0 && rect.height > 0 => rect,
            _ => self.base.geometry(),
        }
    }

    /// Maps a value on the Y axis to its pixel row, or `None` when it cannot be placed.
    fn value_pixel(&self, value: f64) -> Option<i32> {
        let (min, max) = self.value_range?;
        if !value.is_finite() || !min.is_finite() || !max.is_finite() || max == min {
            return None;
        }
        let plot = self.plot();
        let fraction = (value - min) / (max - min);
        Some(plot.y + (plot.height as f64 * (1.0 - fraction)).round() as i32)
    }

    /// Maps a position on the X axis to its pixel column, or `None` when it cannot be placed.
    fn index_pixel(&self, value: f64) -> Option<i32> {
        let (min, max) = self.x_range?;
        if !value.is_finite() || !min.is_finite() || !max.is_finite() || max == min {
            return None;
        }
        let plot = self.plot();
        let fraction = (value - min) / (max - min);
        Some(plot.x + (plot.width as f64 * fraction).round() as i32)
    }

    /// The pixel of a line on the current axis.
    fn line_pixel(&self, spec: &ReferenceLineSpec) -> Option<i32> {
        match self.axis {
            ReferenceAxis::Y => self.value_pixel(spec.value),
            ReferenceAxis::X => self.index_pixel(spec.value),
        }
    }

    /// The resolved line colour: the caller's, or the theme's accent role.
    fn resolved_color(&self) -> Color {
        crate::style::palette_shared()
            .as_ref()
            .map(|colors| colors.accent)
            .unwrap_or_else(|| Color::rgba(255, 140, 0, 200))
    }

    /// The label painted beside a line.
    fn label_text(&self, spec: &ReferenceLineSpec) -> String {
        spec.label.clone().unwrap_or_else(|| std::format!("{:.2}", spec.value))
    }

    /// The labels for the current line set, rebuilt only when `lines_revision` moves.
    ///
    /// Every entry is aligned with `lines` by index, so the paint reads a label by the
    /// same index it reads the spec by. An empty slice is returned for the impossible
    /// unfilled case, which keeps the borrow infallible.
    fn cached_labels(&self) -> Ref<'_, [String]> {
        let fresh = matches!(
            self.label_cache.borrow().as_ref(),
            Some((revision, _)) if *revision == self.lines_revision
        );
        if !fresh {
            let labels = self.lines.iter().map(|spec| self.label_text(spec)).collect();
            *self.label_cache.borrow_mut() = Some((self.lines_revision, labels));
        }
        Ref::map(self.label_cache.borrow(), |slot| {
            slot.as_ref().map_or(&[][..], |(_, labels)| labels.as_slice())
        })
    }

    /// The dash pattern in force: the caller's, or `fallback` when none was supplied.
    ///
    /// The fallback is a stack array owned by the paint site: the default period is two
    /// numbers derived from the stroke width, which the render layer's own `DashPattern`
    /// also computes per call, so no heap allocation is needed.
    fn resolved_dash<'a>(&'a self, fallback: &'a [u32; 2]) -> &'a [u32] {
        if self.dash_style.len() >= 2 {
            &self.dash_style
        } else {
            fallback
        }
    }

    /// Paints one line, dashed or solid.
    fn paint_line(
        &self,
        context: &mut RenderContext,
        from: Point,
        to: Point,
        color: Color,
        width: u32,
        dashed: bool,
    ) {
        if !dashed {
            context.draw_line_stroke_aa(from, to, color, width);
            return;
        }
        let horizontal = from.y == to.y;
        let (start, end) = if horizontal { (from.x, to.x) } else { (from.y, to.y) };
        let fallback = [(width * 3).max(1), (width * 2).max(1)];
        // The resident scratch is cleared and refilled, so a dashed line allocates nothing
        // once the list has grown to its longest span count.
        let mut spans = self.dash_spans_scratch.borrow_mut();
        dash_spans(start, end, self.resolved_dash(&fallback), &mut spans);
        for (run_start, run_end) in spans.iter().copied() {
            let (a, b) = if horizontal {
                (Point::new(run_start, from.y), Point::new(run_end, from.y))
            } else {
                (Point::new(from.x, run_start), Point::new(from.x, run_end))
            };
            if a != b {
                context.draw_line_stroke_aa(a, b, color, width);
            }
        }
    }

    /// Recomputes the hovered line and emits a crossing when it changes.
    fn update_hover(&mut self, pos: Point) {
        if self.lines.is_empty() {
            self.hovered = None;
            return;
        }
        let plot = self.plot();
        if plot.width == 0 || plot.height == 0 {
            self.hovered = None;
            return;
        }
        let target = self.hit_target_px.min(i32::MAX as u32) as i32;
        let hit_area = Rect::new(
            plot.x.saturating_sub(target),
            plot.y.saturating_sub(target),
            plot.width.saturating_add(target as u32 * 2),
            plot.height.saturating_add(target as u32 * 2),
        );
        if !hit_area.contains_point(pos) {
            self.hovered = None;
            return;
        }
        let coordinate = match self.axis {
            ReferenceAxis::Y => pos.y,
            ReferenceAxis::X => pos.x,
        };
        let mut nearest: Option<(usize, i64, f64)> = None;
        for (index, spec) in self.lines.iter().enumerate() {
            let Some(pixel) = self.line_pixel(spec) else { continue };
            let distance = (pixel as i64 - coordinate as i64).abs();
            if distance > i64::from(target) {
                continue;
            }
            if nearest.is_none_or(|(_, near, _)| distance < near) {
                nearest = Some((index, distance, spec.value));
            }
        }
        let next = nearest.map(|(index, _, _)| index);
        if next != self.hovered {
            self.hovered = next;
            if let Some((index, _, value)) = nearest {
                self.on_line_crossed.emit((index, value));
            }
        }
    }
}

/// Splits the interval `start..=end` into the inked runs of an alternating pattern.
///
/// `pattern` holds inked and blank lengths in pixels, starting with an inked run; it
/// cycles until the interval is consumed. The returned pairs are in the same direction
/// as `start → end`. A pattern with no positive run returns no spans, which paints
/// nothing rather than silently reverting to a solid line. The spans are written into
/// the caller's `spans` (cleared first) so a dashed paint can reuse one buffer.
fn dash_spans(start: i32, end: i32, pattern: &[u32], spans: &mut Vec<(i32, i32)>) {
    spans.clear();
    if pattern.len() < 2 || pattern.iter().all(|length| *length == 0) {
        return;
    }
    let total = (end - start).abs();
    if total == 0 {
        return;
    }
    let step = if end >= start { 1 } else { -1 };
    let mut consumed = 0i32;
    let mut index = 0usize;
    while consumed < total {
        let run = pattern[index % pattern.len()].min(i32::MAX as u32) as i32;
        let inked = index.is_multiple_of(2);
        index += 1;
        if run == 0 {
            continue;
        }
        let length = run.min(total - consumed);
        if inked {
            let run_start = start + step * consumed;
            let run_end = start + step * (consumed + length);
            spans.push((run_start, run_end));
        }
        consumed += length;
    }
}

impl Widget for ChartReferenceLine {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::widget::metrics::ControlMetrics::implicit_size(
            crate::core::Size::new(0, 0),
            crate::style::EdgeOffsets::all(0),
            crate::core::Size::new(160, 120),
        )
    }

    crate::impl_draw_bridge!();
    impl_widget_property_hooks!();
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "line_crossed" | "on_line_crossed" => {
                Some(EventSignalRef::mapped("line_crossed", &self.on_line_crossed, |value| {
                    CapabilityValue::Tuple(crate::compat::Vec::from([
                        CapabilityValue::UInt(value.0 as u64),
                        CapabilityValue::Float(value.1),
                    ]))
                }))
            }
            _ => None,
        }
    }
}

impl Draw for ChartReferenceLine {
    fn draw(&mut self, context: &mut RenderContext) {
        if self.lines.is_empty() {
            return;
        }
        let plot = self.plot();
        if plot.width == 0 || plot.height == 0 {
            return;
        }
        let width = 1u32;
        let right = plot.x + plot.width as i32;
        let bottom = plot.y + plot.height as i32;
        // Built once per line-set revision, then indexed by line; a still frame formats nothing.
        let labels = if self.label_visible { Some(self.cached_labels()) } else { None };
        for (index, spec) in self.lines.iter().enumerate() {
            let color = spec.color.unwrap_or_else(|| self.resolved_color());
            match self.axis {
                ReferenceAxis::Y => {
                    let Some(y) = self.value_pixel(spec.value) else { continue };
                    if y < plot.y || y > bottom {
                        continue;
                    }
                    self.paint_line(
                        context,
                        Point::new(plot.x, y),
                        Point::new(right, y),
                        color,
                        width,
                        spec.dashed,
                    );
                    if let Some(label) = labels.as_ref().and_then(|labels| labels.get(index)) {
                        // Above the line when the row is free, below it at the top edge.
                        let label_y = (y - 14).max(plot.y);
                        context.draw_text(
                            Point::new(plot.x + 2, label_y),
                            label,
                            effective_font(self.base.style()),
                            color,
                            HorizontalAlignment::Left,
                        );
                    }
                }
                ReferenceAxis::X => {
                    let Some(x) = self.index_pixel(spec.value) else { continue };
                    if x < plot.x || x > right {
                        continue;
                    }
                    self.paint_line(
                        context,
                        Point::new(x, plot.y),
                        Point::new(x, bottom),
                        color,
                        width,
                        spec.dashed,
                    );
                    if let Some(label) = labels.as_ref().and_then(|labels| labels.get(index)) {
                        context.draw_text(
                            Point::new(x + 2, plot.y + 2),
                            label,
                            effective_font(self.base.style()),
                            color,
                            HorizontalAlignment::Left,
                        );
                    }
                }
            }
        }
    }
}

impl EventHandler for ChartReferenceLine {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        match event {
            Event::MouseMove { pos } | Event::PointerMove { pos, .. } => self.update_hover(*pos),
            Event::MouseLeave { .. } => self.hovered = None,
            _ => {}
        }
    }
}

impl WidgetProperties for ChartReferenceLine {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "line_count" => Ok(CapabilityValue::UInt(self.lines().len() as u64)),
            "label_visible" => Ok(CapabilityValue::Bool(self.label_visible())),
            "hit_target" => Ok(CapabilityValue::UInt(u64::from(self.hit_target_px()))),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "line_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            "label_visible" => {
                self.set_label_visible(expect_bool(value)?);
                Ok(())
            }
            "hit_target" => {
                self.set_hit_target_px(expect_u32(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["line_count", "label_visible", "hit_target", BASE_PROPERTY_NAMES]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference_line(axis: ReferenceAxis) -> ChartReferenceLine {
        let mut line = ChartReferenceLine::new(Rect::new(0, 0, 200, 100));
        line.set_axis(axis);
        line.plot_area = Some(Rect::new(0, 0, 200, 100));
        line.value_range = Some((0.0, 10.0));
        line.x_range = Some((0.0, 10.0));
        line
    }

    #[test]
    fn default_state_is_empty_and_horizontal() {
        let mut line = ChartReferenceLine::new(Rect::new(0, 0, 200, 100));
        assert_eq!(line.axis(), ReferenceAxis::Y);
        assert!(line.label_visible());
        assert_eq!(line.hit_target_px(), 8);
        assert!(line.dash_style().is_empty());
        assert!(line.lines().is_empty());
        assert!(!line.remove_line(0));
        line.clear_lines();
        assert!(line.lines().is_empty());
    }

    #[test]
    fn value_maps_to_the_attached_plot_area() {
        let line = reference_line(ReferenceAxis::Y);
        assert_eq!(line.value_pixel(0.0), Some(100));
        assert_eq!(line.value_pixel(5.0), Some(50));
        assert_eq!(line.value_pixel(10.0), Some(0));
        assert_eq!(line.value_pixel(f64::NAN), None);

        let vertical = reference_line(ReferenceAxis::X);
        assert_eq!(vertical.index_pixel(0.0), Some(0));
        assert_eq!(vertical.index_pixel(5.0), Some(100));
        assert_eq!(vertical.index_pixel(10.0), Some(200));
        assert_eq!(vertical.index_pixel(f64::INFINITY), None);
    }

    #[test]
    fn remove_and_clear_manage_the_line_set() {
        let mut line = reference_line(ReferenceAxis::Y);
        line.add_line(ReferenceLineSpec::new(1.0));
        line.add_line(ReferenceLineSpec::new(2.0).with_label("target"));
        line.add_line(ReferenceLineSpec::new(3.0).with_dashed(true));
        assert_eq!(line.lines().len(), 3);

        assert!(line.remove_line(1));
        assert_eq!(line.lines().len(), 2);
        assert_eq!(line.lines()[1].value, 3.0);
        assert!(!line.remove_line(5));

        line.clear_lines();
        assert!(line.lines().is_empty());

        line.set_lines(vec![ReferenceLineSpec::new(4.0)]);
        assert_eq!(line.lines()[0].value, 4.0);
    }

    #[test]
    fn crossing_a_line_emits_once_per_entry() {
        let mut line = reference_line(ReferenceAxis::Y);
        line.set_hit_target_px(4);
        line.set_lines(vec![ReferenceLineSpec::new(5.0)]);
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = std::sync::Arc::clone(&seen);
        line.on_line_crossed.connect(move |value| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*value);
            }
        });

        line.handle_event(&Event::MouseMove { pos: Point::new(10, 52) });
        // Still on the same line: a second move must not repeat the crossing.
        line.handle_event(&Event::MouseMove { pos: Point::new(10, 53) });
        // Away from every line, then back onto it.
        line.handle_event(&Event::MouseMove { pos: Point::new(10, 80) });
        line.handle_event(&Event::MouseMove { pos: Point::new(10, 50) });

        assert_eq!(*seen.lock().expect("lock"), vec![(0, 5.0), (0, 5.0)]);
    }

    #[test]
    fn drawing_paints_dashed_lines_and_their_labels() {
        let mut line = reference_line(ReferenceAxis::Y);
        line.set_lines(vec![ReferenceLineSpec::new(5.0).with_label("target").with_dashed(true)]);
        let svg = crate::widget::svg::render_to_svg(&mut line);
        assert!(svg.contains("line"), "the reference line was drawn");
        assert!(!crate::widget::svg::text_ink_boxes(&svg).is_empty(), "the label was drawn");

        line.set_label_visible(false);
        let bare = crate::widget::svg::render_to_svg(&mut line);
        assert!(
            crate::widget::svg::text_ink_boxes(&bare).is_empty(),
            "a hidden label paints no text"
        );
    }

    #[test]
    fn attaching_a_chart_copies_its_painted_geometry() {
        let mut chart = LineChart::new(Rect::new(0, 0, 300, 200));
        chart.set_data(vec![(0.0, 0.0), (10.0, 10.0)]);
        let _ = crate::widget::svg::render_to_svg(&mut chart);

        let mut line = ChartReferenceLine::new(Rect::new(0, 0, 20, 20));
        line.attach_line_chart(&chart);
        assert_eq!(line.plot_area, chart.last_plot_area());
        assert_eq!(line.value_range, chart.value_range());
        assert_eq!(line.x_range, chart.x_range());
        assert!(line.plot_area.is_some(), "the chart's paint recorded a plot area");
    }
}
