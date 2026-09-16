// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Date editor widget.
//!
//! [`DateEdit`] stores a calendar date as a [`Date`] value and lets the user
//! step through it or set it programmatically.
//!
//! # Conventions
//!
//! * The month is **1-based** (`1` = January, `12` = December) and the day is
//!   **1-based** (`1` = the first of the month), matching both the text format
//!   and the widget's setters.
//! * The widget's range is **inclusive at both ends**: a date equal to
//!   [`DateEdit::minimum_date`] or [`DateEdit::maximum_date`] is accepted.
//! * A `Date` stores whatever it is constructed with; validity is checked
//!   separately. [`Date::is_valid`] is the predicate, and
//!   [`DateEdit::set_date`] rejects invalid or out-of-range dates silently
//!   rather than clamping them, leaving the previous value in place.
//! * The month and day setters on [`Date`] clamp to the field's numeric range
//!   (`1..=12` and `1..=31`); they do not consult the month length, so they can
//!   leave the value invalid. Use [`DateEdit::set_date`] to keep it valid.
//! * Dates are compared and ordered by year, then month, then day.
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::undo::{CommandDescription, CommandId, UndoCommand, UndoStack};
use crate::widget::capability::access::date_to_string;
use crate::widget::capability::coercion::{expect_bool, expect_date, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_DATE_EDIT_COMMAND_ID: AtomicU64 = AtomicU64::new(1);

struct DateEditCommand {
    id: CommandId,
    target: Rc<RefCell<Date>>,
    before: Date,
    after: Date,
}

impl DateEditCommand {
    fn new(target: Rc<RefCell<Date>>, before: Date, after: Date) -> Self {
        Self {
            id: CommandId(NEXT_DATE_EDIT_COMMAND_ID.fetch_add(1, Ordering::Relaxed)),
            target,
            before,
            after,
        }
    }
}

impl UndoCommand for DateEditCommand {
    fn id(&self) -> CommandId {
        self.id
    }

    fn description(&self) -> CommandDescription {
        CommandDescription {
            text: "Edit date".to_string(),
            timestamp_ms: 0,
            command_type: "date_edit",
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
/// Date value (year, month, day).
///
/// This is a plain value type, not a validated one: [`Date::new`] stores the
/// components as given, including values outside the calendar, and the month
/// and day setters clamp only to their field's numeric range. Call
/// [`Date::is_valid`] to check that the combination exists, or use the widget's
/// [`DateEdit::set_date`], which performs that check for you.
///
/// `Ord` follows chronological order: year, then month, then day.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Date {
    year: i32,
    month: u8, // 1-12
    day: u8,   // 1-31
}
impl Date {
    /// Creates a date from raw components without validation.
    ///
    /// `month` is 1-based (`1..=12`) and `day` is 1-based (`1..=31`); the year
    /// may be negative. Out-of-range components are stored as-is and reported
    /// later by [`Date::is_valid`].
    pub fn new(year: i32, month: u8, day: u8) -> Self {
        Self { year, month, day }
    }
    /// Returns the current date.
    ///
    /// # Caveat
    ///
    /// This does **not** consult the system clock: it currently returns the
    /// fixed date `2024-01-01`. Treat it as "a default date", not "today".
    /// Callers that need the real date should obtain it from the platform and
    /// construct a [`Date`] explicitly.
    pub fn today() -> Self {
        Self { year: 2024, month: 1, day: 1 }
    }
    /// Returns the year component. The year is not range-checked.
    pub fn year(&self) -> i32 {
        self.year
    }
    /// Returns the 1-based month (`1` = January).
    pub fn month(&self) -> u8 {
        self.month
    }
    /// Returns the 1-based day of the month (`1` = first).
    pub fn day(&self) -> u8 {
        self.day
    }
    /// Sets the year, accepting any `i32` including negative values.
    ///
    /// No range check is applied; whether the result is a valid date is left to
    /// [`Date::is_valid`].
    pub fn set_year(&mut self, year: i32) {
        self.year = year;
    }
    /// Sets the 1-based month, clamped into `1..=12`.
    ///
    /// Only the numeric range is enforced, so setting a month to `2` on a
    /// 31-day date leaves an invalid combination for [`Date::is_valid`] to
    /// report; the day is not adjusted.
    pub fn set_month(&mut self, month: u8) {
        self.month = month.clamp(1, 12);
    }
    /// Sets the 1-based day, clamped into `1..=31`.
    ///
    /// The month length is not considered, so the result may be invalid for the
    /// month; see [`Date::is_valid`].
    pub fn set_day(&mut self, day: u8) {
        self.day = day.clamp(1, 31);
    }
    /// Returns the number of days in this date's month.
    ///
    /// February accounts for leap years via [`Date::is_leap_year`], giving 28
    /// or 29 days. If `month` is outside `1..=12` (possible after [`Date::new`]
    /// or a direct field assignment) this returns `30`.
    pub fn days_in_month(&self) -> u8 {
        match self.month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 => {
                if self.is_leap_year() {
                    29
                } else {
                    28
                }
            }
            _ => 30,
        }
    }
    /// Returns `true` for Gregorian leap years.
    ///
    /// The rule is the proleptic Gregorian one: divisible by 4, except centuries,
    /// except millennia. It is applied to negative and zero years too, where the
    /// "divisible by" test follows Rust's remainder semantics.
    pub fn is_leap_year(&self) -> bool {
        (self.year % 4 == 0 && self.year % 100 != 0) || (self.year % 400 == 0)
    }
    /// Returns `true` when the stored combination is a real calendar date.
    ///
    /// Requires a month in `1..=12` and a day in `1..=`[`Date::days_in_month`].
    /// The year is never a reason for invalidity.
    pub fn is_valid(&self) -> bool {
        self.month >= 1 && self.month <= 12 && self.day >= 1 && self.day <= self.days_in_month()
    }
}
/// Formats the date as `YYYY-MM-DD`.
///
/// The year is zero-padded to four digits; the month and day are always two
/// digits. This is the spelling [`crate::widget::capability::coercion::expect_date`]
/// parses back, so the round-trip is lossless. The fields are emitted verbatim
/// even when the date is not a valid calendar date.
impl std::fmt::Display for Date {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}
/// Date editor widget.
///
/// Shows a date and allows it to be changed a day at a time (see
/// [`DateEdit::step_up`] / [`DateEdit::step_down`]), programmatically via
/// [`DateEdit::set_date`], or through the keyboard (Up/Down step, Ctrl+Z/Ctrl+Y
/// undo and redo).
///
/// The accepted range defaults to `1752-09-14 ..= 9999-12-31` and is inclusive
/// at both ends; see [`DateEdit::minimum_date`] and
/// [`DateEdit::maximum_date`].
///
/// # Display format
///
/// [`DateEdit::display_format`] holds a format pattern string that is exposed
/// as a property, but the widget's own `draw` and `Display` output always use
/// the fixed `YYYY-MM-DD` spelling. The pattern is stored and round-tripped, not
/// yet applied when painting.
pub struct DateEdit {
    base: BaseWidget,
    date: Date,
    minimum: Date,
    maximum: Date,
    display_format: String,
    calendar_popup: bool,
    /// Emitted with the new date after every accepted change, including changes
    /// produced by [`DateEdit::undo`] and [`DateEdit::redo`]. Not emitted when a
    /// change is rejected or when the date is already the requested value.
    pub date_changed: Signal1<Date>,
    undo_stack: UndoStack,
    history_target: Rc<RefCell<Date>>,
    restoring_history: bool,
}
impl DateEdit {
    /// Creates a date editor occupying `geometry`.
    ///
    /// The initial date is [`Date::today`], the accepted range is
    /// `1752-09-14 ..= 9999-12-31` (inclusive), the display format is
    /// `"yyyy-MM-dd"`, and the calendar popup is disabled.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::DatePicker, geometry, "DateEdit"),
            date: Date::today(),
            minimum: Date::new(1752, 9, 14),
            maximum: Date::new(9999, 12, 31),
            display_format: "yyyy-MM-dd".to_string(),
            calendar_popup: false,
            date_changed: Signal1::new(),
            undo_stack: UndoStack::new(),
            history_target: Rc::new(RefCell::new(Date::today())),
            restoring_history: false,
        }
    }
    /// Returns the current date.
    pub fn date(&self) -> Date {
        self.date
    }
    /// Returns the inclusive lower bound accepted by [`DateEdit::set_date`].
    ///
    /// Defaults to `1752-09-14`, the first date of the Gregorian calendar in
    /// this implementation's convention.
    pub fn minimum_date(&self) -> Date {
        self.minimum
    }
    /// Returns the inclusive upper bound accepted by [`DateEdit::set_date`].
    ///
    /// Defaults to `9999-12-31`.
    pub fn maximum_date(&self) -> Date {
        self.maximum
    }
    /// Returns the stored display-format pattern.
    ///
    /// This is a plain string; it is currently stored and round-tripped but not
    /// interpreted when the widget paints (painting always uses `YYYY-MM-DD`).
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
    /// Sets the current date, subject to validation and the accepted range.
    ///
    /// The assignment happens only when `date` satisfies [`Date::is_valid`],
    /// falls within `minimum ..= maximum` (both inclusive), and differs from the
    /// current date; otherwise the call is a no-op and the previous date is
    /// kept. Out-of-range input is therefore **rejected, not clamped**.
    ///
    /// # Side effects
    ///
    /// On a successful change, an undo entry is pushed (unless this call comes
    /// from [`DateEdit::undo`] / [`DateEdit::redo`]), `date_changed` is emitted
    /// with the new date, and a redraw is requested.
    pub fn set_date(&mut self, date: Date) {
        if date.is_valid() && date >= self.minimum && date <= self.maximum && self.date != date {
            let before = self.date;
            self.date = date;
            if !self.restoring_history {
                *self.history_target.borrow_mut() = self.date;
                self.undo_stack.push(Box::new(DateEditCommand::new(
                    self.history_target.clone(),
                    before,
                    self.date,
                )));
            }
            self.date_changed.emit(date);
            self.base.request_redraw();
        }
    }
    /// Sets the inclusive lower bound for accepted dates.
    ///
    /// The current date is **not** re-validated against the new bound, so the
    /// widget can be left holding a date below its own minimum. The bound
    /// itself is not validated either.
    pub fn set_minimum_date(&mut self, date: Date) {
        self.minimum = date;
        // Re-clamp the current value into the new range. Without this the widget could
        // hold a date below its own minimum, after which every `set_date` call silently
        // failed the range check — the value was stuck and the failure was invisible.
        self.clamp_to_range();
        self.base.request_redraw();
    }
    /// Sets the inclusive upper bound for accepted dates.
    ///
    /// The current date is clamped into the new range, so lowering the maximum moves
    /// a now-out-of-range value down to the boundary rather than leaving the widget
    /// holding a date it would itself reject.
    pub fn set_maximum_date(&mut self, date: Date) {
        self.maximum = date;
        self.clamp_to_range();
        self.base.request_redraw();
    }
    /// Sets both ends of the accepted range in one call.
    ///
    /// The current date is clamped into the new range. `min` is not required to be
    /// less than or equal to `max`; if the two are inverted the minimum wins, so a
    /// caller that swaps its arguments gets a working widget pinned at `min` rather
    /// than one where every subsequent [`DateEdit::set_date`] silently fails.
    pub fn set_date_range(&mut self, min: Date, max: Date) {
        self.minimum = min;
        self.maximum = max;
        self.clamp_to_range();
        self.base.request_redraw();
    }
    /// Moves the current date inside `minimum..=maximum` if it fell outside.
    ///
    /// Called by every bound setter so the invariant "the value is within the range"
    /// holds after any sequence of calls. When the range is inverted the minimum is
    /// treated as authoritative, which keeps this total (it always terminates with a
    /// value that `set_date` would accept).
    fn clamp_to_range(&mut self) {
        if self.date < self.minimum {
            self.date = self.minimum;
        } else if self.date > self.maximum {
            // Only reachable when `minimum <= maximum`; an inverted range was already
            // resolved to `minimum` by the branch above or by the comparison failing.
            self.date = if self.minimum > self.maximum { self.minimum } else { self.maximum };
        }
    }
    /// Stores the display-format pattern.
    ///
    /// The string is kept verbatim; no validation is performed. See
    /// [`DateEdit::display_format`] for the current limits on its use.
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
    /// Advances the date by one day, rolling over month and year ends.
    ///
    /// The resulting date goes through [`DateEdit::set_date`], so stepping
    /// beyond [`DateEdit::maximum_date`] is rejected and leaves the date
    /// unchanged rather than wrapping around. A successful step is undoable.
    pub fn step_up(&mut self) {
        let mut d = self.date;
        let next_day = d.day() as i32 + 1;
        if next_day > d.days_in_month() as i32 {
            d.day = 1;
            let next_month = d.month() as i32 + 1;
            if next_month > 12 {
                d.month = 1;
                d.year += 1;
            } else {
                d.month = next_month as u8;
            }
        } else {
            d.day = next_day as u8;
        }
        self.set_date(d);
    }
    /// Moves the date back by one day, rolling over month and year starts.
    ///
    /// The month length used when wrapping is derived from the target month, so
    /// `2024-03-01` steps back to `2024-02-29`. The change is applied through
    /// [`DateEdit::set_date`], so going below [`DateEdit::minimum_date`] is
    /// rejected and leaves the date unchanged rather than wrapping around. A
    /// successful step is undoable.
    pub fn step_down(&mut self) {
        let mut d = self.date;
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
        self.set_date(d);
    }
    /// Reverts the most recent date change.
    ///
    /// Returns `true` if a change was undone, `false` when the undo stack is
    /// empty. Undoing emits `date_changed` with the restored date and requests a
    /// redraw, but does not push a new undo entry.
    pub fn undo(&mut self) -> bool {
        if self.undo_stack.undo().is_err() {
            return false;
        }
        self.restore_history_date();
        true
    }
    /// Re-applies the most recently undone date change.
    ///
    /// Returns `true` if a change was redone, `false` when there is nothing to
    /// redo. Like [`DateEdit::undo`], it emits `date_changed` and requests a
    /// redraw without recording a new undo entry.
    pub fn redo(&mut self) -> bool {
        if self.undo_stack.redo().is_err() {
            return false;
        }
        self.restore_history_date();
        true
    }
    /// Returns `true` when there is at least one date change to undo.
    pub fn can_undo(&self) -> bool {
        self.undo_stack.can_undo()
    }
    /// Returns `true` when there is at least one undone date change to redo.
    pub fn can_redo(&self) -> bool {
        self.undo_stack.can_redo()
    }
    fn restore_history_date(&mut self) {
        self.restoring_history = true;
        self.date = *self.history_target.borrow();
        self.restoring_history = false;
        self.date_changed.emit(self.date);
        self.base.request_redraw();
    }
}
impl Widget for DateEdit {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        crate::core::Size::new(120, 28)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `DatePicker`'s property contract (the control is `DateEdit`; `DatePicker` is
/// a type alias for it, so this single impl covers both names).
///
/// Dates round-trip as strings: `date_to_string` on the way out and
/// `expect_date` on the way in, which validates the parsed calendar date so an
/// impossible date is rejected instead of silently stored.
impl WidgetProperties for DateEdit {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "date" => Ok(CapabilityValue::String(date_to_string(self.date()))),
            "minimum_date" => Ok(CapabilityValue::String(date_to_string(self.minimum_date()))),
            "maximum_date" => Ok(CapabilityValue::String(date_to_string(self.maximum_date()))),
            "display_format" => Ok(CapabilityValue::String(self.display_format().to_string())),
            "calendar_popup" => Ok(CapabilityValue::Bool(self.calendar_popup())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "date" => {
                self.set_date(expect_date(value)?);
                Ok(())
            }
            "minimum_date" => {
                self.set_minimum_date(expect_date(value)?);
                Ok(())
            }
            "maximum_date" => {
                self.set_maximum_date(expect_date(value)?);
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
        property_names_of![
            "date",
            "minimum_date",
            "maximum_date",
            "display_format",
            "calendar_popup",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl EventHandler for DateEdit {
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
                38 => self.step_up(),   // Up arrow
                40 => self.step_down(), // Down arrow
                _ => { /* Other keys are not relevant */ }
            }
        }
    }
}
impl Draw for DateEdit {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        context.fill_rect(rect, Color::rgb(255, 255, 255));
        context.draw_rect(rect, Color::rgb(150, 150, 150));
        let text = self.date.to_string();
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
    use std::sync::{Arc, Mutex};

