//! Calendar widget.
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use chrono::Datelike;

/// Advance a date by `delta` months, clamping to the last valid day.
fn advance_month(date: chrono::NaiveDate, delta: i32) -> Option<chrono::NaiveDate> {
    let total_months = date.year() as i32 * 12 + date.month() as i32 + delta;
    let new_year = (total_months - 1) / 12;
    let new_month = ((total_months - 1) % 12) + 1;
    if new_year < 0 || new_year > 9999 {
        return None;
    }
    let day = date.day().min(max_days_in_month(new_year as i32, new_month as u32));
    chrono::NaiveDate::from_ymd_opt(new_year as i32, new_month as u32, day)
}

/// Return the number of days in a given month/year.
fn max_days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

/// Calendar widget — full-featured month-view date picker.
///
/// Layout (top to bottom):
/// 1. Navigation bar (30px, when visible): ◄ Month Year ►
/// 2. Weekday header (24px, when visible): Mon Tue Wed …
/// 3. Day grid (remaining space, when visible): 6 rows × 7 columns
///
/// Supports mouse and keyboard navigation, date range clamping,
/// and emits `selection_changed` on date selection.
pub struct Calendar {
    base: BaseWidget,
    selected_date: chrono::NaiveDate,
    /// The "displayed" month — the month currently shown in the grid
    /// (may differ from `selected_date` after month navigation).
    display_month: chrono::NaiveDate,
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
            display_month: today,
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

