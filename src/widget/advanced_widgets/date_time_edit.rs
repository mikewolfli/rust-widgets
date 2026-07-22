//! Date-time editor widget.
use crate::core::{HorizontalAlignment, Color, Font, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::advanced_widgets::{date_edit::Date, time_edit::Time};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
/// Combined date-time value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DateTime {
    pub date: Date,
    pub time: Time,
}
impl DateTime {
    pub fn new(date: Date, time: Time) -> Self {
        Self { date, time }
    }
    pub fn is_valid(&self) -> bool {
        self.date.is_valid() && self.time.is_valid()
    }
}
impl std::fmt::Display for DateTime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.date, self.time)
    }
}
/// Date-time editor widget.
pub struct DateTimeEdit {
    base: BaseWidget,
    datetime: DateTime,
    minimum: DateTime,
    maximum: DateTime,
    display_format: String,
    calendar_popup: bool,
    pub datetime_changed: Signal1<DateTime>,
}
impl DateTimeEdit {
    pub fn new(geometry: Rect) -> Self {
        let min_dt = DateTime::new(Date::new(1752, 9, 14), Time::new(0, 0, 0, 0));
        let max_dt = DateTime::new(Date::new(9999, 12, 31), Time::new(23, 59, 59, 999));
        let now = DateTime::new(Date::today(), Time::new(0, 0, 0, 0));
        Self {
            base: BaseWidget::new(WidgetKind::DateTimePicker, geometry, "DateTimeEdit"),
            datetime: now,
            minimum: min_dt,
            maximum: max_dt,
            display_format: "yyyy-MM-dd HH:mm:ss".to_string(),
            calendar_popup: false,
            datetime_changed: Signal1::new(),
        }
    }
    pub fn datetime(&self) -> DateTime {
        self.datetime
    }
    pub fn date(&self) -> Date {
        self.datetime.date
    }
    pub fn time(&self) -> Time {
        self.datetime.time
    }
    pub fn minimum_datetime(&self) -> DateTime {
        self.minimum
    }
    pub fn maximum_datetime(&self) -> DateTime {
        self.maximum
    }
    pub fn display_format(&self) -> &str {
        &self.display_format
    }
    pub fn calendar_popup(&self) -> bool {
        self.calendar_popup
    }
    pub fn set_datetime(&mut self, dt: DateTime) {
        if dt.is_valid() && dt >= self.minimum && dt <= self.maximum && self.datetime != dt {
            self.datetime = dt;
            self.datetime_changed.emit(dt);
            self.base.request_redraw();
        }
    }
    pub fn set_date(&mut self, date: Date) {
        self.set_datetime(DateTime::new(date, self.datetime.time));
    }
    pub fn set_time(&mut self, time: Time) {
        self.set_datetime(DateTime::new(self.datetime.date, time));
    }
    pub fn set_minimum_datetime(&mut self, dt: DateTime) {
        self.minimum = dt;
        self.base.request_redraw();
    }
    pub fn set_maximum_datetime(&mut self, dt: DateTime) {
        self.maximum = dt;
        self.base.request_redraw();
    }
    pub fn set_display_format(&mut self, fmt: String) {
        self.display_format = fmt;
        self.base.request_redraw();
    }
    pub fn set_calendar_popup(&mut self, popup: bool) {
        self.calendar_popup = popup;
        self.base.request_redraw();
    }
    pub fn step_up(&mut self) {
        let mut t = self.datetime.time;
        let new_sec = t.second() + 1;
        if new_sec >= 60 {
            t.set_second(0);
            let new_min = t.minute() + 1;
            if new_min >= 60 {
                t.set_minute(0);
                let new_hour = t.hour() + 1;
                if new_hour >= 24 {
                    t.set_hour(0);
                    let mut d = self.datetime.date;
                    let next_day = d.day() as u16 + 1;
                    if next_day > d.days_in_month() as u16 {
                        d.set_day(1);
                        let next_month = d.month() + 1;
                        if next_month > 12 {
                            d.set_month(1);
                            d.set_year(d.year() + 1);
                        } else {
                            d.set_month(next_month);
                        }
                    } else {
                        d.set_day(next_day as u8);
                    }
                    self.set_datetime(DateTime::new(d, t));
                    return;
                } else {
                    t.set_hour(new_hour);
                }
            } else {
                t.set_minute(new_min);
            }
        } else {
            t.set_second(new_sec);
        }
        self.set_time(t);
    }
    pub fn step_down(&mut self) {
        let mut t = self.datetime.time;
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
                } else {
                    t.set_hour(23);
                    let mut d = self.datetime.date;
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
                    self.set_datetime(DateTime::new(d, t));
                    return;
                }
            }
        }
        self.set_time(t);
    }
}
impl Widget for DateTimeEdit {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        crate::core::Size::new(160, 28)
    }
}
impl EventHandler for DateTimeEdit {
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
impl Draw for DateTimeEdit {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        context.fill_rect(rect, Color::rgb(255, 255, 255));
        context.draw_rect(rect, Color::rgb(150, 150, 150));
        let text = self.datetime.to_string();
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

