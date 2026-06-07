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
        Self { year, month, day }
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
        let next_day = d.day() as i32 + 1;
        if next_day > d.days_in_month() as i32 {
            d.day = 1;
            let next_month = d.month() as i32 + 1;
            if next_month > 12 {
                d.month = 1;
                d.year += 1;
            } else {
                d.month = next_month as u8;
            }
        } else {
            d.day = next_day as u8;
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    // ─── Date helper tests ───────────────────────────────────────

    #[test]
    fn date_creation_and_accessors() {
        let d = Date::new(2026, 6, 8);
        assert_eq!(d.year(), 2026);
        assert_eq!(d.month(), 6);
        assert_eq!(d.day(), 8);
    }

    #[test]
    fn date_clamps_to_valid_ranges() {
        // Date::new no longer clamps; is_valid() checks bounds
        let d = Date::new(2024, 13, 32);
        assert_eq!(d.month(), 13);
        assert_eq!(d.day(), 32);
        assert!(!d.is_valid());
    }

    #[test]
    fn date_is_leap_year() {
        assert!(Date::new(2024, 1, 1).is_leap_year());
        assert!(Date::new(2000, 1, 1).is_leap_year());
        assert!(!Date::new(2023, 1, 1).is_leap_year());
        assert!(!Date::new(1900, 1, 1).is_leap_year());
    }

    #[test]
    fn date_days_in_month() {
        let jan = Date::new(2026, 1, 1);
        assert_eq!(jan.days_in_month(), 31);

        let feb_normal = Date::new(2023, 2, 1); // not leap
        assert_eq!(feb_normal.days_in_month(), 28);

        let feb_leap = Date::new(2024, 2, 1); // leap
        assert_eq!(feb_leap.days_in_month(), 29);

        let apr = Date::new(2026, 4, 1);
        assert_eq!(apr.days_in_month(), 30);
    }

    #[test]
    fn date_is_valid() {
        assert!(Date::new(2026, 6, 8).is_valid());
        assert!(Date::new(2024, 2, 29).is_valid()); // leap
        assert!(!Date::new(2023, 2, 29).is_valid()); // not leap
        assert!(!Date::new(2026, 0, 1).is_valid()); // month 0
        assert!(!Date::new(2026, 13, 1).is_valid()); // month 13
        assert!(!Date::new(2026, 1, 0).is_valid()); // day 0
        assert!(!Date::new(2026, 4, 31).is_valid()); // April has 30 days
    }

    #[test]
    fn date_setters_modify_correctly() {
        let mut d = Date::new(2026, 6, 8);
        d.set_year(2027);
        assert_eq!(d.year(), 2027);

        d.set_month(12);
        assert_eq!(d.month(), 12);

        d.set_day(25);
        assert_eq!(d.day(), 25);
    }

    #[test]
    fn date_setters_clamp_values() {
        let mut d = Date::new(2026, 6, 8);
        d.set_month(0);
        assert_eq!(d.month(), 1);
        d.set_month(13);
        assert_eq!(d.month(), 12);
        d.set_day(0);
        assert_eq!(d.day(), 1);
        d.set_day(32);
        assert_eq!(d.day(), 31);
    }

    #[test]
    fn date_display_format() {
        let d = Date::new(2026, 6, 8);
        assert_eq!(d.to_string(), "2026-06-08");

        let d2 = Date::new(2024, 12, 25);
        assert_eq!(d2.to_string(), "2024-12-25");
    }

    #[test]
    fn date_ordering() {
        let earlier = Date::new(2024, 1, 1);
        let later = Date::new(2026, 6, 8);
        assert!(earlier < later);
        assert!(later > earlier);
        assert_eq!(earlier, Date::new(2024, 1, 1));
    }

    #[test]
    fn date_today() {
        let today = Date::today();
        assert_eq!(today.year(), 2024);
        assert_eq!(today.month(), 1);
        assert_eq!(today.day(), 1);
    }

    // ─── DateEdit widget tests ───────────────────────────────────

    #[test]
    fn date_edit_creation_defaults() {
        let editor = DateEdit::new(Rect::new(0, 0, 200, 30));
        assert_eq!(editor.date(), Date::today());
        assert_eq!(editor.display_format(), "yyyy-MM-dd");
        assert!(!editor.calendar_popup());
        assert!(editor.is_visible());
        assert!(editor.is_enabled());
    }

    #[test]
    fn date_edit_set_date() {
        let mut editor = DateEdit::new(Rect::new(0, 0, 200, 30));
        let d = Date::new(2026, 12, 25);
        editor.set_date(d);
        assert_eq!(editor.date(), d);
    }

    #[test]
    fn date_edit_set_date_clamps_to_range() {
        let mut editor = DateEdit::new(Rect::new(0, 0, 200, 30));
        // DateEdit default minimum is 1752-09-14, so 1700 is out of range
        let before_min = Date::new(1700, 1, 1);
        editor.set_date(before_min);
        assert_eq!(editor.date(), Date::today()); // Should stay as today
    }

    #[test]
    fn date_edit_set_date_clamps_to_maximum() {
        let mut editor = DateEdit::new(Rect::new(0, 0, 200, 30));
        // DateEdit default maximum is 9999-12-31, so 10000 is out of range
        let after_max = Date::new(10000, 1, 1);
        editor.set_date(after_max);
        assert_eq!(editor.date(), Date::today()); // Should stay as today
    }

    #[test]
    fn date_edit_set_date_range() {
        let mut editor = DateEdit::new(Rect::new(0, 0, 200, 30));
        let min = Date::new(2020, 1, 1);
        let max = Date::new(2030, 12, 31);
        editor.set_date_range(min, max);
        assert_eq!(editor.minimum_date(), min);
        assert_eq!(editor.maximum_date(), max);
    }

    #[test]
    fn date_edit_set_display_format() {
        let mut editor = DateEdit::new(Rect::new(0, 0, 200, 30));
        editor.set_display_format("dd/MM/yyyy".to_string());
        assert_eq!(editor.display_format(), "dd/MM/yyyy");
    }

    #[test]
    fn date_edit_set_calendar_popup() {
        let mut editor = DateEdit::new(Rect::new(0, 0, 200, 30));
        assert!(!editor.calendar_popup());
        editor.set_calendar_popup(true);
        assert!(editor.calendar_popup());
    }

    #[test]
    fn date_edit_step_up() {
        let mut editor = DateEdit::new(Rect::new(0, 0, 200, 30));
        editor.set_date(Date::new(2026, 6, 8));
        editor.step_up();
        assert_eq!(editor.date(), Date::new(2026, 6, 9));
    }

    #[test]
    fn date_edit_step_up_rolls_month() {
        let mut editor = DateEdit::new(Rect::new(0, 0, 200, 30));
        editor.set_date(Date::new(2026, 6, 30));
        editor.step_up();
        assert_eq!(editor.date(), Date::new(2026, 7, 1));
    }

    #[test]
    fn date_edit_step_up_rolls_year() {
        let mut editor = DateEdit::new(Rect::new(0, 0, 200, 30));
        editor.set_date(Date::new(2026, 12, 31));
        editor.step_up();
        assert_eq!(editor.date(), Date::new(2027, 1, 1));
    }

    #[test]
    fn date_edit_step_down() {
        let mut editor = DateEdit::new(Rect::new(0, 0, 200, 30));
        editor.set_date(Date::new(2026, 6, 8));
        editor.step_down();
        assert_eq!(editor.date(), Date::new(2026, 6, 7));
    }

    #[test]
    fn date_edit_step_down_rolls_month() {
        let mut editor = DateEdit::new(Rect::new(0, 0, 200, 30));
        editor.set_date(Date::new(2026, 6, 1));
        editor.step_down();
        assert_eq!(editor.date(), Date::new(2026, 5, 31));
    }

    #[test]
    fn date_edit_step_down_rolls_year() {
        let mut editor = DateEdit::new(Rect::new(0, 0, 200, 30));
        editor.set_date(Date::new(2026, 1, 1));
        editor.step_down();
        assert_eq!(editor.date(), Date::new(2025, 12, 31));
    }

    #[test]
    fn date_edit_date_changed_signal_emits() {
        let mut editor = DateEdit::new(Rect::new(0, 0, 200, 30));
        let captured = Arc::new(Mutex::new(Date::new(0, 1, 1)));
        let sink = captured.clone();
        editor.date_changed.connect(move |d| {
            if let Ok(mut c) = sink.lock() {
                *c = *d;
            }
        });

        let expected = Date::new(2027, 4, 15);
        editor.set_date(expected);
        let got = *captured.lock().unwrap();
        assert_eq!(got, expected);
    }

    #[test]
    fn date_edit_keyboard_navigation() {
        let mut editor = DateEdit::new(Rect::new(0, 0, 200, 30));
        editor.set_date(Date::new(2026, 6, 8));

        // Up arrow (key 38) = step_up
        editor.handle_event(&Event::KeyPress { key: 38, modifiers: 0 });
        assert_eq!(editor.date(), Date::new(2026, 6, 9));

        // Down arrow (key 40) = step_down
        editor.handle_event(&Event::KeyPress { key: 40, modifiers: 0 });
        assert_eq!(editor.date(), Date::new(2026, 6, 8));
    }

    #[test]
    fn date_edit_geometry_delegation() {
        let rect = Rect::new(10, 20, 200, 30);
        let editor = DateEdit::new(rect);
        assert_eq!(editor.geometry(), rect);
        assert_eq!(editor.position(), Point::new(10, 20));
        assert_eq!(editor.size(), crate::core::Size::new(200, 30));
    }

    #[test]
    fn date_edit_visibility() {
        let mut editor = DateEdit::new(Rect::new(0, 0, 200, 30));
        assert!(editor.is_visible());
        editor.set_visible(false);
        assert!(!editor.is_visible());
        editor.set_visible(true);
        assert!(editor.is_visible());
    }

    #[test]
    fn date_edit_kind() {
        let editor = DateEdit::new(Rect::new(0, 0, 200, 30));
        assert_eq!(editor.kind(), WidgetKind::DatePicker);
    }

    #[test]
    fn date_edit_ids_are_unique() {
        let a = DateEdit::new(Rect::new(0, 0, 200, 30));
        let b = DateEdit::new(Rect::new(0, 0, 200, 30));
        assert_ne!(a.id(), b.id());
    }
}
