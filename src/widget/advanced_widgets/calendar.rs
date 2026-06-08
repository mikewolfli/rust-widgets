//! Calendar widget.
use crate::core::{Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use chrono::Datelike;
/// Calendar widget.
pub struct Calendar {
    base: BaseWidget,
    selected_date: chrono::NaiveDate,
    minimum_date: chrono::NaiveDate,
    maximum_date: chrono::NaiveDate,
    first_day_of_week: chrono::Weekday,
    grid_visible: bool,
    navigation_bar_visible: bool,
    horizontal_header_visible: bool,
    vertical_header_visible: bool,
    pub selection_changed: Signal1<chrono::NaiveDate>,
    date_format: String,
}
impl Calendar {
    /// Creates a calendar widget.
    pub fn new(geometry: Rect) -> Self {
        let today = chrono::Local::now().date_naive();
        Self {
            base: BaseWidget::new(WidgetKind::Calendar, geometry, "Calendar"),
            selected_date: today,
            // SAFETY: 1900-01-01 is a valid Gregorian date.
            minimum_date: chrono::NaiveDate::from_ymd_opt(1900, 1, 1)
                .expect("1900-01-01 is a valid date"),
            // SAFETY: 3000-12-31 is a valid Gregorian date.
            maximum_date: chrono::NaiveDate::from_ymd_opt(3000, 12, 31)
                .expect("3000-12-31 is a valid date"),
            first_day_of_week: chrono::Weekday::Mon,
            grid_visible: true,
            navigation_bar_visible: true,
            horizontal_header_visible: true,
            vertical_header_visible: false,
            selection_changed: Signal1::new(),
            date_format: "%Y-%m-%d".to_string(),
        }
    }
    /// Returns selected date.
    pub fn selected_date(&self) -> chrono::NaiveDate {
        self.selected_date
    }
    /// Sets selected date.
    pub fn set_selected_date(&mut self, date: chrono::NaiveDate) {
        if self.selected_date != date && date >= self.minimum_date && date <= self.maximum_date {
            self.selected_date = date;
            self.selection_changed.emit(date);
        }
    }
    /// Returns minimum date.
    pub fn minimum_date(&self) -> chrono::NaiveDate {
        self.minimum_date
    }
    /// Sets minimum date.
    pub fn set_minimum_date(&mut self, date: chrono::NaiveDate) {
        self.minimum_date = date;
        if self.selected_date < date {
            self.set_selected_date(date);
        }
    }
    /// Returns maximum date.
    pub fn maximum_date(&self) -> chrono::NaiveDate {
        self.maximum_date
    }
    /// Sets maximum date.
    pub fn set_maximum_date(&mut self, date: chrono::NaiveDate) {
        self.maximum_date = date;
        if self.selected_date > date {
            self.set_selected_date(date);
        }
    }
    /// Returns first day of week.
    pub fn first_day_of_week(&self) -> chrono::Weekday {
        self.first_day_of_week
    }
    /// Sets first day of week.
    pub fn set_first_day_of_week(&mut self, weekday: chrono::Weekday) {
        self.first_day_of_week = weekday;
    }
    /// Returns whether grid is visible.
    pub fn is_grid_visible(&self) -> bool {
        self.grid_visible
    }
    /// Sets grid visibility.
    pub fn set_grid_visible(&mut self, visible: bool) {
        self.grid_visible = visible;
    }
    /// Returns whether navigation bar is visible.
    pub fn is_navigation_bar_visible(&self) -> bool {
        self.navigation_bar_visible
    }
    /// Sets navigation bar visibility.
    pub fn set_navigation_bar_visible(&mut self, visible: bool) {
        self.navigation_bar_visible = visible;
    }
    /// Returns whether horizontal header is visible.
    pub fn is_horizontal_header_visible(&self) -> bool {
        self.horizontal_header_visible
    }
    /// Sets horizontal header visibility.
    pub fn set_horizontal_header_visible(&mut self, visible: bool) {
        self.horizontal_header_visible = visible;
    }
    /// Returns whether vertical header is visible.
    pub fn is_vertical_header_visible(&self) -> bool {
        self.vertical_header_visible
    }
    /// Sets vertical header visibility.
    pub fn set_vertical_header_visible(&mut self, visible: bool) {
        self.vertical_header_visible = visible;
    }
    /// Shows today's date.
    pub fn show_today(&mut self) {
        let today = chrono::Local::now().date_naive();
        self.set_selected_date(today);
    }
    /// Shows next month.
    pub fn show_next_month(&mut self) {
        if let Some(next_month) = self.selected_date.with_month(self.selected_date.month() + 1) {
            self.set_selected_date(next_month);
        } else if let Some(next_year) = self.selected_date.with_year(self.selected_date.year() + 1)
        {
            // SAFETY: January (month 1) exists in every year.
            if let Some(jan) = next_year.with_month(1) {
                self.set_selected_date(jan);
            }
        }
    }
    /// Shows previous month.
    pub fn show_previous_month(&mut self) {
        if let Some(prev_month) = self.selected_date.with_month(self.selected_date.month() - 1) {
            self.set_selected_date(prev_month);
        } else if let Some(prev_year) = self.selected_date.with_year(self.selected_date.year() - 1)
        {
            // SAFETY: December (month 12) exists in every year.
            if let Some(dec) = prev_year.with_month(12) {
                self.set_selected_date(dec);
            }
        }
    }
    /// Shows next year.
    pub fn show_next_year(&mut self) {
        if let Some(next_year) = self.selected_date.with_year(self.selected_date.year() + 1) {
            self.set_selected_date(next_year);
        }
    }
    /// Shows previous year.
    pub fn show_previous_year(&mut self) {
        if let Some(prev_year) = self.selected_date.with_year(self.selected_date.year() - 1) {
            self.set_selected_date(prev_year);
        }
    }
    /// Returns the current date format string.
    pub fn date_format(&self) -> &str {
        &self.date_format
    }

    /// Sets the date format string (uses chrono format specifiers).
    pub fn set_date_format(&mut self, format: String) {
        self.date_format = format;
    }

    /// Returns date at position.
    fn date_at_position(&self, pos: Point) -> Option<chrono::NaiveDate> {
        let rect = self.geometry();
        let cell_width = (rect.width / 7).max(1) as i32;
        let cell_height = (rect.height / 8).max(1) as i32;
        let col = (pos.x - rect.x) / cell_width;
        let row = (pos.y - rect.y) / cell_height;
        if !(0..7).contains(&col) || !(0..8).contains(&row) {
            return None;
        }
        // Calculate date based on row and column
        // SAFETY: Day 1 exists in every month.
        let first_day = self.selected_date.with_day(1).expect("day 1 exists in every month");
        let first_weekday = first_day.weekday();
        let days_from_start =
            (row - 1) * 7 + col - (first_weekday.num_days_from_monday() as i32) + 1;
        first_day.checked_add_signed(chrono::TimeDelta::days(days_from_start as i64))
    }
}
// Implement Widget trait
impl Widget for Calendar {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}
impl EventHandler for Calendar {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button } if *button == 1 => {
                if let Some(date) = self.date_at_position(*pos) {
                    self.set_selected_date(date);
                }
            }
            Event::KeyPress { key, modifiers: _ } => {
                match *key {
                    37 => {
                        // Left arrow
                        if let Some(prev_day) = self.selected_date.pred_opt() {
                            self.set_selected_date(prev_day);
                        }
                    }
                    38 => {
                        // Up arrow
                        if let Some(prev_week) =
                            self.selected_date.checked_sub_signed(chrono::TimeDelta::days(7))
                        {
                            self.set_selected_date(prev_week);
                        }
                    }
                    39 => {
                        // Right arrow
                        if let Some(next_day) = self.selected_date.succ_opt() {
                            self.set_selected_date(next_day);
                        }
                    }
                    40 => {
                        // Down arrow
                        if let Some(next_week) =
                            self.selected_date.checked_add_signed(chrono::TimeDelta::days(7))
                        {
                            self.set_selected_date(next_week);
                        }
                    }
                    33 => {
                        // Page up
                        self.show_previous_month();
                    }
                    34 => {
                        // Page down
                        self.show_next_month();
                    }
                    36 => {
                        // Home
                        self.show_today();
                    }
                    _ => { /* Other keys are not relevant */ }
                }
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}

impl Draw for Calendar {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        context.fill_rect(rect, Color::from_rgb(255, 255, 255));
        context.draw_rect(rect, Color::from_rgb(190, 190, 190));
        let title = format!("{} {}", self.selected_date.format("%B"), self.selected_date.year());
        context.draw_text(
            Point { x: rect.x + 8, y: rect.y + 8 },
            &title,
            &Font::default(),
            Color::from_rgb(40, 40, 40),
        );
        let value = self.selected_date.format(&self.date_format).to_string();
        context.draw_text(
            Point { x: rect.x + 8, y: rect.y + 28 },
            &value,
            &Font::default(),
            Color::from_rgb(20, 20, 20),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Rect;
    use chrono::NaiveDate;

    #[test]
    fn calendar_creation_defaults() {
        let cal = Calendar::new(Rect::new(0, 0, 300, 250));
        let today = chrono::Local::now().date_naive();
        assert_eq!(cal.selected_date(), today);
        assert!(cal.is_grid_visible());
        assert!(cal.is_navigation_bar_visible());
        assert!(cal.is_horizontal_header_visible());
        assert!(!cal.is_vertical_header_visible());
    }

    #[test]
    fn calendar_set_selected_date() {
        let mut cal = Calendar::new(Rect::new(0, 0, 300, 250));
        let date = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        cal.set_selected_date(date);
        assert_eq!(cal.selected_date(), date);
    }

    #[test]
    fn calendar_set_selected_date_clamps_to_range() {
        let mut cal = Calendar::new(Rect::new(0, 0, 300, 250));
        let before_min = NaiveDate::from_ymd_opt(1800, 1, 1).unwrap();
        cal.set_selected_date(before_min);
        // Should remain as today because before min
        let today = chrono::Local::now().date_naive();
        assert_eq!(cal.selected_date(), today);
    }

    #[test]
    fn calendar_minimum_date_clamps_selected() {
        let mut cal = Calendar::new(Rect::new(0, 0, 300, 250));
        let future = NaiveDate::from_ymd_opt(2050, 6, 15).unwrap();
        cal.set_selected_date(future);
        assert_eq!(cal.selected_date(), future);
        // Move minimum past selected - should reclamp
        let later = NaiveDate::from_ymd_opt(2060, 1, 1).unwrap();
        cal.set_minimum_date(later);
        assert_eq!(cal.selected_date(), later);
    }

    #[test]
    fn calendar_maximum_date_clamps_selected() {
        let mut cal = Calendar::new(Rect::new(0, 0, 300, 250));
        let past = NaiveDate::from_ymd_opt(2000, 6, 15).unwrap();
        cal.set_selected_date(past);
        assert_eq!(cal.selected_date(), past);
        // Move maximum before selected - should reclamp
        let earlier = NaiveDate::from_ymd_opt(1999, 12, 31).unwrap();
        cal.set_maximum_date(earlier);
        assert_eq!(cal.selected_date(), earlier);
    }

    #[test]
    fn calendar_first_day_of_week() {
        let mut cal = Calendar::new(Rect::new(0, 0, 300, 250));
        assert_eq!(cal.first_day_of_week(), chrono::Weekday::Mon);
        cal.set_first_day_of_week(chrono::Weekday::Sun);
        assert_eq!(cal.first_day_of_week(), chrono::Weekday::Sun);
    }

    #[test]
    fn calendar_show_today() {
        let mut cal = Calendar::new(Rect::new(0, 0, 300, 250));
        let past = NaiveDate::from_ymd_opt(2020, 1, 1).unwrap();
        cal.set_selected_date(past);
        cal.show_today();
        let today = chrono::Local::now().date_naive();
        assert_eq!(cal.selected_date(), today);
    }

    #[test]
    fn calendar_navigation() {
        let mut cal = Calendar::new(Rect::new(0, 0, 300, 250));
        let base = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        cal.set_selected_date(base);

        cal.show_next_month();
        assert_eq!(cal.selected_date().month(), 7);

        cal.show_previous_month();
        assert_eq!(cal.selected_date().month(), 6);

        cal.show_next_year();
        assert_eq!(cal.selected_date().year(), 2027);

        cal.show_previous_year();
        assert_eq!(cal.selected_date().year(), 2026);
    }

    #[test]
    fn calendar_keyboard_navigation() {
        let mut cal = Calendar::new(Rect::new(0, 0, 300, 250));
        let base = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        cal.set_selected_date(base);

        // Right arrow (key 39) advances one day
        cal.handle_event(&Event::KeyPress { key: 39, modifiers: 0 });
        assert_eq!(cal.selected_date(), NaiveDate::from_ymd_opt(2026, 6, 16).unwrap());

        // Left arrow (key 37) goes back one day
        cal.handle_event(&Event::KeyPress { key: 37, modifiers: 0 });
        assert_eq!(cal.selected_date(), NaiveDate::from_ymd_opt(2026, 6, 15).unwrap());

        // Down arrow (key 40) advances by week
        cal.handle_event(&Event::KeyPress { key: 40, modifiers: 0 });
        assert_eq!(cal.selected_date(), NaiveDate::from_ymd_opt(2026, 6, 22).unwrap());

        // Up arrow (key 38) goes back by week
        cal.handle_event(&Event::KeyPress { key: 38, modifiers: 0 });
        assert_eq!(cal.selected_date(), NaiveDate::from_ymd_opt(2026, 6, 15).unwrap());

        // Page down (key 34) advances by month
        cal.handle_event(&Event::KeyPress { key: 34, modifiers: 0 });
        assert_eq!(cal.selected_date().month(), 7);

        // Page up (key 33) goes back by month
        cal.handle_event(&Event::KeyPress { key: 33, modifiers: 0 });
        assert_eq!(cal.selected_date().month(), 6);
    }

    #[test]
    fn calendar_grid_visibility() {
        let mut cal = Calendar::new(Rect::new(0, 0, 300, 250));
        assert!(cal.is_grid_visible());
        cal.set_grid_visible(false);
        assert!(!cal.is_grid_visible());
        cal.set_grid_visible(true);
        assert!(cal.is_grid_visible());
    }

    #[test]
    fn calendar_navigation_bar_visibility() {
        let mut cal = Calendar::new(Rect::new(0, 0, 300, 250));
        assert!(cal.is_navigation_bar_visible());
        cal.set_navigation_bar_visible(false);
        assert!(!cal.is_navigation_bar_visible());
    }

    #[test]
    fn calendar_headers_visibility() {
        let mut cal = Calendar::new(Rect::new(0, 0, 300, 250));
        assert!(cal.is_horizontal_header_visible());
        cal.set_horizontal_header_visible(false);
        assert!(!cal.is_horizontal_header_visible());
        assert!(!cal.is_vertical_header_visible());
        cal.set_vertical_header_visible(true);
        assert!(cal.is_vertical_header_visible());
    }

    #[test]
    fn calendar_date_format() {
        let mut cal = Calendar::new(Rect::new(0, 0, 300, 250));
        assert_eq!(cal.date_format(), "%Y-%m-%d");
        cal.set_date_format("%d/%m/%Y".to_string());
        assert_eq!(cal.date_format(), "%d/%m/%Y");
    }

    #[test]
    fn calendar_signal_accessors() {
        let cal = Calendar::new(Rect::new(0, 0, 300, 250));
        let _ = &cal.selection_changed;
    }

    #[test]
    fn calendar_geometry_delegation() {
        let mut cal = Calendar::new(Rect::new(0, 0, 300, 250));
        cal.set_geometry(Rect::new(10, 10, 400, 350));
        assert_eq!(cal.geometry(), Rect::new(10, 10, 400, 350));
    }

    #[test]
    fn calendar_draw_produces_svg_output() {
        let mut cal = Calendar::new(Rect::new(0, 0, 300, 250));
        let svg = crate::widget::svg::render_to_svg(&mut cal);
        assert!(svg.starts_with("<svg"));
    }
}
