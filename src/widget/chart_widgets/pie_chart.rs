// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! PieChart widget — a circular statistical chart with colored sectors.
//!
//! The PieChart widget displays data as slices of a circle, with optional
//! labels, percentage annotations, exploded slices, and donut mode.
//! Each slice has a label, numeric value, color, and optional explosion offset.

use crate::core::{Color, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::style::{MotionSlot, PropertyDriver};
use crate::widget::capability::coercion::expect_bool;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::role_font;
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
    /// Whether the slice is drawn and contributes to the total.
    ///
    /// A legend click flips this flag; the default is `true`, so a slice list that
    /// never touches it renders exactly as before.
    pub visible: bool,
}

impl PieSlice {
    /// Creates a new pie slice with the given label, value, and color.
    pub fn new(label: impl Into<String>, value: f64, color: Color) -> Self {
        Self { label: label.into(), value, color, exploded: false, visible: true }
    }

    /// Creates a slice whose initial visibility is stated explicitly.
    pub fn with_visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }

    /// Marks this slice as exploded (pulled out from center).
    pub fn with_exploded(mut self, exploded: bool) -> Self {
        self.exploded = exploded;
        self
    }
}

/// The legend column's geometry and clickable row count.
#[derive(Clone, Copy)]
struct LegendArea {
    x: i32,
    y: i32,
    width: i32,
    row_height: i32,
    rows: usize,
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
    /// The slice under the pointer.
    hovered_slice: Option<usize>,
    /// Whether hovering a slice pulls it out of the ring.
    explode_on_hover: bool,
    /// Whether the hovered slice's label/value/percentage is shown (CH-01).
    show_tooltip: bool,
    /// Whether a colour-to-name legend column is reserved and drawn (CH-02).
    show_legend: bool,
    /// The chart title, painted centred at the top (CH-07).
    title: Option<String>,
    /// The x-axis title (CH-07).
    x_axis_label: Option<String>,
    /// The y-axis title (CH-07).
    y_axis_label: Option<String>,
    /// Per-slice explosion progress, in `0.0..=1.0`, advanced by [`Widget::tick`].
    explode_progress: Vec<f32>,
    /// Per-slice `(start_angle, sweep)` for the current data, or `None` for a slice that
    /// is skipped because its value is not a positive finite number.
    angle_cache: Vec<Option<(f32, f32)>>,
    /// The sum of the positive finite slice values the cached angles were built from.
    cached_total: f64,
    /// Whether `angle_cache` and `cached_total` must be rebuilt from `slices`.
    angles_dirty: bool,
    /// Whether value changes ease into place. On by default.
    animate: bool,
    /// Progress of the running value transition, in `0.0..=1.0`.
    transition: PropertyDriver,
    /// The start values of the running transition, one per slice.
    from_values: Vec<f64>,
    /// First legend row drawn; the legend scrolls instead of truncating (CH-009).
    legend_scroll: usize,
    /// The legend column's clickable rows from the last paint.
    last_legend_area: Option<LegendArea>,
    /// The last pointer position, used to decide whether a wheel belongs to the legend.
    last_pointer: Option<Point>,
    /// Emitted with `(slice, visible)` after a legend click or [`Self::set_series_visible`].
    pub series_toggled: Signal1<(usize, bool)>,
    /// Emitted with the slice index when hover enters a slice.
    pub slice_hovered: Signal1<usize>,
    /// Emitted with the slice index when hover leaves it.
    pub slice_unhovered: Signal1<usize>,
    /// Emitted with the slice index of a clicked slice.
    pub slice_clicked: Signal1<usize>,
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
            hovered_slice: None,
            explode_on_hover: false,
            show_tooltip: false,
            show_legend: false,
            title: None,
            x_axis_label: None,
            y_axis_label: None,
            explode_progress: Vec::new(),
            angle_cache: Vec::new(),
            cached_total: 0.0,
            angles_dirty: true,
            animate: true,
            transition: PropertyDriver::at(1.0, MotionSlot::Normal),
            from_values: Vec::new(),
            legend_scroll: 0,
            last_legend_area: None,
            last_pointer: None,
            series_toggled: Signal1::new(),
            slice_hovered: Signal1::new(),
            slice_unhovered: Signal1::new(),
            slice_clicked: Signal1::new(),
        }
    }

    /// Sets the slices to display. Clears any previous slices.
    pub fn set_slices(&mut self, entries: Vec<PieSlice>) {
        let from = if self.animate && self.slices.len() == entries.len() {
            Some((0..self.slices.len()).map(|index| self.displayed_value(index)).collect())
        } else {
            None
        };
        self.slices = entries;
        self.angles_dirty = true;
        self.sync_explode_progress();
        self.install_transition(from);
        self.revalidate_hover();
        self.base.request_redraw();
    }

    /// Adds a single slice to the chart.
    pub fn add_slice(&mut self, slice: PieSlice) {
        self.slices.push(slice);
        self.angles_dirty = true;
        self.sync_explode_progress();
        self.clear_transition();
        self.base.request_redraw();
    }

    /// Removes the slice at the given index.
    /// Returns `true` if the slice was removed, `false` if the index was out of bounds.
    pub fn remove_slice(&mut self, index: usize) -> bool {
        if index < self.slices.len() {
            self.slices.remove(index);
            self.angles_dirty = true;
            self.sync_explode_progress();
            self.clear_transition();
            self.revalidate_hover();
            self.base.request_redraw();
            true
        } else {
            false
        }
    }

    /// Removes all slices from the chart.
    pub fn clear_slices(&mut self) {
        self.slices.clear();
        self.angle_cache.clear();
        self.angles_dirty = true;
        self.explode_progress.clear();
        self.clear_transition();
        self.legend_scroll = 0;
        self.revalidate_hover();
        self.base.request_redraw();
    }

    /// The value currently drawn for slice `index`, including any running transition.
    fn displayed_value(&self, index: usize) -> f64 {
        let Some(slice) = self.slices.get(index) else {
            return 0.0;
        };
        match self.animation_progress() {
            Some(progress) => {
                let start = self.from_values.get(index).copied().unwrap_or(slice.value);
                start + (slice.value - start) * f64::from(progress)
            }
            None => slice.value,
        }
    }

    /// Starts a transition from `from`, or lands immediately when there is nothing to ease.
    fn install_transition(&mut self, from: Option<Vec<f64>>) {
        let from = match from {
            Some(from) if !from.is_empty() => from,
            _ => {
                self.clear_transition();
                return;
            }
        };
        let unchanged = from.len() == self.slices.len()
            && from.iter().zip(&self.slices).all(|(old, slice)| *old == slice.value);
        if unchanged {
            self.clear_transition();
        } else {
            self.from_values = from;
            self.transition.jump_to(0.0);
            self.transition.set_target(1.0);
        }
    }

    /// Lands the running transition on its targets and drops the start values.
    fn clear_transition(&mut self) {
        self.transition.jump_to(1.0);
        self.from_values.clear();
    }

    /// The transition progress, or `None` when no transition is running.
    fn animation_progress(&self) -> Option<f32> {
        (!self.from_values.is_empty()).then(|| self.transition.value().clamp(0.0, 1.0))
    }

    /// Enables or disables easing of slice value changes. On by default.
    pub fn set_animate(&mut self, animate: bool) {
        if self.animate == animate {
            return;
        }
        self.animate = animate;
        if !animate {
            self.clear_transition();
            self.angles_dirty = true;
        }
        self.base.request_redraw();
    }

    /// Whether slice value changes ease into place.
    pub fn is_animated(&self) -> bool {
        self.animate
    }

    /// Whether slice `index` is drawn and contributes to the total.
    pub fn is_series_visible(&self, index: usize) -> bool {
        self.slices.get(index).is_some_and(|slice| slice.visible)
    }

    /// Shows or hides slice `index`, reporting the change through [`Self::series_toggled`].
    ///
    /// Returns `false` when `index` is out of bounds.
    pub fn set_series_visible(&mut self, index: usize, visible: bool) -> bool {
        let Some(slice) = self.slices.get_mut(index) else {
            return false;
        };
        if slice.visible == visible {
            return true;
        }
        slice.visible = visible;
        self.angles_dirty = true;
        self.revalidate_hover();
        self.series_toggled.emit((index, visible));
        self.base.request_redraw();
        true
    }

    /// The first legend row currently drawn.
    pub fn legend_scroll_offset(&self) -> usize {
        self.legend_scroll
    }

    /// The slice index of the legend row under `pos`, when the legend is shown.
    fn legend_entry_at(&self, pos: Point) -> Option<usize> {
        let area = self.last_legend_area?;
        if pos.x < area.x
            || pos.x >= area.x + area.width
            || pos.y < area.y
            || pos.y >= area.y + area.row_height * area.rows as i32
        {
            return None;
        }
        let row = ((pos.y - area.y) / area.row_height.max(1)) as usize;
        if row >= area.rows {
            return None;
        }
        let index = self.legend_scroll.saturating_add(row);
        (index < self.slices.len()).then_some(index)
    }

    /// Scrolls the legend by `rows` when it overflows.
    fn scroll_legend(&mut self, rows: i32) -> bool {
        let Some(area) = self.last_legend_area else {
            return false;
        };
        let max_scroll = self.slices.len().saturating_sub(area.rows);
        if max_scroll == 0 {
            return false;
        }
        let next = (self.legend_scroll as i32).saturating_add(rows).clamp(0, max_scroll as i32);
        let next = next as usize;
        if next == self.legend_scroll {
            return false;
        }
        self.legend_scroll = next;
        self.base.request_redraw();
        true
    }

    /// Enables pulling the hovered slice out of the ring.
    pub fn set_explode_on_hover(&mut self, explode: bool) {
        if self.explode_on_hover == explode {
            return;
        }
        self.explode_on_hover = explode;
        self.base.request_redraw();
    }

    /// Returns whether hovering a slice pulls it out.
    pub fn explode_on_hover(&self) -> bool {
        self.explode_on_hover
    }

    /// The slice under the pointer, if any.
    pub fn hovered_slice(&self) -> Option<usize> {
        self.hovered_slice
    }

    /// Keeps the per-slice animation table the same length as the slice list.
    fn sync_explode_progress(&mut self) {
        self.explode_progress.resize(self.slices.len(), 0.0);
    }

    /// Drops a hover index the slice list no longer has or no longer draws.
    fn revalidate_hover(&mut self) {
        if self
            .hovered_slice
            .is_some_and(|index| !self.slices.get(index).is_some_and(|slice| slice.visible))
        {
            if let Some(index) = self.hovered_slice.take() {
                self.slice_unhovered.emit(index);
            }
        }
    }

    /// The explosion distance for slice `index`, in logical pixels.
    fn explode_offset_for(&self, index: usize) -> f32 {
        let armed = self.slices.get(index).is_some_and(|slice| slice.exploded)
            || (self.explode_on_hover && self.hovered_slice == Some(index));
        let target = if armed { 1.0 } else { 0.0 };
        let progress = self.explode_progress.get(index).copied().unwrap_or(target);
        progress * 6.0
    }

    /// Maps a pointer position to the slice under it.
    fn slice_at(&self, pos: Point) -> Option<usize> {
        let center = self.center()?;
        let dx = f64::from(pos.x - center.x);
        let dy = f64::from(pos.y - center.y);
        let distance = (dx * dx + dy * dy).sqrt() as f32;
        let outer = self.outer_radius();
        let inner = if self.donut { outer * self.donut_ratio } else { 0.0 };
        if distance > outer || distance < inner {
            return None;
        }
        let total = self.total_value();
        if total <= 0.0 {
            return None;
        }
        // atan2 measures from 3 o'clock; the first slice starts at 12 o'clock and the
        // sweep runs clockwise, which is the same direction `atan2` grows in screen space.
        let mut angle = dy.atan2(dx) as f32 + std::f32::consts::FRAC_PI_2;
        if angle < 0.0 {
            angle += std::f32::consts::TAU;
        }
        // The cached angles are the same ones the painter uses, so a pointer cannot hit a
        // slice at a different angle than the one drawn — including mid-transition.
        for (index, entry) in self.angle_cache.iter().enumerate() {
            if !self.slices[index].visible {
                continue;
            }
            let Some((start, sweep)) = *entry else {
                continue;
            };
            let begin = start + std::f32::consts::FRAC_PI_2;
            if angle >= begin && angle < begin + sweep {
                return Some(index);
            }
        }
        self.angle_cache
            .iter()
            .rposition(|entry| entry.is_some())
            .filter(|index| self.slices.get(*index).is_some_and(|slice| slice.visible))
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
        if self.show_labels == show {
            return;
        }
        self.show_labels = show;
        self.base.request_redraw();
    }

    /// Returns whether labels are shown next to slices.
    pub fn show_labels(&self) -> bool {
        self.show_labels
    }

    /// Whether the hovered slice's tooltip is shown (CH-01).
    pub fn show_tooltip(&self) -> bool {
        self.show_tooltip
    }

    /// Enables or disables the hovered slice's tooltip.
    pub fn set_show_tooltip(&mut self, show: bool) {
        if self.show_tooltip == show {
            return;
        }
        self.show_tooltip = show;
        self.base.request_redraw();
    }

    /// Whether the legend column is reserved and drawn (CH-02).
    pub fn show_legend(&self) -> bool {
        self.show_legend
    }

    /// Enables or disables the legend column.
    pub fn set_show_legend(&mut self, show: bool) {
        if self.show_legend == show {
            return;
        }
        self.show_legend = show;
        self.base.request_redraw();
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

    /// The x-axis title.
    pub fn x_axis_label(&self) -> Option<&str> {
        self.x_axis_label.as_deref()
    }

    /// Sets the x-axis title.
    pub fn set_x_axis_label(&mut self, label: impl Into<String>) {
        self.x_axis_label = Some(label.into());
        self.base.request_redraw();
    }

    /// The y-axis title.
    pub fn y_axis_label(&self) -> Option<&str> {
        self.y_axis_label.as_deref()
    }

    /// Sets the y-axis title.
    pub fn set_y_axis_label(&mut self, label: impl Into<String>) {
        self.y_axis_label = Some(label.into());
        self.base.request_redraw();
    }

    /// Enables or disables showing percentage values on each slice.
    pub fn set_show_percentages(&mut self, show: bool) {
        if self.show_percentages == show {
            return;
        }
        self.show_percentages = show;
        self.base.request_redraw();
    }

    /// Returns whether percentage values are shown.
    pub fn show_percentages(&self) -> bool {
        self.show_percentages
    }

    /// Enables or disables donut (ring) mode.
    pub fn set_donut_mode(&mut self, donut: bool) {
        if self.donut == donut {
            return;
        }
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
        let ratio = ratio.clamp(0.0, 1.0);
        if self.donut_ratio == ratio {
            return;
        }
        self.donut_ratio = ratio;
        self.base.request_redraw();
    }

    /// Returns the donut hole ratio.
    pub fn donut_ratio(&self) -> f32 {
        self.donut_ratio
    }

    /// Returns the total sum of all displayed positive finite slice values.
    fn total_value(&self) -> f64 {
        self.slices
            .iter()
            .enumerate()
            .filter(|(_, slice)| slice.visible)
            .map(|(index, _)| self.displayed_value(index))
            .filter(|v| v.is_finite() && *v > 0.0)
            .sum()
    }

    /// Rebuilds the cached total and per-slice angles after the data changed.
    ///
    /// The arithmetic is the same f64 sum and f32 accumulation the draw loop used to
    /// perform on every frame, so a cached angle is bit-identical to the value that
    /// loop computed.
    fn ensure_angle_cache(&mut self) {
        if !self.angles_dirty {
            return;
        }
        let total: f64 = self
            .slices
            .iter()
            .enumerate()
            .filter(|(_, slice)| slice.visible)
            .map(|(index, _)| self.displayed_value(index))
            .filter(|v| v.is_finite() && *v > 0.0)
            .sum();
        self.cached_total = total;
        self.angle_cache.clear();
        if total <= 0.0 || !total.is_finite() {
            // The draw path shows the empty state; an empty table is all it needs.
            self.angles_dirty = false;
            return;
        }
        self.angle_cache.reserve(self.slices.len());
        let mut start = -std::f32::consts::FRAC_PI_2;
        for (index, slice) in self.slices.iter().enumerate() {
            let value = self.displayed_value(index);
            if !slice.visible || value <= 0.0 || !value.is_finite() {
                self.angle_cache.push(None);
                continue;
            }
            let sweep = (value / total * 2.0 * std::f32::consts::PI as f64) as f32;
            self.angle_cache.push(Some((start, sweep)));
            start += sweep;
        }
        self.angles_dirty = false;
    }

    /// Returns the center point of the chart.
    /// The circle's box: the control minus the legend column, when one is shown (CH-02).
    fn plot_geometry(&self) -> Rect {
        let rect = self.base.geometry();
        if !self.show_legend {
            return rect;
        }
        let legend_width = (rect.width / 3).min(140);
        Rect::new(rect.x, rect.y, rect.width.saturating_sub(legend_width), rect.height)
    }

    fn center(&self) -> Option<Point> {
        let rect = self.plot_geometry();
        if rect.width == 0 || rect.height == 0 {
            return None;
        }
        Some(Point::new(rect.x + (rect.width as i32) / 2, rect.y + (rect.height as i32) / 2))
    }

    /// Returns the outer radius of the chart.
    fn outer_radius(&self) -> f32 {
        let rect = self.plot_geometry();
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

    /// Paints the empty state: a panel and "No data".
    ///
    /// Every other chart in the crate draws its frame plus a message when it has no series,
    /// because the no-data state is what a chart shows before its first value and is the
    /// state most easily mistaken for a rendering failure. This control used to return
    /// before painting anything, leaving a hole in the parent.
    fn draw_empty_state(&self, context: &mut RenderContext, rect: Rect) {
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("pie_chart");
        let (window_fill, foreground) = {
            let manager = crate::style::theme_manager();
            match manager.current_theme() {
                Some(active) => (active.colors.background, active.colors.foreground),
                None => (Color::WHITE, Color::BLACK),
            }
        };
        let surface = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(window_fill);
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(foreground);
        context.fill_rect(rect, surface);
        context.draw_rect(rect, surface.blend(&ink, 0.2));
        let font = role_font(style, 12.0);
        let line = context.text_line(rect, &font);
        context.draw_text_fitted_cow(
            line,
            "No data",
            &font,
            ink.with_alpha(160),
            HorizontalAlignment::Center,
        );
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

/// Whether two label boxes intersect.
fn labels_overlap(a: Rect, b: Rect) -> bool {
    a.x < b.x.saturating_add(b.width as i32)
        && b.x < a.x.saturating_add(a.width as i32)
        && a.y < b.y.saturating_add(b.height as i32)
        && b.y < a.y.saturating_add(a.height as i32)
}

/// Moves a slice label out of the way of the labels already placed, or omits it.
///
/// The candidate is tried at its natural position and then at up to two rows above
/// and below it. Returning `None` is the deliberate fallback: a label that cannot be
/// placed without sitting on another label is dropped rather than painted twice at
/// the same spot, which is what made a clustered pie unreadable (CH-006).
fn place_label(candidate: Rect, occupied: &[Rect], rect: Rect, row_height: i32) -> Option<Rect> {
    let row = row_height.max(1);
    let offsets = [0, row, -row, 2 * row, -2 * row];
    for offset in offsets {
        let (clamped, _) = label_box(
            candidate.x,
            candidate.y + offset,
            candidate.width as i32,
            candidate.height as i32,
            rect,
        );
        if !occupied.iter().any(|placed| labels_overlap(clamped, *placed)) {
            return Some(clamped);
        }
    }
    None
}

impl Widget for PieChart {
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
        crate::core::Size::new(300, 300)
    }

    /// Advances the per-slice explosion animation and the value transition.
    fn tick(&mut self, delta_ms: u32) -> bool {
        self.sync_explode_progress();
        let step = (delta_ms as f32 / 150.0).clamp(0.0, 1.0);
        let mut moving = false;
        for index in 0..self.explode_progress.len() {
            let target = if self.slices.get(index).is_some_and(|slice| slice.exploded)
                || (self.explode_on_hover && self.hovered_slice == Some(index))
            {
                1.0
            } else {
                0.0
            };
            let current = self.explode_progress[index];
            if (current - target).abs() > f32::EPSILON {
                let next = if current < target {
                    (current + step).min(target)
                } else {
                    (current - step).max(target)
                };
                self.explode_progress[index] = next;
                moving = true;
            }
        }
        if self.animate && !self.from_values.is_empty() {
            moving |= self.transition.tick(delta_ms);
            // The angles are derived from the eased values, so every advanced frame has to
            // rebuild them; the final frame lands on the targets before the starts are dropped.
            self.angles_dirty = true;
            if !self.transition.is_moving() {
                self.from_values.clear();
                self.transition.jump_to(1.0);
                self.angles_dirty = true;
            }
        }
        if moving {
            self.base.request_redraw();
        }
        moving
    }

    /// Whether any slice is still exploding, collapsing or easing to a value.
    fn is_animating(&self) -> bool {
        let armed = |index: usize| {
            self.slices.get(index).is_some_and(|slice| slice.exploded)
                || (self.explode_on_hover && self.hovered_slice == Some(index))
        };
        (0..self.explode_progress.len()).any(|index| {
            (self.explode_progress[index] - if armed(index) { 1.0 } else { 0.0 }).abs()
                > f32::EPSILON
        }) || (self.animate && !self.from_values.is_empty() && self.transition.is_moving())
    }

    /// Resolves a published event name to the signal this chart actually emits.
    ///
    /// The capability publishes all three names with a `UInt` payload, and the
    /// control carries the slice index as a `usize`, so the conversion is the
    /// widening that keeps the subscriber's number identical to the emitted one.
    /// Without these arms the names would validate in `connect_event` and then
    /// never reach a signal.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "slice_clicked" => {
                Some(EventSignalRef::mapped("slice_clicked", &self.slice_clicked, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
            "slice_hovered" => {
                Some(EventSignalRef::mapped("slice_hovered", &self.slice_hovered, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
            "slice_unhovered" => {
                Some(EventSignalRef::mapped("slice_unhovered", &self.slice_unhovered, |v| {
                    CapabilityValue::UInt(*v as u64)
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
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        if self.slices.is_empty() {
            self.draw_empty_state(context, rect);
            return;
        }

        let is_enabled = self.base.is_enabled();
        self.ensure_angle_cache();
        let total = self.cached_total;
        if total <= 0.0 || !total.is_finite() {
            self.draw_empty_state(context, rect);
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

        // The label and percentage faces are the **effective font** at their role sizes, so the
        // theme family and the user's text scale reach the chart chrome (D09-STYLE-01).
        let label_font = role_font(self.base.style(), 10.0);
        let pct_font = role_font(self.base.style(), 9.0);
        // Chart chrome, derived from the active surface: `DARK_GRAY` was a light-chart literal
        // and rendered at 1.8:1 on the dark appearance's surface. The *slice* colours are
        // untouched — those identify the data (rule #108 ③). Shared with `bar_chart` and the
        // cartesian axes so the three cannot drift apart again.
        let slice_label_color = crate::widget::chart_widgets::charts::axis_chrome_color(0.75);

        // Draw sectors
        // One reused buffer for the percentage labels: `format!` built a fresh `String` per sector
        // per frame, and the text is at most four characters.
        // Labels are placed one at a time and must not land on an earlier label; the
        // `angle_cache` already omits zero-share slices, so they never reach this list.
        let mut occupied_labels: Vec<Rect> = Vec::new();
        let mut pct_text = crate::compat::String::new();
        for (slice_index, slice) in self.slices.iter().enumerate() {
            let Some((start_angle, sweep)) = self.angle_cache.get(slice_index).copied().flatten()
            else {
                continue;
            };
            let end_angle = start_angle + sweep;

            let slice_color =
                if is_enabled { slice.color } else { Color::rgba(200, 200, 200, 200) };

            // Exploded slice: shift the sector outward. The distance animates, so a hover
            // pull-out grows over the next few frames instead of popping.
            let explode_offset = self.explode_offset_for(slice_index);
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
                    let (_, clamped) = label_box(
                        label_x,
                        label_y,
                        metrics.width as i32,
                        metrics.height as i32,
                        rect,
                    );
                    let raw = Rect::new(
                        label_x,
                        label_y,
                        metrics.width.max(1) as u32,
                        metrics.height.max(1) as u32,
                    );
                    let row_height = metrics.height.max(1) as i32 + 2;
                    if let Some(bounds) = place_label(raw, &occupied_labels, rect, row_height) {
                        occupied_labels.push(bounds);
                        // The anchor is deliberately outside the pie, so the panel's chrome ink is
                        // the right colour — unless the control is too small for that and the box was
                        // pulled back inside, which lands the label on a slice. There the ink has to
                        // come from the slice, or the label is drawn panel-on-slice (measured 1.91:1
                        // on the dark appearance and 2.57:1 on the light one).
                        let ink =
                            if clamped { slice_color.contrast_color() } else { slice_label_color };
                        context.draw_text_fitted_cow(
                            bounds,
                            &slice.label,
                            &label_font,
                            ink,
                            HorizontalAlignment::Left,
                        );
                    }
                }

                // Draw percentage on the arc (inside, not overlapping label)
                if self.show_percentages && total > 0.0 {
                    let displayed = self.displayed_value(slice_index);
                    let pct = (displayed / total * 100.0).round() as i32;
                    pct_text.clear();
                    let _ = core::fmt::Write::write_fmt(&mut pct_text, format_args!("{pct}%"));
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
                    context.draw_text_fitted_cow(
                        bounds,
                        &pct_text,
                        &pct_font,
                        slice_color.contrast_color(),
                        HorizontalAlignment::Left,
                    );
                }
            }
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

        // ── Hovered-slice tooltip (CH-01) ──
        if self.show_tooltip {
            if let Some(index) = self.hovered_slice {
                if let (Some(slice), Some((start_angle, sweep))) =
                    (self.slices.get(index), self.angle_cache.get(index).copied().flatten())
                {
                    let mid = start_angle + sweep * 0.5;
                    let anchor = Self::point_on_circle(center, outer_radius * 0.65, mid);
                    let pct = if total > 0.0 { slice.value / total * 100.0 } else { 0.0 };
                    let text = std::format!("{}  {:.1} ({:.0}%)", slice.label, slice.value, pct);
                    let font = role_font(self.base.style(), 10.0);
                    let metrics = context.measure_text(&text, &font);
                    let width = metrics.width as i32 + 8;
                    let height = metrics.height.max(1) as i32 + 8;
                    let mut x = anchor.x + 8;
                    let mut y = anchor.y - height / 2;
                    if x + width > rect.x + rect.width as i32 {
                        x = anchor.x - width - 8;
                    }
                    x = x.max(rect.x);
                    y = y.max(rect.y).min(rect.y + rect.height as i32 - height);
                    let box_rect = Rect::new(x, y, width.max(1) as u32, height.max(1) as u32);
                    let surface = crate::widget::chart_widgets::charts::axis_chrome_color(0.96);
                    let tooltip_ink = surface.contrast_color();
                    context.fill_rect(box_rect, surface);
                    context.draw_rect(box_rect, tooltip_ink.with_alpha_f32(0.35));
                    context.draw_text_line_cow(
                        box_rect,
                        &text,
                        &font,
                        tooltip_ink,
                        HorizontalAlignment::Center,
                    );
                }
            }
        }

        // ── Legend column (CH-02) ──
        if self.show_legend {
            let geometry = self.plot_geometry();
            let legend_x = geometry.x + geometry.width as i32 + 8;
            let font = role_font(self.base.style(), 10.0);
            let legend_ink = crate::widget::chart_widgets::charts::axis_chrome_color(0.75);
            let row = 16i32;
            let visible_rows = ((rect.height as i32 - 16) / row).max(1) as usize;
            let max_scroll = self.slices.len().saturating_sub(visible_rows);
            self.legend_scroll = self.legend_scroll.min(max_scroll);
            let start = self.legend_scroll;
            let mut y = rect.y + 8;
            let mut drawn = 0usize;
            for slice in self.slices.iter().skip(start).take(visible_rows) {
                let color = if slice.visible { slice.color } else { slice.color.with_alpha(70) };
                context.fill_rect(Rect::new(legend_x, y + 3, 10, 10), color);
                context.draw_text_fitted_cow(
                    Rect::new(
                        legend_x + 14,
                        y,
                        (rect.right() - legend_x - 14).max(0) as u32,
                        row as u32,
                    ),
                    &slice.label,
                    &font,
                    legend_ink,
                    HorizontalAlignment::Left,
                );
                y += row;
                drawn += 1;
            }
            self.last_legend_area = Some(LegendArea {
                x: legend_x - 4,
                y: rect.y + 8,
                width: (rect.right() - legend_x + 4).max(0),
                row_height: row,
                rows: drawn,
            });
            // A track and thumb make the hidden rows discoverable rather than silently
            // truncated; the wheel scrolls the column (CH-009).
            if max_scroll > 0 {
                let track =
                    Rect::new(rect.right() - 3, rect.y + 8, 2, (drawn as i32 * row).max(1) as u32);
                context.fill_rect(track, legend_ink.with_alpha(40));
                let thumb_height = ((track.height as usize * visible_rows)
                    / self.slices.len().max(1))
                .max(8)
                .min(track.height as usize) as u32;
                let travel = track.height.saturating_sub(thumb_height);
                let thumb_y = track.y + ((travel as usize * start) / max_scroll.max(1)) as i32;
                context.fill_rect(
                    Rect::new(track.x, thumb_y, track.width, thumb_height.max(1)),
                    legend_ink.with_alpha(140),
                );
            }
        } else {
            self.last_legend_area = None;
        }

        // ── Title and axis titles (CH-07) ──
        let chrome = crate::widget::chart_widgets::charts::axis_chrome_color(0.85);
        if let Some(title) = self.title.as_deref() {
            let font = role_font(self.base.style(), 12.0).with_bold(true);
            context.draw_text_fitted_cow(
                Rect::new(rect.x, rect.y, rect.width, 16),
                title,
                &font,
                chrome,
                HorizontalAlignment::Center,
            );
        }
        if let Some(label) = self.x_axis_label.as_deref() {
            let font = role_font(self.base.style(), 10.0);
            context.draw_text_fitted_cow(
                Rect::new(rect.x, rect.y + rect.height as i32 - 14, rect.width, 12),
                label,
                &font,
                chrome,
                HorizontalAlignment::Center,
            );
        }
        if let Some(label) = self.y_axis_label.as_deref() {
            let font = role_font(self.base.style(), 10.0);
            context.draw_text_fitted_cow(
                Rect::new(rect.x + 2, rect.y + 16, rect.width.min(120), 12),
                label,
                &font,
                chrome,
                HorizontalAlignment::Left,
            );
        }
    }
}

impl EventHandler for PieChart {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MouseMove { pos } => {
                self.last_pointer = Some(*pos);
                let next = self.slice_at(*pos);
                if next != self.hovered_slice {
                    if let Some(index) = self.hovered_slice {
                        self.slice_unhovered.emit(index);
                    }
                    self.hovered_slice = next;
                    if let Some(index) = next {
                        self.slice_hovered.emit(index);
                    }
                    self.base.request_redraw();
                }
            }
            Event::MouseLeave { .. } => {
                if let Some(index) = self.hovered_slice.take() {
                    self.slice_unhovered.emit(index);
                    self.base.request_redraw();
                }
            }
            Event::MousePress { pos, button, .. } if *button == 1 => {
                // A legend row toggles its slice; it is not a pie hit, so consume it first.
                if let Some(index) = self.legend_entry_at(*pos) {
                    let visible = self.is_series_visible(index);
                    self.set_series_visible(index, !visible);
                    return;
                }
                self.base.set_mouse_pressed(true);
                if let Some(index) = self.slice_at(*pos) {
                    self.slice_clicked.emit(index);
                }
            }
            Event::MouseRelease { button, .. } if *button == 1 => {
                self.base.set_mouse_pressed(false);
            }
            Event::Wheel { delta, .. } if delta.y != 0 => {
                // The wheel scrolls the legend only while the pointer is over it; over the
                // ring it stays a no-op, because the pie itself has no zoom.
                let over_legend = self.last_pointer.is_some_and(|pos| {
                    self.legend_entry_at(pos).is_some()
                        || self.last_legend_area.is_some_and(|area| {
                            pos.x >= area.x
                                && pos.x < area.x + area.width
                                && pos.y >= area.y
                                && pos.y < area.y + area.row_height * area.rows as i32
                        })
                });
                if over_legend {
                    self.scroll_legend(if delta.y > 0 { -3 } else { 3 });
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
    fn published_slice_events_resolve_to_signals() {
        let chart = PieChart::new(Rect::new(0, 0, 200, 200));
        for name in ["slice_clicked", "slice_hovered", "slice_unhovered"] {
            assert!(chart.event_signal_dyn(name).is_some(), "{name} must resolve");
        }
        assert!(chart.event_signal_dyn("no_such_event").is_none());
    }

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
        pc.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1, modifiers: 0 });
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

    #[test]
    fn pie_chart_non_positive_and_nan_values_safe() {
        let mut pc = PieChart::new(Rect::new(0, 0, 200, 200));
        pc.add_slice(PieSlice::new("Zero", 0.0, Color::RED));
        pc.add_slice(PieSlice::new("Neg", -10.0, Color::BLUE));
        pc.add_slice(PieSlice::new("NaN", f64::NAN, Color::GREEN));
        let svg = render_to_svg(&mut pc);
        assert!(svg.starts_with("<svg"));
    }

    /// The slice under the pointer is identified from the angle and radius.
    #[test]
    fn a_pointer_position_maps_to_its_slice() {
        let mut pc = PieChart::new(Rect::new(0, 0, 200, 200));
        pc.set_slices(vec![
            PieSlice::new("A", 50.0, Color::RED),
            PieSlice::new("B", 50.0, Color::BLUE),
        ]);
        let center = pc.center().unwrap();
        // The first half starts at 12 o'clock and sweeps clockwise, so the point to the
        // right of the centre belongs to slice A and the point to the left to slice B.
        assert_eq!(pc.slice_at(Point::new(center.x + 60, center.y)), Some(0));
        assert_eq!(pc.slice_at(Point::new(center.x - 60, center.y)), Some(1));
        assert_eq!(pc.slice_at(Point::new(0, 0)), None, "the corner is outside the disc");
    }

    /// Hover and click publish the slice index; hover is cleared on leave.
    #[test]
    fn hover_and_click_emit_slice_signals() {
        use std::sync::{Arc, Mutex};
        let mut pc = PieChart::new(Rect::new(0, 0, 200, 200));
        pc.set_slices(vec![
            PieSlice::new("A", 50.0, Color::RED),
            PieSlice::new("B", 50.0, Color::BLUE),
        ]);
        let center = pc.center().unwrap();
        let hovered = Arc::new(Mutex::new(Vec::<usize>::new()));
        let clicked = Arc::new(Mutex::new(Vec::<usize>::new()));
        let sink = Arc::clone(&hovered);
        pc.slice_hovered.connect(move |index| sink.lock().unwrap().push(*index));
        let sink = Arc::clone(&clicked);
        pc.slice_clicked.connect(move |index| sink.lock().unwrap().push(*index));
        pc.handle_event(&Event::MouseMove { pos: Point::new(center.x + 60, center.y) });
        assert_eq!(pc.hovered_slice(), Some(0));
        pc.handle_event(&Event::MousePress {
            pos: Point::new(center.x - 60, center.y),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(*hovered.lock().unwrap(), vec![0]);
        assert_eq!(*clicked.lock().unwrap(), vec![1]);
        pc.handle_event(&Event::MouseLeave { pos: Point::new(0, 0) });
        assert_eq!(pc.hovered_slice(), None);
    }

    /// CH-01/CH-02/CH-07: a hovered slice names itself and its value, the legend lists the
    /// slices, and the title/axis titles are painted.
    #[test]
    fn tooltip_legend_and_titles_are_drawn() {
        let mut pc = PieChart::new(Rect::new(0, 0, 400, 300));
        pc.set_slices(vec![
            PieSlice::new("Alpha", 1.0, Color::PRIMARY),
            PieSlice::new("Beta", 3.0, Color::rgb(0, 180, 90)),
        ]);
        pc.set_show_tooltip(true);
        pc.set_show_legend(true);
        pc.set_title("Share");
        pc.set_x_axis_label("x");
        pc.set_y_axis_label("y");
        let baseline = render_to_svg(&mut pc);
        let runs_without_tooltip = crate::widget::svg::text_ink_boxes(&baseline).len();

        // The top of the circle is the first slice's start (12 o'clock).
        let center = pc.center().expect("a sized chart has a centre");
        let radius = pc.outer_radius() * 0.6;
        pc.handle_event(&Event::MouseMove { pos: Point::new(center.x, center.y - radius as i32) });
        assert_eq!(pc.hovered_slice(), Some(0));

        let svg = render_to_svg(&mut pc);
        assert!(
            crate::widget::svg::text_ink_boxes(&svg).len() > runs_without_tooltip,
            "the tooltip adds a text run"
        );
    }

    /// CH-001: a legend click hides the slice and reports it.
    #[test]
    fn a_pie_legend_click_toggles_the_slice() {
        use std::sync::{Arc, Mutex};
        let mut pc = PieChart::new(Rect::new(0, 0, 360, 260));
        pc.set_slices(vec![
            PieSlice::new("Alpha", 1.0, Color::PRIMARY),
            PieSlice::new("Beta", 2.0, Color::rgb(0, 180, 90)),
        ]);
        pc.set_show_legend(true);
        let _ = render_to_svg(&mut pc);
        let area = pc.last_legend_area.expect("the legend has a hit area");
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&seen);
        pc.series_toggled.connect(move |value| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*value);
            }
        });
        pc.handle_event(&Event::mouse_press(area.x + 2, area.y + 2, 1));
        assert!(!pc.is_series_visible(0));
        assert_eq!(*seen.lock().unwrap(), vec![(0, false)]);
        pc.ensure_angle_cache();
        assert!(pc.angle_cache[0].is_none(), "a hidden slice drops out of the ring");
    }

    /// CH-005: pie values ease by default and can be made immediate.
    #[test]
    fn pie_value_changes_ease_by_default_and_can_be_disabled() {
        let mut pc = PieChart::new(Rect::new(0, 0, 240, 240));
        pc.set_slices(vec![
            PieSlice::new("A", 1.0, Color::PRIMARY),
            PieSlice::new("B", 1.0, Color::rgb(0, 180, 90)),
        ]);
        pc.set_slices(vec![
            PieSlice::new("A", 3.0, Color::PRIMARY),
            PieSlice::new("B", 1.0, Color::rgb(0, 180, 90)),
        ]);
        assert!(pc.is_animated());
        assert!(pc.displayed_value(0) < 3.0, "the new value starts behind the target");
        assert!(pc.is_animating());
        let mut frames = 0;
        while pc.tick(16) && frames < 200 {
            frames += 1;
        }
        assert!((pc.displayed_value(0) - 3.0).abs() < 1e-6);
        assert!(!pc.is_animating());

        pc.set_animate(false);
        pc.set_slices(vec![
            PieSlice::new("A", 9.0, Color::PRIMARY),
            PieSlice::new("B", 1.0, Color::rgb(0, 180, 90)),
        ]);
        assert!((pc.displayed_value(0) - 9.0).abs() < 1e-9);
    }

    /// CH-006: labels nudge away from an earlier label, and a zero-share slice is skipped.
    #[test]
    fn overlapping_labels_move_or_disappear_and_zero_slices_are_skipped() {
        let rect = Rect::new(0, 0, 120, 120);
        let first = Rect::new(10, 10, 30, 10);
        let second = Rect::new(10, 12, 30, 10);
        let placed = place_label(second, &[first], rect, 12).expect("a row below is free");
        assert!(!labels_overlap(placed, first));
        assert!(place_label(first, &[first], rect, 1).is_none(), "no collision-free slot");

        let mut pc = PieChart::new(Rect::new(0, 0, 240, 240));
        pc.set_slices(vec![
            PieSlice::new("Keep", 100.0, Color::PRIMARY),
            PieSlice::new("Zero", 0.0, Color::rgb(0, 180, 90)),
        ]);
        pc.ensure_angle_cache();
        assert!(pc.angle_cache[0].is_some());
        assert!(pc.angle_cache[1].is_none(), "a zero-share slice gets no sector or label");
    }

    /// CH-009: an overflowing legend scrolls instead of truncating.
    #[test]
    fn an_overflowing_pie_legend_scrolls() {
        let mut pc = PieChart::new(Rect::new(0, 0, 320, 120));
        pc.set_show_legend(true);
        pc.set_slices(
            (0..20)
                .map(|index| PieSlice::new(format!("slice-{index}"), 1.0, Color::PRIMARY))
                .collect(),
        );
        let _ = render_to_svg(&mut pc);
        let area = pc.last_legend_area.expect("the legend has a hit area");
        assert!(area.rows < 20, "the short control cannot show every row");
        pc.handle_event(&Event::mouse_move(area.x + 2, area.y + 2));
        pc.handle_event(&Event::wheel(0, -1, 0));
        assert!(pc.legend_scroll_offset() > 0, "the wheel advances the legend");
        let scrolled = pc.legend_scroll_offset();
        pc.handle_event(&Event::wheel(0, 1, 0));
        assert!(pc.legend_scroll_offset() < scrolled, "the wheel scrolls back");
    }
}
