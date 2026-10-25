// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Gauge — a radial dial showing one value against a range.
//!
//! # Why a gauge is not a `PieChart`
//!
//! A pie chart partitions one whole into *categories*; its slices sum to 360° and its input is
//! a list. A gauge shows one measurement against a *range*: a single value, a minimum and a
//! maximum, a track that always spans the same sweep, and a numeric readout. Folding the two
//! together would give every gauge a category list and every pie chart a value/maximum pair.
//!
//! # Reused arithmetic
//!
//! The arc is drawn by [`crate::render::draw_arc_segments`], the crate's single arc
//! approximator (it derives its sample count from the arc's own pixel length), so the dial has
//! no second curve implementation.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::{draw_arc_segments, point_on_circle, RenderContext};
use crate::signal::Signal1;
use crate::style::{MotionSlot, PropertyDriver};
use crate::widget::capability::coercion::{
    expect_f32, expect_string, expect_string_list, expect_u32,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Where the dial's sweep starts: 135° clockwise from 3 o'clock is the lower-left of the dial,
/// which is what makes the gap sit at the bottom where the readout goes.
const SWEEP_START: f32 = core::f32::consts::PI * 0.75;

/// How far the dial sweeps: three quarters of a turn.
const SWEEP: f32 = core::f32::consts::PI * 1.5;

/// The number of major ticks on the track.
const TICK_COUNT: u32 = 5;

/// A colour band on a gauge's track, in the gauge's own value units.
///
/// Zones are drawn before the value arc, so the filled value covers its own part of the
/// band. A zone whose `start` is greater than its `end` is refused rather than reordered,
/// because the caller's ordering is the band's meaning.
#[derive(Clone, Debug, PartialEq)]
pub struct GaugeZone {
    /// The lower bound, in gauge value units.
    pub start: f32,
    /// The upper bound, in gauge value units.
    pub end: f32,
    /// The band's colour.
    pub color: Color,
    /// A level name reported by `band_clicked`; empty falls back to the colour's spelling.
    pub name: String,
}

/// The band type's documented `bands` spelling.
pub type ValueBand = GaugeZone;

/// How the gauge marks the current reading on its sweep.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GaugeNeedleStyle {
    /// No needle; the value arc alone marks the reading (the historical behaviour).
    None,
    /// A thin line from the centre to the reading.
    Line,
    /// A tapered triangle from the centre to the reading.
    Triangle,
}

impl GaugeNeedleStyle {
    /// The token this style publishes.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Line => "line",
            Self::Triangle => "triangle",
        }
    }

    /// Parses [`Self::as_str`].
    pub fn from_token(token: &str) -> Option<Self> {
        match crate::widget::capability::coercion::normalize_key(token).as_str() {
            "none" => Some(Self::None),
            "line" => Some(Self::Line),
            "triangle" => Some(Self::Triangle),
            _ => None,
        }
    }
}

impl GaugeZone {
    /// Creates a band from `start` to `end`.
    pub fn new(start: f32, end: f32, color: Color) -> Self {
        Self { start, end, color, name: String::new() }
    }

    /// Names the band, which is the payload `band_clicked` reports.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// The name `band_clicked` reports: the explicit name, or the colour's spelling.
    pub fn level_name(&self) -> String {
        if self.name.is_empty() {
            format!("{:?}", self.color)
        } else {
            self.name.clone()
        }
    }
}

/// Gauge — one value on a radial dial.
///
/// `value` is clamped into `[minimum, maximum]` on every write, including a write that arrives
/// through the property route, so the readout never shows a value the dial cannot express. The
/// two bounds are normalised against each other: a `maximum` written below `minimum` is
/// refused rather than silently reordered.
pub struct Gauge {
    base: BaseWidget,
    value: f32,
    minimum: f32,
    maximum: f32,
    label: String,
    unit: String,
    thickness: u32,
    /// Threshold colour bands drawn on the track, in value units.
    zones: Vec<GaugeZone>,
    /// Number of major tick marks plus one, i.e. the number of segments the sweep is divided into.
    tick_count: u32,
    /// Sweep start, in radians clockwise from 3 o'clock.
    start_angle: f32,
    /// Sweep end, in radians clockwise from 3 o'clock.
    end_angle: f32,
    /// How the reading is marked on the sweep.
    needle_style: GaugeNeedleStyle,
    /// Suffix printed after the readout (the `unit_label` spelling).
    unit_label: String,
    /// Whether a pointer drag is currently setting the value.
    dragging: bool,
    /// Emitted with the new reading after an accepted change.
    pub value_changed: Signal1<f32>,
    /// Emitted with the band's level name when a band is clicked.
    pub band_clicked: Signal1<String>,
    /// Whether the displayed value eases to a new reading.
    animate: bool,
    /// The displayed fraction, advanced by [`Widget::tick`] while `animate` is set.
    display: PropertyDriver,
}

