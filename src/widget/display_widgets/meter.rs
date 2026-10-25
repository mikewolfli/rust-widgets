// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Meter widget — gauge with arc and needle indicator (BLUE13 R2.14).
//!
//! A simplified gauge/indicator widget that draws a 270° arc (starting from
//! 135°) as a background track, a colored arc up to the current value, and
//! a needle pointing at the value.
//!
//! # `Meter` vs `ProgressCircle` — and why there is no `Gauge`
//!
//! | | `ProgressCircle` | `Meter` |
//! |---|---|---|
//! | represents | completion (0 → max) | **a measurement** at a place in a range |
//! | ticks | none | ✅ `set_tick_count` |
//! | threshold bands | none | ✅ `add_threshold_range` |
//! | typical reading | "the task is 60% done" | "the CPU is 72 °C, in the red band" |
//!
//! There is deliberately **no separate `Gauge` control**: this module already
//! describes "a gauge with arc and needle" and the factory already accepts `gauge`
//! as an alias for it (see `meter_capability`). A new `Gauge` type would produce two
//! controls both answering to the name `gauge`, which is the duplication rule #78
//! exists to prevent. Missing capability belongs here.

use crate::compat::{format, String, ToString, Vec};
use crate::core::{deg_to_rad, Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler, WheelScroll};
use crate::render::{RenderCommand, RenderContext};
use crate::widget::capability::coercion::{expect_bool, expect_string, expect_u32};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::role_font;
use crate::widget::numeric::ordered_clamp_u32;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// A colored band across part of the meter's range.
///
/// Bands are the gauge reading's second half: a needle says *where* the value is,
/// a band says *whether that is good*. The classic triple is
/// green/yellow/red over low/mid/high.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeterThreshold {
    /// Inclusive lower bound of the band, in the meter's own value units.
    pub from: u32,
    /// Inclusive upper bound of the band.
    pub to: u32,
    /// Colour the arc segment is drawn in.
    pub color: Color,
}

/// The radial length of a tick mark, in pixels: 6.
///
/// Named once because the tick's inner end, its outer end and the radius the labels sit
/// at all measure from it — see `Meter::draw`. It used to appear as a literal `6` at the
/// tick and again inside the label radius, so the two could drift apart.
const TICK_MARK_LENGTH: u32 = 6;

/// Half the width of the gauge's drawn box, as a fraction of its radius: `sin 45° ≈ 0.707`.
///
/// The 270° sweep runs from 45° to 315°, so its extreme x are `±cos 45°` and its extreme y
/// are `±sin 45°`: the box is `1.414 · radius` square. This is the one fact the radius and
/// the centre both derive from — deriving the radius from the *height* alone put the arc off
/// the top edge, and anchoring the centre at the control's left edge left the gauge hanging
/// off its left side while its rightmost vertex ran past the far edge.
const SWEEP_HALF_EXTENT: f32 = std::f32::consts::FRAC_1_SQRT_2;

/// Meter (gauge) widget — displays a value on an arc with a needle indicator.
pub struct Meter {
    base: BaseWidget,
    /// Current displayed value.
    value: u32,
    /// Minimum value of the range.
    min: u32,
    /// Maximum value of the range.
    max: u32,
    /// Number of tick marks.
    tick_count: u32,
    /// Bands drawn over the arc, in insertion order.
    thresholds: Vec<MeterThreshold>,
    /// Whether the numeric value is drawn at each tick.
    show_tick_labels: bool,
    /// Whether pointer drags and the wheel set the value.
    interactive: bool,
    /// Whether the value arc fills whole tick segments instead of continuously.
    discrete: bool,
    /// Whether a primary-button drag is currently underway.
    dragging: bool,
    /// Suffix appended to the value in labels, e.g. `°C`.
    unit: String,
    /// Scratch buffer for an arc's polyline vertices.
    ///
    /// Every arc on the ring is stroked as a sampled polyline, so the draw pass used to
    /// `collect()` a new `Vec<Point>` per arc per frame — three or more allocations while the
    /// needle was still. The sweep fills this buffer instead, hands it to the render command by
    /// reference, and takes it back afterwards, so the capacity survives the frame.
    sweep_points: Vec<Point>,
    /// The reading's face, keyed by the effective style font that produced it.
    value_font_cache: Option<(Font, Font)>,
    /// The fixed tick-label face, built once.
    tick_font_cache: Option<Font>,
    /// The tick-label text and measurements, keyed by the range, count and unit.
    tick_label_cache: Option<TickLabelCache>,
    /// The centre reading, reused while the value and unit are unchanged.
    value_text_cache: Option<ValueTextCache>,
}

/// The tick labels and the `000` half-width the label radius is derived from.
struct TickLabelCache {
    minimum: u32,
    maximum: u32,
    tick_count: u32,
    unit: String,
    dpi_scale_bits: u32,
    label_half: u32,
    /// `(text, width, height)` per tick, in order.
    labels: Vec<(String, u32, u32)>,
}

/// The reading at the gauge's foot and its measured bounds.
struct ValueTextCache {
    value: u32,
    unit: String,
    dpi_scale_bits: u32,
    text: String,
    width: u32,
    height: u32,
}

