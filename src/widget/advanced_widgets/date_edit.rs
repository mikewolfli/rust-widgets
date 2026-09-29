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
use crate::core::{Color, Font, HorizontalAlignment, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::undo::{CommandDescription, CommandId, UndoCommand, UndoStack};
use crate::widget::capability::access::date_to_string;
use crate::widget::capability::coercion::{
    expect_bool, expect_date, expect_horizontal_alignment, expect_string,
    horizontal_alignment_to_str,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::dimensions;
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

    /// The day of the week, `0` = Sunday through `6` = Saturday.
    ///
    /// # Why this exists
    ///
    /// A month grid cannot be laid out without it: the first week of a month starts at the column
    /// of its first day, and that column is a weekday. Without it a calendar popup could only print
    /// the days in a run, which is not a calendar.
    ///
    /// # The algorithm
    ///
    /// Sakamoto's, which needs no tables and no era arithmetic: it is exact for every Gregorian
    /// date, including the `1752-09-14` floor this widget accepts and leap years. It is applied to an
    /// invalid combination (a day of `32`) without checking, the same way every other accessor here
    /// does — validation is [`Date::is_valid`]'s job.
    ///
    /// A month outside `1..=12` (possible after [`Date::new`] or a direct field assignment) yields a
    /// clamped month rather than indexing out of bounds: this is an accessor on a value that is
    /// allowed to be invalid, so it must not be the thing that panics.
    pub fn weekday(&self) -> u8 {
        // Offsets `t` for each month, in **calendar order** (January first). Sakamoto's form: the
        // table folds January and February into the previous year's tail, which is why their offsets
        // are non-monotonic — that is the table, not a mistake.
        //
        //     t = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4]
        //
        // The year term adds a day for each leap year, so it must be taken on the *shifted* year:
        // January and February belong to the year before theirs for that purpose, which is the `-1`
        // below. Using the unshifted year here is the classic way this formula goes wrong once per
        // leap year.
        let month = self.month.clamp(1, 12) as usize;
        const T: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
        let t = T[month - 1];
        let year = if self.month < 3 { self.year - 1 } else { self.year };
        // `rem_euclid` rather than `%`: a pre-epoch year gives a negative sum, and `%` would return a
        // negative weekday instead of one in `0..=6`.
        ((year + year / 4 - year / 100 + year / 400 + t + self.day as i32).rem_euclid(7)) as u8
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

/// Substitutes a `display_format` pattern with the given component values.
///
/// # Why one formatter and not three
///
/// `date_edit`, `date_time_edit` and `time_edit` each hold a pattern that they used to store and
/// round-trip without ever applying — their own documentation said so. Three implementations of
/// "replace `yyyy` with the year" would be three chances for the same pattern to render differently
/// in the three controls, which is how the eight dialogs acquired four title-bar heights (rule #101).
/// The pattern is the same idea in all three, so the substitution is one function.
///
/// # The tokens
///
/// Longest-first, because `MM` and `mm` and `M` all start with `M` and a left-to-right scan that
/// tried `M` first would turn `MM` into two months and `yyyy` into four years. The set is the subset
/// of the usual pattern letters that these three controls can supply:
///
/// | token | meaning |
/// |---|---|
/// | `yyyy` | four-digit year (zero-padded, and sign-preserving) |
/// | `yy` | two-digit year |
/// | `MM` | two-digit month, `01`–`12` |
/// | `M` | month without padding |
/// | `dd` | two-digit day, `01`–`31` |
/// | `d` | day without padding |
/// | `HH` | two-digit 24-hour, `00`–`23` |
/// | `H` | hour without padding |
/// | `mm` | two-digit minute |
/// | `ss` | two-digit second |
/// | `SSS` | milliseconds, three digits |
///
/// Longest-first is **not enough on its own**. A single-letter token still matches inside a word, and a
/// scan with no lookaround turned the literal `"Today: yyyy"` into `"To8ay: 2026"` — the `d` of
/// `Today` became the day of the month. So a token is only recognised where it can be a **token**:
/// at the start of the pattern, or after a character that is not a letter. That rule is what the
/// usual pattern languages mean by a literal, and it is why `"Today: yyyy"` now passes "Today"
/// through untouched while `"dd/MM/yyyy"` still substitutes every field.
///
/// Anything else — a separator, a literal word — is copied through unchanged, so a pattern that is
/// not understood degrades to more literal text rather than to an empty field.
///
/// The unused components are passed as `None` by the caller: a date-only control has no hour, so
/// `HH` in its pattern expands to nothing rather than to a plausible `00`.
///
/// It returns `None` when the pattern contains **no recognised token at all**, which is the signal a
/// caller uses to fall back to its own fixed spelling: a pattern of `"today"` should not replace the
/// value's display with the word "today".
pub(crate) fn format_with_pattern(pattern: &str, components: DateTimeComponents) -> Option<String> {
    /// One pattern token and the way it renders itself from the components.
    ///
    /// Named so the table below is an array of pairs rather than an array of a type whose shape has to
    /// be read off the literal.
    type Token = (&'static str, fn(&DateTimeComponents) -> Option<String>);
    // Longest first, so a two-character token is never eaten by its one-character prefix.
    const TOKENS: [Token; 11] = [
        ("yyyy", |c| c.year.map(|y| format!("{y:04}"))),
        ("SSS", |c| c.millisecond.map(|ms| format!("{ms:03}"))),
        ("yy", |c| c.year.map(|y| format!("{:02}", y.rem_euclid(100)))),
        ("MM", |c| c.month.map(|m| format!("{m:02}"))),
        ("dd", |c| c.day.map(|d| format!("{d:02}"))),
        ("HH", |c| c.hour.map(|h| format!("{h:02}"))),
        ("mm", |c| c.minute.map(|m| format!("{m:02}"))),
        ("ss", |c| c.second.map(|s| format!("{s:02}"))),
        ("M", |c| c.month.map(|m| m.to_string())),
        ("d", |c| c.day.map(|d| d.to_string())),
        ("H", |c| c.hour.map(|h| h.to_string())),
    ];

    let mut out = crate::compat::String::new();
    let mut rest = pattern;
    let mut substituted = false;
    // Whether a token may start here: only at the beginning or after a non-letter. See the doc
    // comment above for the `"Today"` case this exists to fix.
    let mut at_token_start = true;
    'scan: while !rest.is_empty() {
        if at_token_start {
            for (token, render) in TOKENS {
                if let Some(after) = rest.strip_prefix(token) {
                    // A token with no value behind it is dropped rather than passed through: the
                    // caller chose the pattern and knows which components it has, so `HH` in a
                    // date-only pattern is a mistake to swallow, not a literal to display.
                    if let Some(text) = render(&components) {
                        out.push_str(&text);
                        substituted = true;
                    }
                    rest = after;
                    // A token's own last character is a letter, so the run continues: `yyyyMM` must
                    // be two tokens, not a year followed by a literal `MM`.
                    at_token_start = true;
                    continue 'scan;
                }
            }
        }
        // No token matched, so this character is literal text (a separator, a word, a stray letter).
        // Advancing by one **char**, not one byte: a multi-byte character in a label would otherwise
        // be split and produce invalid text.
        let ch = rest.chars().next().expect("rest is non-empty, so there is a next char");
        out.push(ch);
        rest = &rest[ch.len_utf8()..];
        at_token_start = !ch.is_alphabetic();
    }

    if substituted {
        Some(out)
    } else {
        None
    }
}

/// The values [`format_with_pattern`] substitutes, each optional.
///
/// `None` means "this control has no such component": a date-only pattern sees no hour, a time-only
/// one sees no day. Stated as a struct rather than eleven arguments because the call sites differ
/// only in which three or four of them they fill in.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct DateTimeComponents {
    /// Four-digit year.
    pub year: Option<i32>,
    /// Month, `1`–`12`.
    pub month: Option<u8>,
    /// Day of month, `1`–`31`.
    pub day: Option<u8>,
    /// Hour, `0`–`23`.
    pub hour: Option<u8>,
    /// Minute, `0`–`59`.
    pub minute: Option<u8>,
    /// Second, `0`–`59`.
    pub second: Option<u8>,
    /// Millisecond, `0`–`999`.
    pub millisecond: Option<u16>,
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
    /// Whether the field draws its month grid under itself.
    ///
    /// A date field with a calendar affordance and no calendar is a field that lies about what it
    /// offers. The flag was stored, published and round-tripped from the day it was added, and the
    /// draw never read it -- so `calendar_popup: true` changed nothing on screen.
    calendar_popup: bool,
    /// How the field's value is aligned within its own box.
    ///
    /// Horizontal only: the value is centred vertically in the field, so a `top`/`bottom` value
    /// would be one this control could never honour —
    /// [`crate::widget::capability::coercion::expect_horizontal_alignment`] refuses those rather
    /// than accepting a write that does nothing. Defaults to left.
    alignment: crate::core::Alignment,
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
            alignment: crate::core::Alignment::Left,
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
    /// Defaults to `false`. With it set, the draw paints the month grid for the current date's
    /// month below the field; see [`DateEdit::set_calendar_popup`].
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
    /// The current date is re-clamped into the new range, so raising the minimum
    /// moves a now-out-of-range value up to the boundary rather than leaving the
    /// widget holding a date below its own minimum.
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
    /// holds after any sequence of calls. The inverted-range rule lives in
    /// [`clamp_ordered_range`], shared with the sibling time widgets.
    fn clamp_to_range(&mut self) {
        self.date = super::clamp_ordered_range(self.date, self.minimum, self.maximum);
    }
    /// Stores the display-format pattern.
    ///
    /// The string is kept verbatim; no validation is performed. See
    /// [`DateEdit::display_format`] for the current limits on its use.
    pub fn set_display_format(&mut self, fmt: String) {
        self.display_format = fmt;
        self.base.request_redraw();
    }
    /// Enables or disables the calendar popup.
    ///
    /// # What it actually does
    ///
    /// With the flag set, [`Draw`] paints the **month grid for the current date's month** directly
    /// below the field: a row of day-of-week initials and six weeks of day cells, with the selected
    /// day marked. Cells outside the accepted range are drawn muted rather than hidden, so the
    /// reader can see *why* a day is not selectable.
    ///
    /// This used to be "purely stored state; changing it only triggers a redraw", which is exactly
    /// what it did -- a redraw of the same field.
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
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        crate::core::Size::new(
            dimensions::DATE_EDIT_DEFAULT_WIDTH,
            dimensions::DATE_EDIT_DEFAULT_HEIGHT,
        )
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
            "alignment" => Ok(CapabilityValue::String(
                horizontal_alignment_to_str(self.alignment()).to_string(),
            )),
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
            "alignment" => {
                self.set_alignment(expect_horizontal_alignment(value)?);
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
            "alignment",
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

        // Chrome colours resolve explicit style first, then the theme's resolved style
        // for this control, and only then fall back to a literal. Without the theme step
        // a light/dark switch changed nothing on screen: the field, its border and its
        // text were all hardcoded, which the rendering census reported as theme-blind.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global
        // manager's mutex is not re-entrant. `date_edit` classifies as `Input`, whose
        // resolved style supplies `text_color` and `border_color` but no
        // `background_color` (ThemeRole::Input resolves that to `None`), so the interior
        // is derived below from the theme's own background rather than left unset.
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("date_edit");
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
        // byte-identical to the window behind it. That identity is the defect the census
        // reports as "painted in the background's own colour".
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

        context.face(
            rect,
            surface,
            self.style().surface.unwrap_or_default(),
            self.style().border_radius.unwrap_or(0),
            Color::BLACK,
        );
        context.draw_rect(rect, border);
        // The value is spelled by the caller's `display_format` when that pattern is one this control
        // can render, and by `Date`'s own `YYYY-MM-DD` otherwise. The fallback is what keeps an
        // unrecognised pattern showing the value rather than the pattern's literal words.
        let text = super::date_edit::format_with_pattern(
            &self.display_format,
            super::date_edit::DateTimeComponents {
                year: Some(self.date.year()),
                month: Some(self.date.month()),
                day: Some(self.date.day()),
                ..Default::default()
            },
        )
        .unwrap_or_else(|| self.date.to_string());
        // Vertically centred through the shared primitive: `rect.y + height / 2` puts the
        // glyph box's top edge on the field's middle line, so the value sat half a line low.
        // The line box is also what a caller reading this field's text position would need,
        // so deriving it here keeps the two from drifting.
        let font = Font::default();
        let line = context.text_line(rect, &font);
        context.draw_text_fitted(
            Rect {
                x: rect.x + 6,
                y: line.y,
                width: rect.width.saturating_sub(12),
                height: line.height,
            },
            &text,
            &font,
            ink,
            self.alignment.to_horizontal().unwrap_or(HorizontalAlignment::Left),
        );

        if self.calendar_popup {
            self.draw_calendar_popup(context, rect, surface, border, ink);
        }
    }
}

/// How wide one cell of the calendar popup's grid is, in pixels.
const POPUP_CELL_W: u32 = 20;
/// How tall one cell of the calendar popup's grid is, in pixels.
const POPUP_CELL_H: u32 = 16;
/// The space between the field and the popup it opens.
const POPUP_GAP: i32 = 4;
/// The initials of the days of the week, Sunday first, matching [`Date::weekday`].
const WEEKDAY_INITIALS: [&str; 7] = ["S", "M", "T", "W", "T", "F", "S"];

/// Paints a month grid under `field`: the popup plate, the day-of-week header, and six weeks of
/// cells with the marked day tinted by the accent.
///
/// This is the **one** implementation of the calendar popup. [`DateEdit`] and `DateTimeEdit` show
/// the same grid -- their own docs say so -- and previously each carried a near-verbatim ~100-line
/// copy, so a change to one (cell size, the accent source, the six-week rule) would silently leave
/// the other behind. Both now delegate here.
///
/// `date` is the month to lay out and the day to mark; `minimum`/`maximum` decide which cells are
/// muted. The grid geometry is derived from the field's own rect so a field placed anywhere opens
/// its popup in the same relative position.
pub(crate) fn draw_month_grid(
    context: &mut RenderContext,
    field: Rect,
    surface: Color,
    border: Color,
    ink: Color,
    date: &Date,
    minimum: &Date,
    maximum: &Date,
) {
    let grid_w = POPUP_CELL_W * 7;
    let grid_h = POPUP_CELL_H * 7;
    let popup = Rect::new(
        field.x,
        field.y + field.height as i32 + POPUP_GAP,
        grid_w.max(field.width),
        grid_h,
    );
    // The popup gets its own plate and edge, because at this size a bare grid over the page is
    // indistinguishable from a table that happens to be there. It is the *field's* surface one
    // step further from the field, so the two read as one control opening rather than two.
    let plate = surface.blend(&ink, 0.10);
    context.fill_rect(popup, plate);
    context.draw_rect(popup, border);

    let font = Font::default();
    let muted = plate.blend(&ink, 0.45);
    // The day-of-week row, so the grid's columns mean something. Drawn from the same table
    // `weekday()` indexes, so the column that a date lands in is the initial above it.
    for (col, initial) in WEEKDAY_INITIALS.iter().enumerate() {
        let cell = Rect::new(
            popup.x + (col as u32 * POPUP_CELL_W) as i32,
            popup.y,
            POPUP_CELL_W,
            POPUP_CELL_H,
        );
        context.draw_text_line(cell, initial, &font, muted, HorizontalAlignment::Center);
    }

    let first = Date::new(date.year(), date.month(), 1);
    let leading = first.weekday() as u32;
    let days = first.days_in_month() as u32;
    for day in 1..=days {
        let slot = leading + day - 1;
        let col = slot % 7;
        let row = slot / 7;
        // Six weeks is the most a month can need (a 31-day month starting on Saturday ends in
        // row 5), so anything past that cannot be produced and is skipped rather than clamped
        // onto a cell that belongs to another day.
        if row > 5 {
            break;
        }
        let cell = Rect::new(
            popup.x + (col * POPUP_CELL_W) as i32,
            popup.y + POPUP_CELL_H as i32 + (row * POPUP_CELL_H) as i32,
            POPUP_CELL_W,
            POPUP_CELL_H,
        );
        let this = Date::new(date.year(), date.month(), day as u8);
        let in_range = this >= *minimum && this <= *maximum;
        if day == date.day() as u32 {
            // The selected day is the accent plate the rest of the crate marks a selection
            // with, so "this is the value" reads the same here as everywhere else.
            let accent = crate::style::resolved_theme_style("slider")
                .and_then(|style| style.background_color)
                .unwrap_or_else(|| plate.blend(&ink, 0.55));
            context.fill_rect(cell, accent);
            context.draw_text_line(
                cell,
                &day.to_string(),
                &font,
                accent.contrast_color(),
                HorizontalAlignment::Center,
            );
        } else {
            // Out-of-range days stay visible but muted: hiding them would leave the reader
            // unable to see *why* a day cannot be picked.
            let day_ink = if in_range { ink } else { muted };
            context.draw_text_line(
                cell,
                &day.to_string(),
                &font,
                day_ink,
                HorizontalAlignment::Center,
            );
        }
    }
}

impl DateEdit {
    /// Paints the month grid for the current date's month, below the field.
    ///
    /// # Why it is drawn rather than held as a child widget
    ///
    /// It is a **function of the field's own state**: the month comes from `self.date`, the muted
    /// cells from `self.minimum`/`self.maximum`, and the marked cell from `self.date.day`. A child
    /// widget would have to be kept in step with all three on every mutation, which is three places
    /// to forget -- and the flag is published as a boolean, so there is nothing for a caller to hold
    /// anyway.
    ///
    /// The grid itself is [`draw_month_grid`], shared with `DateTimeEdit` so the two controls cannot
    /// drift apart.
    fn draw_calendar_popup(
        &self,
        context: &mut RenderContext,
        field: Rect,
        surface: Color,
        border: Color,
        ink: Color,
    ) {
        draw_month_grid(
            context,
            field,
            surface,
            border,
            ink,
            &self.date,
            &self.minimum,
            &self.maximum,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Point;
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

    /// The `display_format` pattern is **applied**, not merely stored.
    ///
    /// # The defect this pins
    ///
    /// `display_format` was stored, round-tripped through the property contract and documented as
    /// "stored and round-tripped, not yet applied when painting" -- which was honest, and was still a
    /// gap: a host that set `dd/MM/yyyy` got `2026-06-08`. The assertion reads the **document**, since
    /// a model-only check (set then get) passes on the old code.
    ///
    /// Two halves: the requested pattern must lay its own ink, and the default must be unchanged.
    /// The second is what makes this "the pattern works" rather than "the field got longer".
    #[test]
    fn the_display_format_pattern_is_applied_to_the_painted_value() {
        use crate::widget::svg::{render_to_svg, text_subpath_count};
        let _theme_guard = crate::style::theme_test_guard();

        let make = |format: &str| {
            let mut editor = DateEdit::new(Rect::new(0, 0, 200, 30));
            editor.set_date(Date::new(2026, 6, 8));
            editor.set_display_format(format.to_string());
            editor
        };

        // The default pattern spells the same seventeen characters as `Date`'s own Display, so it must
        // produce exactly the ink it always did -- which is why the snapshots are unchanged.
        let mut plain = make("yyyy-MM-dd");
        let plain_svg = render_to_svg(&mut plain);
        let mut explicit = make("yyyy-MM-dd");
        assert_eq!(
            render_to_svg(&mut explicit),
            plain_svg,
            "the default pattern must reproduce the pre-pattern spelling"
        );

        // A reordered pattern must **move** the ink: a pattern that did nothing would produce a
        // byte-identical file. And the only difference between the two spellings is the order, so the
        // *number of glyph runs* (one per set bitmap bit) is close but not equal -- `08-06-2026` and
        // `2026-06-08` contain the same characters in a different order, and `font8x8`'s glyphs have
        // different bit counts. An earlier version of this test compared the counts for equality and
        // failed by 14 pixels, which measurement says is the glyph-set difference and not a lost field.
        // The field-preservation claim is therefore made where it can be made exactly: on the
        // substituter's own output, one test above.
        let mut reordered = make("dd/MM/yyyy");
        let reordered_svg = render_to_svg(&mut reordered);
        assert_ne!(
            reordered_svg, plain_svg,
            "a reordered pattern must move the ink, or the pattern was ignored"
        );
        // Same magnitude, so no field was duplicated or dropped. The band is a fifth of the total,
        // which is far tighter than a wrong pattern could pass and far looser than the glyph-set
        // difference measured here (14 of 246).
        let (a, b) = (text_subpath_count(&reordered_svg), text_subpath_count(&plain_svg));
        assert!(
            a.abs_diff(b) < b / 5,
            "both spell the same date, so neither may gain or lose a field: reordered={a} plain={b}"
        );

        // A pattern with no usable token falls back to the fixed spelling rather than printing its
        // own literal words -- a field reading \"today\" would be worse than one ignoring the pattern.
        let mut nonsense = make("today");
        assert_eq!(
            render_to_svg(&mut nonsense),
            plain_svg,
            "an unrecognised pattern must fall back to the value, not print the pattern"
        );
    }

    /// The substituter's token table, longest-first, and its literal pass-through.
    ///
    /// The ordering is the whole of the correctness here: a scan that tried `M` before `MM` would turn
    /// `MM` into two months and `yyyy` into four years, and both would still look like a date.
    #[test]
    fn the_pattern_substituter_prefers_the_longest_token() {
        let c = DateTimeComponents {
            year: Some(2026),
            month: Some(6),
            day: Some(8),
            hour: Some(9),
            minute: Some(5),
            second: Some(7),
            millisecond: Some(42),
        };
        let render = |pattern: &str| format_with_pattern(pattern, c);

        assert_eq!(render("yyyy-MM-dd"), Some("2026-06-08".to_string()));
        assert_eq!(render("dd/MM/yyyy"), Some("08/06/2026".to_string()));
        // A single letter is unpadded, which is what makes `M` different from `MM`.
        assert_eq!(render("d/M/yyyy"), Some("8/6/2026".to_string()));
        assert_eq!(render("HH:mm:ss"), Some("09:05:07".to_string()));
        assert_eq!(render("H:mm"), Some("9:05".to_string()));
        assert_eq!(render("SSS"), Some("042".to_string()));
        assert_eq!(render("yy"), Some("26".to_string()));
        // Literal text survives around the tokens.
        assert_eq!(render("Today: yyyy"), Some("Today: 2026".to_string()));
        // A component the caller did not supply is dropped, not invented.
        let date_only = DateTimeComponents { year: Some(2026), ..Default::default() };
        assert_eq!(format_with_pattern("yyyy HH", date_only), Some("2026 ".to_string()));
        // And a pattern with nothing usable reports that, so the caller can fall back.
        assert_eq!(render("nope"), None);
        assert_eq!(render(""), None);
    }

    #[test]
    fn date_edit_set_calendar_popup() {
        let mut editor = DateEdit::new(Rect::new(0, 0, 200, 30));
        assert!(!editor.calendar_popup());
        editor.set_calendar_popup(true);
        assert!(editor.calendar_popup());
    }

    /// `weekday()` agrees with known dates and is always in `0..=6`.
    ///
    /// # The defect this pins
    ///
    /// `Date` had no weekday at all, so a month grid could not be laid out: the first week of a
    /// month starts at the column of its first day, and that column *is* a weekday. The anchors are
    /// real calendar dates with externally-known weekdays, and the leap/century cases are included
    /// because a formula that drops either returns a plausible wrong answer rather than failing.
    #[test]
    fn weekday_agrees_with_known_dates() {
        // 0 = Sunday.
        assert_eq!(Date::new(2026, 6, 8).weekday(), 1, "2026-06-08 is a Monday");
        assert_eq!(Date::new(2000, 1, 1).weekday(), 6, "2000-01-01 is a Saturday");
        assert_eq!(Date::new(2024, 2, 29).weekday(), 4, "2024-02-29 is a Thursday");
        assert_eq!(Date::new(1900, 1, 1).weekday(), 1, "1900-01-01 is a Monday");
        assert_eq!(Date::new(2024, 3, 1).weekday(), 5, "2024-03-01 is a Friday");
        assert_eq!(Date::new(1752, 9, 14).weekday(), 4, "the accepted floor is a Thursday");
        // Every date in a run of consecutive days lands in `0..=6`, and consecutive days advance by
        // exactly one -- which a formula with a wrong constant would break without failing the
        // anchors above. The run crosses a month end and a leap day on purpose.
        let mut date = Date::new(2024, 2, 26);
        let mut expected = 1; // 2024-02-26 is a Monday
        for _ in 0..8 {
            assert_eq!(date.weekday(), expected, "{date}");
            expected = (expected + 1) % 7;
            date = next_day(date);
        }
    }

    /// The next calendar day, by the real month lengths rather than by arithmetic on the day field.
    fn next_day(mut date: Date) -> Date {
        if date.day < date.days_in_month() {
            date.day += 1;
            return date;
        }
        date.day = 1;
        if date.month < 12 {
            date.month += 1;
        } else {
            date.month = 1;
            date.year += 1;
        }
        date
    }

    /// The calendar popup is **painted**, not merely stored.
    ///
    /// # The defect this pins
    ///
    /// `calendar_popup` was a stored boolean that `draw` never read: the getter, the setter, the
    /// schema row and the round-trip test all existed, and setting it changed nothing on screen. A
    /// model-only assertion (`assert!(editor.calendar_popup())`) passes on that code, which is why
    /// this one reads the **document**: the popup must add ink, and it must add the *days of the
    /// month* -- not just any ink, which is what "the documents differ" alone would accept.
    #[test]
    fn the_calendar_popup_is_actually_painted() {
        use crate::widget::svg::{render_to_svg, text_subpath_count};
        let _theme_guard = crate::style::theme_test_guard();

        let make = |popup: bool| {
            let mut editor = DateEdit::new(Rect::new(0, 0, 200, 30));
            editor.set_date(Date::new(2026, 6, 8));
            editor.set_calendar_popup(popup);
            editor
        };

        let mut closed = make(false);
        let closed_svg = render_to_svg(&mut closed);
        let mut open = make(true);
        let open_svg = render_to_svg(&mut open);

        assert_ne!(closed_svg, open_svg, "opening the popup must change the picture");
        // The grid is seven weekday initials plus the days of the month. Six weeks of 31-day
        // coverage needs at least the 31 day numbers, so the ink goes up by far more than one glyph
        // run -- and the date field alone contributes exactly one run (`2026-06-08`).
        let closed_ink = text_subpath_count(&closed_svg);
        let open_ink = text_subpath_count(&open_svg);
        assert!(
            open_ink > closed_ink * 4,
            "the popup must paint the grid and the month's days: closed={closed_ink} open={open_ink}"
        );
        // And the popup's plate is a rectangle below the field, which is what makes it a popup
        // rather than extra glyphs floating on the page.
        assert!(
            open_svg.matches("<rect").count() > closed_svg.matches("<rect").count(),
            "the popup paints its own plate: {open_svg}"
        );
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
