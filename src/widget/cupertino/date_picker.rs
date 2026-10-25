// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! CupertinoDatePicker widget — an iOS UIPickerView-style scrolling wheel date picker.
//!
//! The CupertinoDatePicker presents three scrolling columns (year, month, day)
//! side-by-side, similar to the iOS date picker wheel. Options are limited by
//! optional min/max date constraints. A `date_changed` signal is emitted
//! whenever the selected date changes.

use crate::core::{Color, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::style::{MotionSlot, PropertyDriver};
use crate::widget::capability::coercion::{expect_string, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, role_font, ControlMetrics};
use crate::widget::misc_widgets::date_utils::{days_in_month, MONTH_NAMES};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Parses an ISO `YYYY-MM-DD` date into a `(year, month, day)` tuple.
///
/// Returns `None` for anything that is not exactly that shape, so the property
/// contract can answer [`CapabilityAccessError::TypeMismatch`] instead of
/// silently clamping a malformed string into a valid-looking date.
///
/// The day is only range-checked to `1..=31`, not against the length of the
/// specific month: [`CupertinoDatePicker::set_selected_date`] normalises it
/// afterwards, and clamping there is what lets a caller step through months
/// without the day being rejected first.
fn parse_iso_date(text: &str) -> Option<(i32, u32, u32)> {
    let mut parts = text.split('-');
    let year = parts.next()?.parse::<i32>().ok()?;
    let month = parts.next()?.parse::<u32>().ok()?;
    let day = parts.next()?.parse::<u32>().ok()?;
    if parts.next().is_some() || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    Some((year, month, day))
}

/// The order the three date columns are drawn in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DateColumnOrder {
    /// Year, month, day.
    #[default]
    YearMonthDay,
    /// Day, month, year (common in much of Europe).
    DayMonthYear,
    /// Month, day, year (common in the United States).
    MonthDayYear,
}

/// Whether the picker edits a calendar date, a time of day, both, or a countdown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CupertinoDatePickerMode {
    /// Three date columns.
    #[default]
    Date,
    /// Two time columns, hour and minute.
    Time,
    /// The date and time wheels together on one drum.
    DateTime,
    /// Hour, minute and second columns for a countdown duration.
    CountDownTimer,
}

/// A valid date range for year generation.
#[derive(Clone, Debug)]
struct DateConstraint {
    min_year: i32,
    max_year: i32,
}

/// iOS-style scrolling wheel date picker with year/month/day columns.
///
/// Shows three columns (year, month, day) with a wheel-style scroll list.
/// The currently selected value in each column is visually highlighted.
/// A `date_changed` signal is emitted as a `(year, month, day)` tuple
/// whenever the selection changes.
pub struct CupertinoDatePicker {
    base: BaseWidget,
    selected_year: i32,
    selected_month: u32,
    selected_day: u32,
    min_date: Option<(i32, u32, u32)>,
    max_date: Option<(i32, u32, u32)>,
    /// Which kind of value the wheel edits.
    mode: CupertinoDatePickerMode,
    /// The order of the date columns.
    column_order: DateColumnOrder,
    /// Selected hour, `0..=23`, used by [`CupertinoDatePickerMode::Time`].
    selected_hour: u32,
    /// Selected minute, `0..=59`, used by [`CupertinoDatePickerMode::Time`].
    selected_minute: u32,
    /// Selected second, `0..=59`, used by [`CupertinoDatePickerMode::CountDownTimer`].
    selected_second: u32,
    /// Whether the time wheels show a 24-hour clock (`true`) or a 12-hour clock with AM/PM.
    use_24_hour: bool,
    /// Minute step; `1` shows every minute, `5` shows 00/05/10/....
    minute_interval: u32,
    /// An in-flight wheel drag: accumulated pixels since the last whole row.
    drag_accum: f32,
    /// The pointer y of the previous drag sample, for velocity estimation.
    drag_last_y: Option<f32>,
    /// The column the current drag began on.
    drag_column: usize,
    /// Estimated drag velocity in rows per frame, applied on release as inertia.
    drag_velocity: f32,
    /// How far the rows are rolled by the step animation, `0.0` settled to `1.0` just stepped.
    roll: PropertyDriver,
    /// The direction (`-1` or `1`) of the last step, so the roll travels the same way.
    roll_direction: i32,
    /// Emitted with a `(year, month, day)` tuple when the date changes.
    pub date_changed: Signal1<(i32, u32, u32)>,
    /// Emitted with `(hour, minute)` when the time changes in time mode.
    pub time_changed: Signal1<(u32, u32)>,
    /// Emitted with `(hour, minute, second)` in countdown mode, so a host showing a
    /// countdown sees the seconds it is counting (MW-14).
    pub countdown_changed: Signal1<(u32, u32, u32)>,
    /// Localised month names, defaulting to the English table.
    month_names: [String; 12],
    /// Set when `month_names` changed and [`Self::month_labels`] must be rebuilt.
    month_names_stale: bool,
    /// Cached month captions, parallel to `month_names`.
    month_labels: Vec<String>,
    /// Cached minute captions for [`Self::minute_interval`].
    minute_labels: Vec<String>,
    /// `minute_interval` the cached minutes were built for; `0` means stale.
    minute_key: u32,
    /// Cached hour captions for the current 12/24-hour choice.
    hour_labels: Vec<String>,
    /// Whether `hour_labels` is currently the 24-hour table.
    hour_labels_24: bool,
    /// Cached AM/PM captions.
    ampm_labels: Vec<String>,
    /// Cached day-of-month captions (`1..=31`).
    day_labels: Vec<String>,
    /// Cached second captions (`00..=59`).
    second_labels: Vec<String>,
    /// Cached year captions around the selection.
    year_labels: Vec<String>,
    /// `(selected year, min year, max year)` the cached years were built for.
    year_key: (i32, i32, i32),
}

