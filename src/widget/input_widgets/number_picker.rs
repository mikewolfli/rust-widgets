// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Number picker — a vertically scrolling digit wheel.
//!
//! # Why this is not `SpinBox`
//!
//! `SpinBox` is the desktop stepper: a text field with up/down buttons, where the
//! value is typed as often as it is stepped. A picker is the touch idiom (iOS
//! `UIPickerView`, Android `NumberPicker`): a reel of numbers where the selection
//! is made by **scrolling**, the neighbours are visible above and below, and the
//! interaction is a drag or a flick rather than a click per increment.
//!
//! The two differ in state as well as in feel: a picker keeps a scroll offset and
//! knows how many rows are on screen, which is what makes a flick decelerate. None
//! of that exists on a stepper, so this is a separate control rather than a mode of
//! `SpinBox`.
//!
//! # Reachability
//!
//! This is a new control, so it is registered in the widget factory (and therefore
//! reachable by name from the declarative path and CSS selectors), publishes a
//! property contract, and has interaction tests.

use core::fmt::Write as _;

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler, WheelScroll};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::{expect_bool, expect_i64, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Height of one wheel row, in logical pixels.
const DEFAULT_ROW_HEIGHT: u32 = 28;

/// A scroll-wheel numeric selector.
pub struct NumberPicker {
    base: BaseWidget,
    value: i64,
    minimum: i64,
    maximum: i64,
    step: i64,
    /// When true, stepping past an end wraps to the other end instead of stopping.
    wrap: bool,
    /// Text appended to the displayed number, e.g. `" kg"`.
    suffix: String,
    /// Text prepended to the displayed number, e.g. `"$"`.
    prefix: String,
    /// How many decimal places the stored integer is displayed with.
    ///
    /// The value stays an `i64` scaled by `10^decimals`, which keeps the grid, the
    /// wrapping arithmetic and the signal payload exact. Displaying `1234` with two
    /// decimals yields `12.34`.
    decimals: u8,
    /// The value that shows `special_value_text` instead of a number.
    special_value: Option<i64>,
    /// The text shown for [`Self::special_value`].
    special_value_text: Option<String>,
    /// Digits typed since the last commit, shown in place of the selected row.
    entry_buffer: String,
    /// Whether the user is currently typing a value.
    entering: bool,
    /// Sub-row visual offset, in rows, used to settle a drag animation.
    settle_offset: f32,
    /// Whether a settle animation is running.
    settling: bool,
    row_height: u32,
    /// Drag/flick state: the y coordinate the current drag started at.
    drag_origin_y: Option<i32>,
    /// Value when the drag started, so a drag is always computed from the start
    /// rather than accumulated — otherwise rounding would drift mid-drag.
    drag_origin_value: i64,
    /// Emitted with the new value whenever it changes, including when a drag
    /// settles on a different row. Not emitted when the value is unchanged.
    pub value_changed: Signal1<i64>,
}

impl NumberPicker {
    /// Creates a picker over `0..=100`, starting at `0`.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::NumberPicker, geometry, "NumberPicker"),
            value: 0,
            minimum: 0,
            maximum: 100,
            step: 1,
            wrap: false,
            suffix: String::new(),
            prefix: String::new(),
            decimals: 0,
            special_value: None,
            special_value_text: None,
            entry_buffer: String::new(),
            entering: false,
            settle_offset: 0.0,
            settling: false,
            row_height: DEFAULT_ROW_HEIGHT,
            drag_origin_y: None,
            drag_origin_value: 0,
            value_changed: Signal1::new(),
        }
    }

    /// Returns the current value.
    pub fn value(&self) -> i64 {
        self.value
    }

    /// Sets the value, clamped to the range and snapped to the step grid.
    ///
    /// Emits `value_changed` only when the result differs from the current value,
    /// so a caller reacting to the signal cannot loop.
    pub fn set_value(&mut self, value: i64) {
        let next = self.snap(value);
        if self.value == next {
            return;
        }
        self.value = next;
        self.value_changed.emit(next);
        self.base.request_redraw();
    }

    /// Returns the lower bound.
    pub fn minimum(&self) -> i64 {
        self.minimum
    }

    /// Sets the lower bound; the current value is re-snapped so range and value
    /// cannot disagree.
    pub fn set_minimum(&mut self, minimum: i64) {
        self.set_range(minimum, self.maximum);
    }

    /// Returns the upper bound.
    pub fn maximum(&self) -> i64 {
        self.maximum
    }

    /// Sets the upper bound.
    pub fn set_maximum(&mut self, maximum: i64) {
        self.set_range(self.minimum, maximum);
    }

    /// Sets both bounds at once, normalising an inverted pair and re-snapping the
    /// current value into the new range.
    pub fn set_range(&mut self, minimum: i64, maximum: i64) {
        self.minimum = minimum.min(maximum);
        self.maximum = maximum.max(minimum);
        let snapped = self.snap(self.value);
        if self.value != snapped {
            self.value = snapped;
            self.value_changed.emit(snapped);
        }
        self.base.request_redraw();
    }

    /// Returns the step between selectable values.
    pub fn step(&self) -> i64 {
        self.step
    }

    /// Sets the step. A step below 1 is treated as 1, because a zero step would
    /// make the value grid degenerate (every value always "snapped").
    pub fn set_step(&mut self, step: i64) {
        self.step = step.max(1);
        let snapped = self.snap(self.value);
        if self.value != snapped {
            self.value = snapped;
            self.value_changed.emit(snapped);
        }
        self.base.request_redraw();
    }

    /// Returns whether the ends wrap.
    pub fn wrap(&self) -> bool {
        self.wrap
    }

    /// Sets whether stepping past an end wraps around.
    pub fn set_wrap(&mut self, wrap: bool) {
        self.wrap = wrap;
    }

    /// Returns the suffix shown after the number.
    pub fn suffix(&self) -> &str {
        &self.suffix
    }

    /// Returns the prefix shown before the number.
    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    /// Sets the prefix shown before the number.
    ///
    /// Display only, like the suffix: the stored value is unchanged.
    pub fn set_prefix(&mut self, prefix: impl Into<String>) {
        let prefix = prefix.into();
        if self.prefix == prefix {
            return;
        }
        self.prefix = prefix;
        self.base.request_redraw();
    }

    /// How many decimal places the value is displayed with.
    pub fn decimals(&self) -> u8 {
        self.decimals
    }

    /// Sets how many decimal places the value is displayed with.
    ///
    /// The value is interpreted as `10^decimals` units of the displayed number, so a
    /// picker holding `1234` with two decimals reads `12.34`. Values above nine
    /// decimals are clamped: the display scale would overflow the integer type.
    pub fn set_decimals(&mut self, decimals: u8) {
        let decimals = decimals.min(9);
        if self.decimals == decimals {
            return;
        }
        self.decimals = decimals;
        self.base.request_redraw();
    }

    /// The value that shows a text label instead of a number.
    pub fn special_value(&self) -> Option<i64> {
        self.special_value
    }

    /// The text shown for [`Self::special_value`] (I-105).
    pub fn special_value_text(&self) -> Option<&str> {
        self.special_value_text.as_deref()
    }

    /// Makes `value` display `text` instead of its number.
    ///
    /// This is the "auto" or "mixed" position every platform's picker has: a
    /// magnitude that has no meaningful number reads as a word.
    pub fn set_special_value(&mut self, value: Option<i64>, text: Option<String>) {
        let text = text.filter(|text| !text.is_empty());
        if self.special_value == value && self.special_value_text == text {
            return;
        }
        self.special_value = value;
        self.special_value_text = text;
        self.base.request_redraw();
    }

    /// The label shown for one value, including prefix, decimals, suffix and special text.
    pub fn display_text(&self, value: i64) -> String {
        let mut out = String::new();
        self.write_display_text(&mut out, value);
        out
    }

    /// Renders the display label into `out`, so the paint path reuses one buffer (I-P68).
    fn write_display_text(&self, out: &mut String, value: i64) {
        if self.special_value == Some(value) {
            if let Some(text) = &self.special_value_text {
                out.push_str(text);
                return;
            }
        }
        out.push_str(&self.prefix);
        if self.decimals == 0 {
            let _ = write!(out, "{value}");
        } else {
            let negative = value < 0;
            let magnitude = value.unsigned_abs();
            let divisor = 10u64.pow(self.decimals as u32);
            let whole = magnitude / divisor;
            let fraction = magnitude % divisor;
            if negative {
                out.push('-');
            }
            let _ = write!(out, "{whole}.");
            let fraction_text = fraction.to_string();
            for _ in fraction_text.len()..self.decimals as usize {
                out.push('0');
            }
            out.push_str(&fraction_text);
        }
        out.push_str(&self.suffix);
    }

    /// Whether a settle animation is still running.
    pub fn is_settling(&self) -> bool {
        self.settling
    }

    /// Advances the settle animation by `delta_ms`. Returns whether another frame is due.
    ///
    /// The drag snaps to a row on release; this eases the sub-row remainder away so the wheel
    /// finishes with a decelerating nudge instead of a one-frame jump.
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        if !self.settling {
            return false;
        }
        let frames = (delta_ms as f32 / 16.0).max(1.0);
        let decay = 0.78f32.powf(frames);
        self.settle_offset *= decay;
        if self.settle_offset.abs() < 0.01 {
            self.settle_offset = 0.0;
            self.settling = false;
        } else {
            self.base.request_redraw();
        }
        self.settling
    }

    /// Drops the sub-row settle remainder, returning the wheel to rest.
    ///
    /// This is the single clear path for the inertia buffer: the wheel gesture and the
    /// settle animation's own finish both use it.
    fn clear_inertia(&mut self) {
        self.settle_offset = 0.0;
        self.settling = false;
    }

    /// Sets the suffix shown after the number. Display only: it is not part of the
    /// value, so a caller parsing the control's text never sees it.
    pub fn set_suffix(&mut self, suffix: impl Into<String>) {
        let suffix = suffix.into();
        if self.suffix == suffix {
            return;
        }
        self.suffix = suffix;
        self.base.request_redraw();
    }

    /// Returns the height of one wheel row.
    pub fn row_height(&self) -> u32 {
        self.row_height
    }

    /// Sets the height of one wheel row, clamped to at least one pixel.
    pub fn set_row_height(&mut self, row_height: u32) {
        let row_height = row_height.max(1);
        if self.row_height == row_height {
            return;
        }
        self.row_height = row_height;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Moves the value by `rows` steps, honouring `wrap`.
    ///
    /// Returns whether the value changed.
    pub fn scroll_rows(&mut self, rows: i64) -> bool {
        let steps = self.available_steps();
        if steps == 0 {
            return false;
        }
        let target = self.value as i128 + rows as i128 * self.step as i128;
        let next = if self.wrap {
            self.wrap_into(target)
        } else {
            target.clamp(self.minimum as i128, self.maximum as i128) as i64
        };
        let next = self.snap(next);
        if next == self.value {
            return false;
        }
        self.value = next;
        self.value_changed.emit(next);
        self.base.request_redraw();
        true
    }

    /// Number of selectable rows in the range.
    pub fn row_count(&self) -> u64 {
        u64::try_from(self.available_steps().saturating_add(1)).unwrap_or(u64::MAX)
    }

    /// Returns the index of the current value within the grid, from the minimum.
    pub fn selected_row(&self) -> u64 {
        let offset = self.value as i128 - self.minimum as i128;
        u64::try_from(offset / self.step as i128).unwrap_or(u64::MAX)
    }

    /// Number of steps between the bounds, or 0 when the range cannot be stepped.
    fn available_steps(&self) -> i128 {
        if self.maximum <= self.minimum || self.step <= 0 {
            return 0;
        }
        (self.maximum as i128 - self.minimum as i128) / self.step as i128
    }

    /// Parses the typed buffer into a value on the control's grid.
    ///
    /// With `decimals == 0` the buffer is an integer. With decimals the buffer is a decimal
    /// number scaled by `10^decimals`, and the result is snapped into the range like any other
    /// write. A buffer that is not a number (a lone `-`, for example) commits nothing.
    fn parse_entry(&self) -> Option<i64> {
        if self.decimals == 0 {
            return self.entry_buffer.trim().parse::<i64>().ok().map(|value| self.snap(value));
        }
        let parsed = self.entry_buffer.trim().parse::<f64>().ok()?;
        if !parsed.is_finite() {
            return None;
        }
        let scale = 10f64.powi(self.decimals as i32);
        let scaled = (parsed * scale).round();
        if !scaled.is_finite() || scaled < i64::MIN as f64 || scaled > i64::MAX as f64 {
            return None;
        }
        Some(self.snap(scaled as i64))
    }

    /// Clamps `value` into the range and rounds it to the nearest grid point.
    ///
    /// Rounding is done from `minimum` so the grid always contains the bounds
    /// themselves, and it halves away from zero so a value exactly between two
    /// rows lands consistently rather than following float rounding.
    fn snap(&self, value: i64) -> i64 {
        let clamped = value.clamp(self.minimum, self.maximum);
        if self.step <= 1 {
            return clamped;
        }
        let offset = clamped as i128 - self.minimum as i128;
        let step = self.step as i128;
        let steps = (offset + step / 2) / step;
        let snapped = self.minimum as i128 + steps * step;
        snapped.clamp(self.minimum as i128, self.maximum as i128) as i64
    }

    /// Maps `value` into the range by wrapping rather than clamping.
    fn wrap_into(&self, value: i128) -> i64 {
        let span = self.maximum as i128 - self.minimum as i128 + 1;
        if span <= 1 {
            return self.minimum;
        }
        let offset = (value - self.minimum as i128).rem_euclid(span);
        (self.minimum as i128 + offset) as i64
    }

    /// The row-height grid position a y coordinate falls on, as a signed row
    /// offset from the centre.
    fn row_offset_at(&self, pos: Point) -> i64 {
        let rect = self.geometry();
        let centre = rect.y + (rect.height as i32) / 2;
        let delta = pos.y - centre;
        // A smaller y (drag upward) must advance the value, hence the negation:
        // moving the reel up reveals higher numbers, matching every platform's
        // picker.
        -(delta as i64) / self.row_height.max(1) as i64
    }

    /// Rows visible on each side of the centre row.
    fn visible_rows(&self) -> i64 {
        let rows = (self.geometry().height / self.row_height.max(1)) as i64;
        (rows / 2).max(1)
    }
}