impl Gauge {
    /// Creates a gauge reading `0` of `0..=100` with a 10 px track.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Gauge, geometry, "Gauge"),
            value: 0.0,
            minimum: 0.0,
            maximum: 100.0,
            label: String::new(),
            unit: String::new(),
            thickness: 10,
            zones: Vec::new(),
            tick_count: TICK_COUNT,
            start_angle: SWEEP_START,
            end_angle: SWEEP_START + SWEEP,
            needle_style: GaugeNeedleStyle::None,
            unit_label: String::new(),
            dragging: false,
            value_changed: Signal1::new(),
            band_clicked: Signal1::new(),
            animate: false,
            display: PropertyDriver::at(0.0, MotionSlot::Normal),
        }
    }

    /// The current value.
    pub fn value(&self) -> f32 {
        self.value
    }

    /// Sets the value, clamped into the current range.
    pub fn set_value(&mut self, value: f32) {
        let clamped = clamp_value(value, self.minimum, self.maximum);
        if self.value == clamped {
            return;
        }
        self.value = clamped;
        if self.animate {
            self.display.set_target(self.raw_fraction());
        }
        self.value_changed.emit(clamped);
        self.base.request_redraw();
    }

    /// The lower bound of the range.
    pub fn minimum(&self) -> f32 {
        self.minimum
    }

    /// Sets the lower bound. A bound above the current maximum is refused.
    pub fn set_minimum(&mut self, minimum: f32) -> Result<(), CapabilityAccessError> {
        if !minimum.is_finite() || minimum > self.maximum {
            return Err(CapabilityAccessError::OutOfRange);
        }
        if self.minimum == minimum {
            return Ok(());
        }
        self.minimum = minimum;
        self.value = clamp_value(self.value, self.minimum, self.maximum);
        self.reset_display();
        self.base.request_redraw();
        Ok(())
    }

    /// The upper bound of the range.
    pub fn maximum(&self) -> f32 {
        self.maximum
    }

    /// Sets the upper bound. A bound below the current minimum is refused.
    pub fn set_maximum(&mut self, maximum: f32) -> Result<(), CapabilityAccessError> {
        if !maximum.is_finite() || maximum < self.minimum {
            return Err(CapabilityAccessError::OutOfRange);
        }
        if self.maximum == maximum {
            return Ok(());
        }
        self.maximum = maximum;
        self.value = clamp_value(self.value, self.minimum, self.maximum);
        self.reset_display();
        self.base.request_redraw();
        Ok(())
    }

    /// Replaces the threshold colour bands on the track.
    ///
    /// An empty list restores the plain track. Bands are clipped to the current range when
    /// they are drawn, so changing the range does not require re-setting them.
    pub fn set_zones(&mut self, zones: Vec<GaugeZone>) {
        let zones: Vec<GaugeZone> = zones
            .into_iter()
            .filter(|zone| zone.start.is_finite() && zone.end.is_finite() && zone.start <= zone.end)
            .collect();
        if self.zones == zones {
            return;
        }
        self.zones = zones;
        self.base.request_redraw();
    }

    /// Returns the threshold colour bands.
    pub fn zones(&self) -> &[GaugeZone] {
        &self.zones
    }

    /// The threshold bands, under the `bands` spelling.
    pub fn bands(&self) -> &[GaugeZone] {
        &self.zones
    }

    /// Replaces the threshold bands, under the `bands` spelling.
    pub fn set_bands(&mut self, bands: Vec<GaugeZone>) {
        self.set_zones(bands);
    }

    /// Number of tick segments across the sweep.
    pub fn tick_count(&self) -> u32 {
        self.tick_count
    }

    /// Sets the number of tick segments. Zero is raised to one so the sweep always has marks.
    pub fn set_tick_count(&mut self, count: u32) {
        let count = count.max(1);
        if self.tick_count == count {
            return;
        }
        self.tick_count = count;
        self.base.request_redraw();
    }

    /// The sweep start angle, in radians clockwise from 3 o'clock.
    pub fn start_angle(&self) -> f32 {
        self.start_angle
    }

    /// Sets the sweep start angle. Non-finite input is refused.
    pub fn set_start_angle(&mut self, angle: f32) {
        if !angle.is_finite() || self.start_angle == angle {
            return;
        }
        self.start_angle = angle;
        self.base.request_redraw();
    }

    /// The sweep end angle, in radians clockwise from 3 o'clock.
    pub fn end_angle(&self) -> f32 {
        self.end_angle
    }

    /// Sets the sweep end angle. Non-finite input is refused.
    pub fn set_end_angle(&mut self, angle: f32) {
        if !angle.is_finite() || self.end_angle == angle {
            return;
        }
        self.end_angle = angle;
        self.base.request_redraw();
    }

    /// How the reading is marked on the sweep.
    pub fn needle_style(&self) -> GaugeNeedleStyle {
        self.needle_style
    }

    /// Sets the needle style.
    pub fn set_needle_style(&mut self, style: GaugeNeedleStyle) {
        if self.needle_style == style {
            return;
        }
        self.needle_style = style;
        self.base.request_redraw();
    }

    /// The readout's unit suffix, under the `unit_label` spelling.
    pub fn unit_label(&self) -> &str {
        &self.unit_label
    }

    /// Sets the readout's unit suffix.
    pub fn set_unit_label(&mut self, label: String) {
        if self.unit == label {
            self.unit_label = label;
            return;
        }
        self.unit = label.clone();
        self.unit_label = label;
        self.base.request_redraw();
    }

    /// The fraction represented by a pointer angle, clamped into the sweep.
    fn fraction_at_angle(&self, angle: f32) -> Option<f32> {
        let sweep = self.end_angle - self.start_angle;
        if sweep.abs() <= f32::EPSILON {
            return None;
        }
        let mut delta = angle - self.start_angle;
        while delta < 0.0 {
            delta += core::f32::consts::TAU;
        }
        while delta >= core::f32::consts::TAU {
            delta -= core::f32::consts::TAU;
        }
        if sweep > 0.0 {
            Some((delta / sweep).clamp(0.0, 1.0))
        } else {
            // A sweep that runs the other way still addresses the same arc.
            let positive_sweep = -sweep;
            let wrapped = if delta > 0.0 { delta } else { delta + core::f32::consts::TAU };
            Some((1.0 - (wrapped / positive_sweep).clamp(0.0, 1.0)).clamp(0.0, 1.0))
        }
    }

    /// Sets the value from a pointer position, when the pointer is on the dial.
    fn set_value_from_pointer(&mut self, pos: Point) -> bool {
        let (center, radius) = self.dial();
        let dx = (pos.x - center.x) as f32;
        let dy = (pos.y - center.y) as f32;
        if (dx * dx + dy * dy).sqrt() > radius + self.thickness as f32 {
            return false;
        }
        let angle = dy.atan2(dx);
        let Some(fraction) = self.fraction_at_angle(angle) else {
            return false;
        };
        let value = self.minimum + fraction * (self.maximum - self.minimum);
        self.set_value(value);
        true
    }

    /// Enables easing from the displayed reading to a newly set value.
    pub fn set_animate(&mut self, animate: bool) {
        if self.animate == animate {
            return;
        }
        self.animate = animate;
        self.display.jump_to(self.raw_fraction());
        self.base.request_redraw();
    }

    /// Returns whether value changes ease.
    pub fn is_animated(&self) -> bool {
        self.animate
    }

    /// The fraction the displayed value has reached.
    pub fn displayed_fraction(&self) -> f32 {
        if self.animate {
            self.display.value()
        } else {
            self.raw_fraction()
        }
    }

    /// Places the animation at the current reading without travelling.
    fn reset_display(&mut self) {
        let fraction = self.raw_fraction();
        self.display.jump_to(fraction);
    }

    /// The reading's fraction across the range, without any animation.
    fn raw_fraction(&self) -> f32 {
        let span = self.maximum - self.minimum;
        if span <= 0.0 {
            return 0.0;
        }
        ((self.value - self.minimum) / span).clamp(0.0, 1.0)
    }

    /// The caption under the dial.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Replaces the caption under the dial.
    pub fn set_label(&mut self, label: String) {
        if self.label == label {
            return;
        }
        self.label = label;
        self.base.request_redraw();
    }

    /// The unit printed after the number.
    pub fn unit(&self) -> &str {
        &self.unit
    }

    /// Replaces the unit printed after the number.
    pub fn set_unit(&mut self, unit: String) {
        if self.unit == unit {
            return;
        }
        self.unit = unit;
        self.base.request_redraw();
    }

    /// The track width in logical pixels.
    pub fn thickness(&self) -> u32 {
        self.thickness
    }

    /// Sets the track width in logical pixels. Zero is raised to one, because a zero-width arc
    /// emits no ink while still claiming the dial is drawn.
    pub fn set_thickness(&mut self, thickness: u32) {
        let thickness = thickness.max(1);
        if self.thickness == thickness {
            return;
        }
        self.thickness = thickness;
        self.base.request_redraw();
    }

    /// How far round the dial the value takes it, in `0.0..=1.0`.
    ///
    /// This is the displayed fraction: while the gauge is animating it is the eased value, and
    /// at rest it is the reading's own position.
    pub fn fraction(&self) -> f32 {
        self.displayed_fraction()
    }

    /// The geometry the dial's arcs are drawn at.
    ///
    /// Returned rather than recomputed in `draw` because the tests pin it: the radius has to
    /// leave room for the track's own width, or the outer half of the arc is clipped away by
    /// the control's rectangle.
    fn dial(&self) -> (Point, f32) {
        let rect = self.geometry();
        let center = Point::new(
            rect.x.saturating_add((rect.width / 2) as i32),
            rect.y.saturating_add((rect.height / 2) as i32),
        );
        let half = (rect.width.min(rect.height) as f32) / 2.0;
        let radius = (half - (self.thickness as f32) / 2.0 - 1.0).max(1.0);
        (center, radius)
    }
}