impl Meter {
    /// Creates a new Meter widget with the given geometry.
    ///
    /// Defaults: value 0, range 0-100, 5 tick marks, no threshold bands, tick
    /// labels off, no unit, read-only and continuous.
    pub fn new(rect: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Meter, rect, "Meter"),
            value: 0,
            min: 0,
            max: 100,
            tick_count: 5,
            thresholds: Vec::new(),
            show_tick_labels: false,
            interactive: false,
            discrete: false,
            dragging: false,
            unit: String::new(),
            sweep_points: Vec::new(),
            value_font_cache: None,
            tick_font_cache: None,
            tick_label_cache: None,
            value_text_cache: None,
        }
    }

    /// Returns the current value.
    pub fn value(&self) -> u32 {
        self.value
    }

    /// Sets the value, clamped between min and max.
    ///
    /// Emits `changed` signal when the value actually changes.
    ///
    /// `changed` is deliberately not gated by `enabled`: a readout's value transition is
    /// driven by data, not by the user, so disabling the readout must not freeze the
    /// signal its host uses to follow that data. See `Arc::set_value` for the full
    /// reasoning.
    pub fn set_value(&mut self, v: u32) {
        let clamped = ordered_clamp_u32(v, self.min, self.max);
        if self.value == clamped {
            return;
        }
        self.value = clamped;
        self.base.changed.emit();
        self.base.request_redraw();
    }

    /// Sets both minimum and maximum values in one call.
    ///
    /// The current value is re-clamped to the new range.
    ///
    /// Like [`Meter::set_value`], `changed` is deliberately not gated by `enabled`.
    pub fn set_range(&mut self, min: u32, max: u32) {
        self.min = min.min(max);
        self.max = max.max(min);
        let clamped = ordered_clamp_u32(self.value, self.min, self.max);
        if self.value != clamped {
            self.value = clamped;
            self.base.changed.emit();
        }
        self.base.request_redraw();
    }

    /// Returns the lower bound of the range.
    ///
    /// The capability layer publishes `minimum`, so the accessor has to exist on
    /// its own rather than only as half of [`Self::set_range`].
    pub fn minimum(&self) -> u32 {
        self.min
    }

    /// Sets the lower bound, keeping it at or below [`Self::maximum`].
    ///
    /// Delegates to `set_range` so the value is re-clamped exactly once and the
    /// two bounds can never end up inverted.
    pub fn set_minimum(&mut self, minimum: u32) {
        self.set_range(minimum, self.max);
    }

    /// Returns the upper bound of the range.
    pub fn maximum(&self) -> u32 {
        self.max
    }

    /// Sets the upper bound, keeping it at or above [`Self::minimum`].
    pub fn set_maximum(&mut self, maximum: u32) {
        self.set_range(self.min, maximum);
    }

    /// Sets the number of tick marks drawn along the arc.
    pub fn set_tick_count(&mut self, count: u32) {
        let count = count.max(2);
        if self.tick_count == count {
            return;
        }
        self.tick_count = count;
        self.base.request_redraw();
    }

    /// Returns the number of tick marks drawn along the arc.
    pub fn tick_count(&self) -> u32 {
        self.tick_count
    }

    /// Adds a colored band over `from..=to`.
    ///
    /// # Why the bounds are clamped rather than rejected
    ///
    /// A band outside the meter's range is a caller mistake, but it is not an
    /// unrecoverable one and there is exactly one sensible reading of it: the part
    /// that overlaps the range. Rejecting would force every caller to clamp first;
    /// silently keeping out-of-range bounds would draw arc segments past the end of
    /// the track. Clamping does the useful thing and keeps the drawing honest.
    ///
    /// An inverted or empty band (`from > to`) is dropped, because there is no arc
    /// left once the overlap is taken.
    pub fn add_threshold_range(&mut self, from: u32, to: u32, color: Color) {
        let (low, high) = (from.min(to), from.max(to));
        let from = low.max(self.min);
        let to = high.min(self.max);
        if from > to {
            return;
        }
        self.thresholds.push(MeterThreshold { from, to, color });
        self.base.request_redraw();
    }

    /// Removes every band.
    pub fn clear_thresholds(&mut self) {
        self.thresholds.clear();
        self.base.request_redraw();
    }

    /// Returns the bands, in insertion order.
    pub fn thresholds(&self) -> &[MeterThreshold] {
        &self.thresholds
    }

    /// Sets whether the numeric value is drawn beside each tick.
    pub fn set_show_tick_labels(&mut self, show: bool) {
        if self.show_tick_labels == show {
            return;
        }
        self.show_tick_labels = show;
        self.base.request_redraw();
    }

    /// Returns whether tick labels are drawn.
    pub fn show_tick_labels(&self) -> bool {
        self.show_tick_labels
    }

    /// Returns whether dragging and the wheel may set the value.
    ///
    /// A meter is a read-only indicator by default; interaction is opt-in so a
    /// gauge displaying a measurement its host owns cannot be moved by a stray
    /// drag.
    pub fn is_interactive(&self) -> bool {
        self.interactive
    }

    /// Enables or disables pointer and wheel input.
    ///
    /// When enabled, a drag inside the control (and the wheel over it) writes
    /// the value through [`Self::set_value`], so the existing `changed` signal
    /// is emitted exactly once per real change.
    pub fn set_interactive(&mut self, interactive: bool) {
        if self.interactive == interactive {
            return;
        }
        self.interactive = interactive;
        if !interactive {
            self.dragging = false;
        }
        self.base.request_redraw();
    }

    /// Returns whether the value arc fills whole tick segments.
    pub fn is_discrete(&self) -> bool {
        self.discrete
    }

    /// Chooses between a continuous value arc and one that fills whole tick
    /// segments.
    ///
    /// In discrete mode the arc stops at the last tick at or below the value,
    /// so it always agrees with the scale drawn beside it. The default is
    /// `false`, which keeps the continuous sweep.
    pub fn set_discrete(&mut self, discrete: bool) {
        if self.discrete == discrete {
            return;
        }
        self.discrete = discrete;
        self.base.request_redraw();
    }

    /// Sets the unit suffix shown after the value, e.g. `°C`.
    pub fn set_unit(&mut self, unit: &str) {
        if self.unit == unit {
            return;
        }
        self.unit = unit.to_string();
        self.base.request_redraw();
    }

    /// Returns the unit suffix.
    pub fn unit(&self) -> &str {
        &self.unit
    }

    /// The value rendered as text, with the unit appended when one is set.
    ///
    /// Exposed because it is what the control draws and what a caller would show
    /// elsewhere; deriving it in two places is how the two spellings drift.
    pub fn value_text(&self) -> String {
        format!("{}{}", self.value, self.unit)
    }

    /// Normalizes the current value to a fraction in [0.0, 1.0].
    fn normalized_value(&self) -> f32 {
        if self.max <= self.min {
            return 0.0;
        }
        (self.value - self.min) as f32 / (self.max - self.min) as f32
    }

    /// Normalizes an arbitrary value in the meter's range to [0.0, 1.0].
    fn normalized(&self, value: u32) -> f32 {
        if self.max <= self.min {
            return 0.0;
        }
        ((ordered_clamp_u32(value, self.min, self.max) - self.min) as f32)
            / (self.max - self.min) as f32
    }

    /// The colour of the needle and the pivot, derived from the accent.
    ///
    /// The needle is the control's value indicator: it points at a reading, exactly as the
    /// value arc's fill does, so it takes the accent token rather than the text colour.
    /// `text_color` resolves to black in both appearances, so an indicator drawn in it is
    /// identical in light and dark — which is the theme-blindness this control was reported
    /// for.
    ///
    /// `surface` is the meter's own backdrop. The needle is pushed away from **it** rather than
    /// toward a fixed black: blending toward black is a light-theme assumption written as
    /// arithmetic, and on a dark surface it darkened the needle into the background, which is
    /// why the gauge's own pointer measured 3.34:1 while pointing at nothing distinct.
    fn needle_color_on(accent: &Color, surface: &Color) -> Color {
        accent.legible_on(*surface, 4.5)
    }

    /// The colour of the tick marks and their labels, derived from the accent.
    ///
    /// `surface` is the meter's own backdrop: the mark is pushed away from **it**, not toward
    /// a fixed black. Blending toward black is a light-theme assumption baked into arithmetic —
    /// on a dark surface it darkens the mark into the background, which measured 3.34:1 here,
    /// so the scale's own numbers were the least legible thing on the gauge.
    fn tick_color_on(accent: &Color, surface: &Color) -> Color {
        accent.legible_on(*surface, 4.5)
    }

    /// The value a normalized position represents, for tick labels.
    fn value_at_fraction(&self, fraction: f32) -> u32 {
        let span = (self.max - self.min) as f32;
        (self.min as f32 + span * fraction).round() as u32
    }

    /// The fraction of the sweep the value arc covers.
    ///
    /// This is the continuous value fraction unless [`Self::is_discrete`] is
    /// set, in which case it is snapped down to the tick grid the scale is
    /// drawn on. Snapping here rather than at the stored value keeps the
    /// reading and the needle exact while the arc alone moves in whole
    /// segments.
    fn filled_fraction(&self) -> f32 {
        let fraction = self.normalized_value();
        let steps = self.tick_count.saturating_sub(1);
        if !self.discrete || steps == 0 {
            return fraction;
        }
        let step = 1.0 / steps as f32;
        (fraction / step).floor() * step
    }

    /// The value a pointer position selects on the gauge, or `None` when the
    /// control has no drawn gauge to point at.
    ///
    /// The sweep runs clockwise from 45° to 315°, so the pointer's angle is
    /// measured in the same frame the arcs are drawn in. The 90° wedge below the
    /// pivot is outside the scale; a pointer there snaps to the nearer end
    /// rather than wrapping to the other side.
    fn value_at_point(&self, point: Point) -> Option<u32> {
        let (center, _) = self.gauge_geometry()?;
        let dx = (point.x - center.x) as f32;
        let dy = (point.y - center.y) as f32;
        if dx == 0.0 && dy == 0.0 {
            return None;
        }
        let angle = dy.atan2(dx).to_degrees();
        let sweep = 270.0_f32;
        let relative = (angle - 45.0).rem_euclid(360.0);
        let along = if relative <= sweep {
            relative
        } else if relative - sweep <= 360.0 - relative {
            sweep
        } else {
            0.0
        };
        Some(self.value_at_fraction(along / sweep))
    }

    /// How far one wheel notch moves the value: one tick's worth, at least one.
    fn wheel_step(&self) -> u32 {
        let steps = self.tick_count.saturating_sub(1).max(1);
        ((self.max - self.min) / steps).max(1)
    }

    /// The gauge's drawn centre and radius, or `None` when the control is too
    /// small to draw one.
    ///
    /// Shared by the paint path and the input mapping so a drag reads the same
    /// geometry the gauge was drawn with.
    ///
    /// # The fit
    ///
    /// The reading is written below the pivot, so one of the two has to give when
    /// the box is short. Reserving the reading's band up front lets the answer be
    /// "the arc shrinks" rather than "half the arc is drawn outside the control and
    /// a raster backend clips it away silently". A tall box reserves the whole
    /// band; a short one caps it at half the height, because otherwise there would
    /// be no arc left to shrink and the control would paint nothing at all (P1).
    ///
    /// The 270° sweep runs from 45° to 315° once the −90° phase offset is applied.
    /// Its extreme x are `cos 135° = -0.707` (left) and `cos 45° = +0.707` (right),
    /// and its extreme y are `sin 45° = +0.707`/`sin 315° = -0.707`, so the drawn
    /// box is `1.414 · radius` on **both** axes — a square, biased neither left nor
    /// right. The radius therefore comes from the drawn band's half-extent, and the
    /// centre is placed so the drawn box is centred in the control.
    fn gauge_geometry(&self) -> Option<(Point, u32)> {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return None;
        }
        let reading_reserve = Meter::reading_band();
        let arc_band_height =
            (rect.height as i32 - reading_reserve.min(rect.height as i32 / 2)).max(1);
        let ring_radius = ((rect.width.min(rect.height) / 2) as f32 / SWEEP_HALF_EXTENT) as u32;
        let ring_radius = ring_radius.saturating_sub(4).min(arc_band_height as u32 / 2).max(1);
        if ring_radius < 10 {
            return None;
        }
        // The `- 4` above is the ring's breathing room plus the half-width of its
        // stroke; the clamp keeps the whole drawn box inside the control when the
        // radius is small relative to the rectangle it was offered.
        let half_extent = (ring_radius as f32 * SWEEP_HALF_EXTENT).ceil() as i32;
        let max_center_x = (rect.x + rect.width as i32 - half_extent).max(rect.x + half_extent);
        let center_x = (rect.x + rect.width as i32 / 2).clamp(rect.x + half_extent, max_center_x);
        let max_center_y = (rect.y + rect.height as i32 - half_extent).max(rect.y + half_extent);
        let center_y = (rect.y + rect.height as i32 / 2).clamp(rect.y + half_extent, max_center_y);
        Some((Point::new(center_x, center_y), ring_radius))
    }

    /// The angular step between the ring's polyline vertices, in radians.
    ///
    /// Shared by every arc on the ring because the *vertices* are what make two
    /// overlapping strokes coincide; six degrees is fine enough that the chords are
    /// indistinguishable from a true arc at the radii this control draws at.
    const SEGMENT: f32 = std::f32::consts::PI / 30.0;

    /// The height reserved at the bottom of the control for the reading.
    ///
    /// Both the arc's fit and the reading's own placement derive their geometry from
    /// this, so the two cannot disagree about how much room the reading has: a margin
    /// written out twice is a margin that drifts in one of the two places.
    fn reading_band() -> i32 {
        24
    }

    /// Strokes the gauge's whole 270° sweep.
    ///
    /// The sweep is emitted as one stroked polyline through the circle's points rather
    /// than as a `DrawArc` path. The SVG backend writes an arc as its two end points plus
    /// a radius, and a bounds reader can only infer the drawn extent from that: inflating
    /// each end point by the whole radius is the conservative reading, and for this 270°
    /// sweep that inference reports the track at `[103,-35..215,99]` — outside the control
    /// on three sides — even though the true curve fits. A polyline has no implied
    /// geometry: every vertex is a point the backend is given, so the drawn extent and the
    /// reported extent are the same set of coordinates.
    fn draw_gauge_arc(
        points: &mut Vec<Point>,
        context: &mut RenderContext,
        center: Point,
        radius: u32,
        arc_start_deg: f32,
        arc_sweep_deg: f32,
        offset: f32,
        color: Color,
    ) {
        let start = deg_to_rad(arc_start_deg + offset);
        let end = deg_to_rad(arc_start_deg + arc_sweep_deg + offset);
        Self::drag_sweep(points, context, center, radius, start, end, color);
    }

    /// Strokes the arc from `start_angle` to `end_angle` as a polyline on the circle.
    ///
    /// The vertex count follows the swept angle, so a narrow threshold band costs a couple
    /// of segments while the 270° track stays smooth. Six degrees per segment is
    /// indistinguishable from a true arc at the radii this control draws at.
    ///
    /// `points` is the control's reusable vertex buffer. It is cleared and refilled here, then
    /// handed to the command through [`RenderContext::execute_command_ref`] — the entry point that
    /// leaves ownership with the caller — and reclaimed once the backend has consumed the
    /// command. The emitted command, its vertices and their order are exactly what the old
    /// `collect()` produced, so the picture is unchanged.
    fn drag_sweep(
        points: &mut Vec<Point>,
        context: &mut RenderContext,
        center: Point,
        radius: u32,
        start_angle: f32,
        end_angle: f32,
        color: Color,
    ) {
        let sweep = end_angle - start_angle;
        if sweep.abs() < 0.001 {
            return;
        }
        let segments = (sweep.abs() / Self::SEGMENT).ceil().max(1.0) as u32;
        let step = sweep / segments as f32;
        points.clear();
        points.reserve(segments as usize + 1);
        for segment in 0..=segments {
            let angle = start_angle + step * segment as f32;
            points.push(Point::new(
                center.x + (radius as f32 * angle.cos()).round() as i32,
                center.y + (radius as f32 * angle.sin()).round() as i32,
            ));
        }
        // The command shares an immutable snapshot of the vertices (G-02), while `points` stays the
        // control's reusable buffer for the next arc.
        let command = RenderCommand::DrawPath {
            points: points.as_slice().into(),
            closed: false,
            color,
            filled: false,
            width: 1,
        };
        context.execute_command_ref(&command);
    }

    /// Snaps `angle` onto the vertex grid that starts at `origin`.
    ///
    /// Every arc on the ring is drawn from this one grid, so a stroke that covers another
    /// passes through the *same* vertices and covers it exactly, rather than leaving a
    /// hairline of the under-colour between two polylines that approximated the same arc
    /// at different angles.
    fn snap_to_grid(angle: f32, origin: f32) -> f32 {
        let steps = ((angle - origin) / Self::SEGMENT).round();
        origin + steps * Self::SEGMENT
    }
}