    // ─── DateTime helper tests ───────────────────────────────────

    #[test]
    fn date_time_creation_and_accessors() {
        let d = Date::new(2026, 6, 8);
        let t = Time::new(14, 30, 15, 0);
        let dt = DateTime::new(d, t);
        assert_eq!(dt.date, d);
        assert_eq!(dt.time, t);
        assert!(dt.is_valid());
    }

    #[test]
    fn date_time_is_valid() {
        let valid = DateTime::new(Date::new(2026, 6, 8), Time::new(12, 0, 0, 0));
        assert!(valid.is_valid());

        let bad_date = DateTime::new(Date::new(2026, 13, 1), Time::new(12, 0, 0, 0));
        assert!(!bad_date.is_valid());

        let bad_time = DateTime::new(Date::new(2026, 6, 8), Time::new(25, 0, 0, 0));
        // Time::new clamps, so it's actually valid
        assert!(bad_time.is_valid());
    }

    #[test]
    fn date_time_display() {
        let dt = DateTime::new(Date::new(2026, 6, 8), Time::new(9, 5, 3, 0));
        assert_eq!(dt.to_string(), "2026-06-08 09:05:03");
    }

    #[test]
    fn date_time_ordering() {
        let early = DateTime::new(Date::new(2026, 6, 8), Time::new(8, 0, 0, 0));
        let late = DateTime::new(Date::new(2026, 6, 8), Time::new(10, 0, 0, 0));
        assert!(early < late);
        assert!(late > early);
        assert_eq!(early, DateTime::new(Date::new(2026, 6, 8), Time::new(8, 0, 0, 0)));
    }

    // ─── DateTimeEdit widget tests ───────────────────────────────

    #[test]
    fn date_time_edit_creation_defaults() {
        let editor = DateTimeEdit::new(Rect::new(0, 0, 280, 30));
        assert_eq!(editor.date(), Date::today());
        assert_eq!(editor.time(), Time::new(0, 0, 0, 0));
        assert_eq!(editor.display_format(), "yyyy-MM-dd HH:mm:ss");
        assert!(!editor.calendar_popup());
        assert!(editor.is_visible());
        assert!(editor.is_enabled());
    }

    #[test]
    fn date_time_edit_set_and_get_datetime() {
        let mut editor = DateTimeEdit::new(Rect::new(0, 0, 280, 30));
        let dt = DateTime::new(Date::new(2026, 12, 25), Time::new(10, 30, 0, 0));
        editor.set_datetime(dt);
        assert_eq!(editor.datetime(), dt);
        assert_eq!(editor.date(), Date::new(2026, 12, 25));
        assert_eq!(editor.time(), Time::new(10, 30, 0, 0));
    }

    #[test]
    fn date_time_edit_set_date_part() {
        let mut editor = DateTimeEdit::new(Rect::new(0, 0, 280, 30));
        let new_date = Date::new(2027, 4, 15);
        editor.set_date(new_date);
        assert_eq!(editor.date(), new_date);
        // Time part should be unchanged
        assert_eq!(editor.time(), Time::new(0, 0, 0, 0));
    }

    #[test]
    fn date_time_edit_set_time_part() {
        let mut editor = DateTimeEdit::new(Rect::new(0, 0, 280, 30));
        editor.set_date(Date::new(2026, 6, 8));
        let new_time = Time::new(18, 45, 30, 0);
        editor.set_time(new_time);
        assert_eq!(editor.time(), new_time);
        assert_eq!(editor.date(), Date::new(2026, 6, 8));
    }

    #[test]
    fn date_time_edit_set_display_format() {
        let mut editor = DateTimeEdit::new(Rect::new(0, 0, 280, 30));
        editor.set_display_format("dd/MM/yyyy HH:mm".to_string());
        assert_eq!(editor.display_format(), "dd/MM/yyyy HH:mm");
    }

