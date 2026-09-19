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

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
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

    /// Sets the suffix shown after the number. Display only: it is not part of the
    /// value, so a caller parsing the control's text never sees it.
    pub fn set_suffix(&mut self, suffix: impl Into<String>) {
        self.suffix = suffix.into();
        self.base.request_redraw();
    }

    /// Returns the height of one wheel row.
    pub fn row_height(&self) -> u32 {
        self.row_height
    }

    /// Sets the height of one wheel row, clamped to at least one pixel.
    pub fn set_row_height(&mut self, row_height: u32) {
        self.row_height = row_height.max(1);
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
        let delta = rows.saturating_mul(self.step);
        let next = if self.wrap {
            self.wrap_into(self.value.saturating_add(delta))
        } else {
            self.value.saturating_add(delta).clamp(self.minimum, self.maximum)
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
        self.available_steps().saturating_add(1).try_into().unwrap_or(u64::MAX)
    }

    /// Returns the index of the current value within the grid, from the minimum.
    pub fn selected_row(&self) -> u64 {
        let offset = self.value.saturating_sub(self.minimum);
        let index = offset / self.step;
        index.try_into().unwrap_or(u64::MAX)
    }

    /// Number of steps between the bounds, or 0 when the range cannot be stepped.
    fn available_steps(&self) -> i64 {
        if self.maximum <= self.minimum || self.step <= 0 {
            return 0;
        }
        (self.maximum - self.minimum) / self.step
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
        let offset = clamped - self.minimum;
        let steps = (offset + self.step / 2) / self.step;
        let snapped = self.minimum.saturating_add(steps.saturating_mul(self.step));
        snapped.clamp(self.minimum, self.maximum)
    }

    /// Maps `value` into the range by wrapping rather than clamping.
    fn wrap_into(&self, value: i64) -> i64 {
        let span = self.maximum - self.minimum;
        if span <= 0 {
            return self.minimum;
        }
        let offset = (value - self.minimum).rem_euclid(span + 1);
        self.minimum.saturating_add(offset)
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

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        Size::new(80, self.row_height * 3)
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();
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
            Event::MousePress { pos, button: 1 } if self.contains(*pos) => {
                // A drag is measured from where it started, so the state is
                // recorded here and never accumulated during the move.
                self.drag_origin_y = Some(pos.y);
                self.drag_origin_value = self.value;
            }
            Event::MouseMove { pos } => {
                if let Some(origin_y) = self.drag_origin_y {
                    let anchor = Point::new(pos.x, origin_y);
                    let rows = self.row_offset_at(*pos) - self.row_offset_at(anchor);
                    // Computed from the drag origin every time, never accumulated,
                    // so returning the pointer to where the drag began restores the
                    // original value exactly. Guarding this behind `rows != 0` was
                    // wrong: the zero-row case is precisely the one that has to
                    // restore the value, and skipping it left the last intermediate
                    // value in place.
                    let target =
                        self.drag_origin_value.saturating_add(rows.saturating_mul(self.step));
                    let next = if self.wrap {
                        self.wrap_into(target)
                    } else {
                        target.clamp(self.minimum, self.maximum)
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
            }
            // A press whose release lands outside the widget never reaches the arm
            // above: the runtime's hit-test returns `None` for a point outside every
            // control, so no `MouseRelease` is delivered and no pointer capture is
            // taken. The origin stayed set, and every subsequent *hover* then drove the
            // value — the picker scrolled with no button held.
            Event::MouseLeave { .. } if self.drag_origin_y.is_some() => {
                self.drag_origin_y = None;
            }
            // A click on the upper half steps down, the lower half steps up —
            // matching the direction a drag would take.
            Event::MouseDoubleClick { pos, button: 1 } => {
                let centre = self.geometry().y + (self.geometry().height as i32) / 2;
                let rows = if pos.y < centre { -1 } else { 1 };
                let _ = self.scroll_rows_visual(rows);
            }
            Event::KeyPress { key, modifiers: _ } => match *key {
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
            Event::Wheel { delta, .. } => {
                // A wheel notch is one row. The event's sign already means "the
                // content moves down", and moving the content down reveals lower
                // numbers, so the delta is negated to match the visual direction.
                let _ = self.scroll_rows(-(delta.y as i64));
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

        let style = self.base.style().clone();
        let background = style.background_color.unwrap_or(Color::WHITE);
        let text_color = style.text_color.unwrap_or(Color::BLACK);
        // The selected row is drawn in the theme's accent so the current value is
        // identifiable without a separate label.
        let selected_color =
            style.border_color.unwrap_or_else(|| background.blend(&text_color, 0.12));

        context.fill_rect(rect, background);

        let row_h = self.row_height.max(1);
        let centre = rect.y + (rect.height as i32) / 2;
        let font = Font::simple("Sans", (row_h as f32 * 0.55).max(8.0));

        // Selection band across the centre row.
        let band = Rect::new(rect.x, centre - (row_h as i32) / 2, rect.width, row_h);
        context.fill_rect(band, selected_color);

        // Neighbouring values, fading away from the centre. Drawing them is what
        // makes the control read as a wheel rather than a text field.
        let visible = self.visible_rows();
        for offset in -visible..=visible {
            let row_center = centre + (offset as i32) * row_h as i32;
            if row_center < rect.y || row_center > rect.y + rect.height as i32 {
                continue;
            }
            let value = self.value_at_offset(offset);
            let Some(value) = value else {
                continue;
            };
            let faded = offset != 0;
            let color = if faded {
                // Dim the neighbours so the selection reads at a glance.
                background.blend(&text_color, 0.45)
            } else {
                text_color
            };
            let label = if faded { value.to_string() } else { format!("{value}{}", self.suffix) };
            context.draw_text(
                Point::new(rect.x + (rect.width as i32) / 2, row_center),
                &label,
                &font,
                color,
                HorizontalAlignment::Center,
            );
        }

        context.draw_rect(rect, style.border_color.unwrap_or(Color::rgb(200, 200, 200)));
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
        let delta = -offset.saturating_mul(self.step);
        let candidate = self.value.saturating_add(delta);
        if self.wrap {
            return Some(self.wrap_into(candidate));
        }
        if candidate < self.minimum || candidate > self.maximum {
            return None;
        }
        Some(candidate)
    }
}

/// Returns whether `pos` lies inside `rect`.
trait ContainsPoint {
    fn contains(&self, pos: Point) -> bool;
}

impl ContainsPoint for NumberPicker {
    fn contains(&self, pos: Point) -> bool {
        let rect = self.geometry();
        pos.x >= rect.x
            && pos.x < rect.x + rect.width as i32
            && pos.y >= rect.y
            && pos.y < rect.y + rect.height as i32
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

        picker.handle_event(&Event::MousePress { pos: Point::new(30, 100), button: 1 });
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
}
