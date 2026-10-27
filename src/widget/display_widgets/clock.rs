// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Clock — an analog clock face showing a time of day.
//!
//! # Why a clock and not a formatted `Label`
//!
//! A clock face is a *shape*: twelve ticks at fixed angles, three hands whose lengths encode
//! their unit, and an hour hand that advances between the marks as the minutes pass. A label
//! can print `14:05` but cannot show it, and a caller who formatted the digits themselves
//! would re-derive the twelve-hour wrap the face already has to do.
//!
//! # Time is three numbers, not a string
//!
//! `hour` / `minute` / `second` are published separately so a designer can drive the face
//! from three numeric wires; the clock renders them and never parses. Each is normalised on
//! write (hour into `0..24`, minute and second into `0..60`), so a caller that passes `90`
//! minutes gets `1:30` rather than an error or a hand pointing into space.

use core::fmt::Write as _;

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::{point_on_circle, RenderContext};
use crate::signal::Signal1;
use crate::widget::capability::coercion::{expect_bool, expect_string, expect_u32};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Twelve hour marks on the face.
const HOUR_MARKS: u32 = 12;
/// Sixty minute marks, drawn shorter than the hour marks.
const MINUTE_MARKS: u32 = 60;
/// The four dial numerals, each with the hour position it belongs at.
const DIAL_NUMERALS: [(&str, f32); 4] = [("12", 0.0), ("3", 3.0), ("6", 6.0), ("9", 9.0)];

/// Where the face's time comes from.
///
/// [`Manual`](Self::Manual) is the historical contract: the host feeds every change through
/// [`Clock::set_time`]. [`System`](Self::System) lets the clock's `tick` method read the machine
/// clock, so a face that is never fed still runs — and in both modes the `tick` signal is emitted
/// once a second so a host can mirror the reading without polling.
///
/// [`System`](Self::System) is the **local** wall clock, because a wall clock shows the time the
/// person in front of it reads; [`Utc`](Self::Utc) is the explicit alternative for a host that
/// wants a zone-independent face.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClockTimeSource {
    /// The host owns the time; `Widget::tick` only reports elapsed seconds.
    #[default]
    Manual,
    /// `Widget::tick` reads the system clock in the machine's local time zone.
    System,
    /// `Widget::tick` reads the system clock in UTC.
    Utc,
}

impl ClockTimeSource {
    /// Every source, in publication order.
    pub const ALL: [Self; 3] = [Self::Manual, Self::System, Self::Utc];

    /// The token this source is published as.
    pub const fn token(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::System => "system",
            Self::Utc => "utc",
        }
    }

    /// Parses a published token. `None` for a spelling this control does not publish.
    pub fn from_token(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|source| source.token() == token)
    }
}

/// The clock's published form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClockDisplay {
    /// A dial with hands — the historical form and the default.
    #[default]
    Dial,
    /// A numeric `HH:MM[:SS]` reading.
    Digital,
}

impl ClockDisplay {
    /// Every form, in publication order.
    pub const ALL: [Self; 2] = [Self::Dial, Self::Digital];

    /// The token this form is published as.
    pub const fn token(self) -> &'static str {
        match self {
            Self::Dial => "dial",
            Self::Digital => "digital",
        }
    }

    /// Parses a published token. `None` for a spelling this control does not publish.
    pub fn from_token(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|form| form.token() == token)
    }
}

/// The current wall-clock time as `(hour, minute, second, millisecond)`, in the zone the source
/// names.
///
/// `chrono` carries the platform's time-zone database, so the conversion from an instant to a local
/// reading is not re-derived here — the widget layer asks a time library rather than inspecting an
/// OS-specific zone file (principle #36 keeps that knowledge out of the widget layer).
/// Test-only convenience over [`system_time_of_day_in`]; the draw path always names its offset.
#[cfg(test)]
fn system_time_of_day(source: ClockTimeSource) -> Option<(u32, u32, u32, u32)> {
    system_time_of_day_in(source, None)
}

/// Reads the wall clock in `source`'s zone, shifted by `utc_offset_minutes` when one is set.
///
/// The offset is applied to the UTC reading rather than to the local one, so `System + offset`
/// means "the specified zone's wall clock" and `Utc + offset` is the same arithmetic from a
/// fixed base. A `Manual` source never reads the machine and is therefore unaffected.
fn system_time_of_day_in(
    source: ClockTimeSource,
    utc_offset_minutes: Option<i32>,
) -> Option<(u32, u32, u32, u32)> {
    use chrono::Timelike;
    match source {
        ClockTimeSource::System if utc_offset_minutes.is_none() => {
            let now = chrono::Local::now();
            Some((now.hour(), now.minute(), now.second(), now.timestamp_subsec_millis()))
        }
        ClockTimeSource::System | ClockTimeSource::Utc => {
            let now = chrono::Utc::now();
            if let Some(offset) = utc_offset_minutes {
                let shifted = now + chrono::Duration::minutes(i64::from(offset));
                Some((
                    shifted.hour(),
                    shifted.minute(),
                    shifted.second(),
                    shifted.timestamp_subsec_millis(),
                ))
            } else {
                Some((now.hour(), now.minute(), now.second(), now.timestamp_subsec_millis()))
            }
        }
        // Only the two clock-backed sources reach this function; `Manual` never asks the machine.
        ClockTimeSource::Manual => None,
    }
}

