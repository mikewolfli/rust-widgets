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

use crate::core::{Color, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::{point_on_circle, RenderContext};
use crate::widget::capability::coercion::{expect_bool, expect_u32};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Twelve hour marks on the face.
const HOUR_MARKS: u32 = 12;
/// Sixty minute marks, drawn shorter than the hour marks.
const MINUTE_MARKS: u32 = 60;

/// Clock — an analog clock face.
///
/// Seconds are drawn only when `show_seconds` is set: the second hand sweeps every pixel of
/// the dial once a minute, which is a lot of visual motion for a face used as a decoration.
pub struct Clock {
    base: BaseWidget,
    hour: u32,
    minute: u32,
    second: u32,
    show_seconds: bool,
}

impl Clock {
    /// Creates a clock reading midnight, with the second hand hidden.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Clock, geometry, "Clock"),
            hour: 0,
            minute: 0,
            second: 0,
            show_seconds: false,
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

    /// Whether the second hand is drawn.
    pub fn show_seconds(&self) -> bool {
        self.show_seconds
    }

    /// Sets all three components at once, normalising each.
    ///
    /// Minutes and seconds carry into the next larger unit, so `set_time(0, 90, 0)` reads
    /// `1:30` — the same normalisation a wall clock performs. Hours wrap modulo 24.
    pub fn set_time(&mut self, hour: u32, minute: u32, second: u32) {
        let total_seconds = (hour as u64)
            .saturating_mul(3600)
            .saturating_add((minute as u64).saturating_mul(60))
            .saturating_add(second as u64);
        let day = 24u64 * 3600;
        let wrapped = (total_seconds % day) as u32;
        let hour = wrapped / 3600;
        let minute = (wrapped % 3600) / 60;
        let second = wrapped % 60;
        if (self.hour, self.minute, self.second) == (hour, minute, second) {
            return;
        }
        self.hour = hour;
        self.minute = minute;
        self.second = second;
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

    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

impl WidgetProperties for Clock {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "hour" => Ok(CapabilityValue::UInt(u64::from(self.hour))),
            "minute" => Ok(CapabilityValue::UInt(u64::from(self.minute))),
            "second" => Ok(CapabilityValue::UInt(u64::from(self.second))),
            "show_seconds" => Ok(CapabilityValue::Bool(self.show_seconds)),
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
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["hour", "minute", "second", "show_seconds", BASE_PROPERTY_NAMES]
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

        // The face is filled first, so the ticks and hands always have a surface under them.
        let face_fill = ink.blend(&surface, 0.94).legible_on(surface, 1.05);
        ctx.fill_circle_aa(center, radius_px, face_fill);
        ctx.draw_circle_stroke(center, radius_px, ink.with_alpha_f32(0.65), 1);

        // Minute marks first, then hour marks over them: an hour mark is longer, so it covers
        // the minute mark at the same angle rather than the two coexisting.
        for index in 0..MINUTE_MARKS {
            if index % 5 == 0 {
                continue;
            }
            let angle = Clock::angle_of(index as f32, MINUTE_MARKS as f32);
            ctx.draw_line_stroke(
                point_on_circle(center, radius * 0.94, angle),
                point_on_circle(center, radius * 0.88, angle),
                ink.with_alpha_f32(0.35),
                1,
            );
        }
        for index in 0..HOUR_MARKS {
            let angle = Clock::angle_of(index as f32, HOUR_MARKS as f32);
            ctx.draw_line_stroke(
                point_on_circle(center, radius * 0.94, angle),
                point_on_circle(center, radius * 0.82, angle),
                ink.with_alpha_f32(0.75),
                2,
            );
        }

        let (hour_len, minute_len, second_len) = self.hand_extents();
        // The hour hand advances continuously with the minutes, which is what stops it from
        // sitting exactly on a mark at half past the hour.
        let hour_position = (self.hour % 12) as f32 + self.minute as f32 / 60.0;
        let hour_angle = Clock::angle_of(hour_position, HOUR_MARKS as f32);
        ctx.draw_line_stroke(center, point_on_circle(center, hour_len, hour_angle), ink, 3);

        let minute_position = self.minute as f32 + self.second as f32 / 60.0;
        let minute_angle = Clock::angle_of(minute_position, MINUTE_MARKS as f32);
        ctx.draw_line_stroke(center, point_on_circle(center, minute_len, minute_angle), ink, 2);

        if self.show_seconds {
            let second_angle = Clock::angle_of(self.second as f32, MINUTE_MARKS as f32);
            ctx.draw_line_stroke(
                center,
                point_on_circle(center, second_len, second_angle),
                accent,
                1,
            );
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
}
