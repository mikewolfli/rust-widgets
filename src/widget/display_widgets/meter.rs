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
use crate::event::{Event, EventHandler};
use crate::render::{RenderCommand, RenderContext};
use crate::widget::capability::coercion::{expect_bool, expect_string, expect_u32};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
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
    /// Suffix appended to the value in labels, e.g. `°C`.
    unit: String,
}

impl Meter {
    /// Creates a new Meter widget with the given geometry.
    ///
    /// Defaults: value 0, range 0-100, 5 tick marks, no threshold bands, tick
    /// labels off, no unit.
    pub fn new(rect: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Meter, rect, "Meter"),
            value: 0,
            min: 0,
            max: 100,
            tick_count: 5,
            thresholds: Vec::new(),
            show_tick_labels: false,
            unit: String::new(),
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
        self.tick_count = count.max(2);
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
        self.show_tick_labels = show;
        self.base.request_redraw();
    }

    /// Returns whether tick labels are drawn.
    pub fn show_tick_labels(&self) -> bool {
        self.show_tick_labels
    }

    /// Sets the unit suffix shown after the value, e.g. `°C`.
    pub fn set_unit(&mut self, unit: &str) {
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

    /// The value a normalized position represents, for tick labels.
    fn value_at_fraction(&self, fraction: f32) -> u32 {
        let span = (self.max - self.min) as f32;
        (self.min as f32 + span * fraction).round() as u32
    }
}

impl Widget for Meter {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        Size::new(200, 200)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
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
    }
}

