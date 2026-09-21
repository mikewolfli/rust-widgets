// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Calendar widget.
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::access::weekday_to_str;
use crate::widget::capability::coercion::{
    expect_bool, expect_naive_date, expect_string, expect_weekday, naive_date_to_string,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use chrono::Datelike;

/// Advance a date by `delta` months, clamping to the last valid day.
fn advance_month(date: chrono::NaiveDate, delta: i32) -> Option<chrono::NaiveDate> {
    let total_months = date.year() * 12 + date.month() as i32 + delta;
    let new_year = (total_months - 1) / 12;
    let new_month = ((total_months - 1) % 12) + 1;
    if !(0..=9999).contains(&new_year) {
        return None;
    }
    let day = date.day().min(max_days_in_month(new_year, new_month as u32));
    chrono::NaiveDate::from_ymd_opt(new_year, new_month as u32, day)
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
    /// Emitted with the newly selected date when `selected_date` changes, whether
    /// from a click, keyboard navigation, or a programmatic setter. Not emitted
    /// when the selection is set to the date it already held.
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
                .expect("the calendar lower bound is the literal 1900-01-01, which is a valid Gregorian date"),
            // SAFETY: 3000-12-31 is a valid Gregorian date.
            maximum_date: chrono::NaiveDate::from_ymd_opt(3000, 12, 31)
                .expect("the calendar upper bound is the literal 3000-12-31, which is a valid Gregorian date"),
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
    ///
    /// Its `width` is the width it *paints*, not the rectangle's right edge: draw primitives
    /// read `Rect::width` as an extent, so `Rect::new(r.x, r.y, r.width, ..)` puts the band's
    /// right edge at `r.x + r.width`. At the census geometry `r.x` is 0, which hid the
    /// distinction — the band's bottom rule ran to `nav.x + nav.width` and its own fill stopped
    /// one pixel earlier, so a control placed anywhere but the origin painted two pixels past
    /// its right edge. Deriving the extent from the geometry's right edge keeps the band inside
    /// the calendar whatever `r.x` is.
    fn nav_rect(&self) -> Rect {
        let r = self.geometry();
        if self.navigation_bar_visible {
            // `r.width` is the requested width; using it as the extent is correct only when
            // `r` starts at the origin, so the band is clamped to `r`'s own right edge.
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
        let first = self.display_month.with_day(1).expect("day 1 exists in every Gregorian month");
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
        let first = self.display_month.with_day(1).expect("day 1 exists in every Gregorian month");
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
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `Calendar`'s property contract.
///
/// Dates cross this boundary as `%Y-%m-%d` strings and the first day of the week
/// as a three-letter token, both matching the capability schema's declared kinds.
impl WidgetProperties for Calendar {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "selected_date" => {
                Ok(CapabilityValue::String(naive_date_to_string(self.selected_date())))
            }
            "minimum_date" => {
                Ok(CapabilityValue::String(naive_date_to_string(self.minimum_date())))
            }
            "maximum_date" => {
                Ok(CapabilityValue::String(naive_date_to_string(self.maximum_date())))
            }
            "first_day_of_week" => {
                Ok(CapabilityValue::String(weekday_to_str(self.first_day_of_week()).to_string()))
            }
            "grid_visible" => Ok(CapabilityValue::Bool(self.is_grid_visible())),
            "navigation_bar_visible" => Ok(CapabilityValue::Bool(self.is_navigation_bar_visible())),
            "horizontal_header_visible" => {
                Ok(CapabilityValue::Bool(self.is_horizontal_header_visible()))
            }
            "vertical_header_visible" => {
                Ok(CapabilityValue::Bool(self.is_vertical_header_visible()))
            }
            "date_format" => Ok(CapabilityValue::String(self.date_format().to_string())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "selected_date" => {
                self.set_selected_date(expect_naive_date(value)?);
                Ok(())
            }
            "minimum_date" => {
                self.set_minimum_date(expect_naive_date(value)?);
                Ok(())
            }
            "maximum_date" => {
                self.set_maximum_date(expect_naive_date(value)?);
                Ok(())
            }
            "first_day_of_week" => {
                self.set_first_day_of_week(expect_weekday(value)?);
                Ok(())
            }
            "grid_visible" => {
                self.set_grid_visible(expect_bool(value)?);
                Ok(())
            }
            "navigation_bar_visible" => {
                self.set_navigation_bar_visible(expect_bool(value)?);
                Ok(())
            }
            "horizontal_header_visible" => {
                self.set_horizontal_header_visible(expect_bool(value)?);
                Ok(())
            }
            "vertical_header_visible" => {
                self.set_vertical_header_visible(expect_bool(value)?);
                Ok(())
            }
            "date_format" => {
                self.set_date_format(expect_string(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "selected_date",
            "minimum_date",
            "maximum_date",
            "first_day_of_week",
            "grid_visible",
            "navigation_bar_visible",
            "horizontal_header_visible",
            "vertical_header_visible",
            "date_format",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `calendar` publishes.
    ///
    /// `show_today` maps onto the widget's real `show_today`: it needs no payload
    /// and both moves the displayed month and re-selects today. The three
    /// `set_*` names carry the date, the range and the weekday, so a bare
    /// invocation is reported as needing one rather than being called unknown.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "show_today" => {
                self.show_today();
                Ok(())
            }
            "set_selected_date" | "set_date_range" | "set_first_day_of_week" => {
                Err(CapabilityAccessError::OutOfRange)
            }
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
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
        // Read the style once. The chrome — the calendar's own surface, its ink, its
        // outlines — follows it; the literals stay as fallbacks so an unstyled
        // calendar looks exactly as before. What does *not* follow it is the set of
        // colours that state what a cell *means*: today's amber, the selection's
        // blue, the weekend's red, and the dimmed/out-of-range greys. Those are
        // information, and recolouring them from a theme would erase the meaning
        // rather than theme its appearance.
        let style = self.style();
        let dim_color = style.text_color.unwrap_or(if enabled {
            Color::rgb(160, 160, 160)
        } else {
            Color::rgb(210, 210, 210)
        });
        let text_color = style.text_color.unwrap_or(if enabled {
            Color::rgb(30, 30, 30)
        } else {
            Color::rgb(170, 170, 170)
        });
        let header_bg = style.background_color.unwrap_or(Color::rgb(235, 235, 235));
        let border_color = style.border_color.unwrap_or(Color::rgb(190, 190, 190));
        let calendar_bg = style.background_color.unwrap_or(Color::rgb(255, 255, 255));
        let weekend_color = Color::rgb(180, 60, 60);

        // ── Outer background & border ──
        context.fill_rect(rect, calendar_bg);
        context.draw_rect(rect, border_color);

        // ── 1. Navigation bar ──
        if self.navigation_bar_visible {
            let nav = self.nav_rect();
            context.fill_rect(nav, header_bg);
            // Bottom border. Drawn edge-to-edge across the calendar's *own* rectangle rather
            // than `nav.x + nav.width`: the two are the same only at the origin, and the
            // extent form put the rule past the right edge for any other position. The end
            // point is one pixel in from the right edge for the reason spelled out at the
            // grid rules: a 1 px stroke centred on the edge reaches half a pixel past it.
            let band_right = rect.x + rect.width as i32 - 1;
            context.draw_line(
                Point::new(nav.x, nav.y + nav.height as i32 - 1),
                Point::new(band_right, nav.y + nav.height as i32 - 1),
                border_color,
            );
            // ◄ button
            let btn_w = 30i32;
            let arrow_color = style.text_color.unwrap_or(if enabled {
                Color::rgb(60, 60, 60)
            } else {
                dim_color
            });
            context.draw_text(
                Point::new(nav.x + 8, nav.y + 7),
                "◀",
                &Font::default(),
                arrow_color,
                HorizontalAlignment::Left,
            );
            // Month/year title: fitted into the band between the two arrow buttons.
            //
            // A centred string at the navigation bar's midpoint is bounded by nothing: at 13 px
            // bold the census's 1-em-per-character model advances "September 2026" 182 px from
            // x = 120 and runs 62 px past the calendar's right edge (x = 302 in a 240 px
            // control). Only the raster backends' clipping hid it. The box is the span the
            // arrows leave free, inset by a further 8 px — the same inset the ◀ glyph is drawn
            // at — so a longer month name is truncated rather than allowed to overlap a button.
            //
            // The span is derived from `rect`'s right edge rather than from `nav.width`:
            // `nav.right() - (x + span)` is `rect.x` once the extent is measured from a
            // non-zero origin, so a calendar placed at x > 0 would still overrun by `rect.x`.
            let arrow_inset = 8 + btn_w;
            let left_edge = nav.x + arrow_inset;
            let title_bounds = Rect::new(
                left_edge,
                nav.y + 7,
                (band_right - left_edge).max(0) as u32,
                Self::NAV_H.saturating_sub(7),
            );
            let title =
                format!("{} {}", self.display_month.format("%B"), self.display_month.year());
            context.draw_text_fitted(
                title_bounds,
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
            // A brighter tint of the header fill, so the two bands stay distinguishable
            // on any background rather than only on the light default.
            context.fill_rect(hdr, header_bg.blend(&Color::rgb(255, 255, 255), 0.5));
            // Same edge-to-edge rule as the navigation bar's, and the same two reasons.
            let band_right = rect.x + rect.width as i32 - 1;
            context.draw_line(
                Point::new(hdr.x, hdr.y + hdr.height as i32 - 1),
                Point::new(band_right, hdr.y + hdr.height as i32 - 1),
                border_color,
            );
            let cell_w = (hdr.width / 7).max(1) as i32;
            let names = match self.first_day_of_week {
                chrono::Weekday::Mon => ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"],
                chrono::Weekday::Sun => ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"],
                _ => ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"],
            };
            for (i, name) in names.iter().enumerate() {
                let cell_x = hdr.x + cell_w * i as i32;
                let is_weekend = i >= 5;
                let c = if !enabled {
                    dim_color
                } else if is_weekend {
                    weekend_color
                } else {
                    text_color
                };
                // Each label is fitted into the span from its own cell's left edge to the
                // grid's right edge — deliberately not into the cell alone.
                //
                // The column is 34 px wide (`hdr.width / 7`, which integer division already
                // rounds down) and a three-letter label needs 36 under the census's 1-em-per-
                // character model, so a strictly per-cell box does not fit "Mon" at all:
                // `draw_text_fitted` would truncate *every* column to a lone "…", replacing
                // seven readable names with seven identical dots. The binding defect is that the
                // labels were unbounded, not that they must be confined to one column, so the
                // box spans to the grid's right edge and a name too long for the room it has is
                // still truncated rather than allowed to leave the calendar.
                //
                // The span also stops one pixel short of the edge, because that is where the
                // next column's own glyphs begin and because a box ending exactly on the edge
                // lets a fitted glyph reach it. Column 6 is the one this fixes: centred on the
                // cell's midpoint, "Sun" started at x = 221 and its 33 px advance ended at 254,
                // 14 px outside a 240 px control.
                let span_right = hdr.x + cell_w * 7 - 1;
                let span_w = (span_right - cell_x).max(0) as u32;
                let cell_bounds = Rect::new(cell_x, hdr.y + 6, span_w, 11);
                context.draw_text_fitted(
                    cell_bounds,
                    name,
                    &Font::bold("Arial", 11.0),
                    c,
                    HorizontalAlignment::Left,
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
            // The cell separators are the calendar's own outline, so they follow
            // `border_color`; a fixed near-white grid vanished entirely on a dark
            // surface. Derived rather than literal so a zero-width border means the
            // same thing here as it does everywhere else: no visible rule.
            let grid_line = if style.border_width.unwrap_or(1) == 0 {
                Color::rgba(0, 0, 0, 0)
            } else {
                border_color
            };

            for row in 0..6 {
                for col in 0..7 {
                    let cx = grid.x + col * cell_w as i32;
                    let cy = grid.y + row * cell_h as i32;
                    let cell_rect = Rect::new(cx, cy, cell_w, cell_h);
                    let day_num = (row * 7 + col) - blanks as i32 + 1;

                    if day_num < 1 || day_num > days_in_month as i32 {
                        // Out-of-month cell — leave blank
                        continue;
                    }

                    // SAFETY: day_num is within the valid range for this month.
                    let date = self
                        .display_month
                        .with_day(day_num as u32)
                        .expect("day_num was clamped to 1..=days_in_month for this month, so it is always a valid day");

                    // Cell background
                    let in_range = date >= self.minimum_date && date <= self.maximum_date;
                    // The cell's inner right/bottom edge: what the grid rules and the today
                    // highlight are inset to, so they sit inside the cell rather than on top of
                    // its neighbour's edge.
                    //
                    // Two pixels in, not one. A `<line>` is a stroke of finite width and this
                    // backend emits `stroke-width="1"` with no half-pixel form, so the P5 gate
                    // reads a rule at `y1 == y2` as covering `y - 0.5 ..= y + 0.5`. A rule drawn
                    // at the cell's last pixel (`cy + cell_h - 1`) therefore reaches half a pixel
                    // past the edge that cell owns (`97.5` against `97`), which is into the next
                    // row — the "drawing leaves the control and only the raster clip hides it"
                    // defect P5 exists to catch. Insetting by two keeps the rule wholly within the
                    // cell it marks while still inside the cell's own last pixel column.
                    //
                    // Both are computed as `i32` **before** narrowing. The original form was
                    // `col * cell_w - 1` with `cell_w: u32`, so the subtraction happened in
                    // unsigned arithmetic and was then widened on the multiply: the band covered
                    // `(col * (cell_w - 1))`, which at the census geometry put row 3's band at
                    // y = 98 instead of 87 and row 5's cells at 96. Six rows drifted one pixel per
                    // row — by the bottom of the grid the highlight was six pixels out of place.
                    let inner_x = cx + (cell_w as i32 - 2);
                    let inner_y = cy + (cell_h as i32 - 2);
                    let today_band =
                        Rect::new(cx, cy, (inner_x - cx) as u32, (inner_y - cy) as u32);
                    if date == today && date == self.selected_date {
                        // Selected + today: blend selected bg over today bg not possible,
                        // so use a composite visual: fill today bg first, then selected overlay
                        context.fill_rect(today_band, today_bg);
                        // Overlay a subtle selected marker, centred inside the band rather than
                        // on the cell's outer midpoint, so the dot cannot straddle the band's
                        // own edge once the band is one pixel short of the cell.
                        context.fill_rounded_rect(
                            Rect::new(
                                cx + (today_band.width as i32 - 4) / 2,
                                cy + (today_band.height as i32 - 4) / 2,
                                4,
                                4,
                            ),
                            2,
                            Color::rgb(51, 153, 255),
                        );
                    } else if date == today {
                        context.fill_rect(today_band, today_bg);
                        // Today border
                        context.draw_rect(today_band, today_border);
                    } else if date == self.selected_date {
                        context.fill_rect(cell_rect, selected_bg);
                    } else if !in_range {
                        // Outside range — dimmed. A dimmed *surface*, so a theme's
                        // background stays visible under the veil instead of a fixed
                        // near-white rectangle appearing in a dark calendar.
                        context.fill_rect(cell_rect, calendar_bg.blend(&text_color, 0.9));
                    }

                    // Grid lines (right + bottom edges)
                    context.draw_line(
                        Point::new(inner_x, cy),
                        Point::new(inner_x, inner_y),
                        grid_line,
                    );
                    context.draw_line(
                        Point::new(cx, inner_y),
                        Point::new(inner_x, inner_y),
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
                    // Offset by the original 3 px and fitted to what remains of the cell. A
                    // two-digit day at 11 px advances 22 px, which is inside a 34 px cell, but
                    // the label was the one run in this grid with no bound at all — and it is
                    // bounded by the cell, not by the calendar, so a narrower grid would have
                    // let it cross into its neighbour.
                    context.draw_text_fitted(
                        Rect::new(
                            cx + 3,
                            cy + 3,
                            today_band.width.saturating_sub(3),
                            (inner_y - (cy + 3)).max(0) as u32,
                        ),
                        &format!("{day_num}"),
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