/// Clock — an analog clock face.
///
/// Seconds are drawn only when `show_seconds` is set: the second hand sweeps every pixel of
/// the dial once a minute, which is a lot of visual motion for a face used as a decoration.
pub struct Clock {
    base: BaseWidget,
    hour: u32,
    minute: u32,
    second: u32,
    /// The millisecond component of the current second, `0..1000`.
    millis: u32,
    show_seconds: bool,
    show_numerals: bool,
    hand_shading: bool,
    /// The dial numeral face and the metrics derived from it, kept across frames.
    ///
    /// The face size follows the face radius, so this is a one-entry cache rather than a
    /// `OnceLock`: it records the size and device scale that produced it and is rebuilt
    /// when either changes.
    numeral_cache: Option<NumeralCache>,
    /// The tick-mark geometry, rebuilt only when the face's rectangle changes.
    ///
    /// The 59 minute marks and 12 hour marks are a function of the control's rectangle and the
    /// face radius alone — the hands and the numerals move, the tick ring does not. They used to
    /// be re-derived and re-submitted every frame; the cache holds the sampled segments so the
    /// frame path only picks a colour and draws them.
    tick_cache: Option<TickCache>,
    /// Where the time comes from; see [`ClockTimeSource`].
    time_source: ClockTimeSource,
    /// The dial or the numeric form; the dial is the default.
    display: ClockDisplay,
    /// An optional UTC offset in minutes applied to machine readings.
    ///
    /// `None` keeps the source's own zone, which is the historical behaviour. Setting an offset
    /// makes the face show that zone's wall clock without changing the host's locale.
    utc_offset_minutes: Option<i32>,
    /// Milliseconds accumulated toward the next whole second the `tick` signal reports.
    pending_ms: u32,
    /// Emitted once per whole second the clock's `tick` method advances, with that second count.
    pub tick: Signal1<u32>,
}

/// The tick ring's sampled segments, keyed by the rectangle that produced them.
struct TickCache {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    /// `(start, end, thickness)` per mark. A thickness of 2 marks an hour mark.
    marks: Vec<(Point, Point, u32)>,
}

/// A dial numeral face with the measurements `draw_numerals` positions the glyphs by.
struct NumeralCache {
    /// `f32::to_bits` of the point size the face was built at.
    size_bits: u32,
    /// `f32::to_bits` of the logical DPI scale the metrics were measured under.
    dpi_scale_bits: u32,
    font: Font,
    /// Width of the numeral band, including the two-pixel margin per side.
    box_width: u32,
    /// Height of the numeral band.
    height: u32,
}

