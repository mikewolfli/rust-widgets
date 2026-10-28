// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! DateRangePicker widget — a calendar-based date range selection widget.
//!
//! The DateRangePicker displays a calendar month grid where users click to
//! select a start date and then click again to select an end date, forming
//! a date range. The range between the two dates is visually highlighted.
//! A `range_changed` signal is emitted whenever the selection changes.

use super::date_utils::{days_in_month, parse_iso_date, DAY_NAMES, MONTH_NAMES};
use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::{RenderContext, TEXT_FIT_MARGIN};
use crate::signal::Signal1;
use crate::widget::capability::coercion::{expect_string, expect_string_list};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::role_font;
use crate::widget::{BaseWidget, Draw, IconName, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

use chrono::Datelike;

/// A named shortcut range offered above the calendar grid.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DatePreset {
    /// The chip's caption.
    pub label: String,
    /// Inclusive first day, or `None` when the preset only sets an anchor.
    pub start: Option<(i32, u32, u32)>,
    /// Inclusive last day, or `None`.
    pub end: Option<(i32, u32, u32)>,
}

impl DatePreset {
    /// Creates a preset from its label and endpoints.
    pub fn new(label: impl Into<String>, start: (i32, u32, u32), end: (i32, u32, u32)) -> Self {
        Self { label: label.into(), start: Some(start), end: Some(end) }
    }
}

/// A selected date range: `(start_date, end_date)` each as `(year, month, day)`.
pub(crate) type DateRange = (Option<(i32, u32, u32)>, Option<(i32, u32, u32)>);

/// Returns the day of week (0=Sunday, 1=Monday, ..., 6=Saturday)
/// for the given date using Zeller-like / Tomohiko Sakamoto's algorithm.
fn day_of_week(year: i32, month: u32, day: u32) -> u32 {
    let (y, m) = if month < 3 { (year - 1, month + 12) } else { (year, month) };
    let c = y / 100;
    let y = y % 100;
    let d = day as i32;
    let w = (d + (13 * (m as i32 + 1)) / 5 + y + y / 4 + c / 4 - 2 * c) % 7;
    // Zeller's congruence numbers the week **Saturday = 0**, while `DAY_NAMES` and the
    // leading-blank count are Sunday-first. Returning the raw value shifted the whole grid
    // one column: January 2025 (whose 1st is a Wednesday) drew its 1 under "Th", and every
    // click then selected the date one day after the cell under the pointer. Adding 6 and
    // reducing modulo 7 converts Saturday-based to Sunday-based (Wed 4 → 3); `rem_euclid`
    // keeps proleptic years with a negative weekday from producing a negative column.
    (w + 6).rem_euclid(7) as u32
}

/// DateRangePicker widget — select a date range by clicking start/end dates.
///
/// Displays a calendar for a given month/year. The user clicks once to set
/// the start date, and clicks again to set the end date. The range is
/// visually highlighted. Clicking outside the grid (or an already-selected
/// start) with no end date clears the selection.
pub struct DateRangePicker {
    base: BaseWidget,
    display_year: i32,
    display_month: u32,
    start_date: Option<(i32, u32, u32)>,
    end_date: Option<(i32, u32, u32)>,
    hover_date: Option<(i32, u32, u32)>,
    /// The day the grid's first column belongs to.
    ///
    /// The grid offset and the weekday header are derived from this one field, so they cannot
    /// disagree about which column a day sits in. The default is Sunday, the order
    /// [`DAY_NAMES`] is written in and the behaviour before this was configurable.
    first_day_of_week: chrono::Weekday,
    /// Preset chips offered above the grid.
    presets: Vec<DatePreset>,
    /// The index of the preset last applied, when the selection came from one.
    selected_preset: Option<usize>,
    /// An optional `PresetChipBar` kept in step with [`Self::presets`] (MISC-008).
    preset_bar: Option<super::preset_chip_bar::PresetChipBar>,
    /// Whether a second consecutive month is painted beside the first.
    double_month: bool,
    /// Minimum nights between the endpoints; `None` for no minimum.
    min_nights: Option<u32>,
    /// Maximum nights between the endpoints; `None` for no maximum.
    max_nights: Option<u32>,
    /// Emitted when the range changes, with `(start_date, end_date)`.
    pub range_changed: Signal1<DateRange>,
    /// Emitted with the preset index when a preset chip is chosen.
    pub preset_selected: Signal1<usize>,
    /// The earliest selectable day, inclusive; `None` for no lower bound.
    minimum_date: Option<(i32, u32, u32)>,
    /// The latest selectable day, inclusive; `None` for no upper bound.
    maximum_date: Option<(i32, u32, u32)>,
    /// The day the keyboard is on, for the grid focus the arrow keys move.
    ///
    /// `None` until the keyboard or a click establishes it; Enter then applies it through the same
    /// path a click does, so the two ways of choosing cannot diverge.
    focus_date: Option<(i32, u32, u32)>,
    /// Cached numeral widths and header line height (K-P12).
    digit_metrics: crate::compat::RefCell<DigitMetrics>,
    /// The month/year header text of each panel, rebuilt only when its page changes (MISC-05).
    header_text_cache: crate::compat::RefCell<Vec<(i32, u32, String)>>,
}

/// A font's identity, as far as text measurement is concerned.
type FontKey = (u32, u16, u64);

/// The digit and header measurements one font pair produced (K-P12).
///
/// `draw_month_panel` measured each of its 42 day numerals with `measure_text`, and re-measured
/// `"M"` for the weekday header, on every frame. The numerals are the fixed strings `"1"..="31"`
/// and the height is the font's own line height, so the whole table depends only on the two fonts.
#[derive(Default)]
struct DigitMetrics {
    /// The day font the line height was measured with.
    day_key: Option<FontKey>,
    /// The line height of the day font, which every numeral shares.
    day_height: u32,
    /// The weekday-header font the height was measured with.
    header_key: Option<FontKey>,
    /// The line height of the weekday-header font.
    header_height: u32,
}

