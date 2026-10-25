// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Date-time editor widget.
//!
//! [`DateTimeEdit`] combines the calendar date of
//! [`Date`] with the wall-clock time of [`Time`].
//!
//! # Conventions
//!
//! * The calendar and clock conventions are exactly those of the two component
//!   types: the month and day are **1-based**, the hour is **24-hour**
//!   (`0` = midnight), and a whole second carries into the next minute, hour, or
//!   day.
//! * The widget's range is **inclusive at both ends**, and is compared as a
//!   single [`DateTime`], so `minimum` and `maximum` each constrain the date and
//!   the time together.
//! * [`DateTimeEdit::set_datetime`] rejects an invalid or out-of-range value
//!   silently rather than clamping it.
//! * Ordering is by date first, then by time.
use core::fmt::Write as _;

use crate::core::{Color, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::shortcut::Key;
use crate::signal::{GenericSignal, Signal1};
use crate::undo::{UndoStack, ValueCommand};
use crate::widget::advanced_widgets::{date_edit::Date, time_edit::Time};
use crate::widget::capability::coercion::{
    expect_bool, expect_datetime, expect_horizontal_alignment, expect_string,
    horizontal_alignment_to_str,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::input_widgets::shortcut_editor::primary_shortcut_pressed;
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use std::cell::RefCell;
use std::rc::Rc;

/// Combined date-time value.
///
/// This is a plain pair: the fields are public, so they can be replaced
/// directly and no validation happens on assignment. Call [`DateTime::is_valid`]
/// to check the combination, or use [`DateTimeEdit::set_datetime`], which does
/// that check for you.
///
/// `Ord` compares the date first and the time only when the dates are equal, so
/// the ordering is chronological.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DateTime {
    /// Calendar part; see [`Date`] for the 1-based month/day convention.
    pub date: Date,
    /// Time-of-day part; see [`Time`] for the 24-hour convention.
    pub time: Time,
}
impl DateTime {
    /// Combines `date` and `time` without validating either.
    pub fn new(date: Date, time: Time) -> Self {
        Self { date, time }
    }
    /// Returns `true` when both the date and the time are individually valid.
    ///
    /// Because [`Time`] clamps on construction, in practice this reduces to
    /// [`Date::is_valid`].
    pub fn is_valid(&self) -> bool {
        self.date.is_valid() && self.time.is_valid()
    }
}
/// Formats the value as `"<date> <time>"`, i.e. `YYYY-MM-DD HH:MM:SS`.
///
/// The two components use their own `Display` implementations, so the fields
/// are zero-padded and the millisecond component is omitted. This is the
/// spelling [`crate::widget::capability::coercion::expect_datetime`] parses
/// back, split on the first space; the round-trip loses any non-zero
/// milliseconds, exactly as the components' own round-trips do.
impl std::fmt::Display for DateTime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.date, self.time)
    }
}
/// One editable component of a date-time field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateTimeSection {
    /// The year component.
    Year,
    /// The month component.
    Month,
    /// The day component.
    Day,
    /// The hour component.
    Hour,
    /// The minute component.
    Minute,
    /// The second component.
    Second,
}

impl DateTimeSection {
    /// The sections in reading order.
    const ORDER: [Self; 6] =
        [Self::Year, Self::Month, Self::Day, Self::Hour, Self::Minute, Self::Second];

    /// The next section, wrapping.
    pub const fn next(self) -> Self {
        match self {
            Self::Year => Self::Month,
            Self::Month => Self::Day,
            Self::Day => Self::Hour,
            Self::Hour => Self::Minute,
            Self::Minute => Self::Second,
            Self::Second => Self::Year,
        }
    }

    /// The previous section, wrapping.
    pub const fn previous(self) -> Self {
        match self {
            Self::Year => Self::Second,
            Self::Month => Self::Year,
            Self::Day => Self::Month,
            Self::Hour => Self::Day,
            Self::Minute => Self::Hour,
            Self::Second => Self::Minute,
        }
    }

    /// The index in [`Self::ORDER`].
    const fn index(self) -> usize {
        match self {
            Self::Year => 0,
            Self::Month => 1,
            Self::Day => 2,
            Self::Hour => 3,
            Self::Minute => 4,
            Self::Second => 5,
        }
    }

    /// Digits the section collects before the buffer is trimmed.
    const fn digit_capacity(self) -> usize {
        match self {
            Self::Year => 4,
            _ => 2,
        }
    }
}