impl Clock {
    /// Creates a clock reading midnight, with the second hand hidden.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Clock, geometry, "Clock"),
            hour: 0,
            minute: 0,
            second: 0,
            millis: 0,
            show_seconds: false,
            show_numerals: false,
            hand_shading: false,
            numeral_cache: None,
            tick_cache: None,
            time_source: ClockTimeSource::Manual,
            display: ClockDisplay::Dial,
            utc_offset_minutes: None,
            pending_ms: 0,
            tick: Signal1::new(),
        }
    }

    /// The hour, `0..24`.
    pub fn hour(&self) -> u32 {
        self.hour
    }

    /// The minute, `0..60`.
    pub fn minute(&self) -> u32 {
        self.minute
    }

    /// The second, `0..60`.
    pub fn second(&self) -> u32 {
        self.second
    }

    /// The millisecond component of the current second, `0..1000`.
    pub fn millis(&self) -> u32 {
        self.millis
    }

    /// The sub-second fraction, `0.0..1.0`.
    ///
    /// This is what interpolates the second hand between two whole-second positions.
    pub fn sub_second(&self) -> f32 {
        self.millis as f32 / 1000.0
    }

    /// Whether the second hand is drawn.
    pub fn show_seconds(&self) -> bool {
        self.show_seconds
    }

    /// Sets all three components at once, normalising each.
    ///
    /// Minutes and seconds carry into the next larger unit, so `set_time(0, 90, 0)` reads
    /// `1:30` — the same normalisation a wall clock performs. Hours wrap modulo 24.
    ///
    /// This is whole-second input, so it clears any sub-second fraction a previous
    /// [`Self::set_time_with_millis`] left in place.
    pub fn set_time(&mut self, hour: u32, minute: u32, second: u32) {
        self.set_time_with_millis(hour, minute, second, 0);
    }

    /// Sets the time with a millisecond component.
    ///
    /// The four values are normalised together, so excess milliseconds carry into seconds
    /// exactly as excess seconds carry into minutes: `(0, 0, 59, 1500)` reads `00:01:00.500`.
    /// Hours wrap modulo 24.
    ///
    /// The fraction is used when painting the second hand, which is why a watch-style face
    /// can sweep smoothly rather than stepping once a second.
    pub fn set_time_with_millis(&mut self, hour: u32, minute: u32, second: u32, millis: u32) {
        let total_ms = (hour as u64)
            .saturating_mul(3_600_000)
            .saturating_add((minute as u64).saturating_mul(60_000))
            .saturating_add((second as u64).saturating_mul(1_000))
            .saturating_add(millis as u64);
        let day = 24u64 * 3_600_000;
        let wrapped = total_ms % day;
        let hour = (wrapped / 3_600_000) as u32;
        let minute = ((wrapped % 3_600_000) / 60_000) as u32;
        let second = ((wrapped % 60_000) / 1_000) as u32;
        let millis = (wrapped % 1_000) as u32;
        if (self.hour, self.minute, self.second, self.millis) == (hour, minute, second, millis) {
            return;
        }
        self.hour = hour;
        self.minute = minute;
        self.second = second;
        self.millis = millis;
        self.base.request_redraw();
    }

    /// Sets the hour alone, keeping the other components.
    pub fn set_hour(&mut self, hour: u32) {
        self.set_time(hour, self.minute, self.second);
    }

    /// Sets the minute alone, keeping the other components.
    pub fn set_minute(&mut self, minute: u32) {
        self.set_time(self.hour, minute, self.second);
    }

    /// Sets the second alone, keeping the other components.
    pub fn set_second(&mut self, second: u32) {
        self.set_time(self.hour, self.minute, second);
    }

    /// Sets whether the second hand is drawn.
    pub fn set_show_seconds(&mut self, show_seconds: bool) {
        if self.show_seconds == show_seconds {
            return;
        }
        self.show_seconds = show_seconds;
        self.base.request_redraw();
    }

    /// Whether the dial draws the `12`, `3`, `6` and `9` numerals.
    pub fn show_numerals(&self) -> bool {
        self.show_numerals
    }

    /// Turns the four dial numerals on or off. They are off by default.
    pub fn set_show_numerals(&mut self, show: bool) {
        if self.show_numerals == show {
            return;
        }
        self.show_numerals = show;
        self.base.request_redraw();
    }

    /// Whether the hands paint a shadow and a highlight.
    pub fn hand_shading(&self) -> bool {
        self.hand_shading
    }

    /// Enables or disables the two-layer hand shading. It is off by default, which keeps the
    /// resting face a single flat stroke per hand.
    ///
    /// When enabled, each hand is painted three times: a dark 1 px shadow offset down and
    /// right, the hand itself, and a 1 px lightened stroke along one side.
    /// Sets whether the hands paint a shadow and a highlight.
    pub fn set_hand_shading(&mut self, enabled: bool) {
        if self.hand_shading == enabled {
            return;
        }
        self.hand_shading = enabled;
        self.base.request_redraw();
    }

    /// Where the face's time comes from.
    pub fn time_source(&self) -> ClockTimeSource {
        self.time_source
    }

    /// Selects where the face's time comes from.
    ///
    /// Switching to [`ClockTimeSource::System`] does not change the reading immediately: the next
    /// `tick` call installs the machine clock's time. Switching back to
    /// [`ClockTimeSource::Manual`] leaves the last reading in place for the host to overwrite.
    pub fn set_time_source(&mut self, source: ClockTimeSource) {
        if self.time_source == source {
            return;
        }
        self.time_source = source;
        self.pending_ms = 0;
        self.base.request_redraw();
    }

    /// The clock's form: a dial (the default) or a numeric reading.
    pub fn display(&self) -> ClockDisplay {
        self.display
    }

    /// Selects the dial or the numeric form. The dial is the default and keeps the historical
    /// picture; a digital clock paints `HH:MM` (and `:SS` when the seconds are shown).
    pub fn set_display(&mut self, display: ClockDisplay) {
        if self.display == display {
            return;
        }
        self.display = display;
        self.base.request_redraw();
    }

    /// The optional UTC offset applied to machine readings, in minutes.
    pub fn utc_offset_minutes(&self) -> Option<i32> {
        self.utc_offset_minutes
    }

    /// Sets the optional UTC offset, or clears it with `None`.
    ///
    /// The default is `None`, which reads the source's own zone and is therefore the historical
    /// behaviour. The offset is clamped to ±24 hours, the range a real zone can occupy.
    pub fn set_utc_offset_minutes(&mut self, offset: Option<i32>) {
        let offset = offset.map(|minutes| minutes.clamp(-24 * 60, 24 * 60));
        if self.utc_offset_minutes == offset {
            return;
        }
        self.utc_offset_minutes = offset;
        self.base.request_redraw();
    }

    /// Advances the face by `delta_ms` and reports whether anything was repainted.
    ///
    /// With [`ClockTimeSource::System`] the reading is taken from the machine clock (UTC) every
    /// frame. Independently of the source, whole seconds are reported through the `tick` signal —
    /// one emission per second's worth of elapsed time — so a host mirroring the face does not
    /// have to poll it.
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        let mut repainted = false;
        if matches!(self.time_source, ClockTimeSource::System | ClockTimeSource::Utc) {
            if let Some((hour, minute, second, millis)) =
                system_time_of_day_in(self.time_source, self.utc_offset_minutes)
            {
                let before = (self.hour, self.minute, self.second, self.millis);
                self.set_time_with_millis(hour, minute, second, millis);
                repainted = before != (self.hour, self.minute, self.second, self.millis);
            }
        }
        self.pending_ms = self.pending_ms.saturating_add(delta_ms);
        let elapsed_seconds = self.pending_ms / 1_000;
        if elapsed_seconds > 0 {
            self.pending_ms -= elapsed_seconds * 1_000;
            self.tick.emit(elapsed_seconds);
            repainted = true;
        }
        repainted
    }

    /// Rebuilds the tick ring when the face's rectangle changed, then reports it.
    ///
    /// The ring is 60 sampled segments whose endpoints depend only on the face centre and radius,
    /// and the radius is itself a function of the rectangle. The rectangle therefore *is* the
    /// cache key: a resize rebuilds once, and every still frame reuses the segment table.
    fn ensure_tick_marks(&mut self, center: Point, radius: f32) -> bool {
        let rect = self.geometry();
        let fresh = self.tick_cache.as_ref().is_some_and(|cache| {
            cache.x == rect.x
                && cache.y == rect.y
                && cache.width == rect.width
                && cache.height == rect.height
        });
        if fresh {
            return false;
        }
        let mut marks = Vec::with_capacity((MINUTE_MARKS + HOUR_MARKS) as usize);
        for index in 0..MINUTE_MARKS {
            if index % 5 == 0 {
                continue;
            }
            let angle = Clock::angle_of(index as f32, MINUTE_MARKS as f32);
            marks.push((
                point_on_circle(center, radius * 0.94, angle),
                point_on_circle(center, radius * 0.88, angle),
                1,
            ));
        }
        for index in 0..HOUR_MARKS {
            let angle = Clock::angle_of(index as f32, HOUR_MARKS as f32);
            marks.push((
                point_on_circle(center, radius * 0.94, angle),
                point_on_circle(center, radius * 0.82, angle),
                2,
            ));
        }
        self.tick_cache =
            Some(TickCache { x: rect.x, y: rect.y, width: rect.width, height: rect.height, marks });
        true
    }

    /// The centre and radius the face is drawn at.
    ///
    /// Named rather than inlined into `draw` because the hand lengths are derived from it and a
    /// test pins that every hand stays inside the face: a hand that leaves the circle is the
    /// analog of a label that runs past its control.
    fn face(&self) -> (Point, f32) {
        let rect = self.geometry();
        let center = Point::new(
            rect.x.saturating_add((rect.width / 2) as i32),
            rect.y.saturating_add((rect.height / 2) as i32),
        );
        let radius = ((rect.width.min(rect.height) as f32) / 2.0 - 1.0).max(1.0);
        (center, radius)
    }

    /// The three hand lengths, as a fraction of the radius.
    ///
    /// The second hand is the longest — it has to clear the minute marks — the minute hand is
    /// next, and the hour hand is the shortest. Each is > 0 so a hand always has ink.
    fn hand_extents(&self) -> (f32, f32, f32) {
        let (_, radius) = self.face();
        (radius * 0.50, radius * 0.74, radius * 0.86)
    }

    /// The angle one component is drawn at.
    ///
    /// Angle 0 is 3 o'clock and angles increase clockwise (this crate's convention), so
    /// twelve o'clock is `-π/2`. `position` is the component's own count — hours since
    /// midnight, minutes since the hour — and `per_turn` how many of them make a full turn.
    fn angle_of(position: f32, per_turn: f32) -> f32 {
        -core::f32::consts::FRAC_PI_2 + (position / per_turn) * core::f32::consts::TAU
    }

    /// Paints the four dial numerals inside the face.
    fn draw_numerals(&mut self, ctx: &mut RenderContext, center: Point, radius: f32, ink: Color) {
        let size = (radius * 0.14).clamp(8.0, 20.0);
        let size_bits = size.to_bits();
        let dpi_scale_bits = ctx.dpi_scale().to_bits();
        // The face is a constructor call per frame otherwise, and the two measurements
        // below are the same answer until the radius or the device scale moves.
        let cache_valid = self.numeral_cache.as_ref().is_some_and(|cache| {
            cache.size_bits == size_bits && cache.dpi_scale_bits == dpi_scale_bits
        });
        if !cache_valid {
            let font = Font::simple("Arial", size);
            let metrics = ctx.measure_text("12", &font);
            self.numeral_cache = Some(NumeralCache {
                size_bits,
                dpi_scale_bits,
                font,
                box_width: metrics.width.max(1).saturating_add(4),
                height: metrics.height.max(1),
            });
        }
        let Some(cache) = self.numeral_cache.as_ref() else {
            return;
        };
        let font = &cache.font;
        let box_width = cache.box_width;
        let height = cache.height;
        let numeral_ink = ink.with_alpha_f32(0.85);
        for (label, position) in DIAL_NUMERALS {
            let anchor = point_on_circle(
                center,
                radius * 0.70,
                Clock::angle_of(position, HOUR_MARKS as f32),
            );
            // The band is centred on the numeral's anchor, and `draw_text_line` centres the
            // line box inside it, so the glyphs sit on the anchor rather than below it.
            let band = Rect::new(
                anchor.x.saturating_sub((box_width / 2) as i32),
                anchor.y.saturating_sub((height / 2) as i32),
                box_width,
                height,
            );
            ctx.draw_text_line_cow(band, label, font, numeral_ink, HorizontalAlignment::Center);
        }
    }

    /// Paints one hand, with the optional shadow and highlight.
    fn draw_hand(
        &self,
        ctx: &mut RenderContext,
        center: Point,
        angle: f32,
        length: f32,
        color: Color,
        thickness: u32,
    ) {
        let tip = point_on_circle(center, length, angle);
        if !self.hand_shading {
            ctx.draw_line_stroke(center, tip, color, thickness);
            return;
        }
        // The shadow is painted first and offset by one pixel, so the hand covers it except
        // for the sliver that reads as depth.
        ctx.draw_line_stroke(
            Point::new(center.x.saturating_add(1), center.y.saturating_add(1)),
            Point::new(tip.x.saturating_add(1), tip.y.saturating_add(1)),
            Color::BLACK.with_alpha_f32(0.25),
            thickness,
        );
        ctx.draw_line_stroke(center, tip, color, thickness);
        // A 1 px lightened stroke along one side of the hand.
        let offset = (thickness / 2).max(1) as f32;
        let dx = (-angle.sin() * offset).round() as i32;
        let dy = (angle.cos() * offset).round() as i32;
        ctx.draw_line_stroke(
            Point::new(center.x.saturating_add(dx), center.y.saturating_add(dy)),
            Point::new(tip.x.saturating_add(dx), tip.y.saturating_add(dy)),
            color.blend(&Color::WHITE, 0.45),
            1,
        );
    }
}

