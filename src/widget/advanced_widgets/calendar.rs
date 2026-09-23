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
        // The weekend indicator is *semantic* rather than a data colour: it says "this
        // column is not a working day", which is the same class of meaning as an error or
        // a warning (rule #108 ②). It therefore reads `theme.colors.error` instead of the
        // literal `rgb(180, 60, 60)` — that literal was a *light* calendar's red and
        // rendered at 1.64:1 on the dark appearance's header band, i.e. present in the
        // census and unreadable to a person.
        //
        // The token is nudged *away* from the calendar's own surface rather than toward
        // it. Blending a fixed fraction toward the error colour produced whatever
        // lightness that fraction happened to give: 85 % landed at 2.4:1 on the light
        // surface and 3.5:1 on the dark one. Choosing the direction from the surface's own
        // luminance — darken the red on a light surface, lighten it on a dark one — puts
        // the same token on the legible side of the threshold on either appearance, which
        // is what a semantic colour is supposed to guarantee.
        let weekend_color = crate::theme::semantic_color(crate::theme::SemanticColor::Error)
            .map(|error| {
                if calendar_bg.relative_luminance() > 0.179 {
                    error.blend(&Color::BLACK, 0.25)
                } else {
                    error.blend(&Color::WHITE, 0.25)
                }
            })
            .unwrap_or(Color::rgb(180, 60, 60));

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
            //
            // Every run in this bar is positioned by the shared line box, so the two arrows
            // and the title share one centre line instead of each carrying its own literal
            // `+ 7` offset. The literal was correct only for a 14 px font in a 30 px bar; a
            // 13 px bold title next to it landed a pixel off the arrows' baseline, which is
            // the kind of drift that a per-run constant always eventually produces.
            let arrow_color = style.text_color.unwrap_or(if enabled {
                Color::rgb(60, 60, 60)
            } else {
                dim_color
            });
            let arrow_font = Font::default();
            let arrow_line = context.text_line(nav, &arrow_font);
            // The arrows' own box width, used to reserve the span the title may use.
            let btn_w = 30i32;
            context.draw_text(
                Point::new(nav.x + 8, arrow_line.y),
                "◀",
                &arrow_font,
                arrow_color,
                HorizontalAlignment::Left,
            );
            // Month/year title: fitted into the band between the two arrow buttons.
            //
            // A centred string at the navigation bar's midpoint is bounded by nothing, and a
            // month name is data the user can change, so the box is the span the arrows leave
            // free — inset by the same 8 px the ◀ glyph is drawn at — and a longer name is
            // truncated rather than allowed to overlap a button. The renderer charges 0.6 em per
            // narrow cluster (`estimate_cluster_advance`), so "September 2026" advances about
            // 109 px at 13 px bold and fits the ~160 px span at the census geometry; the box is
            // what makes that a *guarantee* rather than a coincidence of the current month.
            //
            // The span is derived from `rect`'s right edge rather than from `nav.width`:
            // `nav.right() - (x + span)` is `rect.x` once the extent is measured from a
            // non-zero origin, so a calendar placed at x > 0 would still overrun by `rect.x`.
            let arrow_inset = 8 + btn_w;
            let left_edge = nav.x + arrow_inset;
            let title_font = Font::bold("Arial", 13.0);
            let title_line = context.text_line(nav, &title_font);
            let title_bounds = Rect::new(
                left_edge,
                title_line.y,
                (band_right - left_edge).max(0) as u32,
                title_line.height,
            );
            let title =
                format!("{} {}", self.display_month.format("%B"), self.display_month.year());
            context.draw_text_fitted(
                title_bounds,
                &title,
                &title_font,
                text_color,
                HorizontalAlignment::Center,
            );
            // ► button
            context.draw_text(
                Point::new(nav.x + nav.width as i32 - 8 - btn_w, arrow_line.y),
                "▶",
                &arrow_font,
                arrow_color,
                HorizontalAlignment::Left,
            );
        }

        // ── 2. Weekday headers ──
        if self.horizontal_header_visible {
            let hdr = self.day_header_rect();
            // A *subtle* step off the calendar's own surface, in whichever direction the
            // surface already leans. The previous form blended the header fill halfway
            // toward a literal white, which is a light-theme assumption written as an
            // arithmetic step: on the dark appearance `rgb(18,18,18)` became
            // `rgb(137,137,137)` — a heavy mid-grey band, far heavier than any mainstream
            // calendar's header (a Material `onSurfaceVariant` header and SwiftUI's graphical
            // `DatePicker` all keep it within a few
            // percent of the body). Blending a *small* fraction toward the calendar's own
            // ink gives the same restrained step on either surface, which is also what
            // keeps the weekday labels legible on top of it.
            context.fill_rect(hdr, calendar_bg.blend(&text_color, 0.08));
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
                // Each weekday label is **centred in its own column**, so the header sits on
                // the same seven-column grid the day cells do.
                //
                // This used to span from the cell's left edge to the grid's *right* edge and
                // left-align inside that, on the stated premise that "a three-letter label needs
                // 36 under the census's 1-em-per-character model" and so would be truncated to a
                // lone ellipsis in a 34 px column. That premise is not what the renderer does:
                // `estimate_cluster_advance` charges 0.6 em per narrow cluster, so "Mon"
                // advances 20 px, not 36, and fits a 34 px cell with room to spare. The
                // workaround was therefore unnecessary — and it *was* the defect, because
                // left-aligning in a span that reaches the grid's right edge pushes the label
                // into the next column: "Sun" rendered at the far right of a box 33 px wider
                // than its column, so the seven headings visibly disagreed with the seven
                // columns of day numbers below them. Bounding each label to its own cell fixes
                // the reading and keeps the truncation guard where it belongs, on a genuinely
                // narrow column.
                let label_bounds = Rect::new(cell_x, hdr.y + 6, cell_w as u32, 11);
                context.draw_text_fitted(
                    label_bounds,
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
            // "Today" is the calendar's *accent* state, so it is drawn from the theme's
            // accent token rather than from a literal amber. The amber was also a contrast
            // hazard in its own right: a 39 %-opaque yellow over the dark surface is a mid
            // olive, and the near-white day number on top of it measured **1.30:1** — the
            // worst text ratio in the whole snapshot set. The same translucent-accent
            // treatment as the selection keeps "today" recognisable without making the
            // date unreadable, on either appearance.
            let today_bg = crate::theme::semantic_color(crate::theme::SemanticColor::Warning)
                .map(|warning| warning.with_alpha(120))
                .unwrap_or(Color::rgba(255, 200, 50, 100));
            let today_border = crate::theme::semantic_color(crate::theme::SemanticColor::Warning)
                .map(|warning| calendar_bg.blend(&warning, 0.75))
                .unwrap_or(Color::rgb(200, 120, 20));
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

                    // Day number text. A selected or "today" cell sits on a translucent
                    // tint, so a fixed glyph colour is only correct when that tint happens
                    // to land on the other side of the luminance threshold — the selected
                    // cell measured 2.4:1 on the light appearance and the today cell
                    // 1.30:1 on the dark one. Compositing the tint over the calendar's own
                    // surface and then asking that result for its contrast colour gives the
                    // legible option on either appearance, and does it without a second
                    // hardcoded pair to keep in sync.
                    let day_color = if !in_range {
                        dim_color
                    } else if date == self.selected_date {
                        let tint = calendar_bg.blend(&selected_bg, selected_bg.a as f32 / 255.0);
                        tint.contrast_color()
                    } else if date == today {
                        let tint = calendar_bg.blend(&today_bg, today_bg.a as f32 / 255.0);
                        tint.contrast_color()
                    } else {
                        text_color
                    };
                    // The day number is **centred in its cell** on both axes, which is what a
                    // calendar cell is for: the mainstream calendar pickers
                    // all put the number in the
                    // middle of the cell, because the cell's own tint (selection, "today") is
                    // the thing being pointed at and the number labels it. The previous form
                    // left-aligned the run at `cx + 3` inside a `cy + 3` line box, so a one-digit
                    // day sat flush against the cell's left rule with ten pixels of dead space
                    // to its right — the digits and the grid visibly disagreed about which cell
                    // a number belonged to, which is how it was reported.
                    //
                    // The box is the cell, not the today-highlight inset: the highlight is
                    // drawn two pixels short of the cell's own right/bottom rule so it cannot
                    // overhang into its neighbour, but the *number* belongs to the whole cell
                    // and must stay centred when that highlight is absent. Centring inside the
                    // tinted band instead would have shifted every non-today number by a pixel.
                    let day_font = Font::new("Arial", 11.0, false, false);
                    let day_line = context.text_line(cell_rect, &day_font);
                    context.draw_text_fitted(
                        day_line,
                        &format!("{day_num}"),
                        &day_font,
                        day_color,
                        HorizontalAlignment::Center,
                    );
                }
            }
        }
    }
}