impl Widget for Meter {
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
        Size::new(200, 200)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why the field is named per arm
    ///
    /// The published name and the Rust field name are not always the same (`find_next` is backed by
    /// `find_next_signal`, `dismissed` by a `Signal1<()>` field). `connect_event` validates a name
    /// against the capability table and registers a hub slot; only `event_signal_dyn` joins that
    /// name to the signal the control actually **emits**. A wrong arm is worse than no arm, because
    /// it reports a wire as live and never fires it, so each field is named explicitly here rather
    /// than derived from the published name.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "changed" => Some(EventSignalRef::unit("changed", self.changed_signal())),
            _ => None,
        }
    }
}

/// `Meter`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` / `access_write_other.in.rs` dispatch.
impl WidgetProperties for Meter {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "value" => Ok(CapabilityValue::UInt(self.value() as u64)),
            "minimum" => Ok(CapabilityValue::UInt(self.minimum() as u64)),
            "maximum" => Ok(CapabilityValue::UInt(self.maximum() as u64)),
            "tick_count" => Ok(CapabilityValue::UInt(self.tick_count() as u64)),
            "show_tick_labels" => Ok(CapabilityValue::Bool(self.show_tick_labels())),
            "unit" => Ok(CapabilityValue::String(self.unit().to_string())),
            "value_text" => Ok(CapabilityValue::String(self.value_text())),
            // The bands are a sequence, so the count is the readable scalar and the
            // list itself is written through `add_threshold` / `clear_thresholds`
            // rather than through a value the property layer would have to encode.
            "threshold_count" => Ok(CapabilityValue::UInt(self.thresholds().len() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "value" => {
                self.set_value(expect_u32(value)?);
                Ok(())
            }
            "minimum" => {
                self.set_minimum(expect_u32(value)?);
                Ok(())
            }
            "maximum" => {
                self.set_maximum(expect_u32(value)?);
                Ok(())
            }
            "tick_count" => {
                self.set_tick_count(expect_u32(value)?);
                Ok(())
            }
            "show_tick_labels" => {
                self.set_show_tick_labels(expect_bool(value)?);
                Ok(())
            }
            "unit" => {
                let unit = expect_string(value)?;
                self.set_unit(&unit);
                Ok(())
            }
            // Derived from the value and the unit.
            "value_text" | "threshold_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Mirrors `METER_PROPERTIES`.
        property_names_of![
            "value",
            "minimum",
            "maximum",
            "tick_count",
            "show_tick_labels",
            "unit",
            "value_text",
            "threshold_count",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `meter` publishes.
    ///
    /// `set_value` assigns the reading and `set_range` the bounds; both need an
    /// argument a command carries none of, so both are refused as
    /// [`CapabilityAccessError::OutOfRange`] — use `set("value", ..)` /
    /// `set("minimum", ..)` / `set("maximum", ..)` — rather than reported unknown.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "set_value" | "set_range" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for Meter {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.interactive || !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button, .. }
                if *button == crate::event::mouse_button::PRIMARY =>
            {
                if !self.geometry().contains(*pos) {
                    return;
                }
                self.dragging = true;
                if let Some(value) = self.value_at_point(*pos) {
                    self.set_value(value);
                }
            }
            Event::MouseMove { pos } if self.dragging => {
                if let Some(value) = self.value_at_point(*pos) {
                    self.set_value(value);
                }
            }
            // Every press is expected to be followed by a release, but a release
            // outside the control is not delivered. Ending the drag on leave as
            // well keeps the next hover from moving the needle with no button held.
            Event::MouseRelease { .. } | Event::MouseLeave { .. } => self.dragging = false,
            Event::Wheel { delta, modifiers } => {
                // One notch is one tick. A negative vertical delta is the wheel
                // turning away, so it raises the reading.
                let scroll = WheelScroll::from_event(*delta, *modifiers, 1);
                if scroll.lines_y == 0 {
                    return;
                }
                let step = self.wheel_step() as i64;
                let next = (self.value as i64 - scroll.lines_y as i64 * step)
                    .clamp(self.min as i64, self.max as i64) as u32;
                self.set_value(next);
            }
            _ => {}
        }
    }
}

impl Draw for Meter {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }

        // Prepare the faces and text caches before the resolved style is borrowed: the
        // preparation mutates this control, and every borrow below is immutable.
        let value_font = self.ensure_value_font();
        let tick_label_font = self.ensure_tick_font();
        self.ensure_tick_labels(context, &tick_label_font);
        self.ensure_value_text(context, &value_font);

        // The arc's fitted radius and centre come from `gauge_geometry`, which
        // reserves the band the reading needs below the pivot. Keeping the fit in
        // one place also means a drag reads the geometry the gauge was drawn with.

        // Arc angles: 270° sweep starting from 135° (top-right quadrant).
        // The offset of -90° converts from "0 = top" to "0 = 3 o'clock".
        //
        // The 270° sweep is stroked as a chord chain of fixed angular step, and the
        // strokes land on the same step grid: a band's ends are snapped to it, and the
        // value arc is drawn with the same step and phase. Two chains that share the grid
        // produce *identical* chords wherever their ranges overlap, so the value arc
        // covers a band fully instead of leaving a sub-pixel sliver of the band's colour
        // along a chord that was drawn at a slightly different angle.
        let arc_start_deg = 135.0_f32;
        let arc_sweep_deg = 270.0_f32;
        let offset = -90.0_f32;
        let start_angle = deg_to_rad(arc_start_deg + offset);
        let value_angle =
            deg_to_rad(arc_start_deg + arc_sweep_deg * self.filled_fraction() + offset);

        // The arc's fitted radius and centre, and the tick label's radius, all come
        // from one derivation so the paint path and the input mapping agree.
        let Some((center, radius)) = self.gauge_geometry() else {
            return;
        };
        // The tick labels sit inside the ring, so the radius they are placed at is derived
        // from what a label actually measures rather than from one more magic subtraction.
        // `tick_inner.saturating_sub(10)` rarely left a radius and was `.max(1)`, which let
        // the fit test below drop every label — `meter.svg` drew the arc and the needle with
        // no scale on them at all.
        let tick_outer = radius.saturating_sub(TICK_MARK_LENGTH).max(1);
        let label_half = self.tick_label_cache.as_ref().map(|cache| cache.label_half).unwrap_or(1);
        let label_radius =
            radius.saturating_sub(TICK_MARK_LENGTH).saturating_sub(label_half).max(1);