impl Widget for Clock {
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
        Size::new(140, 140)
    }

    /// One frame of the clock: the system time (when selected) and the per-second report.
    fn tick(&mut self, delta_ms: u32) -> bool {
        Clock::tick(self, delta_ms)
    }

    /// A system-sourced clock free-runs; a manual one is driven by its host.
    fn is_animating(&self) -> bool {
        matches!(self.time_source, ClockTimeSource::System | ClockTimeSource::Utc)
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();

    /// Resolves a published event name to the signal the clock emits.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "tick" => Some(EventSignalRef::mapped("tick", &self.tick, |value| {
                CapabilityValue::UInt(u64::from(*value))
            })),
            _ => None,
        }
    }
}

impl WidgetProperties for Clock {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "hour" => Ok(CapabilityValue::UInt(u64::from(self.hour))),
            "minute" => Ok(CapabilityValue::UInt(u64::from(self.minute))),
            "second" => Ok(CapabilityValue::UInt(u64::from(self.second))),
            "show_seconds" => Ok(CapabilityValue::Bool(self.show_seconds)),
            "time_source" => Ok(CapabilityValue::String(self.time_source().token().to_string())),
            "display" => Ok(CapabilityValue::String(self.display().token().to_string())),
            "utc_offset" => Ok(match self.utc_offset_minutes() {
                Some(minutes) => CapabilityValue::Int(i64::from(minutes)),
                None => CapabilityValue::Null,
            }),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "hour" => {
                self.set_hour(expect_u32(value)?);
                Ok(())
            }
            "minute" => {
                self.set_minute(expect_u32(value)?);
                Ok(())
            }
            "second" => {
                self.set_second(expect_u32(value)?);
                Ok(())
            }
            "show_seconds" => {
                self.set_show_seconds(expect_bool(value)?);
                Ok(())
            }
            "time_source" => {
                let token = expect_string(value)?;
                let source =
                    ClockTimeSource::from_token(&token).ok_or(CapabilityAccessError::OutOfRange)?;
                self.set_time_source(source);
                Ok(())
            }
            "display" => {
                let token = expect_string(value)?;
                let display =
                    ClockDisplay::from_token(&token).ok_or(CapabilityAccessError::OutOfRange)?;
                self.set_display(display);
                Ok(())
            }
            "utc_offset" => match value {
                CapabilityValue::Null => {
                    self.set_utc_offset_minutes(None);
                    Ok(())
                }
                CapabilityValue::Int(minutes) => {
                    let minutes =
                        i32::try_from(minutes).map_err(|_| CapabilityAccessError::OutOfRange)?;
                    self.set_utc_offset_minutes(Some(minutes));
                    Ok(())
                }
                _ => Err(CapabilityAccessError::TypeMismatch),
            },
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "hour",
            "minute",
            "second",
            "show_seconds",
            "time_source",
            "display",
            "utc_offset",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl Draw for Clock {
    fn draw(&mut self, ctx: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let style = self.base.style();
        let themed = crate::style::resolved_theme_style_shared("clock");
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

        let (center, radius) = self.face();
        let radius_px = radius.round().max(1.0) as u32;

        // The numeric form paints no hands and no tick ring: the reading is the whole control.
        // It is opt-in, so every clock that never asked for it keeps the dial picture.
        if self.display == ClockDisplay::Digital {
            let mut text = crate::compat::String::new();
            let _ = write!(text, "{:02}:{:02}", self.hour, self.minute);
            if self.show_seconds {
                let _ = write!(text, ":{:02}", self.second);
            }
            let size = (radius * 0.42).clamp(10.0, 96.0);
            let font = Font::with_weight("sans-serif", size, 600, false);
            let metrics = ctx.measure_text(&text, &font);
            ctx.draw_text(
                Point::new(
                    center.x - metrics.width as i32 / 2,
                    center.y - metrics.height as i32 / 2,
                ),
                &text,
                &font,
                ink,
                HorizontalAlignment::Left,
            );
            return;
        }

        // The face is filled first, so the ticks and hands always have a surface under them.
        let face_fill = ink.blend(&surface, 0.94).legible_on(surface, 1.05);
        ctx.fill_circle_aa(center, radius_px, face_fill);
        ctx.draw_circle_stroke(center, radius_px, ink.with_alpha_f32(0.65), 1);

        // Minute marks first, then hour marks over them: an hour mark is longer, so it covers
        // the minute mark at the same angle rather than the two coexisting. The segment table
        // itself is rebuilt only by a resize; see `ensure_tick_marks`.
        self.ensure_tick_marks(center, radius);
        if let Some(cache) = self.tick_cache.as_ref() {
            for (from, to, thickness) in &cache.marks {
                let color = if *thickness == 2 {
                    ink.with_alpha_f32(0.75)
                } else {
                    ink.with_alpha_f32(0.35)
                };
                ctx.draw_line_stroke(*from, *to, color, *thickness);
            }
        }

        if self.show_numerals {
            self.draw_numerals(ctx, center, radius, ink);
        }

        let (hour_len, minute_len, second_len) = self.hand_extents();
        // The hour hand advances continuously with the minutes, which is what stops it from
        // sitting exactly on a mark at half past the hour.
        let hour_position = (self.hour % 12) as f32 + self.minute as f32 / 60.0;
        let hour_angle = Clock::angle_of(hour_position, HOUR_MARKS as f32);
        self.draw_hand(ctx, center, hour_angle, hour_len, ink, 3);

        let minute_position = self.minute as f32 + self.second as f32 / 60.0;
        let minute_angle = Clock::angle_of(minute_position, MINUTE_MARKS as f32);
        self.draw_hand(ctx, center, minute_angle, minute_len, ink, 2);

        if self.show_seconds {
            // The millisecond component interpolates the hand within its second, so a
            // 60 Hz face sweeps rather than stepping once per second.
            let second_position = self.second as f32 + self.sub_second();
            let second_angle = Clock::angle_of(second_position, MINUTE_MARKS as f32);
            self.draw_hand(ctx, center, second_angle, second_len, accent, 1);
        }

        // The pin: a small filled dot over the hands' common origin.
        let pin = (radius * 0.06).round().max(1.0) as u32;
        ctx.fill_circle(center, pin, accent);
    }
}

impl EventHandler for Clock {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_clock_reads_midnight() {
        let clock = Clock::new(Rect::new(0, 0, 140, 140));
        assert_eq!((clock.hour(), clock.minute(), clock.second()), (0, 0, 0));
        assert!(!clock.show_seconds());
        assert_eq!(clock.kind(), WidgetKind::Clock);
    }