// These tests drive the **theme**, which only exists in a build with a device profile
// (see `crate::lib`: `pub mod theme` is gated on `device_profile`). Without this gate the
// `mini` and `embedded` profiles fail to compile their test targets, because the test code
// names a module that those builds compile out — the production code is profile-clean and
// only the fixture was not.
#[cfg(all(test, full_widgets))]
mod tests {
    use super::*;
    use crate::core::{Rect, Size};
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

    /// How far apart two `y` values can be and still be the same visual line.
    ///
    /// Reserved for the host's line spacing: rows on one line differ by less than this, lines
    /// differ by more. A single bitmap row is a fraction of the box, so this bound is loose
    /// enough to absorb integer rounding and tight enough to separate adjacent rows.
    const LINE_TOLERANCE: i32 = 3;

    /// One line's worth of ink bits, keyed by the topmost `y` seen on that line.
    type InkLine = (i32, Vec<(i32, i32, i32, i32)>);

    /// Text runs recovered from a rendered SVG, as `(left, top, right, bottom)` ink boxes.
    ///
    /// # Why the ink box and not the string
    ///
    /// Text leaves the backend as `font8x8` **glyph geometry** — one axis-aligned `<path>`
    /// subpath per set bitmap bit, the same rectangles the software rasteriser fills — so the
    /// document contains a picture of the run, not the run. A test therefore has to locate a
    /// run by *where it is*, which is strictly better than locating it by *what it says*:
    /// the old `texts()` helper matched `body == "1"` and then asserted on the element's `x`,
    /// so it verified the backend's attribute rather than the ink the control produced, and a
    /// mis-placed glyph with a correct attribute would have passed.
    ///
    /// # Why grouping is by quantised `y`, then by horizontal gaps
    ///
    /// A glyph box is stretched across the 8 bitmap rows, so those rows land on 8 distinct `y`
    /// values spanning the box; two runs on the same visual line share every one of them, and
    /// the line below shares none. Grouping subpaths by which *bitmap row band* their `y` falls
    /// into therefore separates lines. Within a band, a horizontal gap wider than one glyph
    /// advance separates runs, because the pen advances between clusters.
    ///
    /// Note that a run's subpaths are *not* deduplicated: when a bitmap column is wider than a
    /// pixel (`11 / 8` for a 11 px box), two columns map to the same pixel and the same
    /// rectangle is emitted twice. That is the rasteriser's own geometry — it fills that pixel
    /// twice — so removing it here would make the test disagree with the drawing.
    fn ink_runs(svg: &str) -> Vec<(i32, i32, i32, i32)> {
        /// Bits further apart than this on `x` belong to different runs.
        const RUN_GAP: i32 = 4;
        let mut bits: Vec<(i32, i32, i32, i32)> = Vec::new();
        for line in svg.lines() {
            let Some(path_at) = line.find("<path ") else { continue };
            let Some(d_at) = line[path_at..].find("d=\"") else { continue };
            let start = path_at + d_at + 3;
            let Some(end) = line[start..].find('"') else { continue };
            for subpath in line[start..start + end].split('M').skip(1) {
                let numbers: Vec<i32> = subpath
                    .split(|c: char| !c.is_ascii_digit() && c != '-')
                    .filter(|part| !part.is_empty())
                    .filter_map(|part| part.parse().ok())
                    .collect();
                if numbers.len() < 4 {
                    continue;
                }
                let (x, y, w, h) = (numbers[0], numbers[1], numbers[2], numbers[3]);
                bits.push((x, y, x + w, y + h));
            }
        }
        if bits.is_empty() {
            return Vec::new();
        }
        // Which `y` values are on the same visual line. A new line starts when the `y` jumps
        // by more than one bitmap row's worth, which is a bound rather than a guess: two runs
        // on one line differ by less than one bitmap row, and two lines differ by at least the
        // smaller part of a box height.
        let mut row_tops: Vec<i32> = Vec::new();
        for bit in &bits {
            if !row_tops.iter().any(|top| (top - bit.1).abs() <= LINE_TOLERANCE) {
                row_tops.push(bit.1);
            }
        }
        // Number each line left to right, so a caller can index them in reading order.
        let mut lines: Vec<InkLine> = row_tops.into_iter().map(|top| (top, Vec::new())).collect();
        for bit in bits {
            if let Some((_, group)) =
                lines.iter_mut().find(|(top, _)| (bit.1 - *top).abs() <= LINE_TOLERANCE)
            {
                group.push(bit);
            }
        }
        let mut runs = Vec::new();
        for (_, mut group) in lines {
            group.sort_by_key(|bit| bit.0);
            let mut run: Option<(i32, i32, i32, i32)> = None;
            let mut previous_right = i32::MIN;
            for bit in group {
                if run.is_none() || bit.0 - previous_right > RUN_GAP {
                    if let Some(finished) = run.take() {
                        runs.push(finished);
                    }
                    run = Some(bit);
                } else if let Some(open) = run.as_mut() {
                    open.2 = open.2.max(bit.2);
                    open.3 = open.3.max(bit.3);
                }
                if let Some(open) = run.as_ref() {
                    previous_right = previous_right.max(open.2);
                }
            }
            if let Some(finished) = run {
                runs.push(finished);
            }
        }
        runs
    }