/// Date-time editor widget.
///
/// Shows a combined date and time and allows them to be changed one second at a
/// time (see [`DateTimeEdit::step_up`] / [`DateTimeEdit::step_down`]), one
/// component at a time via [`DateTimeEdit::set_date`] / [`DateTimeEdit::set_time`],
/// or through the keyboard (Up/Down step, Ctrl+Z/Ctrl+Y undo and redo).
///
/// The accepted range defaults to `1752-09-14 00:00:00.000 ..=
/// 9999-12-31 23:59:59.999` and is inclusive at both ends.
///
/// # Display format
///
/// [`DateTimeEdit::display_format`] holds a format pattern string that is
/// exposed as a property and is interpreted when the widget paints, through the
/// shared [`super::date_edit::format_with_pattern_into`] substituter. A pattern
/// with no recognised token falls back to `YYYY-MM-DD HH:MM:SS`.
/// `YYYY-MM-DD HH:MM:SS` spelling. The pattern is stored and round-tripped, not
/// yet applied when painting.
pub struct DateTimeEdit {
    base: BaseWidget,
    datetime: DateTime,
    minimum: DateTime,
    maximum: DateTime,
    display_format: String,
    /// Whether the field shows the calendar affordance (a 16 px arrow zone).
    calendar_popup: bool,
    /// Whether the month grid is currently showing.
    calendar_open: bool,
    /// Emitted with the new open state when the calendar grid is shown or hidden.
    pub popup_visibility_changed: Signal1<bool>,
    /// How the field's value is aligned within its own box.
    ///
    /// Horizontal only: the value is centred vertically in the field, so a `top`/`bottom` value
    /// would be one this control could never honour —
    /// [`crate::widget::capability::coercion::expect_horizontal_alignment`] refuses those rather
    /// than accepting a write that does nothing. Defaults to left.
    alignment: crate::core::Alignment,
    /// Emitted with the new value after every accepted change, including changes
    /// produced by [`DateTimeEdit::undo`] and [`DateTimeEdit::redo`]. Not emitted
    /// when a change is rejected or when the value is already the requested one.
    pub datetime_changed: Signal1<DateTime>,
    /// Emitted with the new date part when it changes, so a subscriber interested only in
    /// the date does not have to filter `datetime_changed` payloads.
    pub date_changed: Signal1<Date>,
    /// Emitted with the new time part when it changes, the time counterpart of
    /// [`Self::date_changed`].
    pub time_changed: Signal1<Time>,
    /// Emitted when editing is committed: Enter, Escape, or focus leaving the field.
    pub editing_finished: GenericSignal,
    undo_stack: UndoStack,
    history_target: Rc<RefCell<DateTime>>,
    restoring_history: bool,
    /// The section the user is editing, or `None` for whole-second stepping.
    current_section: Option<DateTimeSection>,
    /// Digits typed into the active section.
    digit_buffer: String,
    /// Whether the value paints a spin-button pair.
    spin_buttons: bool,
    /// The calendar grid's reveal.
    calendar_reveal: crate::widget::menu_toolbar::popup_reveal::PopupReveal,
    /// Text painted instead of the value at the minimum.
    special_value_text: Option<String>,
}
impl DateTimeEdit {
    /// Creates a date-time editor occupying `geometry`.
    ///
    /// The initial value is midnight on [`Date::today`], the accepted range is
    /// `1752-09-14 00:00:00.000 ..= 9999-12-31 23:59:59.999` (inclusive), the
    /// display format is `"yyyy-MM-dd HH:mm:ss"`, and the calendar popup is
    /// disabled.
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
            calendar_open: false,
            popup_visibility_changed: Signal1::new(),
            alignment: crate::core::Alignment::Left,
            datetime_changed: Signal1::new(),
            date_changed: Signal1::new(),
            time_changed: Signal1::new(),
            editing_finished: GenericSignal::new(),
            undo_stack: UndoStack::new(),
            history_target: Rc::new(RefCell::new(now)),
            restoring_history: false,
            current_section: None,
            digit_buffer: String::new(),
            spin_buttons: false,
            calendar_reveal: crate::widget::menu_toolbar::popup_reveal::PopupReveal::new(false),
            special_value_text: None,
        }
    }
    /// Returns the current combined value.
    pub fn datetime(&self) -> DateTime {
        self.datetime
    }
    /// How the field's value is aligned within its own box.
    pub fn alignment(&self) -> crate::core::Alignment {
        self.alignment
    }

    /// Sets how the field's value is aligned within its own box.
    ///
    /// Horizontal only. A `top`/`bottom` alignment is **ignored**, because the value is centred
    /// vertically in the field by the field's own layout — the property route refuses it through
    /// [`crate::widget::capability::coercion::expect_horizontal_alignment`], and this setter
    /// matching that keeps the two entry points from disagreeing.
    pub fn set_alignment(&mut self, alignment: crate::core::Alignment) {
        if alignment.to_horizontal().is_none() || self.alignment == alignment {
            return;
        }
        self.alignment = alignment;
        self.base.request_redraw();
    }
    /// Returns the date part of the current value.
    pub fn date(&self) -> Date {
        self.datetime.date
    }
    /// Returns the time part of the current value.
    pub fn time(&self) -> Time {
        self.datetime.time
    }
    /// Returns the inclusive lower bound accepted by
    /// [`DateTimeEdit::set_datetime`].
    ///
    /// Defaults to `1752-09-14 00:00:00.000`.
    pub fn minimum_datetime(&self) -> DateTime {
        self.minimum
    }
    /// Returns the inclusive upper bound accepted by
    /// [`DateTimeEdit::set_datetime`].
    ///
    /// Defaults to `9999-12-31 23:59:59.999`.
    pub fn maximum_datetime(&self) -> DateTime {
        self.maximum
    }
    /// Returns the stored display-format pattern.
    ///
    /// The pattern is applied by the next paint; see
    /// [`super::date_edit::format_with_pattern_into`] for the token table.
    pub fn display_format(&self) -> &str {
        &self.display_format
    }
    /// Returns whether the calendar affordance is enabled.
    ///
    /// Defaults to `false`. This is the affordance flag, not the open state; see
    /// [`DateTimeEdit::calendar_open`].
    pub fn calendar_popup(&self) -> bool {
        self.calendar_popup
    }
    /// Returns whether the month grid is currently showing.
    pub fn calendar_open(&self) -> bool {
        self.calendar_open
    }
    /// Sets the combined date-time, subject to validity and the accepted range.
    ///
    /// The assignment happens only when `dt` satisfies [`DateTime::is_valid`],
    /// falls within `minimum ..= maximum` (both inclusive), and differs from the
    /// current value; otherwise the call is a no-op and the previous value is
    /// kept. Out-of-range input is therefore **rejected, not clamped**.
    ///
    /// # Side effects
    ///
    /// On a successful change, an undo entry is pushed (unless this call comes
    /// from [`DateTimeEdit::undo`] / [`DateTimeEdit::redo`]), `datetime_changed`
    /// is emitted with the new value, and a redraw is requested.
    pub fn set_datetime(&mut self, dt: DateTime) {
        if dt.is_valid() && dt >= self.minimum && dt <= self.maximum && self.datetime != dt {
            let before = self.datetime;
            self.datetime = dt;
            if !self.restoring_history {
                *self.history_target.borrow_mut() = self.datetime;
                self.undo_stack.push(Box::new(ValueCommand::new(
                    self.history_target.clone(),
                    before,
                    self.datetime,
                    "Edit date-time",
                    "date_time_edit",
                )));
            }
            self.datetime_changed.emit(dt);
            if before.date != dt.date {
                self.date_changed.emit(dt.date);
            }
            if before.time != dt.time {
                self.time_changed.emit(dt.time);
            }
            self.base.request_redraw();
        }
    }
    /// Replaces only the date, keeping the current time part.
    ///
    /// Delegates to [`DateTimeEdit::set_datetime`], so the combined value is
    /// validated against the accepted range as a whole: a date that is in range
    /// on its own may still be rejected when the retained time pushes the
    /// combination outside `minimum ..= maximum`.
    pub fn set_date(&mut self, date: Date) {
        self.set_datetime(DateTime::new(date, self.datetime.time));
    }
    /// Replaces only the time, keeping the current date part.
    ///
    /// Delegates to [`DateTimeEdit::set_datetime`] and is subject to the same
    /// whole-value range check; see [`DateTimeEdit::set_date`].
    pub fn set_time(&mut self, time: Time) {
        self.set_datetime(DateTime::new(self.datetime.date, time));
    }
    /// Sets the inclusive lower bound for accepted values.
    ///
    /// The current value is clamped up into the new range, so the widget never holds
    /// a value below its own minimum — which would make every later
    /// [`DateTimeEdit::set_datetime`] call fail silently.
    pub fn set_minimum_datetime(&mut self, dt: DateTime) {
        if self.minimum == dt {
            return;
        }
        self.minimum = dt;
        self.clamp_to_range();
        self.base.request_redraw();
    }
    /// Sets the inclusive upper bound for accepted values.
    ///
    /// The current value is clamped down into the new range, like
    /// [`DateTimeEdit::set_minimum_datetime`] clamps up.
    pub fn set_maximum_datetime(&mut self, dt: DateTime) {
        if self.maximum == dt {
            return;
        }
        self.maximum = dt;
        self.clamp_to_range();
        self.base.request_redraw();
    }
    /// Moves the current value inside `minimum..=maximum` if it fell outside.
    ///
    /// Called by both bound setters so "the value is within the range" holds after
    /// any sequence of calls. See [`clamp_ordered_range`] for why an inverted range
    /// resolves to `minimum` — it keeps the widget usable instead of wedged at a
    /// value no write can replace.
    fn clamp_to_range(&mut self) {
        self.datetime = super::clamp_ordered_range(self.datetime, self.minimum, self.maximum);
    }
    /// Stores the display-format pattern.
    ///
    /// The pattern is applied by the next paint; see
    /// [`super::date_edit::format_with_pattern_into`] for the token table.
    pub fn set_display_format(&mut self, fmt: String) {
        if self.display_format == fmt {
            return;
        }
        self.display_format = fmt;
        self.base.request_redraw();
    }
    /// Enables or disables the calendar affordance.
    ///
    /// With it set, the field paints a 16 px arrow zone at its trailing edge; the month grid is
    /// painted only while [`DateTimeEdit::calendar_open`] is true. Disabling the affordance also
    /// closes an open grid.
    pub fn set_calendar_popup(&mut self, popup: bool) {
        if self.calendar_popup == popup {
            return;
        }
        self.calendar_popup = popup;
        if !popup {
            self.set_calendar_open(false);
        }
        self.base.request_redraw();
    }

    /// Shows or hides the month grid.
    ///
    /// Opening requires the affordance to be enabled. The new state is announced through
    /// `popup_visibility_changed` so a host can close its other popups, and a redraw is
    /// requested.
    pub fn set_calendar_open(&mut self, open: bool) {
        let open = open && self.calendar_popup;
        if self.calendar_open == open {
            return;
        }
        self.calendar_open = open;
        self.calendar_reveal.set_open(open);
        self.popup_visibility_changed.emit(open);
        self.base.request_redraw();
    }

    /// Opens the calendar if it is closed, closes it if it is open.
    ///
    /// Does nothing while the affordance is disabled.
    pub fn toggle_calendar_popup(&mut self) {
        if self.calendar_popup {
            self.set_calendar_open(!self.calendar_open);
        }
    }

    /// The section the user is editing, or `None` for whole-second stepping.
    pub fn current_section(&self) -> Option<DateTimeSection> {
        self.current_section
    }

    /// Selects the section Up/Down and typed digits apply to.
    pub fn set_current_section(&mut self, section: Option<DateTimeSection>) {
        if self.current_section == section {
            return;
        }
        self.current_section = section;
        self.digit_buffer.clear();
        self.base.request_redraw();
    }

    /// The box one section occupies inside the value text.
    ///
    /// The six zones are equal sixths of the text run. A custom `display_format` can
    /// order the components differently and this control does not parse the pattern to
    /// recover their positions, so the zones are the documented approximation; the
    /// underline and the pointer target still agree with each other.
    pub fn section_zone(&self, section: DateTimeSection) -> Rect {
        let rect = self.geometry();
        let trailing =
            if self.calendar_popup { 16u32 } else { 0 } + if self.spin_buttons { 24 } else { 0 };
        let width = rect.width.saturating_sub(12 + trailing);
        let part = width / 6;
        Rect::new(rect.x + 6 + section.index() as i32 * part as i32, rect.y, part, rect.height)
    }

    /// The section containing `pos`, when the point is on the value text.
    fn section_at(&self, pos: crate::core::Point) -> Option<DateTimeSection> {
        if !self.geometry().contains_point(pos) {
            return None;
        }
        DateTimeSection::ORDER
            .into_iter()
            .find(|section| self.section_zone(*section).contains_point(pos))
    }

    /// Shifts a date by whole days, rolling months and years.
    fn shift_date(date: &mut Date, days: i32) {
        for _ in 0..days.unsigned_abs() {
            if days > 0 {
                if date.day() < date.days_in_month() {
                    date.set_day(date.day() + 1);
                } else {
                    date.set_day(1);
                    if date.month() < 12 {
                        date.set_month(date.month() + 1);
                    } else {
                        date.set_month(1);
                        date.set_year(date.year() + 1);
                    }
                }
            } else if date.day() > 1 {
                date.set_day(date.day() - 1);
            } else {
                if date.month() > 1 {
                    date.set_month(date.month() - 1);
                } else {
                    date.set_month(12);
                    date.set_year(date.year() - 1);
                }
                date.set_day(date.days_in_month());
            }
        }
    }

    /// Steps the active section, or one whole second when no section is active.
    pub fn step_current_section(&mut self, delta: i32) {
        let Some(section) = self.current_section else {
            if delta >= 0 {
                self.step_up();
            } else {
                self.step_down();
            }
            return;
        };
        if delta == 0 {
            return;
        }
        let mut next = self.datetime;
        match section {
            DateTimeSection::Year => next.date.set_year(next.date.year() + delta),
            DateTimeSection::Month => {
                let total = next.date.year() * 12 + next.date.month() as i32 - 1 + delta;
                next.date.set_year(total.div_euclid(12));
                next.date.set_month((total.rem_euclid(12) + 1) as u8);
            }
            DateTimeSection::Day => Self::shift_date(&mut next.date, delta),
            DateTimeSection::Hour => {
                let total = next.time.hour() as i32 + delta;
                let days = total.div_euclid(24);
                next.time.set_hour(total.rem_euclid(24) as u8);
                Self::shift_date(&mut next.date, days);
            }
            DateTimeSection::Minute => {
                let total = next.time.minute() as i32 + delta;
                let carry = total.div_euclid(60);
                next.time.set_minute(total.rem_euclid(60) as u8);
                let hour_total = next.time.hour() as i32 + carry;
                let days = hour_total.div_euclid(24);
                next.time.set_hour(hour_total.rem_euclid(24) as u8);
                Self::shift_date(&mut next.date, days);
            }
            DateTimeSection::Second => {
                let total = next.time.second() as i32 + delta;
                let carry = total.div_euclid(60);
                next.time.set_second(total.rem_euclid(60) as u8);
                let minute_total = next.time.minute() as i32 + carry;
                let hour_carry = minute_total.div_euclid(60);
                next.time.set_minute(minute_total.rem_euclid(60) as u8);
                let hour_total = next.time.hour() as i32 + hour_carry;
                let days = hour_total.div_euclid(24);
                next.time.set_hour(hour_total.rem_euclid(24) as u8);
                Self::shift_date(&mut next.date, days);
            }
        }
        self.set_datetime(next);
    }

    /// Types one digit into the active section.
    pub fn type_digit(&mut self, digit: char) -> bool {
        if !digit.is_ascii_digit() {
            return false;
        }
        let Some(section) = self.current_section else {
            return false;
        };
        self.digit_buffer.push(digit);
        while self.digit_buffer.len() > section.digit_capacity() {
            self.digit_buffer.remove(0);
        }
        let Ok(value) = self.digit_buffer.parse::<i32>() else {
            return false;
        };
        let mut next = self.datetime;
        match section {
            DateTimeSection::Year => {
                if value < 1000 {
                    return true;
                }
                next.date.set_year(value);
            }
            DateTimeSection::Month => next.date.set_month(value as u8),
            DateTimeSection::Day => next.date.set_day(value as u8),
            DateTimeSection::Hour => next.time.set_hour(value as u8),
            DateTimeSection::Minute => next.time.set_minute(value as u8),
            DateTimeSection::Second => next.time.set_second(value as u8),
        }
        self.set_datetime(next);
        true
    }

    /// Whether the spin-button pair is painted.
    pub fn spin_buttons(&self) -> bool {
        self.spin_buttons
    }

    /// Shows or hides the spin-button pair. Off by default.
    pub fn set_spin_buttons(&mut self, visible: bool) {
        if self.spin_buttons == visible {
            return;
        }
        self.spin_buttons = visible;
        self.base.request_redraw();
    }

    /// The up and down spin buttons, when shown.
    pub fn spin_button_rects(&self) -> Option<(Rect, Rect)> {
        if !self.spin_buttons {
            return None;
        }
        let rect = self.geometry();
        let width = 24u32.min(rect.width);
        let x =
            rect.x + rect.width as i32 - width as i32 - if self.calendar_popup { 16 } else { 0 };
        let half = rect.height / 2;
        Some((
            Rect::new(x, rect.y, width, half),
            Rect::new(x, rect.y + half as i32, width, rect.height - half),
        ))
    }

    /// Text painted instead of the value when the value equals the minimum.
    pub fn special_value_text(&self) -> Option<&str> {
        self.special_value_text.as_deref()
    }

    /// Sets the special value text, or clears it with `None`.
    pub fn set_special_value_text(&mut self, text: Option<String>) {
        if self.special_value_text == text {
            return;
        }
        self.special_value_text = text;
        self.base.request_redraw();
    }
    /// Advances the value by one second, carrying into minutes, hours, and days.
    ///
    /// The millisecond component is preserved. The day carries into the next
    /// month or year as needed, using the length of the month being left. The
    /// result goes through [`DateTimeEdit::set_datetime`], so advancing beyond
    /// [`DateTimeEdit::maximum_datetime`] is rejected and leaves the value
    /// unchanged rather than wrapping around. A successful step is undoable.
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
    /// Moves the value back by one second, borrowing from minutes, hours, and
    /// days.
    ///
    /// The millisecond component is preserved. The day borrows into the previous
    /// month or year as needed, using the length of the month arrived at, so
    /// `2024-03-01 00:00:00` steps back to `2024-02-29 23:59:59`. The change is
    /// applied through [`DateTimeEdit::set_datetime`], so going below
    /// [`DateTimeEdit::minimum_datetime`] is rejected and leaves the value
    /// unchanged rather than wrapping around. A successful step is undoable.
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
    /// Reverts the most recent value change.
    ///
    /// Returns `true` if a change was undone, `false` when the undo stack is
    /// empty. Undoing emits `datetime_changed` with the restored value and
    /// requests a redraw, but does not push a new undo entry.
    pub fn undo(&mut self) -> bool {
        if self.undo_stack.undo().is_err() {
            return false;
        }
        self.restore_history_datetime();
        true
    }
    /// Re-applies the most recently undone value change.
    ///
    /// Returns `true` if a change was redone, `false` when there is nothing to
    /// redo. Like [`DateTimeEdit::undo`], it emits `datetime_changed` and
    /// requests a redraw without recording a new undo entry.
    pub fn redo(&mut self) -> bool {
        if self.undo_stack.redo().is_err() {
            return false;
        }
        self.restore_history_datetime();
        true
    }
    /// Returns `true` when there is at least one value change to undo.
    pub fn can_undo(&self) -> bool {
        self.undo_stack.can_undo()
    }
    /// Returns `true` when there is at least one undone value change to redo.
    pub fn can_redo(&self) -> bool {
        self.undo_stack.can_redo()
    }
    fn restore_history_datetime(&mut self) {
        self.restoring_history = true;
        self.datetime = *self.history_target.borrow();
        self.restoring_history = false;
        self.datetime_changed.emit(self.datetime);
        self.base.request_redraw();
    }
}
impl Widget for DateTimeEdit {
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
        crate::core::Size::new(160, 28)
    }

    /// Advances the calendar popup's reveal; `true` while it owes frames.
    fn tick(&mut self, delta_ms: u32) -> bool {
        self.calendar_reveal.tick(delta_ms)
    }

    /// Whether the calendar popup is mid-reveal.
    fn is_animating(&self) -> bool {
        self.calendar_reveal.is_animating()
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
            "datetime_changed" => {
                Some(EventSignalRef::mapped("datetime_changed", &self.datetime_changed, |v| {
                    CapabilityValue::String(format!("{v:?}"))
                }))
            }
            "date_changed" => {
                Some(EventSignalRef::mapped("date_changed", &self.date_changed, |v| {
                    CapabilityValue::String(format!("{v:?}"))
                }))
            }
            "time_changed" => {
                Some(EventSignalRef::mapped("time_changed", &self.time_changed, |v| {
                    CapabilityValue::String(format!("{v:?}"))
                }))
            }
            "editing_finished" => {
                Some(EventSignalRef::unit("editing_finished", &self.editing_finished))
            }
            "popup_visibility_changed" => Some(EventSignalRef::mapped(
                "popup_visibility_changed",
                &self.popup_visibility_changed,
                |v| CapabilityValue::Bool(*v),
            )),
            _ => None,
        }
    }
}