/// `(size bits, weight, family hash)` for a font.
fn font_key(font: &crate::core::Font) -> FontKey {
    let mut family_hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in font.family().as_bytes() {
        family_hash ^= u64::from(*byte);
        family_hash = family_hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    (font.size().to_bits(), font.weight(), family_hash)
}

impl DateRangePicker {
    /// Creates a new DateRangePicker widget with the given geometry.
    ///
    /// The displayed month is the current month.
    pub fn new(geometry: Rect) -> Self {
        let today = chrono::Local::now().date_naive();
        Self {
            base: BaseWidget::new(WidgetKind::DateRangePicker, geometry, "DateRangePicker"),
            display_year: today.year(),
            display_month: today.month(),
            start_date: None,
            end_date: None,
            hover_date: None,
            first_day_of_week: chrono::Weekday::Sun,
            presets: Vec::new(),
            selected_preset: None,
            preset_bar: None,
            double_month: false,
            min_nights: None,
            max_nights: None,
            range_changed: Signal1::new(),
            preset_selected: Signal1::new(),
            minimum_date: None,
            maximum_date: None,
            focus_date: None,
            digit_metrics: crate::compat::RefCell::new(DigitMetrics::default()),
            header_text_cache: crate::compat::RefCell::new(Vec::new()),
        }
    }

    /// Points the grid at the current month without changing the selection.
    pub fn set_current_month(&mut self) {
        let today = chrono::Local::now().date_naive();
        self.display_year = today.year();
        self.display_month = today.month();
        self.base.request_redraw();
    }

    /// Moves the grid to an explicit year/month without changing the selection.
    ///
    /// The public peer of the arrow buttons: a host that restores a saved view, or jumps to a
    /// reservation's month, states the page instead of stepping to it.
    pub fn set_display_page(&mut self, year: i32, month: u32) {
        let month = month.clamp(1, 12);
        if self.display_year == year && self.display_month == month {
            return;
        }
        self.display_year = year;
        self.display_month = month;
        self.base.request_redraw();
    }

    /// Moves the grid to the current month and selects today as a single-day range.
    pub fn go_to_today(&mut self) {
        let today = chrono::Local::now().date_naive();
        self.display_year = today.year();
        self.display_month = today.month();
        let date = (today.year(), today.month(), today.day());
        self.start_date = Some(date);
        self.end_date = Some(date);
        self.clear_preset_selection();
        self.emit_range_changed();
        self.base.request_redraw();
    }

    /// The preset chips.
    pub fn presets(&self) -> &[DatePreset] {
        &self.presets
    }

    /// Replaces the preset chips.
    pub fn set_presets(&mut self, presets: Vec<DatePreset>) {
        if self.presets == presets {
            return;
        }
        self.presets = presets;
        if self.selected_preset.is_some_and(|index| index >= self.presets.len()) {
            self.selected_preset = None;
        }
        self.sync_preset_bar();
        self.base.request_redraw();
    }

    /// Attaches a generic `PresetChipBar` and fills it with this picker's presets.
    ///
    /// Returns whether a bar is attached afterwards. The picker writes the list and the active
    /// chip into the bar on every change; the host connects the bar's `preset_selected` to
    /// [`Self::select_preset`] (the id is the preset index plus one), which is the same
    /// model/view split [`RangePresetBar`] uses (MISC-008).
    pub fn attach_preset_bar(
        &mut self,
        bar: Option<super::preset_chip_bar::PresetChipBar>,
    ) -> bool {
        self.preset_bar = bar;
        self.sync_preset_bar();
        self.preset_bar.is_some()
    }

    /// The attached preset bar, if any.
    pub fn preset_bar(&self) -> Option<&super::preset_chip_bar::PresetChipBar> {
        self.preset_bar.as_ref()
    }

    /// The attached preset bar for editing, if any.
    pub fn preset_bar_mut(&mut self) -> Option<&mut super::preset_chip_bar::PresetChipBar> {
        self.preset_bar.as_mut()
    }

    /// Pushes the preset list and the active chip into the attached bar.
    fn sync_preset_bar(&mut self) {
        let labels: Vec<String> = self.presets.iter().map(|preset| preset.label.clone()).collect();
        let selected = self.selected_preset;
        if let Some(bar) = self.preset_bar.as_mut() {
            bar.clear_presets();
            for (index, label) in labels.into_iter().enumerate() {
                // Chip ids are one-based: `PresetChipBar` rejects id zero as an identity.
                let _ = bar.add_preset_with_id(index as u64 + 1, label);
            }
            if let Some(index) = selected {
                bar.set_selected_silent(index as u64 + 1);
            }
        }
    }

    /// Whether a second consecutive month is painted.
    pub fn double_month(&self) -> bool {
        self.double_month
    }

    /// Enables or disables the side-by-side second month.
    pub fn set_double_month(&mut self, enabled: bool) {
        if self.double_month == enabled {
            return;
        }
        self.double_month = enabled;
        self.base.request_redraw();
    }

    /// The minimum nights constraint, if any.
    pub fn min_nights(&self) -> Option<u32> {
        self.min_nights
    }

    /// Sets the minimum nights constraint (`0` clears it).
    pub fn set_min_nights(&mut self, nights: Option<u32>) {
        let nights = nights.filter(|n| *n > 0);
        if self.min_nights == nights {
            return;
        }
        self.min_nights = nights;
        self.base.request_redraw();
    }

    /// The maximum nights constraint, if any.
    pub fn max_nights(&self) -> Option<u32> {
        self.max_nights
    }

    /// Sets the maximum nights constraint (`0` clears it).
    pub fn set_max_nights(&mut self, nights: Option<u32>) {
        let nights = nights.filter(|n| *n > 0);
        if self.max_nights == nights {
            return;
        }
        self.max_nights = nights;
        self.base.request_redraw();
    }

    /// The height the preset strip reserves.
    fn preset_height(&self) -> u32 {
        if self.presets.is_empty() {
            0
        } else {
            PRESET_HEIGHT
        }
    }

    /// The box of preset `index`, when presets are shown.
    pub fn preset_rect(&self, index: usize) -> Option<Rect> {
        if index >= self.presets.len() {
            return None;
        }
        let rect = self.geometry();
        let width = rect.width / self.presets.len() as u32;
        Some(Rect::new(
            rect.x + (index as u32 * width) as i32,
            rect.y + 2,
            width.saturating_sub(2),
            PRESET_HEIGHT.saturating_sub(4),
        ))
    }

    /// Chooses preset `index`, applying its dates and reporting the choice.
    pub fn select_preset(&mut self, index: usize) {
        let Some(preset) = self.presets.get(index).cloned() else {
            return;
        };
        self.start_date = preset.start;
        self.end_date = preset.end;
        if let Some((year, month, _)) = preset.start.or(preset.end) {
            self.display_year = year;
            self.display_month = month;
        }
        self.selected_preset = Some(index);
        self.sync_preset_bar();
        self.emit_range_changed();
        self.preset_selected.emit(index);
        self.base.request_redraw();
    }

    /// Applies `preset` to the selection.
    ///
    /// When the preset's label matches one already configured through
    /// [`DateRangePicker::set_presets`], the existing selection path runs and `preset_selected`
    /// carries that index — that is the wiring between a [`RangePresetBar`] and this picker. A
    /// preset that is not in the picker's own list is still applied (the range is the useful part)
    /// but no index can honestly be reported, so the return value is `None`.
    pub fn apply_preset(&mut self, preset: &RangePreset) -> Option<usize> {
        if let Some(index) = self.presets.iter().position(|own| own.label == preset.label) {
            self.select_preset(index);
            return Some(index);
        }
        self.start_date = Some(preset.start);
        self.end_date = Some(preset.end);
        self.display_year = preset.start.0;
        self.display_month = preset.start.1;
        self.clear_preset_selection();
        self.emit_range_changed();
        self.base.request_redraw();
        None
    }

    /// The `(year, month)` shown by panel `panel` (`0` is the primary month).
    fn panel_month(&self, panel: usize) -> (i32, u32) {
        if panel == 0 {
            return (self.display_year, self.display_month);
        }
        if self.display_month == 12 {
            (self.display_year + 1, 1)
        } else {
            (self.display_year, self.display_month + 1)
        }
    }

    /// Number of month panels painted.
    fn panel_count(&self) -> usize {
        if self.double_month {
            2
        } else {
            1
        }
    }

    /// Whether a completed range satisfies the night constraints.
    fn nights_allowed(&self, start: (i32, u32, u32), end: (i32, u32, u32)) -> bool {
        let Some(nights) = Self::nights_between(start, end) else {
            return false;
        };
        if self.min_nights.is_some_and(|minimum| nights < minimum) {
            return false;
        }
        if self.max_nights.is_some_and(|maximum| nights > maximum) {
            return false;
        }
        true
    }

    /// The night count between two dates, when both are real calendar days.
    fn nights_between(start: (i32, u32, u32), end: (i32, u32, u32)) -> Option<u32> {
        let a = chrono::NaiveDate::from_ymd_opt(start.0, start.1, start.2)?;
        let b = chrono::NaiveDate::from_ymd_opt(end.0, end.1, end.2)?;
        let nights = (b - a).num_days();
        u32::try_from(nights).ok()
    }

    /// Returns the current start date, if set.
    pub fn start_date(&self) -> Option<(i32, u32, u32)> {
        self.start_date
    }

    /// Sets the start date and emits `range_changed` if it changes.
    pub fn set_start_date(&mut self, date: Option<(i32, u32, u32)>) {
        if self.start_date != date {
            self.start_date = date;
            self.emit_range_changed();
            self.base.request_redraw();
        }
    }

    /// Returns the current end date, if set.
    pub fn end_date(&self) -> Option<(i32, u32, u32)> {
        self.end_date
    }

    /// Sets the end date and emits `range_changed` if it changes.
    pub fn set_end_date(&mut self, date: Option<(i32, u32, u32)>) {
        if self.end_date != date {
            self.end_date = date;
            self.emit_range_changed();
            self.base.request_redraw();
        }
    }

    /// The earliest selectable day, inclusive.
    pub fn minimum_date(&self) -> Option<(i32, u32, u32)> {
        self.minimum_date
    }

    /// Sets the earliest selectable day; `None` removes the bound.
    ///
    /// The bound is enforced on **both** selection paths (pointer and keyboard) through
    /// [`Self::accepts_date`], and a cell outside it paints in the disabled ink, so a user can see
    /// why a day is not selectable instead of clicking an inert cell.
    pub fn set_minimum_date(&mut self, date: Option<(i32, u32, u32)>) {
        if self.minimum_date == date {
            return;
        }
        self.minimum_date = date;
        self.base.request_redraw();
    }

    /// The latest selectable day, inclusive.
    pub fn maximum_date(&self) -> Option<(i32, u32, u32)> {
        self.maximum_date
    }

    /// Sets the latest selectable day; `None` removes the bound.
    pub fn set_maximum_date(&mut self, date: Option<(i32, u32, u32)>) {
        if self.maximum_date == date {
            return;
        }
        self.maximum_date = date;
        self.base.request_redraw();
    }

    /// The cached digit widths and line heights for the panel's two fonts (K-P12).
    ///
    /// The table costs 31 measurements when a font changes and nothing afterwards; the draw path
    /// then reads it instead of measuring each numeral.
    fn digit_metrics(
        &self,
        dow_font: &crate::core::Font,
        day_font: &crate::core::Font,
        context: &RenderContext,
    ) -> core::cell::RefMut<'_, DigitMetrics> {
        let day_key = font_key(day_font);
        let header_key = font_key(dow_font);
        let mut cache = self.digit_metrics.borrow_mut();
        if cache.day_key != Some(day_key) {
            // Every numeral's line height is the font's own, so the 42 per-cell measurements only
            // ever answered this one question.
            cache.day_height = context.measure_text("M", day_font).height;
            cache.day_key = Some(day_key);
        }
        if cache.header_key != Some(header_key) {
            cache.header_height = context.measure_text("M", dow_font).height;
            cache.header_key = Some(header_key);
        }
        core::cell::RefMut::map(cache, |cache| cache)
    }

    /// Whether `date` lies inside the configured bounds.
    pub fn accepts_date(&self, date: (i32, u32, u32)) -> bool {
        if self.minimum_date.is_some_and(|minimum| date < minimum) {
            return false;
        }
        if self.maximum_date.is_some_and(|maximum| date > maximum) {
            return false;
        }
        true
    }

    /// The day the keyboard grid focus is on, if any.
    pub fn focus_date(&self) -> Option<(i32, u32, u32)> {
        self.focus_date
    }

    /// Moves the keyboard focus by `days` and `weeks`, flipping the page when the new day is in
    /// another month.
    fn move_focus(&mut self, days: i64, weeks: i64) {
        let Some(current) = self.focus_date.or(self.start_date) else {
            // No focus and no selection: start on the first selectable day of the visible month.
            if let Some(day) = self.first_selectable_day(self.display_year, self.display_month) {
                self.focus_date = Some(day);
                self.base.request_redraw();
            }
            return;
        };
        let Some(mut naive) = chrono::NaiveDate::from_ymd_opt(current.0, current.1, current.2)
        else {
            return;
        };
        let delta = days + weeks * 7;
        if delta > 0 {
            for _ in 0..delta {
                naive = naive.succ_opt().unwrap_or(naive);
            }
        } else {
            for _ in 0..(-delta) {
                naive = naive.pred_opt().unwrap_or(naive);
            }
        }
        let next = (naive.year(), naive.month(), naive.day());
        if self.focus_date != Some(next) {
            self.focus_date = Some(next);
            // Keep the focused day on screen: the grid only shows the displayed month (or two).
            if next.0 != self.display_year || next.1 != self.display_month {
                self.display_year = next.0;
                self.display_month = next.1;
            }
            self.base.request_redraw();
        }
    }

    /// The first day of `year`/`month` that the bounds accept.
    fn first_selectable_day(&self, year: i32, month: u32) -> Option<(i32, u32, u32)> {
        for day in 1..=31 {
            let date = (year, month, day);
            if chrono::NaiveDate::from_ymd_opt(year, month, day).is_none() {
                break;
            }
            if self.accepts_date(date) {
                return Some(date);
            }
        }
        None
    }

    /// Applies a day through the same three-state path a pointer click uses.
    ///
    /// This is the shared commit the keyboard path runs, so Enter cannot mean something the click
    /// does not: first pick sets the start, second completes (or restarts) the range, third
    /// restarts it.
    fn apply_day(&mut self, clicked_date: (i32, u32, u32)) {
        if !self.accepts_date(clicked_date) {
            return;
        }
        // A manually chosen day is not a preset, so the bar's active chip is cleared before the
        // new range is reported (MISC-008).
        self.clear_preset_selection();
        match (self.start_date, self.end_date) {
            (None, _) => {
                self.start_date = Some(clicked_date);
                self.emit_range_changed();
                self.base.request_redraw();
            }
            (Some(start), None) => {
                let (new_start, new_end) = if clicked_date > start {
                    (start, clicked_date)
                } else if clicked_date < start {
                    (clicked_date, start)
                } else {
                    self.clear_selection();
                    return;
                };
                if !self.nights_allowed(new_start, new_end) {
                    return;
                }
                self.start_date = Some(new_start);
                self.end_date = Some(new_end);
                self.emit_range_changed();
                self.base.request_redraw();
            }
            (Some(_), Some(_)) => {
                self.start_date = Some(clicked_date);
                self.end_date = None;
                self.emit_range_changed();
                self.base.request_redraw();
            }
        }
    }

    /// Clears both start and end dates.
    pub fn clear_selection(&mut self) {
        if self.start_date.is_some() || self.end_date.is_some() {
            self.start_date = None;
            self.end_date = None;
            self.clear_preset_selection();
            self.emit_range_changed();
            self.base.request_redraw();
        }
    }

    /// Drops the active preset and mirrors the change into the attached bar.
    fn clear_preset_selection(&mut self) {
        if self.selected_preset.take().is_some() {
            self.sync_preset_bar();
        }
    }

    /// Navigates to the previous month.
    pub fn previous_month(&mut self) {
        if self.display_month == 1 {
            self.display_year -= 1;
            self.display_month = 12;
        } else {
            self.display_month -= 1;
        }
        self.base.request_redraw();
    }

    /// Navigates to the next month.
    pub fn next_month(&mut self) {
        if self.display_month == 12 {
            self.display_year += 1;
            self.display_month = 1;
        } else {
            self.display_month += 1;
        }
        self.base.request_redraw();
    }

    /// Returns the currently displayed year.
    pub fn display_year(&self) -> i32 {
        self.display_year
    }

    /// Returns the currently displayed month.
    pub fn display_month(&self) -> u32 {
        self.display_month
    }

    /// Returns the day the grid's week starts on.
    pub fn first_day_of_week(&self) -> chrono::Weekday {
        self.first_day_of_week
    }

    /// Sets the day the grid's week starts on.
    ///
    /// The grid offset and the weekday header rotate together, so a Monday-first locale sees
    /// Monday in the first column and January 2025's Wednesday 1st in column 2. A same-value
    /// write is a no-op, like the advanced calendar's setter.
    pub fn set_first_day_of_week(&mut self, weekday: chrono::Weekday) {
        if self.first_day_of_week == weekday {
            return;
        }
        self.first_day_of_week = weekday;
        self.base.request_redraw();
    }

    /// Emits the `range_changed` signal with current start/end.
    fn emit_range_changed(&self) {
        self.range_changed.emit((self.start_date, self.end_date));
    }

    /// Returns the number of days in panel `panel`'s month.
    fn days_in_panel_month(&self, panel: usize) -> u32 {
        let (year, month) = self.panel_month(panel);
        days_in_month(year, month)
    }

    /// Returns the day of week for the 1st of the displayed month (Sunday-first).
    #[cfg(test)]
    fn first_day_of_week_index(&self) -> u32 {
        day_of_week(self.display_year, self.display_month, 1)
    }

    /// Returns the day of week for the 1st of panel `panel`'s month (Sunday-first).
    fn first_day_of_panel(&self, panel: usize) -> u32 {
        let (year, month) = self.panel_month(panel);
        day_of_week(year, month, 1)
    }

    /// The blank cells before day 1 of panel `panel`'s month, rotated by `first_day_of_week`.
    ///
    /// `day_of_week` answers Sunday-first; a grid that starts on another weekday needs the same
    /// value measured from that day, which is the subtraction here.
    fn leading_blanks(&self, panel: usize) -> u32 {
        let first = self.first_day_of_panel(panel);
        (first + 7 - self.first_day_of_week.num_days_from_sunday()) % 7
    }

    /// The weekday names in the order the header paints them.
    fn weekday_header(&self) -> [&'static str; 7] {
        let first = self.first_day_of_week.num_days_from_sunday() as usize;
        core::array::from_fn(|index| DAY_NAMES[(first + index) % 7])
    }

    /// Returns the day, month, year for a cell in panel `panel`'s grid.
    fn date_at_panel_cell(&self, panel: usize, cell_index: usize) -> Option<(i32, u32, u32)> {
        let first_dow = self.leading_blanks(panel) as usize;
        let total_days = self.days_in_panel_month(panel) as usize;
        if cell_index < first_dow || cell_index >= first_dow + total_days {
            return None;
        }
        let day = (cell_index - first_dow + 1) as u32;
        let (year, month) = self.panel_month(panel);
        Some((year, month, day))
    }

    /// Returns the day, month, year for a given cell position in the primary grid.
    #[cfg(test)]
    fn date_at_cell(&self, cell_index: usize) -> Option<(i32, u32, u32)> {
        self.date_at_panel_cell(0, cell_index)
    }

    /// Returns the grid cell index under `pos`, or `None` when the point is outside the
    /// calendar grid.
    ///
    /// Shared by the press and hover paths so a pointer can never highlight a different
    /// cell from the one a click would select. The returned index is `(panel, cell)`.
    fn cell_at_position(&self, pos: Point) -> Option<(usize, usize)> {
        for panel in 0..self.panel_count() {
            let layout = self.grid_layout_for(panel);
            let rel_x = pos.x - layout.grid_left;
            let rel_y = pos.y - layout.grid_top;
            if rel_x < 0 || rel_y < 0 {
                continue;
            }
            let col = (rel_x as u32) / layout.column_pitch;
            let row = (rel_y as u32) / layout.row_pitch;
            if col >= 7 || row >= 6 {
                continue;
            }
            let cell_idx = (row as usize) * 7 + (col as usize);
            if cell_idx < layout.total_cells {
                return Some((panel, cell_idx));
            }
        }
        None
    }

    /// The date under `pos`, if the point is on a day cell.
    fn date_at_position(&self, pos: Point) -> Option<(i32, u32, u32)> {
        self.cell_at_position(pos).and_then(|(panel, cell)| self.date_at_panel_cell(panel, cell))
    }

    /// Checks if a date falls within the current selection range.
    ///
    /// The comparison is the date tuple's own lexicographic order, which is exactly the order the
    /// former ordinal conversion encoded for a valid calendar date. Inlining it removes the
    /// repeated multiplications this predicate ran once per cell per frame (MISC-06).
    fn is_in_range(&self, date: (i32, u32, u32)) -> bool {
        let Some(start) = self.start_date else {
            return false;
        };
        if date < start {
            return false;
        }
        if let Some(end) = self.end_date {
            date <= end
        } else {
            // If no end date, highlight just the start
            date == start
        }
    }

    /// The range a click on the hovered day would complete, if one is pending.
    ///
    /// Hovering after a start date previews the second endpoint, in either direction; once an end
    /// exists the range is committed and there is nothing left to preview. This is the semantic
    /// the hover arm's own comment promises and the draw previously did not paint (MISC-07).
    fn hover_preview_range(&self) -> Option<(Date, Date)> {
        let start = self.start_date?;
        if self.end_date.is_some() {
            return None;
        }
        let hover = self.hover_date?;
        if hover == start {
            return None;
        }
        if hover < start {
            Some((hover, start))
        } else {
            Some((start, hover))
        }
    }

    /// The header text of `panel`, formatted once per page rather than once per frame.
    fn header_text(&self, panel: usize) -> core::cell::Ref<'_, str> {
        let (year, month) = self.panel_month(panel);
        {
            let mut cache = self.header_text_cache.borrow_mut();
            if cache.len() <= panel {
                cache.resize_with(panel + 1, || (0, 0, String::new()));
            }
            let entry = &mut cache[panel];
            if entry.0 != year || entry.1 != month {
                entry.0 = year;
                entry.1 = month;
                entry.2.clear();
                let _ = core::fmt::Write::write_fmt(
                    &mut entry.2,
                    format_args!("{} {year}", month_name_for(month)),
                );
            }
        }
        core::cell::Ref::map(self.header_text_cache.borrow(), |cache| cache[panel].2.as_str())
    }

    /// The calendar grid's geometry for the current month and rectangle.
    ///
    /// Shared by the draw and the hit test, because a click must land in the cell that is
    /// visibly under it: the two used to derive the grid independently from the same
    /// literals, so any change to one silently desynchronised the other.
    ///
    /// The cell size is derived from the room the grid actually has rather than being a
    /// fixed 28 px. A month can need five or six week rows, so a fixed cell walked the last
    /// row — and the labels in it — past the control's bottom edge, where the raster
    /// backends clipped it and the SVG snapshot showed it leaving the picture. The nominal
    /// size is kept as the *maximum*, so a roomy control is unchanged.
    #[cfg(test)]
    fn grid_layout(&self) -> GridLayout {
        self.grid_layout_for(0)
    }

    /// The grid geometry for one month panel.
    fn grid_layout_for(&self, panel: usize) -> GridLayout {
        let rect = self.geometry();
        let panels = self.panel_count() as u32;
        let panel_width = rect.width / panels.max(1);
        let panel_x = rect.x + (panel as u32 * panel_width) as i32;
        let total_days = self.days_in_panel_month(panel) as usize;
        let first_dow = self.leading_blanks(panel) as usize;
        let rows = ((first_dow + total_days).div_ceil(7) as u32).max(1);
        let grid_left = panel_x + GRID_LEFT_INSET;
        let preset = self.preset_height() as i32;
        let grid_top = rect.y + preset + HEADER_HEIGHT as i32 + DAY_HEADER_HEIGHT as i32;
        let available_height = (rect.y + rect.height as i32 - grid_top).max(1) as u32;
        let gaps = CELL_SPACING * (rows - 1);
        let cell_size = (available_height.saturating_sub(gaps) / rows).clamp(1, NOMINAL_CELL);
        let available_width =
            (panel_x + panel_width as i32 - grid_left - GRID_RIGHT_INSET).max(1) as u32;
        let column_gaps = CELL_SPACING * 6;
        let cell_width = (available_width.saturating_sub(column_gaps) / 7).clamp(1, NOMINAL_CELL);
        GridLayout {
            grid_left,
            grid_top,
            cell_size,
            cell_width,
            row_pitch: cell_size + CELL_SPACING,
            column_pitch: cell_width + CELL_SPACING,
            total_cells: first_dow + total_days,
        }
    }
}