impl CupertinoDatePicker {
    /// Creates a new CupertinoDatePicker with the given geometry.
    ///
    /// Initial date defaults to 2025-01-01.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::CupertinoDatePicker, geometry, "CupertinoDatePicker"),
            selected_year: 2025,
            selected_month: 1,
            selected_day: 1,
            min_date: None,
            max_date: None,
            mode: CupertinoDatePickerMode::Date,
            column_order: DateColumnOrder::YearMonthDay,
            selected_hour: 12,
            selected_minute: 0,
            selected_second: 0,
            use_24_hour: true,
            minute_interval: 1,
            drag_accum: 0.0,
            drag_last_y: None,
            drag_column: 0,
            drag_velocity: 0.0,
            roll: PropertyDriver::at(0.0, MotionSlot::Normal),
            roll_direction: 1,
            date_changed: Signal1::new(),
            time_changed: Signal1::new(),
            countdown_changed: Signal1::new(),
            month_names: core::array::from_fn(|index| MONTH_NAMES[index].to_string()),
            month_names_stale: true,
            month_labels: Vec::new(),
            minute_labels: Vec::new(),
            minute_key: 0,
            hour_labels: Vec::new(),
            hour_labels_24: true,
            ampm_labels: vec!["AM".to_string(), "PM".to_string()],
            day_labels: (1..=31).map(|day| day.to_string()).collect(),
            second_labels: (0..60).map(|second| format!("{second:02}")).collect(),
            year_labels: Vec::new(),
            year_key: (i32::MAX, i32::MAX, i32::MAX),
        }
    }

    /// Returns the current selected date as `(year, month, day)`.
    pub fn selected_date(&self) -> (i32, u32, u32) {
        (self.selected_year, self.selected_month, self.selected_day)
    }

    /// Sets the selected date, clamping month/day to valid ranges.
    /// Emits `date_changed` if the value actually changes.
    pub fn set_selected_date(&mut self, year: i32, month: u32, day: u32) {
        let clamped_month = month.clamp(1, 12);
        let max_day = days_in_month(year, clamped_month);
        let clamped_day = day.clamp(1, max_day);

        // Apply min/max constraints
        let (year, month, day) = self.clamp_to_date_range(year, clamped_month, clamped_day);

        if self.selected_year != year || self.selected_month != month || self.selected_day != day {
            self.selected_year = year;
            self.selected_month = month;
            self.selected_day = day;
            self.date_changed.emit((year, month, day));
            self.base.request_redraw();
        }
    }

    /// Sets the allowable date range. Pass `None` for no constraint.
    pub fn set_date_range(&mut self, min: Option<(i32, u32, u32)>, max: Option<(i32, u32, u32)>) {
        self.min_date = min;
        self.max_date = max;
        // Re-clamp current date to new range
        let clamped =
            self.clamp_to_date_range(self.selected_year, self.selected_month, self.selected_day);
        self.set_selected_date(clamped.0, clamped.1, clamped.2);
    }

    /// Returns the current min date constraint.
    pub fn min_date(&self) -> Option<(i32, u32, u32)> {
        self.min_date
    }

    /// Returns the current max date constraint.
    pub fn max_date(&self) -> Option<(i32, u32, u32)> {
        self.max_date
    }

    /// Returns whether the picker edits a date or a time of day.
    pub fn mode(&self) -> CupertinoDatePickerMode {
        self.mode
    }

    /// Selects date or time mode.
    pub fn set_mode(&mut self, mode: CupertinoDatePickerMode) {
        if self.mode == mode {
            return;
        }
        self.mode = mode;
        self.base.request_redraw();
    }

    /// Returns the column order used in date mode.
    pub fn column_order(&self) -> DateColumnOrder {
        self.column_order
    }

    /// Sets the column order used in date mode.
    pub fn set_column_order(&mut self, order: DateColumnOrder) {
        if self.column_order == order {
            return;
        }
        self.column_order = order;
        self.base.request_redraw();
    }

    /// Returns the selected time as `(hour, minute)`.
    pub fn selected_time(&self) -> (u32, u32) {
        (self.selected_hour, self.selected_minute)
    }

    /// The selected countdown second.
    pub fn selected_second(&self) -> u32 {
        self.selected_second
    }

    /// Whether the time wheels use a 24-hour clock.
    pub fn use_24_hour(&self) -> bool {
        self.use_24_hour
    }

    /// Switches between a 24-hour and a 12-hour clock with an AM/PM column.
    pub fn set_use_24_hour(&mut self, use_24_hour: bool) {
        if self.use_24_hour == use_24_hour {
            return;
        }
        self.use_24_hour = use_24_hour;
        self.base.request_redraw();
    }

    /// The minute step shown by the minute wheel.
    pub fn minute_interval(&self) -> u32 {
        self.minute_interval
    }

    /// Sets the minute step. Clamped to `1..=30` and applied to the current selection.
    pub fn set_minute_interval(&mut self, interval: u32) {
        let interval = interval.clamp(1, 30);
        if self.minute_interval == interval {
            return;
        }
        self.minute_interval = interval;
        self.selected_minute = (self.selected_minute / interval) * interval;
        self.base.request_redraw();
    }

    /// The displayed minute values, stepped by [`Self::minute_interval`].
    fn minute_values(&self) -> &[String] {
        &self.minute_labels
    }

    /// The hour wheel as `(selected offset, labels)`, honouring the 24/12-hour choice.
    fn hour_values(&self) -> (i32, &[String]) {
        if self.use_24_hour {
            (self.selected_hour as i32, &self.hour_labels)
        } else {
            // 12-hour wheel: 12, 1, 2, ... 11. Midnight and noon both display as 12.
            let selected = if self.selected_hour.is_multiple_of(12) {
                0
            } else {
                (self.selected_hour % 12) as i32
            };
            (selected, &self.hour_labels)
        }
    }

    /// The AM/PM wheel.
    fn ampm_values(&self) -> (i32, &[String]) {
        let selected = if self.selected_hour < 12 { 0 } else { 1 };
        (selected, &self.ampm_labels)
    }

    /// The time columns in draw order: hour, minute, and AM/PM when the clock is 12-hour.
    fn time_columns(&self) -> Vec<(i32, &[String])> {
        let minute_offset = (self.selected_minute / self.minute_interval.max(1)) as i32;
        let mut columns = vec![self.hour_values(), (minute_offset, self.minute_values())];
        if !self.use_24_hour {
            columns.push(self.ampm_values());
        }
        columns
    }

    /// The date columns (year, month, day) in logical order, borrowing the cached caption
    /// tables (L-P09).
    fn date_columns(&self) -> (Vec<(i32, &[String])>, Vec<usize>) {
        let max_days = days_in_month(self.selected_year, self.selected_month);
        let months: &[String] = &self.month_labels;
        let days: &[String] = &self.day_labels[..max_days as usize];
        let logical = vec![
            (2, self.year_labels.as_slice()),
            (self.selected_month as i32 - 1, months),
            (self.selected_day as i32 - 1, days),
        ];
        let order = match self.column_order {
            DateColumnOrder::YearMonthDay => vec![0, 1, 2],
            DateColumnOrder::DayMonthYear => vec![2, 1, 0],
            DateColumnOrder::MonthDayYear => vec![1, 2, 0],
        };
        (logical, order)
    }

    /// Rebuilds the cached caption tables whose inputs changed (L-P09).
    ///
    /// The wheels are static text: minutes, hours, month names and day numbers do not change
    /// while a user scrolls, yet the paint path used to format every one of them on every
    /// frame. The tables are rebuilt on the setter that changes their input instead.
    fn refresh_label_tables(&mut self) {
        if self.minute_key != self.minute_interval {
            self.minute_key = self.minute_interval;
            let interval = self.minute_interval.max(1) as usize;
            self.minute_labels =
                (0..60).step_by(interval).map(|minute| format!("{minute:02}")).collect();
        }
        if self.hour_labels_24 != self.use_24_hour || self.hour_labels.is_empty() {
            self.hour_labels_24 = self.use_24_hour;
            self.hour_labels = if self.use_24_hour {
                (0..24).map(|hour| format!("{hour:02}")).collect()
            } else {
                let mut hours = Vec::with_capacity(12);
                hours.push("12".to_string());
                hours.extend((1..12).map(|hour| hour.to_string()));
                hours
            };
        }
        if self.month_names_stale {
            self.month_names_stale = false;
            self.month_labels = self.month_names.to_vec();
        }
        let constraint = self.date_constraint();
        let key = (self.selected_year, constraint.min_year, constraint.max_year);
        if self.year_key != key {
            self.year_key = key;
            self.year_labels = (0..dimensions::PICKER_VISIBLE_ROWS)
                .map(|row| {
                    let year = self.selected_year + row as i32 - 2;
                    if year >= constraint.min_year && year <= constraint.max_year {
                        year.to_string()
                    } else {
                        String::new()
                    }
                })
                .collect();
        }
    }

    /// Replaces the month captions shown by the month wheel.
    ///
    /// An empty entry is rejected as a whole array rather than producing a wheel with blank
    /// rows: a month name is the row's content, so a caller with nothing to say should keep the
    /// default English table.
    pub fn set_month_names(&mut self, names: [String; 12]) {
        if names.iter().any(String::is_empty) {
            return;
        }
        if self.month_names == names {
            return;
        }
        self.month_names = names;
        self.month_names_stale = true;
        self.base.request_redraw();
    }

    /// The month captions currently used by the wheel.
    pub fn month_names(&self) -> &[String; 12] {
        &self.month_names
    }

    /// Emits the countdown tuple when the picker is in countdown mode.
    fn emit_countdown(&self) {
        if self.mode == CupertinoDatePickerMode::CountDownTimer {
            self.countdown_changed.emit((
                self.selected_hour,
                self.selected_minute,
                self.selected_second,
            ));
        }
    }

    /// The number of wheel columns the current mode paints.
    fn column_count(&self) -> usize {
        match self.mode {
            CupertinoDatePickerMode::Date | CupertinoDatePickerMode::CountDownTimer => 3,
            CupertinoDatePickerMode::Time => {
                if self.use_24_hour {
                    2
                } else {
                    3
                }
            }
            CupertinoDatePickerMode::DateTime => 3 + if self.use_24_hour { 2 } else { 3 },
        }
    }

    /// Sets the selected time, clamping each field into its own range.
    pub fn set_selected_time(&mut self, hour: u32, minute: u32) {
        let hour = hour.min(23);
        let minute = (minute.min(59) / self.minute_interval) * self.minute_interval;
        if self.selected_hour == hour && self.selected_minute == minute {
            return;
        }
        self.selected_hour = hour;
        self.selected_minute = minute;
        self.time_changed.emit((hour, minute));
        self.emit_countdown();
        self.base.request_redraw();
    }

    /// Sets the countdown second, clamped to `0..=59`.
    pub fn set_selected_second(&mut self, second: u32) {
        let second = second.min(59);
        if self.selected_second == second {
            return;
        }
        self.selected_second = second;
        self.time_changed.emit((self.selected_hour, self.selected_minute));
        self.emit_countdown();
        self.base.request_redraw();
    }

    /// Advances the roll animation; `true` while it is still moving.
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        self.roll.tick(delta_ms)
    }

    /// Whether the roll animation is between two positions.
    pub fn is_animating(&self) -> bool {
        self.roll.is_moving()
    }

    /// Starts a roll in `direction` (`-1` or `1`) after a step.
    fn start_roll(&mut self, direction: i32) {
        self.roll_direction = direction;
        self.roll.jump_to(1.0);
        self.roll.set_target(0.0);
    }

    /// Applies one step to `(hour, minute)`, wrapping within its own field.
    fn step_time(
        hour: u32,
        minute: u32,
        second: u32,
        column: usize,
        direction: i32,
        interval: u32,
    ) -> (u32, u32, u32) {
        match column {
            0 => (((hour as i32 + direction).rem_euclid(24)) as u32, minute, second),
            1 => {
                let slots = (60 / interval).max(1) as i32;
                let index = (minute / interval) as i32;
                let next = (index + direction).rem_euclid(slots) * interval as i32;
                (hour, next as u32, second)
            }
            _ => (hour, minute, ((second as i32 + direction).rem_euclid(60)) as u32),
        }
    }

    /// Clamps a date to the configured min/max range.
    fn clamp_to_date_range(&self, year: i32, month: u32, day: u32) -> (i32, u32, u32) {
        let (y, m, d) = (year, month, day);
        if let Some((min_y, min_m, min_d)) = self.min_date {
            if y < min_y || (y == min_y && (m < min_m || (m == min_m && d < min_d))) {
                return (min_y, min_m, min_d);
            }
        }
        if let Some((max_y, max_m, max_d)) = self.max_date {
            if y > max_y || (y == max_y && (m > max_m || (m == max_m && d > max_d))) {
                return (max_y, max_m, max_d);
            }
        }
        (y, m, d)
    }

    /// Determines which column (0=year, 1=month, 2=day) contains the given x-coordinate.
    fn column_at(&self, x: i32) -> usize {
        let rect = self.geometry();
        let columns = self.column_count().max(1);
        // A control narrower than `columns` pixels has a `col_width` of `0`, and `rel_x / 0`
        // panicked. Such a control cannot show distinct columns anyway, so every x maps to the
        // first column rather than dividing by zero.
        let col_width = rect.width / columns as u32;
        if col_width == 0 {
            return 0;
        }
        if x < rect.x {
            return 0;
        }
        let rel_x = (x - rect.x) as u32;
        let col = rel_x / col_width;
        (col as usize).min(columns - 1)
    }

    /// Maps a physical column position to the logical field it edits.
    ///
    /// The locale order changes where a date column is *drawn*, so the hit test must translate the
    /// pointer's position back to the field before stepping it; otherwise a `DayMonthYear`
    /// picker would change the year when the user taps the day column.
    ///
    /// The ids are mode-specific: `0..=2` are the date fields in `YearMonthDay` order for the date
    /// modes, `3`/`4` are hour/minute in a combined picker, `5` is its AM/PM column, and the
    /// countdown mode uses `0`/`1`/`2` for hour/minute/second.
    fn logical_column(&self, physical: usize) -> usize {
        let date_column = |physical: usize| -> usize {
            match self.column_order {
                DateColumnOrder::YearMonthDay => physical.min(2),
                DateColumnOrder::DayMonthYear => [2, 1, 0][physical.min(2)],
                DateColumnOrder::MonthDayYear => [1, 2, 0][physical.min(2)],
            }
        };
        match self.mode {
            CupertinoDatePickerMode::Date => date_column(physical),
            // 0 = hour, 1 = minute, 2 = AM/PM when the clock is 12-hour.
            CupertinoDatePickerMode::Time => physical.min(if self.use_24_hour { 1 } else { 2 }),
            CupertinoDatePickerMode::CountDownTimer => physical.min(2),
            CupertinoDatePickerMode::DateTime => {
                if physical < 3 {
                    date_column(physical)
                } else {
                    // 3 = hour, 4 = minute, 5 = AM/PM.
                    (3 + physical - 3).min(if self.use_24_hour { 4 } else { 5 })
                }
            }
        }
    }

    /// Returns the range of years available for selection.
    ///
    /// Returns the constraint for the current selection context.
    fn date_constraint(&self) -> DateConstraint {
        let min_y = self.min_date.map(|d| d.0).unwrap_or(1900);
        let max_y = self.max_date.map(|d| d.0).unwrap_or(2100);
        // Ordering the two ends keeps an inverted `set_date_range` from producing an interval that
        // panics when iterated (`min..=max` with `min > max` is empty). `draw` walks the five rows
        // around the selection from this same constraint, so it cannot see a different interval.
        DateConstraint { min_year: min_y.min(max_y), max_year: min_y.max(max_y) }
    }
}