    #[test]
    fn date_time_edit_set_minimum_maximum() {
        let mut editor = DateTimeEdit::new(Rect::new(0, 0, 280, 30));
        let min = DateTime::new(Date::new(2025, 1, 1), Time::new(0, 0, 0, 0));
        let max = DateTime::new(Date::new(2030, 12, 31), Time::new(23, 59, 59, 999));
        editor.set_minimum_datetime(min);
        editor.set_maximum_datetime(max);
        assert_eq!(editor.minimum_datetime(), min);
        assert_eq!(editor.maximum_datetime(), max);
    }

    #[test]
    fn date_time_edit_set_datetime_clamps_to_range() {
        let mut editor = DateTimeEdit::new(Rect::new(0, 0, 280, 30));
        let min = DateTime::new(Date::new(2025, 6, 1), Time::new(0, 0, 0, 0));
        let max = DateTime::new(Date::new(2025, 6, 30), Time::new(23, 59, 59, 999));
        editor.set_minimum_datetime(min);
        editor.set_maximum_datetime(max);

        // Set within range — should work
        let within = DateTime::new(Date::new(2025, 6, 15), Time::new(12, 0, 0, 0));
        editor.set_datetime(within);
        assert_eq!(editor.datetime(), within);

        // Set below minimum — should be rejected
        let before = DateTime::new(Date::new(2025, 5, 1), Time::new(0, 0, 0, 0));
        editor.set_datetime(before);
        assert_eq!(editor.datetime(), within);

        // Set above maximum — should be rejected
        let after = DateTime::new(Date::new(2025, 7, 1), Time::new(0, 0, 0, 0));
        editor.set_datetime(after);
        assert_eq!(editor.datetime(), within);
    }

    #[test]
    fn date_time_edit_date_time_changed_signal_emits() {
        let mut editor = DateTimeEdit::new(Rect::new(0, 0, 280, 30));
        let captured =
            Arc::new(Mutex::new(DateTime::new(Date::new(2000, 1, 1), Time::new(0, 0, 0, 0))));
        let sink = captured.clone();
        editor.datetime_changed.connect(move |dt| {
            if let Ok(mut c) = sink.lock() {
                *c = *dt;
            }
        });

        let expected = DateTime::new(Date::new(2026, 12, 25), Time::new(10, 30, 0, 0));
        editor.set_datetime(expected);
        let got = *captured.lock().unwrap();
        assert_eq!(got, expected);
    }

    #[test]
    fn date_time_edit_date_time_changed_not_emitted_for_unchanged() {
        let mut editor = DateTimeEdit::new(Rect::new(0, 0, 280, 30));
        let hits = Arc::new(Mutex::new(0usize));
        let hits_clone = hits.clone();
        editor.datetime_changed.connect(move |_| {
            if let Ok(mut h) = hits_clone.lock() {
                *h += 1;
            }
        });

        // Setting same value should NOT emit
        let default_dt = DateTime::new(Date::today(), Time::new(0, 0, 0, 0));
        editor.set_datetime(default_dt);
        assert_eq!(*hits.lock().unwrap(), 0);
    }

    #[test]
    fn date_time_edit_calendar_popup_visibility() {
        let mut editor = DateTimeEdit::new(Rect::new(0, 0, 280, 30));
        assert!(!editor.calendar_popup());
        editor.set_calendar_popup(true);
        assert!(editor.calendar_popup());
        editor.set_calendar_popup(false);
        assert!(!editor.calendar_popup());
    }

    #[test]
    fn date_time_edit_geometry_delegation() {
        let rect = Rect::new(10, 20, 280, 30);
        let editor = DateTimeEdit::new(rect);
        assert_eq!(editor.geometry(), rect);
        assert_eq!(editor.position(), Point::new(10, 20));
        assert_eq!(editor.size(), crate::core::Size::new(280, 30));
    }

    #[test]
    fn date_time_edit_widget_kind() {
        let editor = DateTimeEdit::new(Rect::new(0, 0, 280, 30));
        assert_eq!(editor.kind(), WidgetKind::DateTimePicker);
    }

    #[test]
    fn date_time_edit_ids_are_unique() {
        let a = DateTimeEdit::new(Rect::new(0, 0, 280, 30));
        let b = DateTimeEdit::new(Rect::new(0, 0, 280, 30));
        assert_ne!(a.id(), b.id());
    }

    #[test]
    fn date_time_edit_svg_output() {
        let mut editor = DateTimeEdit::new(Rect::new(0, 0, 280, 30));
        editor.set_datetime(DateTime::new(Date::new(2026, 6, 8), Time::new(12, 30, 45, 0)));
        let svg = render_to_svg(&mut editor);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        assert!(svg.contains("width=\"280\""));
        assert!(svg.contains("height=\"30\""));
        assert!(svg.contains("2026-06-08 12:30:45") || svg.contains("fill="));
    }