/// Reserved height of the preset chip strip.
const PRESET_HEIGHT: u32 = 22;
/// Reserved height of the month/year header band.
const HEADER_HEIGHT: u32 = 40;
/// Reserved height of the weekday row.
const DAY_HEADER_HEIGHT: u32 = 20;
/// Left inset of the grid from the control's edge.
const GRID_LEFT_INSET: i32 = 4;
/// Right inset of the grid from the control's edge.
const GRID_RIGHT_INSET: i32 = 4;
/// Gap between calendar cells.
const CELL_SPACING: u32 = 2;
/// The largest a calendar cell is allowed to be.
const NOMINAL_CELL: u32 = 28;

/// The month name for `month` (`1`–`12`), clamped into the table.
fn month_name_for(month: u32) -> &'static str {
    let idx = (month as usize).saturating_sub(1);
    MONTH_NAMES[idx.min(11)]
}

/// The calendar grid's resolved geometry.
struct GridLayout {
    /// Absolute x of the first column.
    grid_left: i32,
    /// Absolute y of the first week row.
    grid_top: i32,
    /// Edge length of one day cell (the row height limit).
    cell_size: u32,
    /// Width of one day cell, from the grid's width.
    cell_width: u32,
    /// Row pitch: the cell height plus its gap.
    row_pitch: u32,
    /// Column pitch: the cell width plus its gap.
    column_pitch: u32,
    /// Cells to draw, including the leading blanks of the first week.
    total_cells: usize,
}

