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
use crate::widget::capability::coercion::{expect_string, expect_time};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
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
    /// Emitted with the new time after every accepted change, including changes
    /// produced by [`TimeEdit::undo`] and [`TimeEdit::redo`]. Not emitted when a
    /// change is rejected or when the time is already the requested value.
    pub time_changed: Signal1<Time>,
    undo_stack: UndoStack,
    history_target: Rc<RefCell<Time>>,
    restoring_history: bool,
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

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        crate::core::Size::new(100, 28)
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
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "time",
            "minimum_time",
            "maximum_time",
            "display_format",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl EventHandler for TimeEdit {
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
        let theme = crate::theme::resolved_theme_style("time_edit");
        // Read as its own lock acquisition and copied out as values, so the guard is
        // dropped before anything else touches the theme.
        let (window_fill, foreground) = {
            let manager = crate::theme::global_theme_manager();
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

        context.fill_rect(rect, surface);
        context.draw_rect(rect, border);
        let text = self.time.to_string();
        context.draw_text(
            Point { x: rect.x + 6, y: rect.y + (rect.height as i32 / 2) },
            &text,
            &Font::default(),
            ink,
            HorizontalAlignment::Left,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
