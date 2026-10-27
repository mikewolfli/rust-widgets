// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Arc widget — circular progress/indicator (BLUE13 R2.1).
use core::fmt::Write as _;

use crate::compat::{String, ToString, Vec};
use crate::core::{deg_to_rad, Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::{RenderContext, TextMetrics};
use crate::widget::capability::coercion::{
    expect_bool, expect_f64, expect_string, expect_u16_saturating, expect_u32,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::effective_font;
use crate::widget::numeric::ordered_clamp_u32;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// How long one revolution of the indeterminate arc takes, in milliseconds.
///
/// The revolution is a loop rather than a state change, so it is paced from the theme's
/// `slow` token scaled by [`SPIN_SLOW_MULTIPLE`]; `1400` is the value the preset produces
/// and the fallback when no theme is installed.
const DEFAULT_SPIN_PERIOD_MS: u32 = 1400;

/// How many `slow` tokens one revolution is worth.
const SPIN_SLOW_MULTIPLE: u32 = 5;

/// Fills the annulus between `inner` and `outer` over `start..end` as one contour.
///
/// The contour runs along the outer edge and back along the inner edge, so the hole is real
/// geometry rather than an opaque disc painted over the centre: whatever sits behind the
/// control stays visible through it. The outline is sampled from the swept arc length, so a
/// quarter-turn spinner and a full track are both smooth without a fixed vertex count.
fn fill_annulus(
    context: &mut RenderContext,
    center: Point,
    outer: u32,
    inner: u32,
    start: f32,
    end: f32,
    color: Color,
) {
    if outer == 0 || inner >= outer || (end - start).abs() < 0.001 {
        return;
    }
    let sweep = end - start;
    let steps = (((sweep.abs() * outer as f32) / 3.0).ceil() as u32).clamp(8, 256) as usize;
    let mut points = Vec::with_capacity(steps * 2 + 2);
    for index in 0..=steps {
        let t = index as f32 / steps as f32;
        let angle = start + sweep * t;
        points.push(Point::new(
            center.x + (outer as f32 * angle.cos()) as i32,
            center.y + (outer as f32 * angle.sin()) as i32,
        ));
    }
    for index in (0..=steps).rev() {
        let t = index as f32 / steps as f32;
        let angle = start + sweep * t;
        points.push(Point::new(
            center.x + (inner as f32 * angle.cos()) as i32,
            center.y + (inner as f32 * angle.sin()) as i32,
        ));
    }
    let contours = [(0usize, points.len())];
    let _ = context.fill_path_non_zero(&points, &contours, color);
}

/// Arc widget for displaying circular progress or angular values.
pub struct Arc {
    base: BaseWidget,
    /// Current value (0-100 range mapped to arc angle).
    ///
    /// When a signed range has been installed with [`Arc::set_range_signed`], this holds the
    /// **offset** from that range's minimum rather than the absolute reading. The signed
    /// accessors ([`Arc::signed_value`], [`Arc::set_signed_value`]) are the ones that speak in
    /// absolute terms; the legacy unsigned accessors keep their historical clamping rule and
    /// are byte-identical whenever no signed range has ever been set.
    value: u32,
    /// Minimum value.
    min: u32,
    /// Maximum value.
    max: u32,
    /// Start angle in degrees (0 = top, clockwise).
    start_angle: i16,
    /// Total sweep angle in degrees (default 360 for full circle, 270 for gauge).
    sweep_angle: u16,
    /// Arc thickness in pixels.
    thickness: u32,
    /// Whether the arc is rounded at ends.
    rounded: bool,
    /// Whether to show the value as text in the center.
    show_value: bool,
    /// The template the centre reading is rendered from.
    ///
    /// `{percent}` expands to the rounded percentage and `{value}` to the normalised value with
    /// two decimals — the same two placeholders `progress_circle` publishes, so a caller moving
    /// between the two controls does not have to learn a second vocabulary. The default
    /// `"{percent}%"` reproduces the historical integer-percentage reading byte for byte.
    value_format: String,
    /// Whether the arc is in indeterminate (spinning) mode.
    indeterminate: bool,
    /// The indeterminate sweep's rotation, in turns: `0.0` and `1.0` are the same angle.
    ///
    /// Advanced by [`Arc::tick`] rather than read from the wall clock, so two frames of the
    /// same control with the same tick history paint the same picture and a test can assert
    /// that the arc turns without sleeping.
    phase: f32,
    /// Whether the ring is painted as a true arc stroke instead of a filled disc with a
    /// background-coloured hole.
    ///
    /// Defaults to `false`, which is the historical paint order and therefore keeps every
    /// existing snapshot byte-identical.
    true_arc: bool,
    /// Whether a signed range is installed.
    signed: bool,
    /// The installed signed range's lower bound.
    signed_min: i32,
    /// The installed signed range's upper bound.
    signed_max: i32,
    /// Scratch buffer for the centre reading, reused across frames.
    ///
    /// A determinate arc can sit on one value for many frames, so building the label in a
    /// fresh `String` every draw allocated for a string that had not changed.
    value_text: String,
    /// The percentage [`Self::value_text`] currently holds, or `None` before the first draw.
    cached_value_pct: Option<u32>,
    /// `f32::to_bits` of the normalised value the cached reading was rendered from.
    cached_value_bits: u32,
    /// The template the cached reading was rendered from.
    cached_value_format: String,
    /// The resolved font the cached measurement was taken in.
    cached_value_font: Option<Font>,
    /// The device scale the cached measurement was taken at.
    cached_value_scale: f32,
    /// The measured bounds of [`Self::value_text`] under the cached font and scale.
    cached_value_metrics: TextMetrics,
}

impl Arc {
    /// Creates a new arc widget with the given geometry and default settings.
    ///
    /// Default range is 0-100, sweep angle is 360 degrees (full circle),
    /// thickness is 10 pixels, start angle is 0 (top), and the value text
    /// is visible.
    pub fn new(rect: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Arc, rect, "Arc"),
            value: 0,
            min: 0,
            max: 100,
            start_angle: 0,
            sweep_angle: 360,
            thickness: 10,
            rounded: false,
            show_value: true,
            value_format: "{percent}%".to_string(),
            indeterminate: false,
            phase: 0.0,
            true_arc: false,
            signed: false,
            signed_min: 0,
            signed_max: 0,
            value_text: String::new(),
            cached_value_pct: None,
            cached_value_bits: 0.0f32.to_bits(),
            cached_value_format: String::new(),
            cached_value_font: None,
            cached_value_scale: 1.0,
            cached_value_metrics: TextMetrics::default(),
        }
    }

    /// Returns the current value.
    pub fn value(&self) -> u32 {
        self.value
    }

    /// Returns the minimum value of the range.
    pub fn minimum(&self) -> u32 {
        self.min
    }

    /// Returns the maximum value of the range.
    pub fn maximum(&self) -> u32 {
        self.max
    }

    /// Returns the total sweep angle in degrees.
    pub fn sweep_angle(&self) -> u16 {
        self.sweep_angle
    }

    /// Returns the arc's start angle in degrees (`0` is 12 o'clock, clockwise).
    pub fn start_angle(&self) -> f32 {
        f32::from(self.start_angle)
    }

    /// Sets the start angle in degrees (`0` is 12 o'clock, clockwise).
    ///
    /// The field stores whole degrees, so the value is rounded and clamped to the `i16` range it
    /// can hold; a non-finite input is ignored rather than turned into a sentinel angle. This is
    /// the same 12-o'clock origin `crate::widget::display_widgets::ProgressCircle` paints from,
    /// so a gauge arc and a progress ring can start at the same place.
    pub fn set_start_angle(&mut self, angle: f32) {
        if !angle.is_finite() {
            return;
        }
        let rounded = angle.round().clamp(f32::from(i16::MIN), f32::from(i16::MAX)) as i16;
        if self.start_angle == rounded {
            return;
        }
        self.start_angle = rounded;
        self.base.request_redraw();
    }

    /// Returns the arc thickness in pixels.
    pub fn thickness(&self) -> u32 {
        self.thickness
    }

    /// Returns whether the arc is drawn in indeterminate (spinning) mode.
    pub fn is_indeterminate(&self) -> bool {
        self.indeterminate
    }

    /// Sets the value, clamped between min and max.
    ///
    /// Emits the `changed` signal when the value actually changes.
    ///
    /// `changed` is deliberately not gated by `enabled`: it reports a *value* transition
    /// from `set_value`, not a user action. A host that disables a progress indicator and
    /// keeps feeding it data still needs the signal to track the value, and a disabled
    /// `Arc` accepts no pointer input anyway, so there is no user-driven path to silence
    /// here (the round-52 `MiniCanvas` defect was the opposite: a disabled control
    /// responding to presses).
    pub fn set_value(&mut self, value: u32) {
        let clamped = ordered_clamp_u32(value, self.min, self.max);
        if self.value == clamped {
            return;
        }
        self.value = clamped;
        self.base.changed.emit();
        self.base.request_redraw();
    }

    /// Sets the minimum value of the range, re-clamping the current value.
    pub fn set_minimum(&mut self, minimum: u32) {
        self.set_range(minimum, self.max);
    }

    /// Sets the maximum value of the range, re-clamping the current value.
    pub fn set_maximum(&mut self, maximum: u32) {
        self.set_range(self.min, maximum);
    }

    /// Sets both minimum and maximum values in one call.
    ///
    /// The current value is re-clamped to the new range. An unsigned range disables any
    /// signed range installed by [`Arc::set_range_signed`]: the last writer decides which
    /// scale the control speaks in, and the historical unsigned behaviour is the default.
    ///
    /// Like [`Arc::set_value`], `changed` is deliberately not gated by `enabled`.
    pub fn set_range(&mut self, min: u32, max: u32) {
        self.signed = false;
        self.min = min.min(max);
        self.max = max.max(min);
        let clamped = ordered_clamp_u32(self.value, self.min, self.max);
        if self.value != clamped {
            self.value = clamped;
            self.base.changed.emit();
        }
        self.base.request_redraw();
    }

    /// Installs a **signed** value range, allowing negative readings.
    ///
    /// The control stores the reading as an offset from `min`, so a range of
    /// `-40..=40` can be inspected with [`Arc::signed_value`] while the arc fraction — the
    /// part that decides how much ink is painted — remains the same arithmetic.
    ///
    /// # What happens to the legacy accessors
    ///
    /// [`Arc::value`], [`Arc::minimum`] and [`Arc::maximum`] keep their `u32` type and their
    /// historical clamping rule. After this call they describe the *offset scale*: `minimum`
    /// is `0`, `maximum` is the span, and `value` is the offset the handle sits at. A control
    /// that never calls this function is completely unaffected — the legacy path is
    /// byte-identical, including every emitted picture.
    ///
    /// Like [`Arc::set_range`], the bounds are normalised (`min > max` is swapped) and the
    /// current reading is re-clamped into the new range. `changed` is emitted only when that
    /// re-clamp actually moves the reading.
    pub fn set_range_signed(&mut self, min: i32, max: i32) {
        let low = min.min(max);
        let high = max.max(min);
        let current = self.signed_value() as i64;
        let clamped = current.clamp(low as i64, high as i64);
        let span = (high as i64 - low as i64).min(u32::MAX as i64) as u32;
        let offset = (clamped - low as i64).clamp(0, span as i64) as u32;
        if self.signed && self.signed_min == low && self.signed_max == high && self.value == offset
        {
            return;
        }
        self.signed = true;
        self.signed_min = low;
        self.signed_max = high;
        self.min = 0;
        self.max = span;
        if self.value != offset {
            self.value = offset;
            self.base.changed.emit();
        }
        self.base.request_redraw();
    }

    /// Returns the current reading on the signed scale.
    ///
    /// With a signed range installed this is the absolute reading (`minimum` plus the
    /// stored offset). Without one it is the historical unsigned reading widened to `i32`;
    /// a value above `i32::MAX` saturates rather than wrapping.
    pub fn signed_value(&self) -> i32 {
        if self.signed {
            (self.signed_min as i64 + self.value as i64).clamp(i32::MIN as i64, i32::MAX as i64)
                as i32
        } else {
            self.value.min(i32::MAX as u32) as i32
        }
    }

    /// Sets the reading on the signed scale.
    ///
    /// The value is clamped into the installed signed range. With no signed range, it is
    /// clamped into the historical unsigned range (negative input therefore stops at the
    /// minimum). Like [`Arc::set_value`], `changed` fires only on a real transition.
    pub fn set_signed_value(&mut self, value: i32) {
        if !self.signed {
            self.set_value(value.max(0) as u32);
            return;
        }
        let offset = (value as i64 - self.signed_min as i64).max(0) as u32;
        self.set_value(offset);
    }

    /// Sets the total sweep angle in degrees (e.g., 360 for full circle, 270 for gauge).
    pub fn set_sweep_angle(&mut self, angle: u16) {
        if self.sweep_angle == angle {
            return;
        }
        self.sweep_angle = angle;
        self.base.request_redraw();
    }

    /// Sets the arc thickness in pixels.
    pub fn set_thickness(&mut self, thickness: u32) {
        let thickness = thickness.max(1);
        if self.thickness == thickness {
            return;
        }
        self.thickness = thickness;
        self.base.request_redraw();
    }

    /// Returns whether the arc's ends are rounded.
    pub fn rounded(&self) -> bool {
        self.rounded
    }

    /// Returns whether the value label is drawn in the arc's middle.
    pub fn show_value(&self) -> bool {
        self.show_value
    }

    /// Sets whether the arc ends are rounded.
    pub fn set_rounded(&mut self, rounded: bool) {
        if self.rounded == rounded {
            return;
        }
        self.rounded = rounded;
        self.base.request_redraw();
    }

    /// Sets whether the value text is shown in the center of the arc.
    pub fn set_show_value(&mut self, show: bool) {
        if self.show_value == show {
            return;
        }
        self.show_value = show;
        self.base.request_redraw();
    }

    /// Sets whether the arc is in indeterminate (spinning indicator) mode.
    ///
    /// Entering the spinning state restarts at the top rather than wherever a previous
    /// sweep stopped, so two arcs switched on at different times read as one animation.
    pub fn set_indeterminate(&mut self, indeterminate: bool) {
        if self.indeterminate == indeterminate {
            return;
        }
        self.indeterminate = indeterminate;
        if indeterminate {
            self.phase = 0.0;
        }
        self.base.request_redraw();
    }

    /// Returns whether the ring is painted as a true arc stroke.
    pub fn true_arc(&self) -> bool {
        self.true_arc
    }

    /// Selects whether the ring is painted as a true arc stroke.
    ///
    /// The historical picture is a filled disc with a background-coloured disc punched out
    /// of its middle. That hole is an *opaque* paint, so it shows as a white disc whenever
    /// the control sits on a non-solid background. With this switched on the track and the
    /// progress arc are drawn as strokes at the ring's own middle radius, leaving the centre
    /// genuinely untouched.
    ///
    /// Defaults to `false`, so an existing `Arc` keeps the exact pixels it always had.
    pub fn set_true_arc(&mut self, on: bool) {
        if self.true_arc == on {
            return;
        }
        self.true_arc = on;
        self.base.request_redraw();
    }

    /// The indeterminate sweep's current angle, in radians.
    ///
    /// A function of the ticks the control was given, not of the wall clock, so a test can
    /// assert that the arc turns without sleeping.
    pub fn spin_angle(&self) -> f32 {
        self.phase * 2.0 * core::f32::consts::PI
    }

    /// Advances the indeterminate sweep by `delta_ms`.
    ///
    /// Returns `true` while the arc keeps spinning and `false` when it is still, which is
    /// the contract the frame bus schedules frames from. A determinate arc owes no frames.
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        if !self.indeterminate {
            return false;
        }
        let period = self.spin_period_ms();
        // Wrapped into `0.0..1.0`: one turn is one revolution, and keeping the fraction
        // bounded stops a long-running app losing angle precision.
        self.phase = (self.phase + delta_ms as f32 / period as f32).fract();
        self.request_frame_repaint(Some(self.damage_bounds()));
        true
    }

    /// Whether the arc owes animation frames.
    pub fn is_animating(&self) -> bool {
        self.indeterminate
    }

    /// The duration of one full revolution, priced by the theme.
    fn spin_period_ms(&self) -> u32 {
        let slow = crate::style::motion_tokens().2;
        if slow == 0 {
            DEFAULT_SPIN_PERIOD_MS
        } else {
            slow.saturating_mul(SPIN_SLOW_MULTIPLE).max(1)
        }
    }

    /// Submits one animated frame as damage bounded by the painted box plus the margin.
    ///
    /// A control that is unmounted falls back to [`BaseWidget::request_redraw`], keeping the
    /// redraw signal contract for host-owned controls; a tick frame passes the rectangle it
    /// already has because the animation sweep holds the registry across `tick`.
    ///
    /// A tick frame keeps the base signal in [`crate::widget::runtime::RepaintMode::Full`]: the
    /// whole-control fallback would re-enter the registry borrow the sweep holds, so only
    /// interaction paths (`bounds == None`) take it. This is the same split
    /// `progress_circle` and `switch` use.
    fn request_frame_repaint(&self, bounds: Option<Rect>) {
        // Silences the unused-parameter warning in the `alloc_frugal` build, where the damage
        // submit below is compiled out and `request_redraw` is the only route.
        let _ = bounds;
        #[cfg(not(alloc_frugal))]
        {
            if let Some(id) = crate::widget::runtime::registry_id_of(self.id()) {
                let narrow = crate::widget::runtime::repaint_mode(id)
                    != crate::widget::runtime::RepaintMode::Full;
                if narrow || bounds.is_none() {
                    let damage = bounds
                        .or_else(|| crate::widget::runtime::painted_box_with_damage_margin(id))
                        .unwrap_or_else(|| self.damage_bounds());
                    crate::widget::runtime::request_repaint_rect(id, damage);
                    self.base.redraw_requested.emit();
                    return;
                }
            }
        }
        self.base.request_redraw();
    }

    /// This control's own rectangle grown by the shared damage margin.
    fn damage_bounds(&self) -> Rect {
        let rect = self.geometry();
        let margin = crate::widget::metrics::DAMAGE_MARGIN;
        let offset = margin as i32;
        Rect::new(
            rect.x.saturating_sub(offset),
            rect.y.saturating_sub(offset),
            rect.width.saturating_add(margin).saturating_add(margin),
            rect.height.saturating_add(margin).saturating_add(margin),
        )
    }

    /// Normalizes the current value to a fraction in [0.0, 1.0].
    fn normalized_value(&self) -> f32 {
        if self.max <= self.min {
            return 0.0;
        }
        (self.value - self.min) as f32 / (self.max - self.min) as f32
    }

    /// Returns the start and end angles in radians for the progress arc.
    ///
    /// The widget API uses degrees with 0 = top, clockwise. Internally the
    /// angles are converted to radians offset by -PI/2 so that 0 radians
    /// corresponds to the 3 o'clock position (standard math convention for
    /// the rendering backend).
    fn arc_angles(&self) -> (f32, f32) {
        let start_deg = self.start_angle as f32;
        let sweep_deg = self.sweep_angle as f32;
        let progress = self.normalized_value();
        let end_deg = start_deg + sweep_deg * progress;
        // Offset by -90 degrees so that 0 (top) → 3 o'clock convention.
        let offset = -90.0_f32;
        (deg_to_rad(start_deg + offset), deg_to_rad(end_deg + offset))
    }

    /// Writes the current value as a percentage into `out` for the centre display.
    ///
    /// Into-style (the calendar-title arrangement): the draw pass renders into its own
    /// buffer, so a hidden reading (`show_value == false`) costs no string at all and a
    /// shown one costs no intermediate `format!` copy.
    fn write_value_text(&self, out: &mut String) {
        if !self.show_value {
            return;
        }
        let value = self.normalized_value();
        let mut rest = self.value_format.as_str();
        while let Some(open) = rest.find('{') {
            out.push_str(&rest[..open]);
            let tail = &rest[open..];
            if let Some(stripped) = tail.strip_prefix("{percent}") {
                let _ = write!(out, "{}", (value * 100.0).round() as u32);
                rest = stripped;
            } else if let Some(stripped) = tail.strip_prefix("{value}") {
                let _ = write!(out, "{value:.2}");
                rest = stripped;
            } else {
                // An unknown brace is literal text rather than a reason to drop the rest.
                out.push('{');
                rest = &tail[1..];
            }
        }
        out.push_str(rest);
    }

    /// The centre-reading template. See `Self::value_format`.
    pub fn format(&self) -> &str {
        &self.value_format
    }

    /// Replaces the centre-reading template.
    ///
    /// The two placeholders are `{percent}` and `{value}`; any other text is copied through.
    pub fn set_format(&mut self, format: impl Into<String>) {
        let format = format.into();
        if self.value_format == format {
            return;
        }
        self.value_format = format;
        self.base.request_redraw();
    }
}