    #[test]
    fn minutes_and_seconds_carry_into_the_larger_unit() {
        let mut clock = Clock::new(Rect::new(0, 0, 140, 140));
        clock.set_time(0, 90, 0);
        assert_eq!((clock.hour(), clock.minute(), clock.second()), (1, 30, 0));
        clock.set_time(0, 0, 3661);
        assert_eq!((clock.hour(), clock.minute(), clock.second()), (1, 1, 1));
    }

    #[test]
    fn hours_wrap_at_a_day() {
        let mut clock = Clock::new(Rect::new(0, 0, 140, 140));
        clock.set_time(24, 0, 0);
        assert_eq!(clock.hour(), 0);
        clock.set_time(49, 0, 0);
        assert_eq!(clock.hour(), 1);
    }

    #[test]
    fn the_hour_hand_advances_with_the_minutes() {
        let mut clock = Clock::new(Rect::new(0, 0, 140, 140));
        clock.set_time(3, 0, 0);
        let on_the_hour = Clock::angle_of((clock.hour() % 12) as f32, HOUR_MARKS as f32);
        clock.set_time(3, 30, 0);
        let half_past = Clock::angle_of(
            (clock.hour() % 12) as f32 + clock.minute() as f32 / 60.0,
            HOUR_MARKS as f32,
        );
        assert!(
            half_past > on_the_hour,
            "the hour hand must move forward: {on_the_hour} -> {half_past}"
        );
    }

