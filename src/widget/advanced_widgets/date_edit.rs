//! Date editor widget.
use crate::core::{Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
/// Date value (year, month, day).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Date {
    year: i32,
    month: u8, // 1-12
    day: u8,   // 1-31
}
impl Date {
    pub fn new(year: i32, month: u8, day: u8) -> Self {
        Self { year, month: month.clamp(1, 12), day: day.clamp(1, 31) }
    }
    pub fn today() -> Self {
        Self { year: 2024, month: 1, day: 1 }
    }
    pub fn year(&self) -> i32 {
        self.year
    }
    pub fn month(&self) -> u8 {
        self.month
    }
    pub fn day(&self) -> u8 {
        self.day
    }
    pub fn set_year(&mut self, year: i32) {
        self.year = year;
    }
    pub fn set_month(&mut self, month: u8) {
        self.month = month.clamp(1, 12);
    }
    pub fn set_day(&mut self, day: u8) {
        self.day = day.clamp(1, 31);
    }
    pub fn days_in_month(&self) -> u8 {
        match self.month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 => {
                if self.is_leap_year() {
                    29
                } else {
                    28
                }
            }
            _ => 30,
        }
    }
    pub fn is_leap_year(&self) -> bool {
        (self.year % 4 == 0 && self.year % 100 != 0) || (self.year % 400 == 0)
    }
    pub fn is_valid(&self) -> bool {
        self.month >= 1 && self.month <= 12 && self.day >= 1 && self.day <= self.days_in_month()
    }
}
impl std::fmt::Display for Date {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}
/// Date editor widget.
pub struct DateEdit {
    base: BaseWidget,
    date: Date,
    minimum: Date,
    maximum: Date,
    display_format: String,
    calendar_popup: bool,
    pub date_changed: Signal1<Date>,
}
impl DateEdit {
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::DatePicker, geometry, "DateEdit"),
            date: Date::today(),
            minimum: Date::new(1752, 9, 14),
            maximum: Date::new(9999, 12, 31),
            display_format: "yyyy-MM-dd".to_string(),
            calendar_popup: false,
            date_changed: Signal1::new(),
        }
    }
    pub fn date(&self) -> Date {
        self.date
    }
    pub fn minimum_date(&self) -> Date {
        self.minimum
    }
    pub fn maximum_date(&self) -> Date {
        self.maximum
    }
    pub fn display_format(&self) -> &str {
        &self.display_format
    }
    pub fn calendar_popup(&self) -> bool {
        self.calendar_popup
    }
    pub fn set_date(&mut self, date: Date) {
        if date.is_valid() && date >= self.minimum && date <= self.maximum && self.date != date {
            self.date = date;
            self.date_changed.emit(date);
        }
    }
    pub fn set_minimum_date(&mut self, date: Date) {
        self.minimum = date;
    }
    pub fn set_maximum_date(&mut self, date: Date) {
        self.maximum = date;
    }
    /// Sets both minimum and maximum dates in one call.
    /// This is a convenience writer; query bounds via `minimum_date()` and `maximum_date()`.
    pub fn set_date_range(&mut self, min: Date, max: Date) {
        self.minimum = min;
        self.maximum = max;
    }
    pub fn set_display_format(&mut self, fmt: String) {
        self.display_format = fmt;
    }
    pub fn set_calendar_popup(&mut self, popup: bool) {
        self.calendar_popup = popup;
    }
    pub fn step_up(&mut self) {
        let mut d = self.date;
        d.set_day(d.day() + 1);
        if d.day() > d.days_in_month() {
            d.set_day(1);
            d.set_month(d.month() + 1);
        }
        if d.month() > 12 {
            d.set_month(1);
            d.set_year(d.year() + 1);
        }
        self.set_date(d);
    }
    pub fn step_down(&mut self) {
        let mut d = self.date;
        if d.day() > 1 {
            d.set_day(d.day() - 1);
        } else {
            if d.month() > 1 {
                d.set_month(d.month() - 1);
            } else {
                d.set_month(12);
                d.set_year(d.year() - 1);
            }
            d.set_day(d.days_in_month());
        }
        self.set_date(d);
    }
}
impl Widget for DateEdit {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}
impl EventHandler for DateEdit {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        if let Event::KeyPress { key, .. } = event {
            match *key {
                38 => self.step_up(),   // Up arrow
                40 => self.step_down(), // Down arrow
                _ => {}
            }
        }
    }
}
impl Draw for DateEdit {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        context.fill_rect(rect, Color::from_rgb(255, 255, 255));
        context.draw_rect(rect, Color::from_rgb(150, 150, 150));
        let text = self.date.to_string();
        context.draw_text(
            Point { x: rect.x + 6, y: rect.y + (rect.height as i32 / 2) },
            &text,
            &Font::default(),
            Color::from_rgb(0, 0, 0),
        );
    }
}