/// Formats a date as an ISO `YYYY-MM-DD` string.
///
/// Thin wrapper over [`date_utils::format_iso_date`] so this module can keep
/// passing its `(year, month, day)` tuples around as one value.
fn format_iso_date(date: (i32, u32, u32)) -> String {
    super::date_utils::format_iso_date(date.0, date.1, date.2)
}

impl Widget for DateRangePicker {
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
        crate::core::Size::new(500, 300)
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
            "range_changed" => {
                Some(EventSignalRef::mapped("range_changed", &self.range_changed, |v| {
                    CapabilityValue::String(format!("{v:?}"))
                }))
            }
            "preset_selected" => {
                Some(EventSignalRef::mapped("preset_selected", &self.preset_selected, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
            _ => None,
        }
    }
}

/// `DateRangePicker`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_dialog.in.rs` / `access_write_dialog.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. Each endpoint reports the
/// widget's real date as an ISO `YYYY-MM-DD` string, or `Null` while that endpoint
/// is unset, rather than the legacy hardcoded blank. A write parses the same format;
/// malformed text is reported as [`CapabilityAccessError::TypeMismatch`] instead of
/// quietly ignoring the write, and `Null` clears the endpoint.
impl WidgetProperties for DateRangePicker {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "start_date" => Ok(match self.start_date() {
                Some(date) => CapabilityValue::String(format_iso_date(date)),
                None => CapabilityValue::Null,
            }),
            "end_date" => Ok(match self.end_date() {
                Some(date) => CapabilityValue::String(format_iso_date(date)),
                None => CapabilityValue::Null,
            }),
            "presets" => Ok(CapabilityValue::Tuple(
                self.presets
                    .iter()
                    .map(|preset| {
                        CapabilityValue::String(format!(
                            "{}|{}|{}",
                            preset.label,
                            preset.start.map(format_iso_date).unwrap_or_default(),
                            preset.end.map(format_iso_date).unwrap_or_default()
                        ))
                    })
                    .collect(),
            )),
            "double_month" => Ok(CapabilityValue::Bool(self.double_month)),
            "min_nights" => Ok(CapabilityValue::UInt(self.min_nights.unwrap_or(0) as u64)),
            "max_nights" => Ok(CapabilityValue::UInt(self.max_nights.unwrap_or(0) as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "start_date" => {
                self.set_start_date(expect_optional_iso_date(value)?);
                Ok(())
            }
            "end_date" => {
                self.set_end_date(expect_optional_iso_date(value)?);
                Ok(())
            }
            "presets" => {
                let items = expect_string_list(value)?;
                let mut presets = Vec::new();
                for entry in items {
                    let mut parts = entry.splitn(3, '|');
                    let label = parts.next().unwrap_or_default().to_string();
                    let start =
                        parts.next().filter(|text| !text.is_empty()).and_then(parse_iso_date);
                    let end = parts.next().filter(|text| !text.is_empty()).and_then(parse_iso_date);
                    presets.push(DatePreset { label, start, end });
                }
                self.set_presets(presets);
                Ok(())
            }
            "double_month" => {
                let enabled = match value {
                    CapabilityValue::Bool(flag) => flag,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_double_month(enabled);
                Ok(())
            }
            "min_nights" => {
                let nights = match value {
                    CapabilityValue::UInt(value) => value,
                    CapabilityValue::Int(value) if value >= 0 => value as u64,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_min_nights(u32::try_from(nights).ok());
                Ok(())
            }
            "max_nights" => {
                let nights = match value {
                    CapabilityValue::UInt(value) => value,
                    CapabilityValue::Int(value) if value >= 0 => value as u64,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_max_nights(u32::try_from(nights).ok());
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "start_date",
            "end_date",
            "presets",
            "double_month",
            "min_nights",
            "max_nights",
            BASE_PROPERTY_NAMES
        ]
    }
}

/// Reads an optional ISO date: `Null` clears, a string parses, anything else is a
/// type mismatch.
fn expect_optional_iso_date(
    value: CapabilityValue,
) -> Result<Option<(i32, u32, u32)>, CapabilityAccessError> {
    match value {
        CapabilityValue::Null => Ok(None),
        other => {
            let text = expect_string(other)?;
            parse_iso_date(&text).map(Some).ok_or(CapabilityAccessError::TypeMismatch)
        }
    }
}

impl Draw for DateRangePicker {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }

        let is_enabled = self.base.is_enabled();

        // Chrome colours resolve explicit style first, then the theme's resolved
        // style for this control, and only then a literal. The theme step is what
        // makes an appearance switch visible; previously the calendar surface, its
        // text, and the range highlight were all hardcoded, so light and dark
        // rendered identically.
        //
        // `resolved_theme_style` takes and releases the global manager's lock
        // internally, so no guard is held across the draw (the mutex is not
        // re-entrant).
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("date_range_picker");
        let surface = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let text_color = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        // A disabled calendar is the same surface, faded: derived from the resolved
        // colour rather than a second literal so it still follows the appearance.
        let bg_color =
            if is_enabled { surface.with_alpha(255) } else { surface.blend(&text_color, 0.1) };
        // The selected range is a state, so it reads the theme's selection accent
        // rather than a fixed blue. `primary` is the theme's brand/action colour.
        let accent = theme
            .as_ref()
            .and_then(|_| crate::style::semantic_color(crate::style::SemanticColor::Info))
            .unwrap_or(Color::BLUE);

        context.face_with_gradient_corners(
            rect,
            bg_color,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().resolved_corner_radii().to_array(),
            Color::BLACK,
        );

        // ── Preset chips ──
        let preset_font = role_font(style, 10.0);
        if !self.presets.is_empty() {
            for index in 0..self.presets.len() {
                let Some(chip) = self.preset_rect(index) else {
                    continue;
                };
                context.fill_rounded_rect(chip, 4, surface.blend(&text_color, 0.08));
                context.draw_text_fitted_cow(
                    chip,
                    self.presets[index].label.as_str(),
                    &preset_font,
                    text_color,
                    HorizontalAlignment::Center,
                );
            }
        }

        // ── Month/Year headers ──
        let header_top = rect.y + self.preset_height() as i32;
        let header_font = role_font(style, 14.0).with_bold(true);
        let panels = self.panel_count();
        let panel_width = rect.width / panels.max(1) as u32;
        for panel in 0..panels {
            let panel_x = rect.x + (panel as u32 * panel_width) as i32;
            let header_band =
                Rect::new(panel_x + 20, header_top + 6, panel_width.saturating_sub(40), 18);
            let header_text = self.header_text(panel);
            context.draw_text_fitted_cow(
                header_band,
                &header_text,
                &header_font,
                text_color,
                HorizontalAlignment::Center,
            );
        }

        // Navigation arrows, each in its own strip at the band's ends so they cannot collide
        // with a long month name. Two icon outlines rather than triangle glyphs: no bundled face
        // covers U+25C0/U+25B6, so those drew as 8x8 bitmap blocks.
        let nav_font = role_font(style, 12.0).with_bold(true);
        let nav_height = context.measure_text("M", &nav_font).height;
        let arrow_side = nav_height.max(1);
        crate::widget::draw_icon_at(
            context,
            Rect::new(rect.x + 6, header_top + 8, arrow_side, arrow_side),
            text_color,
            IconName::ChevronLeft,
        );
        crate::widget::draw_icon_at(
            context,
            Rect::new(rect.x + rect.width as i32 - 20, header_top + 8, arrow_side, arrow_side),
            text_color,
            IconName::ChevronRight,
        );

        // ── Day-of-week headers and grids, one block per month panel ──
        let dow_font = role_font(style, 9.0);
        let day_font = role_font(style, 10.0);
        for panel in 0..panels {
            draw_month_panel(
                self, context, panel, &dow_font, &day_font, surface, text_color, accent, is_enabled,
            );
        }
    }
}

/// Paints one month panel's weekday header and day grid.
#[allow(clippy::too_many_arguments)]
fn draw_month_panel(
    picker: &DateRangePicker,
    context: &mut RenderContext,
    panel: usize,
    dow_font: &crate::core::Font,
    day_font: &crate::core::Font,
    surface: Color,
    text_color: Color,
    accent: Color,
    is_enabled: bool,
) {
    let layout = picker.grid_layout_for(panel);
    let grid_left = layout.grid_left;
    let grid_top = layout.grid_top;
    let cell_size = layout.cell_size;
    let cell_width = layout.cell_width;
    let row_pitch = layout.row_pitch;
    let column_pitch = layout.column_pitch;
    let dow_color = surface.blend(&text_color, 0.55);
    // Both measurements come from the cached table: the header line height and the widths of
    // `"1"..="31"` depend only on the fonts (K-P12), and the panel measured them per frame — 42
    // day numerals plus the header, twice over for a two-month layout.
    let metrics = picker.digit_metrics(dow_font, day_font, context);
    let dow_height = metrics.header_height;
    let margin = TEXT_FIT_MARGIN as i32;
    for (i, day_name) in picker.weekday_header().iter().enumerate() {
        let cell_x = grid_left + (i as u32 * column_pitch) as i32;
        let cell_y = grid_top - DAY_HEADER_HEIGHT as i32;
        context.draw_text_fitted_cow(
            Rect::new(cell_x - margin, cell_y + 4, cell_width + 2 * margin as u32, dow_height),
            day_name,
            dow_font,
            dow_color,
            HorizontalAlignment::Center,
        );
    }

    let mut day_buf = [0u8; 2];
    let focus_date = picker.focus_date;
    let preview = picker.hover_preview_range();
    for cell_idx in 0..layout.total_cells {
        let Some(date) = picker.date_at_panel_cell(panel, cell_idx) else {
            continue;
        };
        let day = date.2;
        let day_text = if day < 10 {
            day_buf[0] = b'0' + day as u8;
            core::str::from_utf8(&day_buf[..1]).unwrap_or("")
        } else {
            day_buf[0] = b'0' + (day / 10) as u8;
            day_buf[1] = b'0' + (day % 10) as u8;
            core::str::from_utf8(&day_buf[..2]).unwrap_or("")
        };
        let row = cell_idx / 7;
        let col = cell_idx % 7;
        let cell_x = grid_left + (col as u32 * column_pitch) as i32;
        let cell_y = grid_top + (row as u32 * row_pitch) as i32;
        let cell_rect = Rect::new(cell_x, cell_y, cell_width, cell_size);

        let is_start = picker.start_date == Some(date);
        let is_end = picker.end_date == Some(date);
        let is_today = is_start || is_end;
        let in_range = picker.is_in_range(date);
        let is_hover = picker.hover_date == Some(date);
        let in_preview = preview.is_some_and(|(low, high)| date >= low && date <= high);
        let is_focused = focus_date == Some(date);
        // A day outside the configured bounds is not selectable, and it says so in the picture
        // (MISC-08): the fill is skipped and the ink is the disabled tone, so a user can see why
        // the click would do nothing.
        let allowed = picker.accepts_date(date);

        if allowed && in_range && is_enabled {
            let range_color =
                if is_start || is_end { accent.with_alpha(200) } else { accent.with_alpha(60) };
            context.fill_rounded_rect(cell_rect, 4, range_color);
        } else if allowed && is_hover && is_enabled {
            context.fill_rounded_rect(cell_rect, 4, surface.blend(&text_color, 0.16));
        } else if allowed && in_preview && is_enabled {
            // The pending range from the start date to the hovered day, painted more faintly than
            // a committed range so the two cannot be mistaken for each other (MISC-07).
            context.fill_rounded_rect(cell_rect, 4, accent.with_alpha(40));
        } else if allowed && is_today && is_enabled {
            let today_color = if is_start || is_end {
                accent.with_alpha(200)
            } else {
                surface.blend(&text_color, 0.14)
            };
            context.fill_rounded_rect(cell_rect, 4, today_color);
        }

        // The keyboard focus is a ring inside the cell, so it cannot be confused with the range
        // fill it may sit on top of.
        if is_focused && is_enabled {
            context.draw_rect(
                Rect::new(
                    cell_x + 1,
                    cell_y + 1,
                    cell_width.saturating_sub(2),
                    cell_size.saturating_sub(2),
                ),
                accent,
            );
        }

        let day_height = metrics.day_height;
        let day_y = cell_y + (cell_size as i32 - day_height as i32) / 2;
        let day_color = if !is_enabled || !allowed {
            text_color.blend(&surface, 0.55)
        } else if is_start || is_end {
            surface
        } else if in_range || in_preview {
            accent
        } else {
            text_color
        };
        context.draw_text_fitted_cow(
            Rect::new(
                cell_x - TEXT_FIT_MARGIN as i32,
                day_y,
                cell_width + 2 * TEXT_FIT_MARGIN,
                day_height,
            ),
            day_text,
            day_font,
            day_color,
            HorizontalAlignment::Center,
        );
    }
}

impl EventHandler for DateRangePicker {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        match event {
            // Hovering a day cell previews the range the click would complete. Before this
            // arm `hover_date` was only ever `None`, so the draw's highlight branch was
            // unreachable and the control gave no pointer feedback at all.
            Event::MouseMove { pos } => {
                let hovered = self.date_at_position(*pos);
                if self.hover_date != hovered {
                    self.hover_date = hovered;
                    self.base.request_redraw();
                }
                return;
            }
            Event::MouseLeave { .. } => {
                if self.hover_date.is_some() {
                    self.hover_date = None;
                    self.base.request_redraw();
                }
                return;
            }
            Event::KeyPress { key, modifiers } => {
                // Month navigation from the keyboard: PageUp/PageDown step the visible
                // month, and Ctrl+Left/Right do the same for a keyboard user coming from a
                // text-field convention. Neither arm existed, so the grid could only be
                // navigated with the pointer.
                let pressed = crate::shortcut::Key::from_key_code(*key);
                let modifiers = crate::shortcut::Modifiers::from_event_bits(*modifiers);
                match pressed {
                    Some(crate::shortcut::Key::PageUp) => {
                        self.previous_month();
                        return;
                    }
                    Some(crate::shortcut::Key::PageDown) => {
                        self.next_month();
                        return;
                    }
                    Some(crate::shortcut::Key::Left)
                        if modifiers.contains(crate::shortcut::Modifiers::CTRL) =>
                    {
                        self.previous_month();
                        return;
                    }
                    Some(crate::shortcut::Key::Right)
                        if modifiers.contains(crate::shortcut::Modifiers::CTRL) =>
                    {
                        self.next_month();
                        return;
                    }
                    // The grid's own keyboard path (MISC-07): arrows move a focus day (crossing
                    // a month flips the page), Enter applies it through the click's own commit,
                    // and Escape cancels an in-progress range.
                    Some(crate::shortcut::Key::Left) => {
                        self.move_focus(-1, 0);
                        return;
                    }
                    Some(crate::shortcut::Key::Right) => {
                        self.move_focus(1, 0);
                        return;
                    }
                    Some(crate::shortcut::Key::Up) => {
                        self.move_focus(0, -1);
                        return;
                    }
                    Some(crate::shortcut::Key::Down) => {
                        self.move_focus(0, 1);
                        return;
                    }
                    Some(crate::shortcut::Key::Enter) | Some(crate::shortcut::Key::Space) => {
                        if let Some(day) = self.focus_date {
                            self.apply_day(day);
                        }
                        return;
                    }
                    Some(crate::shortcut::Key::Escape) => {
                        // An in-progress selection (start with no end) is what Escape cancels;
                        // a completed range is left alone, because losing a confirmed range to a
                        // stray key is not what "cancel" means to a user.
                        if self.start_date.is_some() && self.end_date.is_none() {
                            self.clear_selection();
                        }
                        return;
                    }
                    _ => {}
                }
            }
            _ => {}
        }

        if let Event::MousePress { pos, button: 1, .. } = event {
            let rect = self.geometry();
            if !rect.contains_point(*pos) {
                return;
            }

            // Preset chips take the press before the calendar behind them.
            for index in 0..self.presets.len() {
                if self.preset_rect(index).is_some_and(|chip| chip.contains_point(*pos)) {
                    self.select_preset(index);
                    return;
                }
            }

            // The header strips move down by the preset row, matching the paint.
            let header_top = rect.y + self.preset_height() as i32;
            let header_rect_left = Rect::new(rect.x + 4, header_top + 4, 20, HEADER_HEIGHT);
            let header_rect_right =
                Rect::new(rect.x + rect.width as i32 - 24, header_top + 4, 20, HEADER_HEIGHT);
            if header_rect_left.contains_point(*pos) {
                self.previous_month();
                return;
            }
            if header_rect_right.contains_point(*pos) {
                self.next_month();
                return;
            }

            let Some((panel, cell_idx)) = self.cell_at_position(*pos) else {
                // Click outside the grid — clear if only start is selected.
                if self.start_date.is_some() && self.end_date.is_none() {
                    self.clear_selection();
                }
                return;
            };
            let Some(clicked_date) = self.date_at_panel_cell(panel, cell_idx) else {
                // A click in the blank tail of the last week belongs to no date; treating it
                // as a selection would invent one.
                return;
            };

            // The click also becomes the keyboard focus, so Enter after a click repeats it and the
            // arrows continue from where the pointer left off.
            self.focus_date = Some(clicked_date);
            self.apply_day(clicked_date);
        }
    }
}

/// A calendar date as `(year, month, day)`, the tuple every date API in this module uses.
pub type Date = (i32, u32, u32);

/// One named shortcut range offered by a [`RangePresetBar`].
///
/// Unlike [`DatePreset`] — which the picker owns and may use as a partial anchor — a range preset
/// is always a complete range, because a bar's whole job is to apply both ends at once.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RangePreset {
    /// The chip's caption, and the key [`RangePresetBar::select`] matches.
    pub label: String,
    /// Inclusive first day.
    pub start: Date,
    /// Inclusive last day.
    pub end: Date,
}

impl RangePreset {
    /// Creates a preset from its caption and endpoints.
    pub fn new(label: impl Into<String>, start: Date, end: Date) -> Self {
        Self { label: label.into(), start, end }
    }

