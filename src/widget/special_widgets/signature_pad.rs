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
use crate::widget::capability::coercion::{expect_f64, expect_u32_saturating, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use core::cell::{Ref, RefCell};
use core::fmt::Write as _;

/// Hands out the identities the render layer cache uses to recognise unchanged committed ink.
///
/// Globally unique because the cache is per surface: two pads that both started at `1` would
/// reuse each other's cached pixels.
fn next_ink_cache_key() -> u64 {
    use core::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

/// A single continuous pen stroke, stored as a polyline of points.
///
/// Each point carries the normalized tip pressure the device reported when the
/// point was taken, so a stroke records not just *where* the pen went but *how
/// hard* it pressed. Mouse and touch input report a neutral pressure (see
/// [`SignatureStroke::push`]), so a stroke drawn with them is uniform.
#[derive(Debug, Clone)]
pub struct SignatureStroke {
    points: Vec<Point>,
    /// Normalized tip pressure (`0.0..=1.0`) for the point at the same index in
    /// `points`. Always the same length as `points`.
    pressures: Vec<f32>,
    /// Stylus tilt in degrees, `(x, y)`, for the point at the same index.
    tilts: Vec<(f32, f32)>,
    /// The smoothed polyline, computed once per stroke (SP-P63).
    ///
    /// Chaikin subdivision is a pure function of `points`, yet the paint ran it for every
    /// committed stroke on every frame — two temporary vectors per stroke per frame. Caching the
    /// result here means a finished stroke is smoothed on its first paint and never again; the
    /// cache is cleared whenever a point is appended, so an in-progress stroke cannot go stale.
    smoothed: RefCell<Option<Vec<Point>>>,
}

/// The pressure a mouse or touch contact reports: neutral, so a stroke drawn with
/// it keeps the pad's configured width (D09-POINTER-01).
const NEUTRAL_PRESSURE: f32 = 1.0;

/// How many committed strokes a pad keeps by default.
///
/// A pad that stays open for a whole shift accumulates one stroke per pen lift with nothing
/// releasing them, so the history needs a bound the same way an undo stack does.
const DEFAULT_STROKE_LIMIT: usize = 1024;

impl PartialEq for SignatureStroke {
    /// The smoothing cache takes no part in equality: it is derived state, and two strokes with
    /// the same recorded points are the same stroke whether or not one of them has been painted.
    fn eq(&self, other: &Self) -> bool {
        self.points == other.points
            && self.pressures == other.pressures
            && self.tilts == other.tilts
    }
}

impl SignatureStroke {
    /// Creates an empty stroke.
    pub fn new() -> Self {
        Self {
            points: Vec::new(),
            pressures: Vec::new(),
            tilts: Vec::new(),
            smoothed: RefCell::new(None),
        }
    }

    /// Creates a stroke from a list of points.
    ///
    /// The points carry a neutral pressure, so a stroke built this way renders at
    /// the pad's configured width. Use [`SignatureStroke::push_with_pressure`] to
    /// attach real per-point pressure.
    pub fn from_points(points: Vec<Point>) -> Self {
        let pressures = vec![NEUTRAL_PRESSURE; points.len()];
        let tilts = vec![(0.0, 0.0); points.len()];
        Self { points, pressures, tilts, smoothed: RefCell::new(None) }
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
        self.tilts.push((0.0, 0.0));
        // The cached subdivision no longer describes this stroke.
        *self.smoothed.borrow_mut() = None;
    }

    /// Appends a point with the pen's pressure and tilt.
    ///
    /// Tilt is stored per point rather than per stroke, so a stroke that turns from an upright
    /// pen to a flat one changes weight along its length. A non-finite angle is stored as zero
    /// rather than rejected, because a device that reports no tilt is the common case and must
    /// draw identically to one that reports "upright".
    pub fn push_with_tilt(&mut self, point: Point, pressure: f32, tilt: (f32, f32)) {
        self.push_with_pressure(point, pressure);
        if let Some(last) = self.tilts.last_mut() {
            *last = (
                if tilt.0.is_finite() { tilt.0 } else { 0.0 },
                if tilt.1.is_finite() { tilt.1 } else { 0.0 },
            );
        }
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

    /// Returns the tilt recorded for each point, in the same order and of the same length
    /// as [`Self::points`].
    pub fn tilts(&self) -> &[(f32, f32)] {
        &self.tilts
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

    /// The stroke's points after the smoothing pass, computed once and reused.
    pub(crate) fn smoothed_points(&self) -> Ref<'_, Vec<Point>> {
        if self.smoothed.borrow().is_none() {
            *self.smoothed.borrow_mut() = Some(chaikin(&self.points));
        }
        Ref::map(self.smoothed.borrow(), |cached| {
            cached.as_ref().expect("the smoothed polyline was just computed")
        })
    }

    /// Returns true when the stroke has no points.
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// A stable, parseable summary of the stroke.
    ///
    /// The published `stroke_completed` payload used to be `Debug` output — `SignatureStroke { … }`
    /// with a `Point { x: 42, y: 17 }` per point — which is not a value a subscriber can read: the
    /// spelling is the struct's own and would change with any field added. This is the contract: four
    /// `key=value` pairs separated by `;`, in this order, numbers formatted with exactly three
    /// decimals.
    ///
    /// ```text
    /// points=<count>;pressure_avg=<f32>;pressure_min=<f32>;pressure_max=<f32>
    /// ```
    ///
    /// An empty stroke reports `points=0` and the neutral pressure for all three figures, so a
    /// consumer never has to special-case a missing value. Coordinates are deliberately not included:
    /// a full point list is what [`Self::points`] is for, and flattening it into a string would be the
    /// same lossy-in-the-other-direction mistake.
    pub fn summary(&self) -> String {
        if self.pressures.is_empty() {
            return format!(
                "points=0;pressure_avg={NEUTRAL_PRESSURE:.3};pressure_min={NEUTRAL_PRESSURE:.3};pressure_max={NEUTRAL_PRESSURE:.3}"
            );
        }
        let mut min = f32::INFINITY;
        let mut max = f32::NEG_INFINITY;
        let mut total = 0.0f32;
        for pressure in &self.pressures {
            min = min.min(*pressure);
            max = max.max(*pressure);
            total += *pressure;
        }
        let average = total / self.pressures.len() as f32;
        format!(
            "points={};pressure_avg={average:.3};pressure_min={min:.3};pressure_max={max:.3}",
            self.points.len()
        )
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
    /// Whether stylus tilt widens the ink.
    tilt_affects_width: bool,
    /// Whether strokes are drawn through a smoothing subdivision.
    smooth_strokes: bool,
    /// Strokes removed by `undo`, available to `redo`.
    redo_stack: Vec<SignatureStroke>,
    /// How many committed strokes the pad keeps; past it the oldest stroke is evicted.
    ///
    /// The same bound applies to `redo_stack`, matching `Canvas::set_command_limit`.
    stroke_limit: usize,
    /// Render-cache identity of the committed ink; a new identity rebuilds its pixels.
    ink_cache_key: u64,
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
            min_point_distance: 1.0,
            min_point_interval_ms: 10,
            last_point_at: None,
            last_pressure: NEUTRAL_PRESSURE,
            last_tilt: (0.0, 0.0),
            pressure_affects_width: true,
            tilt_affects_width: false,
            smooth_strokes: false,
            redo_stack: Vec::new(),
            stroke_limit: DEFAULT_STROKE_LIMIT,
            ink_cache_key: next_ink_cache_key(),
            changed: GenericSignal::new(),
            stroke_completed: Signal1::new(),
        }
    }

    /// Drops the committed-ink render cache, because what it painted is no longer current.
    fn invalidate_ink_cache(&mut self) {
        self.ink_cache_key = next_ink_cache_key();
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
        if let Some(stroke) = self.strokes.pop() {
            self.redo_stack.push(stroke);
            self.invalidate_ink_cache();
            self.changed.emit();
            self.base.request_redraw();
            true
        } else {
            false
        }
    }

    /// Re-applies the stroke most recently removed by [`Self::undo`].
    ///
    /// A new stroke clears the redo history, which is the usual linear-history contract:
    /// once the user draws again, the branch that `undo` left behind is no longer reachable.
    pub fn redo(&mut self) -> bool {
        let Some(stroke) = self.redo_stack.pop() else {
            return false;
        };
        self.strokes.push(stroke);
        self.invalidate_ink_cache();
        self.changed.emit();
        self.base.request_redraw();
        true
    }

    /// Sets how many committed strokes the pad retains (clamped to at least one).
    ///
    /// The undo and redo histories share the bound, so neither can grow without limit. Lowering
    /// the limit evicts the oldest strokes immediately, and the committed ink changes with them,
    /// so the render cache is dropped and `changed` is emitted.
    pub fn set_stroke_limit(&mut self, limit: usize) {
        let limit = limit.max(1);
        if self.stroke_limit == limit {
            return;
        }
        self.stroke_limit = limit;
        let retained = self.strokes.len();
        self.trim_stroke_history();
        if self.strokes.len() != retained {
            self.invalidate_ink_cache();
            self.changed.emit();
        }
        self.base.request_redraw();
    }

    /// Evicts the oldest strokes and redo entries past the stroke limit.
    fn trim_stroke_history(&mut self) {
        if self.strokes.len() > self.stroke_limit {
            let excess = self.strokes.len() - self.stroke_limit;
            self.strokes.drain(..excess);
        }
        if self.redo_stack.len() > self.stroke_limit {
            let excess = self.redo_stack.len() - self.stroke_limit;
            self.redo_stack.drain(..excess);
        }
    }

    /// Enables drawing through a smoothing subdivision.
    ///
    /// Sampling produces a polyline whose corners are real: a fast signature reads as a chain
    /// of straight segments. Smoothing subdivides that polyline before it is stroked, which
    /// rounds the corners without moving the recorded points. It is opt-in because it changes
    /// the ink, and a caller comparing exported geometry must get what was captured.
    pub fn set_smooth_strokes(&mut self, smooth: bool) {
        if self.smooth_strokes == smooth {
            return;
        }
        self.smooth_strokes = smooth;
        self.invalidate_ink_cache();
        self.base.request_redraw();
    }

    /// Returns whether strokes are smoothed.
    pub fn smooth_strokes(&self) -> bool {
        self.smooth_strokes
    }

    /// Makes stylus tilt widen the ink.
    pub fn set_tilt_affects_width(&mut self, affects: bool) {
        if self.tilt_affects_width == affects {
            return;
        }
        self.tilt_affects_width = affects;
        self.invalidate_ink_cache();
        self.base.request_redraw();
    }

    /// Returns whether tilt widens the ink.
    pub fn tilt_affects_width(&self) -> bool {
        self.tilt_affects_width
    }

    /// Exports the committed strokes as an SVG document in the pad's own coordinate space.
    ///
    /// Each stroke is one `<path>` with a round cap and join, so a caller can embed the
    /// signature in a document without the widget. The pad's surface is not painted: an
    /// exported signature is ink on a transparent background.
    pub fn export_svg(&self) -> String {
        let rect = self.geometry();
        // One output buffer, written in place: the old form built a `path` string with a fresh
        // `format!` per point and then copied it into the document, so a long signature allocated
        // twice per point (SP-105). `write!` renders the number directly into the document.
        let mut svg = String::with_capacity(64 + self.strokes.len() * 96);
        let _ = write!(
            svg,
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"{} {} {} {}\">",
            rect.width, rect.height, rect.x, rect.y, rect.width, rect.height
        );
        for stroke in &self.strokes {
            let Some(first) = stroke.points().first() else {
                continue;
            };
            let _ = write!(svg, "<path d=\"M {} {}", first.x, first.y);
            for point in &stroke.points()[1..] {
                let _ = write!(svg, " L {} {}", point.x, point.y);
            }
            let _ = writeln!(
                svg,
                "\" fill=\"none\" stroke=\"rgb({},{},{})\" stroke-width=\"{}\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/>",
                self.stroke_color.r,
                self.stroke_color.g,
                self.stroke_color.b,
                self.stroke_width.max(1)
            );
        }
        svg.push_str("</svg>");
        svg
    }

    /// Rasterises the committed strokes into a transparent RGBA buffer.
    ///
    /// The buffer is `size` pixels, with full coverage on inked pixels: a signature export is
    /// ink, not antialiasing, which keeps the rasteriser dependency-free and its output
    /// deterministic. The caller can hand the bytes to any PNG encoder it already has.
    pub fn export_rgba(&self, size: (u32, u32)) -> Vec<u8> {
        let (width, height) = size;
        let mut pixels = vec![0u8; width as usize * height as usize * 4];
        if width == 0 || height == 0 {
            return pixels;
        }
        let rect = self.geometry();
        let scale_x = width as f32 / rect.width.max(1) as f32;
        let scale_y = height as f32 / rect.height.max(1) as f32;
        for stroke in &self.strokes {
            let points: Vec<(i32, i32)> = stroke
                .points()
                .iter()
                .map(|point| {
                    (
                        ((point.x - rect.x) as f32 * scale_x).round() as i32,
                        ((point.y - rect.y) as f32 * scale_y).round() as i32,
                    )
                })
                .collect();
            for pair in points.windows(2) {
                raster_line(&mut pixels, width, height, pair[0], pair[1], self.stroke_color);
            }
            if points.len() == 1 {
                raster_line(&mut pixels, width, height, points[0], points[0], self.stroke_color);
            }
        }
        pixels
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
            self.invalidate_ink_cache();
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
        if self.stroke_color == color {
            return;
        }
        self.stroke_color = color;
        self.invalidate_ink_cache();
        self.base.request_redraw();
    }

    /// Returns the pen stroke width in pixels.
    pub fn stroke_width(&self) -> u32 {
        self.stroke_width
    }

    /// Sets the pen stroke width, floored at 1, and requests a redraw.
    pub fn set_stroke_width(&mut self, width: u32) {
        let width = width.max(1);
        if self.stroke_width == width {
            return;
        }
        self.stroke_width = width;
        self.invalidate_ink_cache();
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
            self.invalidate_ink_cache();
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
        stroke.push_with_tilt(point, self.last_pressure, self.last_tilt);
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
        let new_tilt = self.last_tilt;

        let Some(current) = self.current.as_mut() else {
            return;
        };
        let Some(last) = current.points().last().copied() else {
            // A stroke always begins with a point, but a caller-built empty `current`
            // must not silently swallow the move.
            current.push_with_tilt(point, new_pressure, new_tilt);
            self.last_point_at = Some(now);
            self.base.request_redraw();
            return;
        };

        let dx = (point.x - last.x) as f32;
        let dy = (point.y - last.y) as f32;
        let distance = (dx * dx + dy * dy).sqrt();
        // Each condition can be switched off by its own zero: `min_distance == 0` means "no
        // distance limit" and `min_interval_ms == 0` means "no time requirement". Treating the
        // interval of zero as *always satisfied* recorded every event of a stationary pointer,
        // which is the opposite of the density control a caller sets (SP-P64).
        let far_enough = min_distance <= 0.0 || distance >= min_distance;
        let long_enough = min_interval_ms > 0 && elapsed_ms >= min_interval_ms;
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
        //
        // The distance is the whole condition: `min_interval_ms == 0` means "no time limit",
        // an *unrestricted* setting rather than a switched-off one. Gating the fill on
        // `min_interval_ms > 0` read that `0` as "do not fill", so a caller who asked for
        // distance-only control got the sparsest possible sampling instead (SP-P64).
        if min_distance > 0.0 && distance > min_distance {
            let from_pressure = current.last_pressure();
            for (step, step_pressure) in interpolation_steps_with_pressure(
                last,
                point,
                distance,
                min_distance,
                from_pressure,
                new_pressure,
            ) {
                current.push_with_tilt(step, step_pressure, new_tilt);
            }
        }
        current.push_with_tilt(point, new_pressure, new_tilt);
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
        // Drawing again abandons the undone branch, which is the linear-history contract
        // `redo` documents.
        self.redo_stack.clear();
        self.trim_stroke_history();
        self.invalidate_ink_cache();
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
                    // A documented key=value summary rather than the struct's `Debug` spelling; see
                    // `SignatureStroke::summary` for the grammar.
                    CapabilityValue::String(v.summary())
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
                self.set_stroke_width(expect_u32_saturating(value)?);
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
            // `redo` and `clear` are payload-free and already exist as methods; only `undo` was
            // routed, so a designer could publish an Undo affordance and nothing else even though
            // the capability advertises all three names.
            "redo" => {
                let _ = self.redo();
                Ok(())
            }
            "clear" => {
                self.clear();
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
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("signature_pad");
        // `signature_pad` is absent from `WidgetRole::for_kind_name`'s table, so it classifies
        // as `Surface` and resolves to `theme.colors.background` — the window's own fill. A pad
        // filled with that colour is byte-identical to the frame behind it, which is why
        // `signature_pad.svg` showed only the baseline and the two `rgba(18,18,18)` rectangles
        // were indistinguishable. The fill is therefore derived one step from the window fill
        // for a `Base`/`Input`-style face, so the writing area reads as a well the ink sits in.
        // A colour the caller set explicitly still wins: it is a deliberate choice, not the
        // window fill arriving through the resolver.
        let window_fill =
            crate::style::palette_shared().map(|colors| colors.background).unwrap_or(Color::WHITE);
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

        context.face_with_gradient_corners(
            rect,
            background,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().resolved_corner_radii().to_array(),
            Color::BLACK,
        );
        crate::widget::draw::stroke_surface_border(
            context,
            rect,
            self.style().resolved_corner_radii().to_array(),
            border,
            1,
        );

        // Draw committed strokes inside one render-cached layer group: their ink only changes
        // when the stroke set, the pen settings or the transform do, so a still pad re-uses the
        // previous frame's pixels instead of replaying every point (SP-P63).
        if !self.strokes.is_empty() {
            let key = self.ink_cache_key;
            context.push_layer_group(rect, None, 1.0, Some(key));
            for stroke in &self.strokes {
                draw_stroke(
                    context,
                    stroke,
                    self.stroke_color,
                    self.stroke_width,
                    self.pressure_affects_width,
                    self.tilt_affects_width,
                    self.smooth_strokes,
                );
            }
            context.pop_layer();
        }
        // Draw the in-progress stroke on top.
        if let Some(current) = &self.current {
            draw_stroke(
                context,
                current,
                self.stroke_color,
                self.stroke_width,
                self.pressure_affects_width,
                self.tilt_affects_width,
                self.smooth_strokes,
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
    tilt_affects_width: bool,
    smooth: bool,
) {
    // The smoothing pass is a pure function of the stroke's points, so the stroke caches it: a
    // committed stroke was re-subdivided on every frame (SP-P63).
    let smoothed;
    let points = if smooth {
        smoothed = stroke.smoothed_points();
        smoothed.as_slice()
    } else {
        stroke.points()
    };
    let pressures = stroke.pressures();
    let tilts = stroke.tilts();
    match points.len() {
        0 => {}
        1 => {
            // A single point renders as a dot.
            let pressure = pressures.first().copied().unwrap_or(NEUTRAL_PRESSURE);
            let tilt = tilts.first().copied().unwrap_or((0.0, 0.0));
            let dot_width = segment_width(width, pressure, pressure, pressure_affects_width)
                * tilt_factor(tilt, tilt_affects_width);
            context.draw_line_stroke_aa(points[0], points[0], color, dot_width.max(2));
        }
        _ => {
            for (index, pair) in points.windows(2).enumerate() {
                let from_pressure = pressures.get(index).copied().unwrap_or(NEUTRAL_PRESSURE);
                let to_pressure = pressures.get(index + 1).copied().unwrap_or(NEUTRAL_PRESSURE);
                let from_tilt = tilts.get(index).copied().unwrap_or((0.0, 0.0));
                let to_tilt = tilts.get(index + 1).copied().unwrap_or((0.0, 0.0));
                let segment =
                    segment_width(width, from_pressure, to_pressure, pressure_affects_width);
                let tilt = ((from_tilt.0 + to_tilt.0) * 0.5, (from_tilt.1 + to_tilt.1) * 0.5);
                let segment = segment * tilt_factor(tilt, tilt_affects_width);
                context.draw_line_stroke_aa(pair[0], pair[1], color, segment.max(1));
            }
        }
    }
}

/// The width multiplier a stylus tilt contributes.
///
/// A pen laid over widens its contact patch, so the angle between upright and flat is mapped
/// to `1.0..=2.0`. With the setting off (the default) the factor is exactly one, so a device
/// that reports tilt draws identically to one that does not.
fn tilt_factor(tilt: (f32, f32), enabled: bool) -> u32 {
    if !enabled {
        return 1;
    }
    let magnitude = (tilt.0.abs() + tilt.1.abs()).min(180.0);
    let factor = 1.0 + magnitude / 180.0;
    factor.round().max(1.0) as u32
}

/// A Chaikin corner-cutting pass over a polyline.
///
/// Each segment contributes two points at a quarter and three quarters along it, so the
/// original corners are replaced by a rounded path. Endpoints are preserved, which is what
/// keeps the ink anchored where the pen touched down and lifted.
fn chaikin(points: &[Point]) -> Vec<Point> {
    if points.len() < 3 {
        return points.to_vec();
    }
    let mut out = Vec::with_capacity(points.len() * 2);
    out.push(points[0]);
    for pair in points.windows(2) {
        let (from, to) = (pair[0], pair[1]);
        out.push(Point::new(
            from.x + ((to.x - from.x) as f32 * 0.25).round() as i32,
            from.y + ((to.y - from.y) as f32 * 0.25).round() as i32,
        ));
        out.push(Point::new(
            from.x + ((to.x - from.x) as f32 * 0.75).round() as i32,
            from.y + ((to.y - from.y) as f32 * 0.75).round() as i32,
        ));
    }
    out.push(points[points.len() - 1]);
    out
}

/// Stamps one line into an RGBA buffer.
///
/// Bresenham's integer walk, with one pixel per step: the export is a signature's ink, so a
/// deterministic centreline is more useful than an antialiased edge the caller did not ask
/// for.
fn raster_line(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    from: (i32, i32),
    to: (i32, i32),
    color: Color,
) {
    let (mut x0, mut y0) = from;
    let (x1, y1) = to;
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut error = dx + dy;
    loop {
        if x0 >= 0 && y0 >= 0 && (x0 as u32) < width && (y0 as u32) < height {
            let index = ((y0 as u32 * width + x0 as u32) * 4) as usize;
            if index + 3 < pixels.len() {
                pixels[index] = color.r;
                pixels[index + 1] = color.g;
                pixels[index + 2] = color.b;
                pixels[index + 3] = 255;
            }
        }
        if x0 == x1 && y0 == y1 {
            break;
        }
        let double = 2 * error;
        if double >= dy {
            error += dy;
            x0 += sx;
        }
        if double <= dx {
            error += dx;
            y0 += sy;
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
        // Distance-only sampling: the time rule is switched off, so the assertion does not depend
        // on how long the test takes to run, and this is the "no time limit" case SP-P64 fixed.
        p.set_min_point_distance(1.5);
        p.set_min_point_interval_ms(0);
        p.handle_event(&Event::mouse_press(10, 20, 1));
        // Two near-identical points: both within the 1.5 px threshold of the
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

        // The datum is queryable through the published property contract too. The value
        // round-trips through the property channel as `f64` (the capability value kind),
        // so compare within a tolerance rather than by bit equality: `0.1_f32` widened to
        // `f64` is `0.10000000149...`, not the literal `0.1`.
        match p.get("last_pressure").expect("readable") {
            CapabilityValue::Float(value) => assert!(
                (value - 0.1).abs() < 1e-6,
                "last_pressure must read back the retained datum, got {value}"
            ),
            other => panic!("last_pressure must be a Float, got {other:?}"),
        }
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

    #[test]
    fn command_routes_redo_and_clear() {
        let mut p = pad();
        p.strokes.push(SignatureStroke::from_points(vec![Point::new(1, 1)]));
        assert!(p.undo());
        assert_eq!(p.stroke_count(), 0);

        p.command("redo").expect("redo is command-routed");
        assert_eq!(p.stroke_count(), 1);

        p.command("clear").expect("clear is command-routed");
        assert_eq!(p.stroke_count(), 0);
    }

    #[test]
    fn stroke_summary_is_a_documented_key_value_string() {
        let mut stroke = SignatureStroke::new();
        stroke.push_with_pressure(Point::new(1, 1), 0.25);
        stroke.push_with_pressure(Point::new(2, 2), 1.0);
        let summary = stroke.summary();
        assert_eq!(
            summary, "points=2;pressure_avg=0.625;pressure_min=0.250;pressure_max=1.000",
            "the payload's grammar must stay stable"
        );

        // An empty stroke reports zeros and the neutral pressure rather than a missing value.
        assert_eq!(
            SignatureStroke::new().summary(),
            "points=0;pressure_avg=1.000;pressure_min=1.000;pressure_max=1.000"
        );
    }

    /// SP-105: the single-pass SVG writer emits the same document the per-point form did.
    #[test]
    fn svg_export_writes_one_path_per_stroke() {
        let mut p = pad();
        p.strokes.push(SignatureStroke::from_points(vec![
            Point::new(1, 2),
            Point::new(3, 4),
            Point::new(5, 6),
        ]));
        // An empty stroke contributes nothing rather than an empty path.
        p.strokes.push(SignatureStroke::new());
        p.strokes.push(SignatureStroke::from_points(vec![Point::new(7, 8)]));

        let svg = p.export_svg();
        assert_eq!(
            svg,
            concat!(
                "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"320\" height=\"160\" ",
                "viewBox=\"0 0 320 160\">",
                "<path d=\"M 1 2 L 3 4 L 5 6\" fill=\"none\" stroke=\"rgb(20,20,20)\" ",
                "stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/>\n",
                "<path d=\"M 7 8\" fill=\"none\" stroke=\"rgb(20,20,20)\" ",
                "stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/>\n",
                "</svg>"
            )
        );
    }

    /// SPX-X10: the committed-stroke history is bounded, evicting the oldest strokes first.
    #[test]
    fn the_stroke_limit_evicts_the_oldest_strokes() {
        let mut p = pad();
        p.set_stroke_limit(2);
        for index in 0..3 {
            let mut stroke = SignatureStroke::new();
            stroke.push(Point::new(index * 10, index * 10));
            p.current = Some(stroke);
            p.end_stroke();
        }
        assert_eq!(p.stroke_count(), 2);
        assert_eq!(p.strokes()[0].points()[0], Point::new(10, 10));
        assert_eq!(p.strokes()[1].points()[0], Point::new(20, 20));

        // Lowering the limit below the current count evicts immediately.
        p.set_stroke_limit(1);
        assert_eq!(p.stroke_count(), 1);
        assert_eq!(p.strokes()[0].points()[0], Point::new(20, 20));
    }
}