impl Widget for Arc {
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
        let diameter = self.thickness * 4;
        Size::new(diameter.max(60), diameter.max(60))
    }

    /// One frame of the indeterminate sweep.
    fn tick(&mut self, delta_ms: u32) -> bool {
        Arc::tick(self, delta_ms)
    }

    /// The arc owes frames while it is spinning.
    fn is_animating(&self) -> bool {
        Arc::is_animating(self)
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

/// `Arc`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` / `access_write_other.in.rs` dispatch. Every name the
/// `ARC_PROPERTIES` schema publishes as readable is answered here, so the schema
/// and the contract cannot disagree about what exists.
impl WidgetProperties for Arc {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "value" => Ok(CapabilityValue::UInt(self.value() as u64)),
            "minimum" => Ok(CapabilityValue::UInt(self.minimum() as u64)),
            "maximum" => Ok(CapabilityValue::UInt(self.maximum() as u64)),
            "thickness" => Ok(CapabilityValue::UInt(self.thickness() as u64)),
            "sweep_angle" => Ok(CapabilityValue::UInt(self.sweep_angle() as u64)),
            "start_angle" => Ok(CapabilityValue::Float(f64::from(self.start_angle()))),
            "indeterminate" => Ok(CapabilityValue::Bool(self.is_indeterminate())),
            "rounded" => Ok(CapabilityValue::Bool(self.rounded())),
            "show_value" => Ok(CapabilityValue::Bool(self.show_value())),
            "value_format" => Ok(CapabilityValue::String(self.format().to_string())),
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
            "thickness" => {
                self.set_thickness(expect_u32(value)?);
                Ok(())
            }
            "sweep_angle" => {
                self.set_sweep_angle(expect_u16_saturating(value)?);
                Ok(())
            }
            "start_angle" => {
                self.set_start_angle(expect_f64(value)? as f32);
                Ok(())
            }
            "indeterminate" => {
                self.set_indeterminate(expect_bool(value)?);
                Ok(())
            }
            "rounded" => {
                self.set_rounded(expect_bool(value)?);
                Ok(())
            }
            "show_value" => {
                self.set_show_value(expect_bool(value)?);
                Ok(())
            }
            "value_format" => {
                self.set_format(expect_string(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "value",
            "minimum",
            "maximum",
            "thickness",
            "sweep_angle",
            "start_angle",
            "indeterminate",
            "rounded",
            "show_value",
            "value_format",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `arc` publishes.
    ///
    /// Both published names (`set_value`, `set_range`) assign a number and are
    /// answered through the property path (`set("value", ..)`), so neither can be a
    /// zero-argument action; they are refused as
    /// [`CapabilityAccessError::OutOfRange`] rather than reported unknown.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "set_value" | "set_range" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for Arc {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        // Arc is non-interactive by default; subclasses can override.
    }
}

impl Draw for Arc {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let center = Point::new(rect.x + rect.width as i32 / 2, rect.y + rect.height as i32 / 2);
        let is_enabled = self.base.is_enabled();

        // Resolve colors from style overrides, falling back to defaults. A disabled arc
        // follows the same family rule as `progress_circle`: the track fades toward the
        // disabled foreground and the value ink is the disabled tone.
        let track_color = if is_enabled {
            self.style().background_color.unwrap_or(Color::rgb(220, 220, 220))
        } else {
            self.style()
                .background_color
                .unwrap_or(Color::rgb(220, 220, 220))
                .blend(&Color::DISABLED_FOREGROUND, 0.5)
        };

        let arc_color = if is_enabled {
            self.style().text_color.unwrap_or(Color::rgb(0, 120, 215))
        } else {
            Color::DISABLED_FOREGROUND
        };

        // Radius is half the smaller dimension minus a small inset.
        let outer_radius = rect.width.min(rect.height).saturating_sub(2) / 2;
        if outer_radius < self.thickness {
            // Not enough space to draw anything meaningful.
            return;
        }

        // The arc's own geometry, shared by the filled-disc and true-stroke paths so the two
        // cannot disagree about where the ring is.
        let (start_rad, determinate_end_rad) = self.arc_angles();
        let inner_radius = outer_radius.saturating_sub(self.thickness);
        let indet_sweep = deg_to_rad(90.0);
        let (arc_start, arc_end) = if self.indeterminate {
            // The phase advances in `tick`; at phase zero this is the historical static arc, so
            // a control that is never ticked keeps its old picture.
            let start = start_rad + self.spin_angle();
            (start, start + indet_sweep)
        } else {
            (start_rad, determinate_end_rad)
        };

        if self.true_arc {
            // A real stroke at the ring's middle radius: no disc, and therefore no opaque
            // "hole" painted over whatever sits behind the control.
            let stroke = self.thickness.max(1);
            let centre_radius = outer_radius as f32 - stroke as f32 / 2.0;
            if centre_radius <= 0.0 {
                return;
            }
            let centre_radius = centre_radius.round().max(1.0) as u32;
            context.draw_circle_stroke(center, centre_radius, track_color, stroke);
            if (arc_end - arc_start).abs() > 0.001 {
                crate::render::draw_arc_segments(
                    context,
                    center,
                    centre_radius as f32,
                    arc_start,
                    arc_end,
                    arc_color,
                    stroke,
                );
                if self.rounded {
                    let cap_radius = (stroke / 2).max(1);
                    let sx = center.x + (centre_radius as f32 * arc_start.cos()) as i32;
                    let sy = center.y + (centre_radius as f32 * arc_start.sin()) as i32;
                    context.fill_circle(Point::new(sx, sy), cap_radius, arc_color);
                    let ex = center.x + (centre_radius as f32 * arc_end.cos()) as i32;
                    let ey = center.y + (centre_radius as f32 * arc_end.sin()) as i32;
                    context.fill_circle(Point::new(ex, ey), cap_radius, arc_color);
                }
            }
        } else {
            // The pancake form is a real ring: the track and the value arc are both drawn as
            // an annulus, so the centre is a hole in the geometry rather than an opaque disc
            // painted over whatever sits behind the control.
            fill_annulus(
                context,
                center,
                outer_radius,
                inner_radius,
                0.0,
                std::f32::consts::TAU,
                track_color,
            );

            if self.indeterminate {
                // In indeterminate mode, draw a quarter-circle arc as a spinning indicator.
                fill_annulus(
                    context,
                    center,
                    outer_radius,
                    inner_radius,
                    arc_start,
                    arc_end,
                    arc_color,
                );
            } else {
                // Draw the progress arc from start_angle to start_angle + sweep * progress.
                if (arc_end - arc_start).abs() > 0.001 {
                    fill_annulus(
                        context,
                        center,
                        outer_radius,
                        inner_radius,
                        arc_start,
                        arc_end,
                        arc_color,
                    );
                }

                // When rounded ends are enabled, draw small filled circles at the start and
                // end of the arc to create a rounded cap appearance. The caps sit on the
                // ring's centreline, which is the same radius the annulus occupies.
                if self.rounded && (arc_end - arc_start).abs() > 0.001 {
                    let half_thick = (self.thickness as f32 / 2.0).max(1.0) as u32;
                    let cap_radius = half_thick.max(2);
                    let centre_radius = inner_radius as f32 + self.thickness as f32 / 2.0;

                    // Cap at the start of the arc.
                    let sx = center.x + (centre_radius * arc_start.cos()) as i32;
                    let sy = center.y + (centre_radius * arc_start.sin()) as i32;
                    context.fill_circle(Point::new(sx, sy), cap_radius, arc_color);

                    // Cap at the end of the arc.
                    let ex = center.x + (centre_radius * arc_end.cos()) as i32;
                    let ey = center.y + (centre_radius * arc_end.sin()) as i32;
                    context.fill_circle(Point::new(ex, ey), cap_radius, arc_color);
                }
            }
        }

        // Draw the value text in the centre of the arc, if enabled.
        //
        // The label belongs in the **ring's hole**, not in the middle of the control: the
        // hole's diameter is `2 * inner_radius`, and an arc drawn as a ring only has room for
        // text narrower and shorter than that hole. The old placement centred the text in the
        // whole control, so on a ring whose hole is smaller than the label the digits were
        // painted over the ring itself. It is also dropped when it cannot fit, rather than
        // straddling the arc: a reading that covers the progress it reports is worse than no
        // reading, and the raster backend hides the overflow while the SVG one shows it.
        let mut text = core::mem::take(&mut self.value_text);
        text.clear();
        self.write_value_text(&mut text);
        if !text.is_empty() && inner_radius > 0 {
            // The reading only changes when the value or the resolved font changes, and its
            // measurement additionally depends on the backend's device scale. The pct, the
            // font value and the scale are therefore the whole cache key: measured bounds
            // are not a function of the control's rectangle.
            let pct = (self.normalized_value() * 100.0).round() as u32;
            let value_bits = self.normalized_value().to_bits();
            let scale = context.dpi_scale();
            let stale = {
                let font = effective_font(self.style());
                self.cached_value_pct != Some(pct)
                    || self.cached_value_bits != value_bits
                    || self.cached_value_format != self.value_format
                    || self.cached_value_scale != scale
                    || self.cached_value_font.as_ref() != Some(font)
            };
            if stale {
                let metrics = {
                    let font = effective_font(self.style());
                    context.measure_text(&text, font)
                };
                self.cached_value_pct = Some(pct);
                self.cached_value_bits = value_bits;
                self.cached_value_format = self.value_format.clone();
                self.cached_value_scale = scale;
                self.cached_value_font = Some(effective_font(self.style()).clone());
                self.cached_value_metrics = metrics;
            }
            let metrics = self.cached_value_metrics;
            let font = effective_font(self.style());
            let hole = inner_radius.saturating_mul(2);
            // A one-pixel margin on each side of the hole, so the glyphs do not touch the
            // ring they sit inside.
            let fits =
                metrics.width <= hole.saturating_sub(2) && metrics.height <= hole.saturating_sub(2);
            if fits {
                let text_x = center.x - (metrics.width as i32 / 2);
                let text_y = center.y - (metrics.height as i32 / 2);
                let text_color = if is_enabled {
                    self.style().text_color.unwrap_or(Color::rgb(0, 0, 0))
                } else {
                    Color::DISABLED_FOREGROUND
                };
                context.draw_text(
                    Point::new(text_x, text_y),
                    text.as_str(),
                    font,
                    text_color,
                    HorizontalAlignment::Left,
                );
            }
        }
        // The buffer keeps its capacity, so the next frame's `write!` does not allocate while
        // the reading stays the same length or shorter.
        self.value_text = text;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Color, Rect, Size};
    use crate::render::{PaintBackend, SoftwarePaintBackend};

    /// The bitmap face fills its glyph box, so its ink top *is* the box top and its ink edges *are*
    /// the box edges. An outline face draws a real glyph whose ink is inset, so the ink top sits one
    /// or two pixels below the box top at these sizes and its sides come in by as much. Pinning the
    /// two equal encoded a property of the bitmap face, not of the layout.
    ///
    /// Gated with its only consumer: `arc_value_is_drawn_only_when_it_fits_in_the_ring_hole` renders
    /// through the SVG backend, which is `#[cfg(not(alloc_frugal))]`. An ungated constant therefore
    /// became dead code under `mini` (which *is* `alloc_frugal`) and `clippy -D warnings` reported it
    /// there while every other profile stayed green.
    #[cfg(not(alloc_frugal))]
    const INK_INSET_TOLERANCE: i32 = 3;

    #[test]
    fn arc_creation_defaults() {
        let arc = Arc::new(Rect::new(0, 0, 200, 200));
        assert_eq!(arc.value(), 0);
        assert_eq!(arc.min, 0);
        assert_eq!(arc.max, 100);
        assert_eq!(arc.sweep_angle, 360);
        assert_eq!(arc.thickness, 10);
        assert!(!arc.rounded);
        assert!(arc.show_value);
        assert!(!arc.indeterminate);
        assert_eq!(arc.start_angle, 0);
    }

    #[test]
    fn arc_set_value_clamps() {
        let mut arc = Arc::new(Rect::new(0, 0, 200, 200));
        arc.set_value(50);
        assert_eq!(arc.value(), 50);

        // Above max should clamp to 100.
        arc.set_value(200);
        assert_eq!(arc.value(), 100);

        // Below min should clamp to 0.
        arc.set_value(0);
        assert_eq!(arc.value(), 0);
    }

    #[test]
    fn arc_set_range() {
        let mut arc = Arc::new(Rect::new(0, 0, 200, 200));
        arc.set_range(10, 50);
        assert_eq!(arc.min, 10);
        assert_eq!(arc.max, 50);

        // Value should be re-clamped to the new range.
        arc.set_value(30);
        assert_eq!(arc.value(), 30);

        // Value below new min clamps up.
        arc.set_value(5);
        assert_eq!(arc.value(), 10);

        // Value above new max clamps down.
        arc.set_value(60);
        assert_eq!(arc.value(), 50);
    }

    #[test]
    fn arc_indeterminate_mode() {
        let mut arc = Arc::new(Rect::new(0, 0, 200, 200));
        assert!(!arc.indeterminate);

        arc.set_indeterminate(true);
        assert!(arc.indeterminate);

        arc.set_indeterminate(false);
        assert!(!arc.indeterminate);
    }

    #[test]
    fn arc_draw_does_not_panic() {
        let mut arc = Arc::new(Rect::new(0, 0, 200, 200));
        arc.set_value(65);

        let mut backend = SoftwarePaintBackend::new(Size::new(200, 200), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut context = RenderContext::new(&mut backend);
        arc.draw(&mut context);
        backend.end_frame();
    }

    /// The `0%` reading is drawn in the ring's **hole**, and dropped when it cannot fit there.
    ///
    /// The label used to be centred on the control and printed whatever its size, so on a ring
    /// whose hole is smaller than the text the digits were painted over the arc they annotate —
    /// invisible in a raster (clipped) and visible in the SVG. This pins the two halves of the
    /// fix: a roomy ring still shows the reading, and a ring with no hole at all (a disc, below
    /// the thickness) lays down no text ink.
    ///
    /// # Why the measure is ink and not a `<text>` element
    ///
    /// The backend no longer hands the string to the viewer's font engine: it emits the same
    /// `font8x8` rectangles the software rasteriser fills, inside a single `<path>` (see
    /// `text_subpath_count`). The string is therefore absent from the document in every form,
    /// and "was the reading drawn?" is a question about **ink** — subpath count and ink box —
    /// rather than about the presence of an element.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn arc_value_is_drawn_only_when_it_fits_in_the_ring_hole() {
        // Holds the crate-wide theme guard: this test renders, and a concurrent
        // test that switches the appearance would otherwise change a later frame.
        let _theme_guard = crate::style::theme_test_guard();
        /// The ink the reading leaves, as `(subpaths, ink box)`.
        fn reading_ink(arc: &mut Arc, side: u32) -> (usize, Option<(i32, i32, i32, i32)>) {
            let mut backend = SoftwarePaintBackend::new(Size::new(side, side), 1.0);
            backend.begin_frame(Color::WHITE);
            {
                let mut context = RenderContext::new(&mut backend);
                arc.draw(&mut context);
            }
            backend.end_frame();
            let svg = crate::widget::svg::render_to_svg(arc);
            (crate::widget::svg::text_subpath_count(&svg), crate::widget::svg::text_ink_box(&svg))
        }

        // A 200 px ring with the default 20 px thickness has a 140 px hole: the 0% fits.
        let mut roomy = Arc::new(Rect::new(0, 0, 200, 200));
        roomy.set_show_value(true);
        let (subpaths, ink) = reading_ink(&mut roomy, 200);
        assert!(subpaths > 0, "a reading that fits inside the ring hole must still be drawn");
        // …and it is drawn in the hole, not over the ring. An outline face's ink is narrower and
        // lower than the bitmap's — it is inset from the glyph box on every side — so the run's
        // ink is *within* the bounds a full-box face would fill rather than exactly on them. A
        // label at the arc's own radius, or one wider than the hole, overshoots these bounds.
        let (left, top, right, _) = ink.expect("the drawn reading has an ink box");
        let field = roomy.geometry();
        let outer_radius = field.width.min(field.height).saturating_sub(2) / 2;
        let inner_radius = outer_radius.saturating_sub(roomy.thickness);
        let center_x = field.x + field.width as i32 / 2;
        let center_y = field.y + field.height as i32 / 2;
        // The run is drawn on the hole's **own** centre line and its ink fits inside the hole:
        // `inner_radius` is that hole's half-width, so neither edge of the ink reaches past
        // `inner_radius` from `center_x`. A reading placed at the arc's own radius (the defect
        // this pins) overshoots that bound; one positioned from the field rather than from the
        // hole lands off the hole's centre line entirely.
        assert!(
            left >= center_x - inner_radius as i32 && right <= center_x + inner_radius as i32,
            "the reading's ink {left}..{right} must lie within the hole's centre ± {inner_radius}"
        );
        // Hang on that centre line: `left - center_x == center_x - right` under the bitmap face,
        // where both halves of a glyph's last column carry ink. An outline face's rightmost ink
        // sits a pixel or two inside the box (that column has no ink), so the two distances
        // balance to within the same inset; a label hung on the *field's* edge is off by
        // `center_x - inner_radius` — 70 px here — and still fails.
        assert!(
            (left - center_x + right - center_x).abs() <= INK_INSET_TOLERANCE,
            "the reading hangs on the hole's own centre line, not the field's: \
             ink {left}..{right}, centre {center_x}"
        );
        // The digit's ink reaches the line box's top edge under the bitmap face and sits an inset
        // below it under an outline face; either way the run is on the hole's middle line, so a
        // reading on the field's top or at the arc's radius is a whole line away and fails.
        assert!(
            (top - (center_y - 7)).abs() <= INK_INSET_TOLERANCE,
            "and is centred on the hole's middle line: ink top {top}, box top {}",
            center_y - 7
        );

        // A ring whose thickness equals its radius has no hole for the label, so there is
        // nowhere legal to put it — it is dropped rather than overprinted on the arc.
        let mut solid = Arc::new(Rect::new(0, 0, 200, 200));
        solid.set_show_value(true);
        solid.set_thickness(99);
        assert_eq!(
            reading_ink(&mut solid, 200),
            (0, None),
            "with no ring hole the reading must be dropped, not drawn over the arc"
        );
    }

    #[test]
    fn arc_normalized_value_returns_zero_for_empty_range() {
        let mut arc = Arc::new(Rect::new(0, 0, 100, 100));
        arc.set_range(50, 50);
        assert!((arc.normalized_value() - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn arc_normalized_value_half() {
        let mut arc = Arc::new(Rect::new(0, 0, 100, 100));
        arc.set_value(50);
        assert!((arc.normalized_value() - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn arc_set_sweep_angle() {
        let mut arc = Arc::new(Rect::new(0, 0, 100, 100));
        assert_eq!(arc.sweep_angle, 360);
        arc.set_sweep_angle(270);
        assert_eq!(arc.sweep_angle, 270);
    }

    #[test]
    fn arc_set_thickness() {
        let mut arc = Arc::new(Rect::new(0, 0, 100, 100));
        assert_eq!(arc.thickness, 10);
        arc.set_thickness(20);
        assert_eq!(arc.thickness, 20);
        // Thickness should not drop below 1.
        arc.set_thickness(0);
        assert_eq!(arc.thickness, 1);
    }

    #[test]
    fn arc_rounded_toggle() {
        let mut arc = Arc::new(Rect::new(0, 0, 100, 100));
        assert!(!arc.rounded);
        arc.set_rounded(true);
        assert!(arc.rounded);
    }

    #[test]
    fn arc_show_value_toggle() {
        let mut arc = Arc::new(Rect::new(0, 0, 100, 100));
        assert!(arc.show_value);
        arc.set_show_value(false);
        assert!(!arc.show_value);
    }

    #[test]
    fn arc_geometry_delegation() {
        let mut arc = Arc::new(Rect::new(0, 0, 200, 200));
        arc.set_geometry(Rect::new(10, 10, 150, 150));
        assert_eq!(arc.geometry(), Rect::new(10, 10, 150, 150));
    }

    #[test]
    fn arc_size_hint() {
        let arc = Arc::new(Rect::new(0, 0, 200, 200));
        let hint = arc.size_hint();
        assert_eq!(hint, Size::new(60, 60));
    }

    #[test]
    fn arc_draw_indeterminate_does_not_panic() {
        let mut arc = Arc::new(Rect::new(0, 0, 200, 200));
        arc.set_indeterminate(true);

        let mut backend = SoftwarePaintBackend::new(Size::new(200, 200), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut context = RenderContext::new(&mut backend);
        arc.draw(&mut context);
        backend.end_frame();
    }

    /// The indeterminate arc turns on the ticks it is given, and owes frames while it does.
    ///
    /// The sweep is state advanced by `tick`, not a wall-clock read inside `draw`, so the
    /// angle is a function of the frames the control received and can be asserted without
    /// sleeping. A determinate arc is a still picture and owes nothing.
    #[test]
    fn arc_indeterminate_spins_on_the_ticks_it_is_given() {
        let mut arc = Arc::new(Rect::new(0, 0, 200, 200));
        assert!(!arc.is_animating(), "a determinate arc is still");
        assert!(!arc.tick(16), "and owes no frames");
        assert_eq!(arc.spin_angle(), 0.0);

        arc.set_indeterminate(true);
        assert!(arc.is_animating(), "an indeterminate arc always owes frames");
        let first = arc.spin_angle();
        assert!(arc.tick(100), "a spin frame reports more work");
        let second = arc.spin_angle();
        assert_ne!(first, second, "the sweep must advance with the tick");
        assert!(second > 0.0 && second < 2.0 * core::f32::consts::PI);

        // Turning the mode off stops the spin and the frame debt with it.
        arc.set_indeterminate(false);
        assert!(!arc.is_animating());
        assert!(!arc.tick(100));
    }

    /// A signed range reports absolute readings while the legacy accessors keep their
    /// historical unsigned clamping.
    #[test]
    fn arc_signed_range_reports_absolute_readings() {
        let mut arc = Arc::new(Rect::new(0, 0, 200, 200));
        arc.set_range_signed(-40, 40);
        assert_eq!(
            arc.signed_value(),
            0,
            "installing a range preserves a reading that is already inside it"
        );

        arc.set_signed_value(-10);
        assert_eq!(arc.signed_value(), -10);
        assert_eq!(arc.value(), 30, "the legacy accessor reports the offset from the minimum");
        assert!(
            (arc.normalized_value() - 30.0 / 80.0).abs() < f32::EPSILON,
            "the arc fraction is the same arithmetic on the offset scale"
        );

        // Above and below the signed bounds clamp to the ends.
        arc.set_signed_value(1000);
        assert_eq!(arc.signed_value(), 40);
        arc.set_signed_value(-1000);
        assert_eq!(arc.signed_value(), -40);

        // An unsigned range switches the control back to its historical scale.
        arc.set_range(0, 100);
        arc.set_value(25);
        assert_eq!(arc.signed_value(), 25);
    }

    /// The true-arc picture is opt-in and paints a stroke rather than a punched-out disc.
    #[test]
    fn arc_true_arc_is_opt_in_and_paints_a_stroke() {
        let mut arc = Arc::new(Rect::new(0, 0, 200, 200));
        arc.set_value(50);
        assert!(!arc.true_arc(), "the historical filled-disc picture is the default");
        let filled = crate::widget::svg::render_to_svg(&mut arc);
        assert!(
            !filled.contains("fill=\"none\""),
            "the default path paints no ring stroke: {filled}"
        );

        arc.set_true_arc(true);
        assert!(arc.true_arc());
        let stroked = crate::widget::svg::render_to_svg(&mut arc);
        assert!(
            stroked.contains("fill=\"none\"") && stroked.contains("stroke-width="),
            "the true-arc path must paint a real stroke: {stroked}"
        );
    }

    /// The centre reading is a template with the same two placeholders `progress_circle` uses.
    #[test]
    fn the_centre_reading_template_supports_percent_and_value() {
        let mut arc = Arc::new(Rect::new(0, 0, 120, 120));
        arc.set_range(0, 4);
        arc.set_value(3);
        assert_eq!(arc.format(), "{percent}%", "the default keeps the historical reading");

        let mut text = String::new();
        arc.write_value_text(&mut text);
        assert_eq!(text, "75%");

        arc.set_format("{value} ({percent}%)");
        let mut text = String::new();
        arc.write_value_text(&mut text);
        assert_eq!(text, "0.75 (75%)");

        assert_eq!(
            arc.get("value_format").unwrap(),
            CapabilityValue::String("{value} ({percent}%)".to_string())
        );
    }

    /// The pancake form's hole is real geometry: the backdrop shows through it.
    #[test]
    fn the_pancake_hole_is_transparent() {
        let backdrop = Color::rgb(10, 20, 30);
        let mut arc = Arc::new(Rect::new(0, 0, 120, 120));
        arc.set_show_value(false);
        arc.set_value(50);
        arc.set_style(
            crate::style::WidgetStyle::default()
                .with_background(Color::rgb(200, 200, 200))
                .with_text_color(Color::rgb(0, 120, 215)),
        );

        let mut backend = SoftwarePaintBackend::new(Size::new(120, 120), 1.0);
        backend.begin_frame(backdrop);
        {
            let mut context = RenderContext::new(&mut backend);
            arc.draw(&mut context);
        }
        backend.end_frame();
        let pixels = backend.frame_rgba().to_vec();
        let centre = ((60 * 120 + 60) * 4) as usize;
        assert_eq!(
            &pixels[centre..centre + 3],
            &[backdrop.r, backdrop.g, backdrop.b],
            "the hole must not paint an opaque background over the backdrop"
        );

        // The ring itself is still painted.
        let ring = ((60 * 120 + 6) * 4) as usize;
        assert_ne!(&pixels[ring..ring + 3], &[backdrop.r, backdrop.g, backdrop.b]);
    }

    /// A disabled arc paints the same family's disabled tone and differs from the enabled one.
    #[test]
    fn a_disabled_arc_has_a_disabled_visual() {
        let _theme_guard = crate::style::theme_test_guard();
        let mut arc = Arc::new(Rect::new(0, 0, 120, 120));
        arc.set_value(60);
        let enabled = crate::widget::svg::render_to_svg(&mut arc);

        arc.set_enabled(false);
        assert!(!arc.is_enabled());
        let disabled = crate::widget::svg::render_to_svg(&mut arc);
        assert_ne!(enabled, disabled, "a disabled arc must not paint the enabled colours");
    }
}