impl Widget for CupertinoDatePicker {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        crate::core::Size::new(300, 200)
    }

    /// Advances the roll animation.
    fn tick(&mut self, delta_ms: u32) -> bool {
        CupertinoDatePicker::tick(self, delta_ms)
    }

    fn is_animating(&self) -> bool {
        CupertinoDatePicker::is_animating(self)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why the field is named per arm
    ///
    /// The published name and the Rust field name are not always the same (`find_next` is backed by
    /// `find_next_signal`, `dismissed` by a `Signal1<()>` field). `connect_event` validates a name
    /// against the capability table and registers a hub slot; only `event_signal_dyn` joins that
    /// name to the signal the control actually **emits**. A wrong arm is worse than no arm, because
    /// it reports a wire as live and never fires it, so each field is named explicitly here rather
    /// than derived from the published name.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "date_changed" => {
                Some(EventSignalRef::mapped("date_changed", &self.date_changed, |v| {
                    CapabilityValue::Tuple(alloc::vec![
                        CapabilityValue::Int(v.0 as i64),
                        CapabilityValue::UInt(v.1 as u64),
                        CapabilityValue::UInt(v.2 as u64),
                    ])
                }))
            }
            "time_changed" => {
                Some(EventSignalRef::mapped("time_changed", &self.time_changed, |v| {
                    crate::widget::capability::CapabilityValue::Tuple(crate::compat::Vec::from([
                        crate::widget::capability::CapabilityValue::UInt(v.0 as u64),
                        crate::widget::capability::CapabilityValue::UInt(v.1 as u64),
                    ]))
                }))
            }
            _ => None,
        }
    }
}