    // ─── Date helper tests ───────────────────────────────────────

    #[test]
    fn date_creation_and_accessors() {
        let d = Date::new(2026, 6, 8);
        assert_eq!(d.year(), 2026);
        assert_eq!(d.month(), 6);
        assert_eq!(d.day(), 8);
    }

    #[test]
    fn date_clamps_to_valid_ranges() {
        // Date::new no longer clamps; is_valid() checks bounds
        let d = Date::new(2024, 13, 32);
        assert_eq!(d.month(), 13);
        assert_eq!(d.day(), 32);
        assert!(!d.is_valid());
    }

    #[test]
    fn date_is_leap_year() {
        assert!(Date::new(2024, 1, 1).is_leap_year());
        assert!(Date::new(2000, 1, 1).is_leap_year());
        assert!(!Date::new(2023, 1, 1).is_leap_year());
        assert!(!Date::new(1900, 1, 1).is_leap_year());
    }

    #[test]
    fn date_days_in_month() {
        let jan = Date::new(2026, 1, 1);
        assert_eq!(jan.days_in_month(), 31);

        let feb_normal = Date::new(2023, 2, 1); // not leap
        assert_eq!(feb_normal.days_in_month(), 28);

        let feb_leap = Date::new(2024, 2, 1); // leap
        assert_eq!(feb_leap.days_in_month(), 29);

        let apr = Date::new(2026, 4, 1);
        assert_eq!(apr.days_in_month(), 30);
    }