impl Widget for NumberPicker {
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
        Size::new(80, self.row_height * 3)
    }

    /// A settling wheel owes frames until it comes to rest.
    fn is_animating(&self) -> bool {
        self.settling
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
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "value_changed" => {
                Some(EventSignalRef::mapped("value_changed", &self.value_changed, |v| {
                    CapabilityValue::Int(*v)
                }))
            }
            _ => None,
        }
    }
}

/// `NumberPicker`'s property contract.
///
/// `value` writes go through `set_value`, which snaps and clamps, so a caller
/// cannot put the control into a state its own drawing code does not expect.
/// `row_count` and `selected_row` are derived from the range.
impl WidgetProperties for NumberPicker {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "value" => Ok(CapabilityValue::Int(self.value())),
            "minimum" => Ok(CapabilityValue::Int(self.minimum())),
            "maximum" => Ok(CapabilityValue::Int(self.maximum())),
            "step" => Ok(CapabilityValue::Int(self.step())),
            "wrap" => Ok(CapabilityValue::Bool(self.wrap())),
            "suffix" => Ok(CapabilityValue::String(self.suffix().to_string())),
            // I-105: the same names `spin_box` publishes.
            "prefix" => Ok(CapabilityValue::String(self.prefix().to_string())),
            "decimals" => Ok(CapabilityValue::UInt(self.decimals() as u64)),
            "special_value_text" => match self.special_value_text() {
                Some(text) => Ok(CapabilityValue::String(text.to_string())),
                None => Ok(CapabilityValue::Null),
            },
            "row_count" => Ok(CapabilityValue::UInt(self.row_count())),
            "selected_row" => Ok(CapabilityValue::UInt(self.selected_row())),
            "row_height" => Ok(CapabilityValue::UInt(self.row_height() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "value" => {
                self.set_value(expect_i64(value)?);
                Ok(())
            }
            "minimum" => {
                self.set_minimum(expect_i64(value)?);
                Ok(())
            }
            "maximum" => {
                self.set_maximum(expect_i64(value)?);
                Ok(())
            }
            "step" => {
                self.set_step(expect_i64(value)?);
                Ok(())
            }
            "wrap" => {
                self.set_wrap(expect_bool(value)?);
                Ok(())
            }
            "suffix" => {
                self.set_suffix(expect_string(value)?);
                Ok(())
            }
            "prefix" => {
                self.set_prefix(expect_string(value)?);
                Ok(())
            }
            "decimals" => {
                let decimals = match value {
                    CapabilityValue::UInt(decimals) => decimals,
                    CapabilityValue::Int(decimals) if decimals >= 0 => decimals as u64,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                let decimals =
                    u8::try_from(decimals).map_err(|_| CapabilityAccessError::OutOfRange)?;
                self.set_decimals(decimals);
                Ok(())
            }
            "special_value_text" => {
                let text = match value {
                    CapabilityValue::Null => None,
                    other => Some(expect_string(other)?),
                };
                self.set_special_value(self.special_value, text.filter(|t| !t.is_empty()));
                Ok(())
            }
            "row_height" => match value {
                CapabilityValue::UInt(height) => {
                    let height =
                        u32::try_from(height).map_err(|_| CapabilityAccessError::TypeMismatch)?;
                    self.set_row_height(height);
                    Ok(())
                }
                _ => Err(CapabilityAccessError::TypeMismatch),
            },
            // Derived from the range.
            "row_count" | "selected_row" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "value",
            "minimum",
            "maximum",
            "step",
            "wrap",
            "suffix",
            "prefix",
            "decimals",
            "special_value_text",
            "row_count",
            "selected_row",
            "row_height",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `number_picker` publishes.
    ///
    /// `scroll_rows` names a row count and a direction, so a bare command has no
    /// distance to scroll. The natural zero-argument meaning of "scroll" on a picker is
    /// one row forward, which is what `1` asks for here and is the same unit the control
    /// exposes through `step`; `set_value`, `set_range` and `set_step` assign state and
    /// are answered through the property route.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "scroll_rows" => {
                self.scroll_rows(1);
                Ok(())
            }
            "set_value" | "set_range" | "set_step" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for NumberPicker {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MousePress { pos, button: 1, .. } if self.contains(*pos) => {
                // A drag is measured from where it started, so the state is
                // recorded here and never accumulated during the move.
                self.drag_origin_y = Some(pos.y);
                self.drag_origin_value = self.value;
            }
            Event::MouseMove { pos } => {
                if let Some(origin_y) = self.drag_origin_y {
                    let anchor = Point::new(pos.x, origin_y);
                    let rows = self.row_offset_at(*pos) - self.row_offset_at(anchor);
                    // The raw pointer travel in rows, kept as a fraction so the reel can follow
                    // the finger between two rows and settle onto the nearest one on release.
                    let raw = -(pos.y - origin_y) as f32 / self.row_height.max(1) as f32;
                    self.settle_offset = raw - raw.round();
                    self.settling = false;
                    // Computed from the drag origin every time, never accumulated,
                    // so returning the pointer to where the drag began restores the
                    // original value exactly. Guarding this behind `rows != 0` was
                    // wrong: the zero-row case is precisely the one that has to
                    // restore the value, and skipping it left the last intermediate
                    // value in place.
                    let target = self.drag_origin_value as i128 + rows as i128 * self.step as i128;
                    let next = if self.wrap {
                        self.wrap_into(target)
                    } else {
                        target.clamp(self.minimum as i128, self.maximum as i128) as i64
                    };
                    let next = self.snap(next);
                    if next != self.value {
                        self.value = next;
                        self.value_changed.emit(next);
                        self.base.request_redraw();
                    }
                }
            }
            Event::MouseRelease { .. } => {
                self.drag_origin_y = None;
                if self.settle_offset.abs() > 0.01 {
                    self.settling = true;
                    self.base.request_redraw();
                } else {
                    self.settle_offset = 0.0;
                }
            }
            // A press whose release lands outside the widget never reaches the arm
            // above: the runtime's hit-test returns `None` for a point outside every
            // control, so no `MouseRelease` is delivered and no pointer capture is
            // taken. The origin stayed set, and every subsequent *hover* then drove the
            // value — the picker scrolled with no button held.
            Event::MouseLeave { .. } if self.drag_origin_y.is_some() => {
                self.drag_origin_y = None;
                self.settling = self.settle_offset.abs() > 0.01;
            }
            // A click on the upper half steps down, the lower half steps up —
            // matching the direction a drag would take.
            Event::MouseDoubleClick { pos, button: 1 } => {
                let centre = self.geometry().y + (self.geometry().height as i32) / 2;
                let rows = if pos.y < centre { -1 } else { 1 };
                let _ = self.scroll_rows_visual(rows);
            }
            Event::KeyPress { key, modifiers: _ } => match *key {
                // Typed value entry: digits, a sign and a decimal point build a buffer, Enter
                // commits it and Escape abandons it. Before this arm the picker was
                // scroll-only, so a large jump took dozens of notches.
                48..=57 | 45 | 46 if self.entering || matches!(*key, 48..=57 | 45) => {
                    if let Some(ch) = char::from_u32(*key) {
                        if !self.entering {
                            self.entering = true;
                            self.entry_buffer.clear();
                        }
                        self.entry_buffer.push(ch);
                        self.base.request_redraw();
                    }
                }
                8 if self.entering => {
                    self.entry_buffer.pop();
                    if self.entry_buffer.is_empty() {
                        self.entering = false;
                    }
                    self.base.request_redraw();
                }
                13 | 10 if self.entering => {
                    if let Some(value) = self.parse_entry() {
                        self.set_value(value);
                    }
                    self.entering = false;
                    self.entry_buffer.clear();
                    self.base.request_redraw();
                }
                27 if self.entering => {
                    self.entering = false;
                    self.entry_buffer.clear();
                    self.base.request_redraw();
                }
                // Up / Right.
                38 | 39 => {
                    let _ = self.scroll_rows(1);
                }
                // Down / Left.
                40 | 37 => {
                    let _ = self.scroll_rows(-1);
                }
                // Page Up / Page Down move by a screenful.
                33 => {
                    let rows = self.visible_rows();
                    let _ = self.scroll_rows(rows);
                }
                34 => {
                    let rows = self.visible_rows();
                    let _ = self.scroll_rows(-rows);
                }
                // Home / End jump to the bounds.
                36 => self.set_value(self.minimum),
                35 => self.set_value(self.maximum),
                _ => {}
            },
            Event::Wheel { delta, modifiers } => {
                // A wheel notch is a discrete step, so it starts from rest. Leaving the
                // drag settle remainder in place made the wheel and the drag paths end
                // in two different states (I-110); the same clear path is used here.
                self.clear_inertia();
                // One notch is one step. A positive event delta means the content
                // moves down, revealing lower numbers, so the sign is inverted:
                // scrolling up increments and scrolling down decrements.
                let scroll = WheelScroll::from_event(*delta, *modifiers, 1);
                let _ = self.scroll_rows(-(scroll.lines_y as i64));
            }
            _ => {}
        }
    }
}

impl NumberPicker {
    /// Steps by `rows` in **screen** direction: a positive `rows` moves the
    /// highlighted number downward, which is what a click below the centre or a
    /// downward drag means.
    fn scroll_rows_visual(&mut self, rows: i64) -> bool {
        self.scroll_rows(-rows)
    }
}

impl Draw for NumberPicker {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.base.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }

        let style = self.base.style();
        let background = style.background_color.unwrap_or(Color::WHITE);
        let text_color = style.text_color.unwrap_or(Color::BLACK);
        // The selected row is drawn in the theme's accent so the current value is
        // identifiable without a separate label.
        let selected_color =
            style.border_color.unwrap_or_else(|| background.blend(&text_color, 0.12));
        // The band replaces the surface under the centre row, so the centre label's ink has
        // to be chosen against the *band*, not against the picker's surface. Using the
        // surface ink put `rgb(225,225,225)` on the `rgb(130,130,130)` band — 2.94:1, below
        // even the 3:1 large-text floor, on the one row the control exists to show.
        let selected_text_color = selected_color.contrast_color();

        context.face_with_gradient_corners(
            rect,
            background,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().resolved_corner_radii().to_array(),
            Color::BLACK,
        );

        let row_h = self.row_height.max(1);
        let centre = rect.y + (rect.height as i32) / 2;
        // The **effective font**: the resolved theme/caller font when the style names one, else the
        // row-height-derived face this control has always used. Either way the hint, the row height
        // and the ink agree (D09-STYLE-01).
        let fallback_font = Font::simple("Sans", (row_h as f32 * 0.55).max(8.0));
        let font = style.font.as_ref().unwrap_or(&fallback_font);