    /// Converts a picker-owned [`DatePreset`] that names both ends.
    ///
    /// Returns `None` for a partial preset: a bar chip cannot stand for "start only", and
    /// inventing an end would make the chip's caption a lie.
    pub fn from_date_preset(preset: &DatePreset) -> Option<Self> {
        Some(Self { label: preset.label.clone(), start: preset.start?, end: preset.end? })
    }

    /// The published property spelling: `label|start|end` with ISO dates.
    pub fn to_token(&self) -> String {
        format!("{}|{}|{}", self.label, format_iso_date(self.start), format_iso_date(self.end))
    }

    /// Parses [`Self::to_token`].
    pub fn from_token(token: &str) -> Option<Self> {
        let mut parts = token.splitn(3, '|');
        let label = parts.next()?.to_string();
        let start = parse_iso_date(parts.next()?)?;
        let end = parse_iso_date(parts.next()?)?;
        Some(Self { label, start, end })
    }
}

/// Whether a preset bar lays its chips out across or down.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PresetBarOrientation {
    /// Chips share one row; the default.
    #[default]
    Horizontal,
    /// Chips share one column.
    Vertical,
}

impl PresetBarOrientation {
    /// The token spelling published as the `orientation` property.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Horizontal => "horizontal",
            Self::Vertical => "vertical",
        }
    }

    /// Parses a published token spelling.
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "horizontal" | "row" => Some(Self::Horizontal),
            "vertical" | "column" => Some(Self::Vertical),
            _ => None,
        }
    }
}