    #[test]
    fn date_is_valid() {
        assert!(Date::new(2026, 6, 8).is_valid());
        assert!(Date::new(2024, 2, 29).is_valid()); // leap
        assert!(!Date::new(2023, 2, 29).is_valid()); // not leap
        assert!(!Date::new(2026, 0, 1).is_valid()); // month 0
        assert!(!Date::new(2026, 13, 1).is_valid()); // month 13
        assert!(!Date::new(2026, 1, 0).is_valid()); // day 0
        assert!(!Date::new(2026, 4, 31).is_valid()); // April has 30 days
    }

    #[test]
    fn date_setters_modify_correctly() {
        let mut d = Date::new(2026, 6, 8);
        d.set_year(2027);
        assert_eq!(d.year(), 2027);

        d.set_month(12);
        assert_eq!(d.month(), 12);

        d.set_day(25);
        assert_eq!(d.day(), 25);
    }

    #[test]
    fn date_setters_clamp_values() {
        let mut d = Date::new(2026, 6, 8);
        d.set_month(0);
        assert_eq!(d.month(), 1);
        d.set_month(13);
        assert_eq!(d.month(), 12);
        d.set_day(0);
        assert_eq!(d.day(), 1);
        d.set_day(32);
        assert_eq!(d.day(), 31);
    }

