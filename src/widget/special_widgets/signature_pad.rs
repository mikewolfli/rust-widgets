// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! SignaturePad widget — a touch-friendly freehand drawing surface.
//!
//! Captures pen strokes as a list of [`SignatureStroke`]s, each a polyline of
//! [`Point`]s. A stroke is in progress between a press and the matching release,
//! and every move appends a point. Optional smoothing collapses near-duplicate
//! points so a jittery finger produces a clean line rather than a cloud of
//! specks. The pad supports undo, clear, and export to a plain point list.

use crate::compat::Instant;
use crate::core::{Color, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::capability::coercion::{expect_f64, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// A single continuous pen stroke, stored as a polyline of points.
///
/// Each point carries the normalized tip pressure the device reported when the
/// point was taken, so a stroke records not just *where* the pen went but *how
/// hard* it pressed. Mouse and touch input report a neutral pressure (see
/// [`SignatureStroke::push`]), so a stroke drawn with them is uniform.
#[derive(Debug, Clone, PartialEq)]
pub struct SignatureStroke {
    points: Vec<Point>,
    /// Normalized tip pressure (`0.0..=1.0`) for the point at the same index in
    /// `points`. Always the same length as `points`.
    pressures: Vec<f32>,
}

/// The pressure a mouse or touch contact reports: neutral, so a stroke drawn with
/// it keeps the pad's configured width (D09-POINTER-01).
const NEUTRAL_PRESSURE: f32 = 1.0;

impl SignatureStroke {
    /// Creates an empty stroke.
    pub fn new() -> Self {
        Self { points: Vec::new(), pressures: Vec::new() }
    }

    /// Creates a stroke from a list of points.
    ///
    /// The points carry a neutral pressure, so a stroke built this way renders at
    /// the pad's configured width. Use [`SignatureStroke::push_with_pressure`] to
    /// attach real per-point pressure.
    pub fn from_points(points: Vec<Point>) -> Self {
        let pressures = vec![NEUTRAL_PRESSURE; points.len()];
        Self { points, pressures }
    }

    /// Appends a point to the stroke with neutral pressure.
    pub fn push(&mut self, point: Point) {
        self.push_with_pressure(point, NEUTRAL_PRESSURE);
    }

    /// Appends a point and the normalized tip pressure reported with it.
    ///
    /// The pressure is clamped to `0.0..=1.0`; a non-finite value collapses to the
    /// neutral pressure rather than poisoning the stroke's width (D09-POINTER-01).
    pub fn push_with_pressure(&mut self, point: Point, pressure: f32) {
        let clamped =
            if pressure.is_finite() { pressure.clamp(0.0, 1.0) } else { NEUTRAL_PRESSURE };
        self.points.push(point);
        self.pressures.push(clamped);
    }

    /// Returns the stroke's points.
    pub fn points(&self) -> &[Point] {
        &self.points
    }

    /// Returns the normalized tip pressure recorded for each point, in the same
    /// order and of the same length as [`Self::points`].
    pub fn pressures(&self) -> &[f32] {
        &self.pressures
    }

    /// Returns the pressure recorded with the most recent point, or the neutral
    /// pressure when the stroke is empty.
    pub fn last_pressure(&self) -> f32 {
        self.pressures.last().copied().unwrap_or(NEUTRAL_PRESSURE)
    }

    /// Returns the number of points in the stroke.
    pub fn len(&self) -> usize {
        self.points.len()
    }

    /// Returns true when the stroke has no points.
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }
}

impl Default for SignatureStroke {
    fn default() -> Self {
        Self::new()
    }
}

/// A touch-friendly signature capture surface.
///
/// A drag (press → move → release) records one stroke on top of the previous
/// ones. The last stroke can be undone, or the whole pad cleared. `changed`
/// fires whenever the stroke set changes, and `stroke_count` reflects the
/// current total.
pub struct SignaturePad {
    base: BaseWidget,
    strokes: Vec<SignatureStroke>,
    current: Option<SignatureStroke>,
    stroke_color: Color,
    stroke_width: u32,
    /// Minimum distance, in pixels, a new point must be from the previous one
    /// before it is recorded. Points closer than this are dropped.
    min_point_distance: f32,
    /// Minimum time, in milliseconds, between recorded points.
    ///
    /// # Why distance alone was not enough
    ///
    /// A distance-only filter has no way to tell "the pointer is moving slowly and
    /// has already produced enough points" from "the pointer is moving fast and has
    /// produced too few". Sampling a fast stroke at one point per input event left
    /// every segment long, so a curve drawn quickly came out as a **polygon**: the
    /// pad recorded where the pointer *was*, never where it *went*. Time is the
    /// missing dimension -- a point is now recorded when it is far enough away *or*
    /// when enough time has passed, and a gap wider than this interval is filled by
    /// interpolating along the segment so the curve is smooth at any speed.
    min_point_interval_ms: u64,
    /// When the most recently recorded point was taken, from the OS monotonic clock.
    ///
    /// `None` until a stroke begins. The clock is read *inside* the widget when a
    /// pointer event arrives (see [`Self::extend_stroke`]): the event payloads carry
    /// position, pressure and tilt but no time, and a drawing surface must not
    /// acquire a network dependency for a value the operating system already
    /// provides. A network timestamp answers a different question -- *provenance*,
    /// "when was this signed" -- which is the caller's to attach and must not gate
    /// whether ink can be drawn.
    last_point_at: Option<Instant>,
    /// The tip pressure reported with the most recent pointer event, normalized to
    /// `0.0..=1.0`. Retained so a caller can read the last pen datum even after the
    /// stroke commits, and so the pressure datum is never silently discarded
    /// (D09-POINTER-01).
    last_pressure: f32,
    /// The stylus tilt (about X then Y, in device degrees) reported with the most
    /// recent pointer event. Retained for the same reason as `last_pressure`; the pad
    /// does not yet shape ink from tilt, so it is stored rather than dropped
    /// (D09-POINTER-01).
    last_tilt: (f32, f32),
    /// When `true` (the default) a stroke's per-segment width follows the tip
    /// pressure a pen reports, so pressing harder draws a wider line. When `false`
    /// every segment uses [`Self::stroke_width`]. A mouse or touch contact reports a
    /// neutral pressure, so both settings render the same width for them.
    pressure_affects_width: bool,
    /// Emitted with no payload whenever the committed stroke set changes.
    pub changed: GenericSignal,
    /// Emitted with the final stroke when a drag is released and committed.
    pub stroke_completed: Signal1<SignatureStroke>,
}

impl SignaturePad {
    /// Creates an empty signature pad.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::SignaturePad, geometry, "SignaturePad"),
            strokes: Vec::new(),
            current: None,
            stroke_color: Color::rgb(20, 20, 20),
            stroke_width: 2,
            min_point_distance: 1.5,
            min_point_interval_ms: 10,
            last_point_at: None,
            last_pressure: NEUTRAL_PRESSURE,
            last_tilt: (0.0, 0.0),
            pressure_affects_width: true,
            changed: GenericSignal::new(),
            stroke_completed: Signal1::new(),
        }
    }

    /// Returns the committed strokes (excluding any in-progress stroke).
    pub fn strokes(&self) -> &[SignatureStroke] {
        &self.strokes
    }

    /// Returns the total number of committed strokes.
    pub fn stroke_count(&self) -> usize {
        self.strokes.len()
    }

    /// Returns true when the pad has no committed strokes.
    pub fn is_empty(&self) -> bool {
        self.strokes.is_empty()
    }

    /// Removes the most recently committed stroke, returning whether one was
    /// removed. Emits `changed` when a stroke is actually removed.
    pub fn undo(&mut self) -> bool {
        if self.strokes.pop().is_some() {
            self.changed.emit();
            self.base.request_redraw();
            true
        } else {
            false
        }
    }

    /// Removes every committed stroke. Emits `changed`.
    pub fn clear(&mut self) {
        if !self.strokes.is_empty() {
            self.strokes.clear();
            self.current = None;
            // The interval is measured from the last recorded point, so clearing must
            // forget it too -- otherwise the next stroke's first move is compared
            // against a moment that belongs to a stroke no longer on the pad.
            self.last_point_at = None;
            self.changed.emit();
            self.base.request_redraw();
        }
    }

    /// Returns the pen stroke color.
    pub fn stroke_color(&self) -> Color {
        self.stroke_color
    }

    /// Sets the pen stroke color and requests a redraw.
    pub fn set_stroke_color(&mut self, color: Color) {
        self.stroke_color = color;
        self.base.request_redraw();
    }

    /// Returns the pen stroke width in pixels.
    pub fn stroke_width(&self) -> u32 {
        self.stroke_width
    }

    /// Sets the pen stroke width, floored at 1, and requests a redraw.
    pub fn set_stroke_width(&mut self, width: u32) {
        self.stroke_width = width.max(1);
        self.base.request_redraw();
    }

    /// Returns the tip pressure reported with the most recent pointer event,
    /// normalized to `0.0..=1.0` (D09-POINTER-01).
    ///
    /// This is the retained pen datum: a `PointerMove` carrying pressure updates it
    /// even when the point itself is dropped by the sampling rule, so the value is
    /// queryable rather than silently discarded. A mouse or touch contact reports the
    /// neutral `1.0`.
    pub fn last_pressure(&self) -> f32 {
        self.last_pressure
    }

    /// Returns the stylus tilt (about X, then Y, in device degrees) reported with the
    /// most recent pointer event (D09-POINTER-01).
    ///
    /// The pad retains tilt but does not yet shape ink from it; this accessor exists so
    /// the datum is observable instead of being dropped on the floor.
    pub fn last_tilt(&self) -> (f32, f32) {
        self.last_tilt
    }

    /// Returns whether tip pressure modulates the drawn stroke width.
    pub fn pressure_affects_width(&self) -> bool {
        self.pressure_affects_width
    }

    /// Enables or disables pressure-driven stroke width and requests a redraw when the
    /// setting changes.
    ///
    /// When enabled, each segment is drawn at the pad's configured width scaled by the
    /// pressure of the points that bound it, so a pen that presses harder draws a
    /// heavier line. When disabled, every segment uses [`Self::stroke_width`]
    /// regardless of pressure.
    pub fn set_pressure_affects_width(&mut self, enabled: bool) {
        if self.pressure_affects_width != enabled {
            self.pressure_affects_width = enabled;
            self.base.request_redraw();
        }
    }

    /// Returns the minimum distance between recorded points, in pixels.
    pub fn min_point_distance(&self) -> f32 {
        self.min_point_distance
    }

    /// Sets the minimum distance between recorded points. Values below 0 collapse
    /// to 0 (every point is recorded).
    pub fn set_min_point_distance(&mut self, distance: f32) {
        self.min_point_distance = distance.max(0.0);
    }

    /// Returns the minimum interval between recorded points, in milliseconds.
    ///
    /// This is the time half of the sampling rule; see
    /// [`Self::set_min_point_interval_ms`] for why distance alone polygonised fast
    /// strokes.
    pub fn min_point_interval_ms(&self) -> u64 {
        self.min_point_interval_ms
    }

    /// Sets the minimum interval between recorded points, in **milliseconds**.
    ///
    /// A point is recorded when it is at least [`Self::min_point_distance`] from the
    /// last one **or** at least this long after it, and a gap wider than this
    /// interval is subdivided by interpolation so a fast stroke stays a curve
    /// rather than becoming a polygon.
    ///
    /// `0` records every input event, which is the pre-existing behaviour of a pad
    /// with no smoothing; there is no upper bound because the interval only ever
    /// makes the pad *sparser*, and a caller asking for a very long one is asking
    /// for a deliberately coarse capture.
    pub fn set_min_point_interval_ms(&mut self, interval_ms: u64) {
        self.min_point_interval_ms = interval_ms;
    }

    /// Exports every committed stroke's points as a flat list, each stroke
    /// followed by a `(-1, -1)` sentinel so the caller can reconstruct stroke
    /// boundaries from a single vector.
    pub fn export_polylines(&self) -> Vec<Point> {
        let mut out = Vec::new();
        for stroke in &self.strokes {
            out.extend_from_slice(stroke.points());
            out.push(Point::new(-1, -1));
        }
        out
    }

    fn in_progress(&self) -> bool {
        self.current.is_some()
    }

    /// Begins a stroke at `point`.
    ///
    /// `pressure` and `tilt` are the pen data (if any) reported with the press: the
    /// pressure seeds the first point's width, and both are retained on the pad so
    /// they remain queryable through [`Self::last_pressure`]/[`Self::last_tilt`]
    /// (D09-POINTER-01). A mouse or touch press passes the neutral pressure and zero
    /// tilt.
    fn begin_stroke_with(&mut self, point: Point, pressure: f32, tilt: (f32, f32)) {
        self.record_pen_data(pressure, tilt);
        let mut stroke = SignatureStroke::new();
        stroke.push_with_pressure(point, self.last_pressure);
        self.current = Some(stroke);
        // The clock starts with the stroke, so the first interval is measured
        // against the press rather than against whenever a pointer last moved.
        self.last_point_at = Some(Instant::now());
    }

    /// Begins a stroke at `point` with neutral pen data.
    fn begin_stroke(&mut self, point: Point) {
        self.begin_stroke_with(point, NEUTRAL_PRESSURE, (0.0, 0.0));
    }

    /// Retains the pressure/tilt a pointer event reported so they stay queryable, and
    /// clamps a non-finite pressure to neutral (D09-POINTER-01).
    fn record_pen_data(&mut self, pressure: f32, tilt: (f32, f32)) {
        self.last_pressure =
            if pressure.is_finite() { pressure.clamp(0.0, 1.0) } else { NEUTRAL_PRESSURE };
        self.last_tilt = (
            if tilt.0.is_finite() { tilt.0 } else { 0.0 },
            if tilt.1.is_finite() { tilt.1 } else { 0.0 },
        );
    }

    /// Extends the in-progress stroke to `point` with neutral pen data.
    fn extend_stroke(&mut self, point: Point) {
        self.extend_stroke_with(point, NEUTRAL_PRESSURE, (0.0, 0.0));
    }

    /// Extends the in-progress stroke to `point`, attributing `pressure` to the new
    /// point(s) so the stroke's width can follow the pen (D09-POINTER-01).
    fn extend_stroke_with(&mut self, point: Point, pressure: f32, tilt: (f32, f32)) {
        self.record_pen_data(pressure, tilt);
        // The clock is read once, before the borrow of `self.current`, so the event's
        // arrival time is a fact about this call rather than about the point's fate.
        let now = Instant::now();
        let elapsed_ms = self
            .last_point_at
            .map(|last| now.saturating_duration_since(last).as_millis() as u64)
            // A move with no press behind it (a synthetic event, or a stroke removed
            // under the pointer) has no interval to measure; treating it as `0` keeps
            // the point on the distance rule rather than fabricating a timestamp.
            .unwrap_or(0);
        // Read out as values before the mutable borrow of `self.current` below, so the
        // sampling rule and its resolution are snapshots of this call rather than reads
        // through a borrow that outlives them.
        let min_distance = self.min_point_distance;
        let min_interval_ms = self.min_point_interval_ms;
        // The pressure attributed to the new point(s): the caller's datum, already
        // clamped by `record_pen_data`, which the interpolation below interpolates.
        let new_pressure = self.last_pressure;

        let Some(current) = self.current.as_mut() else {
            return;
        };
        let Some(last) = current.points().last().copied() else {
            // A stroke always begins with a point, but a caller-built empty `current`
            // must not silently swallow the move.
            current.push_with_pressure(point, new_pressure);
            self.last_point_at = Some(now);
            self.base.request_redraw();
            return;
        };

        let dx = (point.x - last.x) as f32;
        let dy = (point.y - last.y) as f32;
        let distance = (dx * dx + dy * dy).sqrt();
        let far_enough = distance >= min_distance;
        let long_enough = elapsed_ms >= min_interval_ms;
        if !far_enough && !long_enough {
            // Too close *and* too soon: this is the jitter the smoothing exists to
            // drop. The clock is deliberately **not** advanced, so the interval is
            // measured against the last point actually recorded rather than against
            // the last event seen -- otherwise a stream of dropped jitter would keep
            // pushing the next accepted point further away.
            return;
        }

        // A long gap is filled in rather than cut across. This is the half that makes
        // a fast stroke smooth: without it, a pointer that travelled 60 px between
        // two events would deposit two points and the pad would draw one straight
        // edge, which is the polygonisation this control was reported for.
        if distance > min_distance && min_interval_ms > 0 {
            let from_pressure = current.last_pressure();
            for (step, step_pressure) in interpolation_steps_with_pressure(
                last,
                point,
                distance,
                min_distance,
                from_pressure,
                new_pressure,
            ) {
                current.push_with_pressure(step, step_pressure);
            }
        }
        current.push_with_pressure(point, new_pressure);
        self.last_point_at = Some(now);
        self.base.request_redraw();
    }

    fn end_stroke(&mut self) {
        let Some(stroke) = self.current.take() else {
            return;
        };
        // A single-point stroke still counts: it is a tap, which for a signature
        // pad is a legitimate dot.
        self.strokes.push(stroke.clone());
        self.stroke_completed.emit(stroke);
        self.changed.emit();
        self.base.request_redraw();
    }

    /// Discards the in-progress stroke without committing it (D09-EVT-02).
    ///
    /// This is the cancel counterpart of [`Self::end_stroke`]: the platform withdrew the
    /// contact (a `TouchCancel`), so the stroke must not be saved or announced. Unlike
    /// `end_stroke` it emits neither `stroke_completed` nor `changed`, so a cancelled
    /// gesture leaves the committed strokes and the change signal untouched. It is a no-op
    /// when no stroke is in progress.
    fn cancel_stroke(&mut self) {
        if self.current.take().is_some() {
            self.last_point_at = None;
            self.base.request_redraw();
        }
    }
}