    #[test]
    fn twelve_oclock_points_straight_up() {
        let angle = Clock::angle_of(0.0, 12.0);
        let point = point_on_circle(Point::new(0, 0), 10.0, angle);
        assert_eq!(point, Point::new(0, -10));
        // Three o'clock points right, six down, nine left.
        assert_eq!(
            point_on_circle(Point::new(0, 0), 10.0, Clock::angle_of(3.0, 12.0)),
            Point::new(10, 0)
        );
        assert_eq!(
            point_on_circle(Point::new(0, 0), 10.0, Clock::angle_of(6.0, 12.0)),
            Point::new(0, 10)
        );
        assert_eq!(
            point_on_circle(Point::new(0, 0), 10.0, Clock::angle_of(9.0, 12.0)),
            Point::new(-10, 0)
        );
    }

    #[test]
    fn every_hand_stays_inside_the_face() {
        let clock = Clock::new(Rect::new(0, 0, 140, 140));
        let (_, radius) = clock.face();
        let (hour, minute, second) = clock.hand_extents();
        assert!(hour > 0.0 && minute > hour && second > minute);
        assert!(second < radius, "the longest hand must stay inside the face: {second} < {radius}");
    }

    #[test]
    fn the_property_route_round_trips_every_field() {
        let mut clock = Clock::new(Rect::new(0, 0, 140, 140));
        clock.set("hour", CapabilityValue::UInt(14)).expect("writable");
        clock.set("minute", CapabilityValue::UInt(5)).expect("writable");
        clock.set("second", CapabilityValue::UInt(9)).expect("writable");
        clock.set("show_seconds", CapabilityValue::Bool(true)).expect("writable");
        assert_eq!(clock.get("hour"), Ok(CapabilityValue::UInt(14)));
        assert_eq!(clock.get("minute"), Ok(CapabilityValue::UInt(5)));
        assert_eq!(clock.get("second"), Ok(CapabilityValue::UInt(9)));
        assert_eq!(clock.get("show_seconds"), Ok(CapabilityValue::Bool(true)));
    }