    #[test]
    fn date_display_format() {
        let d = Date::new(2026, 6, 8);
        assert_eq!(d.to_string(), "2026-06-08");

        let d2 = Date::new(2024, 12, 25);
        assert_eq!(d2.to_string(), "2024-12-25");
    }

    #[test]
    fn date_ordering() {
        let earlier = Date::new(2024, 1, 1);
        let later = Date::new(2026, 6, 8);
        assert!(earlier < later);
        assert!(later > earlier);
        assert_eq!(earlier, Date::new(2024, 1, 1));
    }

    #[test]
    fn date_today() {
        let today = Date::today();
        assert_eq!(today.year(), 2024);
        assert_eq!(today.month(), 1);
        assert_eq!(today.day(), 1);
    }

    // ─── DateEdit widget tests ───────────────────────────────────

    #[test]
    fn date_edit_creation_defaults() {
        let editor = DateEdit::new(Rect::new(0, 0, 200, 30));
        assert_eq!(editor.date(), Date::today());
        assert_eq!(editor.display_format(), "yyyy-MM-dd");
        assert!(!editor.calendar_popup());
        assert!(editor.is_visible());
        assert!(editor.is_enabled());
    }

    #[test]
    fn date_edit_set_date() {
        let mut editor = DateEdit::new(Rect::new(0, 0, 200, 30));
        let d = Date::new(2026, 12, 25);
        editor.set_date(d);
        assert_eq!(editor.date(), d);
    }