/// `DateTimePicker`'s property contract (the control is `DateTimeEdit`;
/// `DateTimePicker` is a type alias for it, so this single impl covers both
/// names).
///
/// The read path matches the old dispatch exactly: `datetime` is published
/// through the value's `Display`, and the schema declares all three properties
/// non-writable, so only the base helpers answer on write.
impl WidgetProperties for DateTimeEdit {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "datetime" => Ok(CapabilityValue::String(self.datetime().to_string())),
            "display_format" => Ok(CapabilityValue::String(self.display_format().to_string())),
            "calendar_popup" => Ok(CapabilityValue::Bool(self.calendar_popup())),
            "alignment" => Ok(CapabilityValue::String(
                horizontal_alignment_to_str(self.alignment()).to_string(),
            )),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "datetime" => {
                self.set_datetime(expect_datetime(value)?);
                Ok(())
            }
            "display_format" => {
                self.set_display_format(expect_string(value)?);
                Ok(())
            }
            "calendar_popup" => {
                self.set_calendar_popup(expect_bool(value)?);
                Ok(())
            }
            "alignment" => {
                self.set_alignment(expect_horizontal_alignment(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "datetime",
            "display_format",
            "calendar_popup",
            "alignment",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl EventHandler for DateTimeEdit {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button, .. } if *button == 1 => {
                let field = self.geometry();
                if let Some((up, down)) = self.spin_button_rects() {
                    if up.contains_point(*pos) {
                        self.step_current_section(1);
                        return;
                    }
                    if down.contains_point(*pos) {
                        self.step_current_section(-1);
                        return;
                    }
                }
                // The arrow is inside the field's own box, so it is checked before anything
                // else that treats the field as the value's display area.
                if self.calendar_popup
                    && super::date_edit::calendar_arrow_rect(field).contains_point(*pos)
                {
                    self.toggle_calendar_popup();
                } else if self.calendar_open {
                    // The cell geometry is the same derivation `draw_month_grid` paints with.
                    // `set_date` keeps the current time part, so a date pick cannot silently
                    // reset the user's time.
                    if let Some(day) =
                        super::date_edit::calendar_day_at(field, &self.datetime.date, *pos)
                    {
                        self.set_date(day);
                        self.set_calendar_open(false);
                    }
                } else if let Some(section) = self.section_at(*pos) {
                    self.set_current_section(Some(section));
                }
            }
            Event::Wheel { delta, .. } => {
                if delta.y > 0 {
                    self.step_current_section(1);
                } else if delta.y < 0 {
                    self.step_current_section(-1);
                }
            }
            Event::FocusLost => {
                self.editing_finished.emit();
            }
            Event::KeyPress { key, modifiers } => match *key {
                // Escape closes an open grid first, before any value navigation runs.
                27 if self.calendar_open => self.set_calendar_open(false),
                13 => {
                    // Enter commits the edited value, which is where a field's edit ends.
                    self.editing_finished.emit();
                }
                // Undo and redo are the platform primary chord: Control+Z/Y on
                // Windows and Linux, Command+Z/Y on macOS. A shifted chord is a
                // different binding and stays unbound here.
                _ if primary_shortcut_pressed(*key, *modifiers, Key::Z) => {
                    let _ = self.undo();
                }
                _ if primary_shortcut_pressed(*key, *modifiers, Key::Y) => {
                    let _ = self.redo();
                }
                38 => self.step_current_section(1),
                40 => self.step_current_section(-1),
                37 => {
                    if let Some(section) = self.current_section {
                        let previous = section.previous();
                        self.set_current_section(Some(previous));
                    }
                }
                39 => {
                    if let Some(section) = self.current_section {
                        let next = section.next();
                        self.set_current_section(Some(next));
                    }
                }
                48..=57 if self.current_section.is_some() => {
                    if let Some(digit) = char::from_u32(*key) {
                        self.type_digit(digit);
                    }
                }
                _ => { /* Other keys are not relevant */ }
            },
            _ => { /* Other events are not relevant */ }
        }
    }
}
impl Draw for DateTimeEdit {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();