    #[test]
    fn a_clock_paints_its_face() {
        let mut clock = Clock::new(Rect::new(0, 0, 140, 140));
        clock.set_time(10, 10, 30);
        clock.set_show_seconds(true);
        let svg = crate::widget::svg::render_to_svg(&mut clock);
        assert!(svg.contains("circle"), "the face must be drawn: {svg}");
        assert!(svg.contains("<line"), "the hands and marks must be drawn: {svg}");
    }

    /// The four dial numerals are opt-in, and each one lands inside the face.
    #[test]
    fn numerals_are_opt_in_and_sit_inside_the_face() {
        let mut clock = Clock::new(Rect::new(0, 0, 140, 140));
        assert!(!clock.show_numerals(), "the default face has no numerals");

        let plain = crate::widget::svg::render_to_svg(&mut clock);
        assert!(
            crate::widget::svg::text_ink_boxes(&plain).is_empty(),
            "a default clock paints no text: {plain}"
        );

        clock.set_show_numerals(true);
        assert!(clock.show_numerals());
        let annotated = crate::widget::svg::render_to_svg(&mut clock);
        let boxes = crate::widget::svg::text_ink_boxes(&annotated);
        assert_eq!(boxes.len(), DIAL_NUMERALS.len(), "one run per numeral: {annotated}");

        let (center, radius) = clock.face();
        // `text_ink_boxes` reports `(left, top, right, bottom)`.
        for (left, top, right, bottom) in boxes {
            let cx = (left + right) as f32 / 2.0;
            let cy = (top + bottom) as f32 / 2.0;
            let half_reach = ((right - left).max(bottom - top) as f32) / 2.0;
            let dx = cx - center.x as f32;
            let dy = cy - center.y as f32;
            assert!(
                (dx * dx + dy * dy).sqrt() + half_reach <= radius + 1.0,
                "a numeral at ({cx}, {cy}) must stay inside the face of radius {radius}: {annotated}"
            );
        }
    }

    /// The second hand reads the millisecond component instead of stepping once a second.
    #[test]
    fn the_second_hand_interpolates_by_milliseconds() {
        let mut clock = Clock::new(Rect::new(0, 0, 140, 140));
        clock.set_show_seconds(true);

        clock.set_time_with_millis(10, 10, 30, 0);
        assert_eq!(clock.millis(), 0);
        let whole =
            Clock::angle_of(clock.second() as f32 + clock.sub_second(), MINUTE_MARKS as f32);

        clock.set_time_with_millis(10, 10, 30, 500);
        assert_eq!(clock.millis(), 500);
        assert!((clock.sub_second() - 0.5).abs() < 1e-6, "half a second is 0.5");
        let half = Clock::angle_of(clock.second() as f32 + clock.sub_second(), MINUTE_MARKS as f32);
        assert!(half > whole, "the hand must move forward within the second: {whole} -> {half}");

        // Carrying works the same way it does for seconds and minutes.
        clock.set_time_with_millis(0, 0, 59, 1500);
        assert_eq!((clock.hour(), clock.minute(), clock.second(), clock.millis()), (0, 1, 0, 500));

        // The whole-second setter clears the fraction, so existing callers keep their picture.
        clock.set_time(10, 10, 30);
        assert_eq!(clock.millis(), 0);
    }

    /// Hand shading is opt-in and paints the extra shadow and highlight strokes.
    #[test]
    fn hand_shading_adds_a_shadow_and_a_highlight() {
        let mut clock = Clock::new(Rect::new(0, 0, 140, 140));
        clock.set_time(10, 10, 30);
        clock.set_show_seconds(true);
        assert!(!clock.hand_shading(), "shading is off by default");

        let flat = crate::widget::svg::render_to_svg(&mut clock);
        let flat_strokes = flat.matches("<line").count();

        clock.set_hand_shading(true);
        assert!(clock.hand_shading());
        let shaded = crate::widget::svg::render_to_svg(&mut clock);
        let shaded_strokes = shaded.matches("<line").count();

        // Three hands, each gaining a shadow and a highlight.
        assert_eq!(
            shaded_strokes - flat_strokes,
            6,
            "each hand paints two extra strokes: {flat_strokes} -> {shaded_strokes}"
        );
    }