        // Resolve colors from style.
        //
        // The track is the well the value arc travels in and the value arc is progress
        // through it, so they are two different marks and cannot share one source. Both
        // used to read `style.background_color`, which made them identical whenever a style
        // set a background — a gauge whose filled and unfilled halves are the same colour
        // reads as no progress at all.
        //
        // The needle, the pivot and the tick marks are the control's *value indicator*, not
        // text: they are drawn over the ring to point at a reading, which is the same job
        // the arc's fill does. They therefore take the accent the role classification
        // already assigns this control (`WidgetRole::Accent`), falling back to the theme's
        // accent token and then to the text colour for a build with no theme at all. Using
        // `text_color` for them was why the census read this control as theme-blind with
        // the default style: the resolved text colour is black in *both* appearances, and
        // the many needle-and-tick pixels then outnumbered the one accent-coloured ring.
        //
        // The threshold bands are deliberately *not* themed: a caller assigns each band its
        // own colour to encode a range, so those are data, not chrome.
        let style = self.style();
        let theme = crate::style::resolved_theme_style_shared("meter");
        // Resolved before the sweep buffers are borrowed mutably: `role_font` reads `style`, and
        // the font is needed again after the arcs are drawn.
        let value_font = self
            .value_font_cache
            .as_ref()
            .map(|(resolved, _)| resolved.clone())
            .unwrap_or(value_font);
        // The accent the gauge's value arc is drawn in. It is **not** read from
        // `style.background_color`: this control classifies as `WidgetRole::Accent`, so that field
        // carries the accent the role assigned it — but a role's fill is also what a *container*
        // would carry, and `Surface` now resolves to `surface_container` rather than to the window
        // fill. Reading the role's fill here would therefore make the arc's colour depend on which
        // role the surface table happens to give a meter, which is not a fact about a meter. The
        // semantic token is the direct answer, and the role fill is the fallback for a theme that
        // sets none.
        let accent = crate::style::semantic_color(crate::style::SemanticColor::Info)
            .or(style.background_color)
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::rgb(0, 120, 215));
        let track_color = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .map(|resolved| resolved.blend(&Color::WHITE, 0.55))
            .unwrap_or(Color::rgb(230, 230, 230));
        let value_arc_color = accent;
        // The needle, pivot and ticks all have to stand out from the backdrop the meter draws on,
        // so one derivation serves all three. `text_color` is not usable here: the theme resolves
        // it to black in both appearances, so an indicator drawn in it is identical in light and
        // dark — which is precisely the theme-blindness this control was reported for. The accent
        // is the token the `Accent` role already gives this control, and pushing it clear of the
        // surface keeps the indicator readable without assuming which way "clear" lies.
        //
        // The meter paints no panel of its own — the arcs sit on the control's backdrop — so the
        // surface to measure against is the window fill.
        let meter_surface = {
            let manager = crate::style::theme_manager();
            manager
                .current_theme()
                .map(|active| active.colors.background)
                .unwrap_or(Color::rgb(240, 240, 240))
        };
        let needle_color = Meter::needle_color_on(&accent, &meter_surface);
        let tick_color = Meter::tick_color_on(&accent, &meter_surface);

        // Draw the background track arc (270° sweep, light gray).
        Self::draw_gauge_arc(
            &mut self.sweep_points,
            context,
            center,
            radius,
            arc_start_deg,
            arc_sweep_deg,
            offset,
            track_color,
        );

        // Threshold bands, drawn over the track and under the value arc.
        //
        // Order matters: the track is the whole arc, a band highlights part of it,
        // and the value arc shows progress on top. Drawing the bands first means a
        // band never hides how far the needle's arc has reached.
        for band in &self.thresholds {
            let band_start = Self::snap_to_grid(
                deg_to_rad(arc_start_deg + arc_sweep_deg * self.normalized(band.from) + offset),
                start_angle,
            );
            let band_end = Self::snap_to_grid(
                deg_to_rad(arc_start_deg + arc_sweep_deg * self.normalized(band.to) + offset),
                start_angle,
            );
            // A band whose ends land on the same vertex is not a range, so there is
            // nothing to draw between them.
            if (band_end - band_start).abs() < 0.001 {
                continue;
            }
            // Each band is a sub-range of the same sweep, so it is stroked by the same
            // polyline builder and crosses the sweep's cardinal points without
            // special-casing. Its ends are snapped onto the shared vertex grid first, so
            // the value arc drawn over it later passes through the same points and covers
            // it exactly instead of leaving a hairline of the band's colour along a vertex
            // drawn at a marginally different angle.
            Self::drag_sweep(
                &mut self.sweep_points,
                context,
                center,
                radius,
                band_start,
                band_end,
                band.color,
            );
        }

        // Draw the value arc (colored arc from start to value position).
        //
        // Its angle range is clipped to the value rather than its radius re-fitted, which is
        // what keeps it concentric with the track it is drawn over; its end is snapped onto the
        // same grid for the reason at the bands above.
        let value_end = Self::snap_to_grid(value_angle, start_angle);
        if self.value > self.min && (value_end - start_angle).abs() > 0.001 {
            Self::drag_sweep(
                &mut self.sweep_points,
                context,
                center,
                radius,
                start_angle,
                value_end,
                value_arc_color,
            );
        }

        // Draw tick marks at regular intervals along the arc.
        //
        // A tick's angle comes from the *same* mapping the arcs use —
        // `deg_to_rad(angle + offset)` — rather than its own. The old
        // `tick_angle_deg = arc_start_deg + tick_step * i` omitted the `offset`, so the
        // ticks were 90° out of phase with the track and the value arc they annotate:
        // the arc's first vertex is at (71,71) in `meter.svg` while tick 0 started from
        // 135°, which is the top-left quadrant rather than the sweep's own start. Sharing
        // the mapping is also what makes the tick's label land on the same ray as the tick
        // it names.
        if self.tick_count >= 2 {
            let tick_step = arc_sweep_deg / (self.tick_count - 1) as f32;

            for i in 0..self.tick_count {
                let tick_angle_deg = arc_start_deg + tick_step * i as f32 + offset;
                // Both ends are placed from this one snapped angle, so a tick is a radial
                // segment rather than two independently-placed points. Rounding each end
                // from its own distance let truncation shorten a 45° tick to 5 px while a
                // 90° one stayed 6 px — on a symmetric dial that reads as "one tick is
                // short". Deriving the ends from the same centre and the same direction
                // (and rounding the radius once, with `round()` rather than truncation)
                // keeps every tick the same length in every direction.
                let tick_rad = Self::snap_to_grid(deg_to_rad(tick_angle_deg), start_angle);
                let dir_x = tick_rad.cos();
                let dir_y = tick_rad.sin();

                let outer_x = center.x + (tick_outer as f32 * dir_x).round() as i32;
                let outer_y = center.y + (tick_outer as f32 * dir_y).round() as i32;
                let inner_radius = tick_outer.saturating_sub(TICK_MARK_LENGTH).max(1);
                let inner_x = center.x + (inner_radius as f32 * dir_x).round() as i32;
                let inner_y = center.y + (inner_radius as f32 * dir_y).round() as i32;

                context.draw_line_stroke(
                    Point::new(inner_x, inner_y),
                    Point::new(outer_x, outer_y),
                    tick_color,
                    1,
                );

                if self.show_tick_labels {
                    // The tick's value is its position along the arc, so the label
                    // is derived from the same fraction the tick was placed at
                    // rather than from an independent step that could disagree.
                    // Both the text and its measurement come from the cache built at
                    // the top of this draw, so a still gauge does no string work.
                    let Some((tick_text, tick_width, tick_height)) = self
                        .tick_label_cache
                        .as_ref()
                        .and_then(|cache| cache.labels.get(i as usize))
                    else {
                        continue;
                    };
                    let lx = center.x + (label_radius as f32 * dir_x).round() as i32;
                    let ly = center.y + (label_radius as f32 * dir_y).round() as i32;
                    // Centred on the point the tick sits at: the glyph origin is the box's
                    // top edge, so that is `ly - height/2` — the old `ly + ascent/2` left the
                    // label half a line below its own tick.
                    //
                    // The label is drawn **only when its whole box lies inside the control**.
                    // A label placed on the ring's own radius has its ends outside the vertical
                    // band the arc occupies — the sweep's endpoints sit at the top and bottom
                    // of the circle — so an unclamped label would be painted past the edge by a
                    // backend that clips nothing. The old code had no such test and its label
                    // radius collapsed to 1, so in practice the whole scale simply never
                    // appeared; this restores it while keeping every drawn label inside.
                    let label_x = lx - *tick_width as i32 / 2;
                    let label_y = ly - *tick_height as i32 / 2;
                    let fits = label_x >= rect.x
                        && label_y >= rect.y
                        && label_x + *tick_width as i32 <= rect.x + rect.width as i32
                        && label_y + *tick_height as i32 <= rect.y + rect.height as i32;
                    if fits {
                        context.draw_text(
                            Point::new(label_x, label_y),
                            tick_text,
                            &tick_label_font,
                            tick_color,
                            HorizontalAlignment::Left,
                        );
                    }
                }
            }
        }

        // Draw the needle line from center outward to the value angle.
        //
        // The pivot disc below reaches 4 px in every direction, so the spoke stops one
        // px short of the tracked ring and the disc's edge stays inside it.
        let needle_length = radius.saturating_sub(5).max(1);
        let needle_x = center.x + (needle_length as f32 * value_angle.cos()) as i32;
        let needle_y = center.y + (needle_length as f32 * value_angle.sin()) as i32;
        context.draw_line_stroke(center, Point::new(needle_x, needle_y), needle_color, 2);

        // Draw a small filled circle at the center as the needle pivot.
        context.fill_circle(center, 4, needle_color);

        // The reading itself, below the pivot, so the needle can be read exactly
        // rather than only approximately.
        //
        // Only drawn when the band reserved for it — by the same helper the arc's fit
        // uses — lies wholly inside the control. A reading whose glyph box runs past the
        // bottom edge is precisely the defect a raster backend hides and an SVG one
        // shows, so "it fits" is answered against the rectangle rather than the arc.
        // The same spelling `value_text()` produces, reused from the cache prepared at
        // the top of this draw: the property getter keeps its own `format!` for the
        // contract, but the paint path must not allocate the reading every frame.
        if let Some(cache) = self.value_text_cache.as_ref() {
            let reading_reserve = Meter::reading_band();
            let band_top = rect.y + rect.height as i32 - reading_reserve;
            let value_y = band_top + (reading_reserve - cache.height as i32) / 2;
            if value_y >= rect.y && value_y + cache.height as i32 <= rect.y + rect.height as i32 {
                context.draw_text(
                    Point::new(center.x - cache.width as i32 / 2, value_y),
                    cache.text.as_str(),
                    &value_font,
                    needle_color,
                    HorizontalAlignment::Left,
                );
            }
        }
    }
}

impl Meter {
    /// The reading's face, re-derived only when the effective style font changes.
    fn ensure_value_font(&mut self) -> Font {
        let source = crate::widget::metrics::effective_font(self.style()).clone();
        if let Some((resolved, key)) = self.value_font_cache.as_ref() {
            if *key == source {
                return resolved.clone();
            }
        }
        let resolved = role_font(self.style(), 12.0);
        self.value_font_cache = Some((resolved.clone(), source));
        resolved
    }

    /// The fixed tick-label face, constructed once.
    fn ensure_tick_font(&mut self) -> Font {
        if self.tick_font_cache.is_none() {
            self.tick_font_cache = Some(Font::simple("Sans", 9.0));
        }
        self.tick_font_cache.clone().unwrap_or_default()
    }

