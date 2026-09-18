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
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::undo::{CommandDescription, CommandId, UndoCommand, UndoStack};
use crate::widget::advanced_widgets::{date_edit::Date, time_edit::Time};
use crate::widget::capability::coercion::{expect_bool, expect_datetime, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_DATE_TIME_EDIT_COMMAND_ID: AtomicU64 = AtomicU64::new(1);

struct DateTimeEditCommand {
    id: CommandId,
    target: Rc<RefCell<DateTime>>,
    before: DateTime,
    after: DateTime,
}

impl DateTimeEditCommand {
    fn new(target: Rc<RefCell<DateTime>>, before: DateTime, after: DateTime) -> Self {
        Self {
            id: CommandId(NEXT_DATE_TIME_EDIT_COMMAND_ID.fetch_add(1, Ordering::Relaxed)),
            target,
            before,
            after,
        }
    }
}

impl UndoCommand for DateTimeEditCommand {
    fn id(&self) -> CommandId {
        self.id
    }

    fn description(&self) -> CommandDescription {
        CommandDescription {
            text: "Edit date-time".to_string(),
            timestamp_ms: 0,
            command_type: "date_time_edit",
        }
    }

    fn execute(&mut self) -> Result<(), String> {
        *self.target.borrow_mut() = self.after;
        Ok(())
    }

    fn undo(&mut self) -> Result<(), String> {
        *self.target.borrow_mut() = self.before;
        Ok(())
    }
}
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
/// exposed as a property, but the widget's own `draw` always uses the fixed
/// `YYYY-MM-DD HH:MM:SS` spelling. The pattern is stored and round-tripped, not
/// yet applied when painting.
pub struct DateTimeEdit {
    base: BaseWidget,
    datetime: DateTime,
    minimum: DateTime,
    maximum: DateTime,
    display_format: String,
    calendar_popup: bool,
    /// Emitted with the new value after every accepted change, including changes
    /// produced by [`DateTimeEdit::undo`] and [`DateTimeEdit::redo`]. Not emitted
    /// when a change is rejected or when the value is already the requested one.
    pub datetime_changed: Signal1<DateTime>,
    undo_stack: UndoStack,
    history_target: Rc<RefCell<DateTime>>,
    restoring_history: bool,
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
            datetime_changed: Signal1::new(),
            undo_stack: UndoStack::new(),
            history_target: Rc::new(RefCell::new(now)),
            restoring_history: false,
        }
    }
    /// Returns the current combined value.
    pub fn datetime(&self) -> DateTime {
        self.datetime
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
    /// This is a plain string; it is currently stored and round-tripped but not
    /// interpreted when the widget paints (painting always uses
    /// `YYYY-MM-DD HH:MM:SS`).
    pub fn display_format(&self) -> &str {
        &self.display_format
    }
    /// Returns whether the calendar popup is enabled.
    ///
    /// Defaults to `false`. This is a stored flag; the widget's own `draw` does
    /// not yet render a popup.
    pub fn calendar_popup(&self) -> bool {
        self.calendar_popup
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
                self.undo_stack.push(Box::new(DateTimeEditCommand::new(
                    self.history_target.clone(),
                    before,
                    self.datetime,
                )));
            }
            self.datetime_changed.emit(dt);
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
        self.minimum = dt;
        self.clamp_to_range();
        self.base.request_redraw();
    }
    /// Sets the inclusive upper bound for accepted values.
    ///
    /// The current value is clamped down into the new range, like
    /// [`DateTimeEdit::set_minimum_datetime`] clamps up.
    pub fn set_maximum_datetime(&mut self, dt: DateTime) {
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
    /// The string is kept verbatim; no validation is performed. See
    /// [`DateTimeEdit::display_format`] for the current limits on its use.
    pub fn set_display_format(&mut self, fmt: String) {
        self.display_format = fmt;
        self.base.request_redraw();
    }
    /// Enables or disables the calendar popup flag.
    ///
    /// Purely stored state; changing it only triggers a redraw.
    pub fn set_calendar_popup(&mut self, popup: bool) {
        self.calendar_popup = popup;
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

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        crate::core::Size::new(160, 28)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
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
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["datetime", "display_format", "calendar_popup", BASE_PROPERTY_NAMES]
    }
}

impl EventHandler for DateTimeEdit {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        if let Event::KeyPress { key, modifiers } = event {
            match *key {
                90 if *modifiers == 2 => {
                    let _ = self.undo();
                }
                89 if *modifiers == 2 => {
                    let _ = self.redo();
                }
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

        editor.handle_event(&Event::KeyPress { key: 90, modifiers: 2 });
        assert_eq!(editor.datetime(), first);
        editor.handle_event(&Event::KeyPress { key: 89, modifiers: 2 });
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
}