impl Widget for SignaturePad {
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
        crate::core::Size::new(320, 160)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why this is explicit
    ///
    /// `connect_event` validates a name against the capability table and registers a hub slot; only
    /// `event_signal_dyn` joins that name to the signal the control actually **emits**. Without an
    /// arm a published name is valid and inert, which is the silent failure
    /// `tools/check_event_signal_dyn.sh` exists to make impossible: the arm set is compared against
    /// the capability's published names, so the two cannot drift.
    ///
    /// # Payload mapping
    ///
    /// A hub name carries no value, so a `Signal1<T>` needs a conversion. The one used here is the
    /// same spelling the property side uses, so a designer reads one representation per kind: a
    /// number as `Int`, a float as `Float`, a flag as `Bool`, text as `String`, and a structured
    /// payload as its debug spelling. A `None` maps to `Null`, which is the same value an absent
    /// optional property uses — so "no selection" is distinguishable from "selection 0".
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "changed" => Some(EventSignalRef::unit("changed", &self.changed)),
            "stroke_completed" => {
                Some(EventSignalRef::mapped("stroke_completed", &self.stroke_completed, |v| {
                    CapabilityValue::String(format!("{v:?}"))
                }))
            }
            _ => None,
        }
    }
}

/// `SignaturePad`'s property contract.
impl WidgetProperties for SignaturePad {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "stroke_count" => Ok(CapabilityValue::UInt(self.stroke_count() as u64)),
            "stroke_width" => Ok(CapabilityValue::UInt(self.stroke_width() as u64)),
            "stroke_color" => Ok(CapabilityValue::Color(self.stroke_color())),
            "min_point_distance" => Ok(CapabilityValue::Float(self.min_point_distance() as f64)),
            "min_point_interval_ms" => Ok(CapabilityValue::UInt(self.min_point_interval_ms())),
            // D09-POINTER-01: the retained pen datum is published so a designer or a host can
            // observe it instead of it being silently dropped.
            "pressure_affects_width" => Ok(CapabilityValue::Bool(self.pressure_affects_width())),
            "last_pressure" => Ok(CapabilityValue::Float(self.last_pressure() as f64)),
            "last_tilt_x" => Ok(CapabilityValue::Float(self.last_tilt().0 as f64)),
            "last_tilt_y" => Ok(CapabilityValue::Float(self.last_tilt().1 as f64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "stroke_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            "stroke_width" => {
                self.set_stroke_width(expect_usize(value)? as u32);
                Ok(())
            }
            "stroke_color" => match value {
                CapabilityValue::Color(color) => {
                    self.set_stroke_color(color);
                    Ok(())
                }
                _ => Err(CapabilityAccessError::TypeMismatch),
            },
            "min_point_distance" => {
                self.set_min_point_distance(expect_f64(value)? as f32);
                Ok(())
            }
            "min_point_interval_ms" => {
                self.set_min_point_interval_ms(expect_usize(value)? as u64);
                Ok(())
            }
            "pressure_affects_width" => match value {
                CapabilityValue::Bool(enabled) => {
                    self.set_pressure_affects_width(enabled);
                    Ok(())
                }
                _ => Err(CapabilityAccessError::TypeMismatch),
            },
            // The retained pen datum is a report of what the device sent, so it is read-only:
            // writing it would fabricate a fact about the input device.
            "last_pressure" | "last_tilt_x" | "last_tilt_y" => {
                Err(CapabilityAccessError::ReadOnlyProperty)
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "stroke_count",
            "stroke_width",
            "stroke_color",
            "min_point_distance",
            "min_point_interval_ms",
            "pressure_affects_width",
            "last_pressure",
            "last_tilt_x",
            "last_tilt_y",
            BASE_PROPERTY_NAMES
        ]
    }

    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "undo" => {
                let _ = self.undo();
                Ok(())
            }
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for SignaturePad {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        let rect = self.geometry();
        match event {
            Event::MousePress { pos, button: 1, .. } if rect.contains_point(*pos) => {
                self.begin_stroke(*pos);
            }
            // D09-POINTER-01: a stylus press starts a stroke exactly like a mouse press, and
            // the pen pressure/tilt it carries seed the first point's width and are retained
            // (see `last_pressure`/`last_tilt`). A `PointerPress` with a non-primary button is
            // a barrel switch, not the tip, so it does not begin a stroke — the same rule the
            // mouse arm applies to button 1.
            Event::PointerPress { pos, button: 1, pressure, tilt_x, tilt_y }
                if rect.contains_point(*pos) =>
            {
                self.begin_stroke_with(*pos, *pressure, (*tilt_x, *tilt_y));
            }
            #[cfg(feature = "touch")]
            Event::TouchBegin { pos, .. } if rect.contains_point(*pos) => {
                self.begin_stroke(*pos);
            }
            Event::MouseMove { pos } if self.in_progress() => {
                self.extend_stroke(*pos);
            }
            // D09-POINTER-01: a stylus move extends the stroke and its pressure/tilt are
            // attributed to the new point(s) rather than dropped, so the ink's width follows
            // the pen.
            Event::PointerMove { pos, pressure, tilt_x, tilt_y } if self.in_progress() => {
                self.extend_stroke_with(*pos, *pressure, (*tilt_x, *tilt_y));
            }
            #[cfg(feature = "touch")]
            Event::TouchMove { pos, .. } if self.in_progress() => {
                self.extend_stroke(*pos);
            }
            Event::MouseRelease { .. } if self.in_progress() => {
                self.end_stroke();
            }
            // D09-POINTER-01: a stylus release ends the stroke. The release's own pressure is
            // retained (it is the last datum the device sent) but does not add a point: the
            // lift-off position is the stroke's last move, so committing here would append a
            // duplicated or jumpy terminal point.
            Event::PointerRelease { pressure, .. } if self.in_progress() => {
                let tilt = self.last_tilt;
                self.record_pen_data(*pressure, tilt);
                self.end_stroke();
            }
            #[cfg(feature = "touch")]
            Event::TouchEnd { .. } if self.in_progress() => {
                self.end_stroke();
            }
            // D09-EVT-02: a withdrawn contact abandons the in-progress stroke instead of
            // saving it. A platform `TouchCancel` (which may arrive normalised as an end)
            // is rerouted to this internal cancel event, so the cancelled stroke is neither
            // committed nor announced via `stroke_completed`/`changed`.
            #[cfg(feature = "touch")]
            event if crate::event::translator::is_touch_cancel(event) => {
                self.cancel_stroke();
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}

impl Draw for SignaturePad {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();

        // The pad surface, its frame, and the empty-state baseline are chrome, so
        // they resolve explicit style first, then the theme's resolved style for
        // this control, and only then a literal. Without the theme step the pad
        // stayed white in both appearances and a switch changed nothing.
        //
        // `self.stroke_color` is deliberately *not* themed: it is the caller's
        // configured ink, which the `stroke_color` property writes, so the caller's
        // value must win over anything the theme says.
        //
        // `resolved_theme_style` takes and releases the global manager's lock
        // internally, so no guard is held across the draw (the mutex is not
        // re-entrant).
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("signature_pad");
        // `signature_pad` is absent from `WidgetRole::for_kind_name`'s table, so it classifies
        // as `Surface` and resolves to `theme.colors.background` — the window's own fill. A pad
        // filled with that colour is byte-identical to the frame behind it, which is why
        // `signature_pad.svg` showed only the baseline and the two `rgba(18,18,18)` rectangles
        // were indistinguishable. The fill is therefore derived one step from the window fill
        // for a `Base`/`Input`-style face, so the writing area reads as a well the ink sits in.
        // A colour the caller set explicitly still wins: it is a deliberate choice, not the
        // window fill arriving through the resolver.
        let window_fill = {
            let manager = crate::style::theme_manager();
            manager.current_theme().map(|active| active.colors.background).unwrap_or(Color::WHITE)
        };
        let text_color = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        let background = match style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
        {
            Some(resolved) if resolved != window_fill => resolved,
            _ => window_fill.blend(&text_color, 0.10),
        };
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .filter(|resolved| *resolved != background)
            .unwrap_or_else(|| background.blend(&text_color, 0.35));
        // The hint line is secondary chrome: derived from the resolved colours so it
        // stays visible against either surface.
        let hint = background.blend(&text_color, 0.4);

        context.face_with_gradient(
            rect,
            background,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().border_radius.unwrap_or(0),
            Color::BLACK,
        );
        context.draw_rect(rect, border);

        // Draw committed strokes.
        for stroke in &self.strokes {
            draw_stroke(
                context,
                stroke,
                self.stroke_color,
                self.stroke_width,
                self.pressure_affects_width,
            );
        }
        // Draw the in-progress stroke on top.
        if let Some(current) = &self.current {
            draw_stroke(
                context,
                current,
                self.stroke_color,
                self.stroke_width,
                self.pressure_affects_width,
            );
        }

        // Empty-state hint: a baseline that reads like a signature line.
        if self.strokes.is_empty() && self.current.is_none() {
            let mid_y = rect.y + rect.height as i32 / 2;
            let inset = rect.width as i32 / 6;
            context.draw_line_stroke(
                Point::new(rect.x + inset, mid_y),
                Point::new(rect.x + rect.width as i32 - inset, mid_y),
                hint,
                1,
            );
        }
    }
}

/// The intermediate points to lay between `from` and `to` so that no drawn segment is
/// longer than `min_distance`, each with a linearly interpolated tip pressure
/// (D09-POINTER-01).
///
/// The count is `ceil(distance / min_distance) - 1`, which spaces the inserted points at
/// the same resolution the pad accepts directly: a slow stroke and a fast one therefore
/// produce strokes of the **same** geometric fidelity, which is the property the old
/// distance-only rule could not express. When `min_distance` is `0` there is no resolution
/// to honour and nothing is inserted. The pressure of an inserted point is the linear blend
/// of the two endpoint pressures, so a stroke that fades from a light touch to a hard press
/// keeps a smooth width ramp instead of stepping.
///
/// # Why a free function
///
/// It is called from the one place that appends to a stroke, and it reads nothing from the
/// widget: taking the threshold as a parameter is what lets the caller hold a mutable
/// borrow of the stroke while the count is computed, and it makes the spacing rule testable
/// without constructing a pad.
fn interpolation_steps_with_pressure(
    from: Point,
    to: Point,
    distance: f32,
    min_distance: f32,
    from_pressure: f32,
    to_pressure: f32,
) -> Vec<(Point, f32)> {
    if min_distance <= 0.0 || !distance.is_finite() || !min_distance.is_finite() {
        return Vec::new();
    }
    let steps = (distance / min_distance).ceil() as usize;
    if steps <= 1 {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(steps - 1);
    for i in 1..steps {
        let t = i as f32 / steps as f32;
        let point = Point::new(
            from.x + ((to.x - from.x) as f32 * t).round() as i32,
            from.y + ((to.y - from.y) as f32 * t).round() as i32,
        );
        let pressure = from_pressure + (to_pressure - from_pressure) * t;
        out.push((point, pressure));
    }
    out
}

/// Draws a single stroke as a connected polyline.
///
/// When `pressure_affects_width` is `true`, each segment's width is `width` scaled by the
/// mean pressure of the points that bound it, floored at 1 so a light touch still leaves
/// visible ink (D09-POINTER-01). A stroke whose points all carry the neutral pressure — every
/// mouse or touch stroke — therefore renders at exactly `width`, so enabling the feature
/// changes nothing for a device that reports no pressure.
fn draw_stroke(
    context: &mut RenderContext,
    stroke: &SignatureStroke,
    color: Color,
    width: u32,
    pressure_affects_width: bool,
) {
    let points = stroke.points();
    let pressures = stroke.pressures();
    match points.len() {
        0 => {}
        1 => {
            // A single point renders as a dot.
            let pressure = pressures.first().copied().unwrap_or(NEUTRAL_PRESSURE);
            let dot_width = segment_width(width, pressure, pressure, pressure_affects_width);
            context.draw_line_stroke_aa(points[0], points[0], color, dot_width.max(2));
        }
        _ => {
            for (index, pair) in points.windows(2).enumerate() {
                let from_pressure = pressures.get(index).copied().unwrap_or(NEUTRAL_PRESSURE);
                let to_pressure = pressures.get(index + 1).copied().unwrap_or(NEUTRAL_PRESSURE);
                let segment =
                    segment_width(width, from_pressure, to_pressure, pressure_affects_width);
                context.draw_line_stroke_aa(pair[0], pair[1], color, segment);
            }
        }
    }
}

/// The drawn width of one segment, derived from the configured `width` and the tip pressure
/// at its two ends (D09-POINTER-01).
///
/// Pressure scales the width linearly and the result is floored at `1`, so a zero-pressure
/// segment still leaves a hairline rather than disappearing. With `pressure_affects_width`
/// off, the configured width is returned unchanged.
fn segment_width(
    width: u32,
    from_pressure: f32,
    to_pressure: f32,
    pressure_affects_width: bool,
) -> u32 {
    if !pressure_affects_width {
        return width.max(1);
    }
    let mean = ((from_pressure + to_pressure) * 0.5).clamp(0.0, 1.0);
    let scaled = (width as f32 * mean).round();
    (scaled as u32).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    fn pad() -> SignaturePad {
        SignaturePad::new(Rect::new(0, 0, 320, 160))
    }

    #[test]
    fn new_pad_is_empty() {
        let p = pad();
        assert!(p.is_empty());
        assert_eq!(p.stroke_count(), 0);
        assert!(p.strokes().is_empty());
    }

    #[test]
    fn a_press_move_release_records_one_stroke() {
        let mut p = pad();
        p.handle_event(&Event::mouse_press(10, 20, 1));
        p.handle_event(&Event::mouse_move(30, 40));
        p.handle_event(&Event::mouse_move(60, 50));
        p.handle_event(&Event::mouse_release(60, 50, 1));

        assert_eq!(p.stroke_count(), 1);
        assert!(!p.is_empty());
        // The **ends** are asserted rather than the length: the moves above are 20 and ~30 px
        // apart, so they are now filled in by interpolation and the count is a function of the
        // distance threshold, not of how many events arrived. What must not change is that the
        // stroke begins at the press and ends at the release.
        let stroke = &p.strokes()[0];
        assert_eq!(stroke.points()[0], Point::new(10, 20));
        assert_eq!(stroke.points().last().copied(), Some(Point::new(60, 50)));
        assert!(stroke.len() >= 3, "the real points survive interpolation");
    }

    #[test]
    fn smoothing_drops_near_duplicate_points() {
        let mut p = pad();
        p.handle_event(&Event::mouse_press(10, 20, 1));
        // Two near-identical points: both within the 1.5px threshold of the
        // previous point, so neither is recorded.
        p.handle_event(&Event::mouse_move(10, 20));
        p.handle_event(&Event::mouse_move(10, 21));
        p.handle_event(&Event::mouse_release(10, 21, 1));

        let stroke = &p.strokes()[0];
        // Only the starting press survived; the near-duplicate moves were dropped.
        assert_eq!(stroke.len(), 1);
        assert_eq!(stroke.points()[0], Point::new(10, 20));
    }

    /// `interpolation_steps` spaces inserted points at the pad's own resolution.
    ///
    /// The count is asserted exactly, because the *rule* is the point: `ceil(d / min) - 1`
    /// inserted points is what makes a fast stroke and a slow one land at the same fidelity.
    #[test]
    fn interpolation_spaces_points_at_the_pads_own_resolution() {
        // 10 px at a 1.5 px threshold: `ceil(10 / 1.5) = 7` steps, so 6 inserted points, the
        // first at `t = 1/7` (x = 1.43 -> 1) and the last at `t = 6/7` (x = 8.57 -> 9).
        let steps = interpolation_steps_with_pressure(
            Point::new(0, 0),
            Point::new(10, 0),
            10.0,
            1.5,
            NEUTRAL_PRESSURE,
            NEUTRAL_PRESSURE,
        );
        assert_eq!(steps.len(), 6);
        assert_eq!(steps[0].0, Point::new(1, 0));
        assert_eq!(steps[5].0, Point::new(9, 0));

        // A hop shorter than the threshold has no interior to fill.
        assert!(interpolation_steps_with_pressure(
            Point::new(0, 0),
            Point::new(1, 0),
            1.0,
            1.5,
            NEUTRAL_PRESSURE,
            NEUTRAL_PRESSURE,
        )
        .is_empty());

        // A zero threshold means "no resolution to honour", not "divide by zero".
        assert!(interpolation_steps_with_pressure(
            Point::new(0, 0),
            Point::new(100, 0),
            100.0,
            0.0,
            NEUTRAL_PRESSURE,
            NEUTRAL_PRESSURE,
        )
        .is_empty());
    }

    /// The interval rule accepts a point that has not moved, once enough time has passed.
    ///
    /// This is the half the old distance-only rule could not express, and the reason a
    /// **stationary** pen still produces a record of how long it rested there.
    #[test]
    fn a_stationary_point_is_recorded_once_the_interval_passes() {
        let mut p = pad();
        // Nothing moves, so the default 10 ms interval is what admits the second point. The
        // test sleeps rather than scaling time, because the clock is the OS's and the point of
        // the rule is that it reads that clock.
        p.handle_event(&Event::mouse_press(10, 20, 1));
        std::thread::sleep(std::time::Duration::from_millis(25));
        p.handle_event(&Event::mouse_move(10, 20));

        let current_points = p.current.as_ref().expect("a stroke is in progress").len();
        assert_eq!(
            current_points, 2,
            "a point held in place past the interval must be recorded, so a pause is visible \
             in the stroke rather than being silently swallowed"
        );
    }

    /// The interval rule can be switched off, restoring the pre-existing distance-only
    /// behaviour, and setting it is reflected by the capability contract.
    #[test]
    fn the_interval_is_configurable_and_reported() {
        let mut p = pad();
        assert_eq!(p.min_point_interval_ms(), 10, "the shipped default");
        p.set_min_point_interval_ms(0);
        assert_eq!(p.min_point_interval_ms(), 0);
        assert_eq!(p.get("min_point_interval_ms").expect("readable"), CapabilityValue::UInt(0));
        p.set("min_point_interval_ms", CapabilityValue::UInt(40)).expect("writable");
        assert_eq!(p.min_point_interval_ms(), 40);
    }

    #[test]
    fn undo_and_clear_work() {
        let mut p = pad();
        p.handle_event(&Event::mouse_press(10, 20, 1));
        p.handle_event(&Event::mouse_release(10, 20, 1));
        assert_eq!(p.stroke_count(), 1);

        assert!(p.undo());
        assert_eq!(p.stroke_count(), 0);
        assert!(!p.undo()); // nothing left to undo

        p.handle_event(&Event::mouse_press(10, 20, 1));
        p.handle_event(&Event::mouse_release(10, 20, 1));
        assert_eq!(p.stroke_count(), 1);
        p.clear();
        assert!(p.is_empty());
    }

    #[test]
    fn changed_and_stroke_completed_signals_fire() {
        let mut p = pad();
        let changed = Arc::new(AtomicUsize::new(0));
        let c = changed.clone();
        p.changed.connect(move || {
            c.fetch_add(1, Ordering::SeqCst);
        });

        let completed = Arc::new(AtomicUsize::new(0));
        let cc = completed.clone();
        p.stroke_completed.connect(move |_| {
            cc.fetch_add(1, Ordering::SeqCst);
        });

        p.handle_event(&Event::mouse_press(10, 20, 1));
        p.handle_event(&Event::mouse_release(10, 20, 1));
        assert_eq!(changed.load(Ordering::SeqCst), 1);
        assert_eq!(completed.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn export_polylines_emits_sentinels() {
        let mut p = pad();
        p.handle_event(&Event::mouse_press(10, 20, 1));
        p.handle_event(&Event::mouse_move(30, 40));
        p.handle_event(&Event::mouse_release(30, 40, 1));
        p.handle_event(&Event::mouse_press(50, 60, 1));
        p.handle_event(&Event::mouse_release(50, 60, 1));

        let flat = p.export_polylines();
        // The structure is what this test is about: two strokes, each terminated by a sentinel.
        // The stroke lengths are a function of the distance threshold now that moves are
        // interpolated, so they are read back rather than hardcoded.
        let first = p.strokes()[0].len();
        let second = p.strokes()[1].len();
        assert_eq!(flat.len(), first + 1 + second + 1);
        assert_eq!(flat[first], Point::new(-1, -1));
        assert_eq!(flat[first + 1 + second], Point::new(-1, -1));
    }

    #[test]
    #[cfg(feature = "touch")]
    fn touch_events_record_strokes_too() {
        let mut p = pad();
        p.handle_event(&crate::event::Event::touch_begin(5, 5, 1));
        p.handle_event(&crate::event::Event::touch_move(40, 40, 1));
        p.handle_event(&crate::event::Event::touch_end(40, 40, 1));
        assert_eq!(p.stroke_count(), 1);
    }

    /// D09-EVT-02: a touch cancel abandons the in-progress stroke without committing it.
    ///
    /// A `TouchCancel` normalised to a `TouchEnd` used to call `end_stroke`, saving the
    /// interrupted stroke and firing `stroke_completed`/`changed`. The internal cancel event
    /// must discard it silently. An earlier committed stroke must be untouched.
    #[test]
    #[cfg(feature = "touch")]
    fn touch_cancel_discards_the_in_progress_stroke() {
        let mut p = pad();

        // One committed stroke.
        p.handle_event(&crate::event::Event::touch_begin(5, 5, 1));
        p.handle_event(&crate::event::Event::touch_move(40, 40, 1));
        p.handle_event(&crate::event::Event::touch_end(40, 40, 1));
        assert_eq!(p.stroke_count(), 1);

        let changed = Arc::new(AtomicUsize::new(0));
        let c = changed.clone();
        p.changed.connect(move || {
            c.fetch_add(1, Ordering::SeqCst);
        });
        let completed = Arc::new(AtomicUsize::new(0));
        let cc = completed.clone();
        p.stroke_completed.connect(move |_| {
            cc.fetch_add(1, Ordering::SeqCst);
        });

        // Begin a second stroke, then cancel it.
        p.handle_event(&crate::event::Event::touch_begin(60, 60, 2));
        p.handle_event(&crate::event::Event::touch_move(80, 80, 2));
        let cancel = crate::event::translator::touch_cancel(crate::core::Point::new(80, 80), 2);
        p.handle_event(&cancel);

        assert_eq!(p.stroke_count(), 1, "a cancelled stroke must not be committed");
        assert_eq!(changed.load(Ordering::SeqCst), 0, "a cancel must not emit `changed`");
        assert_eq!(
            completed.load(Ordering::SeqCst),
            0,
            "a cancel must not emit `stroke_completed`"
        );

        // A normal touch end still commits.
        p.handle_event(&crate::event::Event::touch_begin(90, 90, 3));
        p.handle_event(&crate::event::Event::touch_end(90, 90, 3));
        assert_eq!(p.stroke_count(), 2, "a completed stroke still commits");
    }

    #[test]
    fn stroke_width_is_floored_at_one() {
        let mut p = pad();
        p.set_stroke_width(0);
        assert_eq!(p.stroke_width(), 1);
    }

    /// D09-POINTER-01: a `PointerPress`/`PointerMove`/`PointerRelease` sequence begins,
    /// extends and commits a stroke — the same lifecycle as the mouse and touch paths.
    ///
    /// Before the fix a stroke could only start from `MousePress`/`TouchBegin`, so a pen
    /// contact produced no ink at all.
    #[test]
    fn pointer_events_record_a_stroke() {
        let mut p = pad();
        p.handle_event(&Event::pointer_press(Point::new(10, 20), 1, 0.3, 0.0, 0.0));
        assert!(p.current.is_some(), "a stylus press must begin a stroke");
        p.handle_event(&Event::pointer_move(Point::new(40, 40), 0.5, 0.0, 0.0));
        p.handle_event(&Event::pointer_move(Point::new(80, 50), 0.7, 0.0, 0.0));
        p.handle_event(&Event::pointer_release(Point::new(80, 50), 1, 0.0));

        assert_eq!(p.stroke_count(), 1, "a stylus gesture commits exactly one stroke");
        let stroke = &p.strokes()[0];
        assert_eq!(stroke.points()[0], Point::new(10, 20), "the stroke starts at the press");
        assert_eq!(stroke.points().last().copied(), Some(Point::new(80, 50)));
    }

    /// D09-POINTER-01: the pressure a pen reports is retained per point and queryable, so it
    /// is not silently dropped as it was when `PointerMove` only forwarded the position.
    #[test]
    fn pointer_pressure_is_recorded_per_point_and_retained() {
        let mut p = pad();
        p.handle_event(&Event::pointer_press(Point::new(10, 20), 1, 0.25, 0.0, 0.0));
        assert!((p.last_pressure() - 0.25).abs() < 1e-6, "the press pressure is retained");
        p.handle_event(&Event::pointer_move(Point::new(40, 40), 0.75, 0.0, 0.0));
        assert!((p.last_pressure() - 0.75).abs() < 1e-6, "a move updates the retained pressure");
        p.handle_event(&Event::pointer_release(Point::new(40, 40), 1, 0.9));

        let stroke = &p.strokes()[0];
        assert_eq!(stroke.pressures().len(), stroke.points().len(), "pressure tracks every point");
        // The press and the single accepted move carry the reported pressures; interpolation
        // (if any) blends between them, so the endpoints are exact.
        assert!((stroke.pressures()[0] - 0.25).abs() < 1e-6);
        assert!((stroke.last_pressure() - 0.75).abs() < 1e-6);
        // The release's own pressure is retained on the pad even though it adds no point.
        assert!((p.last_pressure() - 0.9).abs() < 1e-6);
    }

    /// D09-POINTER-01: pen pressure changes the drawn stroke width, so the datum reaches a
    /// **visible** behaviour rather than being recorded and ignored.
    ///
    /// `segment_width` is the drawing kernel, so it is asserted directly: a light press draws
    /// a thinner segment than a hard one at the same configured width, and disabling
    /// pressure-driven width makes both fall back to the configured width.
    #[test]
    fn pressure_scales_the_drawn_segment_width() {
        // 10 px configured: a full-pressure segment stays 10, a light one is thinner.
        assert_eq!(segment_width(10, 1.0, 1.0, true), 10);
        assert_eq!(segment_width(10, 0.2, 0.2, true), 2);
        assert_eq!(segment_width(10, 0.0, 0.0, true), 1, "a zero-pressure segment is a hairline");

        // Pressure off: the configured width wins regardless of the datum.
        assert_eq!(segment_width(10, 0.2, 0.2, false), 10);

        // A stroke's width is derived from the pressure recorded at its points.
        let mut p = pad();
        p.set_stroke_width(10);
        p.handle_event(&Event::pointer_press(Point::new(0, 0), 1, 0.1, 0.0, 0.0));
        p.handle_event(&Event::pointer_move(Point::new(60, 0), 0.1, 0.0, 0.0));
        let light = &p.current.as_ref().expect("in progress").pressures()[0];
        assert!((*light - 0.1).abs() < 1e-6);
        assert_eq!(segment_width(10, *light, *light, true), 1, "a light stroke draws thin");

        // The datum is queryable through the published property contract too.
        assert_eq!(p.get("last_pressure").expect("readable"), CapabilityValue::Float(0.1));
        assert_eq!(p.get("pressure_affects_width").expect("readable"), CapabilityValue::Bool(true));
        p.set("pressure_affects_width", CapabilityValue::Bool(false)).expect("writable");
        assert!(!p.pressure_affects_width());
    }

    /// D09-POINTER-01: the tilt a pen reports is retained rather than dropped, even though
    /// the pad does not yet shape ink from it.
    #[test]
    fn pointer_tilt_is_retained() {
        let mut p = pad();
        p.handle_event(&Event::pointer_press(Point::new(10, 20), 1, 0.5, 0.25, -0.5));
        assert_eq!(p.last_tilt(), (0.25, -0.5));
        assert_eq!(p.get("last_tilt_x").expect("readable"), CapabilityValue::Float(0.25));
        assert_eq!(p.get("last_tilt_y").expect("readable"), CapabilityValue::Float(-0.5));
    }

    /// D09-POINTER-01: a non-tip `PointerPress` (a barrel switch) must not begin a stroke,
    /// matching the mouse arm's primary-button rule.
    #[test]
    fn pointer_press_with_a_non_primary_button_does_not_begin_a_stroke() {
        let mut p = pad();
        p.handle_event(&Event::pointer_press(Point::new(10, 20), 2, 0.5, 0.0, 0.0));
        assert!(p.current.is_none(), "a barrel switch must not draw ink");
    }

    #[test]
    fn disabled_pad_ignores_input() {
        let mut p = pad();
        p.set_enabled(false);
        p.handle_event(&Event::mouse_press(10, 20, 1));
        p.handle_event(&Event::mouse_release(10, 20, 1));
        assert!(p.is_empty());
    }
}