    /// Rebuilds the tick labels when the range, count, unit or device scale changed.
    fn ensure_tick_labels(&mut self, context: &RenderContext, font: &Font) {
        let dpi_scale_bits = context.dpi_scale().to_bits();
        let fresh = self.tick_label_cache.as_ref().is_some_and(|cache| {
            cache.minimum == self.min
                && cache.maximum == self.max
                && cache.tick_count == self.tick_count
                && cache.unit == self.unit
                && cache.dpi_scale_bits == dpi_scale_bits
        });
        if fresh {
            return;
        }
        let label_half = context.measure_text("000", font).width.max(1) / 2;
        let mut labels = Vec::with_capacity(self.tick_count as usize);
        for i in 0..self.tick_count {
            let fraction =
                if self.tick_count > 1 { i as f32 / (self.tick_count - 1) as f32 } else { 0.0 };
            let text = format!("{}{}", self.value_at_fraction(fraction), self.unit);
            let metrics = context.measure_text(&text, font);
            labels.push((text, metrics.width, metrics.height));
        }
        self.tick_label_cache = Some(TickLabelCache {
            minimum: self.min,
            maximum: self.max,
            tick_count: self.tick_count,
            unit: self.unit.clone(),
            dpi_scale_bits,
            label_half,
            labels,
        });
    }

