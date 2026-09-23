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
        Self::drag_sweep(context, center, radius, start, end, color);
    }

    /// Strokes the arc from `start_angle` to `end_angle` as a polyline on the circle.
    ///
    /// The vertex count follows the swept angle, so a narrow threshold band costs a couple
    /// of segments while the 270° track stays smooth. Six degrees per segment is
    /// indistinguishable from a true arc at the radii this control draws at.
    fn drag_sweep(
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
        let points = (0..=segments)
            .map(|segment| {
                let angle = start_angle + step * segment as f32;
                Point::new(
                    center.x + (radius as f32 * angle.cos()).round() as i32,
                    center.y + (radius as f32 * angle.sin()).round() as i32,
                )
            })
            .collect();
        context.execute_command(RenderCommand::DrawPath {
            points,
            closed: false,
            color,
            filled: false,
            width: 1,
        });
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

        // Vertical band the arc may occupy.
        //
        // The reading is written below the pivot, so one of the two has to give when the
        // box is short. Reserving the reading's band up front lets the answer be "the arc
        // shrinks" rather than "half the arc is drawn outside the control and a raster
        // backend clips it away silently". A tall box reserves the whole band; a short one
        // caps it at half the height, because otherwise there would be no arc left to
        // shrink and the control would paint nothing at all (P1). The reading itself is
        // then dropped by its own fit test below rather than being drawn off the edge.
        let reading_reserve = Meter::reading_band();
        let arc_band_height =
            (rect.height as i32 - reading_reserve.min(rect.height as i32 / 2)).max(1);

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
            deg_to_rad(arc_start_deg + arc_sweep_deg * self.normalized_value() + offset);

        // Radius and centre fitted to the sweep's **actual** bounding box, then centred in
        // the control.
        //
        // The 270° sweep runs from 45° to 315° once the −90° phase offset is applied. Its
        // extreme x are `cos 135° = -0.707` (left) and `cos 45° = +0.707` (right), and its
        // extreme y are `sin 45° = +0.707`/`sin 315° = -0.707`, so the drawn box is
        // `1.414 · radius` on **both** axes — a square, biased neither left nor right.
        //
        // Two earlier readings of this were wrong in opposite directions. Deriving the
        // radius from the *height* while centring on the rectangle put the top of the arc a
        // full radius above the centre and past the control's edge. Anchoring the centre at
        // `rect.x + radius + 4` then pinned the whole gauge to the left edge — `meter.svg`
        // drew its arc at x 4..71 inside a 240 px cell — while the sweep's rightmost point
        // still reached a full radius to the right of that centre and left the control.
        //
        // So the radius comes from the drawn band's **half-extent** (half of `1.414 ·
        // radius`), and the centre is placed so the drawn box is centred in the control. The
        // `- 4` at each end is the ring's breathing room plus the half-width of its stroke.
        let ring_radius = ((rect.width.min(rect.height) / 2) as f32 / SWEEP_HALF_EXTENT) as u32;
        let ring_radius = ring_radius.saturating_sub(4).min(arc_band_height as u32 / 2).max(1);
        if ring_radius < 10 {
            return;
        }
        // The tick labels sit inside the ring, so the radius they are placed at is derived
        // from what a label actually measures rather than from one more magic subtraction.
        // `tick_inner.saturating_sub(10)` rarely left a radius and was `.max(1)`, which let
        // the fit test below drop every label — `meter.svg` drew the arc and the needle with
        // no scale on them at all.
        let tick_label_font = Font::simple("Sans", 9.0);
        let tick_outer = ring_radius.saturating_sub(TICK_MARK_LENGTH).max(1);
        let label_half = context.measure_text("000", &tick_label_font).width.max(1) / 2;
        let label_radius =
            ring_radius.saturating_sub(TICK_MARK_LENGTH).saturating_sub(label_half).max(1);
        let radius = ring_radius;
        // The centre. The drawn horizontal box is `[center.x - 0.707r, center.x + 0.707r]`, so
        // the centre is placed by centring that box in the control — and the vertical box is
        // the same size, so the centre sits one half-extent below the control's own middle.
        // The clamp keeps the whole drawn box inside the control when the radius is small
        // relative to the rectangle it was offered.
        let half_extent = (ring_radius as f32 * SWEEP_HALF_EXTENT).ceil() as i32;
        let max_center_x = (rect.x + rect.width as i32 - half_extent).max(rect.x + half_extent);
        let center_x = (rect.x + rect.width as i32 / 2).clamp(rect.x + half_extent, max_center_x);
        let max_center_y = (rect.y + rect.height as i32 - half_extent).max(rect.y + half_extent);
        let center_y = (rect.y + rect.height as i32 / 2).clamp(rect.y + half_extent, max_center_y);
        let center = Point::new(center_x, center_y);

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
        let style = self.style().clone();
        let theme = crate::style::resolved_theme_style("meter");
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
            Self::drag_sweep(context, center, radius, band_start, band_end, band.color);
        }

        // Draw the value arc (colored arc from start to value position).
        //
        // Its angle range is clipped to the value rather than its radius re-fitted, which is
        // what keeps it concentric with the track it is drawn over; its end is snapped onto the
        // same grid for the reason at the bands above.
        let value_end = Self::snap_to_grid(value_angle, start_angle);
        if self.value > self.min && (value_end - start_angle).abs() > 0.001 {
            Self::drag_sweep(context, center, radius, start_angle, value_end, value_arc_color);
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
            let label_font = tick_label_font.clone();

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
                    let fraction = i as f32 / (self.tick_count - 1) as f32;
                    let text = format!("{}{}", self.value_at_fraction(fraction), self.unit);
                    let metrics = context.measure_text(&text, &label_font);
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
                    let label_x = lx - metrics.width as i32 / 2;
                    let label_y = ly - metrics.height as i32 / 2;
                    let fits = label_x >= rect.x
                        && label_y >= rect.y
                        && label_x + metrics.width as i32 <= rect.x + rect.width as i32
                        && label_y + metrics.height as i32 <= rect.y + rect.height as i32;
                    if fits {
                        context.draw_text(
                            Point::new(label_x, label_y),
                            &text,
                            &label_font,
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
        let text = self.value_text();
        let value_font = Font::simple("Sans", 12.0);
        let metrics = context.measure_text(&text, &value_font);
        let band_top = rect.y + rect.height as i32 - reading_reserve;
        let value_y = band_top + (reading_reserve - metrics.height as i32) / 2;
        if value_y >= rect.y && value_y + metrics.height as i32 <= rect.y + rect.height as i32 {
            context.draw_text(
                Point::new(center.x - metrics.width as i32 / 2, value_y),
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
        let _guard = crate::theme::theme_test_guard();
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
}