    /// A day number sits in the **middle of its cell**, on both axes.
    ///
    /// The cell's own tint (selection, "today") is the thing being pointed at and the number
    /// labels it; the mainstream calendar pickers
    /// all centre it. The previous form left-aligned the run at `cx + 3` inside a
    /// `cy + 3` line box, so a one-digit day hugged the cell's left rule and the digits
    /// visibly disagreed with the grid about which cell they labelled.
    ///
    /// # What is asserted, and why the numbers are what they are
    ///
    /// The check is on the **ink**, not on an element's `x`/`y` attributes: a mis-placed glyph
    /// with a correct attribute would pass an attribute comparison, and only the ink is what
    /// the user sees. Two quantities are then checked separately, because they differ for a
    /// reason worth pinning:
    ///
    /// * the glyph **box** centre is asserted *exactly* — the run is centred in its cell and
    ///   `draw_text_fitted` computes that position from the measured advance, so any drift here
    ///   is a layout bug;
    /// * the **ink** centre is asserted within `TEXT_FIT_MARGIN`, not exactly. A glyph bitmap
    ///   is 8 columns stretched over `round(0.6 * font_size)` pixels, so the leftmost lit column
    ///   is not the box's first pixel: at the default 11 px font the box is 7 px wide and a
    ///   digit's ink starts one pixel in, which biases the ink up to 0.5 px right or left of the
    ///   box centre depending on which digit it is. Demanding exactness there would assert a
    ///   property of `font8x8` rather than of the layout.
    #[test]
    fn a_day_number_is_centred_in_its_cell() {
        // A month whose first day makes the arithmetic readable, and a geometry where the
        // cell extents divide exactly (240 / 7 and (120 - 54) / 6 are both whole here).
        let rect = Rect::new(0, 0, 240, 120);
        let mut cal = Calendar::new(rect);
        let grid = cal.grid_rect();
        let cell_w = (grid.width / 7).max(1) as i32;
        let cell_h = (grid.height / 6).max(1) as i32;
        let blanks = cal.leading_blank_count() as i32;
        let svg = crate::widget::svg::render_to_svg(&mut cal);
        let runs = ink_runs(&svg);

        let font = Font::new("Arial", 11.0, false, false);
        let mut probe = crate::render::SvgPaintBackend::new(Size::new(1, 1));
        let measure = RenderContext::new(&mut probe);
        let line_h = measure.measure_text("M", &font).height as i32;

        let mut checked = 0;
        for index in 0..42 {
            let day_num = index - blanks + 1;
            if !(1..=28).contains(&day_num) {
                continue;
            }
            let row = index / 7;
            let col = index % 7;
            let cx = grid.x + col * cell_w;
            let cy = grid.y + row * cell_h;
            let advance = measure.measure_text(&day_num.to_string(), &font).width as i32;
            // Where the run's glyph box must be: centred in the cell on both axes. This is the
            // layout contract, and it is exact.
            let box_x = cx + (cell_w - advance) / 2;
            let box_y = cy + (cell_h - line_h) / 2;
            // The matching ink, located by the box rather than by the string it spells.
            let ink = runs
                .iter()
                .find(|(l, t, r, _)| {
                    let inside_x = (l + r) / 2 >= box_x && (l + r) / 2 < box_x + advance;
                    let inside_y = *t >= box_y && *t < box_y + line_h;
                    inside_x && inside_y
                })
                .copied()
                .unwrap_or_else(|| {
                    panic!(
                        "day {day_num} painted no ink in box at {box_x},{box_y} (cell {col},{row})"
                    )
                });
            // The box's own centre is the cell's centre to within the integer division of the
            // centring term, which is the strongest statement the geometry supports.
            let box_cx = box_x + advance / 2;
            assert!(
                (box_cx - (cx + cell_w / 2)).abs() <= 1,
                "day {day_num} box centre {box_cx} must be the cell centre {}",
                cx + cell_w / 2
            );
            // Vertically the box top is exact, and the ink starts inside the box: neither a
            // digit's first nor its last bitmap row is fully lit, so the ink is inset within
            // the box rather than spanning it. Asserting the box exactly and the inset to
            // within `TEXT_FIT_MARGIN` keeps the test true of the layout without restating
            // which rows `font8x8` leaves blank for which digit.
            assert!(
                ink.1 >= box_y && ink.3 <= box_y + line_h,
                "day {day_num} ink {ink:?} must stay inside its box {box_y}..{}",
                box_y + line_h
            );
            assert!(
                ink.1 - box_y <= crate::render::TEXT_FIT_MARGIN as i32,
                "day {day_num} ink must start at the top of its box {box_y}, started at {}",
                ink.1
            );
            let ink_cx = (ink.0 + ink.2) / 2;
            assert!(
                (ink_cx - (cx + cell_w / 2)).abs() <= crate::render::TEXT_FIT_MARGIN as i32,
                "day {day_num} ink centre {ink_cx} must be within a fit margin of the cell centre {}",
                cx + cell_w / 2
            );
            // And it must stay inside the cell, which is the reading the report was about.
            assert!(
                ink.0 >= cx && ink.2 <= cx + cell_w,
                "day {day_num} ink {ink:?} must stay inside its cell {cx}..{}",
                cx + cell_w
            );
            checked += 1;
        }
        assert!(checked >= 28, "the fixture must have measured a whole month, got {checked}");
    }