    /// Returns the currently displayed month (the month shown in the grid).
    pub fn display_month(&self) -> chrono::NaiveDate {
        self.display_month
    }
    /// Sets selected date and syncs the displayed month to match.
    pub fn set_selected_date(&mut self, date: chrono::NaiveDate) {
        if self.selected_date != date && date >= self.minimum_date && date <= self.maximum_date {
            self.selected_date = date;
            self.display_month = date;
            self.selection_changed.emit(date);
            self.base.request_redraw();
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
        self.base.request_redraw();
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
        self.base.request_redraw();
    }
    /// Returns first day of week.
    pub fn first_day_of_week(&self) -> chrono::Weekday {
        self.first_day_of_week
    }
    /// Sets first day of week.
    pub fn set_first_day_of_week(&mut self, weekday: chrono::Weekday) {
        self.first_day_of_week = weekday;
        self.base.request_redraw();
    }
    /// Returns whether grid is visible.
    pub fn is_grid_visible(&self) -> bool {
        self.grid_visible
    }
    /// Sets grid visibility.
    pub fn set_grid_visible(&mut self, visible: bool) {
        self.grid_visible = visible;
        self.base.request_redraw();
    }
    /// Returns whether navigation bar is visible.
    pub fn is_navigation_bar_visible(&self) -> bool {
        self.navigation_bar_visible
    }
    /// Sets navigation bar visibility.
    pub fn set_navigation_bar_visible(&mut self, visible: bool) {
        self.navigation_bar_visible = visible;
        self.base.request_redraw();
    }
    /// Returns whether horizontal header is visible.
    pub fn is_horizontal_header_visible(&self) -> bool {
        self.horizontal_header_visible
    }
    /// Sets horizontal header visibility.
    pub fn set_horizontal_header_visible(&mut self, visible: bool) {
        self.horizontal_header_visible = visible;
        self.base.request_redraw();
    }
    /// Returns whether vertical header is visible.
    pub fn is_vertical_header_visible(&self) -> bool {
        self.vertical_header_visible
    }
    /// Sets vertical header visibility.
    pub fn set_vertical_header_visible(&mut self, visible: bool) {
        self.vertical_header_visible = visible;
        self.base.request_redraw();
    }
    /// Shows today's date and resets display to current month.
    pub fn show_today(&mut self) {
        let today = chrono::Local::now().date_naive();
        self.display_month = today;
        self.set_selected_date(today);
    }
    /// Shows next month in the grid (does not change selected date).
    pub fn show_next_month(&mut self) {
        if let Some(next) = advance_month(self.display_month, 1) {
            self.display_month = next;
            self.base.request_redraw();
        }
    }
    /// Shows previous month in the grid (does not change selected date).
    pub fn show_previous_month(&mut self) {
        if let Some(prev) = advance_month(self.display_month, -1) {
            self.display_month = prev;
            self.base.request_redraw();
        }
    }
    /// Shows next year in the grid (does not change selected date).
    pub fn show_next_year(&mut self) {
        if let Some(next) = self.display_month.with_year(self.display_month.year() + 1) {
            self.display_month = next;
            self.base.request_redraw();
        }
    }
    /// Shows previous year in the grid (does not change selected date).
    pub fn show_previous_year(&mut self) {
        if let Some(prev) = self.display_month.with_year(self.display_month.year() - 1) {
            self.display_month = prev;
            self.base.request_redraw();
        }
    }
    /// Returns the current date format string.
    pub fn date_format(&self) -> &str {
        &self.date_format
    }

    /// Sets the date format string (uses chrono format specifiers).
    pub fn set_date_format(&mut self, format: String) {
        self.date_format = format;
        self.base.request_redraw();
    }

    /// Layout constants.
    const NAV_H: u32 = 30;
    const DAY_HEADER_H: u32 = 24;

    /// Returns the navigation bar rectangle, or zero-sized if hidden.
    fn nav_rect(&self) -> Rect {
        let r = self.geometry();
        if self.navigation_bar_visible {
            Rect::new(r.x, r.y, r.width, Self::NAV_H)
        } else {
            Rect::new(r.x, r.y, 0, 0)
        }
    }

    /// Returns the weekday-header rectangle, or zero-sized if hidden.
    fn day_header_rect(&self) -> Rect {
        let r = self.geometry();
        let nav_h = if self.navigation_bar_visible { Self::NAV_H as i32 } else { 0 };
        if self.horizontal_header_visible {
            Rect::new(r.x, r.y + nav_h, r.width, Self::DAY_HEADER_H)
        } else {
            Rect::new(r.x, r.y + nav_h, 0, 0)
        }
    }

    /// Returns the day-grid rectangle (remaining area after nav + header).
    fn grid_rect(&self) -> Rect {
        let r = self.geometry();
        let top = (if self.navigation_bar_visible { Self::NAV_H } else { 0 })
            + (if self.horizontal_header_visible { Self::DAY_HEADER_H } else { 0 });
        let h = r.height.saturating_sub(top);
        Rect::new(r.x, r.y + top as i32, r.width, h)
    }

    /// Compute the number of leading blank cells before day 1 of the displayed month.
    fn leading_blank_count(&self) -> u32 {
        // SAFETY: day 1 exists in every month.
        let first = self.display_month.with_day(1).expect("day 1 always valid");
        let wd = first.weekday();
        let from_mon = wd.num_days_from_monday();
        match self.first_day_of_week {
            chrono::Weekday::Mon => from_mon,
            chrono::Weekday::Sun => (from_mon + 1) % 7,
            _ => from_mon, // fallback
        }
    }

    /// Returns the date at a given pixel position, or `None` if outside the grid.
    fn date_at_position(&self, pos: Point) -> Option<chrono::NaiveDate> {
        if !self.grid_visible {
            return None;
        }
        let grid = self.grid_rect();
        if pos.x < grid.x
            || pos.x >= grid.x + grid.width as i32
            || pos.y < grid.y
            || pos.y >= grid.y + grid.height as i32
        {
            return None;
        }
        let cell_w = (grid.width / 7).max(1) as i32;
        let cell_h = (grid.height / 6).max(1) as i32;
        let col = ((pos.x - grid.x) / cell_w).clamp(0, 6);
        let row = ((pos.y - grid.y) / cell_h).clamp(0, 5);
        let day_num = row * 7 + col - self.leading_blank_count() as i32;
        // SAFETY: day 1 exists in every month.
        let first = self.display_month.with_day(1).expect("day 1 always valid");
        first
            .checked_add_signed(chrono::TimeDelta::days(day_num as i64))
            .filter(|d| d.month() == self.display_month.month())
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

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(260, 240)
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
                // Check nav-bar button clicks
                if self.navigation_bar_visible {
                    let nav = self.nav_rect();
                    // ◄ prev month: left 40px of nav bar
                    if pos.y >= nav.y
                        && pos.y < nav.y + nav.height as i32
                        && pos.x >= nav.x
                        && pos.x < nav.x + 40
                    {
                        self.show_previous_month();
                        return;
                    }
                    // ► next month: right 40px of nav bar
                    if pos.y >= nav.y
                        && pos.y < nav.y + nav.height as i32
                        && pos.x >= nav.x + nav.width as i32 - 40
                        && pos.x < nav.x + nav.width as i32
                    {
                        self.show_next_month();
                        return;
                    }
                }
                // Click on day grid
                if let Some(date) = self.date_at_position(*pos) {
                    if date >= self.minimum_date && date <= self.maximum_date {
                        self.set_selected_date(date);
                        // Keep display month synced to the selected month
                        self.display_month = date;
                    }
                }
            }
            Event::KeyPress { key, modifiers: _ } => {
                match *key {
                    37 => {
                        // Left arrow
                        if let Some(prev_day) = self.selected_date.pred_opt() {
                            self.set_selected_date(prev_day);
                            self.display_month = prev_day;
                        }
                    }
                    38 => {
                        // Up arrow
                        if let Some(prev_week) =
                            self.selected_date.checked_sub_signed(chrono::TimeDelta::days(7))
                        {
                            self.set_selected_date(prev_week);
                            self.display_month = prev_week;
                        }
                    }
                    39 => {
                        // Right arrow
                        if let Some(next_day) = self.selected_date.succ_opt() {
                            self.set_selected_date(next_day);
                            self.display_month = next_day;
                        }
                    }
                    40 => {
                        // Down arrow
                        if let Some(next_week) =
                            self.selected_date.checked_add_signed(chrono::TimeDelta::days(7))
                        {
                            self.set_selected_date(next_week);
                            self.display_month = next_week;
                        }
                    }
                    33 => {
                        // Page up — previous month
                        self.show_previous_month();
                    }
                    34 => {
                        // Page down — next month
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
        let enabled = self.base.is_enabled();
        let today = chrono::Local::now().date_naive();
        let dim_color = if enabled { Color::rgb(160, 160, 160) } else { Color::rgb(210, 210, 210) };
        let text_color = if enabled { Color::rgb(30, 30, 30) } else { Color::rgb(170, 170, 170) };
        let header_bg = Color::rgb(235, 235, 235);
        let border_color = Color::rgb(190, 190, 190);

        // ── Outer background & border ──
        context.fill_rect(rect, Color::rgb(255, 255, 255));
        context.draw_rect(rect, border_color);

        // ── 1. Navigation bar ──
        if self.navigation_bar_visible {
            let nav = self.nav_rect();
            context.fill_rect(nav, header_bg);
            // Bottom border
            context.draw_line(
                Point::new(nav.x, nav.y + nav.height as i32 - 1),
                Point::new(nav.x + nav.width as i32, nav.y + nav.height as i32 - 1),
                border_color,
            );
            // ◄ button
            let btn_w = 30i32;
            let arrow_color = if enabled { Color::rgb(60, 60, 60) } else { dim_color };
            context.draw_text(
                Point::new(nav.x + 8, nav.y + 7),
                "◀",
                &Font::default(),
                arrow_color,
                HorizontalAlignment::Left,
            );
            // Month/year title (centered)
            let title =
                format!("{} {}", self.display_month.format("%B"), self.display_month.year());
            context.draw_text(
                Point::new(nav.x + nav.width as i32 / 2, nav.y + 7),
                &title,
                &Font::bold("Arial", 13.0),
                text_color,
                HorizontalAlignment::Center,
            );
            // ► button
            context.draw_text(
                Point::new(nav.x + nav.width as i32 - 8 - btn_w, nav.y + 7),
                "▶",
                &Font::default(),
                arrow_color,
                HorizontalAlignment::Left,
            );
        }

        // ── 2. Weekday headers ──
        if self.horizontal_header_visible {
            let hdr = self.day_header_rect();
            context.fill_rect(hdr, Color::rgb(245, 245, 245));
            context.draw_line(
                Point::new(hdr.x, hdr.y + hdr.height as i32 - 1),
                Point::new(hdr.x + hdr.width as i32, hdr.y + hdr.height as i32 - 1),
                border_color,
            );
            let cell_w = (hdr.width / 7).max(1) as i32;
            let names = match self.first_day_of_week {
                chrono::Weekday::Mon => ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"],
                chrono::Weekday::Sun => ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"],
                _ => ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"],
            };
            for (i, name) in names.iter().enumerate() {
                let cx = hdr.x + cell_w * i as i32 + cell_w / 2;
                let is_weekend = i >= 5;
                let c = if !enabled {
                    dim_color
                } else if is_weekend {
                    Color::rgb(180, 60, 60)
                } else {
                    text_color
                };
                context.draw_text(
                    Point::new(cx, hdr.y + 6),
                    name,
                    &Font::bold("Arial", 11.0),
                    c,
                    HorizontalAlignment::Center,
                );
            }
        }

        // ── 3. Day grid ──
        if self.grid_visible {
            let grid = self.grid_rect();
            let cell_w = (grid.width / 7).max(1);
            let cell_h = (grid.height / 6).max(1);
            let days_in_month =
                max_days_in_month(self.display_month.year(), self.display_month.month());
            let blanks = self.leading_blank_count();
            let selected_bg = Color::rgba(51, 153, 255, 120);
            let today_bg = Color::rgba(255, 200, 50, 100);
            let today_border = Color::rgb(200, 120, 20);
            let grid_line = Color::rgb(220, 220, 220);

            for row in 0..6 {
                for col in 0..7 {
                    let cx = grid.x + col as i32 * cell_w as i32;
                    let cy = grid.y + row as i32 * cell_h as i32;
                    let cell_rect = Rect::new(cx, cy, cell_w, cell_h);
                    let day_num = (row * 7 + col) as i32 - blanks as i32 + 1;

                    if day_num < 1 || day_num > days_in_month as i32 {
                        // Out-of-month cell — leave blank
                        continue;
                    }

                    // SAFETY: day_num is within the valid range for this month.
                    let date = self
                        .display_month
                        .with_day(day_num as u32)
                        .expect("day_num validated against days_in_month");

                    // Cell background
                    let in_range = date >= self.minimum_date && date <= self.maximum_date;
                    if date == today && date == self.selected_date {
                        // Selected + today: blend selected bg over today bg not possible,
                        // so use a composite visual: fill today bg first, then selected overlay
                        context.fill_rect(cell_rect, today_bg);
                        // Overlay a subtle selected marker
                        context.fill_rounded_rect(
                            Rect::new(cx + cell_w as i32 / 2 - 2, cy + cell_h as i32 / 2 - 2, 4, 4),
                            2,
                            Color::rgb(51, 153, 255),
                        );
                    } else if date == today {
                        context.fill_rect(cell_rect, today_bg);
                        // Today border
                        context.draw_rect(cell_rect, today_border);
                    } else if date == self.selected_date {
                        context.fill_rect(cell_rect, selected_bg);
                    } else if !in_range {
                        // Outside range — dimmed
                        context.fill_rect(cell_rect, Color::rgb(248, 248, 248));
                    }

                    // Grid lines (right + bottom edges)
                    context.draw_line(
                        Point::new(cx + cell_w as i32 - 1, cy),
                        Point::new(cx + cell_w as i32 - 1, cy + cell_h as i32 - 1),
                        grid_line,
                    );
                    context.draw_line(
                        Point::new(cx, cy + cell_h as i32 - 1),
                        Point::new(cx + cell_w as i32 - 1, cy + cell_h as i32 - 1),
                        grid_line,
                    );

                    // Day number text
                    let day_color = if !in_range {
                        dim_color
                    } else if date == self.selected_date {
                        Color::rgb(255, 255, 255)
                    } else {
                        text_color
                    };
                    context.draw_text(
                        Point::new(cx + 3, cy + 3),
                        &format!("{}", day_num),
                        &Font::new("Arial", 11.0, false, false),
                        day_color,
                        HorizontalAlignment::Left,
                    );
                }
            }
        }
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
        // Navigation changes the displayed month, not the selected date.
        cal.show_next_month();
        assert_eq!(cal.display_month.month(), 7, "display should advance to July");
        assert_eq!(cal.selected_date().month(), 6, "selected date unchanged");

        cal.show_previous_month();
        assert_eq!(cal.display_month.month(), 6, "display back to June");

        cal.show_next_year();
        assert_eq!(cal.display_month.year(), 2027, "display year advances");

        cal.show_previous_year();
        assert_eq!(cal.display_month.year(), 2026, "display year back");
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

        // Page down (key 34) advances displayed month (not selection)
        cal.handle_event(&Event::KeyPress { key: 34, modifiers: 0 });
        assert_eq!(cal.display_month.month(), 7, "display advances to July");
        assert_eq!(cal.selected_date().month(), 6, "selection unchanged");

        // Page up (key 33) goes back by month
        cal.handle_event(&Event::KeyPress { key: 33, modifiers: 0 });
        assert_eq!(cal.display_month.month(), 6, "display back to June");
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