/// `CupertinoDatePicker`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_input.in.rs` / `access_write_input.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. `selected_date` reads the
/// picker's real date and publishes it as ISO `YYYY-MM-DD`, replacing the fixed
/// `2025-01-01` the centralised defaults table returned.
impl WidgetProperties for CupertinoDatePicker {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "selected_date" => {
                let (year, month, day) = self.selected_date();
                Ok(CapabilityValue::String(format!("{year:04}-{month:02}-{day:02}")))
            }
            "mode" => Ok(CapabilityValue::String(
                match self.mode() {
                    CupertinoDatePickerMode::Date => "date",
                    CupertinoDatePickerMode::Time => "time",
                    CupertinoDatePickerMode::DateTime => "datetime",
                    CupertinoDatePickerMode::CountDownTimer => "countdown",
                }
                .to_string(),
            )),
            "use_24_hour" => Ok(CapabilityValue::Bool(self.use_24_hour())),
            "minute_interval" => Ok(CapabilityValue::UInt(self.minute_interval() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "selected_date" => {
                let parsed = parse_iso_date(&expect_string(value)?)
                    .ok_or(CapabilityAccessError::TypeMismatch)?;
                self.set_selected_date(parsed.0, parsed.1, parsed.2);
                Ok(())
            }
            "mode" => {
                let mode = match expect_string(value)?.to_ascii_lowercase().as_str() {
                    "date" => CupertinoDatePickerMode::Date,
                    "time" => CupertinoDatePickerMode::Time,
                    "datetime" => CupertinoDatePickerMode::DateTime,
                    "countdown" | "countdowntimer" => CupertinoDatePickerMode::CountDownTimer,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_mode(mode);
                Ok(())
            }
            "use_24_hour" => {
                let use_24_hour = match value {
                    CapabilityValue::Bool(value) => value,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_use_24_hour(use_24_hour);
                Ok(())
            }
            "minute_interval" => {
                let interval = expect_usize(value)?.min(u32::MAX as usize) as u32;
                self.set_minute_interval(interval);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "selected_date",
            "mode",
            "use_24_hour",
            "minute_interval",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl Draw for CupertinoDatePicker {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let is_enabled = self.base.is_enabled();
        self.refresh_label_tables();

        // Chrome colours resolve explicit style first, then the theme's resolved style for
        // this control, and only then fall back to a literal. Every colour below used to be
        // a literal, so a light/dark switch left the wheel, its columns, its divider, its
        // selection band and its text unchanged — the rendering census reported the control
        // as theme-blind.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global manager's
        // mutex is not re-entrant.
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("cupertino_date_picker");
        // Read as its own lock acquisition and copied out as values, so the guard is dropped
        // before anything else touches the theme. The picker is not in the role table, so it
        // classifies as `Surface` and its resolved background is the window fill itself; the
        // wheel below therefore derives its own distinct surface rather than painting the
        // window's. The selected row is the wheel's value indicator, so it reads the theme's
        // accent. `muted` is the theme's own de-emphasised colour, which replaces the two
        // fixed greys the unselected rows and the arrows used.
        let (window_fill, foreground, accent, muted, disabled) = {
            let manager = crate::style::theme_manager();
            match manager.current_theme() {
                Some(active) => (
                    active.colors.background,
                    active.colors.foreground,
                    active.colors.accent,
                    active.colors.secondary,
                    active.colors.disabled,
                ),
                None => (
                    Color::rgb(240, 240, 240),
                    Color::BLACK,
                    Color::rgb(255, 152, 0),
                    Color::rgb(158, 158, 158),
                    Color::rgb(200, 200, 200),
                ),
            }
        };

        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(foreground);
        // The wheel interior: one step from the window fill toward the text colour, so it
        // reads as a recessed field on a light theme and on a dark one. The filter is on the
        // **resolved** value, not only on the theme's: the active theme is applied to every
        // control before it is drawn, so `style.background_color` already holds `Surface`'s
        // window fill and letting it through unfiltered is exactly the invisible-field defect
        // this guards against. A caller's own colour still wins.
        let wheel_from_theme = window_fill.blend(&ink, 0.06);
        let wheel = match style.background_color {
            Some(resolved) if resolved != window_fill => resolved,
            _ => wheel_from_theme,
        };
        // A `Surface` role resolves no border colour, so the column rule is derived one
        // visible step from the wheel and a caller's explicit border still wins.
        let column_rule = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .filter(|resolved| *resolved != wheel)
            .unwrap_or_else(|| wheel.blend(&muted, 0.45));
        // The selection band, the selected row's text and the arrows are all accents: opaque
        // variants for the text and arrows, a low-alpha one for the band. The band is painted
        // with `fill_rounded_rect`, which alpha-blends, so a translucent accent really does
        // tint the wheel behind it here (unlike `fill_rect`, which overwrites).
        //
        // The selected row's ink is therefore chosen against the surface it is really painted
        // on — the band composited over the wheel — and not against `accent`. Choosing it from
        // the accent (and then blending 80% of the way back to the accent) made the label and
        // its backdrop the same hue at the same lightness: `rgb(204,122,0)` on the light
        // theme's `rgb(243,222,192)` band measured 2.52:1. `Color::blend` is the same alpha
        // composite the renderer performs, so this predicts the painted colour exactly.
        let band_alpha = 50.0 / 255.0;
        let banded_surface = wheel.blend(&accent.with_alpha(255), band_alpha);
        let selected_text = banded_surface.contrast_color();
        let band = Color::rgba(accent.r, accent.g, accent.b, 50);
        // The up/down arrows say "there are more values this way", so they are held legible on
        // the wheel while keeping the accent's hue — the plain 35% blend measured 3.68:1 on the
        // light wheel, so the only cue that the list continues was the faintest mark in it.
        let arrow = accent.blend(&ink, 0.35).legible_on(wheel, 4.5);
        // The disabled treatment of each of the three, so the appearance agrees with the
        // control's refusal to accept a pick.
        let disabled_text = muted.blend(&wheel, 0.35);
        let disabled_arrow = muted.blend(&wheel, 0.20);
        let disabled_surface = disabled.blend(&wheel, 0.55);

        // Background
        let bg_color = if is_enabled { wheel } else { disabled_surface };
        // ── The drum actually painted ──
        //
        // `rect` is the area the picker was *given*; the drum is a fixed-row wheel, so its
        // height is [`dimensions::PICKER_VISIBLE_ROWS`] rows of [`dimensions::PICKER_ROW_HEIGHT`]
        // — not a fifth of whatever height the caller supplied. Deriving the row from
        // `rect.height / 5` made a 120 px cell show five 24 px rows and the 300x200 panel five
        // 60 px rows: the same control at two densities, with the font size tied to the row.
        // The band is centred and clamped to the rectangle, so a picker shorter than five rows
        // still fits inside it.
        let wheel_rect = ControlMetrics::center_in(
            rect,
            Size::new(rect.width, dimensions::PICKER_ROW_HEIGHT * dimensions::PICKER_VISIBLE_ROWS),
        );
        context.fill_rect(wheel_rect, bg_color);

        // Column layout
        //
        // The column width still divides the available width (a wheel fills its panel
        // horizontally), but the row height is the picker's own fact. `row_height` is the row
        // **as painted**, which equals `PICKER_ROW_HEIGHT` when the band was not clamped — and
        // it is what the font must follow: deriving the font from the nominal 32 px row while
        // the band drew 24 px rows put a 15.8 px arrow inside a 24 px row and pushed the first
        // and last visible values off the wheel in the census cell.
        let col_count = self.column_count().max(1);
        let col_width = wheel_rect.width / col_count as u32;
        let row_height = (wheel_rect.height / dimensions::PICKER_VISIBLE_ROWS).max(1);
        let font_size = (row_height as f32 * 0.38).clamp(9.0, 15.0);
        let font = role_font(style, font_size);
        // The rows roll by the step animation's fraction; at rest the fraction is exactly zero,
        // so the settled wheel is byte-identical to the un-animated one.
        let roll_shift =
            (self.roll.value() * row_height as f32 * self.roll_direction as f32) as i32;

        let (columns, order) = match self.mode {
            CupertinoDatePickerMode::Date => self.date_columns(),
            CupertinoDatePickerMode::CountDownTimer => (
                vec![
                    (self.selected_hour as i32, self.hour_labels.as_slice()),
                    (self.selected_minute as i32, self.minute_labels.as_slice()),
                    (self.selected_second as i32, self.second_labels.as_slice()),
                ],
                vec![0, 1, 2],
            ),
            CupertinoDatePickerMode::Time => {
                let columns = self.time_columns();
                let order = (0..columns.len()).collect();
                (columns, order)
            }
            CupertinoDatePickerMode::DateTime => {
                let (mut columns, mut order) = self.date_columns();
                let offset = columns.len();
                columns.extend(self.time_columns());
                order.extend(offset..columns.len());
                (columns, order)
            }
        };

        for (position, &logical_index) in order.iter().enumerate() {
            let Some((sel_offset, items)) = columns.get(logical_index) else {
                continue;
            };
            let col_x = wheel_rect.x + (position as u32 * col_width) as i32;

            // Vertical rule between columns. Drawn as a *line* rather than a 1 px wide
            // rectangle: `draw_rect_stroke` on a 1-wide rect emits a degenerate element
            // (`width="1"` in the SVG) that the two strokes of its own outline overdraw, so the
            // division was both wider than a divider and indistinguishable from a module.
            if position > 0 {
                context.draw_line(
                    Point::new(col_x, wheel_rect.y),
                    Point::new(col_x, wheel_rect.y + wheel_rect.height as i32),
                    column_rule,
                );
            }

            // Highlight bar for the center (selected) row
            let highlight_y = wheel_rect.y + 2 * row_height as i32;
            let highlight_rect =
                Rect::new(col_x + 4, highlight_y, col_width.saturating_sub(8), row_height);
            context.fill_rounded_rect(highlight_rect, 6, band);

            // Draw the five visible rows
            for row in 0..dimensions::PICKER_VISIBLE_ROWS {
                let item_idx = row as i32 + (sel_offset - 2);
                if item_idx < 0 || item_idx >= items.len() as i32 {
                    continue;
                }

                let text = &items[item_idx as usize];
                let is_selected = row == 2;
                let item_y = wheel_rect.y + (row * row_height) as i32 + roll_shift;

                let metrics =
                    crate::widget::base_widgets::measure_cache::measure_text(context, text, &font);
                // Centred inside the row's own band. The old origin added `ascent` on top of
                // an already-centred y, so the glyph box (which starts at the origin and runs
                // a full line down) began half a line low — the fifth row was pushed past the
                // control and only its top edge was ever painted.
                let text_y = item_y + (row_height as i32 - metrics.height as i32) / 2;

                let text_color = if !is_enabled {
                    disabled_text
                } else if is_selected {
                    selected_text
                } else {
                    // The neighbouring rows are *read*: a wheel's whole purpose is to show the
                    // values next to the selected one. They are secondary relative to the
                    // selection, not faint to the point of unreadable — the theme's `muted`
                    // token measured 2.07:1 on the light appearance's wheel, below even the 3:1
                    // large-text floor, so the rows a user is scrolling towards were the ones
                    // they could not read. Pushed clear while staying below the selection's
                    // contrast, which is what makes the selection still read as selected.
                    muted.legible_on(wheel, 4.5)
                };

                context.draw_text_fitted_cow(
                    Rect::new(col_x, text_y, col_width, metrics.height),
                    text,
                    &font,
                    text_color,
                    HorizontalAlignment::Center,
                );
            }

            // Chevron indicators drawn as strokes: a `^`/`v` glyph depends on the face having
            // those characters and reads as text rather than as a control affordance.
            let arrow_color = if is_enabled { arrow } else { disabled_arrow };
            let center_x = col_x + col_width as i32 / 2;
            let up_y = wheel_rect.y + 3;
            context.draw_line(
                Point::new(center_x - 4, up_y + 4),
                Point::new(center_x, up_y),
                arrow_color,
            );
            context.draw_line(
                Point::new(center_x, up_y),
                Point::new(center_x + 4, up_y + 4),
                arrow_color,
            );
            // Down chevron, on the wheel rather than on the control's own bottom edge.
            let down_y = wheel_rect.y + wheel_rect.height as i32 - row_height as i32 + 3;
            context.draw_line(
                Point::new(center_x - 4, down_y + 4),
                Point::new(center_x, down_y + 8),
                arrow_color,
            );
            context.draw_line(
                Point::new(center_x, down_y + 8),
                Point::new(center_x + 4, down_y + 4),
                arrow_color,
            );
        }
    }
}

impl EventHandler for CupertinoDatePicker {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        let rect = self.geometry();
        let wheel_rect = ControlMetrics::center_in(
            rect,
            Size::new(rect.width, dimensions::PICKER_ROW_HEIGHT * dimensions::PICKER_VISIBLE_ROWS),
        );
        let row_height = (wheel_rect.height / dimensions::PICKER_VISIBLE_ROWS).max(1) as f32;

        match event {
            Event::MousePress { pos, button: 1, .. } => {
                self.drag_accum = 0.0;
                self.drag_last_y = Some(pos.y as f32);
                self.drag_column = self.column_at(pos.x);
                self.drag_velocity = 0.0;
                if !wheel_rect.contains_point(*pos) {
                    return;
                }
                // A click on the upper half of a column steps forward, the lower half back.
                let col = self.logical_column(self.column_at(pos.x));
                let rel_y = pos.y - wheel_rect.y;
                let row = rel_y / row_height as i32;
                self.apply_step(col, row <= 2);
            }
            Event::MouseMove { pos } => {
                let Some(last) = self.drag_last_y else {
                    return;
                };
                let delta = pos.y as f32 - last;
                self.drag_last_y = Some(pos.y as f32);
                self.drag_velocity = delta / 16.0;
                self.drag_accum += delta;
                // Whole rows only: the remainder stays accumulated, so a slow drag still
                // advances once it has covered a full row.
                let mut steps = (self.drag_accum / row_height) as i32;
                if steps != 0 {
                    self.drag_accum -= steps as f32 * row_height;
                    while steps != 0 {
                        // Dragging down reveals the previous value, so the wheel follows the
                        // finger rather than fighting it.
                        self.apply_step(self.logical_column(self.drag_column), steps < 0);
                        steps += if steps > 0 { -1 } else { 1 };
                    }
                }
            }
            Event::MouseRelease { button: 1, .. } => {
                self.drag_last_y = None;
                self.drag_accum = 0.0;
                // Inertia: a fast flick keeps stepping a little after release. The estimate is
                // per nominal 16 ms frame, so it is independent of the host's actual rate.
                let extra = (self.drag_velocity.abs() as i32).min(3);
                if extra > 0 {
                    for _ in 0..extra {
                        self.apply_step(
                            self.logical_column(self.drag_column),
                            self.drag_velocity < 0.0,
                        );
                    }
                }
                self.drag_velocity = 0.0;
            }
            _ => {}
        }
    }
}

impl CupertinoDatePicker {
    /// Applies one selection step to `column`, where `forward` means a later value.
    fn apply_step(&mut self, column: usize, forward: bool) {
        let direction = if forward { 1 } else { -1 };
        match self.mode {
            CupertinoDatePickerMode::Date => self.apply_date_step(column, direction),
            CupertinoDatePickerMode::CountDownTimer => {
                let (hour, minute, second) = Self::step_time(
                    self.selected_hour,
                    self.selected_minute,
                    self.selected_second,
                    column.min(2),
                    direction,
                    self.minute_interval,
                );
                self.set_selected_time(hour, minute);
                self.set_selected_second(second);
                self.start_roll(direction);
            }
            CupertinoDatePickerMode::Time | CupertinoDatePickerMode::DateTime => {
                let (date_column, time_column, ampm_column) = match self.mode {
                    CupertinoDatePickerMode::DateTime => (Some((0, 1, 2)), 3, 5),
                    _ => (None, 0, 2),
                };
                if let Some((year, _month, day)) = date_column {
                    if column <= day {
                        // Date fields keep their own stepping; an inverted locale order was already
                        // resolved by `logical_column`.
                        self.apply_date_step(column.max(year).min(day), direction);
                        return;
                    }
                }
                if column == ampm_column {
                    // AM/PM is one bit: stepping it moves the hour by twelve.
                    self.selected_hour = (self.selected_hour + 12) % 24;
                    self.time_changed.emit((self.selected_hour, self.selected_minute));
                    self.start_roll(direction);
                    return;
                }
                let local_column = column.saturating_sub(time_column).min(1);
                let (hour, minute, second) = Self::step_time(
                    self.selected_hour,
                    self.selected_minute,
                    self.selected_second,
                    local_column,
                    direction,
                    self.minute_interval,
                );
                self.set_selected_time(hour, minute);
                self.set_selected_second(second);
                self.start_roll(direction);
            }
        }
    }

    /// Steps one calendar field by `direction` and starts the roll animation.
    fn apply_date_step(&mut self, column: usize, direction: i32) {
        let forward = direction > 0;
        match column {
            0 => {
                let constraint = self.date_constraint();
                let new_year = if forward {
                    (self.selected_year + 1).min(constraint.max_year)
                } else {
                    (self.selected_year - 1).max(constraint.min_year)
                };
                let max_day = days_in_month(new_year, self.selected_month);
                let clamped_day = self.selected_day.min(max_day);
                self.set_selected_date(new_year, self.selected_month, clamped_day);
            }
            1 => {
                let new_month = if forward {
                    (self.selected_month as i32 + 1).clamp(1, 12) as u32
                } else {
                    (self.selected_month as i32 - 1).clamp(1, 12) as u32
                };
                let max_day = days_in_month(self.selected_year, new_month);
                let clamped_day = self.selected_day.min(max_day);
                self.set_selected_date(self.selected_year, new_month, clamped_day);
            }
            _ => {
                let max_days = days_in_month(self.selected_year, self.selected_month) as i32;
                let new_day = if forward {
                    (self.selected_day as i32 + 1).clamp(1, max_days) as u32
                } else {
                    (self.selected_day as i32 - 1).clamp(1, max_days) as u32
                };
                self.set_selected_date(self.selected_year, self.selected_month, new_day);
            }
        }
        self.start_roll(direction);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::svg::render_to_svg;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    fn make_picker() -> CupertinoDatePicker {
        CupertinoDatePicker::new(Rect::new(0, 0, 240, 200))
    }

    #[test]
    fn picker_default_creation() {
        let picker = make_picker();
        assert_eq!(picker.kind(), WidgetKind::CupertinoDatePicker);
        assert_eq!(picker.selected_date(), (2025, 1, 1));
        assert!(picker.is_visible());
        assert!(picker.is_enabled());
        assert_eq!(picker.geometry(), Rect::new(0, 0, 240, 200));
    }

    #[test]
    fn picker_set_selected_date() {
        let mut picker = make_picker();
        picker.set_selected_date(2026, 12, 25);
        assert_eq!(picker.selected_date(), (2026, 12, 25));
    }

    #[test]
    fn picker_set_date_clamps_day_out_of_range() {
        let mut picker = make_picker();
        // January has 31 days, so day 35 should clamp to 31
        picker.set_selected_date(2025, 1, 35);
        assert_eq!(picker.selected_date(), (2025, 1, 31));

        // February in non-leap year has 28 days
        picker.set_selected_date(2025, 2, 30);
        assert_eq!(picker.selected_date(), (2025, 2, 28));
    }

    #[test]
    fn picker_set_date_handles_leap_year() {
        let mut picker = make_picker();
        // 2024 is a leap year
        picker.set_selected_date(2024, 2, 29);
        assert_eq!(picker.selected_date(), (2024, 2, 29));

        // 2025 is not a leap year - should clamp
        picker.set_selected_date(2025, 2, 29);
        assert_eq!(picker.selected_date(), (2025, 2, 28));
    }

    #[test]
    fn picker_set_date_range() {
        let mut picker = make_picker();
        picker.set_date_range(Some((2020, 1, 1)), Some((2030, 12, 31)));
        assert_eq!(picker.min_date(), Some((2020, 1, 1)));
        assert_eq!(picker.max_date(), Some((2030, 12, 31)));

        // Setting date outside range should clamp
        picker.set_selected_date(2019, 6, 15);
        assert_eq!(picker.selected_date(), (2020, 1, 1));

        picker.set_selected_date(2035, 6, 15);
        assert_eq!(picker.selected_date(), (2030, 12, 31));
    }

    #[test]
    fn picker_same_value_no_emit() {
        let mut picker = make_picker();
        let emitted = Arc::new(AtomicBool::new(false));
        let e = emitted.clone();
        picker.date_changed.connect(move |_val: Arc<(i32, u32, u32)>| {
            e.store(true, Ordering::SeqCst);
        });

        // Set same date - should NOT emit
        picker.set_selected_date(2025, 1, 1);
        assert!(!emitted.load(Ordering::SeqCst));

        // Set different date - SHOULD emit
        picker.set_selected_date(2025, 3, 15);
        assert!(emitted.load(Ordering::SeqCst));
    }

    #[test]
    fn picker_mouse_press_changes_date() {
        let mut picker = make_picker();
        // Click upper half of year column -> increment year
        picker.handle_event(&Event::MousePress {
            pos: Point::new(20, 30),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(picker.selected_date().0, 2026);

        // Click lower half of year column -> decrement year
        picker.handle_event(&Event::MousePress {
            pos: Point::new(20, 150),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(picker.selected_date().0, 2025);

        // Click upper half of month column -> increment month
        picker.handle_event(&Event::MousePress {
            pos: Point::new(100, 30),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(picker.selected_date().1, 2);

        // Click upper half of day column -> increment day
        picker.handle_event(&Event::MousePress {
            pos: Point::new(180, 30),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(picker.selected_date().2, 2);
    }

    #[test]
    fn picker_svg_output() {
        let mut picker = make_picker();
        picker.set_selected_date(2026, 7, 4);
        let svg = render_to_svg(&mut picker);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        assert!(svg.contains("width=\"240\""));
        assert!(svg.contains("height=\"200\""));
    }

    #[test]
    fn picker_disabled_blocks_events() {
        let mut picker = make_picker();
        picker.set_enabled(false);

        picker.handle_event(&Event::MousePress {
            pos: Point::new(20, 30),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(picker.selected_date().0, 2025);
    }

    /// The drum's rows are a fixed height, not a fifth of whatever the caller supplied.
    ///
    /// The defect this pins: `row_height` was `rect.height / 5`, so a 120 px cell drew five
    /// 24 px rows and the 300x200 panel drew five 60 px rows — the same wheel at two densities,
    /// with the font and the arrows tied to the row and therefore changing with it. A wheel's
    /// row height is chrome.
    #[test]
    fn the_wheel_rows_are_a_fixed_height() {
        // Tall enough for the full five rows, so the band is not clamped and the rows are
        // exactly the picker's own value.
        let mut picker = CupertinoDatePicker::new(Rect::new(0, 0, 300, 200));
        picker.set_selected_date(2025, 6, 15);
        let svg = crate::widget::svg::render_to_svg(&mut picker);
        // The rows tile the drum: 5 rows of `PICKER_ROW_HEIGHT` = 160, centred in 200.
        assert!(
            svg.contains("height=\"32\""),
            "a row must be PICKER_ROW_HEIGHT tall, not a fraction of the panel"
        );
    }

    /// N-V-01: an inverted date range must not build an empty (panicking) `RangeInclusive`.
    ///
    /// `set_date_range` takes its two ends independently, so a `max` year below the `min` year is
    /// reachable. The old `min_y..=max_y` then panicked the moment the render loop iterated it.
    #[test]
    fn inverted_date_range_renders_without_panicking() {
        let mut picker = make_picker();
        // `max` (2020) is *before* `min` (2030): an inverted request.
        picker.set_date_range(Some((2030, 1, 1)), Some((2020, 12, 31)));
        // The render loop iterates the year range; a panic here is the defect.
        let svg = crate::widget::svg::render_to_svg(&mut picker);
        assert!(svg.starts_with("<svg"));
    }

    /// N-V-02: a control narrower than three pixels must not divide by zero in `column_at`.
    #[test]
    fn a_very_narrow_picker_hit_test_does_not_divide_by_zero() {
        let mut picker = CupertinoDatePicker::new(Rect::new(0, 0, 2, 100));
        // A click anywhere would have divided by the zero column width.
        picker.handle_event(&crate::event::Event::mouse_press(1, 50, 1));
        // Reaching here without a panic is the assertion; the control also still renders.
        let svg = crate::widget::svg::render_to_svg(&mut picker);
        assert!(svg.starts_with("<svg"));
    }

    #[test]
    fn time_mode_has_two_wheels_and_emits_a_time() {
        let mut picker = CupertinoDatePicker::new(Rect::new(0, 0, 300, 200));
        picker.set_mode(CupertinoDatePickerMode::Time);
        assert_eq!(picker.mode(), CupertinoDatePickerMode::Time);
        assert_eq!(picker.selected_time(), (12, 0));
        let fired = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let f = fired.clone();
        picker.time_changed.connect(move |_: std::sync::Arc<(u32, u32)>| {
            f.store(true, std::sync::atomic::Ordering::SeqCst);
        });
        // Upper half of the first column increments the hour.
        picker.handle_event(&Event::MousePress {
            pos: Point::new(20, 40),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(picker.selected_time().0, 13);
        assert!(fired.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn a_locale_can_put_the_day_before_the_month() {
        let mut picker = CupertinoDatePicker::new(Rect::new(0, 0, 300, 200));
        picker.set_column_order(DateColumnOrder::DayMonthYear);
        assert_eq!(picker.column_order(), DateColumnOrder::DayMonthYear);
        let svg = crate::widget::svg::render_to_svg(&mut picker);
        assert!(svg.starts_with("<svg"));
    }

    #[test]
    fn dragging_a_wheel_steps_through_whole_rows() {
        let mut picker = CupertinoDatePicker::new(Rect::new(0, 0, 300, 200));
        let start = picker.selected_date();
        picker.handle_event(&Event::MousePress {
            pos: Point::new(20, 80),
            button: 1,
            modifiers: 0,
        });
        picker.handle_event(&Event::MouseMove { pos: Point::new(20, 130) });
        picker.handle_event(&Event::MouseRelease { pos: Point::new(20, 130), button: 1 });
        assert!(picker.selected_date() != start, "dragging past a row steps the wheel");
    }

    #[test]
    fn date_time_and_countdown_modes_render_their_columns() {
        let mut picker = CupertinoDatePicker::new(Rect::new(0, 0, 360, 200));
        picker.set_mode(CupertinoDatePickerMode::DateTime);
        assert_eq!(picker.column_count(), 5, "three date columns plus hour and minute");
        let svg = render_to_svg(&mut picker);
        assert!(svg.starts_with("<svg"));

        picker.set_mode(CupertinoDatePickerMode::CountDownTimer);
        assert_eq!(picker.column_count(), 3);
        picker.set_selected_second(45);
        assert_eq!(picker.selected_second(), 45);
        let svg = render_to_svg(&mut picker);
        assert!(svg.contains("45"), "the seconds wheel shows the selected second");
    }

    #[test]
    fn a_twelve_hour_clock_adds_an_ampm_column_and_minute_steps_are_honoured() {
        let mut picker = CupertinoDatePicker::new(Rect::new(0, 0, 300, 200));
        picker.set_mode(CupertinoDatePickerMode::Time);
        picker.set_selected_time(13, 7);
        assert_eq!(picker.column_count(), 2, "24-hour mode has no AM/PM wheel");
        picker.set_use_24_hour(false);
        assert_eq!(picker.column_count(), 3, "12-hour mode adds the AM/PM wheel");
        assert_eq!(picker.selected_time(), (13, 7), "display choice does not change the value");
        let svg = render_to_svg(&mut picker);
        assert!(svg.starts_with("<svg"));
        let columns = picker.time_columns();
        assert_eq!(columns.len(), 3, "hour, minute and AM/PM");
        assert_eq!(columns[2].1, vec!["AM".to_string(), "PM".to_string()]);
        assert_eq!(columns[2].0, 1, "13:07 is PM");

        picker.set_minute_interval(15);
        assert_eq!(picker.minute_interval(), 15);
        assert_eq!(picker.selected_time().1, 0, "the selection snaps into the new step");
        picker.set_selected_time(9, 59);
        assert_eq!(picker.selected_time().1, 45, "setters snap to the minute step");
    }
}
