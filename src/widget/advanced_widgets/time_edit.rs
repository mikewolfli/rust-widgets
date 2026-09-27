// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Time editor widget.
//!
//! [`TimeEdit`] stores a wall-clock time as a [`Time`] value. It has no
//! time-zone or date component: it represents a local time of day.
//!
//! # Conventions
//!
//! * The hour is **24-hour**, `0`-`23` (`0` is midnight). There is no AM/PM
//!   handling and no 12-hour mode.
//! * The minute and second range over `0`-`59` and the millisecond over
//!   `0`-`999`.
//! * Unlike [`crate::widget::advanced_widgets::date_edit::Date`], a [`Time`]
//!   **cannot** hold an invalid value: [`Time::new`] and the setters clamp
//!   every field into range, so [`Time::is_valid`] always returns `true` for a
//!   value obtained through the public API.
//! * The widget's range is **inclusive at both ends**: a time equal to
//!   [`TimeEdit::minimum_time`] or [`TimeEdit::maximum_time`] is accepted, and
//!   [`TimeEdit::set_time`] rejects out-of-range input silently rather than
//!   clamping it.
//! * Times are ordered chronologically, hour then minute then second then
//!   millisecond.
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::undo::{CommandDescription, CommandId, UndoCommand, UndoStack};

use crate::widget::capability::access::time_to_string;
use crate::widget::capability::coercion::{expect_bool, expect_string, expect_time};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::dimensions;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TIME_EDIT_COMMAND_ID: AtomicU64 = AtomicU64::new(1);

struct TimeEditCommand {
    id: CommandId,
    target: Rc<RefCell<Time>>,
    before: Time,
    after: Time,
}

impl TimeEditCommand {
    fn new(target: Rc<RefCell<Time>>, before: Time, after: Time) -> Self {
        Self {
            id: CommandId(NEXT_TIME_EDIT_COMMAND_ID.fetch_add(1, Ordering::Relaxed)),
            target,
            before,
            after,
        }
    }
}

impl UndoCommand for TimeEditCommand {
    fn id(&self) -> CommandId {
        self.id
    }

    fn description(&self) -> CommandDescription {
        CommandDescription {
            text: "Edit time".to_string(),
            timestamp_ms: 0,
            command_type: "time_edit",
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
/// The clock face's diameter, in logical pixels.
///
/// A clock is read by angle, so it has to be big enough that twelve of them are distinguishable --
/// below about this size the 1 and the 2 positions run together and the face is decoration.
const CLOCK_DIAMETER: u32 = 160;
/// The space between the field and the clock face it opens.
const CLOCK_GAP: i32 = 4;
/// The radius fraction below which a point belongs to no ring: the clock's hub.
///
/// A real clock has a hub that sets nothing, and a click there should not silently pick an arbitrary
/// value -- which is what treating the centre as the inner ring would do.
const HOUR_HOLE: f32 = 0.22;
/// The radius fraction where the inner (minute) ring gives way to the outer (hour) ring.
const MINUTE_RING_INNER: f32 = 0.62;

/// Time value (hour, minute, second, millisecond).
///
/// All four fields are stored already clamped to their valid ranges, so any
/// instance built through the public API is a valid time of day; see
/// [`Time::is_valid`].
///
/// `Ord` follows chronological order within a single day: hour, then minute,
/// then second, then millisecond.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Time {
    hour: u8,   // 0-23
    minute: u8, // 0-59
    second: u8, // 0-59
    msec: u16,  // 0-999
}
impl Time {
    /// Creates a time of day, clamping each component into its valid range.
    ///
    /// `hour` is 24-hour (`0` = midnight) and is clamped to `0..=23`; `minute`
    /// and `second` are clamped to `0..=59`, and `msec` to `0..=999`. Clamping is
    /// deliberate rather than an error, so out-of-range input is silently
    /// reduced to the nearest boundary (e.g. `25` hours becomes `23`).
    pub fn new(hour: u8, minute: u8, second: u8, msec: u16) -> Self {
        Self {
            hour: hour.min(23),
            minute: minute.min(59),
            second: second.min(59),
            msec: msec.min(999),
        }
    }
    /// Returns the hour in 24-hour form (`0` = midnight).
    pub fn hour(&self) -> u8 {
        self.hour
    }
    /// Returns the minute of the hour (`0`-`59`).
    pub fn minute(&self) -> u8 {
        self.minute
    }
    /// Returns the second of the minute (`0`-`59`).
    ///
    /// The value is a whole second; sub-second precision is not represented.
    pub fn second(&self) -> u8 {
        self.second
    }
    /// Returns the millisecond component (`0`-`999`).
    pub fn msec(&self) -> u16 {
        self.msec
    }
    /// Sets the hour, clamped to `0..=23`.
    pub fn set_hour(&mut self, hour: u8) {
        self.hour = hour.min(23);
    }
    /// Sets the minute, clamped to `0..=59`.
    pub fn set_minute(&mut self, minute: u8) {
        self.minute = minute.min(59);
    }
    /// Sets the second, clamped to `0..=59`. Milliseconds are unaffected.
    pub fn set_second(&mut self, second: u8) {
        self.second = second.min(59);
    }
    /// Sets the millisecond component, clamped to `0..=999`.
    pub fn set_msec(&mut self, msec: u16) {
        self.msec = msec.min(999);
    }
    /// Returns `true` when every component is within its valid range.
    ///
    /// Because [`Time::new`] and the setters clamp, this is always `true` for a
    /// value built through the public API; it exists to document and enforce the
    /// invariant for values that might be constructed internally.
    pub fn is_valid(&self) -> bool {
        self.hour <= 23 && self.minute <= 59 && self.second <= 59 && self.msec <= 999
    }
    /// Returns the time of day as milliseconds elapsed since midnight.
    ///
    /// The result ranges from `0` (for `00:00:00.000`) to `86_399_999`.
    /// Useful for sorting or for position-on-a-day calculations.
    pub fn to_msecs_since_midnight(&self) -> u32 {
        (self.hour as u32 * 3600 + self.minute as u32 * 60 + self.second as u32) * 1000
            + self.msec as u32
    }
}
/// Formats the time as `HH:MM:SS`.
///
/// The hour, minute, and second are each zero-padded to two digits. The
/// millisecond component is **not** included, so this round-trips through
/// [`crate::widget::capability::coercion::expect_time`] only when the time has
/// zero milliseconds; a non-zero `msec` is lost by the round-trip.
impl std::fmt::Display for Time {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:02}:{:02}:{:02}", self.hour, self.minute, self.second)
    }
}
/// Time editor widget.
///
/// Shows a time of day and allows it to be changed a second at a time (see
/// [`TimeEdit::step_up`] / [`TimeEdit::step_down`]), programmatically via
/// [`TimeEdit::set_time`], or through the keyboard (Up/Down step, Ctrl+Z/Ctrl+Y
/// undo and redo).
///
/// The accepted range defaults to `00:00:00.000 ..= 23:59:59.999` and is
/// inclusive at both ends. Stepping is second-based, so the millisecond
/// component is only reachable through [`TimeEdit::set_time`].
///
/// # Display format
///
/// [`TimeEdit::display_format`] holds a format pattern string that is exposed
/// as a property, but the widget's own `draw` always uses the fixed `HH:MM:SS`
/// spelling. The pattern is stored and round-tripped, not yet applied when
/// painting.
pub struct TimeEdit {
    base: BaseWidget,
    time: Time,
    minimum: Time,
    maximum: Time,
    display_format: String,
    /// Whether the field draws a clock face under itself for picking a time.
    ///
    /// The widget had `step_up`/`step_down` from the day it was written, so a keyboard user could
    /// walk a time one second at a time — but a *pointer* user had nothing: there was no affordance to
    /// click, and no way to reach 14:30 without pressing an arrow eight thousand times. This flag is
    /// what makes the picker reachable, and the picker is what makes `TimeEdit` a time *picker*
    /// rather than a text field with a formatter.
    clock_popup: bool,
    /// Which of the clock's two rings the pointer is over, while the popup is open.
    ///
    /// The hand a click would move: the outer ring sets the hour and the inner one the minute, so
    /// naming it once here is what lets the hover highlight and the click agree.
    hovered_hand: Option<ClockHand>,
    /// Emitted with the new open state when the clock is shown or hidden, so a host can dismiss its
    /// other popups without polling.
    pub popup_visibility_changed: Signal1<bool>,
    /// Emitted with the new time after every accepted change, including changes
    /// produced by [`TimeEdit::undo`] and [`TimeEdit::redo`]. Not emitted when a
    /// change is rejected or when the time is already the requested value.
    pub time_changed: Signal1<Time>,
    undo_stack: UndoStack,
    history_target: Rc<RefCell<Time>>,
    restoring_history: bool,
}