        // Chrome colours resolve explicit style first, then the theme's resolved style
        // for this control, and only then fall back to a literal. Without the theme step
        // a light/dark switch changed nothing on screen: the field, its border and its
        // text were all hardcoded, which the rendering census reported as theme-blind.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global
        // manager's mutex is not re-entrant. `date_time_edit` classifies as `Input`, whose
        // resolved style supplies `text_color` and `border_color` but no
        // `background_color` (ThemeRole::Input resolves that to `None`), so the interior
        // is derived below from the theme's own background rather than left unset.
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("date_time_edit");
        // Read as its own lock acquisition and copied out as values, so the guard is
        // dropped before anything else touches the theme.
        let (window_fill, foreground) = {
            let manager = crate::style::theme_manager();
            match manager.current_theme() {
                Some(active) => (active.colors.background, active.colors.foreground),
                None => (Color::rgb(240, 240, 240), Color::BLACK),
            }
        };

        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(foreground);
        // An editable field's interior: one step from the window fill toward the text
        // colour, so it reads as a field on a light theme and on a dark one, and is never
        // byte-identical to the window behind it.
        let field = window_fill.blend(&ink, 0.08);
        // The filter is on the **resolved** value, not only on the theme's: the active
        // theme is applied to every control before it is drawn, so `style.background_color`
        // already holds the resolved fill and letting it through unfiltered is exactly the
        // invisible-field defect this guards against. A caller's own colour still wins.
        let surface = match style.background_color {
            Some(resolved) if resolved != window_fill => resolved,
            _ => field,
        };
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .filter(|resolved| *resolved != surface)
            .unwrap_or_else(|| surface.blend(&ink, 0.35));