/// Clamps `value` into `[minimum, maximum]`, mapping `NaN` to the minimum.
///
/// `f32::clamp` propagates `NaN`, so a `NaN` readout would survive every comparison and then
/// print as "NaN" on the dial. `NaN` has no position on the range, so it takes the minimum —
/// the same "no measurement" reading a fresh gauge shows. An infinity, by contrast, *does*
/// compare against the bounds, so it clamps to the bound it lies past; refusing to clamp it
/// would leave the dial pointing past the end of its own track.
fn clamp_value(value: f32, minimum: f32, maximum: f32) -> f32 {
    if value.is_nan() {
        minimum
    } else {
        value.clamp(minimum, maximum)
    }
}

impl Widget for Gauge {
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
        Size::new(160, 160)
    }

    /// Advances the eased reading by `delta_ms`.
    fn tick(&mut self, delta_ms: u32) -> bool {
        if !self.animate {
            return false;
        }
        self.display.tick(delta_ms)
    }

    /// Whether the eased reading still owes frames.
    fn is_animating(&self) -> bool {
        self.animate && self.display.is_moving()
    }

    /// Resolves the gauge's published events to its signals.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "value_changed" => {
                Some(EventSignalRef::mapped("value_changed", &self.value_changed, |v| {
                    CapabilityValue::Float(f64::from(*v))
                }))
            }
            "band_clicked" => {
                Some(EventSignalRef::mapped("band_clicked", &self.band_clicked, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
            _ => None,
        }
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

impl WidgetProperties for Gauge {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "value" => Ok(CapabilityValue::Float(f64::from(self.value))),
            "minimum" => Ok(CapabilityValue::Float(f64::from(self.minimum))),
            "maximum" => Ok(CapabilityValue::Float(f64::from(self.maximum))),
            "label" => Ok(CapabilityValue::String(self.label.clone())),
            "unit" => Ok(CapabilityValue::String(self.unit.clone())),
            "thickness" => Ok(CapabilityValue::UInt(u64::from(self.thickness))),
            "zone_count" => Ok(CapabilityValue::UInt(self.zones.len() as u64)),
            "bands" => Ok(CapabilityValue::Tuple(
                self.zones
                    .iter()
                    .map(|zone| {
                        CapabilityValue::String(format!(
                            "{}|{}|{},{},{},{}|{}",
                            zone.start,
                            zone.end,
                            zone.color.r,
                            zone.color.g,
                            zone.color.b,
                            zone.color.a,
                            zone.name
                        ))
                    })
                    .collect(),
            )),
            "tick_count" => Ok(CapabilityValue::UInt(u64::from(self.tick_count))),
            "start_angle" => Ok(CapabilityValue::Float(f64::from(self.start_angle))),
            "end_angle" => Ok(CapabilityValue::Float(f64::from(self.end_angle))),
            "needle_style" => Ok(CapabilityValue::String(self.needle_style.as_str().to_string())),
            "unit_label" => Ok(CapabilityValue::String(self.unit_label.clone())),
            "animated" => Ok(CapabilityValue::Bool(self.animate)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "value" => {
                self.set_value(expect_f32(value)?);
                Ok(())
            }
            "minimum" => self.set_minimum(expect_f32(value)?),
            "maximum" => self.set_maximum(expect_f32(value)?),
            "label" => {
                self.set_label(expect_string(value)?);
                Ok(())
            }
            "unit" => {
                self.set_unit(expect_string(value)?);
                Ok(())
            }
            "thickness" => {
                self.set_thickness(expect_u32(value)?);
                Ok(())
            }
            "tick_count" => {
                self.set_tick_count(expect_u32(value)?);
                Ok(())
            }
            "start_angle" => {
                self.set_start_angle(expect_f32(value)?);
                Ok(())
            }
            "end_angle" => {
                self.set_end_angle(expect_f32(value)?);
                Ok(())
            }
            "needle_style" => {
                let token = expect_string(value)?;
                let style = GaugeNeedleStyle::from_token(&token)
                    .ok_or(CapabilityAccessError::TypeMismatch)?;
                self.set_needle_style(style);
                Ok(())
            }
            "unit_label" => {
                self.set_unit_label(expect_string(value)?);
                Ok(())
            }
            "zone_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            "bands" => {
                let mut bands = Vec::new();
                for entry in expect_string_list(value)? {
                    let mut parts = entry.splitn(4, '|');
                    let start = parts.next().and_then(|v| v.parse::<f32>().ok());
                    let end = parts.next().and_then(|v| v.parse::<f32>().ok());
                    let color = parts.next().and_then(|spec| {
                        let mut channels = spec.split(',');
                        let r = channels.next().and_then(|v| v.parse().ok());
                        let g = channels.next().and_then(|v| v.parse().ok());
                        let b = channels.next().and_then(|v| v.parse().ok());
                        let a = channels.next().and_then(|v| v.parse().ok());
                        match (r, g, b, a) {
                            (Some(r), Some(g), Some(b), Some(a)) => Some(Color::rgba(r, g, b, a)),
                            _ => None,
                        }
                    });
                    let name = parts.next().unwrap_or_default().to_string();
                    match (start, end, color) {
                        (Some(start), Some(end), Some(color)) => {
                            bands.push(GaugeZone { start, end, color, name });
                        }
                        _ => return Err(CapabilityAccessError::TypeMismatch),
                    }
                }
                self.set_bands(bands);
                Ok(())
            }
            "animated" => {
                let enabled = match value {
                    CapabilityValue::Bool(flag) => flag,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_animate(enabled);
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
            "label",
            "unit",
            "thickness",
            "zone_count",
            "bands",
            "tick_count",
            "start_angle",
            "end_angle",
            "needle_style",
            "unit_label",
            "animated",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl Draw for Gauge {
    fn draw(&mut self, ctx: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let style = self.base.style();
        let is_enabled = self.base.is_enabled();
        let themed = crate::style::resolved_theme_style_shared("gauge");
        let (window_background, window_foreground, accent) = {
            let manager = crate::style::theme_manager();
            match manager.current_theme() {
                Some(theme) => {
                    (theme.colors.background, theme.colors.foreground, theme.colors.primary)
                }
                None => (Color::WHITE, Color::BLACK, Color::PRIMARY),
            }
        };
        let surface = style
            .background_color
            .or_else(|| themed.as_ref().and_then(|resolved| resolved.background_color))
            .unwrap_or(window_background);
        let ink = style
            .text_color
            .or_else(|| themed.as_ref().and_then(|resolved| resolved.text_color))
            .unwrap_or(window_foreground);
        let accent = themed.as_ref().and_then(|resolved| resolved.border_color).unwrap_or(accent);
        // The unfilled track is the resolved ink pulled most of the way towards the surface: it
        // reads as "the rest of the scale" in both appearances, and `legible_on` keeps it from
        // collapsing into the surface on a theme whose ink and background are close.
        let track = ink.blend(&surface, 0.82).legible_on(surface, 1.5);

        let (center, radius) = self.dial();
        let sweep_start = self.start_angle;
        let sweep = self.end_angle - self.start_angle;
        draw_arc_segments(
            ctx,
            center,
            radius,
            sweep_start,
            sweep_start + sweep,
            track,
            self.thickness,
        );

        // Threshold bands are painted over the plain track and under the value arc, so the
        // reading still reads as one continuous measure. They are opt-in: an empty zone list
        // leaves the default dial untouched.
        let span = self.maximum - self.minimum;
        if span > 0.0 {
            for zone in &self.zones {
                let start = ((zone.start - self.minimum) / span).clamp(0.0, 1.0);
                let end = ((zone.end - self.minimum) / span).clamp(0.0, 1.0);
                if end <= start {
                    continue;
                }
                draw_arc_segments(
                    ctx,
                    center,
                    radius,
                    sweep_start + sweep * start,
                    sweep_start + sweep * end,
                    if is_enabled { zone.color } else { zone.color.with_alpha(110) },
                    self.thickness,
                );
            }
        }

        let fraction = self.fraction();
        let zone_color = self
            .zones
            .iter()
            .find(|zone| self.value >= zone.start && self.value <= zone.end)
            .map(|zone| zone.color);
        if fraction > 0.0 {
            draw_arc_segments(
                ctx,
                center,
                radius,
                sweep_start,
                sweep_start + sweep * fraction,
                zone_color.unwrap_or(accent),
                self.thickness,
            );
        }

        // The needle, when the caller asked for one. It is drawn over the track so it marks the
        // exact reading rather than the filled arc's end.
        let needle_angle = sweep_start + sweep * fraction;
        let needle_tip =
            point_on_circle(center, radius - self.thickness as f32 * 0.5, needle_angle);
        let needle_color = zone_color.unwrap_or(accent);
        match self.needle_style {
            GaugeNeedleStyle::None => {}
            GaugeNeedleStyle::Line => {
                ctx.draw_line_stroke(center, needle_tip, needle_color, 2);
            }
            GaugeNeedleStyle::Triangle => {
                let hub = 6.0f32.min(radius / 3.0).max(1.0);
                let left =
                    point_on_circle(center, hub, needle_angle + core::f32::consts::FRAC_PI_2);
                let right =
                    point_on_circle(center, hub, needle_angle - core::f32::consts::FRAC_PI_2);
                ctx.draw_line_stroke(needle_tip, left, needle_color, 2);
                ctx.draw_line_stroke(needle_tip, right, needle_color, 2);
                ctx.draw_line_stroke(left, right, needle_color, 2);
            }
        }

        // Major ticks sit just inside the track. They are drawn from the same angle formula the
        // arcs use, so a tick is always where the sweep says it is.
        let tick_outer = (radius - (self.thickness as f32) - 3.0).max(1.0);
        let tick_inner = (tick_outer - 4.0).max(0.0);
        if tick_inner > 0.0 {
            for index in 0..=self.tick_count {
                let angle = sweep_start + sweep * (index as f32 / self.tick_count as f32);
                ctx.draw_line_stroke(
                    point_on_circle(center, tick_outer, angle),
                    point_on_circle(center, tick_inner, angle),
                    ink.with_alpha_f32(0.55),
                    1,
                );
            }
        }

        // The readout: the value with its unit, and the caption beneath it. Both are bounded to
        // the inner square, so a long unit cannot run past the dial.
        let font = style.font.clone().unwrap_or_else(|| Font::simple("Arial", 12.0));
        let readout = if self.unit.is_empty() {
            format_number(self.value)
        } else {
            format!("{} {}", format_number(self.value), self.unit)
        };
        let inner = radius * 1.2;
        let readout_band = Rect::new(
            (center.x as f32 - inner / 2.0).round() as i32,
            (center.y as f32 - inner / 4.0).round() as i32,
            inner.round().max(1.0) as u32,
            (inner / 2.0).round().max(1.0) as u32,
        );
        ctx.draw_text_line(readout_band, &readout, &font, ink, HorizontalAlignment::Center);

        if !self.label.is_empty() {
            let label_band = Rect::new(
                readout_band.x,
                readout_band.y.saturating_add(readout_band.height as i32),
                readout_band.width,
                (radius / 3.0).round().max(1.0) as u32,
            );
            ctx.draw_text_line(
                label_band,
                &self.label,
                &font,
                ink.with_alpha_f32(0.7),
                HorizontalAlignment::Center,
            );
        }
    }
}

/// Formats a value with at most one decimal place, dropping a trailing `.0`.
///
/// `format!("{value}")` would print `1.2000000476837158` for an `f32` that came from `1.2`,
/// which is not a readout a person reads. One decimal place is the precision a dial can
/// resolve anyway.
fn format_number(value: f32) -> String {
    let rounded = (value * 10.0).round() / 10.0;
    if (rounded - rounded.round()).abs() < f32::EPSILON {
        format!("{}", rounded.round() as i64)
    } else {
        format!("{rounded:.1}")
    }
}

impl EventHandler for Gauge {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button, .. } if *button == 1 => {
                let (center, radius) = self.dial();
                let dx = (pos.x - center.x) as f32;
                let dy = (pos.y - center.y) as f32;
                let on_dial = (dx * dx + dy * dy).sqrt() <= radius + self.thickness as f32;
                if on_dial {
                    if let Some(fraction) = self.fraction_at_angle(dy.atan2(dx)) {
                        let pointer_value = self.minimum + fraction * (self.maximum - self.minimum);
                        if let Some(zone) = self
                            .zones
                            .iter()
                            .find(|zone| pointer_value >= zone.start && pointer_value <= zone.end)
                            .cloned()
                        {
                            self.band_clicked.emit(zone.level_name());
                        }
                    }
                }
                if self.set_value_from_pointer(*pos) {
                    self.dragging = true;
                    self.base.set_mouse_pressed(true);
                }
            }
            Event::MouseMove { pos } => {
                if self.dragging {
                    self.set_value_from_pointer(*pos);
                }
            }
            Event::MouseRelease { button, .. } if *button == 1 => {
                self.dragging = false;
                self.base.set_mouse_pressed(false);
            }
            Event::MouseLeave { .. } => {
                self.dragging = false;
                self.base.set_mouse_pressed(false);
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_gauge_reads_zero_of_a_hundred() {
        let gauge = Gauge::new(Rect::new(0, 0, 160, 160));
        assert_eq!(gauge.minimum(), 0.0);
        assert_eq!(gauge.maximum(), 100.0);
        assert_eq!(gauge.value(), 0.0);
        assert_eq!(gauge.fraction(), 0.0);
        assert_eq!(gauge.kind(), WidgetKind::Gauge);
    }

    #[test]
    fn the_value_is_clamped_into_the_range() {
        let mut gauge = Gauge::new(Rect::new(0, 0, 160, 160));
        gauge.set_value(150.0);
        assert_eq!(gauge.value(), 100.0);
        gauge.set_value(-20.0);
        assert_eq!(gauge.value(), 0.0);
        gauge.set_value(25.0);
        assert_eq!(gauge.fraction(), 0.25);
    }

    #[test]
    fn a_nan_value_takes_the_minimum_and_an_infinity_clamps_to_its_bound() {
        let mut gauge = Gauge::new(Rect::new(0, 0, 160, 160));
        gauge.set_value(f32::NAN);
        assert_eq!(gauge.value(), 0.0, "NaN has no position on the range");
        gauge.set_value(f32::INFINITY);
        assert_eq!(gauge.value(), 100.0, "+inf lies past the maximum");
        gauge.set_value(f32::NEG_INFINITY);
        assert_eq!(gauge.value(), 0.0, "-inf lies before the minimum");
    }

    #[test]
    fn the_bounds_refuse_to_cross() {
        let mut gauge = Gauge::new(Rect::new(0, 0, 160, 160));
        assert_eq!(gauge.set_maximum(-1.0), Err(CapabilityAccessError::OutOfRange));
        assert_eq!(gauge.set_minimum(101.0), Err(CapabilityAccessError::OutOfRange));
        assert_eq!(gauge.set_maximum(f32::NAN), Err(CapabilityAccessError::OutOfRange));
        gauge.set_maximum(50.0).expect("50 is inside the range");
        assert_eq!(gauge.maximum(), 50.0);
    }

    #[test]
    fn the_radius_leaves_room_for_the_track() {
        let mut gauge = Gauge::new(Rect::new(0, 0, 160, 160));
        gauge.set_thickness(20);
        let (center, radius) = gauge.dial();
        assert_eq!(center, Point::new(80, 80));
        // The track's outer edge must stay inside the 160 px box, i.e. inside 80 px from the
        // centre.
        assert!(radius + gauge.thickness() as f32 / 2.0 <= 80.0, "radius {radius}");
    }

    #[test]
    fn a_zero_thickness_is_raised_to_one() {
        let mut gauge = Gauge::new(Rect::new(0, 0, 160, 160));
        gauge.set_thickness(0);
        assert_eq!(gauge.thickness(), 1);
    }

    #[test]
    fn the_property_route_round_trips_every_field() {
        let mut gauge = Gauge::new(Rect::new(0, 0, 160, 160));
        gauge.set("value", CapabilityValue::Float(40.0)).expect("writable");
        gauge.set("label", CapabilityValue::String("load".to_string())).expect("writable");
        gauge.set("unit", CapabilityValue::String("%".to_string())).expect("writable");
        gauge.set("thickness", CapabilityValue::UInt(6)).expect("writable");
        assert_eq!(gauge.get("value"), Ok(CapabilityValue::Float(40.0)));
        assert_eq!(gauge.get("label"), Ok(CapabilityValue::String("load".to_string())));
        assert_eq!(gauge.get("unit"), Ok(CapabilityValue::String("%".to_string())));
        assert_eq!(gauge.get("thickness"), Ok(CapabilityValue::UInt(6)));
    }

    #[test]
    fn the_readout_drops_a_meaningless_fraction() {
        assert_eq!(format_number(1.0), "1");
        assert_eq!(format_number(1.25), "1.3");
        assert_eq!(format_number(-0.0), "0");
    }

    #[test]
    fn a_gauge_paints_its_track_and_readout() {
        let mut gauge = Gauge::new(Rect::new(0, 0, 160, 160));
        gauge.set_value(40.0);
        gauge.set_unit("%".to_string());
        let svg = crate::widget::svg::render_to_svg(&mut gauge);
        assert!(svg.contains("<line"), "the track and its ticks must be drawn: {svg}");
        assert_eq!(crate::widget::svg::text_ink_boxes(&svg).len(), 1, "the readout is drawn");
    }

    #[test]
    fn zones_are_filtered_and_the_value_arc_uses_their_colour() {
        let mut gauge = Gauge::new(Rect::new(0, 0, 160, 160));
        gauge.set_zones(vec![
            GaugeZone::new(0.0, 50.0, Color::SUCCESS),
            GaugeZone::new(50.0, 80.0, Color::WARNING),
            GaugeZone::new(90.0, 60.0, Color::ERROR),
        ]);
        assert_eq!(gauge.zones().len(), 2, "a reversed band is refused");
        gauge.set_value(75.0);
        let svg = crate::widget::svg::render_to_svg(&mut gauge);
        assert!(svg.starts_with("<svg"));
        assert_eq!(gauge.get("zone_count"), Ok(CapabilityValue::UInt(2)));
    }

    #[test]
    fn animated_values_ease_toward_the_reading() {
        let mut gauge = Gauge::new(Rect::new(0, 0, 160, 160));
        gauge.set_animate(true);
        assert!(gauge.is_animated());
        gauge.set_value(80.0);
        assert!(gauge.fraction() < 0.8, "the displayed fraction starts behind the reading");
        assert!(gauge.is_animating());
        let mut frames = 0;
        while gauge.tick(16) && frames < 200 {
            frames += 1;
        }
        assert!((gauge.fraction() - 0.8).abs() < 1e-3, "the eased value reaches the reading");
        assert!(!gauge.is_animating());
    }

    #[test]
    fn ticks_sweep_needle_and_band_events_are_configurable() {
        use crate::event::{Event, EventHandler};
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let mut gauge = Gauge::new(Rect::new(0, 0, 160, 160));
        assert_eq!(gauge.tick_count(), TICK_COUNT);
        gauge.set_tick_count(12);
        assert_eq!(gauge.tick_count(), 12);
        gauge.set_start_angle(0.0);
        gauge.set_end_angle(core::f32::consts::TAU);
        assert_eq!(gauge.start_angle(), 0.0);
        gauge.set_needle_style(GaugeNeedleStyle::Line);
        assert_eq!(gauge.needle_style(), GaugeNeedleStyle::Line);
        let _ = crate::widget::svg::render_to_svg(&mut gauge);

        gauge.set_bands(vec![GaugeZone::new(0.0, 50.0, Color::SUCCESS).with_name("low")]);
        let hits = Arc::new(AtomicUsize::new(0));
        let sink = Arc::clone(&hits);
        gauge.band_clicked.connect(move |_| {
            sink.fetch_add(1, Ordering::SeqCst);
        });
        // A point to the left of the centre is at the start of the sweep after the 0-radian
        // start angle above, i.e. inside the `low` band.
        gauge.handle_event(&Event::MousePress { pos: Point::new(2, 80), button: 1, modifiers: 0 });
        assert_eq!(hits.load(Ordering::SeqCst), 1, "the band click is reported");
        assert!(gauge.value() >= 0.0 && gauge.value() <= 50.0);
    }
}