    #[test]
    fn date_edit_set_date_clamps_to_range() {
        let mut editor = DateEdit::new(Rect::new(0, 0, 200, 30));
        // DateEdit default minimum is 1752-09-14, so 1700 is out of range
        let before_min = Date::new(1700, 1, 1);
        editor.set_date(before_min);
        assert_eq!(editor.date(), Date::today()); // Should stay as today
    }

    #[test]
    fn date_edit_set_date_clamps_to_maximum() {
        let mut editor = DateEdit::new(Rect::new(0, 0, 200, 30));
        // DateEdit default maximum is 9999-12-31, so 10000 is out of range
        let after_max = Date::new(10000, 1, 1);
        editor.set_date(after_max);
        assert_eq!(editor.date(), Date::today()); // Should stay as today
    }

    #[test]
    fn date_edit_set_date_range() {
        let mut editor = DateEdit::new(Rect::new(0, 0, 200, 30));
        let min = Date::new(2020, 1, 1);
        let max = Date::new(2030, 12, 31);
        editor.set_date_range(min, max);
        assert_eq!(editor.minimum_date(), min);
        assert_eq!(editor.maximum_date(), max);
    }

    #[test]
    fn date_edit_set_display_format() {
        let mut editor = DateEdit::new(Rect::new(0, 0, 200, 30));
        editor.set_display_format("dd/MM/yyyy".to_string());
        assert_eq!(editor.display_format(), "dd/MM/yyyy");
    }