    /// The system source reports the machine's **local** wall clock, and the explicit `utc` source
    /// reports UTC. Before this, `System` silently read UTC, so a wall clock in a non-UTC zone
    /// disagreed with every other clock on the desktop.
    #[test]
    fn the_system_source_reads_local_time_and_utc_reads_utc() {
        use chrono::Timelike;

        let (local_h, local_m, _, _) =
            system_time_of_day(ClockTimeSource::System).expect("the local clock is readable");
        let now = chrono::Local::now();
        assert_eq!((local_h, local_m), (now.hour(), now.minute()));

        let (utc_h, utc_m, _, _) =
            system_time_of_day(ClockTimeSource::Utc).expect("the UTC clock is readable");
        let now = chrono::Utc::now();
        assert_eq!((utc_h, utc_m), (now.hour(), now.minute()));

        // `Manual` has no machine reading at all, so it must not fabricate one.
        assert_eq!(system_time_of_day(ClockTimeSource::Manual), None);
    }

    /// The three published time sources round-trip through their tokens.
    #[test]
    fn every_time_source_round_trips_through_its_token() {
        for source in ClockTimeSource::ALL {
            assert_eq!(ClockTimeSource::from_token(source.token()), Some(source));
        }
        assert_eq!(ClockTimeSource::from_token("local"), None);
    }

    /// The tick ring is sampled once and reused until the face's rectangle changes.
    #[test]
    fn the_tick_ring_is_cached_until_the_rectangle_changes() {
        let mut clock = Clock::new(Rect::new(0, 0, 140, 140));
        let (center, radius) = clock.face();
        assert!(clock.ensure_tick_marks(center, radius), "the first call builds the ring");
        assert!(
            !clock.ensure_tick_marks(center, radius),
            "a second call with the same rectangle must reuse the table"
        );
        let marks = clock.tick_cache.as_ref().expect("the cache exists").marks.len();
        assert_eq!(marks, (MINUTE_MARKS - MINUTE_MARKS / 5 + HOUR_MARKS) as usize);

        clock.set_geometry(Rect::new(0, 0, 200, 200));
        let (center, radius) = clock.face();
        assert!(clock.ensure_tick_marks(center, radius), "a resize rebuilds the ring once");
    }

    /// The digital form is opt-in and paints the reading as text; the default is the dial.
    #[test]
    fn the_digital_form_is_opt_in_and_paints_a_reading() {
        let mut clock = Clock::new(Rect::new(0, 0, 140, 140));
        assert_eq!(clock.display(), ClockDisplay::Dial, "the dial is the historical default");
        assert!(
            crate::widget::svg::text_ink_boxes(&crate::widget::svg::render_to_svg(&mut clock))
                .is_empty(),
            "a default clock paints no reading"
        );

        clock.set_display(ClockDisplay::Digital);
        clock.set_time(9, 5, 30);
        let svg = crate::widget::svg::render_to_svg(&mut clock);
        assert_eq!(crate::widget::svg::text_ink_boxes(&svg).len(), 1);

        clock.set_show_seconds(true);
        let with_seconds = crate::widget::svg::render_to_svg(&mut clock);
        assert_ne!(with_seconds, svg, "the reading gains the seconds when shown");
    }

    /// The UTC offset is optional, defaults to the source's own zone, and round-trips.
    #[test]
    fn the_utc_offset_is_optional_and_round_trips() {
        let mut clock = Clock::new(Rect::new(0, 0, 140, 140));
        assert_eq!(clock.utc_offset_minutes(), None);
        assert_eq!(clock.get("utc_offset"), Ok(CapabilityValue::Null));

        assert!(clock.set("utc_offset", CapabilityValue::Int(330)).is_ok());
        assert_eq!(clock.utc_offset_minutes(), Some(330));
        assert_eq!(clock.get("utc_offset"), Ok(CapabilityValue::Int(330)));

        clock.set("utc_offset", CapabilityValue::Null).expect("clearing is writable");
        assert_eq!(clock.utc_offset_minutes(), None);

        // The offset shifts a machine reading without changing the source.
        let base = system_time_of_day_in(ClockTimeSource::Utc, None).expect("utc is readable");
        let shifted =
            system_time_of_day_in(ClockTimeSource::Utc, Some(60)).expect("utc is readable");
        assert_eq!(
            (base.0 * 60 + base.1 + 60) % (24 * 60),
            shifted.0 * 60 + shifted.1,
            "one hour east is one hour later"
        );
    }

    /// The display and offset are published properties.
    #[test]
    fn the_display_property_round_trips() {
        let mut clock = Clock::new(Rect::new(0, 0, 140, 140));
        clock.set("display", CapabilityValue::String("digital".into())).expect("writable");
        assert_eq!(clock.display(), ClockDisplay::Digital);
        assert_eq!(clock.get("display"), Ok(CapabilityValue::String("digital".into())));
        assert!(clock.set("display", CapabilityValue::String("nope".into())).is_err());
        for form in ClockDisplay::ALL {
            assert_eq!(ClockDisplay::from_token(form.token()), Some(form));
        }
    }
}