impl Draw for Meter {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }

        let center = Point::new(rect.x + rect.width as i32 / 2, rect.y + rect.height as i32 / 2);

        // Radius is half the smaller dimension minus padding.
        let radius = rect.width.min(rect.height).saturating_sub(8) / 2;
        if radius < 10 {
            return;
        }

        // Arc angles: 270° sweep starting from 135° (top-right quadrant).
        // The offset of -90° converts from "0 = top" to "0 = 3 o'clock".
        let arc_start_deg = 135.0_f32;
        let arc_sweep_deg = 270.0_f32;
        let offset = -90.0_f32;

        let start_angle = deg_to_rad(arc_start_deg + offset);
        let end_angle = deg_to_rad(arc_start_deg + arc_sweep_deg + offset);
        let value_angle =
            deg_to_rad(arc_start_deg + arc_sweep_deg * self.normalized_value() + offset);

        // Resolve colors from style.
        let track_color = Color::rgb(230, 230, 230);
        let value_arc_color = self.style().background_color.unwrap_or(Color::rgb(0, 120, 215));
        let needle_color = self.style().text_color.unwrap_or(Color::rgb(60, 60, 60));
        let tick_color = Color::rgb(160, 160, 160);

        // Draw the background track arc (270° sweep, light gray).
        if (end_angle - start_angle).abs() > 0.001 {
            context.execute_command(RenderCommand::DrawArc {
                center,
                radius,
                start_angle,
                end_angle,
                color: track_color,
                filled: false,
            });
        }

        // Threshold bands, drawn over the track and under the value arc.
        //
        // Order matters: the track is the whole arc, a band highlights part of it,
        // and the value arc shows progress on top. Drawing the bands first means a
        // band never hides how far the needle's arc has reached.
        for band in &self.thresholds {
            let band_start =
                deg_to_rad(arc_start_deg + arc_sweep_deg * self.normalized(band.from) + offset);
            let band_end =
                deg_to_rad(arc_start_deg + arc_sweep_deg * self.normalized(band.to) + offset);
            // Half a degree of width for a zero-length band, so a single-value
            // threshold is still visible instead of collapsing to nothing.
            if (band_end - band_start).abs() < 0.001 {
                continue;
            }
            context.execute_command(RenderCommand::DrawArc {
                center,
                radius,
                start_angle: band_start,
                end_angle: band_end,
                color: band.color,
                filled: false,
            });
        }

        // Draw the value arc (colored arc from start to value position).
        if self.value > self.min && (value_angle - start_angle).abs() > 0.001 {
            context.execute_command(RenderCommand::DrawArc {
                center,
                radius,
                start_angle,
                end_angle: value_angle,
                color: value_arc_color,
                filled: false,
            });
        }

        // Draw tick marks at regular intervals along the arc.
        if self.tick_count >= 2 {
            let tick_outer = radius;
            let tick_inner = radius.saturating_sub(6).max(1);
            let tick_step = arc_sweep_deg / (self.tick_count - 1) as f32;
            // Labels go inside the arc so they cannot fall outside the control's
            // rectangle, which a backend would clip away silently.
            let label_radius = tick_inner.saturating_sub(10).max(1) as f32;
            let label_font = Font::simple("Sans", 9.0);

            for i in 0..self.tick_count {
                let tick_angle_deg = arc_start_deg + tick_step * i as f32;
                let tick_rad = deg_to_rad(tick_angle_deg + offset);

                let outer_x = center.x + (tick_outer as f32 * tick_rad.cos()) as i32;
                let outer_y = center.y + (tick_outer as f32 * tick_rad.sin()) as i32;
                let inner_x = center.x + (tick_inner as f32 * tick_rad.cos()) as i32;
                let inner_y = center.y + (tick_inner as f32 * tick_rad.sin()) as i32;

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
                    let fraction = i as f32 / (self.tick_count - 1) as f32;
                    let text = format!("{}{}", self.value_at_fraction(fraction), self.unit);
                    let metrics = context.measure_text(&text, &label_font);
                    let lx = center.x + (label_radius * tick_rad.cos()) as i32;
                    let ly = center.y + (label_radius * tick_rad.sin()) as i32;
                    context.draw_text(
                        Point::new(lx - metrics.width as i32 / 2, ly + metrics.ascent as i32 / 2),
                        &text,
                        &label_font,
                        tick_color,
                        HorizontalAlignment::Left,
                    );
                }
            }
        }

        // Draw the needle line from center outward to the value angle.
        let needle_length = radius.saturating_sub(8).max(1);
        let needle_x = center.x + (needle_length as f32 * value_angle.cos()) as i32;
        let needle_y = center.y + (needle_length as f32 * value_angle.sin()) as i32;
        context.draw_line_stroke(center, Point::new(needle_x, needle_y), needle_color, 2);

        // Draw a small filled circle at the center as the needle pivot.
        context.fill_circle(center, 4, needle_color);

        // The reading itself, below the pivot, so the needle can be read exactly
        // rather than only approximately.
        //
        // Only drawn when it fits: at small geometries the text would overlap the
        // arc, and a clipped reading is worse than none. The `+ 24` reserves the
        // room the text needs vertically.
        let text = self.value_text();
        let value_font = Font::simple("Sans", 12.0);
        let metrics = context.measure_text(&text, &value_font);
        if rect.height as i32 >= radius as i32 + 24 {
            context.draw_text(
                Point::new(
                    center.x - metrics.width as i32 / 2,
                    center.y + (radius as i32 / 2) + metrics.ascent as i32,
                ),
                &text,
                &value_font,
                needle_color,
                HorizontalAlignment::Left,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Color, Rect, Size};
    use crate::render::{PaintBackend, SoftwarePaintBackend};

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
        rgba.chunks_exact(4)
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
        let size = Size::new(200, 200);
        let mut without = Meter::new(Rect::new(0, 0, 200, 200));
        without.set_tick_count(5);
        without.set_show_tick_labels(false);
        let bare = render(&mut without, size);

        let mut with = Meter::new(Rect::new(0, 0, 200, 200));
        with.set_tick_count(5);
        with.set_show_tick_labels(true);
        let labelled = render(&mut with, size);

        // Labels are drawn in the tick colour (160,160,160); with labels off, that
        // colour appears only on the short tick strokes, so switching them on must
        // add a substantial number of such pixels.
        let bare_ticks = count_near(&bare, (160, 160, 160));
        let labelled_ticks = count_near(&labelled, (160, 160, 160));
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
        // The reading itself is painted.
        assert!(count_near(&rgba, (60, 60, 60)) > 0);
    }

    #[test]
    fn meter_tiny_geometry_does_not_panic_with_labels() {
        let mut meter = Meter::new(Rect::new(0, 0, 12, 12));
        meter.set_show_tick_labels(true);
        meter.add_threshold_range(0, 100, Color::rgb(255, 0, 0));
        let rgba = render(&mut meter, Size::new(12, 12));
        assert!(!rgba.is_empty());
    }
}