        context.face_with_gradient(
            rect,
            surface,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().border_radius.unwrap_or(0),
            Color::BLACK,
        );
        context.draw_rect(rect, border);
        // The value is spelled by the caller's `display_format` when that pattern is one this control
        // can render, and by `DateTime`'s own spelling otherwise — the same rule and the same shared
        // substituter as `date_edit` and `time_edit`, so the same pattern cannot render three ways.
        // Into the buffer directly: the allocating wrapper built the result plus one string per token.
        let mut text = String::new();
        let special = self.special_value_text.as_deref().filter(|_| self.datetime == self.minimum);
        if let Some(special) = special {
            text.push_str(special);
        } else if !super::date_edit::format_with_pattern_into(
            &self.display_format,
            super::date_edit::DateTimeComponents {
                year: Some(self.datetime.date.year()),
                month: Some(self.datetime.date.month()),
                day: Some(self.datetime.date.day()),
                hour: Some(self.datetime.time.hour()),
                minute: Some(self.datetime.time.minute()),
                second: Some(self.datetime.time.second()),
                millisecond: Some(self.datetime.time.msec()),
            },
            &mut text,
        ) {
            text.clear();
            let _ = write!(text, "{}", self.datetime);
        }
        // Vertically centred through the shared primitive: `rect.y + height / 2` puts the
        // glyph box's top edge on the field's middle line, so the value sat half a line low.
        // The **effective font** — the resolved theme/caller font — so the value honours the theme
        // body font and the user's text scale (D09-STYLE-01).
        let font = effective_font(style);
        let line = context.text_line(rect, font);
        // The arrow zone is part of the field's chrome, so the value must not run under it.
        let text_width = rect
            .width
            .saturating_sub(12)
            .saturating_sub(if self.calendar_popup {
                super::date_edit::CALENDAR_ARROW_W
            } else {
                0
            })
            .saturating_sub(if self.spin_buttons { 24 } else { 0 });
        context.draw_text_fitted_cow(
            Rect { x: rect.x + 6, y: line.y, width: text_width, height: line.height },
            text.as_str(),
            font,
            ink,
            self.alignment.to_horizontal().unwrap_or(HorizontalAlignment::Left),
        );

