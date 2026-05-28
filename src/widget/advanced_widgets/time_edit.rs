//! Time editor widget.
use crate::core::{Color, Font, Point, Rect};
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
    /// Query current bounds via `minimum_time()` and `maximum_time()`.
    pub fn set_time_range(&mut self, min: Time, max: Time) {
        self.minimum = min;
        self.maximum = max;
    }
    pub fn set_display_format(&mut self, fmt: String) {
        self.display_format = fmt;
    }
    pub fn step_up(&mut self) {
        let mut t = self.time;
        t.set_second(t.second() + 1);
        if t.second() >= 60 {
            t.set_second(0);
            t.set_minute(t.minute() + 1);
        }
        if t.minute() >= 60 {
            t.set_minute(0);
            if t.hour() < 23 {
                t.set_hour(t.hour() + 1);
            }
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
                _ => {}
            }
        }
    }
}
impl Draw for TimeEdit {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        context.fill_rect(rect, Color::from_rgb(255, 255, 255));
        context.draw_rect(rect, Color::from_rgb(150, 150, 150));
        let text = self.time.to_string();
        context.draw_text(
            Point {
                x: rect.x + 6,
                y: rect.y + (rect.height as i32 / 2),
            },
            &text,
            &Font::default(),
            Color::from_rgb(0, 0, 0),
        );
    }
}
