// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Calendar widget.
use core::fmt::Write as _;

use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::access::weekday_to_str;
use crate::widget::capability::coercion::{
    expect_bool, expect_naive_date, expect_string, expect_string_list, expect_weekday,
    naive_date_to_string,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, effective_font, role_font};
use crate::widget::{BaseWidget, Draw, IconName, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use chrono::Datelike;

/// The English month names, indexed `month - 1`.
///
/// One table, read through [`Calendar::month_name`]. The draw path used to ask `chrono` for
/// `%B` inside the title and once per open-menu row, which allocated a `String` per label per
/// frame; this is the same text, borrowed.
const ENGLISH_MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// The English weekday names, indexed by `Weekday::num_days_from_monday()` (Monday first).
///
/// All seven are present. The header used to hardcode a three-letter list and a lookup keyed on
/// only Mon/Sun, so any other `first_day_of_week` fell through to `Mon` for every column — six of
/// the seven possible headers were wrong.
const ENGLISH_WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

/// The default `date_format` pattern, rendered through the calendar's names layer.
const DEFAULT_DATE_FORMAT: &str = "%B %Y";

/// The navigation title's cached text and the inputs it was built from.
#[derive(Default)]
struct MonthTitleCache {
    /// The `date_format` pattern the text was rendered with.
    format: String,
    /// The year the text names.
    year: i32,
    /// The month the text names, `1..=12`.
    month: u32,
    /// The rendered title.
    text: String,
}

/// The year menu's cached row labels and the window they cover.
#[derive(Default)]
struct YearMenuCache {
    /// The first year of the window the labels were built for; `None` before the first build.
    start: Option<i32>,
    /// The five row labels.
    labels: [String; 5],
}

/// The day grid's cached text line boxes and the geometry they were built for.
#[derive(Default)]
struct DayLineCache {
    /// The inputs the boxes were built from; `None` before the first build.
    key: Option<DayLineKey>,
    /// One line box per cell, indexed `row * 7 + col`.
    lines: Vec<Rect>,
}

/// Everything a day cell's text line box depends on.
///
/// The font is inside the key because the box's height is the font's line height, and its width
/// is the cell's own — a theme switch that changes the role font must rebuild the table.
#[derive(Clone, Copy, PartialEq, Eq)]
struct DayLineKey {
    grid_x: i32,
    grid_y: i32,
    cell_w: u32,
    cell_h: u32,
    font_size_bits: u32,
    font_weight: u16,
    font_family_hash: u64,
}

impl DayLineKey {
    /// Folds the inputs into one comparable value.
    fn of(grid: Rect, cell_w: u32, cell_h: u32, font: &crate::core::Font) -> Self {
        // FNV-1a over the family name: the table is rebuilt only on a theme/class change, so a
        // per-frame hash of a short string is cheaper than storing an owned copy to compare.
        let mut font_family_hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in font.family().as_bytes() {
            font_family_hash ^= u64::from(*byte);
            font_family_hash = font_family_hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        Self {
            grid_x: grid.x,
            grid_y: grid.y,
            cell_w,
            cell_h,
            font_size_bits: font.size().to_bits(),
            font_weight: font.weight(),
            font_family_hash,
        }
    }
}

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

/// Every date from `a` to `b` inclusive, in ascending order.
///
/// A range selection is materialised as dates rather than kept as a pair so the paint path has
/// one membership test for every mode (see [`Calendar::is_date_selected`]).
fn range_between(a: chrono::NaiveDate, b: chrono::NaiveDate) -> Vec<chrono::NaiveDate> {
    let (start, end) = if a <= b { (a, b) } else { (b, a) };
    let mut dates = Vec::new();
    let mut cursor = start;
    while cursor <= end {
        dates.push(cursor);
        match cursor.succ_opt() {
            Some(next) => cursor = next,
            None => break,
        }
    }
    dates
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

/// How a [`Calendar`] interprets a user selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalendarSelectionMode {
    /// One date at a time (the historical behaviour).
    Single,
    /// Clicking a day toggles it in the selected set.
    Multi,
    /// The first click anchors the range and the second completes it.
    Range,
}

impl CalendarSelectionMode {
    /// The token this mode publishes through the property contract.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Single => "single",
            Self::Multi => "multi",
            Self::Range => "range",
        }
    }

    /// Parses [`Self::as_str`].
    pub fn from_token(token: &str) -> Option<Self> {
        match crate::widget::capability::coercion::normalize_key(token).as_str() {
            "single" => Some(Self::Single),
            "multi" | "multiple" => Some(Self::Multi),
            "range" => Some(Self::Range),
            _ => None,
        }
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
    /// Every selected date, ascending. Always contains `selected_date` while a selection exists.
    selected_dates: Vec<chrono::NaiveDate>,
    /// How a click is interpreted; see [`CalendarSelectionMode`].
    selection_mode: CalendarSelectionMode,
    /// The first click of an in-progress range selection.
    selection_anchor: Option<chrono::NaiveDate>,
    /// The "displayed" month — the month currently shown in the grid
    /// (may differ from `selected_date` after month navigation).
    display_month: chrono::NaiveDate,
    /// The date the control paints as **today** (the accent-tinted cell).
    ///
    /// # Why this is a field rather than a `Local::now()` call in `draw`
    ///
    /// `draw` read the wall clock directly, so the picture was a function of the day it was
    /// rendered. A committed snapshot therefore changed at every midnight with no edit to explain
    /// it — the same defect `selected_date` had, and the reason `sample_fill` pins that one. It
    /// also made the control untestable for anything that depends on the today cell, because there
    /// was no way to say which day "today" was. Seeding it from the clock at construction keeps the
    /// host-visible behaviour (a calendar opens on today) while letting a caller — or a snapshot
    /// fixture — state the day explicitly.
    today: chrono::NaiveDate,
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
    /// Emitted when the user **commits** the selected date: a double click on a day
    /// cell, or Enter/Space with the grid focused.
    ///
    /// 参考工具包 draws the same line between selection and activation: navigating a calendar
    /// changes `selectedDate` continuously, while `activated` is the "use this date and
    /// close me" event a date picker's host listens to. A host that closed its popup on
    /// `selection_changed` would shut it after a single arrow-key move.
    pub activated: Signal1<chrono::NaiveDate>,
    /// Emitted when the displayed month changes, as `(year, month)`.
    ///
    /// Covers every way the page can move — the navigation bar, Page Up/Down, arrow keys
    /// crossing a month boundary, and the programmatic setters — because they all go through
    /// `Calendar::show_page`. the reference toolkit's `currentPageChanged` answers the same question.
    pub current_page_changed: Signal1<(i32, u32)>,
    date_format: String,
    /// The day cell under the pointer, painted with a subtle highlight.
    hovered_date: Option<chrono::NaiveDate>,
    /// Per-date ink override, applied to individual day numbers.
    ///
    /// A function pointer rather than a boxed closure so the control stays `Send` and
    /// cheap to copy into a draw pass; a holiday or weekend palette is a lookup, not a
    /// captured environment.
    date_text_color: Option<fn(chrono::NaiveDate) -> Option<Color>>,
    /// Dates the user cannot select by pointer or keyboard.
    disabled_dates: Vec<chrono::NaiveDate>,
    /// Whether the month/year title segments open navigation menus.
    dropdown_navigation: bool,
    /// Whether the month menu is open.
    month_menu_open: bool,
    /// Whether the year menu is open.
    year_menu_open: bool,
    /// Whether the navigation bar paints a Today button.
    today_button_visible: bool,
    /// Whether a page change animates its grid.
    page_animation: bool,
    /// The grid's page-change reveal.
    page_reveal: crate::widget::menu_toolbar::popup_reveal::PopupReveal,
    /// Full month names, indexed `month - 1`; see [`Calendar::month_name`].
    month_names: [String; 12],
    /// Weekday names, indexed by `Weekday::num_days_from_monday()`; see [`Calendar::weekday_name`].
    weekday_names: [String; 7],
    /// The navigation title's rendered text, rebuilt only when its inputs change (K-P01).
    title_cache: crate::compat::RefCell<MonthTitleCache>,
    /// The year menu's row labels, rebuilt only when its window changes (K-P01).
    year_menu_cache: crate::compat::RefCell<YearMenuCache>,
    /// The day grid's text line boxes, rebuilt only when the grid geometry or font changes
    /// (K-P01). A `RefCell` because `draw` resolves `style` once, which holds an immutable
    /// borrow of `self` for the rest of the pass — the same reason `ToolBar` caches its item
    /// bands this way.
    day_line_cache: crate::compat::RefCell<DayLineCache>,
}
impl Calendar {
    /// Creates a calendar widget.
    pub fn new(geometry: Rect) -> Self {
        let today = chrono::Local::now().date_naive();
        Self {
            base: BaseWidget::new(WidgetKind::Calendar, geometry, "Calendar"),
            selected_date: today,
            selected_dates: vec![today],
            selection_mode: CalendarSelectionMode::Single,
            selection_anchor: None,
            display_month: today,
            today,
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
            activated: Signal1::new(),
            current_page_changed: Signal1::new(),
            date_format: DEFAULT_DATE_FORMAT.to_string(),
            hovered_date: None,
            date_text_color: None,
            disabled_dates: Vec::new(),
            dropdown_navigation: false,
            month_menu_open: false,
            year_menu_open: false,
            today_button_visible: false,
            page_animation: false,
            page_reveal: crate::widget::menu_toolbar::popup_reveal::PopupReveal::new(true),
            month_names: core::array::from_fn(|index| ENGLISH_MONTHS[index].to_string()),
            weekday_names: core::array::from_fn(|index| ENGLISH_WEEKDAYS[index].to_string()),
            title_cache: crate::compat::RefCell::new(MonthTitleCache::default()),
            year_menu_cache: crate::compat::RefCell::new(YearMenuCache::default()),
            day_line_cache: crate::compat::RefCell::new(DayLineCache::default()),
        }
    }
    /// Returns the date this calendar paints as "today".
    pub fn today(&self) -> chrono::NaiveDate {
        self.today
    }

    /// Sets the date painted as "today", and repaints.
    ///
    /// The control's default is the wall clock's day at construction; this exists so a caller (or a
    /// snapshot fixture) can state it. See [`Calendar::today`]'s field docs for why the day is a
    /// field rather than a clock read in `draw`.
    pub fn set_today(&mut self, date: chrono::NaiveDate) {
        if self.today == date {
            return;
        }
        self.today = date;
        self.base.request_redraw();
    }

    /// The full name of `month` (`1..=12`).
    ///
    /// This and [`Calendar::weekday_name`] are the calendar's **date-name layer**: the title, the
    /// month menu and the weekday header all read their text from here, so a caller that supplies
    /// localised names sees them in every place at once, and the three sites cannot drift apart.
    /// The default is English with all twelve months and all seven weekdays present.
    pub fn month_name(&self, month: u32) -> &str {
        let index = month.clamp(1, 12) as usize - 1;
        &self.month_names[index]
    }

    /// The name of `weekday` from this calendar's names layer.
    ///
    /// Indexed by the weekday itself (`Weekday::num_days_from_monday`), not by its position in
    /// the visible grid, so a `first_day_of_week` change cannot shift the names.
    pub fn weekday_name(&self, weekday: chrono::Weekday) -> &str {
        &self.weekday_names[weekday.num_days_from_monday() as usize]
    }

    /// The twelve month names this calendar paints, January first.
    pub fn month_names(&self) -> &[String; 12] {
        &self.month_names
    }

    /// Replaces the month names and repaints.
    ///
    /// A localisation hook, not a formatting one: the *order* is fixed (January first), so a
    /// caller supplies the same twelve slots in its own language.
    pub fn set_month_names(&mut self, names: [String; 12]) {
        if self.month_names == names {
            return;
        }
        self.month_names = names;
        self.title_cache.get_mut().month = 0;
        self.base.request_redraw();
    }

    /// The weekday names this calendar paints, indexed by `num_days_from_monday`.
    pub fn weekday_names(&self) -> &[String; 7] {
        &self.weekday_names
    }

    /// The seven weekdays in the order the header paints them.
    ///
    /// The header's columns are derived from `first_day_of_week` here, once, so the label column
    /// and the weekend tint column cannot disagree about which weekday a column belongs to.
    fn weekday_name_order(&self) -> [chrono::Weekday; 7] {
        let first = self.first_day_of_week.num_days_from_monday();
        core::array::from_fn(|index| {
            let day = ((first + index as u32) % 7) as u8;
            chrono::Weekday::try_from(day).unwrap_or(self.first_day_of_week)
        })
    }

    /// Replaces the weekday names and repaints.
    ///
    /// Indexed by `num_days_from_monday`, the same index the header uses, so Monday's slot is
    /// always slot 0 regardless of which day the header starts on.
    pub fn set_weekday_names(&mut self, names: [String; 7]) {
        if self.weekday_names == names {
            return;
        }
        self.weekday_names = names;
        self.base.request_redraw();
    }

    /// Returns selected date.
    pub fn selected_date(&self) -> chrono::NaiveDate {
        self.selected_date
    }

    /// Returns every selected date, ascending.
    pub fn selected_dates(&self) -> &[chrono::NaiveDate] {
        &self.selected_dates
    }

    /// How user clicks are interpreted.
    pub fn selection_mode(&self) -> CalendarSelectionMode {
        self.selection_mode
    }

    /// Changes the selection mode.
    ///
    /// Switching to single keeps only the primary selection; multi and range keep the current
    /// dates so a caller can switch modes without losing what the user chose.
    pub fn set_selection_mode(&mut self, mode: CalendarSelectionMode) {
        if self.selection_mode == mode {
            return;
        }
        self.selection_mode = mode;
        self.selection_anchor = None;
        if mode == CalendarSelectionMode::Single {
            self.selected_dates.clear();
            self.selected_dates.push(self.selected_date);
        } else if self.selected_dates.is_empty() {
            self.selected_dates.push(self.selected_date);
        }
        self.selected_dates.sort_unstable();
        self.selected_dates.dedup();
        self.base.request_redraw();
    }

    /// Clears every selected date.
    ///
    /// `selected_date` keeps its value because the field is not optional; it simply stops being
    /// painted as selected. A later click starts a fresh selection.
    pub fn clear_selection(&mut self) {
        if self.selected_dates.is_empty() && self.selection_anchor.is_none() {
            return;
        }
        self.selected_dates.clear();
        self.selection_anchor = None;
        self.base.request_redraw();
    }

    /// Whether `date` is in the selected set.
    pub fn is_date_selected(&self, date: chrono::NaiveDate) -> bool {
        self.selected_dates.binary_search(&date).is_ok()
    }

    /// Returns the currently displayed month (the month shown in the grid).
    pub fn display_month(&self) -> chrono::NaiveDate {
        self.display_month
    }

    /// Sets the month shown in the grid **without** changing the selected date.
    ///
    /// This is the counterpart to [`Calendar::show_next_month`] and friends for a
    /// caller that wants to point the grid at an arbitrary month. The selected
    /// date is left untouched (and is therefore not emitted on
    /// `selection_changed`).
    pub fn set_display_month(&mut self, date: chrono::NaiveDate) {
        if self.display_month == date {
            return;
        }
        self.show_page(date);
    }
    /// Moves the displayed page to `date`'s month, emitting `current_page_changed` when the
    /// `(year, month)` pair actually changed.
    ///
    /// # One place for the page transition
    ///
    /// Every navigation path used to assign `display_month` directly, which made a page change
    /// invisible to a host: there was no way to know that the grid had moved to another month.
    /// Routing them through this helper is what makes `current_page_changed` complete rather
    /// than a signal that only fires when someone remembers to emit it.
    fn show_page(&mut self, date: chrono::NaiveDate) {
        let before = (self.display_month.year(), self.display_month.month());
        let menus_open = self.month_menu_open || self.year_menu_open;
        self.display_month = date;
        let after = (date.year(), date.month());
        self.month_menu_open = false;
        self.year_menu_open = false;
        // A selection that stays on the same page damages only the cells whose wash changed
        // (ADV-001); the caller adds those. A page change, or a menu that has to close, still
        // needs the whole control.
        if before != after || menus_open {
            self.base.request_redraw();
        }
        if before != after {
            if self.page_animation {
                // Start a fresh reveal for the new page. The driver settles at 1, so a
                // later page change restarts from the closed end.
                self.page_reveal.jump_to(false);
                self.page_reveal.set_open(true);
            }
            self.current_page_changed.emit(after);
        }
    }
    /// Sets selected date and syncs the displayed month to match.
    ///
    /// In `Single` mode this replaces the selection. In `Multi` mode the date is added to the
    /// selected set (a programmatic setter never removes a date). In `Range` mode a first call
    /// anchors a range and a second call completes it.
    pub fn set_selected_date(&mut self, date: chrono::NaiveDate) {
        if date < self.minimum_date || date > self.maximum_date {
            return;
        }
        if self.selected_date == date && self.is_date_selected(date) {
            return;
        }
        self.selected_date = date;
        let previous = self.selected_dates.clone();
        match self.selection_mode {
            CalendarSelectionMode::Single => {
                self.selected_dates.clear();
                self.selected_dates.push(date);
            }
            CalendarSelectionMode::Multi => {
                if let Err(index) = self.selected_dates.binary_search(&date) {
                    self.selected_dates.insert(index, date);
                }
            }
            CalendarSelectionMode::Range => match self.selection_anchor.take() {
                Some(anchor) if anchor != date => {
                    self.selected_dates = range_between(anchor, date);
                }
                _ => {
                    self.selected_dates.clear();
                    self.selected_dates.push(date);
                    self.selection_anchor = Some(date);
                }
            },
        }
        self.show_page(date);
        self.selection_changed.emit(date);
        self.damage_dates(previous.iter().chain(self.selected_dates.iter()).copied());
    }

    /// Damages the cells showing `dates`, falling back to the whole control when one is off the
    /// displayed page.
    fn damage_dates(&self, dates: impl IntoIterator<Item = chrono::NaiveDate>) {
        let mut off_page = false;
        for date in dates {
            match self.date_cell_rect(date) {
                Some(rect) => self.base.request_redraw_in(rect),
                None => off_page = true,
            }
        }
        if off_page {
            self.base.request_redraw();
        }
    }
    /// Returns minimum date.
    pub fn minimum_date(&self) -> chrono::NaiveDate {
        self.minimum_date
    }
    /// Sets minimum date.
    ///
    /// A same-value write is a no-op: it cannot clamp anything (the selected date was already
    /// clamped against this value or a larger one) and it cannot change a pixel, so the repaint it
    /// used to request is pure waste in a host that syncs state in a loop.
    pub fn set_minimum_date(&mut self, date: chrono::NaiveDate) {
        if self.minimum_date == date {
            return;
        }
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
    ///
    /// See [`Calendar::set_minimum_date`] for why a same-value write returns early.
    pub fn set_maximum_date(&mut self, date: chrono::NaiveDate) {
        if self.maximum_date == date {
            return;
        }
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
    ///
    /// Returns early on a same-value write, like every setter in this file: the column order cannot
    /// change, so there is nothing to repaint.
    pub fn set_first_day_of_week(&mut self, weekday: chrono::Weekday) {
        if self.first_day_of_week == weekday {
            return;
        }
        self.first_day_of_week = weekday;
        self.base.request_redraw();
    }
    /// Returns whether grid is visible.
    pub fn is_grid_visible(&self) -> bool {
        self.grid_visible
    }
    /// Sets grid visibility.
    pub fn set_grid_visible(&mut self, visible: bool) {
        if self.grid_visible == visible {
            return;
        }
        self.grid_visible = visible;
        self.base.request_redraw();
    }
    /// Whether the week-number gutter is shown.
    ///
    /// This is the name the horizontal-calendar contract uses for the vertical header's gutter;
    /// the two accessors address the same strip.
    pub fn show_week_numbers(&self) -> bool {
        self.vertical_header_visible
    }

    /// Shows or hides the week-number gutter.
    pub fn set_show_week_numbers(&mut self, show: bool) {
        self.set_vertical_header_visible(show);
    }

    /// Returns whether navigation bar is visible.
    pub fn is_navigation_bar_visible(&self) -> bool {
        self.navigation_bar_visible
    }
    /// Sets navigation bar visibility.
    pub fn set_navigation_bar_visible(&mut self, visible: bool) {
        if self.navigation_bar_visible == visible {
            return;
        }
        self.navigation_bar_visible = visible;
        self.base.request_redraw();
    }
    /// Returns whether horizontal header is visible.
    pub fn is_horizontal_header_visible(&self) -> bool {
        self.horizontal_header_visible
    }
    /// Sets horizontal header visibility.
    pub fn set_horizontal_header_visible(&mut self, visible: bool) {
        if self.horizontal_header_visible == visible {
            return;
        }
        self.horizontal_header_visible = visible;
        self.base.request_redraw();
    }
    /// Returns whether vertical header is visible.
    pub fn is_vertical_header_visible(&self) -> bool {
        self.vertical_header_visible
    }
    /// Sets vertical header visibility.
    pub fn set_vertical_header_visible(&mut self, visible: bool) {
        if self.vertical_header_visible == visible {
            return;
        }
        self.vertical_header_visible = visible;
        self.base.request_redraw();
    }
    /// Shows today's date and resets display to current month.
    ///
    /// Also re-reads the clock into the control's `today` field, which is what makes this the
    /// "go back to the present" action rather than only a selection change: after a caller had
    /// called [`Calendar::set_today`], the accent cell would otherwise keep pointing at the pinned
    /// day.
    pub fn show_today(&mut self) {
        let today = chrono::Local::now().date_naive();
        self.today = today;
        self.show_page(today);
        self.set_selected_date(today);
    }
    /// Shows next month in the grid (does not change selected date).
    pub fn show_next_month(&mut self) {
        if let Some(next) = advance_month(self.display_month, 1) {
            self.show_page(next);
        }
    }
    /// Shows previous month in the grid (does not change selected date).
    pub fn show_previous_month(&mut self) {
        if let Some(prev) = advance_month(self.display_month, -1) {
            self.show_page(prev);
        }
    }
    /// Shows next year in the grid (does not change selected date).
    pub fn show_next_year(&mut self) {
        if let Some(next) = self.display_month.with_year(self.display_month.year() + 1) {
            self.show_page(next);
        }
    }
    /// Shows previous year in the grid (does not change selected date).
    pub fn show_previous_year(&mut self) {
        if let Some(prev) = self.display_month.with_year(self.display_month.year() - 1) {
            self.show_page(prev);
        }
    }
    /// Returns the current date format string.
    ///
    /// The pattern uses `chrono`'s strftime specifiers and is applied to the
    /// navigation-bar title. A pattern `chrono` cannot resolve falls back to the
    /// `"<Month> <Year>"` rendering rather than panicking the paint pass.
    pub fn date_format(&self) -> &str {
        &self.date_format
    }

    /// Sets the date format string (uses chrono format specifiers).
    ///
    /// A same-value write returns early: the pattern is what the navigation-bar title is rendered
    /// from, so an unchanged one cannot change the ink.
    pub fn set_date_format(&mut self, format: String) {
        if self.date_format == format {
            return;
        }
        self.date_format = format;
        self.base.request_redraw();
    }

    /// The ink override applied to individual day numbers, if one was installed.
    pub fn date_text_color(&self) -> Option<fn(chrono::NaiveDate) -> Option<Color>> {
        self.date_text_color
    }

    /// Installs a per-date ink override, or clears it with `None`.
    ///
    /// The callback runs once per painted day, so a holiday or a special date can be
    /// tinted without forking the control. Returning `None` keeps the calendar's own
    /// colour for that day. The default is `None`, so an unconfigured calendar paints
    /// exactly as before.
    pub fn set_date_text_color(
        &mut self,
        color_for: Option<fn(chrono::NaiveDate) -> Option<Color>>,
    ) {
        // No equality guard: function pointers from different codegen units are not
        // reliably comparable, and a redundant repaint is cheaper than a guard that can
        // silently skip a real change.
        self.date_text_color = color_for;
        self.base.request_redraw();
    }

    /// The ink for `date` after the override is applied, or `None` to keep the default.
    pub fn date_text_color_for(&self, date: chrono::NaiveDate) -> Option<Color> {
        self.date_text_color.and_then(|color_for| color_for(date))
    }

    /// The dates the user cannot pick.
    pub fn disabled_dates(&self) -> &[chrono::NaiveDate] {
        &self.disabled_dates
    }

    /// Replaces the user-unselectable dates and requests a repaint.
    ///
    /// The list is kept sorted and deduplicated so the paint path can binary-search it.
    /// A programmatic [`Calendar::set_selected_date`] still accepts these dates — the
    /// host may own the current value — but a pointer or keyboard selection refuses them.
    pub fn set_disabled_dates(&mut self, dates: Vec<chrono::NaiveDate>) {
        let mut sorted = dates;
        sorted.sort_unstable();
        sorted.dedup();
        if self.disabled_dates == sorted {
            return;
        }
        self.disabled_dates = sorted;
        self.base.request_redraw();
    }

    /// Whether `date` is in the user-unselectable set.
    pub fn is_date_disabled(&self, date: chrono::NaiveDate) -> bool {
        self.disabled_dates.binary_search(&date).is_ok()
    }

    /// Selects `date` on behalf of the user, refusing a disabled day.
    fn select_from_user(&mut self, date: chrono::NaiveDate) {
        if self.is_date_disabled(date) {
            return;
        }
        match self.selection_mode {
            CalendarSelectionMode::Single => self.set_selected_date(date),
            CalendarSelectionMode::Multi => {
                if let Ok(index) = self.selected_dates.binary_search(&date) {
                    self.selected_dates.remove(index);
                    let mut affected = vec![date];
                    if self.selected_date == date {
                        if let Some(first) = self.selected_dates.first().copied() {
                            self.selected_date = first;
                            affected.push(first);
                        }
                    }
                    self.selection_changed.emit(date);
                    self.damage_dates(affected);
                } else {
                    self.set_selected_date(date);
                }
            }
            CalendarSelectionMode::Range => match self.selection_anchor {
                Some(anchor) => {
                    self.selected_dates = range_between(anchor, date);
                    self.selected_date = date;
                    self.selection_anchor = None;
                    self.show_page(date);
                    self.selection_changed.emit(date);
                    self.base.request_redraw();
                }
                None => {
                    self.selection_anchor = Some(date);
                    self.selected_date = date;
                    self.selected_dates.clear();
                    self.selected_dates.push(date);
                    self.show_page(date);
                    self.selection_changed.emit(date);
                    self.base.request_redraw();
                }
            },
        }
    }

    /// Whether the month/year title opens navigation menus.
    pub fn is_dropdown_navigation_enabled(&self) -> bool {
        self.dropdown_navigation
    }

    /// Enables the month/year navigation menus. Off by default, so the navigation bar
    /// keeps its single centred title.
    pub fn set_dropdown_navigation(&mut self, enabled: bool) {
        if self.dropdown_navigation == enabled {
            return;
        }
        self.dropdown_navigation = enabled;
        if !enabled {
            self.month_menu_open = false;
            self.year_menu_open = false;
        }
        self.base.request_redraw();
    }

    /// Whether the month menu is open.
    pub fn is_month_menu_open(&self) -> bool {
        self.month_menu_open
    }

    /// Whether the year menu is open.
    pub fn is_year_menu_open(&self) -> bool {
        self.year_menu_open
    }

    /// Whether the navigation bar paints a Today button.
    pub fn is_today_button_visible(&self) -> bool {
        self.today_button_visible
    }

    /// Shows or hides the Today button. Off by default.
    pub fn set_today_button_visible(&mut self, visible: bool) {
        if self.today_button_visible == visible {
            return;
        }
        self.today_button_visible = visible;
        self.base.request_redraw();
    }

    /// Whether a page change animates the grid.
    pub fn is_page_animation_enabled(&self) -> bool {
        self.page_animation
    }

    /// Enables the page-change reveal. Off by default, so a page change stays a hard
    /// cut as it always was.
    pub fn set_page_animation(&mut self, enabled: bool) {
        if self.page_animation == enabled {
            return;
        }
        self.page_animation = enabled;
        if !enabled {
            self.page_reveal.jump_to(true);
        }
        self.base.request_redraw();
    }

    /// The navigation bar's inner span: between the arrows, minus the Today button.
    fn nav_content_span(&self) -> Rect {
        let nav = self.nav_rect();
        let arrow_inset = 8 + 30;
        let today = self.today_button_rect().map(|_| 52).unwrap_or(0);
        let left = nav.x + arrow_inset;
        let right = nav.x + nav.width as i32 - arrow_inset - today;
        Rect::new(left, nav.y, (right - left).max(0) as u32, nav.height)
    }

    /// The Today button's box, or `None` when it is hidden.
    pub fn today_button_rect(&self) -> Option<Rect> {
        if !self.today_button_visible || !self.navigation_bar_visible {
            return None;
        }
        let nav = self.nav_rect();
        Some(Rect::new(nav.x + nav.width as i32 - 38 - 44, nav.y, 44, nav.height))
    }

    /// The month segment's box in the navigation bar.
    pub fn month_title_rect(&self) -> Rect {
        let span = self.nav_content_span();
        if self.dropdown_navigation {
            Rect::new(span.x, span.y, span.width / 2, span.height)
        } else {
            span
        }
    }

    /// The year segment's box in the navigation bar, when dropdown navigation is on.
    pub fn year_title_rect(&self) -> Option<Rect> {
        if !self.dropdown_navigation {
            return None;
        }
        let span = self.nav_content_span();
        let half = span.width / 2;
        Some(Rect::new(span.x + half as i32, span.y, span.width - half, span.height))
    }

    /// The open month menu's panel.
    pub fn month_menu_rect(&self) -> Option<Rect> {
        if !self.month_menu_open {
            return None;
        }
        let anchor = self.month_title_rect();
        Some(Rect::new(anchor.x, anchor.y + anchor.height as i32, anchor.width.max(90), 12 * 20))
    }

    /// The open year menu's panel.
    pub fn year_menu_rect(&self) -> Option<Rect> {
        if !self.year_menu_open {
            return None;
        }
        let anchor = self.year_title_rect()?;
        Some(Rect::new(anchor.x, anchor.y + anchor.height as i32, anchor.width.max(70), 5 * 20))
    }

    /// The year at the top of the year menu.
    fn year_menu_start(&self) -> i32 {
        use chrono::Datelike;
        self.display_month.year() - 2
    }

    /// The date the month menu's row `index` would show.
    pub fn month_menu_option(&self, index: u32) -> Option<chrono::NaiveDate> {
        if index == 0 || index > 12 {
            return None;
        }
        use chrono::Datelike;
        chrono::NaiveDate::from_ymd_opt(self.display_month.year(), index, 1)
    }

    /// The year the year menu's row `index` would show.
    pub fn year_menu_option(&self, index: u32) -> Option<i32> {
        if index >= 5 {
            return None;
        }
        Some(self.year_menu_start() + index as i32)
    }

    /// The rendered navigation title, borrowed from the frame cache (K-P01).
    ///
    /// The title used to be re-formatted every frame — `format("%B")`, `format("%Y")` or the
    /// caller's own `date_format` pattern. The pattern is the only input besides the displayed
    /// month, so the cache is keyed on exactly those three facts; a page change, a `date_format`
    /// write or a names-layer swap rebuilds it and nothing else does.
    fn month_title_text(&self) -> core::cell::RefMut<'_, String> {
        let mut cache = self.title_cache.borrow_mut();
        let month = self.display_month.month();
        if cache.month != month
            || cache.year != self.display_month.year()
            || cache.format != self.date_format
        {
            let mut text = String::new();
            self.write_month_title(&mut text);
            cache.text = text;
            cache.format.clear();
            cache.format.push_str(&self.date_format);
            cache.month = month;
            cache.year = self.display_month.year();
        }
        core::cell::RefMut::map(cache, |cache| &mut cache.text)
    }

    /// The year menu's row label for `index`, borrowed from the window cache (K-P01).
    fn year_menu_label(&self, index: usize) -> core::cell::RefMut<'_, str> {
        let start = self.year_menu_start();
        let mut cache = self.year_menu_cache.borrow_mut();
        if cache.start != Some(start) {
            for (slot, label) in cache.labels.iter_mut().enumerate() {
                label.clear();
                let _ = write!(label, "{}", start + slot as i32);
            }
            cache.start = Some(start);
        }
        core::cell::RefMut::map(cache, |cache| cache.labels[index].as_mut_str())
    }

    /// The text line box for the day cell at `(row, col)`, from the geometry cache (K-P01).
    ///
    /// The 42 boxes are a pure function of the grid's origin, the cell size and the day-number
    /// font, so they are built once per geometry and reused. `context.text_line` is not free: it
    /// measures the font's line height through the backend, and the loop asked it 42 times a
    /// frame for answers that could not differ.
    fn day_line_rect(
        &self,
        row: usize,
        col: usize,
        grid: Rect,
        cell_w: u32,
        cell_h: u32,
        font: &crate::core::Font,
        context: &RenderContext,
    ) -> Rect {
        let key = DayLineKey::of(grid, cell_w, cell_h, font);
        let mut cache = self.day_line_cache.borrow_mut();
        if cache.key != Some(key) {
            cache.lines.clear();
            for row in 0..6 {
                for col in 0..7 {
                    let cell = Rect::new(
                        grid.x + col * cell_w as i32,
                        grid.y + row * cell_h as i32,
                        cell_w,
                        cell_h,
                    );
                    cache.lines.push(context.text_line(cell, font));
                }
            }
            cache.key = Some(key);
        }
        cache.lines[row * 7 + col]
    }

    /// Renders `display_month` through this calendar's `date_format` pattern into `out`.
    /// This is what makes `date_format` a live property: it was stored, published
    /// and settable from the day it was added, but `draw` painted a hardcoded
    /// `"<Month> <Year>"`, so setting it changed nothing on screen.
    ///
    /// `chrono`'s `NaiveDate::format(...).to_string()` panics on an unrecognised
    /// specifier, so the pattern is validated with `StrftimeItems` first. An
    /// invalid pattern falls back to the `"<Month> <Year>"` rendering (the
    /// previous, always-valid paint) rather than crashing the draw pass.
    ///
    /// # Why the validation scan is not cached
    ///
    /// The scan runs over the pattern on every frame. Caching it would mean keying a
    /// stored flag on `date_format` — new mutable state derived from a property, which
    /// the plan deliberately does not introduce. The scan is linear in the pattern's
    /// length (a handful of bytes) and is charged once per frame, so the cost is noise
    /// next to the grid paint.
    fn write_month_title(&self, out: &mut String) {
        use chrono::format::{Item, StrftimeItems};
        // The default pattern is rendered through this calendar's names layer, because `%B` means
        // "the month's name" and the layer is where that name lives. A caller-supplied pattern is
        // left to `chrono`, whose own English names are the only thing a literal `%B` can mean once
        // the caller spells the pattern itself.
        if self.date_format == DEFAULT_DATE_FORMAT {
            let _ = write!(
                out,
                "{} {}",
                self.month_name(self.display_month.month()),
                self.display_month.year()
            );
            return;
        }
        let valid = StrftimeItems::new(&self.date_format).all(|item| !matches!(item, Item::Error));
        if valid {
            let _ = write!(out, "{}", self.display_month.format(&self.date_format));
        } else {
            let _ = write!(
                out,
                "{} {}",
                self.month_name(self.display_month.month()),
                self.display_month.year()
            );
        }
    }

    /// The rendered month title; see [`Self::write_month_title`].
    ///
    /// Test-only: the draw pass writes into its own buffer via [`Self::write_month_title`]
    /// directly, so the owned-string convenience is kept for the property tests that pin
    /// the `date_format` contract.
    #[cfg(test)]
    fn formatted_month_title(&self) -> String {
        let mut out = String::new();
        self.write_month_title(&mut out);
        out
    }

    /// Layout constants.
    const NAV_H: u32 = 30;
    const DAY_HEADER_H: u32 = 24;
    /// Width of the week-number gutter, in pixels.
    ///
    /// Two digits at the day-number font plus the column's own padding. Wider than a day column
    /// needs to be for its own text, because the number here is a whole month's ordinal rather than
    /// a day, and a two-digit value that does not fit would be elided to `1…`.
    const WEEK_GUTTER_W: u32 = 26;

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
    ///
    /// # Why the header starts at the grid's left edge, not the control's
    ///
    /// The vertical header (when shown) owns a strip down the left, and the seven day columns begin
    /// to the right of it. The header must start where the columns do, or the eight labels and the
    /// seven columns underneath them disagree about which column is which — which is exactly the
    /// defect a week-number gutter introduces if it is bolted on without moving the sibling.
    fn day_header_rect(&self) -> Rect {
        let r = self.geometry();
        let nav_h = if self.navigation_bar_visible { Self::NAV_H as i32 } else { 0 };
        if self.horizontal_header_visible {
            let grid = self.grid_rect();
            Rect::new(grid.x, r.y + nav_h, grid.width, Self::DAY_HEADER_H)
        } else {
            Rect::new(r.x + self.vertical_header_width() as i32, r.y + nav_h, 0, 0)
        }
    }

    /// The width of the week-number strip: the gutter when the vertical header is shown, else zero.
    ///
    /// One function, read by the header, the grid and the hit test, so the three cannot disagree
    /// about how much room the gutter took. A second `if self.vertical_header_visible` in each of
    /// them is how the eight labels end up one column off from the thirty-one cells.
    fn vertical_header_width(&self) -> u32 {
        if self.vertical_header_visible {
            Self::WEEK_GUTTER_W
        } else {
            0
        }
    }

    /// Returns the day-grid rectangle (remaining area after nav + header).
    fn grid_rect(&self) -> Rect {
        let r = self.geometry();
        let top = (if self.navigation_bar_visible { Self::NAV_H } else { 0 })
            + (if self.horizontal_header_visible { Self::DAY_HEADER_H } else { 0 });
        let h = r.height.saturating_sub(top);
        let gutter = self.vertical_header_width();
        Rect::new(r.x + gutter as i32, r.y + top as i32, r.width.saturating_sub(gutter), h)
    }

    /// Returns the week-number strip's rectangle, or zero-sized if the header is hidden.
    fn week_gutter_rect(&self) -> Rect {
        let grid = self.grid_rect();
        Rect::new(self.geometry().x, grid.y, self.vertical_header_width(), grid.height)
    }

    /// The cell rectangle showing `date`, when it is in the displayed month.
    ///
    /// The inverse of [`Self::date_at_position`], used to damage only the cell a hover or a
    /// selection changed (ADV-001). A date outside the displayed month has no cell here, which
    /// is the honest answer rather than a rectangle from the neighbouring page.
    fn date_cell_rect(&self, date: chrono::NaiveDate) -> Option<Rect> {
        if date.year() != self.display_month.year() || date.month() != self.display_month.month() {
            return None;
        }
        let grid = self.grid_rect();
        if grid.width == 0 || grid.height == 0 {
            return None;
        }
        let cell_w = (grid.width / 7).max(1) as i32;
        let cell_h = (grid.height / 6).max(1) as i32;
        let index = self.leading_blank_count() as i32 + date.day() as i32 - 1;
        if index < 0 {
            return None;
        }
        let (row, col) = (index / 7, index % 7);
        if row >= 6 {
            return None;
        }
        Some(Rect::new(grid.x + col * cell_w, grid.y + row * cell_h, cell_w as u32, cell_h as u32))
    }

    /// Compute the number of leading blank cells before day 1 of the displayed month.
    fn leading_blank_count(&self) -> u32 {
        let first = match self.display_month.with_day(1) {
            Some(d) => d,
            None => return 0,
        };
        let wd = first.weekday();
        let from_mon = wd.num_days_from_monday();
        match self.first_day_of_week {
            chrono::Weekday::Mon => from_mon,
            chrono::Weekday::Sun => (from_mon + 1) % 7,
            _ => from_mon, // fallback
        }
    }

    /// The ISO-8601 week number of `date`, as the vertical header's label.
    ///
    /// ISO week 1 is the week containing the year's first Thursday, which is what makes a week's
    /// number depend on the **Thursday** it contains rather than on its Monday — the reason this is
    /// not simply "the day of the year divided by seven". Weeks that straddle a year boundary
    /// therefore report `52`/`53` of the old year or `1` of the new one, and `chrono` answers that
    /// directly, so the rule lives in one place rather than being re-derived here.
    fn iso_week_of(date: chrono::NaiveDate) -> u32 {
        use chrono::Datelike;
        date.iso_week().week()
    }

    /// The first date of the row-th week of the displayed month, which is what a week number labels.
    ///
    /// `None` when the whole row falls outside the month, because a week label on an empty row would
    /// name a week that the calendar is not showing.
    fn week_row_start(&self, row: u32) -> Option<chrono::NaiveDate> {
        let blanks = self.leading_blank_count();
        let days_in_month =
            max_days_in_month(self.display_month.year(), self.display_month.month());
        // The row's first *painted* day: the leading blanks belong to the previous month
        // and are not drawn, so the label must key off the first in-month cell of the row.
        //
        // # What this used to be
        //
        // `if first_slot < blanks { blanks - first_slot } else { 1 }` returned day 1 for
        // **every row**: the subtraction was backwards, and every row after the first
        // short-circuited to 1. The week gutter therefore painted the same ISO week
        // (`36 36 36 36 36`) beside five different rows — the label named a week the row
        // did not contain. `slot - blanks + 1` is the in-month day at the row's first cell,
        // and saturating at zero gives day 1 for the short first row.
        let first_slot = row * 7;
        let first_day = first_slot.saturating_sub(blanks) + 1;
        if first_day > days_in_month {
            return None;
        }
        let last_slot = first_slot + 6;
        if last_slot > blanks + days_in_month - 1 {
            // The row starts past the month's last day: nothing is painted in it at all.
            if first_slot > blanks + days_in_month - 1 {
                return None;
            }
        }
        self.display_month.with_day(first_day)
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
        let first = self.display_month.with_day(1)?;
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
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(
            dimensions::CALENDAR_DEFAULT_WIDTH,
            dimensions::CALENDAR_DEFAULT_HEIGHT,
        )
    }

    /// Advances the page-change reveal; `true` while it still owes frames.
    fn tick(&mut self, delta_ms: u32) -> bool {
        if !self.page_animation {
            return false;
        }
        self.page_reveal.tick(delta_ms)
    }

    /// Whether the page-change reveal is in flight.
    fn is_animating(&self) -> bool {
        self.page_animation && self.page_reveal.is_animating()
    }

    /// Announces the selected date, because that is the calendar's value.
    ///
    /// # Why the trait default cannot answer here
    ///
    /// It looks the value up under `value` / `progress` / `rating` / `level`, and a calendar
    /// publishes none of those — its value is called `selected_date` and is a `%Y-%m-%d` string. So
    /// a screen reader was told nothing about which day was chosen, on the one control whose entire
    /// state *is* the chosen day.
    ///
    /// The name is read back through the property contract rather than formatted here, so the
    /// announced text and the published `selected_date` are the same string by construction — a
    /// reader and a caller cannot be told two different dates.
    fn accessible_value(&self) -> String {
        crate::widget::capability::types::CapabilityValue::String(naive_date_to_string(
            self.selected_date(),
        ))
        .to_announcement_string()
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why this is explicit
    ///
    /// `connect_event` validates a name against the capability table and registers a hub slot; only
    /// `event_signal_dyn` joins that name to the signal the control actually **emits**. Without an
    /// arm a published name is valid and inert, which is the silent failure
    /// `tools/check_event_signal_dyn.sh` exists to make impossible: the arm set is compared against
    /// the capability's published names, so the two cannot drift.
    ///
    /// # Payload mapping
    ///
    /// A hub name carries no value, so a `Signal1<T>` needs a conversion. The one used here is the
    /// same spelling the property side uses, so a designer reads one representation per kind: a
    /// number as `Int`, a float as `Float`, a flag as `Bool`, text as `String`, and a structured
    /// payload as its debug spelling. A `None` maps to `Null`, which is the same value an absent
    /// optional property uses — so "no selection" is distinguishable from "selection 0".
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "selection_changed" => {
                Some(EventSignalRef::mapped("selection_changed", &self.selection_changed, |v| {
                    CapabilityValue::String(format!("{v:?}"))
                }))
            }
            "activated" => Some(EventSignalRef::mapped("activated", &self.activated, |v| {
                CapabilityValue::String(format!("{v:?}"))
            })),
            "current_page_changed" => Some(EventSignalRef::mapped(
                "current_page_changed",
                &self.current_page_changed,
                // The payload is `(year, month)`, and the declared shape is a mixed tuple, so a
                // subscriber that read the schema expects a `Tuple` of `Int` and `UInt`. This
                // arm used to deliver the `Debug` spelling as a `String`, which the payload gate
                // reported as a declared/received disagreement — a designer could not read the
                // two numbers back out of it.
                |v| {
                    CapabilityValue::Tuple(crate::compat::Vec::from([
                        CapabilityValue::Int(v.0 as i64),
                        CapabilityValue::UInt(v.1 as u64),
                    ]))
                },
            )),
            _ => None,
        }
    }
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
            "show_week_numbers" => Ok(CapabilityValue::Bool(self.show_week_numbers())),
            "selection_mode" => {
                Ok(CapabilityValue::String(self.selection_mode().as_str().to_string()))
            }
            "selected_dates" => Ok(CapabilityValue::Tuple(
                self.selected_dates()
                    .iter()
                    .map(|date| CapabilityValue::String(naive_date_to_string(*date)))
                    .collect(),
            )),
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
            "show_week_numbers" => {
                self.set_show_week_numbers(expect_bool(value)?);
                Ok(())
            }
            "selection_mode" => {
                let token = expect_string(value)?;
                let mode = CalendarSelectionMode::from_token(&token)
                    .ok_or(CapabilityAccessError::TypeMismatch)?;
                self.set_selection_mode(mode);
                Ok(())
            }
            "selected_dates" => {
                let mut dates = Vec::new();
                for text in expect_string_list(value)? {
                    dates.push(crate::widget::capability::coercion::expect_naive_date(
                        CapabilityValue::String(text),
                    )?);
                }
                self.selected_dates = dates;
                self.selected_dates.sort_unstable();
                self.selected_dates.dedup();
                if let Some(last) = self.selected_dates.last().copied() {
                    self.selected_date = last;
                    self.show_page(last);
                }
                self.base.request_redraw();
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
            "show_week_numbers",
            "selection_mode",
            "selected_dates",
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
            Event::MouseMove { pos } => {
                let next = self.date_at_position(*pos);
                if next != self.hovered_date {
                    let previous = self.hovered_date;
                    self.hovered_date = next;
                    // Hover is a cell-local affordance, so only the two cells whose wash changed
                    // are damaged (ADV-001). A date with no cell on this page falls back to the
                    // whole control rather than leaving a stale highlight.
                    for date in [previous, next].into_iter().flatten() {
                        match self.date_cell_rect(date) {
                            Some(rect) => self.base.request_redraw_in(rect),
                            None => self.base.request_redraw(),
                        }
                    }
                }
            }
            Event::MouseLeave { .. } => {
                if let Some(date) = self.hovered_date.take() {
                    match self.date_cell_rect(date) {
                        Some(rect) => self.base.request_redraw_in(rect),
                        None => self.base.request_redraw(),
                    }
                }
            }
            Event::MousePress { pos, button, .. } if *button == 1 => {
                // Check nav-bar button clicks
                if self.navigation_bar_visible {
                    // The Today button comes first: it is the most specific target in the bar.
                    if self.today_button_rect().is_some_and(|rect| rect.contains_point(*pos)) {
                        self.show_today();
                        return;
                    }
                    // An open menu takes the press before the title that anchors it.
                    if let Some(panel) = self.month_menu_rect() {
                        if panel.contains_point(*pos) {
                            let row = ((pos.y - panel.y) / 20).clamp(0, 11) as u32 + 1;
                            if let Some(date) = self.month_menu_option(row) {
                                self.show_page(date);
                            }
                            self.month_menu_open = false;
                            self.base.request_redraw();
                            return;
                        }
                    }
                    if let Some(panel) = self.year_menu_rect() {
                        if panel.contains_point(*pos) {
                            let row = ((pos.y - panel.y) / 20).clamp(0, 4) as u32;
                            if let Some(year) = self.year_menu_option(row) {
                                if let Some(date) = self.display_month.with_year(year) {
                                    self.show_page(date);
                                }
                            }
                            self.year_menu_open = false;
                            self.base.request_redraw();
                            return;
                        }
                    }
                    if self.dropdown_navigation {
                        if self.month_title_rect().contains_point(*pos) {
                            self.month_menu_open = !self.month_menu_open;
                            self.year_menu_open = false;
                            self.base.request_redraw();
                            return;
                        }
                        if self.year_title_rect().is_some_and(|rect| rect.contains_point(*pos)) {
                            self.year_menu_open = !self.year_menu_open;
                            self.month_menu_open = false;
                            self.base.request_redraw();
                            return;
                        }
                    }
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
                // A press anywhere else dismisses an open navigation menu before it
                // reaches the grid, so the first click only closes the panel.
                if self.month_menu_open || self.year_menu_open {
                    self.month_menu_open = false;
                    self.year_menu_open = false;
                    self.base.request_redraw();
                    return;
                }
                // Click on day grid
                if let Some(date) = self.date_at_position(*pos) {
                    if date >= self.minimum_date && date <= self.maximum_date {
                        // `set_selected_date` moves the page with the selection, so the grid
                        // follows the click without a second assignment here.
                        self.select_from_user(date);
                    }
                }
            }
            Event::MouseDoubleClick { pos, .. } => {
                // Double click commits the date: the reference toolkit's `activated`. A double click also delivers
                // a press first, so the cell is already selected; emitting here is what lets a
                // date-picker host close its popup on the gesture the user made.
                if let Some(date) = self.date_at_position(*pos) {
                    if date >= self.minimum_date
                        && date <= self.maximum_date
                        && !self.is_date_disabled(date)
                    {
                        self.set_selected_date(date);
                        self.activated.emit(date);
                    }
                }
            }
            Event::KeyPress { key, modifiers: _ } => {
                match *key {
                    37 => {
                        // Left arrow
                        if let Some(prev_day) = self.selected_date.pred_opt() {
                            self.select_from_user(prev_day);
                        }
                    }
                    38 => {
                        // Up arrow
                        if let Some(prev_week) =
                            self.selected_date.checked_sub_signed(chrono::TimeDelta::days(7))
                        {
                            self.select_from_user(prev_week);
                        }
                    }
                    39 => {
                        // Right arrow
                        if let Some(next_day) = self.selected_date.succ_opt() {
                            self.select_from_user(next_day);
                        }
                    }
                    40 => {
                        // Down arrow
                        if let Some(next_week) =
                            self.selected_date.checked_add_signed(chrono::TimeDelta::days(7))
                        {
                            self.select_from_user(next_week);
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
                    13 | 32 => {
                        // Enter / Space commits the current selection: the reference toolkit's `activated`.
                        self.activated.emit(self.selected_date);
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
        // The day painted as "today" is the control's own field, not a clock read: see its field
        // docs. Reading `Local::now()` here made every snapshot a function of the render date.
        let today = self.today;
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
        context.face_with_gradient_corners(
            rect,
            calendar_bg,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().resolved_corner_radii().to_array(),
            Color::BLACK,
        );
        // Same radius as the face above, so the outline closes at all four corners.
        crate::widget::draw::stroke_surface_border(
            context,
            rect,
            self.style().resolved_corner_radii().to_array(),
            border_color,
            1,
        );

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
            // Every run in this bar shares one centre line; the arrows are `ChevronLeft` /
            // `ChevronRight` **icon outlines**, not the `◀`/`▶` text glyphs (U+25C0/U+25B6 are
            // covered by no bundled face, so those drew as 8x8 bitmap blocks). Centring comes from
            // `arrow_line`, the bar's own line box, so the arrows and the title keep one baseline.
            let arrow_color = style.text_color.unwrap_or(if enabled {
                Color::rgb(60, 60, 60)
            } else {
                dim_color
            });
            let arrow_font = effective_font(style);
            let arrow_line = context.text_line(nav, arrow_font);
            // The arrows' own box width, used to reserve the span the title may use.
            let btn_w = 30i32;
            let arrow_side = arrow_line.height.max(1);
            let prev_box = Rect::new(nav.x + 8, arrow_line.y, arrow_side, arrow_side);
            crate::widget::draw_icon_at(context, prev_box, arrow_color, IconName::ChevronLeft);
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
            let title_font = role_font(style, 13.0).with_bold(true);
            let title_line = context.text_line(nav, &title_font);
            let title_bounds = Rect::new(
                left_edge,
                title_line.y,
                (band_right - left_edge).max(0) as u32,
                title_line.height,
            );
            if self.dropdown_navigation || self.today_button_visible {
                // The navigable layout: a month segment and a year segment, then the
                // Today button. Every box comes from the same span derivation the hit
                // test reads.
                if self.dropdown_navigation {
                    let month_box = self.month_title_rect();
                    let month_text = self.month_name(self.display_month.month());
                    context.draw_text_fitted_cow(
                        Rect::new(month_box.x, title_line.y, month_box.width, title_line.height),
                        month_text,
                        &title_font,
                        text_color,
                        HorizontalAlignment::Center,
                    );
                    if let Some(year_box) = self.year_title_rect() {
                        // The year segment is the cached year-menu window's first label offset by
                        // two: the window starts at `year - 2`, so the displayed year is slot 2.
                        let year_text = self.year_menu_label(2);
                        context.draw_text_fitted_cow(
                            Rect::new(year_box.x, title_line.y, year_box.width, title_line.height),
                            &year_text,
                            &title_font,
                            text_color,
                            HorizontalAlignment::Center,
                        );
                    }
                } else {
                    let title = self.month_title_text();
                    let span = self.nav_content_span();
                    context.draw_text_fitted_cow(
                        Rect::new(span.x, title_line.y, span.width, title_line.height),
                        title.as_str(),
                        &title_font,
                        text_color,
                        HorizontalAlignment::Center,
                    );
                }
                if let Some(today_rect) = self.today_button_rect() {
                    context.draw_text_fitted_cow(
                        Rect::new(today_rect.x, title_line.y, today_rect.width, title_line.height),
                        "Today",
                        &role_font(style, 11.0),
                        text_color,
                        HorizontalAlignment::Center,
                    );
                }
            } else {
                let title = self.month_title_text();
                context.draw_text_fitted_cow(
                    title_bounds,
                    title.as_str(),
                    &title_font,
                    text_color,
                    HorizontalAlignment::Center,
                );
            }
            // ► button
            let next_box = Rect::new(
                nav.x + nav.width as i32 - 8 - arrow_side as i32,
                arrow_line.y,
                arrow_side,
                arrow_side,
            );
            crate::widget::draw_icon_at(context, next_box, arrow_color, IconName::ChevronRight);
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
            let names = self.weekday_name_order();
            for (i, weekday) in names.iter().enumerate() {
                let cell_x = hdr.x + cell_w * i as i32;
                // The weekend tint follows the weekday itself, not its column position: with
                // Sunday first, columns 5 and 6 are Friday and Saturday, so the old `i >= 5`
                // coloured the wrong two headings.
                let is_weekend = matches!(weekday, chrono::Weekday::Sat | chrono::Weekday::Sun);
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
                context.draw_text_fitted_cow(
                    label_bounds,
                    self.weekday_name(*weekday),
                    &role_font(style, 11.0).with_bold(true),
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
            // A page change may reveal the new grid rather than hard-cutting to it. The
            // clip is only pushed while the reveal is in flight, so the settled frame is
            // byte-identical to the un-animated one.
            let revealing = self.page_animation && self.page_reveal.is_animating();
            if revealing {
                let revealed = self.page_reveal.revealed(
                    grid,
                    crate::widget::menu_toolbar::popup_reveal::RevealDirection::Down,
                    crate::widget::menu_toolbar::popup_reveal::PopupReveal::travel_cap_for(
                        crate::core::Size::new(grid.width, grid.height),
                    ),
                );
                context.push_clip(revealed.x, revealed.y, revealed.width, revealed.height);
            }

            // ── 3a. The week-number gutter ──
            //
            // The ISO week number of each row, in the strip the grid left on its own left. Drawn
            // from the grid's own `cell_h` and rows, so a label sits beside the row it names even
            // when the control is shorter than six rows' worth and the rows are compressed.
            let gutter = self.week_gutter_rect();
            if gutter.width > 0 {
                let font = role_font(style, 10.0);
                let mut week_text = String::new();
                for row in 0..6u32 {
                    let Some(start) = self.week_row_start(row) else {
                        continue;
                    };
                    let band =
                        Rect::new(gutter.x, grid.y + (row * cell_h) as i32, gutter.width, cell_h);
                    if band.y + band.height as i32 > rect.y + rect.height as i32 {
                        break;
                    }
                    // A step quieter than the day numbers: a week number is an index into the
                    // calendar, not a selectable day, and drawing it at full strength made it read
                    // as one more date in the row.
                    week_text.clear();
                    let _ = core::fmt::Write::write_fmt(
                        &mut week_text,
                        format_args!("{}", Self::iso_week_of(start)),
                    );
                    context.draw_text_line_cow(
                        band,
                        &week_text,
                        &font,
                        text_color.blend(&calendar_bg, 0.45),
                        HorizontalAlignment::Center,
                    );
                }
                // The gutter's right rule is what separates the index from the dates, and it is the
                // grid's own line so the two cannot drift apart in a light or dark appearance.
                if style.border_width.unwrap_or(1) > 0 {
                    context.draw_line(
                        Point::new(gutter.x + gutter.width as i32 - 1, grid.y),
                        Point::new(gutter.x + gutter.width as i32 - 1, grid.y + grid.height as i32),
                        border_color,
                    );
                }
            }

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

            // The day-number face is a property of the control, not of a cell: it was resolved
            // inside the 42-cell loop, so a calendar repaint asked the theme for the role font 42
            // times a frame.
            let day_font = role_font(style, 11.0);
            // One reused buffer for the day numbers: `format!` built a fresh `String` for each of
            // the 42 cells, and the text is at most two digits.
            let mut day_text = String::new();

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

                    let Some(date) = self.display_month.with_day(day_num as u32) else {
                        continue;
                    };

                    // Cell background
                    let in_range = date >= self.minimum_date && date <= self.maximum_date;
                    let user_disabled = self.is_date_disabled(date);
                    let blocked = !in_range || user_disabled;
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
                    // A hovered cell is a pointer affordance, not a state: one faint wash,
                    // skipped when the cell already owns a state colour or cannot be chosen.
                    if !blocked
                        && self.hovered_date == Some(date)
                        && date != today
                        && !self.is_date_selected(date)
                    {
                        context.fill_rect(cell_rect, calendar_bg.blend(&text_color, 0.06));
                    }
                    if date == today && self.is_date_selected(date) {
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
                    } else if self.is_date_selected(date) {
                        context.fill_rect(cell_rect, selected_bg);
                    } else if !in_range {
                        // Outside range — dimmed. A dimmed *surface*, so a theme's
                        // background stays visible under the veil instead of a fixed
                        // near-white rectangle appearing in a dark calendar.
                        context.fill_rect(cell_rect, calendar_bg.blend(&text_color, 0.9));
                    } else if user_disabled {
                        // A user-disabled day keeps the calendar's surface and takes the
                        // dimmed ink below; it must not look selectable.
                        context.fill_rect(cell_rect, calendar_bg.blend(&text_color, 0.94));
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
                    let day_color = if blocked {
                        dim_color
                    } else if self.is_date_selected(date) {
                        let tint = calendar_bg.blend(&selected_bg, selected_bg.a as f32 / 255.0);
                        tint.contrast_color()
                    } else if date == today {
                        let tint = calendar_bg.blend(&today_bg, today_bg.a as f32 / 255.0);
                        tint.contrast_color()
                    } else {
                        // The per-date override is the caller's semantics (a holiday, a
                        // billing date), so it wins over the plain ink but never over a
                        // state colour or a disabled day.
                        self.date_text_color_for(date).unwrap_or(text_color)
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
                    let day_line = self.day_line_rect(
                        row as usize,
                        col as usize,
                        grid,
                        cell_w,
                        cell_h,
                        &day_font,
                        context,
                    );
                    day_text.clear();
                    let _ = core::fmt::Write::write_fmt(&mut day_text, format_args!("{day_num}"));
                    context.draw_text_fitted_cow(
                        day_line,
                        &day_text,
                        &day_font,
                        day_color,
                        HorizontalAlignment::Center,
                    );
                }
            }
            if revealing {
                context.pop_clip();
            }
        }

        // ── 4. Open navigation menus ──
        //
        // Painted last so they sit above the grid, and only when the caller enabled
        // dropdown navigation; the default bar has neither panel.
        if let Some(panel) = self.month_menu_rect() {
            context.fill_rect(panel, calendar_bg);
            context.draw_rect(panel, border_color);
            let font = role_font(style, 11.0);
            for row in 0..12u32 {
                let band = Rect::new(panel.x + 2, panel.y + row as i32 * 20, panel.width - 4, 20);
                let Some(option) = self.month_menu_option(row + 1) else { continue };
                let is_current = option.month() == self.display_month.month()
                    && option.year() == self.display_month.year();
                if is_current {
                    context.fill_rect(band, calendar_bg.blend(&text_color, 0.14));
                }
                let label = self.month_name(option.month());
                context.draw_text_fitted_cow(
                    Rect::new(band.x + 6, band.y, band.width - 8, band.height),
                    label,
                    &font,
                    text_color,
                    HorizontalAlignment::Left,
                );
            }
        }
        if let Some(panel) = self.year_menu_rect() {
            context.fill_rect(panel, calendar_bg);
            context.draw_rect(panel, border_color);
            let font = role_font(style, 11.0);
            for row in 0..5u32 {
                let Some(year) = self.year_menu_option(row) else { continue };
                let band = Rect::new(panel.x + 2, panel.y + row as i32 * 20, panel.width - 4, 20);
                if year == self.display_month.year() {
                    context.fill_rect(band, calendar_bg.blend(&text_color, 0.14));
                }
                let label = self.year_menu_label(row as usize);
                context.draw_text_fitted_cow(
                    Rect::new(band.x + 6, band.y, band.width - 8, band.height),
                    &label,
                    &font,
                    text_color,
                    HorizontalAlignment::Left,
                );
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
    use crate::core::{Font, Rect, Size};
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

    /// Turning on the vertical header paints a week-number gutter, and shifts the grid right.
    ///
    /// # The defect this pins
    ///
    /// `vertical_header_visible` was the one of four sibling visibility flags with **no reader in
    /// `draw` at all**: `grid_visible`, `navigation_bar_visible` and `horizontal_header_visible` each
    /// gate real chrome, while this one had only its getter, its setter, its schema row and a
    /// round-trip test. Setting it redrew an identical calendar, so a host asking for week numbers
    /// got a silent no-op.
    ///
    /// The assertions cover both halves of the feature, because either alone is satisfiable by a
    /// wrong implementation: the document must gain ink (the gutter is painted), **and** the day grid
    /// must start further right (the gutter *took* room rather than overlaying the first column --
    /// an overlay would corrupt column 0, which is a worse defect than the one being fixed).
    #[test]
    fn the_vertical_header_paints_week_numbers_and_yields_its_room() {
        use crate::widget::svg::{render_to_svg, text_subpath_count};
        let _theme_guard = crate::style::theme_test_guard();

        let make = |vertical: bool| {
            let mut cal = Calendar::new(Rect::new(0, 0, 300, 250));
            cal.set_grid_visible(true);
            cal.set_vertical_header_visible(vertical);
            cal
        };

        let mut off = make(false);
        let plain_grid = off.grid_rect();
        let off_svg = render_to_svg(&mut off);

        let mut on = make(true);
        let guttered_grid = on.grid_rect();
        let on_svg = render_to_svg(&mut on);

        assert_ne!(off_svg, on_svg, "the week-number gutter must be painted");
        assert!(
            text_subpath_count(&on_svg) > text_subpath_count(&off_svg),
            "the gutter paints week numbers, so it must add glyph ink"
        );
        assert_eq!(
            guttered_grid.x,
            plain_grid.x + Calendar::WEEK_GUTTER_W as i32,
            "the seven day columns must start past the gutter, not underneath it"
        );
        assert_eq!(
            guttered_grid.width,
            plain_grid.width - Calendar::WEEK_GUTTER_W,
            "and the grid gives up exactly the gutter's width"
        );
    }

    /// ISO week numbers are the Thursday-based ones, including at a year boundary.
    ///
    /// The rule is what makes a week's number depend on the Thursday it contains, so a naive
    /// "day of year / 7" is wrong twice a year. `2026-01-01` is a Thursday, so it starts ISO week 1;
    /// `2026-12-31` is a Thursday, so it *starts* week 53 of 2026 rather than belonging to 2027.
    #[test]
    fn week_numbers_follow_the_iso_thursday_rule() {
        assert_eq!(Calendar::iso_week_of(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()), 1);
        assert_eq!(Calendar::iso_week_of(NaiveDate::from_ymd_opt(2026, 12, 31).unwrap()), 53);
        assert_eq!(Calendar::iso_week_of(NaiveDate::from_ymd_opt(2026, 6, 8).unwrap()), 24);
        // The two days either side of a year boundary that a naive rule gets wrong.
        assert_eq!(Calendar::iso_week_of(NaiveDate::from_ymd_opt(2024, 12, 30).unwrap()), 1);
        assert_eq!(Calendar::iso_week_of(NaiveDate::from_ymd_opt(2025, 1, 1).unwrap()), 1);
    }

    /// Each painted row reports the week it actually contains.
    ///
    /// The gutter used to repeat one number for every row (`36 36 36 36 36` in the snapshot):
    /// `week_row_start`'s first-day expression was inverted, so every row after the first
    /// returned day 1. A label that names the wrong week is worse than none — it is the thing
    /// the reader scrolls a calendar to count.
    #[test]
    fn each_row_reports_its_own_week_number() {
        let mut cal = Calendar::new(Rect::new(0, 0, 300, 250));
        cal.set_display_month(NaiveDate::from_ymd_opt(2026, 9, 1).unwrap());
        let weeks: Vec<u32> =
            (0..6).filter_map(|row| cal.week_row_start(row)).map(Calendar::iso_week_of).collect();
        assert!(weeks.len() >= 5, "September 2026 spans six rows, five of them populated");
        assert!(
            weeks.windows(2).all(|pair| pair[1] == pair[0] + 1),
            "consecutive rows are consecutive weeks, got {weeks:?}"
        );
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
        // A month header, not a date: the title names the month the grid shows, which is
        // what a reader looks for at the top of a calendar.
        assert_eq!(cal.date_format(), "%B %Y");
        cal.set_date_format("%d/%m/%Y".to_string());
        assert_eq!(cal.date_format(), "%d/%m/%Y");
    }

    #[test]
    fn date_format_is_applied_to_the_painted_title() {
        // Pins the defect: `date_format` was stored, published and settable, but
        // `draw` painted a hardcoded "<Month> <Year>" so setting it changed
        // nothing. This asserts the *rendered* title, not a get/set round-trip.
        let mut cal = Calendar::new(Rect::new(0, 0, 300, 250));
        cal.set_display_month(chrono::NaiveDate::from_ymd_opt(2026, 3, 15).unwrap());
        assert_eq!(cal.formatted_month_title(), "March 2026");

        cal.set_date_format("%d/%m/%Y".to_string());
        assert_eq!(cal.formatted_month_title(), "15/03/2026");

        // A pattern chrono cannot resolve must fall back, never panic in paint.
        cal.set_date_format("%Q".to_string());
        assert_eq!(cal.formatted_month_title(), "March 2026");
    }

    #[test]
    fn calendar_signal_accessors() {
        let cal = Calendar::new(Rect::new(0, 0, 300, 250));
        let _ = &cal.selection_changed;
        let _ = &cal.activated;
        let _ = &cal.current_page_changed;
    }

    /// A page move is reported exactly once per month crossed, from every navigation path.
    #[test]
    fn moving_the_page_reports_the_new_month() {
        let mut cal = Calendar::new(Rect::new(0, 0, 300, 250));
        cal.set_display_month(NaiveDate::from_ymd_opt(2026, 12, 1).unwrap());
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = seen.clone();
        cal.current_page_changed.connect(move |page| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*page);
            }
        });

        cal.show_next_month();
        assert_eq!(cal.display_month().month(), 1);
        assert_eq!(cal.display_month().year(), 2027, "December rolls into January");
        cal.show_previous_month();
        // Enter/Space and the paging keys route through the same helper, so a programmatic
        // call and a key press cannot disagree about what counts as a page change.
        cal.handle_event(&Event::key_press(34, 0));
        let pages = seen.lock().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(pages, vec![(2027, 1), (2026, 12), (2027, 1)]);
    }

    /// Activation is the commit gesture, distinct from selection.
    #[test]
    fn a_double_click_or_enter_activates_the_selected_date() {
        let mut cal = Calendar::new(Rect::new(0, 0, 300, 250));
        cal.set_display_month(NaiveDate::from_ymd_opt(2026, 3, 1).unwrap());
        cal.set_selected_date(NaiveDate::from_ymd_opt(2026, 3, 12).unwrap());
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = seen.clone();
        cal.activated.connect(move |date| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*date);
            }
        });

        cal.handle_event(&Event::key_press(13, 0));
        // And the pointer gesture: hit the centre of the cell that shows the 12th.
        let grid = cal.grid_rect();
        let cell_w = (grid.width / 7).max(1) as i32;
        let cell_h = (grid.height / 6).max(1) as i32;
        let mut target = None;
        'search: for row in 0..6 {
            for col in 0..7 {
                let point = Point::new(
                    grid.x + col * cell_w + cell_w / 2,
                    grid.y + row * cell_h + cell_h / 2,
                );
                if cal.date_at_position(point)
                    == Some(NaiveDate::from_ymd_opt(2026, 3, 12).unwrap())
                {
                    target = Some(point);
                    break 'search;
                }
            }
        }
        let point = target.expect("the 12th is in the displayed March grid");
        cal.handle_event(&Event::MouseDoubleClick { pos: point, button: 1 });
        let activated = seen.lock().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(
            activated,
            vec![
                NaiveDate::from_ymd_opt(2026, 3, 12).unwrap(),
                NaiveDate::from_ymd_opt(2026, 3, 12).unwrap()
            ],
            "Enter and a double click are both commits"
        );
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

    /// How far inside its glyph **box** a run's ink can start and still count as being on
    /// that box's own line.
    ///
    /// # Why a tolerance is needed at all
    ///
    /// The bitmap face fills its whole glyph box (`font8x8` rectangles span the box), so a run's
    /// ink top *is* the box top, exactly. An **outline** face draws a real glyph whose ink is
    /// inset — a digit's cap or an `M`'s shoulder does not reach the box's top edge — so the ink
    /// top sits one or two pixels below the box top at this font size. A lookup that required
    /// `ink.top == box.top`, or an assertion that pinned the two equal, encoded a property of the
    /// bitmap face rather than of the layout, and is false the moment an outline is drawn.
    ///
    /// The bound is deliberately tight: it has to absorb the outline inset (≤ 2 px at 11–14 px)
    /// and nothing more, so a run drawn a whole line low still fails.
    const INK_INSET_TOLERANCE: i32 = 3;

    /// Text runs recovered from a rendered SVG, as `(left, top, right, bottom)` ink boxes.
    ///
    /// # Why the ink box and not the string
    ///
    /// Text leaves the backend as **glyph geometry** — one `<path>` per `draw_text` — so the
    /// document contains a picture of the run, not the run. A test therefore has to locate a run
    /// by *where it is*, which is strictly better than locating it by *what it says*: the old
    /// `texts()` helper matched `body == "1"` and then asserted on the element's `x`, so it
    /// verified the backend's attribute rather than the ink the control produced, and a mis-placed
    /// glyph with a correct attribute would have passed.
    ///
    /// # Why this delegates instead of parsing `d` itself
    ///
    /// The first form of this helper read the `font8x8` rectangle spelling directly: split `d` on
    /// `M` and take the four integers of `M{x} {y}h{w}v{h}`. That is the *bitmap* path's grammar
    /// and only its grammar. An outline face writes `M21.45 30.24L...Z` with fractional vertices,
    /// where the same split reads `21` as `x`, `45` as `y`, `30` as `w` — geometry that is not ink
    /// and not where the glyph is, so every lookup missed and the test panicked with an empty
    /// search rather than a wrong value. The crate already has the correct reader, which knows
    /// both grammars (and the `data-text` provenance tag that distinguishes a run from a drawn
    /// shape); duplicating it here was the actual bug. One reader, so the two cannot disagree.
    ///
    /// One `<path>` is one `draw_text`, so this is one box per run, in document order.
    fn ink_runs(svg: &str) -> Vec<(i32, i32, i32, i32)> {
        crate::widget::svg::text_ink_boxes(svg)
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
            // The matching ink, located by the box rather than by the string it spells. The run
            // is matched by its **centre** on both axes: the centre is the quantity the test
            // asserts on, and unlike the ink's top edge it lies in the same band whichever face
            // drew the run (a bitmap's top edge is the box top; an outline's is inset below it).
            // Matching on the centre is therefore the same predicate for both backends rather than
            // one that silently only ever matched the bitmap.
            let ink = runs
                .iter()
                .find(|(l, t, r, b)| {
                    let centre_x = (l + r) / 2;
                    let centre_y = (t + b) / 2;
                    let inside_x = centre_x >= box_x && centre_x < box_x + advance;
                    let inside_y = centre_y >= box_y && centre_y < box_y + line_h;
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
                ink.1 - box_y <= INK_INSET_TOLERANCE,
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
    /// glyph's own bearings bias the ink by up to half a pixel). Asserting the margin rather than
    /// exactness keeps the test a statement about the layout and not about the typeface.
    ///
    /// The heading's **line box** top is `hdr.y + 6`, the `y` `draw_text_fitted` was handed, and
    /// the ink starts there exactly under the bitmap face. Under an outline face the ink is inset
    /// a pixel or two (`INK_INSET_TOLERANCE`), so the check is that the ink begins *on* that line
    /// rather than that it begins on the same integer; the column-space and centring assertions
    /// above are unchanged and remain the substance of the test.
    #[test]
    fn a_weekday_heading_is_centred_in_its_column() {
        // Holds the crate-wide theme guard: this test renders, and a concurrent
        // test that switches the appearance would otherwise change a later frame.
        let _theme_guard = crate::style::theme_test_guard();
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
            assert!(
                (found.1 - label_top).abs() <= INK_INSET_TOLERANCE,
                "column {i}: the heading starts on its line box top {label_top}, started at {}",
                found.1
            );
        }
    }

    /// A screen reader is told which day is selected.
    ///
    /// # The defect this pins
    ///
    /// The trait default looks the value up under `value` / `progress` / `rating` / `level`, and a
    /// calendar's value is called `selected_date` -- so the one control whose entire state is the
    /// chosen day announced nothing about it.
    ///
    /// The assertion reads the *date the control reports* and requires the announcement to contain
    /// it, so the two cannot be told two different days.
    #[test]
    fn the_selected_day_is_announced() {
        use crate::widget::capability::WidgetProperties;
        use crate::widget::Widget;

        let mut calendar = Calendar::new(Rect::new(0, 0, 260, 240));
        let day = match chrono::NaiveDate::from_ymd_opt(2026, 3, 9) {
            Some(date) => date,
            None => panic!("a valid literal date"),
        };
        calendar.set_selected_date(day);

        let announced = calendar.accessible_value();
        let reported = match calendar.get("selected_date") {
            Ok(value) => value.to_announcement_string(),
            Err(e) => panic!("the calendar publishes selected_date: {e:?}"),
        };
        assert_eq!(announced, reported, "the announcement and the published property must agree");
        assert!(
            announced.contains("2026-03-09"),
            "and it must be the date that was set, got {announced:?}"
        );
    }

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("a valid literal date")
    }

    /// The month menu opens from the title and picks the row that was clicked.
    #[test]
    fn dropdown_navigation_opens_the_month_menu_and_picks_a_row() {
        let mut cal = Calendar::new(Rect::new(0, 0, 300, 260));
        cal.set_display_month(date(2026, 5, 1));
        let selection_before = cal.selected_date();
        assert!(!cal.is_dropdown_navigation_enabled(), "opt-in");

        cal.set_dropdown_navigation(true);
        let title = cal.month_title_rect();
        cal.handle_event(&Event::mouse_press(title.x + 4, title.y + 4, 1));
        assert!(cal.is_month_menu_open());
        let panel = cal.month_menu_rect().expect("the panel is open");

        // Row 2 is March (1-based month options).
        cal.handle_event(&Event::mouse_press(panel.x + 4, panel.y + 40 + 4, 1));
        assert_eq!(cal.display_month().month(), 3);
        assert!(!cal.is_month_menu_open(), "choosing a month closes the panel");
        assert_eq!(cal.selected_date(), selection_before, "the page moves only");

        let year_title = cal.year_title_rect().expect("the year segment exists");
        cal.handle_event(&Event::mouse_press(year_title.x + 4, year_title.y + 4, 1));
        assert!(cal.is_year_menu_open());
    }

    /// The Today button is opt-in and returns the page and the selection to today.
    #[test]
    fn the_today_button_is_opt_in_and_resets_the_page() {
        let mut cal = Calendar::new(Rect::new(0, 0, 320, 240));
        assert!(cal.today_button_rect().is_none());
        cal.set_today_button_visible(true);
        let button = cal.today_button_rect().expect("the button exists once enabled");
        assert!(button.x + button.width as i32 <= cal.nav_rect().x + cal.nav_rect().width as i32);

        cal.set_display_month(date(2020, 1, 1));
        cal.handle_event(&Event::mouse_press(button.x + 4, button.y + 4, 1));
        assert_eq!(cal.selected_date(), chrono::Local::now().date_naive());
        assert_eq!(cal.display_month().month(), cal.selected_date().month());
    }

    /// A disabled day refuses user selection but stays programmatically settable.
    #[test]
    fn disabled_dates_refuse_pointer_and_keyboard_selection() {
        let mut cal = Calendar::new(Rect::new(0, 0, 280, 240));
        cal.set_selected_date(date(2026, 6, 10));
        cal.set_disabled_dates(vec![date(2026, 6, 11)]);
        assert!(cal.is_date_disabled(date(2026, 6, 11)));

        cal.handle_event(&Event::key_press(39, 0));
        assert_eq!(cal.selected_date(), date(2026, 6, 10), "Right must skip the disabled day");

        cal.handle_event(&Event::key_press(37, 0));
        assert_eq!(cal.selected_date(), date(2026, 6, 9));

        // The host may still pin a disabled day as the value.
        cal.set_selected_date(date(2026, 6, 11));
        assert_eq!(cal.selected_date(), date(2026, 6, 11));
    }

    /// Hovering a cell tracks it and clears on leave.
    #[test]
    fn the_hovered_day_follows_the_pointer() {
        let mut cal = Calendar::new(Rect::new(0, 0, 280, 240));
        let grid = cal.grid_rect();
        let cell_w = (grid.width / 7) as i32;
        let cell_h = (grid.height / 6) as i32;
        let blanks = cal.leading_blank_count() as i32;
        // Day 15 is always inside the month, whatever day the first falls on.
        let slot = blanks + 14;
        let pos = Point::new(grid.x + (slot % 7) * cell_w + 2, grid.y + (slot / 7) * cell_h + 2);
        cal.handle_event(&Event::mouse_move(pos.x, pos.y));
        assert_eq!(
            cal.hovered_date,
            Some(cal.display_month().with_day(15).expect("day 15 exists")),
            "the hovered cell must be the date under the pointer"
        );
        cal.handle_event(&Event::MouseLeave { pos });
        assert_eq!(cal.hovered_date, None, "leaving the control clears the highlight");
    }

    /// ADV-001: a date maps back to exactly the cell the pointer path maps from it.
    #[test]
    fn a_date_maps_back_to_its_cell_rectangle() {
        let mut cal = Calendar::new(Rect::new(0, 0, 280, 240));
        cal.set_display_month(NaiveDate::from_ymd_opt(2026, 3, 1).unwrap());
        let date = NaiveDate::from_ymd_opt(2026, 3, 12).unwrap();
        let rect = cal.date_cell_rect(date).expect("the 12th is on the March page");
        let grid = cal.grid_rect();
        let cell_w = (grid.width / 7).max(1) as i32;
        let cell_h = (grid.height / 6).max(1) as i32;
        assert_eq!(rect.width as i32, cell_w);
        assert_eq!(rect.height as i32, cell_h);
        assert_eq!(
            cal.date_at_position(Point::new(rect.x + cell_w / 2, rect.y + cell_h / 2)),
            Some(date),
            "the damage rectangle must cover the cell the hit test names"
        );
        assert!(
            cal.date_cell_rect(NaiveDate::from_ymd_opt(2026, 4, 12).unwrap()).is_none(),
            "another month has no cell on this page"
        );
    }

    /// A per-date ink and a page reveal are both opt-in and reach the painter.
    #[test]
    fn per_date_ink_and_the_page_reveal_are_opt_in() {
        use crate::widget::Widget;

        fn holiday(date: NaiveDate) -> Option<Color> {
            (date.day() == 1).then_some(Color::rgb(200, 30, 30))
        }

        let mut cal = Calendar::new(Rect::new(0, 0, 280, 240));
        cal.set_display_month(date(2026, 6, 1));
        let plain = crate::widget::svg::render_to_svg(&mut cal);
        cal.set_date_text_color(Some(holiday));
        let tinted = crate::widget::svg::render_to_svg(&mut cal);
        assert_ne!(plain, tinted, "the per-date ink must reach the painter");
        assert_eq!(cal.date_text_color_for(date(2026, 6, 1)), Some(Color::rgb(200, 30, 30)));
        assert_eq!(cal.date_text_color_for(date(2026, 6, 2)), None);

        assert!(!cal.is_page_animation_enabled());
        cal.set_page_animation(true);
        cal.show_next_month();
        assert!(cal.is_animating(), "a page change starts the reveal");
        for _ in 0..120 {
            cal.tick(16);
        }
        assert!(!cal.is_animating(), "the reveal settles");
    }

    #[test]
    fn multi_and_range_modes_share_the_selected_dates() {
        let mut cal = Calendar::new(Rect::new(0, 0, 280, 240));
        assert_eq!(cal.selection_mode(), CalendarSelectionMode::Single);

        cal.set_selection_mode(CalendarSelectionMode::Multi);
        cal.clear_selection();
        cal.set_selected_date(date(2026, 6, 1));
        cal.set_selected_date(date(2026, 6, 3));
        assert_eq!(cal.selected_dates(), &[date(2026, 6, 1), date(2026, 6, 3)]);
        assert!(cal.is_date_selected(date(2026, 6, 3)));
        cal.set_selected_date(date(2026, 6, 1));
        assert_eq!(cal.selected_dates(), &[date(2026, 6, 1), date(2026, 6, 3)]);
        cal.clear_selection();
        assert!(cal.selected_dates().is_empty());

        cal.set_selection_mode(CalendarSelectionMode::Range);
        cal.set_selected_date(date(2026, 6, 10));
        cal.set_selected_date(date(2026, 6, 12));
        assert_eq!(
            cal.selected_dates(),
            &[date(2026, 6, 10), date(2026, 6, 11), date(2026, 6, 12)]
        );
    }

    #[test]
    fn the_week_number_property_addresses_the_gutter() {
        let mut cal = Calendar::new(Rect::new(0, 0, 280, 240));
        assert!(!cal.show_week_numbers());
        cal.set_show_week_numbers(true);
        assert!(cal.show_week_numbers());
        assert!(cal.is_vertical_header_visible());
        assert_eq!(cal.get("show_week_numbers"), Ok(CapabilityValue::Bool(true)));
    }

    /// All seven weekday names resolve, in the order the header paints them (ADV-01).
    #[test]
    fn every_weekday_name_resolves_and_the_order_follows_the_first_day() {
        let mut cal = Calendar::new(Rect::new(0, 0, 300, 250));
        let expected = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
        for (index, name) in expected.iter().enumerate() {
            let weekday = chrono::Weekday::try_from(index as u8).expect("0..6 is a weekday");
            assert_eq!(cal.weekday_name(weekday), *name, "{weekday:?} must name itself");
        }

        assert_eq!(cal.weekday_name_order()[0], chrono::Weekday::Mon);
        cal.set_first_day_of_week(chrono::Weekday::Sun);
        assert_eq!(cal.weekday_name_order()[0], chrono::Weekday::Sun);
        assert_eq!(cal.weekday_name_order()[6], chrono::Weekday::Sat);
        // The names follow the weekday, not the column: Sunday is still Sunday.
        assert_eq!(cal.weekday_name(cal.weekday_name_order()[0]), "Sun");
    }

    /// The names layer is one source for the header, the title and the month menu.
    #[test]
    fn replacing_the_names_changes_what_is_painted() {
        let _theme_guard = crate::style::theme_test_guard();
        let mut cal = Calendar::new(Rect::new(0, 0, 300, 250));
        let before = crate::widget::svg::render_to_svg(&mut cal);
        let mut months = core::array::from_fn(|index| format!("M{}", index + 1));
        months[0] = "Januar".to_string();
        cal.set_month_names(months);
        let after = crate::widget::svg::render_to_svg(&mut cal);
        assert_ne!(before, after, "the header/title text comes from the names layer");
        assert_eq!(cal.month_name(1), "Januar");
    }
}