        if let Some(section) = self.current_section {
            let zone = self.section_zone(section);
            let y = line.y + line.height as i32;
            context.draw_line(
                Point::new(zone.x + 1, y),
                Point::new(zone.x + zone.width as i32 - 1, y),
                ink,
            );
        }

        if self.calendar_popup {
            super::date_edit::draw_calendar_arrow(context, rect, ink);
        }
        if let Some((up, down)) = self.spin_button_rects() {
            let fill = surface.blend(&ink, 0.08);
            context.fill_rect(up, fill);
            context.fill_rect(down, fill);
            context.draw_rect(up, border);
            context.draw_rect(down, border);
            let side = up.width.min(up.height).min(10);
            crate::widget::draw_icon_centered(
                context,
                up,
                side,
                ink,
                crate::widget::IconName::ChevronUp,
            );
            crate::widget::draw_icon_centered(
                context,
                down,
                side,
                ink,
                crate::widget::IconName::ChevronDown,
            );
        }
        if self.calendar_open || !self.calendar_reveal.is_closed() {
            let popup = super::date_edit::calendar_popup_rect(rect);
            let revealed = self.calendar_reveal.revealed(
                popup,
                crate::widget::menu_toolbar::popup_reveal::RevealDirection::Down,
                crate::widget::menu_toolbar::popup_reveal::PopupReveal::travel_cap_for(Size::new(
                    popup.width,
                    popup.height,
                )),
            );
            let clipped = !self.calendar_reveal.is_closed();
            if clipped {
                context.push_clip(revealed.x, revealed.y, revealed.width, revealed.height);
            }
            self.draw_calendar_popup(context, rect, surface, border, ink);
            if clipped {
                context.pop_clip();
            }
        }
    }
}