    /// A weekday heading sits in the middle of the same column its days do.
    ///
    /// `draw_text_fitted` bounds each label to its own 34 px column (see the header's own
    /// comment): the label used to be given a span reaching the grid's right edge, which pushed
    /// every heading right of its column. The check is therefore that the heading's ink lies
    /// inside its column's bounds **and** is centred in it — the first half is what the old
    /// form got wrong, and it is the half an "is centred" assertion alone cannot see, because a
    /// run centred in the wrong box is still centred.
    ///
    /// The heading is drawn through `draw_text_fitted`, whose usable width is inset at **both**
    /// ends by `TEXT_FIT_MARGIN` — so its centre is the centre of that inset span, which is the
    /// column's centre, and the ink lands within a margin of it rather than exactly on it (a
    /// 11 px glyph box is 7 px wide and `font8x8`'s lit columns start at column 1, biasing the
    /// ink by up to half a pixel). Asserting the margin rather than exactness keeps the test a
    /// statement about the layout and not about the bitmap.
    #[test]
    fn a_weekday_heading_is_centred_in_its_column() {
        let rect = Rect::new(0, 0, 240, 120);
        let mut cal = Calendar::new(rect);
        let hdr = cal.day_header_rect();
        let cell_w = (hdr.width / 7).max(1) as i32;
        let svg = crate::widget::svg::render_to_svg(&mut cal);
        let runs = ink_runs(&svg);
        let label_top = hdr.y + 6;

        for i in 0..7 {
            let cell_x = hdr.x + cell_w * i;
            let found = runs
                .iter()
                .find(|(l, t, r, _)| {
                    let mx = (l + r) / 2;
                    mx >= cell_x && mx < cell_x + cell_w && (*t - label_top).abs() < 12
                })
                .copied()
                .unwrap_or_else(|| {
                    panic!("no heading ink in column {i} (cell x {cell_x}..{})", cell_x + cell_w)
                });
            assert!(
                found.0 >= cell_x && found.2 <= cell_x + cell_w,
                "column {i}: heading ink {found:?} must stay inside {cell_x}..{}",
                cell_x + cell_w
            );
            let ink_cx = (found.0 + found.2) / 2;
            assert!(
                (ink_cx - (cell_x + cell_w / 2)).abs() <= crate::render::TEXT_FIT_MARGIN as i32,
                "column {i}: heading ink centre {ink_cx} must be within a fit margin of the column centre {}",
                cell_x + cell_w / 2
            );
            assert_eq!(found.1, label_top, "column {i}: the heading starts on its line box top");
        }
    }
}
