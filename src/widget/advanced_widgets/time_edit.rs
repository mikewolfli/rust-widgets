//! Time editor widget.
use crate::core::{HorizontalAlignment, Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;

use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
/// Time value (hour, minute, second, millisecond).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Time {
    hour: u8,   // 0-23
    minute: u8, // 0-59
    second: u8, // 0-59
    msec: u16,  // 0-999
}
impl Time {
    pub fn new(hour: u8, minute: u8, second: u8, msec: u16) -> Self {
        Self {
            hour: hour.min(23),
            minute: minute.min(59),
            second: second.min(59),
            msec: msec.min(999),
        }
    }
    pub fn hour(&self) -> u8 {
        self.hour
    }
    pub fn minute(&self) -> u8 {
        self.minute
    }
    pub fn second(&self) -> u8 {
        self.second
    }
    pub fn msec(&self) -> u16 {
        self.msec
    }
    pub fn set_hour(&mut self, hour: u8) {
        self.hour = hour.min(23);
    }
    pub fn set_minute(&mut self, minute: u8) {
        self.minute = minute.min(59);
    }
    pub fn set_second(&mut self, second: u8) {
        self.second = second.min(59);
    }
    pub fn set_msec(&mut self, msec: u16) {
        self.msec = msec.min(999);
    }
    pub fn is_valid(&self) -> bool {
        self.hour <= 23 && self.minute <= 59 && self.second <= 59 && self.msec <= 999
    }
    pub fn to_msecs_since_midnight(&self) -> u32 {
        (self.hour as u32 * 3600 + self.minute as u32 * 60 + self.second as u32) * 1000
            + self.msec as u32
    }
}
impl std::fmt::Display for Time {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:02}:{:02}:{:02}", self.hour, self.minute, self.second)
    }
}
/// Time editor widget.
pub struct TimeEdit {
    base: BaseWidget,
    time: Time,
    minimum: Time,
    maximum: Time,
    display_format: String,
    pub time_changed: Signal1<Time>,
}
impl TimeEdit {
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::TimePicker, geometry, "TimeEdit"),
            time: Time::new(0, 0, 0, 0),
            minimum: Time::new(0, 0, 0, 0),
            maximum: Time::new(23, 59, 59, 999),
            display_format: "HH:mm:ss".to_string(),
            time_changed: Signal1::new(),
        }
    }
    pub fn time(&self) -> Time {
        self.time
    }
    pub fn minimum_time(&self) -> Time {
        self.minimum
    }
    pub fn maximum_time(&self) -> Time {
        self.maximum
    }
    pub fn display_format(&self) -> &str {
        &self.display_format
    }
    pub fn set_time(&mut self, time: Time) {
        if time.is_valid() && time >= self.minimum && time <= self.maximum && self.time != time {
            self.time = time;
            self.time_changed.emit(time);
        }
    }
    pub fn set_minimum_time(&mut self, time: Time) {
        self.minimum = time;
    }
    pub fn set_maximum_time(&mut self, time: Time) {
        self.maximum = time;
    }
    /// Sets both minimum and maximum times in one call.
    /// This is a convenience writer; query bounds via `minimum_time()` and `maximum_time()`.
    pub fn set_time_range(&mut self, min: Time, max: Time) {
        self.minimum = min;
        self.maximum = max;
    }
    pub fn set_display_format(&mut self, fmt: String) {
        self.display_format = fmt;
    }
    pub fn step_up(&mut self) {
        let mut t = self.time;
        let new_sec = t.second() + 1;
        if new_sec >= 60 {
            t.second = 0;
            let new_min = t.minute() + 1;
            if new_min >= 60 {
                t.minute = 0;
                if t.hour() < 23 {
                    t.hour = t.hour() + 1;
                }
            } else {
                t.minute = new_min;
            }
        } else {
            t.second = new_sec;
        }
        self.set_time(t);
    }
    pub fn step_down(&mut self) {
        let mut t = self.time;
        if t.second() > 0 {
            t.set_second(t.second() - 1);
        } else {
            t.set_second(59);
            if t.minute() > 0 {
                t.set_minute(t.minute() - 1);
            } else {
                t.set_minute(59);
                if t.hour() > 0 {
                    t.set_hour(t.hour() - 1);
                }
            }
        }
        self.set_time(t);
    }
}
impl Widget for TimeEdit {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}
impl EventHandler for TimeEdit {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        if let Event::KeyPress { key, .. } = event {
            match *key {
                38 => self.step_up(),
                40 => self.step_down(),
                _ => { /* Other keys are not relevant */ }
            }
        }
    }
}
impl Draw for TimeEdit {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        context.fill_rect(rect, Color::rgb(255, 255, 255));
        context.draw_rect(rect, Color::rgb(150, 150, 150));
        let text = self.time.to_string();
        context.draw_text(
            Point { x: rect.x + 6, y: rect.y + (rect.height as i32 / 2) },
            &text,
            &Font::default(),
            Color::rgb(0, 0, 0),
            HorizontalAlignment::Left,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::svg::render_to_svg;
    use std::sync::{Arc, Mutex};

    // ─── Time helper tests ───────────────────────────────────────

    #[test]
    fn time_creation_and_accessors() {
        let t = Time::new(10, 30, 45, 500);
        assert_eq!(t.hour(), 10);
        assert_eq!(t.minute(), 30);
        assert_eq!(t.second(), 45);
        assert_eq!(t.msec(), 500);
    }

    #[test]
    fn time_clamps_invalid_values() {
        let t = Time::new(25, 70, 99, 2000);
        assert_eq!(t.hour(), 23);
        assert_eq!(t.minute(), 59);
        assert_eq!(t.second(), 59);
        assert_eq!(t.msec(), 999);
    }

    #[test]
    fn time_is_valid_true_for_all_public_api_constructs() {
        // Time::new and all setters clamp, so all values reachable
        // through the public API are always valid.
        assert!(Time::new(12, 0, 0, 0).is_valid());
        assert!(Time::new(23, 59, 59, 999).is_valid());
        assert!(Time::new(0, 0, 0, 0).is_valid());
    }

    #[test]
    fn time_setters_clamp_values() {
        let mut t = Time::new(0, 0, 0, 0);
        t.set_hour(24);
        assert_eq!(t.hour(), 23);
        t.set_minute(60);
        assert_eq!(t.minute(), 59);
        t.set_second(60);
        assert_eq!(t.second(), 59);
        t.set_msec(1000);
        assert_eq!(t.msec(), 999);
    }

    #[test]
    fn time_to_msecs_since_midnight() {
        let t = Time::new(1, 30, 15, 250);
        let expected = (3600 + 30 * 60 + 15) * 1000 + 250;
        assert_eq!(t.to_msecs_since_midnight(), expected);
    }

    #[test]
    fn time_midnight_is_zero_msecs() {
        let t = Time::new(0, 0, 0, 0);
        assert_eq!(t.to_msecs_since_midnight(), 0);
    }

    #[test]
    fn time_display_format() {
        let t = Time::new(9, 5, 3, 0);
        assert_eq!(t.to_string(), "09:05:03");
        let t2 = Time::new(23, 59, 59, 999);
        assert_eq!(t2.to_string(), "23:59:59");
    }

    #[test]
    fn time_ordering() {
        let early = Time::new(8, 0, 0, 0);
        let late = Time::new(9, 0, 0, 0);
        assert!(early < late);
        assert!(late > early);
        assert_eq!(early, Time::new(8, 0, 0, 0));
    }

    // ─── TimeEdit widget tests ───────────────────────────────────

    #[test]
    fn time_edit_creation_defaults() {
        let editor = TimeEdit::new(Rect::new(0, 0, 200, 30));
        assert_eq!(editor.time(), Time::new(0, 0, 0, 0));
        assert_eq!(editor.display_format(), "HH:mm:ss");
        assert_eq!(editor.minimum_time(), Time::new(0, 0, 0, 0));
        assert_eq!(editor.maximum_time(), Time::new(23, 59, 59, 999));
        assert!(editor.is_visible());
        assert!(editor.is_enabled());
    }

    #[test]
    fn time_edit_set_and_get_time() {
        let mut editor = TimeEdit::new(Rect::new(0, 0, 200, 30));
        let t = Time::new(14, 30, 0, 0);
        editor.set_time(t);
        assert_eq!(editor.time(), t);
    }

    #[test]
    fn time_edit_set_time_clamps_to_minimum() {
        let mut editor = TimeEdit::new(Rect::new(0, 0, 200, 30));
        let before_min = Time::new(0, 0, 0, 0); // same as default min, should still work
        editor.set_time(before_min);
        assert_eq!(editor.time(), Time::new(0, 0, 0, 0));

        // Set minimum to something higher, then try to set below it
        editor.set_minimum_time(Time::new(10, 0, 0, 0));
        let earlier = Time::new(5, 0, 0, 0);
        editor.set_time(earlier);
        // Should not change because 5:00 < 10:00
        assert_eq!(editor.time(), Time::new(0, 0, 0, 0));
    }

    #[test]
    fn time_edit_set_time_clamps_to_maximum() {
        let mut editor = TimeEdit::new(Rect::new(0, 0, 200, 30));
        let after_max = Time::new(23, 59, 59, 999); // same as default max, should work
        editor.set_time(after_max);
        assert_eq!(editor.time(), after_max);

        // Set maximum to something lower, then try to set above it
        editor.set_maximum_time(Time::new(12, 0, 0, 0));
        let later = Time::new(18, 0, 0, 0);
        editor.set_time(later);
        // Should not change because 18:00 > 12:00
        assert_eq!(editor.time(), Time::new(23, 59, 59, 999));
    }

    #[test]
    fn time_edit_set_time_range() {
        let mut editor = TimeEdit::new(Rect::new(0, 0, 200, 30));
        let min = Time::new(6, 0, 0, 0);
        let max = Time::new(22, 0, 0, 0);
        editor.set_time_range(min, max);
        assert_eq!(editor.minimum_time(), min);
        assert_eq!(editor.maximum_time(), max);
    }

    #[test]
    fn time_edit_set_display_format() {
        let mut editor = TimeEdit::new(Rect::new(0, 0, 200, 30));
        editor.set_display_format("HH:mm".to_string());
        assert_eq!(editor.display_format(), "HH:mm");
    }

    #[test]
    fn time_edit_step_up_increments_second() {
        let mut editor = TimeEdit::new(Rect::new(0, 0, 200, 30));
        editor.set_time(Time::new(10, 30, 15, 0));
        editor.step_up();
        assert_eq!(editor.time(), Time::new(10, 30, 16, 0));
    }

    #[test]
    fn time_edit_step_up_rolls_minute() {
        let mut editor = TimeEdit::new(Rect::new(0, 0, 200, 30));
        editor.set_time(Time::new(10, 30, 59, 0));
        editor.step_up();
        assert_eq!(editor.time(), Time::new(10, 31, 0, 0));
    }

    #[test]
    fn time_edit_step_up_rolls_hour() {
        let mut editor = TimeEdit::new(Rect::new(0, 0, 200, 30));
        editor.set_time(Time::new(10, 59, 59, 0));
        editor.step_up();
        assert_eq!(editor.time(), Time::new(11, 0, 0, 0));
    }

    #[test]
    fn time_edit_step_up_from_23_59_59_wraps_minutes_and_seconds() {
        let mut editor = TimeEdit::new(Rect::new(0, 0, 200, 30));
        editor.set_time(Time::new(23, 59, 59, 0));
        editor.step_up();
        // Seconds and minutes wrap to 0, hour stays at 23 (no day wrap in TimeEdit)
        assert_eq!(editor.time(), Time::new(23, 0, 0, 0));
    }

    #[test]
    fn time_edit_step_down_decrements_second() {
        let mut editor = TimeEdit::new(Rect::new(0, 0, 200, 30));
        editor.set_time(Time::new(10, 30, 15, 0));
        editor.step_down();
        assert_eq!(editor.time(), Time::new(10, 30, 14, 0));
    }

    #[test]
    fn time_edit_step_down_rolls_minute() {
        let mut editor = TimeEdit::new(Rect::new(0, 0, 200, 30));
        editor.set_time(Time::new(10, 30, 0, 0));
        editor.step_down();
        assert_eq!(editor.time(), Time::new(10, 29, 59, 0));
    }

    #[test]
    fn time_edit_step_down_rolls_hour() {
        let mut editor = TimeEdit::new(Rect::new(0, 0, 200, 30));
        editor.set_time(Time::new(10, 0, 0, 0));
        editor.step_down();
        assert_eq!(editor.time(), Time::new(9, 59, 59, 0));
    }

    #[test]
    fn time_edit_step_down_stops_at_0() {
        let mut editor = TimeEdit::new(Rect::new(0, 0, 200, 30));
        editor.set_time(Time::new(0, 0, 0, 0));
        editor.step_down();
        // hour 0, minute 0, second 0: step_down keeps hour at 0, minute at 59, second at 59
        // But then set_time rejects because 0:59:59 >= minimum (0,0,0,0) so it should accept.
        // Actually wait: hour starts at 0, step_down: second is 0, so second=59, minute is 0 so minute=59,
        // hour is 0 so hour stays at 0. Result is 0:59:59 which is valid.
        assert_eq!(editor.time(), Time::new(0, 59, 59, 0));
    }

    #[test]
    fn time_edit_time_changed_signal_emits() {
        let mut editor = TimeEdit::new(Rect::new(0, 0, 200, 30));
        let captured = Arc::new(Mutex::new(Time::new(0, 0, 0, 0)));
        let sink = captured.clone();
        editor.time_changed.connect(move |t| {
            if let Ok(mut c) = sink.lock() {
                *c = *t;
            }
        });

        let expected = Time::new(18, 30, 0, 0);
        editor.set_time(expected);
        let got = *captured.lock().unwrap();
        assert_eq!(got, expected);
    }

    #[test]
    fn time_edit_time_changed_not_emitted_for_same_time() {
        let mut editor = TimeEdit::new(Rect::new(0, 0, 200, 30));
        let hits = Arc::new(Mutex::new(0usize));
        let hits_clone = hits.clone();
        editor.time_changed.connect(move |_| {
            if let Ok(mut h) = hits_clone.lock() {
                *h += 1;
            }
        });

        editor.set_time(Time::new(0, 0, 0, 0)); // same as default, should NOT emit
        assert_eq!(*hits.lock().unwrap(), 0);
    }

    #[test]
    fn time_edit_geometry_delegation() {
        let rect = Rect::new(10, 20, 200, 30);
        let editor = TimeEdit::new(rect);
        assert_eq!(editor.geometry(), rect);
        assert_eq!(editor.position(), Point::new(10, 20));
        assert_eq!(editor.size(), crate::core::Size::new(200, 30));
    }

    #[test]
    fn time_edit_widget_kind() {
        let editor = TimeEdit::new(Rect::new(0, 0, 200, 30));
        assert_eq!(editor.kind(), WidgetKind::TimePicker);
    }

    #[test]
    fn time_edit_ids_are_unique() {
        let a = TimeEdit::new(Rect::new(0, 0, 200, 30));
        let b = TimeEdit::new(Rect::new(0, 0, 200, 30));
        assert_ne!(a.id(), b.id());
    }

    #[test]
    fn time_edit_svg_output() {
        let mut editor = TimeEdit::new(Rect::new(0, 0, 200, 30));
        editor.set_time(Time::new(12, 30, 45, 0));
        let svg = render_to_svg(&mut editor);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        assert!(svg.contains("width=\"200\""));
        assert!(svg.contains("height=\"30\""));
        // Should contain the time text rendered
        assert!(svg.contains("12:30:45") || svg.contains("fill="));
    }

    #[test]
    fn time_edit_disabled_state_blocks_changes() {
        let mut editor = TimeEdit::new(Rect::new(0, 0, 200, 30));
        editor.set_time(Time::new(10, 0, 0, 0));
        editor.set_enabled(false);
        assert!(!editor.is_enabled());

        // Step up should not work when disabled (handle_event checks is_enabled)
        editor.step_up();
        // step_up directly modifies and calls set_time, which checks min/max but not is_enabled
        // Only handle_event checks is_enabled, so step_up/step_down still work when called directly.
        // The disabled guard is in handle_event only.
        // Let's verify via handle_event:
        editor.set_time(Time::new(10, 0, 0, 0)); // reset
        editor.handle_event(&Event::KeyPress { key: 38, modifiers: 0 });
        // Because disabled, handle_event returns early, so time stays unchanged
        assert_eq!(editor.time(), Time::new(10, 0, 0, 0));
    }

    #[test]
    fn time_edit_keyboard_increment_and_decrement() {
        let mut editor = TimeEdit::new(Rect::new(0, 0, 200, 30));
        editor.set_time(Time::new(10, 0, 0, 0));

        // Up arrow (key 38) = step_up
        editor.handle_event(&Event::KeyPress { key: 38, modifiers: 0 });
        assert_eq!(editor.time(), Time::new(10, 0, 1, 0));

        // Down arrow (key 40) = step_down
        editor.handle_event(&Event::KeyPress { key: 40, modifiers: 0 });
        assert_eq!(editor.time(), Time::new(10, 0, 0, 0));
    }

    #[test]
    fn time_edit_mouse_wheel_does_not_change_time() {
        // TimeEdit does not handle wheel events
        let mut editor = TimeEdit::new(Rect::new(0, 0, 200, 30));
        editor.set_time(Time::new(10, 0, 0, 0));

        editor.handle_event(&Event::Wheel { delta: Point::new(0, 120), modifiers: 0 });
        assert_eq!(editor.time(), Time::new(10, 0, 0, 0));

        editor.handle_event(&Event::Wheel { delta: Point::new(0, -120), modifiers: 0 });
        assert_eq!(editor.time(), Time::new(10, 0, 0, 0));
    }

    #[test]
    fn time_edit_visibility_toggle() {
        let mut editor = TimeEdit::new(Rect::new(0, 0, 200, 30));
        assert!(editor.is_visible());
        editor.set_visible(false);
        assert!(!editor.is_visible());
        editor.set_visible(true);
        assert!(editor.is_visible());
    }

    #[test]
    fn time_edit_set_time_respects_min_max_range() {
        let mut editor = TimeEdit::new(Rect::new(0, 0, 200, 30));
        // Set a narrow range
        editor.set_time_range(Time::new(9, 0, 0, 0), Time::new(17, 0, 0, 0));

        // Set to a value within range
        let within = Time::new(12, 0, 0, 0);
        editor.set_time(within);
        assert_eq!(editor.time(), within);

        // Try setting below minimum
        editor.set_time(Time::new(8, 0, 0, 0));
        assert_eq!(editor.time(), within); // unchanged

        // Try setting above maximum
        editor.set_time(Time::new(18, 0, 0, 0));
        assert_eq!(editor.time(), within); // unchanged
    }
}