impl DateTimeEdit {
    /// Paints the month grid for the current value's month, below the field.
    ///
    /// # Why the grid is here and not a child widget
    ///
    /// It is a **function of the field's own state**: the month comes from `self.datetime.date`, the
    /// muted cells from `self.minimum`/`self.maximum`, and the marked cell from the same date. A child
    /// would have to be kept in step with all three at every mutation, and the flag is published as a
    /// boolean, so there is nothing for a caller to hold anyway. Same shape as `date_edit`'s popup,
    /// because the grid itself is the shared [`super::date_edit::draw_month_grid`].
    fn draw_calendar_popup(
        &self,
        context: &mut RenderContext,
        field: Rect,
        surface: Color,
        border: Color,
        ink: Color,
    ) {
        super::date_edit::draw_month_grid(
            context,
            field,
            super::date_edit::GridChrome { surface, border, ink },
            &self.datetime.date,
            &self.minimum.date,
            &self.maximum.date,
            effective_font(self.base.style()),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Point;
    use crate::widget::input_widgets::shortcut_editor::{
        non_primary_modifier_event_bits, primary_modifier_event_bits,
    };
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

    /// The calendar popup is **painted**, not merely stored.
    ///
    /// # The defect this pins
    ///
    /// `calendar_popup` was stored, published, settable, and read by nothing -- the exact defect
    /// `date_edit` had, in its sibling control. The getter and setter and the schema row all existed,
    /// and setting it redrew the same field.
    ///
    /// The assertion reads the **document**, not the accessor: a model-only round-trip (`set` then
    /// `get`) is exactly what the broken version already passed. And it checks that the ink is the
    /// *month's days*, because "the documents differ" alone is satisfied by any extra element.
    #[test]
    fn the_calendar_popup_is_actually_painted() {
        use crate::widget::svg::{render_to_svg, text_subpath_count};
        let _theme_guard = crate::style::theme_test_guard();

        let make = |popup: bool| {
            let mut editor = DateTimeEdit::new(Rect::new(0, 0, 280, 30));
            editor.set_datetime(DateTime::new(Date::new(2026, 6, 8), Time::new(12, 30, 45, 0)));
            editor.set_calendar_popup(popup);
            if popup {
                editor.set_calendar_open(true);
            }
            editor
        };

        let mut closed = make(false);
        let closed_svg = render_to_svg(&mut closed);
        let mut open = make(true);
        let open_svg = render_to_svg(&mut open);

        assert_ne!(closed_svg, open_svg, "opening the popup must change the picture");
        // The grid is the weekday row plus the month's days: 7 + 30 for June, against the single
        // `2026-06-08 12:30:45` run the closed field paints. The bound is stated as "at least the
        // month's day count" rather than as a ratio, so it names the thing that must be there.
        let closed_ink = text_subpath_count(&closed_svg);
        let open_ink = text_subpath_count(&open_svg);
        assert!(
            open_ink > closed_ink,
            "the popup must add ink: closed={closed_ink} open={open_ink}"
        );
        assert!(
            open_ink >= 30,
            "the popup must paint the month's days, not a stub: open={open_ink}"
        );
        assert!(
            open_svg.matches("<rect").count() > closed_svg.matches("<rect").count(),
            "the popup paints its own plate: {open_svg}"
        );
    }

    /// The affordance flag alone must not paint the grid.
    #[test]
    fn calendar_popup_alone_does_not_paint_the_grid() {
        use crate::widget::svg::{render_to_svg, text_subpath_count};
        let _theme_guard = crate::style::theme_test_guard();

        let mut plain = DateTimeEdit::new(Rect::new(0, 0, 280, 30));
        plain.set_datetime(DateTime::new(Date::new(2026, 6, 8), Time::new(12, 30, 45, 0)));
        let mut affordance = DateTimeEdit::new(Rect::new(0, 0, 280, 30));
        affordance.set_datetime(DateTime::new(Date::new(2026, 6, 8), Time::new(12, 30, 45, 0)));
        affordance.set_calendar_popup(true);

        assert!(!affordance.calendar_open(), "the flag alone must not open the grid");
        let plain_svg = render_to_svg(&mut plain);
        let affordance_svg = render_to_svg(&mut affordance);
        assert_ne!(plain_svg, affordance_svg, "the arrow affordance itself must be painted");
        assert_eq!(
            text_subpath_count(&affordance_svg),
            text_subpath_count(&plain_svg),
            "no weekday/day runs may appear until the grid is open"
        );
    }

    /// Clicking the arrow opens the grid and announces the new state.
    #[test]
    fn arrow_click_emits_popup_visibility_changed_and_paints_the_grid() {
        use crate::widget::svg::{render_to_svg, text_subpath_count};
        let _theme_guard = crate::style::theme_test_guard();

        let mut editor = DateTimeEdit::new(Rect::new(0, 0, 280, 30));
        editor.set_datetime(DateTime::new(Date::new(2026, 6, 8), Time::new(12, 30, 45, 0)));
        editor.set_calendar_popup(true);

        let seen = Arc::new(Mutex::new(Vec::<bool>::new()));
        let sink = Arc::clone(&seen);
        editor.popup_visibility_changed.connect(move |open| {
            if let Ok(mut v) = sink.lock() {
                v.push(*open);
            }
        });

        let closed_svg = render_to_svg(&mut editor);
        let arrow = super::super::date_edit::calendar_arrow_rect(editor.geometry()).center();
        editor.handle_event(&Event::mouse_press(arrow.x, arrow.y, 1));

        assert!(editor.calendar_open(), "the arrow press must open the grid");
        assert_eq!(*seen.lock().expect("visibility lock"), vec![true]);
        let open_svg = render_to_svg(&mut editor);
        assert!(
            text_subpath_count(&open_svg) > text_subpath_count(&closed_svg),
            "the open grid must add the month's day runs"
        );
    }

    /// Choosing a day updates the date part and preserves the time part.
    #[test]
    fn selecting_a_day_updates_the_date_and_keeps_the_time() {
        let mut editor = DateTimeEdit::new(Rect::new(0, 0, 280, 30));
        editor.set_datetime(DateTime::new(Date::new(2026, 6, 8), Time::new(14, 30, 15, 0)));
        editor.set_calendar_popup(true);
        editor.set_calendar_open(true);

        let cell = super::super::date_edit::calendar_cell_rect(
            editor.geometry(),
            &editor.datetime.date,
            15,
        )
        .expect("June 15 has a grid cell")
        .center();
        editor.handle_event(&Event::mouse_press(cell.x, cell.y, 1));

        assert_eq!(editor.date(), Date::new(2026, 6, 15));
        assert_eq!(editor.time(), Time::new(14, 30, 15, 0), "the time part must survive");
        assert!(!editor.calendar_open(), "picking a day finishes the interaction");
    }

    /// Escape closes an open grid before any value navigation runs.
    #[test]
    fn escape_closes_the_calendar_first() {
        let mut editor = DateTimeEdit::new(Rect::new(0, 0, 280, 30));
        let value = DateTime::new(Date::new(2026, 6, 8), Time::new(14, 30, 15, 0));
        editor.set_datetime(value);
        editor.set_calendar_popup(true);
        editor.set_calendar_open(true);

        editor.handle_event(&Event::KeyPress { key: 27, modifiers: 0 });

        assert!(!editor.calendar_open());
        assert_eq!(editor.datetime(), value, "closing must not change the value");
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
    fn date_time_edit_undo_redo_restores_datetime() {
        let mut editor = DateTimeEdit::new(Rect::new(0, 0, 280, 30));
        let first = DateTime::new(Date::new(2026, 6, 8), Time::new(10, 0, 0, 0));
        let second = DateTime::new(Date::new(2026, 6, 8), Time::new(10, 0, 1, 0));
        editor.set_datetime(first);
        editor.set_datetime(second);

        assert!(editor.can_undo());
        assert!(editor.undo());
        assert_eq!(editor.datetime(), first);
        assert!(editor.can_redo());
        assert!(editor.redo());
        assert_eq!(editor.datetime(), second);
    }

    #[test]
    fn date_time_edit_keyboard_shortcuts_drive_history() {
        let mut editor = DateTimeEdit::new(Rect::new(0, 0, 280, 30));
        let first = DateTime::new(Date::new(2026, 6, 8), Time::new(10, 0, 0, 0));
        let second = DateTime::new(Date::new(2026, 6, 9), Time::new(11, 0, 0, 0));
        editor.set_datetime(first);
        editor.set_datetime(second);

        editor.handle_event(&Event::KeyPress { key: 90, modifiers: primary_modifier_event_bits() });
        assert_eq!(editor.datetime(), first);
        editor.handle_event(&Event::KeyPress { key: 89, modifiers: primary_modifier_event_bits() });
        assert_eq!(editor.datetime(), second);
    }

    #[test]
    fn date_time_edit_primary_shortcut_and_shifted_chord() {
        let mut editor = DateTimeEdit::new(Rect::new(0, 0, 280, 30));
        let first = DateTime::new(Date::new(2026, 6, 8), Time::new(10, 0, 0, 0));
        let second = DateTime::new(Date::new(2026, 6, 9), Time::new(11, 0, 0, 0));
        editor.set_datetime(first);
        editor.set_datetime(second);

        let primary = primary_modifier_event_bits();
        // Shift+Primary+Z is a different chord, so it is not plain undo.
        editor.handle_event(&Event::key_press(90, primary | 0b0001));
        assert_eq!(editor.datetime(), second);

        // Primary+Z / Primary+Y are undo / redo on this host.
        editor.handle_event(&Event::key_press(90, primary));
        assert_eq!(editor.datetime(), first);
        editor.handle_event(&Event::key_press(89, primary));
        assert_eq!(editor.datetime(), second);

        // The accelerator that is not primary on this host does not undo.
        editor.handle_event(&Event::key_press(90, non_primary_modifier_event_bits()));
        assert_eq!(editor.datetime(), second);
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

    /// Sections cover every component; typing edits the active one and stepping wraps it.
    #[test]
    fn date_time_sections_edit_each_component() {
        let mut editor = DateTimeEdit::new(Rect::new(0, 0, 280, 30));
        editor.set_datetime(DateTime::new(Date::new(2026, 6, 15), Time::new(10, 0, 0, 0)));
        assert!(!editor.type_digit('2'), "no section, no typing");

        editor.set_current_section(Some(DateTimeSection::Hour));
        assert!(editor.type_digit('2'));
        assert!(editor.type_digit('3'));
        assert_eq!(editor.time().hour(), 23);
        editor.step_current_section(1);
        assert_eq!(editor.time().hour(), 0, "the hour wraps");
        assert_eq!(editor.date(), Date::new(2026, 6, 16), "and carries the day");

        editor.set_current_section(Some(DateTimeSection::Month));
        editor.step_current_section(1);
        assert_eq!(editor.date().month(), 7);
        editor.set_current_section(Some(DateTimeSection::Second));
        editor.step_current_section(60);
        assert_eq!(editor.time().minute(), 1, "seconds carry into minutes");
    }

    /// Spin buttons and the wheel are opt-in and step the value.
    #[test]
    fn date_time_spin_buttons_and_wheel_step_the_value() {
        let mut editor = DateTimeEdit::new(Rect::new(0, 0, 280, 30));
        editor.set_datetime(DateTime::new(Date::new(2026, 6, 15), Time::new(10, 0, 0, 0)));
        assert!(editor.spin_button_rects().is_none());
        editor.set_spin_buttons(true);
        let (up, _) = editor.spin_button_rects().expect("the pair exists");
        editor.handle_event(&Event::mouse_press(up.x + 2, up.y + 2, 1));
        assert_eq!(editor.time().second(), 1);

        editor.handle_event(&Event::wheel(0, 1, 0));
        assert_eq!(editor.time().second(), 2);
        editor.handle_event(&Event::wheel(0, -1, 0));
        assert_eq!(editor.time().second(), 1);
    }

    /// Clicking a zone picks its section, and the special value text is opt-in.
    #[test]
    fn date_time_zones_and_special_text() {
        let mut editor = DateTimeEdit::new(Rect::new(0, 0, 280, 30));
        let minute = editor.section_zone(DateTimeSection::Minute);
        editor.handle_event(&Event::mouse_press(minute.x + 1, minute.y + 2, 1));
        assert_eq!(editor.current_section(), Some(DateTimeSection::Minute));

        let before = render_to_svg(&mut editor);
        editor.set_datetime(editor.minimum_datetime());
        let at_minimum = render_to_svg(&mut editor);
        editor.set_special_value_text(Some("Unset".to_string()));
        let after = render_to_svg(&mut editor);
        assert_ne!(at_minimum, after, "the special text must replace the value at the minimum");
        assert_ne!(before, after);
    }

    /// The calendar reveal animates and settles.
    #[test]
    fn date_time_calendar_reveal_settles() {
        use crate::widget::Widget;

        let mut editor = DateTimeEdit::new(Rect::new(0, 0, 280, 30));
        editor.set_calendar_popup(true);
        editor.set_calendar_open(true);
        assert!(editor.is_animating());
        for _ in 0..120 {
            editor.tick(16);
        }
        assert!(!editor.is_animating());
    }

    #[test]
    fn date_time_and_finish_signals_split_the_change() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let mut editor = DateTimeEdit::new(Rect::new(0, 0, 280, 30));
        let dates = Arc::new(AtomicUsize::new(0));
        let times = Arc::new(AtomicUsize::new(0));
        let fin = Arc::new(AtomicUsize::new(0));
        let (d, t, f) = (Arc::clone(&dates), Arc::clone(&times), Arc::clone(&fin));
        editor.date_changed.connect(move |_| {
            d.fetch_add(1, Ordering::SeqCst);
        });
        editor.time_changed.connect(move |_| {
            t.fetch_add(1, Ordering::SeqCst);
        });
        editor.editing_finished.connect(move || {
            f.fetch_add(1, Ordering::SeqCst);
        });

        editor.set_date(Date::new(2026, 7, 4));
        assert_eq!(dates.load(Ordering::SeqCst), 1);
        assert_eq!(times.load(Ordering::SeqCst), 0);
        editor.set_time(Time::new(12, 0, 0, 0));
        assert_eq!(times.load(Ordering::SeqCst), 1);
        editor.handle_event(&Event::key_press(13, 0));
        assert_eq!(fin.load(Ordering::SeqCst), 1);
        assert!(editor.event_signal_dyn("date_changed").is_some());
        assert!(editor.event_signal_dyn("editing_finished").is_some());
    }
}