    #[test]
    fn date_edit_set_calendar_popup() {
        let mut editor = DateEdit::new(Rect::new(0, 0, 200, 30));
        assert!(!editor.calendar_popup());
        editor.set_calendar_popup(true);
        assert!(editor.calendar_popup());
    }

    #[test]
    fn date_edit_step_up() {
        let mut editor = DateEdit::new(Rect::new(0, 0, 200, 30));
        editor.set_date(Date::new(2026, 6, 8));
        editor.step_up();
        assert_eq!(editor.date(), Date::new(2026, 6, 9));
    }

    #[test]
    fn date_edit_step_up_rolls_month() {
        let mut editor = DateEdit::new(Rect::new(0, 0, 200, 30));
        editor.set_date(Date::new(2026, 6, 30));
        editor.step_up();
        assert_eq!(editor.date(), Date::new(2026, 7, 1));
    }

    #[test]
    fn date_edit_step_up_rolls_year() {
        let mut editor = DateEdit::new(Rect::new(0, 0, 200, 30));
        editor.set_date(Date::new(2026, 12, 31));
        editor.step_up();
        assert_eq!(editor.date(), Date::new(2027, 1, 1));
    }

    #[test]
    fn date_edit_step_down() {
        let mut editor = DateEdit::new(Rect::new(0, 0, 200, 30));
        editor.set_date(Date::new(2026, 6, 8));
        editor.step_down();
        assert_eq!(editor.date(), Date::new(2026, 6, 7));
    }

