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

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
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
/// The four dial numerals, each with the hour position it belongs at.
const DIAL_NUMERALS: [(&str, f32); 4] = [("12", 0.0), ("3", 3.0), ("6", 6.0), ("9", 9.0)];

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
    pub fn set_hand_shading(&mut self, enabled: bool) {
        if self.hand_shading == enabled {
            return;
        }
        self.hand_shading = enabled;
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

    /// Paints the four dial numerals inside the face.
    fn draw_numerals(&self, ctx: &mut RenderContext, center: Point, radius: f32, ink: Color) {
        let size = (radius * 0.14).clamp(8.0, 20.0);
        let font = Font::simple("Arial", size);
        let advance = ctx.measure_text("12", &font).width.max(1);
        let height = ctx.measure_text("12", &font).height.max(1);
        let box_width = advance.saturating_add(4);
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
            ctx.draw_text_line(band, label, &font, numeral_ink, HorizontalAlignment::Center);
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
}