    /// Rebuilds the foot reading when the value, unit or device scale changed.
    fn ensure_value_text(&mut self, context: &RenderContext, font: &Font) {
        let dpi_scale_bits = context.dpi_scale().to_bits();
        let fresh = self.value_text_cache.as_ref().is_some_and(|cache| {
            cache.value == self.value
                && cache.unit == self.unit
                && cache.dpi_scale_bits == dpi_scale_bits
        });
        if fresh {
            return;
        }
        let text = format!("{}{}", self.value, self.unit);
        let metrics = context.measure_text(&text, font);
        self.value_text_cache = Some(ValueTextCache {
            value: self.value,
            unit: self.unit.clone(),
            dpi_scale_bits,
            text,
            width: metrics.width,
            height: metrics.height,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Color, Rect, Size};
    use crate::render::{PaintBackend, RenderContext, SoftwarePaintBackend};

    #[test]
    fn meter_creation() {
        let meter = Meter::new(Rect::new(0, 0, 200, 200));
        assert_eq!(meter.value(), 0);
        assert_eq!(meter.min, 0);
        assert_eq!(meter.max, 100);
        assert_eq!(meter.tick_count, 5);
        assert_eq!(meter.kind(), WidgetKind::Meter);
    }

    #[test]
    fn meter_set_value() {
        let mut meter = Meter::new(Rect::new(0, 0, 200, 200));
        meter.set_value(50);
        assert_eq!(meter.value(), 50);

        // Above max should clamp to 100.
        meter.set_value(200);
        assert_eq!(meter.value(), 100);

        // Below min should clamp to 0.
        meter.set_value(0);
        assert_eq!(meter.value(), 0);
    }

    #[test]
    fn meter_set_range() {
        let mut meter = Meter::new(Rect::new(0, 0, 200, 200));
        meter.set_range(10, 50);
        assert_eq!(meter.min, 10);
        assert_eq!(meter.max, 50);

        // Value should be re-clamped to the new range.
        meter.set_value(30);
        assert_eq!(meter.value(), 30);

        // Value below new min clamps up.
        meter.set_value(5);
        assert_eq!(meter.value(), 10);

        // Value above new max clamps down.
        meter.set_value(60);
        assert_eq!(meter.value(), 50);
    }

    #[test]
    fn meter_draw_no_panic() {
        let mut meter = Meter::new(Rect::new(0, 0, 200, 200));
        meter.set_value(65);

        let mut backend = SoftwarePaintBackend::new(Size::new(200, 200), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut context = RenderContext::new(&mut backend);
        meter.draw(&mut context);
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert!(!rgba.is_empty());
    }

    #[test]
    fn meter_draw_zero_value_no_panic() {
        let mut meter = Meter::new(Rect::new(0, 0, 200, 200));
        meter.set_value(0);

        let mut backend = SoftwarePaintBackend::new(Size::new(200, 200), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut context = RenderContext::new(&mut backend);
        meter.draw(&mut context);
        backend.end_frame();
    }

    #[test]
    fn meter_draw_zero_geometry_no_panic() {
        let mut meter = Meter::new(Rect::new(0, 0, 0, 0));
        let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut context = RenderContext::new(&mut backend);
        meter.draw(&mut context);
        backend.end_frame();
    }

    #[test]
    fn meter_set_tick_count() {
        let mut meter = Meter::new(Rect::new(0, 0, 200, 200));
        assert_eq!(meter.tick_count, 5);

        meter.set_tick_count(10);
        assert_eq!(meter.tick_count, 10);

        // Minimum is 2.
        meter.set_tick_count(0);
        assert_eq!(meter.tick_count, 2);
    }

    #[test]
    fn meter_normalized_value() {
        let meter = Meter::new(Rect::new(0, 0, 200, 200));
        assert!((meter.normalized_value() - 0.0).abs() < f32::EPSILON);

        let mut meter = Meter::new(Rect::new(0, 0, 200, 200));
        meter.set_value(50);
        assert!((meter.normalized_value() - 0.5).abs() < f32::EPSILON);

        meter.set_value(100);
        assert!((meter.normalized_value() - 1.0).abs() < f32::EPSILON);

        // Empty range returns 0.
        meter.set_range(50, 50);
        assert!((meter.normalized_value() - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn meter_size_hint() {
        let meter = Meter::new(Rect::new(0, 0, 100, 100));
        assert_eq!(meter.size_hint(), Size::new(200, 200));
    }

    #[test]
    fn meter_geometry_delegation() {
        let mut meter = Meter::new(Rect::new(0, 0, 200, 200));
        meter.set_geometry(Rect::new(10, 10, 150, 150));
        assert_eq!(meter.geometry(), Rect::new(10, 10, 150, 150));
    }

    #[test]
    fn meter_event_delegation() {
        let mut meter = Meter::new(Rect::new(0, 0, 200, 200));
        // Handle event without panicking.
        meter.handle_event(&Event::KeyDown((37, 0)));
    }

    /// Renders the meter and returns the RGBA frame.
    fn render(meter: &mut Meter, size: Size) -> Vec<u8> {
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);
        let mut context = RenderContext::new(&mut backend);
        meter.draw(&mut context);
        backend.end_frame();
        backend.frame_rgba().to_vec()
    }

    /// Counts pixels matching `target` within a small tolerance.
    ///
    /// Anti-aliasing blends arc edges into the background, so an exact-equality
    /// count finds nothing; a tolerance band finds the drawn segment without
    /// mistaking a near-colour for it.
    fn count_near(rgba: &[u8], target: (u8, u8, u8)) -> usize {
        const TOLERANCE: i32 = 24;
        rgba.as_chunks::<4>()
            .0
            .iter()
            .filter(|px| {
                let dr = (px[0] as i32 - target.0 as i32).abs();
                let dg = (px[1] as i32 - target.1 as i32).abs();
                let db = (px[2] as i32 - target.2 as i32).abs();
                dr <= TOLERANCE && dg <= TOLERANCE && db <= TOLERANCE && px[3] > 0
            })
            .count()
    }

    // ── B3-1: threshold bands (pixel assertion) ─────────────────────────────

    #[test]
    fn meter_threshold_band_paints_its_colour_on_the_arc() {
        let size = Size::new(200, 200);
        // A distinct colour so it cannot be confused with the track (230,230,230),
        // the value arc (0,120,215) or the needle (60,60,60).
        let band = Color::rgb(255, 0, 0);

        let mut without = Meter::new(Rect::new(0, 0, 200, 200));
        without.set_tick_count(2);
        let baseline = render(&mut without, size);
        assert_eq!(count_near(&baseline, (255, 0, 0)), 0, "nothing red before the band");

        let mut with = Meter::new(Rect::new(0, 0, 200, 200));
        with.set_tick_count(2);
        with.add_threshold_range(70, 100, band);
        let painted = render(&mut with, size);
        assert!(
            count_near(&painted, (255, 0, 0)) > 0,
            "the band's arc segment must be painted in the band's colour"
        );
    }

    #[test]
    fn meter_threshold_bounds_are_clamped_to_the_range() {
        let mut meter = Meter::new(Rect::new(0, 0, 200, 200));
        meter.set_range(0, 100);
        // A band extending past both ends keeps only its overlap.
        meter.add_threshold_range(0, 500, Color::rgb(255, 0, 0));
        assert_eq!(meter.thresholds().len(), 1);
        assert_eq!(meter.thresholds()[0].from, 0);
        assert_eq!(meter.thresholds()[0].to, 100);

        // A band entirely outside the range has no overlap and is dropped rather
        // than stored as an arc that would draw nothing.
        meter.add_threshold_range(200, 300, Color::rgb(0, 255, 0));
        assert_eq!(meter.thresholds().len(), 1);

        // An inverted pair is read as the interval between them, not as empty.
        meter.add_threshold_range(60, 40, Color::rgb(0, 0, 255));
        assert_eq!(meter.thresholds().len(), 2);
        assert_eq!(meter.thresholds()[1].from, 40);
        assert_eq!(meter.thresholds()[1].to, 60);
    }

    #[test]
    fn meter_clear_thresholds_empties_the_bands() {
        let mut meter = Meter::new(Rect::new(0, 0, 200, 200));
        meter.add_threshold_range(0, 50, Color::rgb(255, 0, 0));
        meter.add_threshold_range(50, 100, Color::rgb(0, 255, 0));
        assert_eq!(meter.get("threshold_count").unwrap(), CapabilityValue::UInt(2));
        meter.clear_thresholds();
        assert!(meter.thresholds().is_empty());
        assert_eq!(meter.get("threshold_count").unwrap(), CapabilityValue::UInt(0));
    }

    // ── B3-2: tick labels (pixel assertion) ────────────────────────────────

    #[test]
    fn meter_tick_labels_add_text_pixels() {
        // Holds the crate-wide theme guard: this test **derives a colour from the active theme** and
        // then renders, so a concurrent test switching the appearance would change `tick_rgb` between
        // the two renders and the label pixels would match neither. It failed roughly one run in
        // sixteen before this line existed, which is the same class of defect the guard was added for
        // across the rendering tests -- one of them was simply missed.
        let _guard = crate::style::theme_test_guard();
        let size = Size::new(200, 200);
        let mut without = Meter::new(Rect::new(0, 0, 200, 200));
        without.set_tick_count(5);
        without.set_show_tick_labels(false);
        let bare = render(&mut without, size);

        let mut with = Meter::new(Rect::new(0, 0, 200, 200));
        with.set_tick_count(5);
        with.set_show_tick_labels(true);
        let labelled = render(&mut with, size);

        // Labels are drawn in the control's tick colour, which is the accent darkened — a
        // themed value, so the expectation is derived from the same helper the draw uses
        // rather than from a literal that would silently stop matching. With labels off,
        // that colour appears only on the short tick strokes, so switching them on must add
        // a substantial number of such pixels.
        let tick_rgb = {
            let accent = crate::style::semantic_color(crate::style::SemanticColor::Info)
                .unwrap_or(Color::rgb(0, 120, 215));
            let surface = crate::style::theme_manager()
                .current_theme()
                .map(|active| active.colors.background)
                .unwrap_or(Color::rgb(240, 240, 240));
            let color = Meter::tick_color_on(&accent, &surface);
            (color.r, color.g, color.b)
        };
        let bare_ticks = count_near(&bare, tick_rgb);
        let labelled_ticks = count_near(&labelled, tick_rgb);
        assert!(
            labelled_ticks > bare_ticks,
            "tick labels must paint text: {bare_ticks} -> {labelled_ticks}"
        );
    }

    #[test]
    fn meter_tick_labels_track_the_range() {
        let mut meter = Meter::new(Rect::new(0, 0, 200, 200));
        meter.set_range(0, 100);
        // Ticks are at even fractions of the range, so the ends are the range's
        // ends and the middle is its midpoint.
        assert_eq!(meter.value_at_fraction(0.0), 0);
        assert_eq!(meter.value_at_fraction(0.5), 50);
        assert_eq!(meter.value_at_fraction(1.0), 100);

        meter.set_range(50, 150);
        assert_eq!(meter.value_at_fraction(0.0), 50);
        assert_eq!(meter.value_at_fraction(1.0), 150);
    }

    // ── B3-3: unit suffix ──────────────────────────────────────────────────

    #[test]
    fn meter_value_text_appends_the_unit() {
        let mut meter = Meter::new(Rect::new(0, 0, 200, 200));
        assert_eq!(meter.value_text(), "0", "no unit means no suffix");

        meter.set_value(72);
        meter.set_unit("°C");
        assert_eq!(meter.value_text(), "72°C");
        assert_eq!(meter.unit(), "°C");

        meter.set_unit("");
        assert_eq!(meter.value_text(), "72");
    }

    // ── B3-4: property contract (rule #82) ─────────────────────────────────

    #[test]
    fn meter_new_properties_round_trip() {
        let mut meter = Meter::new(Rect::new(0, 0, 200, 200));

        meter.set("tick_count", CapabilityValue::UInt(9)).unwrap();
        assert_eq!(meter.tick_count(), 9);
        assert_eq!(meter.get("tick_count").unwrap(), CapabilityValue::UInt(9));

        meter.set("show_tick_labels", CapabilityValue::Bool(true)).unwrap();
        assert!(meter.show_tick_labels());
        assert_eq!(meter.get("show_tick_labels").unwrap(), CapabilityValue::Bool(true));

        meter.set("unit", CapabilityValue::String("kPa".to_string())).unwrap();
        assert_eq!(meter.unit(), "kPa");
        assert_eq!(meter.get("unit").unwrap(), CapabilityValue::String("kPa".to_string()));

        // Wrong types are rejected rather than coerced.
        assert!(meter.set("tick_count", CapabilityValue::Bool(true)).is_err());
        assert!(meter.set("unit", CapabilityValue::UInt(1)).is_err());
    }

    #[test]
    fn meter_derived_properties_are_read_only() {
        let mut meter = Meter::new(Rect::new(0, 0, 200, 200));
        meter.set_value(42);
        meter.add_threshold_range(0, 50, Color::rgb(255, 0, 0));
        assert_eq!(meter.get("value_text").unwrap(), CapabilityValue::String("42".to_string()));
        assert_eq!(meter.get("threshold_count").unwrap(), CapabilityValue::UInt(1));
        assert!(meter.set("value_text", CapabilityValue::String("x".into())).is_err());
        assert!(meter.set("threshold_count", CapabilityValue::UInt(3)).is_err());
    }

    #[test]
    fn meter_tick_count_floor_is_two() {
        let mut meter = Meter::new(Rect::new(0, 0, 200, 200));
        meter.set_tick_count(0);
        // One tick is not a scale: a 270-degree arc needs both ends marked.
        assert_eq!(meter.tick_count(), 2);
    }

    #[test]
    fn meter_draws_with_thresholds_and_labels_without_panicking() {
        // # Why this test takes the theme guard
        //
        // It renders the meter and then looks for the **needle's** colour in the pixels, and the needle
        // colour is derived from the process-wide active theme (`needle_color_on(&accent, &meter_surface)`
        // inside `draw`). A test that switches the theme concurrently can therefore land between this
        // test's own read and the render — the assertion then looks for one theme's needle in another
        // theme's picture, and fails. Measured: 2 failures in 6 full-suite runs before the guard, 0 in 15
        // after. The guard is the crate's existing answer to exactly this (see its own doc), and taking it
        // for a *read* of shared mutable state is the same requirement as taking it for a write.
        //
        // Gated on `device_profile` because that is what compiles the theme registry at all: the stripped
        // profiles have `theme_manager()` as a placeholder, so there is nothing to serialise against and
        // no guard to take.
        #[cfg(device_profile)]
        let _guard = crate::style::theme_test_guard();
        let mut meter = Meter::new(Rect::new(0, 0, 200, 200));
        meter.set_value(72);
        meter.set_unit("°C");
        meter.set_show_tick_labels(true);
        meter.add_threshold_range(0, 40, Color::rgb(15, 157, 88));
        meter.add_threshold_range(40, 70, Color::rgb(244, 180, 0));
        meter.add_threshold_range(70, 100, Color::rgb(219, 68, 55));
        let rgba = render(&mut meter, Size::new(200, 200));
        assert!(!rgba.is_empty());
        // The red band is *above* the value, so the value arc (drawn last) has not
        // covered it and it must be visible.
        assert!(
            count_near(&rgba, (219, 68, 55)) > 0,
            "a band ahead of the needle is not covered by the value arc"
        );
        // The green band is entirely *behind* the needle at 72, so the value arc
        // covers it. Its absence is the correct reading, and asserting it pins the
        // draw order rather than leaving it incidental.
        assert_eq!(
            count_near(&rgba, (15, 157, 88)),
            0,
            "a band behind the needle is covered by the value arc"
        );
        // The reading itself is painted, in the same themed gauge colour the needle uses.
        let needle = Meter::needle_color_on(
            &crate::style::semantic_color(crate::style::SemanticColor::Info)
                .unwrap_or(Color::rgb(0, 120, 215)),
            &crate::style::theme_manager()
                .current_theme()
                .map(|active| active.colors.background)
                .unwrap_or(Color::rgb(240, 240, 240)),
        );
        assert!(count_near(&rgba, (needle.r, needle.g, needle.b)) > 0);
    }

    #[test]
    fn meter_tiny_geometry_does_not_panic_with_labels() {
        let mut meter = Meter::new(Rect::new(0, 0, 12, 12));
        meter.set_show_tick_labels(true);
        meter.add_threshold_range(0, 100, Color::rgb(255, 0, 0));
        let rgba = render(&mut meter, Size::new(12, 12));
        assert!(!rgba.is_empty());
    }

    /// An interactive meter writes drags and wheel notches through
    /// `set_value`, so the widget's existing `changed` signal follows them;
    /// a read-only meter ignores both.
    #[test]
    fn meter_interaction_sets_the_value_and_emits_changed() {
        let mut meter = Meter::new(Rect::new(0, 0, 200, 200));
        assert!(!meter.is_interactive(), "a meter is a read-only indicator by default");

        // Read-only: neither a drag nor a wheel notch moves it.
        meter.handle_event(&Event::Wheel { delta: Point::new(0, -1), modifiers: 0 });
        meter.handle_event(&Event::MousePress {
            pos: Point::new(100, 188),
            button: crate::event::mouse_button::PRIMARY,
            modifiers: 0,
        });
        assert_eq!(meter.value(), 0);

        meter.set_interactive(true);
        assert!(meter.is_interactive());
        let changes = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        meter.base.changed.connect({
            let changes = std::sync::Arc::clone(&changes);
            move || {
                changes.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
        });

        // The pivot is the middle of this square control. A press at the
        // sweep's start angle (45° below the pivot) reads one sixth of the way
        // round, and a move to the sweep's beginning reads the maximum.
        meter.handle_event(&Event::MousePress {
            pos: Point::new(100, 188),
            button: crate::event::mouse_button::PRIMARY,
            modifiers: 0,
        });
        assert_eq!(meter.value(), 17, "the press maps its angle onto the sweep");
        meter.handle_event(&Event::MouseMove { pos: Point::new(188, 100) });
        assert_eq!(meter.value(), 100, "the drag keeps tracking the pointer");

        // One notch is one tick (the range split into tick intervals), and a
        // notch away from the reader lowers the value.
        meter.handle_event(&Event::Wheel { delta: Point::new(0, 1), modifiers: 0 });
        assert_eq!(meter.value(), 75, "a notch is one tick of the range");
        meter.handle_event(&Event::Wheel { delta: Point::new(0, -1), modifiers: 0 });
        assert_eq!(meter.value(), 100);

        // Four real changes: the press, the move and the two notches. A
        // clamped write to the value it already holds stays silent.
        assert_eq!(changes.load(std::sync::atomic::Ordering::SeqCst), 4);

        // A release ends the drag: hover motion no longer moves the needle.
        meter.handle_event(&Event::MouseRelease {
            pos: Point::new(188, 100),
            button: crate::event::mouse_button::PRIMARY,
        });
        meter.handle_event(&Event::MouseMove { pos: Point::new(100, 188) });
        assert_eq!(meter.value(), 100, "a released meter ignores hover motion");

        // Disabling interaction cancels a drag in progress.
        meter.set_interactive(false);
        meter.handle_event(&Event::Wheel { delta: Point::new(0, 1), modifiers: 0 });
        assert_eq!(meter.value(), 100);
    }

    /// Discrete mode fills whole tick segments while the needle and reading
    /// keep the exact value.
    #[test]
    fn meter_discrete_fills_whole_tick_segments() {
        let mut meter = Meter::new(Rect::new(0, 0, 200, 200));
        meter.set_value(30);
        assert!(!meter.is_discrete(), "the sweep is continuous by default");
        assert!((meter.filled_fraction() - 0.3).abs() < 1e-6, "continuous arc follows the value");

        // With five ticks the range has four intervals, so 30% falls in the
        // first one and the arc stops at 25%.
        meter.set_discrete(true);
        assert!(meter.is_discrete());
        assert!((meter.filled_fraction() - 0.25).abs() < 1e-6, "the arc snaps down to the tick");
        assert_eq!(meter.value(), 30, "the reading itself is not snapped");

        // Both ends land exactly on the scale.
        meter.set_value(100);
        assert!((meter.filled_fraction() - 1.0).abs() < 1e-6);
        meter.set_value(0);
        assert!((meter.filled_fraction() - 0.0).abs() < 1e-6);

        // Changing the tick count changes the grid the arc snaps to.
        meter.set_tick_count(3);
        meter.set_value(60);
        assert!((meter.filled_fraction() - 0.5).abs() < 1e-6, "two intervals snap 60% down to 50%");

        // The rendered arc changes with the mode, proving the flag reaches the
        // paint path rather than only the model.
        let continuous = {
            meter.set_discrete(false);
            render(&mut meter, Size::new(200, 200))
        };
        let discrete = {
            meter.set_discrete(true);
            render(&mut meter, Size::new(200, 200))
        };
        assert_ne!(continuous, discrete, "discrete mode paints a different arc");
    }

    /// The tick labels, the reading and the faces are cached across frames.
    #[test]
    fn the_tick_labels_and_reading_are_cached() {
        let mut meter = Meter::new(Rect::new(0, 0, 200, 200));
        meter.set_tick_count(5);
        meter.set_unit("°C");
        let mut backend = SoftwarePaintBackend::new(Size::new(200, 200), 1.0);
        backend.begin_frame(Color::WHITE);
        let context = RenderContext::new(&mut backend);
        let tick_font = meter.ensure_tick_font();

        meter.ensure_tick_labels(&context, &tick_font);
        let cache = meter.tick_label_cache.as_ref().expect("labels are built");
        assert_eq!(cache.labels.len(), 5);
        assert_eq!(cache.labels[0].0, "0°C");
        assert_eq!(cache.labels[4].0, "100°C");
        let ptr = cache.labels.as_ptr();

        // Same inputs: the buffer is reused.
        meter.ensure_tick_labels(&context, &tick_font);
        assert_eq!(meter.tick_label_cache.as_ref().unwrap().labels.as_ptr(), ptr);

        // The unit changes: both text and measurements rebuild.
        meter.set_unit("K");
        meter.ensure_tick_labels(&context, &tick_font);
        assert_eq!(meter.tick_label_cache.as_ref().unwrap().labels[0].0, "0K");

        // The reading follows the value and is reused while it does not move.
        let value_font = meter.ensure_value_font();
        meter.set_value(42);
        meter.ensure_value_text(&context, &value_font);
        assert_eq!(meter.value_text_cache.as_ref().unwrap().text, "42K");
        let ptr = meter.value_text_cache.as_ref().unwrap().text.as_ptr();
        meter.ensure_value_text(&context, &value_font);
        assert_eq!(meter.value_text_cache.as_ref().unwrap().text.as_ptr(), ptr);
    }
}