    #[test]
    fn date_edit_step_down_rolls_month() {
        let mut editor = DateEdit::new(Rect::new(0, 0, 200, 30));
        editor.set_date(Date::new(2026, 6, 1));
        editor.step_down();
        assert_eq!(editor.date(), Date::new(2026, 5, 31));
    }

    #[test]
    fn date_edit_step_down_rolls_year() {
        let mut editor = DateEdit::new(Rect::new(0, 0, 200, 30));
        editor.set_date(Date::new(2026, 1, 1));
        editor.step_down();
        assert_eq!(editor.date(), Date::new(2025, 12, 31));
    }

    #[test]
    fn date_edit_undo_redo_restores_date() {
        let mut editor = DateEdit::new(Rect::new(0, 0, 200, 30));
        editor.set_date(Date::new(2026, 6, 8));
        editor.step_up();

        assert!(editor.can_undo());
        assert!(editor.undo());
        assert_eq!(editor.date(), Date::new(2026, 6, 8));
        assert!(editor.can_redo());
        assert!(editor.redo());
        assert_eq!(editor.date(), Date::new(2026, 6, 9));
    }

    #[test]
    fn date_edit_keyboard_shortcuts_drive_history() {
        let mut editor = DateEdit::new(Rect::new(0, 0, 200, 30));
        editor.set_date(Date::new(2026, 6, 8));
        editor.set_date(Date::new(2026, 6, 9));

        editor.handle_event(&Event::KeyPress { key: 90, modifiers: 2 });
        assert_eq!(editor.date(), Date::new(2026, 6, 8));
        editor.handle_event(&Event::KeyPress { key: 89, modifiers: 2 });
        assert_eq!(editor.date(), Date::new(2026, 6, 9));
    }

    #[test]
    fn date_edit_date_changed_signal_emits() {
        let mut editor = DateEdit::new(Rect::new(0, 0, 200, 30));
        let captured = Arc::new(Mutex::new(Date::new(0, 1, 1)));
        let sink = captured.clone();
        editor.date_changed.connect(move |d| {
            if let Ok(mut c) = sink.lock() {
                *c = *d;
            }
        });

        let expected = Date::new(2027, 4, 15);
        editor.set_date(expected);
        let got = *captured.lock().unwrap();
        assert_eq!(got, expected);
    }

    #[test]
    fn date_edit_keyboard_navigation() {
        let mut editor = DateEdit::new(Rect::new(0, 0, 200, 30));
        editor.set_date(Date::new(2026, 6, 8));

        // Up arrow (key 38) = step_up
        editor.handle_event(&Event::KeyPress { key: 38, modifiers: 0 });
        assert_eq!(editor.date(), Date::new(2026, 6, 9));

        // Down arrow (key 40) = step_down
        editor.handle_event(&Event::KeyPress { key: 40, modifiers: 0 });
        assert_eq!(editor.date(), Date::new(2026, 6, 8));
    }

    #[test]
    fn date_edit_geometry_delegation() {
        let rect = Rect::new(10, 20, 200, 30);
        let editor = DateEdit::new(rect);
        assert_eq!(editor.geometry(), rect);
        assert_eq!(editor.position(), Point::new(10, 20));
        assert_eq!(editor.size(), crate::core::Size::new(200, 30));
    }