/// Height of one horizontal chip, and of a compact one.
const CHIP_HEIGHT: u32 = 24;
const COMPACT_CHIP_HEIGHT: u32 = 18;

/// A strip of named range shortcuts that reports the range a reader picked.
///
/// The bar is the companion [`DateRangePicker`] lacked rather than a second calendar: it paints
/// the presets, owns which one is active, and emits the range as a `(start, end)` pair. A host
/// feeds that pair to [`DateRangePicker::apply_preset`], which is the same value type, so the two
/// controls cannot disagree about what "last 7 days" means.
pub struct RangePresetBar {
    base: BaseWidget,
    presets: Vec<RangePreset>,
    orientation: PresetBarOrientation,
    compact: bool,
    /// Index of the active chip, once one has been chosen.
    selected: Option<usize>,
    /// Index of the chip the pointer is over, for the hover wash.
    hovered: Option<usize>,
    /// Index keyboard navigation points at, whether or not it is selected yet.
    cursor: usize,
    /// Emitted with the applied range whenever a chip is chosen.
    pub preset_selected: Signal1<(Date, Date)>,
}

impl RangePresetBar {
    /// Creates an empty horizontal bar.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::RangePresetBar, geometry, "RangePresetBar"),
            presets: Vec::new(),
            orientation: PresetBarOrientation::Horizontal,
            compact: false,
            selected: None,
            hovered: None,
            cursor: 0,
            preset_selected: Signal1::new(),
        }
    }

    /// The configured presets, in display order.
    pub fn presets(&self) -> &[RangePreset] {
        &self.presets
    }

    /// Replaces the presets.
    ///
    /// An active chip is kept when its label still exists, so a refreshed preset list (the
    /// classic "last 7 days" sliding forward overnight) keeps the reader's choice rather than
    /// clearing it.
    pub fn set_presets(&mut self, presets: Vec<RangePreset>) {
        let active_label = self
            .selected
            .and_then(|index| self.presets.get(index))
            .map(|preset| preset.label.clone());
        self.presets = presets;
        self.selected = active_label
            .and_then(|label| self.presets.iter().position(|preset| preset.label == label));
        self.cursor = self.cursor.min(self.presets.len().saturating_sub(1));
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The layout direction.
    pub fn orientation(&self) -> PresetBarOrientation {
        self.orientation
    }

    /// Sets the layout direction.
    pub fn set_orientation(&mut self, orientation: PresetBarOrientation) {
        if self.orientation == orientation {
            return;
        }
        self.orientation = orientation;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Whether chips are drawn in their compact size.
    pub fn compact(&self) -> bool {
        self.compact
    }

    /// Enables or disables the compact chip size.
    pub fn set_compact(&mut self, compact: bool) {
        if self.compact == compact {
            return;
        }
        self.compact = compact;
        self.base.request_redraw();
    }

    /// The active preset's label, when one has been chosen.
    pub fn selected_label(&self) -> Option<&str> {
        self.selected.and_then(|index| self.presets.get(index)).map(|preset| preset.label.as_str())
    }

    /// The active preset's range, when one has been chosen.
    pub fn selected_range(&self) -> Option<(Date, Date)> {
        self.selected
            .and_then(|index| self.presets.get(index))
            .map(|preset| (preset.start, preset.end))
    }

    /// Chooses the preset whose label is `label`, emitting its range.
    ///
    /// Returns `false` for an unknown label instead of silently choosing the first chip: a host
    /// that mistyped a caption must find out, not discover later that the picker was showing a
    /// different range from the one it asked for.
    pub fn select(&mut self, label: &str) -> bool {
        let Some(index) = self.presets.iter().position(|preset| preset.label == label) else {
            return false;
        };
        self.select_index(index);
        true
    }

    /// Chooses preset `index`, emitting its range.
    pub fn select_index(&mut self, index: usize) {
        let Some(preset) = self.presets.get(index) else {
            return;
        };
        let range = (preset.start, preset.end);
        let changed = self.selected != Some(index);
        self.selected = Some(index);
        self.cursor = index;
        if changed {
            self.preset_selected.emit(range);
        }
        self.base.request_redraw();
    }

    /// The painted box of chip `index`.
    ///
    /// Draw and hit-testing both call this, so a click can never choose a chip the reader is not
    /// pointing at.
    pub fn chip_rect(&self, index: usize) -> Option<Rect> {
        if index >= self.presets.len() {
            return None;
        }
        let rect = self.geometry();
        let count = self.presets.len() as u32;
        match self.orientation {
            PresetBarOrientation::Horizontal => {
                let width = rect.width / count.max(1);
                let height = if self.compact {
                    COMPACT_CHIP_HEIGHT.min(rect.height)
                } else {
                    CHIP_HEIGHT.min(rect.height)
                };
                Some(Rect::new(
                    rect.x + (index as u32 * width) as i32,
                    rect.y,
                    width.saturating_sub(2),
                    height,
                ))
            }
            PresetBarOrientation::Vertical => {
                let height = rect.height / count.max(1);
                Some(Rect::new(
                    rect.x,
                    rect.y + (index as u32 * height) as i32,
                    rect.width,
                    height.saturating_sub(2),
                ))
            }
        }
    }

    /// The chip under `pos`.
    fn chip_at(&self, pos: Point) -> Option<usize> {
        (0..self.presets.len())
            .find(|index| self.chip_rect(*index).is_some_and(|chip| chip.contains_point(pos)))
    }

    /// Moves keyboard navigation by `delta` and selects the reached chip.
    fn move_cursor(&mut self, delta: isize) {
        if self.presets.is_empty() {
            return;
        }
        let last = self.presets.len() - 1;
        let next = (self.cursor as isize + delta).clamp(0, last as isize) as usize;
        self.select_index(next);
    }
}

impl Widget for RangePresetBar {
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
        match self.orientation {
            PresetBarOrientation::Horizontal => crate::core::Size::new(320, CHIP_HEIGHT),
            PresetBarOrientation::Vertical => crate::core::Size::new(160, 160),
        }
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves `preset_selected`; see `DiffViewer`'s note for why the arms are explicit.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "preset_selected" => {
                Some(EventSignalRef::mapped("preset_selected", &self.preset_selected, |range| {
                    CapabilityValue::Tuple(crate::compat::Vec::from([
                        CapabilityValue::String(format_iso_date(range.0)),
                        CapabilityValue::String(format_iso_date(range.1)),
                    ]))
                }))
            }
            _ => None,
        }
    }
}

impl WidgetProperties for RangePresetBar {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "presets" => Ok(CapabilityValue::Tuple(
                self.presets
                    .iter()
                    .map(|preset| CapabilityValue::String(preset.to_token()))
                    .collect(),
            )),
            "orientation" => Ok(CapabilityValue::String(self.orientation.as_str().to_string())),
            "compact" => Ok(CapabilityValue::Bool(self.compact)),
            "selected_label" => Ok(match self.selected_label() {
                Some(label) => CapabilityValue::String(label.to_string()),
                None => CapabilityValue::Null,
            }),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "presets" => {
                let items = expect_string_list(value)?;
                let mut presets = Vec::with_capacity(items.len());
                for item in items {
                    let preset = RangePreset::from_token(&item)
                        .ok_or(CapabilityAccessError::TypeMismatch)?;
                    presets.push(preset);
                }
                self.set_presets(presets);
                Ok(())
            }
            "orientation" => {
                let token = expect_string(value)?;
                let orientation = PresetBarOrientation::from_token(&token)
                    .ok_or(CapabilityAccessError::TypeMismatch)?;
                self.set_orientation(orientation);
                Ok(())
            }
            "compact" => {
                let compact = match value {
                    CapabilityValue::Bool(flag) => flag,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_compact(compact);
                Ok(())
            }
            "selected_label" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "presets",
            "orientation",
            "compact",
            "selected_label",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl EventHandler for RangePresetBar {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button: 1, .. } => {
                if let Some(index) = self.chip_at(*pos) {
                    self.select_index(index);
                }
            }
            Event::MouseMove { pos } => {
                let hovered = self.chip_at(*pos);
                if hovered != self.hovered {
                    self.hovered = hovered;
                    self.base.request_redraw();
                }
            }
            Event::KeyPress { key, .. } => match *key {
                37 | 38 => self.move_cursor(-1),
                39 | 40 => self.move_cursor(1),
                13 if !self.presets.is_empty() => {
                    self.select_index(self.cursor.min(self.presets.len() - 1));
                }
                _ => { /* Other keys are not relevant */ }
            },
            _ => {}
        }
    }
}

impl Draw for RangePresetBar {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("date_range_picker");
        let surface = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let text_color = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        let accent =
            crate::style::semantic_color(crate::style::SemanticColor::Info).unwrap_or(Color::BLUE);
        let font = role_font(style, 10.0);

        context.face_with_gradient_corners(
            rect,
            surface,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().resolved_corner_radii().to_array(),
            Color::BLACK,
        );