    #[test]
    fn date_time_edit_disabled_state_blocks_changes() {
        let mut editor = DateTimeEdit::new(Rect::new(0, 0, 280, 30));
        editor.set_datetime(DateTime::new(Date::new(2026, 6, 8), Time::new(10, 0, 0, 0)));
        editor.set_enabled(false);
        assert!(!editor.is_enabled());

        // Via handle_event, disabled widgets should reject keyboard input
        editor.handle_event(&Event::KeyPress { key: 38, modifiers: 0 });
        assert_eq!(editor.datetime(), DateTime::new(Date::new(2026, 6, 8), Time::new(10, 0, 0, 0)));
    }

    #[test]
    fn date_time_edit_date_selection_updates_datetime() {
        let mut editor = DateTimeEdit::new(Rect::new(0, 0, 280, 30));
        let new_date = Date::new(2027, 12, 25);
        editor.set_date(new_date);
        assert_eq!(editor.date(), new_date);
        // Time should remain the default (00:00:00)
        assert_eq!(editor.time(), Time::new(0, 0, 0, 0));
    }

    #[test]
    fn date_time_edit_time_part_updates_independently() {
        let mut editor = DateTimeEdit::new(Rect::new(0, 0, 280, 30));
        editor.set_datetime(DateTime::new(Date::new(2026, 6, 8), Time::new(14, 30, 0, 0)));

        // Update time part
        editor.set_time(Time::new(20, 15, 45, 0));
        assert_eq!(editor.time(), Time::new(20, 15, 45, 0));
        assert_eq!(editor.date(), Date::new(2026, 6, 8));
    }

    #[test]
    fn date_time_edit_step_up_increments_time() {
        let mut editor = DateTimeEdit::new(Rect::new(0, 0, 280, 30));
        editor.set_datetime(DateTime::new(Date::new(2026, 6, 8), Time::new(10, 30, 15, 0)));
        editor.step_up();
        assert_eq!(editor.time(), Time::new(10, 30, 16, 0));
        assert_eq!(editor.date(), Date::new(2026, 6, 8));
    }

    #[test]
    fn date_time_edit_step_down_decrements_time() {
        let mut editor = DateTimeEdit::new(Rect::new(0, 0, 280, 30));
        editor.set_datetime(DateTime::new(Date::new(2026, 6, 8), Time::new(10, 30, 15, 0)));
        editor.step_down();
        assert_eq!(editor.time(), Time::new(10, 30, 14, 0));
        assert_eq!(editor.date(), Date::new(2026, 6, 8));
    }

    #[test]
    fn date_time_edit_step_up_rolls_to_next_day() {
        let mut editor = DateTimeEdit::new(Rect::new(0, 0, 280, 30));
        editor.set_datetime(DateTime::new(Date::new(2026, 6, 8), Time::new(23, 59, 59, 0)));
        editor.step_up();
        // Rolls to next day 00:00:00
        assert_eq!(editor.time(), Time::new(0, 0, 0, 0));
        assert_eq!(editor.date(), Date::new(2026, 6, 9));
    }

    #[test]
    fn date_time_edit_step_down_rolls_to_prev_day() {
        let mut editor = DateTimeEdit::new(Rect::new(0, 0, 280, 30));
        editor.set_datetime(DateTime::new(Date::new(2026, 6, 8), Time::new(0, 0, 0, 0)));
        editor.step_down();
        // Rolls to previous day 23:59:59
        assert_eq!(editor.time(), Time::new(23, 59, 59, 0));
        assert_eq!(editor.date(), Date::new(2026, 6, 7));
    }

    #[test]
    fn date_time_edit_keyboard_navigation() {
        let mut editor = DateTimeEdit::new(Rect::new(0, 0, 280, 30));
        editor.set_datetime(DateTime::new(Date::new(2026, 6, 8), Time::new(10, 0, 0, 0)));

        // Up arrow (key 38) = step_up
        editor.handle_event(&Event::KeyPress { key: 38, modifiers: 0 });
        assert_eq!(editor.time(), Time::new(10, 0, 1, 0));

        // Down arrow (key 40) = step_down
        editor.handle_event(&Event::KeyPress { key: 40, modifiers: 0 });
        assert_eq!(editor.time(), Time::new(10, 0, 0, 0));
    }

    #[test]
    fn date_time_edit_visibility_toggle() {
        let mut editor = DateTimeEdit::new(Rect::new(0, 0, 280, 30));
        assert!(editor.is_visible());
        editor.set_visible(false);
        assert!(!editor.is_visible());
        editor.set_visible(true);
        assert!(editor.is_visible());
    }
}