    #[test]
    fn date_edit_visibility() {
        let mut editor = DateEdit::new(Rect::new(0, 0, 200, 30));
        assert!(editor.is_visible());
        editor.set_visible(false);
        assert!(!editor.is_visible());
        editor.set_visible(true);
        assert!(editor.is_visible());
    }

    #[test]
    fn date_edit_kind() {
        let editor = DateEdit::new(Rect::new(0, 0, 200, 30));
        assert_eq!(editor.kind(), WidgetKind::DatePicker);
    }

    #[test]
    fn date_edit_ids_are_unique() {
        let a = DateEdit::new(Rect::new(0, 0, 200, 30));
        let b = DateEdit::new(Rect::new(0, 0, 200, 30));
        assert_ne!(a.id(), b.id());
    }

    /// Lowering the maximum must pull the current date down into the new range.
    ///
    /// The bug this pins: the bound setters stored the new bounds without re-checking
    /// the value, so the widget could hold a date outside its own range — after which
    /// every `set_date` call failed the range check silently and the widget was stuck
    /// at a value it reported as invalid.
    #[test]
    fn lowering_the_maximum_clamps_the_current_date() {
        let mut edit = DateEdit::new(Rect::new(0, 0, 200, 30));
        edit.set_date_range(Date::new(2024, 1, 1), Date::new(2024, 12, 31));
        edit.set_date(Date::new(2024, 6, 15));

        edit.set_maximum_date(Date::new(2024, 3, 31));

        assert_eq!(
            edit.date(),
            Date::new(2024, 3, 31),
            "the value must be clamped to the new maximum rather than left out of range"
        );
        edit.set_date(Date::new(2024, 2, 1));
    }

    /// Raising the minimum must pull the current date up into the new range.
    #[test]
    fn raising_the_minimum_clamps_the_current_date() {
        let mut edit = DateEdit::new(Rect::new(0, 0, 200, 30));
        edit.set_date_range(Date::new(2024, 1, 1), Date::new(2024, 12, 31));
        edit.set_date(Date::new(2024, 6, 15));

        edit.set_minimum_date(Date::new(2024, 9, 1));

        assert_eq!(edit.date(), Date::new(2024, 9, 1), "clamped up to the new minimum");
        edit.set_date(Date::new(2024, 10, 1));
    }

    /// An inverted range must not wedge the editor.
    ///
    /// `min > max` is caller error, but it must not leave a value that no write can
    /// replace; the minimum wins so the widget stays usable and pinned at a bound it
    /// would itself accept.
    #[test]
    fn an_inverted_range_pins_to_the_minimum_and_stays_usable() {
        let mut edit = DateEdit::new(Rect::new(0, 0, 200, 30));
        edit.set_date_range(Date::new(2024, 1, 1), Date::new(2024, 12, 31));
        edit.set_date(Date::new(2024, 6, 15));

        edit.set_date_range(Date::new(2024, 10, 1), Date::new(2024, 3, 1));

        assert_eq!(edit.date(), Date::new(2024, 10, 1), "the minimum is authoritative");
        edit.set_date(Date::new(2024, 10, 1));
    }

    /// Bounds that already contain the value must leave it alone.
    #[test]
    fn setting_a_range_that_contains_the_value_does_not_move_it() {
        let mut edit = DateEdit::new(Rect::new(0, 0, 200, 30));
        edit.set_date_range(Date::new(2024, 1, 1), Date::new(2024, 12, 31));
        edit.set_date(Date::new(2024, 6, 15));

        edit.set_minimum_date(Date::new(2024, 2, 1));
        edit.set_maximum_date(Date::new(2024, 11, 1));

        assert_eq!(edit.date(), Date::new(2024, 6, 15), "an in-range value must not be nudged");
    }
}