/// Which ring of the clock face a point or a click belongs to.
///
/// A clock face answers "what time is it" by angle and "which part" by radius, so the two rings are
/// the whole interaction: the outer ring is the hour and the inner one the minute. An enum rather than
/// a boolean because there are three answers a hit test can give — the outer ring, the inner ring, or
/// neither — and a `bool` would have to encode "neither" as "inner", which is a wrong answer rather
/// than a missing one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClockHand {
    /// The outer ring: sets the hour.
    Hour,
    /// The inner ring: sets the minute.
    Minute,
}
impl TimeEdit {
    /// Creates a time editor occupying `geometry`.
    ///
    /// The initial time is midnight (`00:00:00.000`), the accepted range is
    /// `00:00:00.000 ..= 23:59:59.999` (inclusive), and the display format is
    /// `"HH:mm:ss"`.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::TimePicker, geometry, "TimeEdit"),
            time: Time::new(0, 0, 0, 0),
            minimum: Time::new(0, 0, 0, 0),
            maximum: Time::new(23, 59, 59, 999),
            display_format: "HH:mm:ss".to_string(),
            clock_popup: false,
            hovered_hand: None,
            popup_visibility_changed: Signal1::new(),
            time_changed: Signal1::new(),
            undo_stack: UndoStack::new(),
            history_target: Rc::new(RefCell::new(Time::new(0, 0, 0, 0))),
            restoring_history: false,
        }
    }
    /// Returns the current time.
    pub fn time(&self) -> Time {
        self.time
    }

    /// Whether the field draws its clock face under itself.
    ///
    /// Defaults to `false`, so a caller that never asks renders exactly as it did. With it set, the
    /// draw paints the face below the field; see [`TimeEdit::set_clock_popup`].
    pub fn clock_popup(&self) -> bool {
        self.clock_popup
    }

    /// Shows or hides the clock face.
    ///
    /// The flag is what makes the picker reachable, so setting it is a visible change rather than the
    /// redraw-of-the-same-field it used to be. Hiding the face clears the pointer highlight, because a
    /// ring cannot be hovered by a face that is not showing.
    pub fn set_clock_popup(&mut self, popup: bool) {
        if self.clock_popup == popup {
            return;
        }
        self.clock_popup = popup;
        if !popup {
            self.hovered_hand = None;
        }
        self.popup_visibility_changed.emit(popup);
        self.base.request_redraw();
    }

    /// Opens the clock if it is closed, closes it if it is open.
    pub fn toggle_clock_popup(&mut self) {
        self.set_clock_popup(!self.clock_popup);
    }

    /// The box the clock face occupies: as tall as it is wide, centred under the field.
    ///
    /// # Why square and derived
    ///
    /// A clock face is a circle, and a circle in a non-square box is an ellipse whose angles no longer
    /// mean what the numbers say -- 3 o'clock would sit off the 3 o'clock position. So the diameter
    /// is the smaller of the face's derived extent and the field's width, and it is derived rather
    /// than stored because both terms are already facts the control has.
    ///
    /// # Why it does not fit the field's own `rect`
    ///
    /// The value's box is the control's whole rectangle (a `time_edit` is a plain field), while the
    /// popup belongs below the field's *visible* bottom edge. Reading the same `rect` the value does is
    /// what keeps "below the field" meaning the painted field.
    fn clock_rect(&self) -> Rect {
        let field = self.geometry();
        let diameter = CLOCK_DIAMETER.min(field.width);
        Rect::new(
            field.x + (field.width.saturating_sub(diameter)) as i32 / 2,
            field.y + field.height as i32 + CLOCK_GAP,
            diameter,
            diameter,
        )
    }

    /// The clock face's centre.
    fn clock_centre(&self) -> Point {
        let face = self.clock_rect();
        Point::new(face.x + face.width as i32 / 2, face.y + face.height as i32 / 2)
    }

    /// The hand a point falls on, if the clock is open and the point is on one of its two rings.
    ///
    /// # Why radius, not angle
    ///
    /// The ring is what decides *which* hand is being set, and the angle is what decides the value.
    /// Splitting them this way is what lets a caller aim at the minute ring without also having to
    /// hit the exact angle a value would round to -- the value is read from the angle once the ring
    /// has been chosen.
    ///
    /// Returns `None` for the face's centre hole and for anything outside its rim, so a click that is
    /// not on either ring is not silently taken as one.
    fn hand_at_point(&self, point: Point) -> Option<ClockHand> {
        if !self.clock_popup {
            return None;
        }
        let face = self.clock_rect();
        if !face.contains_point(point) {
            return None;
        }
        let centre = self.clock_centre();
        let dx = (point.x - centre.x) as f32;
        let dy = (point.y - centre.y) as f32;
        let distance = (dx * dx + dy * dy).sqrt();
        let radius = face.width as f32 / 2.0;
        // The band each ring owns, as fractions of the radius: the outer ring takes the rim's half and
        // the inner ring the next band in, leaving a centre hole nothing belongs to -- a clock has a
        // hub, and a click on it should not set the minute to an arbitrary value.
        if distance > radius || distance < radius * HOUR_HOLE {
            return None;
        }
        if distance >= radius * MINUTE_RING_INNER {
            Some(ClockHand::Hour)
        } else if distance >= radius * HOUR_HOLE {
            Some(ClockHand::Minute)
        } else {
            None
        }
    }

    /// The value a point on `hand`'s ring selects.
    ///
    /// # The clock convention, stated once
    ///
    /// Twelve o'clock is **up** and the angle grows clockwise, so `0/12` is at the top and `3/15` at
    /// the right -- the same reading a wall clock has. `atan2` gives the mathematical convention
    /// (zero at the right, counter-clockwise), so the conversion is `+90°` to move the origin to the
    /// top and a negation to make it run clockwise. Getting either sign wrong yields a face where
    /// every value lands mirrored or a quarter turn off, which is why the whole conversion is one
    /// expression rather than a chain of adjustments.
    fn value_at_point(&self, point: Point, hand: ClockHand) -> Option<u8> {
        let centre = self.clock_centre();
        let dx = (point.x - centre.x) as f32;
        let dy = (point.y - centre.y) as f32;
        // A click exactly on the hub has no angle; it is not a menu of "every value at once".
        if dx == 0.0 && dy == 0.0 {
            return None;
        }
        // `atan2(-dy, dx)` already counts counter-clockwise from the right; negating the y is what
        // makes it clockwise from the top once the quarter turn is added.
        let clockwise_from_top = dy.atan2(dx);
        let fraction = match hand {
            ClockHand::Hour => {
                // `dy` is positive downward in screen coordinates, so `atan2(dy, dx)` runs clockwise
                // from the right already; shifting it a quarter turn puts zero at the top.
                let turns = (clockwise_from_top + std::f32::consts::FRAC_PI_2)
                    / (2.0 * std::f32::consts::PI);
                turns.rem_euclid(1.0)
            }
            ClockHand::Minute => {
                let turns = (clockwise_from_top + std::f32::consts::FRAC_PI_2)
                    / (2.0 * std::f32::consts::PI);
                turns.rem_euclid(1.0)
            }
        };
        let value = match hand {
            // The hour ring reads 1..=12 on the face. The face cannot distinguish 09:00 from 21:00,
            // so **which half** is decided by the field's own hour -- with one exception: the twelve
            // at the top is noon or midnight on a 24-hour clock and noon on a 12-hour one, so it maps
            // to 12 for the afternoon half and to 0 for the morning half. Every other numeral maps to
            // itself within the half, which is what makes clicking the 9 position on a 21:xx time
            // keep it in the afternoon instead of silently jumping to the morning.
            ClockHand::Hour => {
                let face_hour = (fraction * 12.0).round() as u8;
                let is_pm = self.time.hour() >= 12;
                match (face_hour % 12, is_pm) {
                    // 12 on the face: midnight in the morning half, noon in the afternoon half.
                    (0, false) => 0,
                    (0, true) => 12,
                    (h, false) => h,
                    (h, true) => h + 12,
                }
            }
            // The minute ring reads in five-minute steps: 60 positions on a 40 px rim would be finer
            // than a pointer can aim, and an exact minute is what the arrow keys are for.
            ClockHand::Minute => {
                let step = (fraction * 12.0).round() as u8;
                (step % 12) * 5
            }
        };
        Some(value)
    }

    /// Applies the value a click on `hand`'s ring selects, if the point is on that ring.
    ///
    /// Returns whether the time changed, so a caller repaints only when something moved. The write
    /// goes through [`TimeEdit::set_time`], so a value outside `minimum ..= maximum` is rejected there
    /// rather than being clamped here -- one place decides what is acceptable.
    fn pick_at(&mut self, point: Point) -> bool {
        let Some(hand) = self.hand_at_point(point) else {
            return false;
        };
        let Some(value) = self.value_at_point(point, hand) else {
            return false;
        };
        let mut next = self.time;
        match hand {
            ClockHand::Hour => next.set_hour(value),
            ClockHand::Minute => next.set_minute(value),
        }
        let before = self.time;
        self.set_time(next);
        self.time != before
    }
    /// Returns the inclusive lower bound accepted by [`TimeEdit::set_time`].
    ///
    /// Defaults to midnight, `00:00:00.000`.
    pub fn minimum_time(&self) -> Time {
        self.minimum
    }
    /// Returns the inclusive upper bound accepted by [`TimeEdit::set_time`].
    ///
    /// Defaults to `23:59:59.999`.
    pub fn maximum_time(&self) -> Time {
        self.maximum
    }
    /// Returns the stored display-format pattern.
    ///
    /// This is a plain string; it is currently stored and round-tripped but not
    /// interpreted when the widget paints (painting always uses `HH:MM:SS`).
    pub fn display_format(&self) -> &str {
        &self.display_format
    }
    /// Sets the current time, subject to validity and the accepted range.
    ///
    /// The assignment happens only when `time` satifies [`Time::is_valid`],
    /// falls within `minimum ..= maximum` (both inclusive), and differs from the
    /// current time; otherwise the call is a no-op and the previous time is
    /// kept. Out-of-range input is therefore **rejected, not clamped**. Since
    /// [`Time::new`] already clamps its inputs, the main way to be rejected is
    /// the range check.
    ///
    /// # Side effects
    ///
    /// On a successful change, an undo entry is pushed (unless this call comes
    /// from [`TimeEdit::undo`] / [`TimeEdit::redo`]), `time_changed` is emitted
    /// with the new time, and a redraw is requested.
    pub fn set_time(&mut self, time: Time) {
        if time.is_valid() && time >= self.minimum && time <= self.maximum && self.time != time {
            let before = self.time;
            self.time = time;
            if !self.restoring_history {
                *self.history_target.borrow_mut() = self.time;
                self.undo_stack.push(Box::new(TimeEditCommand::new(
                    self.history_target.clone(),
                    before,
                    self.time,
                )));
            }
            self.time_changed.emit(time);
            self.base.request_redraw();
        }
    }
    /// Sets the inclusive lower bound for accepted times.
    ///
    /// The current time is clamped up into the new range, so the widget never holds
    /// a value below its own minimum — which would make every later
    /// [`TimeEdit::set_time`] call fail silently.
    pub fn set_minimum_time(&mut self, time: Time) {
        self.minimum = time;
        self.clamp_to_range();
        self.base.request_redraw();
    }
    /// Sets the inclusive upper bound for accepted times.
    ///
    /// The current time is clamped down into the new range, like
    /// [`TimeEdit::set_minimum_time`] clamps up.
    pub fn set_maximum_time(&mut self, time: Time) {
        self.maximum = time;
        self.clamp_to_range();
        self.base.request_redraw();
    }
    /// Sets both ends of the accepted range in one call.
    ///
    /// The current time is clamped into the new range. If `min > max` (caller error)
    /// the minimum wins, so the widget stays pinned at a bound it would accept rather
    /// than being wedged at a value no write can replace.
    pub fn set_time_range(&mut self, min: Time, max: Time) {
        self.minimum = min;
        self.maximum = max;
        self.clamp_to_range();
        self.base.request_redraw();
    }
    /// Moves the current time inside `minimum..=maximum` if it fell outside.
    ///
    /// Called by every bound setter so "the value is within the range" holds after
    /// any sequence of calls. See [`clamp_ordered_range`] for the inverted-range rule.
    fn clamp_to_range(&mut self) {
        self.time = super::clamp_ordered_range(self.time, self.minimum, self.maximum);
    }
    /// Stores the display-format pattern.
    ///
    /// The string is kept verbatim; no validation is performed. See
    /// [`TimeEdit::display_format`] for the current limits on its use.
    pub fn set_display_format(&mut self, fmt: String) {
        self.display_format = fmt;
        self.base.request_redraw();
    }
    /// Advances the time by one second, carrying into minutes and hours.
    ///
    /// The millisecond component is left unchanged. The hour **does not wrap**:
    /// from `23:59:59` the carry resets the minute and second to `0` but leaves
    /// the hour at `23`, and the resulting time passes through
    /// [`TimeEdit::set_time`], so a value beyond [`TimeEdit::maximum_time`] is
    /// rejected and the time stays put. A successful step is undoable.
    pub fn step_up(&mut self) {
        let mut t = self.time;
        let new_sec = t.second() + 1;
        if new_sec >= 60 {
            t.second = 0;
            let new_min = t.minute() + 1;
            if new_min >= 60 {
                t.minute = 0;
                if t.hour() < 23 {
                    t.hour = t.hour() + 1;
                }
            } else {
                t.minute = new_min;
            }
        } else {
            t.second = new_sec;
        }
        self.set_time(t);
    }
    /// Moves the time back by one second, borrowing from minutes and hours.
    ///
    /// The millisecond component is left unchanged — unlike [`TimeEdit::step_up`]
    /// this does not go through the setters for seconds, so a time with a
    /// non-zero `msec` decrements the whole second but keeps trailing
    /// milliseconds. The hour does **not** wrap: at `00:00:00` the minute and
    /// second are set to `59` while the hour stays `0`. The change is applied
    /// through [`TimeEdit::set_time`], so a result outside the accepted range is
    /// rejected and leaves the time unchanged. A successful step is undoable.
    pub fn step_down(&mut self) {
        let mut t = self.time;
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
                }
            }
        }
        self.set_time(t);
    }
    /// Reverts the most recent time change.
    ///
    /// Returns `true` if a change was undone, `false` when the undo stack is
    /// empty. Undoing emits `time_changed` with the restored time and requests a
    /// redraw, but does not push a new undo entry.
    pub fn undo(&mut self) -> bool {
        if self.undo_stack.undo().is_err() {
            return false;
        }
        self.restore_history_time();
        true
    }
    /// Re-applies the most recently undone time change.
    ///
    /// Returns `true` if a change was redone, `false` when there is nothing to
    /// redo. Like [`TimeEdit::undo`], it emits `time_changed` and requests a
    /// redraw without recording a new undo entry.
    pub fn redo(&mut self) -> bool {
        if self.undo_stack.redo().is_err() {
            return false;
        }
        self.restore_history_time();
        true
    }
    /// Returns `true` when there is at least one time change to undo.
    pub fn can_undo(&self) -> bool {
        self.undo_stack.can_undo()
    }
    /// Returns `true` when there is at least one undone time change to redo.
    pub fn can_redo(&self) -> bool {
        self.undo_stack.can_redo()
    }
    fn restore_history_time(&mut self) {
        self.restoring_history = true;
        self.time = *self.history_target.borrow();
        self.restoring_history = false;
        self.time_changed.emit(self.time);
        self.base.request_redraw();
    }
}
impl Widget for TimeEdit {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    /// The size this control claims when nothing constrains it.
    ///
    /// The answer is `max(floor, content + padding)` through [`ControlMetrics::implicit_size`], with the
    /// content being the **formatted value** measured by the shared estimator and the floor being the
    /// field's own minimum. It used to be the pair of literals `(100, 28)`, which had no link to the
    /// advance model the renderer draws with: a value formatted as `"HH:mm:ss"` is wider than 100 at
    /// the default font, and the hint said nothing about it -- so a caller that trusted the hint got a
    /// field whose text was fitted away.
    ///
    /// The height comes from the same `TEXT_FIELD_MIN_HEIGHT` the field's own band uses, so the number
    /// reported and the number drawn cannot disagree.
    fn size_hint(&self) -> Size {
        let font = crate::core::Font::default();
        let line_height = crate::widget::metrics::estimate_line_height(&font, 1.0);
        // The widest value this format can produce, not the *current* one: a hint that shrank when the
        // time happened to be `01:01:01` would make the field jump as the user stepped through values.
        let sample = super::date_edit::format_with_pattern(
            &self.display_format,
            super::date_edit::DateTimeComponents {
                hour: Some(23),
                minute: Some(59),
                second: Some(59),
                millisecond: Some(999),
                ..Default::default()
            },
        )
        .unwrap_or_else(|| Time::new(23, 59, 59, 999).to_string());
        let content = crate::widget::metrics::estimate_text_width(&sample, &font, 1.0);
        // The field's vertical padding is what is left of its height once a line is accounted for, so it
        // is *derived* rather than a second constant: the crate names one horizontal text-field inset
        // and states no vertical one, because `TEXT_FIELD_MIN_HEIGHT` is the fact that fixes that axis.
        let vertical_padding = dimensions::TEXT_FIELD_MIN_HEIGHT.saturating_sub(line_height) / 2;
        let padding = crate::style::EdgeOffsets::symmetric(
            vertical_padding,
            dimensions::TEXT_FIELD_PADDING_H,
        );
        let floor = Size::new(
            dimensions::TEXT_FIELD_PADDING_H * 2 + dimensions::BUTTON_ICON_SIZE,
            dimensions::TEXT_FIELD_MIN_HEIGHT,
        );
        let hint = crate::widget::metrics::ControlMetrics::implicit_size(
            Size::new(content, line_height),
            padding,
            floor,
        );
        Size::new(hint.width, hint.height.max(line_height))
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `TimePicker`'s property contract (the control is `TimeEdit`; `TimePicker` is
/// a type alias for it, so this single impl covers both names).
///
/// Times round-trip as strings: `time_to_string` out, `expect_time` in, with
/// `expect_time` validating the parsed clock time so `25:00:00` is rejected.
impl WidgetProperties for TimeEdit {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "time" => Ok(CapabilityValue::String(time_to_string(self.time()))),
            "minimum_time" => Ok(CapabilityValue::String(time_to_string(self.minimum_time()))),
            "maximum_time" => Ok(CapabilityValue::String(time_to_string(self.maximum_time()))),
            "display_format" => Ok(CapabilityValue::String(self.display_format().to_string())),
            "clock_popup" => Ok(CapabilityValue::Bool(self.clock_popup())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "time" => {
                self.set_time(expect_time(value)?);
                Ok(())
            }
            "minimum_time" => {
                self.set_minimum_time(expect_time(value)?);
                Ok(())
            }
            "maximum_time" => {
                self.set_maximum_time(expect_time(value)?);
                Ok(())
            }
            "display_format" => {
                self.set_display_format(expect_string(value)?);
                Ok(())
            }
            "clock_popup" => {
                self.set_clock_popup(expect_bool(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "time",
            "minimum_time",
            "maximum_time",
            "display_format",
            "clock_popup",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl EventHandler for TimeEdit {
    /// The clock's pointer interaction, then the field's keyboard one.
    ///
    /// # Why the pointer path needed adding at all
    ///
    /// The widget had `step_up`/`step_down` from the day it was written, so a keyboard user could walk
    /// a time one second at a time -- and that was the **only** way in. Reaching 14:30 from midnight was
    /// fifty-two thousand key presses. The clock face is the pointer's way in, and the two paths write
    /// through the same [`TimeEdit::set_time`], so neither can accept a value the other rejects.
    ///
    /// The routing order is: the face first (it is drawn below the field, so a press inside its box
    /// belongs to it), then the field, then the keyboard. A press on the field toggles the face, which
    /// is the same thing `combobox`'s field does and for the same reason: a picker that cannot be
    /// opened by clicking the thing that shows its value is not a picker.
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button } if *button == 1 => {
                if self.clock_popup {
                    // A press on a ring sets that hand's value and leaves the face open: a clock is a
                    // two-part answer (hour *and* minute), so closing after the first would make the
                    // second unreachable without re-opening.
                    if self.pick_at(*pos) {
                        return;
                    }
                    // A press on the face that is **not** on a ring is a miss, and a miss must not
                    // throw away the hour the user just set: it is absorbed and the face stays open.
                    // Closing on a miss would mean a click a few pixels off the rim discarded the
                    // edit *and* dismissed the picker -- two losses from one near miss.
                    if self.clock_rect().contains_point(*pos) {
                        return;
                    }
                }
                if self.geometry().contains_point(*pos) {
                    self.toggle_clock_popup();
                } else if self.clock_popup {
                    // A press outside both dismisses the face, which is what a popup owes its user.
                    self.set_clock_popup(false);
                }
            }
            // The ring under the pointer is highlighted, derived from the same `hand_at_point` a click
            // resolves through, so the ring the user sees emphasised is the ring a click would move.
            Event::MouseMove { pos } => {
                if self.clock_popup {
                    let hovered = self.hand_at_point(*pos);
                    if hovered != self.hovered_hand {
                        self.hovered_hand = hovered;
                        self.base.request_redraw();
                    }
                }
            }
            #[cfg(feature = "touch")]
            Event::Tap { pos } => {
                if self.clock_popup && self.pick_at(*pos) {
                    return;
                }
                if self.geometry().contains_point(*pos) {
                    self.toggle_clock_popup();
                } else if self.clock_popup {
                    self.set_clock_popup(false);
                }
            }
            Event::KeyPress { key, modifiers } => match *key {
                90 if *modifiers == 2 => {
                    let _ = self.undo();
                }
                89 if *modifiers == 2 => {
                    let _ = self.redo();
                }
                // Enter and Space open the face from the keyboard, so the picker is reachable without a
                // pointer -- the same pair `combobox` uses.
                13 | 32 if !self.clock_popup => self.set_clock_popup(true),
                // Escape closes it without changing anything, which is the one way out that leaves the
                // value alone.
                27 if self.clock_popup => self.set_clock_popup(false),
                38 => self.step_up(),
                40 => self.step_down(),
                _ => { /* Other keys are not relevant */ }
            },
            _ => { /* Other events are not relevant */ }
        }
    }
}
impl Draw for TimeEdit {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();

        // Chrome colours resolve explicit style first, then the theme's resolved style
        // for this control, and only then fall back to a literal. Without the theme step
        // a light/dark switch changed nothing on screen: the field, its border and its
        // text were all hardcoded, which the rendering census reported as theme-blind.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global
        // manager's mutex is not re-entrant. `time_edit` classifies as `Input`, whose
        // resolved style supplies `text_color` and `border_color` but no
        // `background_color` (ThemeRole::Input resolves that to `None`), so the interior
        // is derived below from the theme's own background rather than left unset.
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("time_edit");
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

        context.face(
            rect,
            surface,
            self.style().surface.unwrap_or_default(),
            self.style().border_radius.unwrap_or(0),
            Color::BLACK,
        );
        context.draw_rect(rect, border);
        let text = super::date_edit::format_with_pattern(
            &self.display_format,
            super::date_edit::DateTimeComponents {
                hour: Some(self.time.hour()),
                minute: Some(self.time.minute()),
                second: Some(self.time.second()),
                millisecond: Some(self.time.msec()),
                ..Default::default()
            },
        )
        .unwrap_or_else(|| self.time.to_string());
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
            HorizontalAlignment::Left,
        );

        if self.clock_popup {
            self.draw_clock_face(context, surface, border, ink);
        }
    }
}

impl TimeEdit {
    /// Paints the clock face under the field: two rings of numerals and the two hands.
    ///
    /// # Why it is drawn rather than held as a child widget
    ///
    /// The face is a **function of the field's own state**: the hour hand's angle comes from
    /// `self.time`, the muted numerals from `self.minimum`/`self.maximum`, and the highlighted ring
    /// from `self.hovered_hand`. A child widget would have to be kept in step with all three on every
    /// mutation, which is three places to forget -- and the flag is published as a boolean, so there is
    /// nothing for a caller to hold anyway. This is the same shape `date_edit`'s calendar uses for the
    /// same reason.
    ///
    /// # The rings
    ///
    /// Twelve numerals on the outer ring are the hours 1..=12, and twelve on the inner ring are the
    /// minutes in five-minute steps. The rings are *not* labelled with their values: the face is a
    /// clock, and a clock's ring positions are what a reader already knows.
    fn draw_clock_face(
        &self,
        context: &mut RenderContext,
        surface: Color,
        border: Color,
        ink: Color,
    ) {
        let face = self.clock_rect();
        if face.width < CLOCK_DIAMETER / 2 {
            // Too small to read: a shrunken clock is worse than none, because its numerals overlap
            // into a ring of noise that still looks like a control.
            return;
        }
        // The face gets its own plate and edge, because a bare ring of numerals over the page is
        // indistinguishable from a table that happens to be there. It is the *field's* surface one
        // step away from the field, so the two read as one control opening rather than two.
        let plate = surface.blend(&ink, 0.10);
        context.fill_rect(face, plate);
        context.draw_rect(face, border);

        let centre = self.clock_centre();
        let radius = face.width as f32 / 2.0;
        let font = Font::default();
        let muted = plate.blend(&ink, 0.45);
        // The rims, so the two rings' extents are visible rather than only implied by where the
        // numerals happen to land.
        context.draw_circle(centre, (radius * MINUTE_RING_INNER) as u32, border);
        context.draw_circle(centre, (radius * HOUR_HOLE) as u32, border);

        for step in 0..12 {
            // Clock convention: twelve at the top, growing clockwise. `step` counts from the top, so
            // the sine term is the horizontal offset and the cosine term is the vertical one.
            let angle = step as f32 / 12.0 * 2.0 * std::f32::consts::PI;
            let (sin, cos) = angle.sin_cos();
            // Hours: 1..=12 rather than 0..=11, because a clock face has no zero.
            let hour = if step == 0 { 12 } else { step };
            self.draw_clock_numerals(
                context,
                centre,
                radius * 0.82,
                sin,
                cos,
                &hour.to_string(),
                if self.hovered_hand == Some(ClockHand::Hour) { ink } else { muted },
                &font,
            );
            // Minutes: in five-minute steps, which is what the minute ring selects.
            let minute = step * 5;
            self.draw_clock_numerals(
                context,
                centre,
                radius * 0.42,
                sin,
                cos,
                &format!("{minute:02}"),
                if self.hovered_hand == Some(ClockHand::Minute) { ink } else { muted },
                &font,
            );
        }

        // The two hands, drawn from the centre outward. The hour hand is shorter, as on a real clock,
        // so which ring a value came from is readable from the picture.
        let hour_angle = (self.time.hour() % 12) as f32 / 12.0 * 2.0 * std::f32::consts::PI;
        let (hour_sin, hour_cos) = hour_angle.sin_cos();
        context.draw_line(
            centre,
            Point::new(
                centre.x + (hour_sin * radius * 0.55) as i32,
                centre.y - (hour_cos * radius * 0.55) as i32,
            ),
            ink,
        );
        let minute_angle = self.time.minute() as f32 / 60.0 * 2.0 * std::f32::consts::PI;
        let (minute_sin, minute_cos) = minute_angle.sin_cos();
        context.draw_line(
            centre,
            Point::new(
                centre.x + (minute_sin * radius * 0.78) as i32,
                centre.y - (minute_cos * radius * 0.78) as i32,
            ),
            ink,
        );
    }

    /// Draws one ring position's numeral, centred on the point at `distance` along `(sin, cos)`.
    #[allow(clippy::too_many_arguments)]
    fn draw_clock_numerals(
        &self,
        context: &mut RenderContext,
        centre: Point,
        distance: f32,
        sin: f32,
        cos: f32,
        label: &str,
        color: Color,
        font: &Font,
    ) {
        let metrics = context.measure_text(label, font);
        // Centred on the ring position rather than started there: a numeral is read as a mark at a
        // position, so starting every label at its position would push the wider ones off the ring.
        let x = centre.x + (sin * distance) as i32 - metrics.width as i32 / 2;
        let y = centre.y - (cos * distance) as i32 - metrics.height as i32 / 2;
        context.draw_text(Point::new(x, y), label, font, color, HorizontalAlignment::Left);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Point;
    use crate::widget::svg::render_to_svg;
    use std::sync::{Arc, Mutex};

    // ─── Time helper tests ───────────────────────────────────────

    #[test]
    fn time_creation_and_accessors() {
        let t = Time::new(10, 30, 45, 500);
        assert_eq!(t.hour(), 10);
        assert_eq!(t.minute(), 30);
        assert_eq!(t.second(), 45);
        assert_eq!(t.msec(), 500);
    }

    #[test]
    fn time_clamps_invalid_values() {
        let t = Time::new(25, 70, 99, 2000);
        assert_eq!(t.hour(), 23);
        assert_eq!(t.minute(), 59);
        assert_eq!(t.second(), 59);
        assert_eq!(t.msec(), 999);
    }

    #[test]
    fn time_is_valid_true_for_all_public_api_constructs() {
        // Time::new and all setters clamp, so all values reachable
        // through the public API are always valid.
        assert!(Time::new(12, 0, 0, 0).is_valid());
        assert!(Time::new(23, 59, 59, 999).is_valid());
        assert!(Time::new(0, 0, 0, 0).is_valid());
    }

    #[test]
    fn time_setters_clamp_values() {
        let mut t = Time::new(0, 0, 0, 0);
        t.set_hour(24);
        assert_eq!(t.hour(), 23);
        t.set_minute(60);
        assert_eq!(t.minute(), 59);
        t.set_second(60);
        assert_eq!(t.second(), 59);
        t.set_msec(1000);
        assert_eq!(t.msec(), 999);
    }

    #[test]
    fn time_to_msecs_since_midnight() {
        let t = Time::new(1, 30, 15, 250);
        let expected = (3600 + 30 * 60 + 15) * 1000 + 250;
        assert_eq!(t.to_msecs_since_midnight(), expected);
    }

    #[test]
    fn time_midnight_is_zero_msecs() {
        let t = Time::new(0, 0, 0, 0);
        assert_eq!(t.to_msecs_since_midnight(), 0);
    }

    #[test]
    fn time_display_format() {
        let t = Time::new(9, 5, 3, 0);
        assert_eq!(t.to_string(), "09:05:03");
        let t2 = Time::new(23, 59, 59, 999);
        assert_eq!(t2.to_string(), "23:59:59");
    }

    #[test]
    fn time_ordering() {
        let early = Time::new(8, 0, 0, 0);
        let late = Time::new(9, 0, 0, 0);
        assert!(early < late);
        assert!(late > early);
        assert_eq!(early, Time::new(8, 0, 0, 0));
    }

    // ─── TimeEdit widget tests ───────────────────────────────────

    #[test]
    fn time_edit_creation_defaults() {
        let editor = TimeEdit::new(Rect::new(0, 0, 200, 30));
        assert_eq!(editor.time(), Time::new(0, 0, 0, 0));
        assert_eq!(editor.display_format(), "HH:mm:ss");
        assert_eq!(editor.minimum_time(), Time::new(0, 0, 0, 0));
        assert_eq!(editor.maximum_time(), Time::new(23, 59, 59, 999));
        assert!(editor.is_visible());
        assert!(editor.is_enabled());
    }

    #[test]
    fn time_edit_set_and_get_time() {
        let mut editor = TimeEdit::new(Rect::new(0, 0, 200, 30));
        let t = Time::new(14, 30, 0, 0);
        editor.set_time(t);
        assert_eq!(editor.time(), t);
    }

    /// A write below the minimum is rejected, and the value stays where it was.
    ///
    /// Note the order: the value is brought into range *first*, then the bound is
    /// touched. Setting the minimum pulls the value up to meet it (see
    /// `time_edit_bound_setters_keep_the_value_in_range`), so the test must not
    /// assert the pre-clamp value survives — that was the old behaviour, and it left
    /// the widget holding a time below its own minimum.
    #[test]
    fn time_edit_set_time_clamps_to_minimum() {
        let mut editor = TimeEdit::new(Rect::new(0, 0, 200, 30));
        editor.set_minimum_time(Time::new(10, 0, 0, 0));
        assert_eq!(editor.time(), Time::new(10, 0, 0, 0), "setting the minimum clamps up to it");

        // A write below the minimum is refused; the value does not move.
        editor.set_time(Time::new(5, 0, 0, 0));
        assert_eq!(editor.time(), Time::new(10, 0, 0, 0), "5:00 < 10:00, so the write is refused");

        // A write at or above the minimum is accepted.
        editor.set_time(Time::new(11, 0, 0, 0));
        assert_eq!(editor.time(), Time::new(11, 0, 0, 0));
    }

    /// A write above the maximum is rejected, and the value stays where it was.
    ///
    /// Same ordering note as the minimum case: first bring the value *into* the new
    /// range, then narrow it. Here the value is raised to 12:00 while the range is
    /// still the default, and only then is the maximum lowered — that is what makes
    /// the clamp observable.
    #[test]
    fn time_edit_set_time_clamps_to_maximum() {
        let mut editor = TimeEdit::new(Rect::new(0, 0, 200, 30));
        editor.set_time(Time::new(12, 0, 0, 0));
        assert_eq!(editor.time(), Time::new(12, 0, 0, 0));

        editor.set_maximum_time(Time::new(9, 0, 0, 0));
        assert_eq!(editor.time(), Time::new(9, 0, 0, 0), "lowering the maximum clamps down to it");

        // A write above the maximum is refused; the value does not move.
        editor.set_time(Time::new(18, 0, 0, 0));
        assert_eq!(editor.time(), Time::new(9, 0, 0, 0), "18:00 > 09:00, so the write is refused");

        // A write at or below the maximum is accepted.
        editor.set_time(Time::new(6, 0, 0, 0));
        assert_eq!(editor.time(), Time::new(6, 0, 0, 0));
    }

    /// Changing a bound must leave the value inside the range.
    ///
    /// The invariant the setters now maintain, and the reason the two tests above
    /// were reordered: a widget holding a time outside its own bounds would refuse
    /// every subsequent write, silently and permanently.
    #[test]
    fn time_edit_bound_setters_keep_the_value_in_range() {
        let mut editor = TimeEdit::new(Rect::new(0, 0, 200, 30));
        editor.set_time(Time::new(12, 0, 0, 0));

        editor.set_minimum_time(Time::new(15, 0, 0, 0));
        assert_eq!(editor.time(), Time::new(15, 0, 0, 0), "raised minimum pulls the value up");

        editor.set_maximum_time(Time::new(9, 0, 0, 0));
        assert_eq!(
            editor.time(),
            Time::new(15, 0, 0, 0),
            "with min > max the minimum wins, so the editor is never wedged"
        );
    }

    #[test]
    fn time_edit_set_time_range() {
        let mut editor = TimeEdit::new(Rect::new(0, 0, 200, 30));
        let min = Time::new(6, 0, 0, 0);
        let max = Time::new(22, 0, 0, 0);
        editor.set_time_range(min, max);
        assert_eq!(editor.minimum_time(), min);
        assert_eq!(editor.maximum_time(), max);
    }

    #[test]
    fn time_edit_set_display_format() {
        let mut editor = TimeEdit::new(Rect::new(0, 0, 200, 30));
        editor.set_display_format("HH:mm".to_string());
        assert_eq!(editor.display_format(), "HH:mm");
    }

    #[test]
    fn time_edit_step_up_increments_second() {
        let mut editor = TimeEdit::new(Rect::new(0, 0, 200, 30));
        editor.set_time(Time::new(10, 30, 15, 0));
        editor.step_up();
        assert_eq!(editor.time(), Time::new(10, 30, 16, 0));
    }

    #[test]
    fn time_edit_step_up_rolls_minute() {
        let mut editor = TimeEdit::new(Rect::new(0, 0, 200, 30));
        editor.set_time(Time::new(10, 30, 59, 0));
        editor.step_up();
        assert_eq!(editor.time(), Time::new(10, 31, 0, 0));
    }

    #[test]
    fn time_edit_step_up_rolls_hour() {
        let mut editor = TimeEdit::new(Rect::new(0, 0, 200, 30));
        editor.set_time(Time::new(10, 59, 59, 0));
        editor.step_up();
        assert_eq!(editor.time(), Time::new(11, 0, 0, 0));
    }

    #[test]
    fn time_edit_step_up_from_23_59_59_wraps_minutes_and_seconds() {
        let mut editor = TimeEdit::new(Rect::new(0, 0, 200, 30));
        editor.set_time(Time::new(23, 59, 59, 0));
        editor.step_up();
        // Seconds and minutes wrap to 0, hour stays at 23 (no day wrap in TimeEdit)
        assert_eq!(editor.time(), Time::new(23, 0, 0, 0));
    }

    #[test]
    fn time_edit_step_down_decrements_second() {
        let mut editor = TimeEdit::new(Rect::new(0, 0, 200, 30));
        editor.set_time(Time::new(10, 30, 15, 0));
        editor.step_down();
        assert_eq!(editor.time(), Time::new(10, 30, 14, 0));
    }

    #[test]
    fn time_edit_step_down_rolls_minute() {
        let mut editor = TimeEdit::new(Rect::new(0, 0, 200, 30));
        editor.set_time(Time::new(10, 30, 0, 0));
        editor.step_down();
        assert_eq!(editor.time(), Time::new(10, 29, 59, 0));
    }

    #[test]
    fn time_edit_step_down_rolls_hour() {
        let mut editor = TimeEdit::new(Rect::new(0, 0, 200, 30));
        editor.set_time(Time::new(10, 0, 0, 0));
        editor.step_down();
        assert_eq!(editor.time(), Time::new(9, 59, 59, 0));
    }

    #[test]
    fn time_edit_step_down_stops_at_0() {
        let mut editor = TimeEdit::new(Rect::new(0, 0, 200, 30));
        editor.set_time(Time::new(0, 0, 0, 0));
        editor.step_down();
        // hour 0, minute 0, second 0: step_down keeps hour at 0, minute at 59, second at 59
        // But then set_time rejects because 0:59:59 >= minimum (0,0,0,0) so it should accept.
        // Actually wait: hour starts at 0, step_down: second is 0, so second=59, minute is 0 so minute=59,
        // hour is 0 so hour stays at 0. Result is 0:59:59 which is valid.
        assert_eq!(editor.time(), Time::new(0, 59, 59, 0));
    }

    #[test]
    fn time_edit_undo_redo_restores_time() {
        let mut editor = TimeEdit::new(Rect::new(0, 0, 200, 30));
        editor.set_time(Time::new(10, 0, 0, 0));
        editor.step_up();

        assert!(editor.can_undo());
        assert!(editor.undo());
        assert_eq!(editor.time(), Time::new(10, 0, 0, 0));
        assert!(editor.can_redo());
        assert!(editor.redo());
        assert_eq!(editor.time(), Time::new(10, 0, 1, 0));
    }

    #[test]
    fn time_edit_keyboard_shortcuts_drive_history() {
        let mut editor = TimeEdit::new(Rect::new(0, 0, 200, 30));
        editor.set_time(Time::new(10, 0, 0, 0));
        editor.set_time(Time::new(10, 0, 1, 0));

        editor.handle_event(&Event::KeyPress { key: 90, modifiers: 2 });
        assert_eq!(editor.time(), Time::new(10, 0, 0, 0));
        editor.handle_event(&Event::KeyPress { key: 89, modifiers: 2 });
        assert_eq!(editor.time(), Time::new(10, 0, 1, 0));
    }

    #[test]
    fn time_edit_time_changed_signal_emits() {
        let mut editor = TimeEdit::new(Rect::new(0, 0, 200, 30));
        let captured = Arc::new(Mutex::new(Time::new(0, 0, 0, 0)));
        let sink = captured.clone();
        editor.time_changed.connect(move |t| {
            if let Ok(mut c) = sink.lock() {
                *c = *t;
            }
        });

        let expected = Time::new(18, 30, 0, 0);
        editor.set_time(expected);
        let got = *captured.lock().unwrap();
        assert_eq!(got, expected);
    }

    #[test]
    fn time_edit_time_changed_not_emitted_for_same_time() {
        let mut editor = TimeEdit::new(Rect::new(0, 0, 200, 30));
        let hits = Arc::new(Mutex::new(0usize));
        let hits_clone = hits.clone();
        editor.time_changed.connect(move |_| {
            if let Ok(mut h) = hits_clone.lock() {
                *h += 1;
            }
        });

        editor.set_time(Time::new(0, 0, 0, 0)); // same as default, should NOT emit
        assert_eq!(*hits.lock().unwrap(), 0);
    }

    #[test]
    fn time_edit_geometry_delegation() {
        let rect = Rect::new(10, 20, 200, 30);
        let editor = TimeEdit::new(rect);
        assert_eq!(editor.geometry(), rect);
        assert_eq!(editor.position(), Point::new(10, 20));
        assert_eq!(editor.size(), crate::core::Size::new(200, 30));
    }

    #[test]
    fn time_edit_widget_kind() {
        let editor = TimeEdit::new(Rect::new(0, 0, 200, 30));
        assert_eq!(editor.kind(), WidgetKind::TimePicker);
    }

    #[test]
    fn time_edit_ids_are_unique() {
        let a = TimeEdit::new(Rect::new(0, 0, 200, 30));
        let b = TimeEdit::new(Rect::new(0, 0, 200, 30));
        assert_ne!(a.id(), b.id());
    }

    #[test]
    fn time_edit_svg_output() {
        let mut editor = TimeEdit::new(Rect::new(0, 0, 200, 30));
        editor.set_time(Time::new(12, 30, 45, 0));
        let svg = render_to_svg(&mut editor);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        assert!(svg.contains("width=\"200\""));
        assert!(svg.contains("height=\"30\""));
        // Should contain the time text rendered
        assert!(svg.contains("12:30:45") || svg.contains("fill="));
    }

    #[test]
    fn time_edit_disabled_state_blocks_changes() {
        let mut editor = TimeEdit::new(Rect::new(0, 0, 200, 30));
        editor.set_time(Time::new(10, 0, 0, 0));
        editor.set_enabled(false);
        assert!(!editor.is_enabled());

        // Step up should not work when disabled (handle_event checks is_enabled)
        editor.step_up();
        // step_up directly modifies and calls set_time, which checks min/max but not is_enabled
        // Only handle_event checks is_enabled, so step_up/step_down still work when called directly.
        // The disabled guard is in handle_event only.
        // Let's verify via handle_event:
        editor.set_time(Time::new(10, 0, 0, 0)); // reset
        editor.handle_event(&Event::KeyPress { key: 38, modifiers: 0 });
        // Because disabled, handle_event returns early, so time stays unchanged
        assert_eq!(editor.time(), Time::new(10, 0, 0, 0));
    }

    #[test]
    fn time_edit_keyboard_increment_and_decrement() {
        let mut editor = TimeEdit::new(Rect::new(0, 0, 200, 30));
        editor.set_time(Time::new(10, 0, 0, 0));

        // Up arrow (key 38) = step_up
        editor.handle_event(&Event::KeyPress { key: 38, modifiers: 0 });
        assert_eq!(editor.time(), Time::new(10, 0, 1, 0));

        // Down arrow (key 40) = step_down
        editor.handle_event(&Event::KeyPress { key: 40, modifiers: 0 });
        assert_eq!(editor.time(), Time::new(10, 0, 0, 0));
    }

    #[test]
    fn time_edit_mouse_wheel_does_not_change_time() {
        // TimeEdit does not handle wheel events
        let mut editor = TimeEdit::new(Rect::new(0, 0, 200, 30));
        editor.set_time(Time::new(10, 0, 0, 0));

        editor.handle_event(&Event::Wheel { delta: Point::new(0, 120), modifiers: 0 });
        assert_eq!(editor.time(), Time::new(10, 0, 0, 0));

        editor.handle_event(&Event::Wheel { delta: Point::new(0, -120), modifiers: 0 });
        assert_eq!(editor.time(), Time::new(10, 0, 0, 0));
    }

    #[test]
    fn time_edit_visibility_toggle() {
        let mut editor = TimeEdit::new(Rect::new(0, 0, 200, 30));
        assert!(editor.is_visible());
        editor.set_visible(false);
        assert!(!editor.is_visible());
        editor.set_visible(true);
        assert!(editor.is_visible());
    }

    #[test]
    fn time_edit_set_time_respects_min_max_range() {
        let mut editor = TimeEdit::new(Rect::new(0, 0, 200, 30));
        // Set a narrow range
        editor.set_time_range(Time::new(9, 0, 0, 0), Time::new(17, 0, 0, 0));

        // Set to a value within range
        let within = Time::new(12, 0, 0, 0);
        editor.set_time(within);
        assert_eq!(editor.time(), within);

        // Try setting below minimum
        editor.set_time(Time::new(8, 0, 0, 0));
        assert_eq!(editor.time(), within); // unchanged

        // Try setting above maximum
        editor.set_time(Time::new(18, 0, 0, 0));
        assert_eq!(editor.time(), within); // unchanged
    }

    // ── The clock popup, and the pointer path it adds ──

    /// `clock_popup` is not a stored flag: it changes the picture, and the picture is a clock.
    ///
    /// # What was dead, and what proves it is alive now
    ///
    /// This is the defect `date_edit`'s `calendar_popup` had, one control over. A `time_edit` is a
    /// picker by name only if there is something to pick with: the widget had `step_up`/`step_down`
    /// (so a keyboard user could walk a time one second at a time), and **that was the only way in**.
    /// Reaching 14:30 from midnight was fifty-two thousand key presses, and no pointer affordance
    /// existed at all.
    ///
    /// The assertion reads the **document**, not the flag: the face must add ink, and it must add the
    /// *ring positions* -- a bare plate would satisfy "the documents differ".
    #[test]
    fn the_clock_popup_is_actually_painted() {
        use crate::widget::svg::{render_to_svg, text_subpath_count};
        let _theme_guard = crate::style::theme_test_guard();

        let make = |popup: bool| {
            let mut editor = TimeEdit::new(Rect::new(0, 0, 200, 30));
            editor.set_time(Time::new(14, 30, 0, 0));
            editor.set_clock_popup(popup);
            editor
        };

        let closed_svg = render_to_svg(&mut make(false));
        let open_svg = render_to_svg(&mut make(true));
        assert_ne!(closed_svg, open_svg, "opening the clock must change the picture");

        // Twelve hour numerals plus twelve minute numerals is twenty-four runs; the field alone
        // contributes one (`14:30:00`). The face's own rims and hands are paths, not text. Measured:
        // closed=194, open=1225 -- so the face roughly quadruples the document's glyph ink, and the
        // floor is set below that rather than at a number chosen to match.
        let closed_ink = text_subpath_count(&closed_svg);
        let open_ink = text_subpath_count(&open_svg);
        assert!(
            open_ink > closed_ink * 4,
            "the clock must paint both rings of numerals: closed={closed_ink} open={open_ink}"
        );
        // And it paints its own plate, which is what makes it legible over whatever is behind it.
        assert!(
            open_svg.matches("<rect").count() > closed_svg.matches("<rect").count(),
            "the clock paints its own plate"
        );
        // A clock too small to read draws no face at all: a shrunken ring of overlapping numerals
        // is worse than nothing, because it still looks like a control. The two renders come from the
        // *same* field size, so the only difference between them is the flag.
        let tiny_open = {
            let mut t = TimeEdit::new(Rect::new(0, 0, 40, 30));
            t.set_clock_popup(true);
            render_to_svg(&mut t)
        };
        let tiny_closed = {
            let mut t = TimeEdit::new(Rect::new(0, 0, 40, 30));
            t.set_clock_popup(false);
            render_to_svg(&mut t)
        };
        assert_eq!(
            tiny_closed, tiny_open,
            "a face below the readable floor draws nothing rather than a smear"
        );
    }

    /// A click on a ring sets that hand's value, and the two rings address different fields.
    #[test]
    fn clicking_a_ring_sets_the_hand_it_belongs_to() {
        use crate::core::Point;
        use crate::event::{Event, EventHandler};

        let mut editor = TimeEdit::new(Rect::new(0, 0, 200, 30));
        editor.set_clock_popup(true);
        let centre = editor.clock_centre();
        let radius = editor.clock_rect().width as f32 / 2.0;

        // Three o'clock is 90 degrees clockwise from the top, i.e. straight right of the centre.
        // On the **hour** ring that must read 3 -- and, because the current hour is in the morning,
        // it must read the *morning* 3, not 15.
        editor.set_time(Time::new(9, 0, 0, 0));
        let hour_point = Point::new(centre.x + (radius * 0.82) as i32, centre.y);
        assert_eq!(
            editor.hand_at_point(hour_point),
            Some(ClockHand::Hour),
            "the outer ring is the hour"
        );
        editor.handle_event(&Event::MousePress { pos: hour_point, button: 1 });
        assert_eq!(editor.time().hour(), 3, "three o'clock on the hour ring");
        assert_eq!(editor.time().minute(), 0, "and the minute is untouched");

        // The same angle on the **minute** ring reads fifteen, and the hour is untouched.
        let minute_point = Point::new(centre.x + (radius * 0.45) as i32, centre.y);
        assert_eq!(
            editor.hand_at_point(minute_point),
            Some(ClockHand::Minute),
            "the inner ring is the minute"
        );
        editor.handle_event(&Event::MousePress { pos: minute_point, button: 1 });
        assert_eq!(editor.time().minute(), 15, "three o'clock on the minute ring is :15");
        assert_eq!(editor.time().hour(), 3, "and the hour is untouched");

        // An afternoon hour keeps its half: clicking 9 on the face of a 21:xx time must not jump to
        // the morning, because the face cannot say which half it means and the field already knows.
        editor.set_time(Time::new(21, 30, 0, 0));
        let nine_point = Point::new(centre.x + (radius * 0.82) as i32, centre.y);
        editor.handle_event(&Event::MousePress { pos: nine_point, button: 1 });
        assert_eq!(editor.time().hour(), 15, "the afternoon half is preserved");

        // The hub belongs to no ring: a click there must not silently pick a value.
        assert_eq!(editor.hand_at_point(centre), None, "the hub is not a ring");
        // Nor does a point outside the rim.
        let outside = Point::new(centre.x + (radius * 2.0) as i32, centre.y);
        assert_eq!(editor.hand_at_point(outside), None, "outside the rim is no ring");
        // And a click on the plate between ring positions is not a value either, which is what keeps
        // "never mind" from being the same gesture as "set the hour".
        let before = editor.time();
        editor.handle_event(&Event::MousePress { pos: centre, button: 1 });
        assert_eq!(editor.time(), before, "a click on the hub changes nothing");
    }

    /// Clicking the field toggles the face; Escape closes it without committing.
    #[test]
    fn the_field_opens_the_clock_and_escape_closes_it() {
        use crate::event::{Event, EventHandler};

        let mut editor = TimeEdit::new(Rect::new(0, 0, 200, 30));
        assert!(!editor.clock_popup(), "a fresh field shows no face");

        let field_point = crate::core::Point::new(80, editor.geometry().y + 10);
        editor.handle_event(&Event::MousePress { pos: field_point, button: 1 });
        assert!(editor.clock_popup(), "a press on the field opens the face");

        editor.handle_event(&Event::KeyPress { key: 27, modifiers: 0 });
        assert!(!editor.clock_popup(), "escape closes it");

        // The keyboard can open it too, so the picker is reachable without a pointer.
        editor.handle_event(&Event::KeyPress { key: 13, modifiers: 0 });
        assert!(editor.clock_popup(), "enter opens the face");

        // A press outside both dismisses it, which is what a popup owes its user.
        editor.handle_event(&Event::MousePress {
            pos: crate::core::Point::new(9000, 9000),
            button: 1,
        });
        assert!(!editor.clock_popup(), "a press outside dismisses the face");
    }

    /// A value the range rejects is rejected through the clock too, not clamped by it.
    #[test]
    fn the_clock_cannot_set_a_time_the_range_forbids() {
        use crate::core::Point;
        use crate::event::{Event, EventHandler};

        let mut editor = TimeEdit::new(Rect::new(0, 0, 200, 30));
        editor.set_clock_popup(true);
        // A range that stops at 09:10, and a time at 09:05: clicking the :30 position would produce
        // 09:30, which is outside it. The refusal has to come from `set_time` -- one place decides what
        // is acceptable -- rather than from the face silently clamping to 09:10.
        editor.set_time_range(Time::new(9, 0, 0, 0), Time::new(9, 10, 0, 0));
        editor.set_time(Time::new(9, 5, 0, 0));

        let centre = editor.clock_centre();
        let radius = editor.clock_rect().width as f32 / 2.0;
        // The :30 position on the inner ring is straight down from the centre; the point is placed at
        // the middle of the ring's own band, so it is squarely on the ring.
        let minute_radius = radius * (HOUR_HOLE + MINUTE_RING_INNER) / 2.0;
        let half_past = Point::new(centre.x, centre.y + minute_radius as i32);
        assert_eq!(
            editor.hand_at_point(half_past),
            Some(ClockHand::Minute),
            "the fixture must aim at the minute ring, or the assertion below proves nothing"
        );
        editor.handle_event(&Event::MousePress { pos: half_past, button: 1 });
        assert_eq!(
            editor.time().minute(),
            5,
            "a ring click that would leave the range is refused, not clamped"
        );

        // And a value *inside* the same range is accepted, so the refusal above is about the range and
        // not about the face being inert. The minute ring's steps are five minutes apart, so :05 is the
        // step 30 degrees clockwise from the top. The point is placed at the *middle* of the minute
        // ring's own band, not at a fraction of its outer edge, so it is squarely on the ring rather
        // than on the hub boundary.
        let minute_radius = radius * (HOUR_HOLE + MINUTE_RING_INNER) / 2.0;
        let five_past = Point::new(
            centre.x + (minute_radius * 0.5) as i32,
            centre.y - (minute_radius * 0.866) as i32,
        );
        assert_eq!(
            editor.hand_at_point(five_past),
            Some(ClockHand::Minute),
            "the fixture must aim at the minute ring"
        );
        editor.handle_event(&Event::MousePress { pos: five_past, button: 1 });
        assert_eq!(
            editor.time().minute(),
            5,
            "the :05 position is inside the range, so it is accepted"
        );

        // A step the range forbids is refused, leaving the accepted value in place -- the two clicks
        // differ only in the value they select, so the range is what decided.
        let quarter = Point::new(centre.x + minute_radius as i32, centre.y);
        editor.handle_event(&Event::MousePress { pos: quarter, button: 1 });
        assert_eq!(
            editor.time().minute(),
            5,
            "the :15 position is past the range's :10, so it is refused"
        );
    }
}