        // Selection band across the centre row.
        let band = Rect::new(rect.x, centre - (row_h as i32) / 2, rect.width, row_h);
        context.fill_rect(band, selected_color);

        // Neighbouring values, fading away from the centre. Drawing them is what
        // makes the control read as a wheel rather than a text field.
        let visible = self.visible_rows();
        // The glyph box, not the row's centre line, is what has to fit. The guard used to
        // compare `row_center` against the frame, which admits a row whose centre is just
        // inside while its 15 px label reaches half a line above (or below) it: the top
        // neighbour of the centre row was drawn at `y = -3` and the bottom one past the
        // frame's last pixel. Testing the box the text actually occupies is what the
        // assertion measures, so it is what the loop must test too.
        let text_h = context.measure_text("M", font).height as i32;
        let frame_bottom = rect.y + rect.height as i32;
        // A settling drag shifts the whole reel by the sub-row remainder, which is what
        // turns the release from a one-frame snap into a short decelerating finish.
        let settle = (self.settle_offset * row_h as f32).round() as i32;
        let mut label = String::new();
        for offset in -visible..=visible {
            let row_center = centre + (offset as i32) * row_h as i32 + settle;
            let text_top = row_center - text_h / 2;
            if text_top < rect.y || text_top + text_h > frame_bottom {
                continue;
            }
            let value = self.value_at_offset(offset);
            let Some(value) = value else {
                continue;
            };
            let faded = offset != 0;
            let color = if faded {
                // The neighbours are *deliberately* dimmer than the selection, but "dimmer"
                // is a relation to the surface, not a blend toward a fixed colour: the old
                // 45% blend toward the ink produced `rgb(111,111,111)` on the dark surface,
                // which is 3.73:1 — a neighbour that is hard to read while *also* failing to
                // look secondary. Dimming the resolved ink by alpha keeps it below the
                // selection's contrast while staying above the legibility floor.
                text_color.legible_on(background, 4.5).with_alpha(191)
            } else {
                selected_text_color
            };
            // One buffer reused across the rows: `to_string`/`format!` per row per frame
            // was a fresh allocation for a label a `write!` renders in place.
            label.clear();
            if !faded && self.entering {
                // While a value is typed, the centre row shows the entry buffer verbatim so
                // the user can see the digits they are building, including a bare sign.
                label.push_str(&self.entry_buffer);
            } else if !faded {
                // I-P68: render into the reused buffer instead of allocating a label.
                self.write_display_text(&mut label, value);
            } else {
                let _ = write!(label, "{value}");
            }
            // The row's centre is where the glyph *box* centres, not where its top edge goes:
            // the renderer paints downward from the origin, so using `row_center` directly put
            // a 14 px value in a 15 px row spanning `centre .. centre + 14` — half a row below
            // the band it belongs to.
            context.draw_text_fitted_cow(
                Rect::new(rect.x, text_top, rect.width, text_h.max(1) as u32),
                label.as_str(),
                font,
                color,
                HorizontalAlignment::Center,
            );
        }

