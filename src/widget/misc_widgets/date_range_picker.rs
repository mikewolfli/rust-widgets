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
    /// Preset chips offered above the grid.
    presets: Vec<DatePreset>,
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
            presets: Vec::new(),
            double_month: false,
            min_nights: None,
            max_nights: None,
            range_changed: Signal1::new(),
            preset_selected: Signal1::new(),
        }
    }

    /// Points the grid at the current month without changing the selection.
    pub fn set_current_month(&mut self) {
        let today = chrono::Local::now().date_naive();
        self.display_year = today.year();
        self.display_month = today.month();
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
        self.base.request_redraw();
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
        self.emit_range_changed();
        self.preset_selected.emit(index);
        self.base.request_redraw();
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

    /// Clears both start and end dates.
    pub fn clear_selection(&mut self) {
        if self.start_date.is_some() || self.end_date.is_some() {
            self.start_date = None;
            self.end_date = None;
            self.emit_range_changed();
            self.base.request_redraw();
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

    /// Emits the `range_changed` signal with current start/end.
    fn emit_range_changed(&self) {
        self.range_changed.emit((self.start_date, self.end_date));
    }

    /// Returns the number of days in panel `panel`'s month.
    fn days_in_panel_month(&self, panel: usize) -> u32 {
        let (year, month) = self.panel_month(panel);
        days_in_month(year, month)
    }

    /// Returns the day of week for the 1st of the displayed month.
    #[cfg(test)]
    fn first_day_of_week(&self) -> u32 {
        day_of_week(self.display_year, self.display_month, 1)
    }

    /// Returns the day of week for the 1st of panel `panel`'s month.
    fn first_day_of_panel(&self, panel: usize) -> u32 {
        let (year, month) = self.panel_month(panel);
        day_of_week(year, month, 1)
    }

    /// Returns the day, month, year for a cell in panel `panel`'s grid.
    fn date_at_panel_cell(&self, panel: usize, cell_index: usize) -> Option<(i32, u32, u32)> {
        let first_dow = self.first_day_of_panel(panel) as usize;
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
    fn is_in_range(&self, date: (i32, u32, u32)) -> bool {
        let Some(start) = self.start_date else {
            return false;
        };
        if date_to_ordinal(date) < date_to_ordinal(start) {
            return false;
        }
        if let Some(end) = self.end_date {
            date_to_ordinal(date) <= date_to_ordinal(end)
        } else {
            // If no end date, highlight just the start
            date == start
        }
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
        let first_dow = self.first_day_of_panel(panel) as usize;
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

/// Converts a date to a single ordinal number for easy comparison.
fn date_to_ordinal(date: (i32, u32, u32)) -> i64 {
    date.0 as i64 * 10000 + date.1 as i64 * 100 + date.2 as i64
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

        context.face_with_gradient(
            rect,
            bg_color,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().border_radius.unwrap_or(0),
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
            let (year, month) = self.panel_month(panel);
            let header_band =
                Rect::new(panel_x + 20, header_top + 6, panel_width.saturating_sub(40), 18);
            let header_text = format!("{} {year}", month_name_for(month));
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
    let dow_height = context.measure_text("M", dow_font).height;
    let margin = TEXT_FIT_MARGIN as i32;
    for (i, day_name) in DAY_NAMES.iter().enumerate() {
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

        if in_range && is_enabled {
            let range_color =
                if is_start || is_end { accent.with_alpha(200) } else { accent.with_alpha(60) };
            context.fill_rounded_rect(cell_rect, 4, range_color);
        } else if is_hover && is_enabled {
            context.fill_rounded_rect(cell_rect, 4, surface.blend(&text_color, 0.16));
        } else if is_today && is_enabled {
            let today_color = if is_start || is_end {
                accent.with_alpha(200)
            } else {
                surface.blend(&text_color, 0.14)
            };
            context.fill_rounded_rect(cell_rect, 4, today_color);
        }

        let day_metrics = context.measure_text(day_text, day_font);
        let day_y = cell_y + (cell_size as i32 - day_metrics.height as i32) / 2;
        let day_color = if !is_enabled {
            text_color.blend(&surface, 0.55)
        } else if is_start || is_end {
            surface
        } else if in_range {
            accent
        } else {
            text_color
        };
        context.draw_text_fitted_cow(
            Rect::new(
                cell_x - TEXT_FIT_MARGIN as i32,
                day_y,
                cell_width + 2 * TEXT_FIT_MARGIN,
                day_metrics.height,
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

            match (self.start_date, self.end_date) {
                (None, _) => {
                    self.start_date = Some(clicked_date);
                    self.emit_range_changed();
                    self.base.request_redraw();
                }
                (Some(start), None) => {
                    let start_ord = date_to_ordinal(start);
                    let click_ord = date_to_ordinal(clicked_date);
                    let (new_start, new_end) = if click_ord > start_ord {
                        (start, clicked_date)
                    } else if click_ord < start_ord {
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
        assert_eq!(picker.first_day_of_week(), 3);
        assert_eq!(picker.date_at_cell(2), None);
        assert_eq!(picker.date_at_cell(3), Some((2025, 1, 1)));
        assert_eq!(picker.date_at_cell(9), Some((2025, 1, 7)));
    }

    #[test]
    fn a_click_selects_the_day_painted_in_that_cell() {
        // The draw places day `d` at `col = (offset + d - 1) % 7`; the hit test must resolve a
        // press at that cell's centre back to the same `d`. Before the geometry fix the two
        // agreed with each other but disagreed with the painted weekday, because the offset
        // itself was a day off.
        let picker = make_picker();
        let offset = picker.first_day_of_week();
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
        let offset = picker.first_day_of_week();
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

    #[test]
    fn go_to_today_selects_the_wall_clock_day() {
        let mut picker = make_picker();
        picker.go_to_today();
        let today = chrono::Local::now().date_naive();
        assert_eq!(picker.display_year(), today.year());
        assert_eq!(picker.display_month(), today.month());
        assert_eq!(picker.start_date(), Some((today.year(), today.month(), today.day())));
    }
}