        for index in 0..self.presets.len() {
            let Some(chip) = self.chip_rect(index) else {
                continue;
            };
            let active = self.selected == Some(index);
            let fill = if active {
                accent.blend(&surface, 0.25)
            } else if self.hovered == Some(index) {
                surface.blend(&text_color, 0.08)
            } else {
                surface.blend(&text_color, 0.03)
            };
            context.fill_rounded_rect(chip, 4, fill);
            if active {
                crate::widget::draw::stroke_surface_border(context, chip, [4, 4, 4, 4], accent, 1);
            }
            let label = crate::widget::text_utils::elide_end(
                &self.presets[index].label,
                (chip.width as usize / 7).max(1),
            );
            context.draw_text(
                Point::new(chip.x + 6, chip.y + 4),
                &label,
                &font,
                if active { text_color } else { text_color.blend(&surface, 0.25) },
                HorizontalAlignment::Left,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::svg::render_to_svg;

    fn make_picker() -> DateRangePicker {
        let mut picker = DateRangePicker::new(Rect::new(0, 0, 240, 260));
        // The default now follows the wall clock (MISC-02); the geometry tests below pin the
        // month so their cell arithmetic stays deterministic.
        picker.display_year = 2025;
        picker.display_month = 1;
        picker
    }

    #[test]
    fn date_range_picker_default_creation() {
        let picker = DateRangePicker::new(Rect::new(0, 0, 240, 260));
        let today = chrono::Local::now().date_naive();
        assert_eq!(picker.kind(), WidgetKind::DateRangePicker);
        assert!(picker.start_date().is_none());
        assert!(picker.end_date().is_none());
        assert_eq!(picker.display_year(), today.year());
        assert_eq!(picker.display_month(), today.month());
    }

    #[test]
    fn date_range_picker_set_start_end() {
        let mut picker = make_picker();
        picker.set_start_date(Some((2025, 1, 10)));
        assert_eq!(picker.start_date(), Some((2025, 1, 10)));

        picker.set_end_date(Some((2025, 1, 20)));
        assert_eq!(picker.end_date(), Some((2025, 1, 20)));
    }

    #[test]
    fn date_range_picker_clear_selection() {
        let mut picker = make_picker();
        picker.set_start_date(Some((2025, 1, 10)));
        picker.set_end_date(Some((2025, 1, 20)));
        assert!(picker.start_date().is_some());
        assert!(picker.end_date().is_some());

        picker.clear_selection();
        assert!(picker.start_date().is_none());
        assert!(picker.end_date().is_none());
    }

    #[test]
    fn date_range_picker_navigation() {
        let mut picker = make_picker();
        picker.set_current_month();
        let today = chrono::Local::now().date_naive();
        assert_eq!(picker.display_month(), today.month());

        picker.previous_month();
        let (year, month) = if today.month() == 1 {
            (today.year() - 1, 12)
        } else {
            (today.year(), today.month() - 1)
        };
        assert_eq!(picker.display_month(), month);
        assert_eq!(picker.display_year(), year);

        picker.next_month();
        assert_eq!(picker.display_month(), today.month());
        assert_eq!(picker.display_year(), today.year());
    }

    #[test]
    fn date_range_picker_svg_output() {
        let mut picker = make_picker();
        picker.set_start_date(Some((2025, 1, 5)));
        picker.set_end_date(Some((2025, 1, 15)));
        let svg = render_to_svg(&mut picker);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }

    /// Centre of the cell at `col`/`row` in the picker's own grid geometry.
    fn cell_centre(picker: &DateRangePicker, col: u32, row: u32) -> Point {
        let layout = picker.grid_layout();
        Point::new(
            layout.grid_left + (col * layout.column_pitch) as i32 + layout.cell_width as i32 / 2,
            layout.grid_top + (row * layout.row_pitch) as i32 + layout.cell_size as i32 / 2,
        )
    }

    #[test]
    fn january_2025_starts_in_the_wednesday_column() {
        // The 1st of January 2025 is a Wednesday. Sunday-first numbering therefore puts it in
        // column 3, with three leading blanks in the first week. The grid used to start it in
        // column 4 (Zeller's Saturday-based value used verbatim), shifting every day one cell
        // to the right of the weekday printed above it.
        assert_eq!(day_of_week(2025, 1, 1), 3);
        let picker = make_picker();
        assert_eq!(picker.first_day_of_week_index(), 3);
        assert_eq!(picker.date_at_cell(2), None);
        assert_eq!(picker.date_at_cell(3), Some((2025, 1, 1)));
        assert_eq!(picker.date_at_cell(9), Some((2025, 1, 7)));
    }

    /// A Monday-first week rotates both the grid offset and the header, and a click still lands
    /// on the day painted in that cell (M-003).
    #[test]
    fn a_monday_first_week_rotates_the_grid_and_the_header() {
        let mut picker = make_picker();
        assert_eq!(picker.first_day_of_week(), chrono::Weekday::Sun);
        assert_eq!(picker.date_at_cell(3), Some((2025, 1, 1)));

        picker.set_first_day_of_week(chrono::Weekday::Mon);
        assert_eq!(picker.first_day_of_week(), chrono::Weekday::Mon);
        assert_eq!(picker.leading_blanks(0), 2, "Wednesday is column 2 of a Monday-first week");
        assert_eq!(picker.date_at_cell(1), None);
        assert_eq!(picker.date_at_cell(2), Some((2025, 1, 1)));
        assert_eq!(picker.date_at_cell(8), Some((2025, 1, 7)));
        assert_eq!(
            picker.weekday_header(),
            ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"],
            "the header rotates with the same first day"
        );

        let mut clicked = make_picker();
        clicked.set_first_day_of_week(chrono::Weekday::Mon);
        clicked.handle_event(&Event::MousePress {
            pos: cell_centre(&clicked, 2, 0),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(
            clicked.start_date(),
            Some((2025, 1, 1)),
            "the cell under the pointer is still the day painted there"
        );

        // A same-value write changes nothing.
        clicked.set_first_day_of_week(chrono::Weekday::Mon);
        assert_eq!(clicked.first_day_of_week(), chrono::Weekday::Mon);
    }

    /// Every weekday is a valid start, not only Sunday and Monday: a Saturday-first grid offsets
    /// January 2025 by four blanks (M-003 boundary).
    #[test]
    fn every_weekday_can_start_the_week() {
        let mut picker = make_picker();
        picker.set_first_day_of_week(chrono::Weekday::Sat);
        assert_eq!(picker.leading_blanks(0), 4);
        assert_eq!(picker.date_at_cell(3), None);
        assert_eq!(picker.date_at_cell(4), Some((2025, 1, 1)));
        assert_eq!(picker.weekday_header(), ["Sa", "Su", "Mo", "Tu", "We", "Th", "Fr"]);
    }

    #[test]
    fn a_click_selects_the_day_painted_in_that_cell() {
        // The draw places day `d` at `col = (offset + d - 1) % 7`; the hit test must resolve a
        // press at that cell's centre back to the same `d`. Before the geometry fix the two
        // agreed with each other but disagreed with the painted weekday, because the offset
        // itself was a day off.
        let picker = make_picker();
        let offset = picker.leading_blanks(0);
        for day in [1u32, 7, 15, 22, 31] {
            let index = offset + day - 1;
            let (col, row) = (index % 7, index / 7);

            let mut clicked = make_picker();
            clicked.handle_event(&Event::MousePress {
                pos: cell_centre(&picker, col, row),
                button: 1,
                modifiers: 0,
            });
            assert_eq!(
                clicked.start_date(),
                Some((2025, 1, day)),
                "day {day} drawn at column {col}, row {row} was hit-tested as {:?}",
                clicked.start_date()
            );
            assert!(clicked.end_date().is_none());
        }
    }

    #[test]
    fn hover_tracks_the_day_under_the_pointer_and_clears_on_leave() {
        let mut picker = make_picker();
        let offset = picker.leading_blanks(0);
        let index = offset + 14; // day 15
        let (col, row) = (index % 7, index / 7);

        picker.handle_event(&Event::MouseMove { pos: cell_centre(&picker, col, row) });
        assert_eq!(
            picker.hover_date,
            Some((2025, 1, 15)),
            "the cell under the pointer must be the highlighted date"
        );

        picker.handle_event(&Event::MouseLeave { pos: Point::new(0, 0) });
        assert_eq!(picker.hover_date, None, "leaving the control clears the preview");
    }

    #[test]
    fn date_range_picker_mouse_press_sets_start() {
        let mut picker = make_picker();
        // Jan 1, 2025 is Wednesday -> column 3 (Sunday-based). Day 15 is therefore cell
        // index 3 + 14 = 17 => row 2, column 3; the click below is that cell's centre.
        picker.handle_event(&Event::MousePress {
            pos: cell_centre(&picker, 3, 2),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(picker.start_date(), Some((2025, 1, 15)));
        assert!(picker.end_date().is_none());
    }

    #[test]
    fn a_click_in_a_leading_blank_does_not_invent_a_date() {
        let mut picker = make_picker();
        // Column 0..2 of the first week precede January 1st and own no date.
        picker.handle_event(&Event::MousePress {
            pos: cell_centre(&picker, 0, 0),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(picker.start_date(), None);
    }

    #[test]
    fn date_range_picker_mouse_press_clears_existing() {
        let mut picker = make_picker();
        picker.set_start_date(Some((2025, 1, 10)));

        // Click outside the grid area (row >= 6)
        picker.handle_event(&Event::MousePress {
            pos: Point::new(4, 250), // bottom of widget
            button: 1,
            modifiers: 0,
        });
        assert!(picker.start_date().is_none());
    }

    #[test]
    fn presets_double_month_and_night_bounds_are_live() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let mut picker = make_picker();
        picker.set_presets(vec![DatePreset::new("Week", (2025, 1, 1), (2025, 1, 7))]);
        let hits = Arc::new(AtomicUsize::new(0));
        let sink = Arc::clone(&hits);
        picker.preset_selected.connect(move |_| {
            sink.fetch_add(1, Ordering::SeqCst);
        });
        let chip = picker.preset_rect(0).expect("the preset chip exists");
        picker.handle_event(&Event::MousePress {
            pos: Point::new(chip.x + 2, chip.y + 2),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(picker.start_date(), Some((2025, 1, 1)));
        assert_eq!(picker.end_date(), Some((2025, 1, 7)));
        assert_eq!(hits.load(Ordering::SeqCst), 1);

        picker.set_double_month(true);
        assert!(picker.double_month());
        let svg = render_to_svg(&mut picker);
        assert!(svg.starts_with("<svg"));

        assert!(picker.nights_allowed((2025, 1, 1), (2025, 1, 2)));
        picker.set_min_nights(Some(2));
        assert!(!picker.nights_allowed((2025, 1, 1), (2025, 1, 2)));
        assert!(picker.nights_allowed((2025, 1, 1), (2025, 1, 3)));
        picker.set_max_nights(Some(3));
        assert!(!picker.nights_allowed((2025, 1, 1), (2025, 1, 5)));
    }

    /// A preset bar's range applies to the picker, and the picker reports the matching index.
    #[test]
    fn a_range_preset_applies_to_the_picker() {
        let mut picker = make_picker();
        picker.set_presets(vec![DatePreset::new("Week", (2025, 1, 1), (2025, 1, 7))]);
        let reported = std::sync::Arc::new(std::sync::Mutex::new(Vec::<usize>::new()));
        let sink = reported.clone();
        picker.preset_selected.connect(move |index| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*index);
            }
        });

        let known = RangePreset::new("Week", (2025, 1, 1), (2025, 1, 7));
        assert_eq!(picker.apply_preset(&known), Some(0));
        assert_eq!(picker.start_date(), Some((2025, 1, 1)));
        assert_eq!(picker.end_date(), Some((2025, 1, 7)));
        assert_eq!(*reported.lock().expect("reported lock"), vec![0]);

        let unknown = RangePreset::new("Quarter", (2025, 4, 1), (2025, 6, 30));
        assert_eq!(picker.apply_preset(&unknown), None);
        assert_eq!(picker.start_date(), Some((2025, 4, 1)));
        assert_eq!(picker.end_date(), Some((2025, 6, 30)));
    }

    /// MISC-008: the generic chip bar mirrors the picker's presets and active choice.
    #[test]
    fn a_preset_chip_bar_mirrors_the_picker_presets() {
        use crate::widget::misc_widgets::preset_chip_bar::PresetChipBar;

        let mut picker = make_picker();
        picker.set_presets(vec![
            DatePreset::new("Week", (2025, 1, 1), (2025, 1, 7)),
            DatePreset::new("Month", (2025, 1, 1), (2025, 1, 31)),
        ]);
        assert!(picker.attach_preset_bar(Some(PresetChipBar::new(Rect::new(0, 0, 240, 24)))));
        {
            let bar = picker.preset_bar().expect("attached");
            assert_eq!(bar.presets().len(), 2);
            assert_eq!(bar.presets()[0].label, "Week");
            assert_eq!(bar.selected_preset(), None);
        }

        picker.select_preset(1);
        assert_eq!(
            picker.preset_bar().expect("attached").selected_preset(),
            Some(2),
            "the chip id is the preset index plus one"
        );

        picker.clear_selection();
        assert_eq!(
            picker.preset_bar().expect("attached").selected_preset(),
            None,
            "a manual clear drops the active preset"
        );
    }

    #[test]
    fn preset_bar_emits_the_range_it_applies() {
        let mut bar = RangePresetBar::new(Rect::new(0, 0, 300, 24));
        bar.set_presets(vec![
            RangePreset::new("Today", (2025, 1, 1), (2025, 1, 1)),
            RangePreset::new("Week", (2025, 1, 1), (2025, 1, 7)),
        ]);
        let ranges = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = ranges.clone();
        bar.preset_selected.connect(move |range| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*range);
            }
        });

        assert!(bar.select("Week"));
        assert!(!bar.select("Missing"));
        assert_eq!(bar.selected_label(), Some("Week"));
        assert_eq!(bar.selected_range(), Some(((2025, 1, 1), (2025, 1, 7))));
        assert_eq!(*ranges.lock().expect("ranges lock"), vec![((2025, 1, 1), (2025, 1, 7))]);

        // A press on the first chip chooses it, and the chip boxes are the hit targets.
        let chip = bar.chip_rect(0).expect("chip 0");
        bar.handle_event(&Event::mouse_press(chip.x + 2, chip.y + 2, 1));
        assert_eq!(bar.selected_label(), Some("Today"));
    }

    #[test]
    fn preset_bar_properties_drive_layout_and_round_trip() {
        let mut bar = RangePresetBar::new(Rect::new(0, 0, 300, 100));
        let presets = CapabilityValue::Tuple(vec![
            CapabilityValue::String("Today|2025-01-01|2025-01-01".to_string()),
            CapabilityValue::String("Week|2025-01-01|2025-01-07".to_string()),
        ]);
        bar.set("presets", presets.clone()).expect("presets writable");
        assert_eq!(bar.get("presets"), Ok(presets));

        bar.set("orientation", CapabilityValue::String("vertical".to_string()))
            .expect("orientation writable");
        assert_eq!(bar.orientation(), PresetBarOrientation::Vertical);
        assert_eq!(bar.chip_rect(1).expect("chip 1").y, 50);

        bar.set("compact", CapabilityValue::Bool(true)).expect("compact writable");
        assert!(bar.compact());
        assert_eq!(bar.get("selected_label"), Ok(CapabilityValue::Null));
        assert_eq!(
            bar.set("selected_label", CapabilityValue::String("Week".to_string())),
            Err(CapabilityAccessError::ReadOnlyProperty)
        );
    }

    #[test]
    fn go_to_today_selects_the_wall_clock_day() {
        let mut picker = make_picker();
        picker.go_to_today();
        let today = chrono::Local::now().date_naive();
        assert_eq!(picker.display_year(), today.year());
        assert_eq!(picker.display_month(), today.month());
        assert_eq!(picker.start_date(), Some((today.year(), today.month(), today.day())));
    }

    /// A bounded picker refuses a day outside its range on every path (MISC-08).
    #[test]
    fn the_date_bounds_block_a_selection() {
        let mut picker = DateRangePicker::new(Rect::new(0, 0, 240, 260));
        picker.set_minimum_date(Some((2025, 6, 10)));
        picker.set_maximum_date(Some((2025, 6, 20)));
        assert!(!picker.accepts_date((2025, 6, 9)));
        assert!(picker.accepts_date((2025, 6, 10)));
        assert!(picker.accepts_date((2025, 6, 20)));
        assert!(!picker.accepts_date((2025, 6, 21)));

        // The shared commit refuses the out-of-range day without touching the selection.
        picker.apply_day((2025, 6, 5));
        assert_eq!(picker.start_date(), None);
        picker.apply_day((2025, 6, 12));
        assert_eq!(picker.start_date(), Some((2025, 6, 12)));
    }

    /// Arrows move a focus day across months; Enter applies it; Escape cancels a partial range
    /// (MISC-07).
    #[test]
    fn the_keyboard_walks_the_grid_and_commits() {
        let mut picker = DateRangePicker::new(Rect::new(0, 0, 240, 260));
        picker.set_display_page(2025, 6);
        picker.set_start_date(Some((2025, 6, 10)));

        picker.handle_event(&Event::key_press(crate::event::key_codes::RIGHT, 0));
        assert_eq!(picker.focus_date(), Some((2025, 6, 11)));
        picker.handle_event(&Event::key_press(crate::event::key_codes::DOWN, 0));
        assert_eq!(picker.focus_date(), Some((2025, 6, 18)));

        // Enter completes the range through the click path.
        picker.handle_event(&Event::key_press(crate::event::key_codes::ENTER, 0));
        assert_eq!(picker.end_date(), Some((2025, 6, 18)));

        // Escape clears an in-progress (start-only) selection.
        picker.set_end_date(None);
        picker.handle_event(&Event::key_press(crate::event::key_codes::ESCAPE, 0));
        assert_eq!(picker.start_date(), None);
    }

    /// Walking past the end of a month flips the page the grid shows.
    #[test]
    fn the_keyboard_flips_the_page_across_a_month_boundary() {
        let mut picker = DateRangePicker::new(Rect::new(0, 0, 240, 260));
        picker.set_display_page(2025, 6);
        picker.set_start_date(Some((2025, 6, 30)));
        picker.handle_event(&Event::key_press(crate::event::key_codes::RIGHT, 0));
        assert_eq!(picker.focus_date(), Some((2025, 7, 1)));
        assert_eq!(picker.display_month(), 7);
    }

    /// The header text is formatted once per page and follows a page change (MISC-05).
    #[test]
    fn the_month_header_is_cached_and_rebuilt_when_the_page_moves() {
        let mut picker = make_picker();
        assert_eq!(&*picker.header_text(0), "Jan 2025");
        // A second read is the cached value, not a fresh allocation with stale inputs.
        assert_eq!(&*picker.header_text(0), "Jan 2025");

        picker.set_display_page(2025, 2);
        assert_eq!(&*picker.header_text(0), "Feb 2025");
        picker.set_double_month(true);
        assert_eq!(&*picker.header_text(1), "Mar 2025");
    }

    /// Range membership matches date order at both boundaries (MISC-06).
    #[test]
    fn range_membership_and_bounds_use_date_order() {
        let mut picker = make_picker();
        picker.set_start_date(Some((2025, 1, 10)));
        picker.set_end_date(Some((2025, 1, 20)));
        assert!(!picker.is_in_range((2025, 1, 9)));
        assert!(picker.is_in_range((2025, 1, 10)));
        assert!(picker.is_in_range((2025, 1, 15)));
        assert!(picker.is_in_range((2025, 1, 20)));
        assert!(!picker.is_in_range((2025, 1, 21)));

        picker.set_minimum_date(Some((2025, 2, 1)));
        picker.set_maximum_date(Some((2025, 2, 28)));
        assert!(!picker.accepts_date((2025, 1, 31)));
        assert!(picker.accepts_date((2025, 2, 1)));
        assert!(picker.accepts_date((2025, 2, 28)));
        assert!(!picker.accepts_date((2025, 3, 1)));
    }

    /// A start date plus a hovered day previews the pending range in either direction, and the
    /// paint reflects it (MISC-07).
    #[test]
    fn hovering_after_a_start_previews_the_pending_range() {
        let mut picker = make_picker();
        picker.set_start_date(Some((2025, 1, 10)));

        picker.hover_date = Some((2025, 1, 15));
        assert_eq!(picker.hover_preview_range(), Some(((2025, 1, 10), (2025, 1, 15))));
        picker.hover_date = Some((2025, 1, 5));
        assert_eq!(picker.hover_preview_range(), Some(((2025, 1, 5), (2025, 1, 10))));

        // A committed range has nothing left to preview.
        picker.set_end_date(Some((2025, 1, 20)));
        assert_eq!(picker.hover_preview_range(), None);

        // The preview is visible, not just recorded.
        let _theme_guard = crate::style::theme_test_guard();
        let mut previewed = make_picker();
        previewed.set_start_date(Some((2025, 1, 10)));
        previewed.hover_date = Some((2025, 1, 15));
        let preview_svg = render_to_svg(&mut previewed);
        let mut plain = make_picker();
        plain.set_start_date(Some((2025, 1, 10)));
        let plain_svg = render_to_svg(&mut plain);
        assert_ne!(plain_svg, preview_svg, "the pending range must be painted");
    }
}