        crate::widget::draw::stroke_surface_border(
            context,
            rect,
            style.resolved_corner_radii().to_array(),
            style.border_color.unwrap_or(Color::rgb(200, 200, 200)),
            1,
        );
    }

    fn uses_custom_drawing(&self) -> bool {
        true
    }
}

impl NumberPicker {
    /// The value `offset` rows from the selection, or `None` when the wheel end
    /// has been passed and nothing should be drawn there.
    fn value_at_offset(&self, offset: i64) -> Option<i64> {
        // Offsets count in screen rows; a row above the centre is a higher value.
        let candidate = self.value as i128 - offset as i128 * self.step as i128;
        if self.wrap {
            return Some(self.wrap_into(candidate));
        }
        if candidate < self.minimum as i128 || candidate > self.maximum as i128 {
            return None;
        }
        Some(candidate as i64)
    }
}

/// Returns whether `pos` lies inside `rect`.
///
/// I-P68: the picker reuses [`Rect::contains_point`] rather than carrying a second
/// spelling of the same four comparisons.
trait ContainsPoint {
    fn contains(&self, pos: Point) -> bool;
}

impl ContainsPoint for NumberPicker {
    fn contains(&self, pos: Point) -> bool {
        self.geometry().contains_point(pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picker() -> NumberPicker {
        NumberPicker::new(Rect::new(0, 0, 80, 84))
    }

    #[test]
    fn starts_at_the_minimum_with_the_default_range() {
        let picker = picker();
        assert_eq!(picker.value(), 0);
        assert_eq!(picker.minimum(), 0);
        assert_eq!(picker.maximum(), 100);
        assert_eq!(picker.row_count(), 101);
    }

    /// A value outside the range must be clamped, not stored.
    #[test]
    fn set_value_clamps_to_the_range() {
        let mut picker = picker();
        picker.set_range(10, 20);

        picker.set_value(999);
        assert_eq!(picker.value(), 20);
        picker.set_value(-5);
        assert_eq!(picker.value(), 10);
    }

    /// N-B-02: a full-domain range must not overflow the range arithmetic.
    ///
    /// `set_range(i64::MIN, i64::MAX)` is a legal public call, and it immediately snaps the current
    /// value. The old `maximum - minimum`, `clamped - minimum` and `span + 1` were bare arithmetic on
    /// `i64`, which aborted in a debug build and wrapped to a wrong range in release. The assertions
    /// here are that the calls complete and that every accessor stays consistent.
    #[test]
    fn extreme_range_does_not_overflow() {
        let mut picker = picker();
        // Debug builds panic on the bare subtractions this used to do, so reaching the asserts is
        // itself the check.
        picker.set_range(i64::MIN, i64::MAX);
        assert_eq!(picker.minimum(), i64::MIN);
        assert_eq!(picker.maximum(), i64::MAX);
        assert_eq!(picker.row_count(), u64::MAX);
        // The value is a real grid point, not a wrapped constant: with a step of 1 the snapped value
        // must equal whatever `set_range` left it at (it starts at 0).
        assert_eq!(picker.value(), 0);

        // Every derived accessor must terminate and stay in range.
        let rows = picker.row_count();
        let _ = picker.selected_row();
        assert!(rows >= 1);

        // Wrapping covers the entire inclusive range, including 2^64 possible values.
        picker.set_wrap(true);
        assert!(picker.scroll_rows(1));
        assert_eq!(picker.value(), 1);
        assert!(picker.scroll_rows(-1));
        assert_eq!(picker.value(), 0);
        picker.set_value(i64::MIN);
        assert!(picker.scroll_rows(-1));
        assert_eq!(picker.value(), i64::MAX);

        // Snapping an on-grid value remains stable even when the offset exceeds i64.
        picker.set_step(5);
        picker.set_value(i64::MIN + 10);
        assert_eq!(picker.value(), i64::MIN + 10);
        assert_eq!(picker.selected_row(), 2);
    }

    /// With a step of 5, only multiples of the step (offset from the minimum)
    /// are selectable.
    #[test]
    fn set_value_snaps_to_the_step_grid() {
        let mut picker = picker();
        picker.set_range(0, 100);
        picker.set_step(5);

        picker.set_value(7);
        assert_eq!(picker.value(), 5, "7 rounds down to the nearer grid point");
        picker.set_value(8);
        assert_eq!(picker.value(), 10, "8 rounds up to the nearer grid point");
        assert_eq!(picker.selected_row(), 2);
    }

    /// The grid must contain the bounds themselves, otherwise the range appears
    /// to be unreachable at one end.
    #[test]
    fn the_step_grid_contains_both_bounds() {
        let mut picker = picker();
        picker.set_range(1, 9);
        picker.set_step(4);

        picker.set_value(1);
        assert_eq!(picker.value(), 1);
        picker.set_value(9);
        assert_eq!(picker.value(), 9, "the maximum must be selectable");
    }

    #[test]
    fn scroll_rows_stops_at_the_bounds_without_wrap() {
        let mut picker = picker();
        picker.set_range(0, 3);

        assert!(picker.scroll_rows(1));
        assert_eq!(picker.value(), 1);
        // A large step is clamped to the end and *does* change the value, so it
        // reports true.
        assert!(picker.scroll_rows(10));
        assert_eq!(picker.value(), 3);
        // Already at the end, another step has nowhere to go, so it must report
        // that nothing changed rather than fire a redundant signal.
        assert!(!picker.scroll_rows(10), "a scroll at the bound reports no change");
        assert_eq!(picker.value(), 3);
    }

    #[test]
    fn a_wheel_notch_clears_the_drag_settle_remainder() {
        let mut picker = picker();
        picker.set_range(0, 10);
        picker.settle_offset = 0.4;
        picker.settling = true;
        picker.handle_event(&Event::Wheel { delta: Point::new(0, -1), modifiers: 0 });
        assert_eq!(picker.settle_offset, 0.0, "the wheel path must start from rest");
        assert!(!picker.is_settling());
        assert_eq!(picker.value(), 1);
    }

    #[test]
    fn scroll_rows_wraps_when_enabled() {
        let mut picker = picker();
        picker.set_range(0, 3);
        picker.set_wrap(true);

        picker.set_value(3);
        assert!(picker.scroll_rows(1));
        assert_eq!(picker.value(), 0, "stepping past the end wraps to the start");

        assert!(picker.scroll_rows(-1));
        assert_eq!(picker.value(), 3, "stepping below the start wraps to the end");
    }

    /// An inverted range must be normalised rather than producing a control whose
    /// bounds cannot be satisfied.
    #[test]
    fn an_inverted_range_is_normalised() {
        let mut picker = picker();
        picker.set_range(50, 10);
        assert_eq!(picker.minimum(), 10);
        assert_eq!(picker.maximum(), 50);
    }

    /// A zero step would make the value grid degenerate, so it is raised to 1.
    #[test]
    fn a_zero_step_is_treated_as_one() {
        let mut picker = picker();
        picker.set_step(0);
        assert_eq!(picker.step(), 1);
    }

    /// Keyboard stepping must move the value, which is what makes the control
    /// usable without a pointer.
    #[test]
    fn arrow_keys_step_the_value() {
        let mut picker = picker();
        picker.set_range(0, 10);

        picker.handle_event(&Event::key_press(38, 0));
        assert_eq!(picker.value(), 1, "Up increases");
        picker.handle_event(&Event::key_press(40, 0));
        assert_eq!(picker.value(), 0, "Down decreases");

        picker.handle_event(&Event::key_press(35, 0));
        assert_eq!(picker.value(), 10, "End jumps to the maximum");
        picker.handle_event(&Event::key_press(36, 0));
        assert_eq!(picker.value(), 0, "Home jumps to the minimum");
    }

    /// A drag measured from its origin must land on the row the pointer is over,
    /// and must not accumulate rounding drift from intermediate moves.
    #[test]
    fn a_drag_selects_the_row_under_the_pointer() {
        let mut picker = picker();
        picker.set_range(0, 100);
        picker.set_row_height(28);
        // 84px tall, so the centre row is y = 42.
        picker.set_value(50);

        picker.handle_event(&Event::mouse_press(10, 42, 1));
        // Three rows up (y = 42 - 84) must select three higher values.
        picker.handle_event(&Event::mouse_move(10, 42 - 28 * 3));
        assert_eq!(picker.value(), 53, "an upward drag reveals higher numbers");

        // Returning to the origin must return to the original value exactly.
        picker.handle_event(&Event::mouse_move(10, 42));
        assert_eq!(picker.value(), 50, "no drift after a round trip");

        picker.handle_event(&Event::mouse_release(10, 42, 1));
        // After release, a move must not keep changing the value.
        picker.handle_event(&Event::mouse_move(10, 0));
        assert_eq!(picker.value(), 50, "a move outside a drag is ignored");
    }

    /// The property contract must round-trip the writable names.
    #[test]
    fn properties_round_trip() {
        use crate::widget::capability::properties_trait::{
            widget_property_get, widget_property_set,
        };

        let mut picker = picker();
        widget_property_set(&mut picker, "value", CapabilityValue::Int(42)).expect("value");
        assert_eq!(widget_property_get(&picker, "value"), Ok(CapabilityValue::Int(42)));

        widget_property_set(&mut picker, "wrap", CapabilityValue::Bool(true)).expect("wrap");
        assert_eq!(widget_property_get(&picker, "wrap"), Ok(CapabilityValue::Bool(true)));

        widget_property_set(&mut picker, "suffix", CapabilityValue::String(" kg".to_string()))
            .expect("suffix");
        assert_eq!(
            widget_property_get(&picker, "suffix"),
            Ok(CapabilityValue::String(" kg".to_string()))
        );

        // Derived names must be refused rather than silently accepted.
        assert_eq!(
            widget_property_set(&mut picker, "row_count", CapabilityValue::UInt(1)),
            Err(CapabilityAccessError::ReadOnlyProperty)
        );
    }

    /// Changing the value must announce it exactly once, so a listener cannot
    /// observe a value it never had.
    #[test]
    fn a_drag_that_leaves_the_control_stops_driving_the_value() {
        // A press whose release lands outside the widget never delivers `MouseRelease`:
        // the runtime's hit-test answers `None` for a point outside every control. With
        // no `MouseLeave` arm the drag origin stayed set, so every later *hover* moved
        // the value with no button held — the picker scrolled by itself.
        let mut picker = NumberPicker::new(Rect::new(0, 0, 60, 200));
        picker.set_range(0, 100);
        picker.set_value(50);
        let before = picker.value();

        picker.handle_event(&Event::MousePress {
            pos: Point::new(30, 100),
            button: 1,
            modifiers: 0,
        });
        // The pointer leaves upward without a release being delivered.
        picker.handle_event(&Event::MouseLeave { pos: Point::new(30, 5) });

        // Hover moves now must not change anything: no button is held.
        for y in [80, 60, 40, 20] {
            picker.handle_event(&Event::MouseMove { pos: Point::new(30, y) });
        }
        assert_eq!(
            picker.value(),
            before,
            "hovering after the pointer left the control must not move the value"
        );
    }

    #[test]
    fn value_changed_fires_only_on_a_real_change() {
        use std::sync::{Arc, Mutex};

        let mut picker = picker();
        picker.set_range(0, 10);
        let seen = Arc::new(Mutex::new(Vec::<i64>::new()));
        let sink = Arc::clone(&seen);
        picker.value_changed.connect(move |value| {
            sink.lock().expect("signal sink poisoned").push(*value);
        });

        picker.set_value(3);
        picker.set_value(3);
        picker.set_value(4);

        let recorded = seen.lock().expect("signal sink poisoned").clone();
        assert_eq!(recorded, vec![3, 4], "a no-op write must not emit");
    }

    /// A picker with no room to move must not claim it changed.
    #[test]
    fn a_single_value_range_cannot_scroll() {
        let mut picker = picker();
        picker.set_range(7, 7);
        assert_eq!(picker.row_count(), 1);
        assert!(!picker.scroll_rows(1));
        assert_eq!(picker.value(), 7);
    }

    /// The wheel must draw neighbours, not just the current value; that is the
    /// difference between a picker and a label.
    #[test]
    fn the_value_at_a_row_offset_follows_the_wheel_direction() {
        let mut picker = picker();
        picker.set_range(0, 100);
        picker.set_value(50);

        assert_eq!(picker.value_at_offset(0), Some(50));
        assert_eq!(picker.value_at_offset(1), Some(49), "one row up is one lower");
        assert_eq!(picker.value_at_offset(-1), Some(51), "one row down is one higher");
    }

    /// Past the end, a non-wrapping wheel must draw nothing rather than clamp,
    /// otherwise the same number would appear on several rows.
    #[test]
    fn rows_past_the_end_are_empty_without_wrap() {
        let mut picker = picker();
        picker.set_range(0, 2);
        picker.set_value(0);

        assert_eq!(picker.value_at_offset(-1), Some(1));
        assert_eq!(picker.value_at_offset(-3), None);
    }

    /// Prefix, decimals and a special-value label are display properties.
    #[test]
    fn prefix_decimals_and_special_text_shape_the_label() {
        let mut picker = picker();
        picker.set_range(0, 10_000);
        picker.set_prefix("$");
        picker.set_suffix(" kg");
        picker.set_decimals(2);
        picker.set_value(1234);
        assert_eq!(picker.display_text(1234), "$12.34 kg");
        assert_eq!(picker.display_text(-1234), "$-12.34 kg");

        picker.set_special_value(Some(1234), Some("auto".to_string()));
        assert_eq!(picker.display_text(1234), "auto");
        assert_eq!(picker.special_value(), Some(1234));
    }

    /// Typed digits build a value; Enter commits and Escape abandons it.
    #[test]
    fn a_typed_value_is_parsed_on_commit() {
        let mut picker = picker();
        picker.set_range(0, 1_000);
        for key in [49u32, 50, 51] {
            picker.handle_event(&Event::KeyPress { key, modifiers: 0 });
        }
        assert_eq!(picker.value(), 0, "the value changes only on commit");
        picker.handle_event(&Event::KeyPress { key: 13, modifiers: 0 });
        assert_eq!(picker.value(), 123);

        picker.handle_event(&Event::KeyPress { key: 57, modifiers: 0 });
        picker.handle_event(&Event::KeyPress { key: 27, modifiers: 0 });
        assert_eq!(picker.value(), 123, "Escape abandons the entry");
    }

    /// A released drag settles instead of jumping on the next frame.
    #[test]
    fn a_released_drag_settles_with_a_decaying_offset() {
        let mut picker = picker();
        let rect = picker.geometry();
        let start = Point::new(rect.x + 4, rect.y + rect.height as i32 / 2);
        picker.handle_event(&Event::MousePress { pos: start, button: 1, modifiers: 0 });
        picker.handle_event(&Event::MouseMove {
            pos: Point::new(start.x, start.y - picker.row_height() as i32 - 6),
        });
        picker.handle_event(&Event::MouseRelease { pos: start, button: 1 });
        assert!(picker.is_settling());
        assert!(picker.tick(16));
        let mut frames = 0;
        while picker.tick(16) && frames < 200 {
            frames += 1;
        }
        assert!(!picker.is_settling(), "the settle must terminate");
    }
}
