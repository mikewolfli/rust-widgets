// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Slider widget.
use crate::compat::ToString;
use crate::core::{Color, Orientation, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::capability::coercion::{
    expect_bool, expect_i64, expect_orientation, orientation_to_str,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::numeric::ordered_clamp_i32;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
/// Slider widget.
pub struct Slider {
    base: BaseWidget,
    minimum: i32,
    maximum: i32,
    value: i32,
    single_step: i32,
    page_step: i32,
    orientation: Orientation,
    tick_position: TickPosition,
    tick_interval: i32,
    tracking: bool,
    slider_position: i32,
    mouse_pressed: bool,
    /// The writing direction the value axis runs in.
    ///
    /// Defaults to left-to-right, so a slider that never asks for a direction behaves exactly as it
    /// did before the field existed. In a right-to-left interface the minimum belongs at the right
    /// edge, and without this the handle moved the opposite way from the value it reported — the one
    /// functional consequence of missing RTL support in this crate.
    direction: crate::core::TextDirection,
    /// Emitted with the new value on every value change, from user interaction
    /// or from a programmatic setter.
    pub value_changed: Signal1<i32>,
    /// Emitted with the new value when the value changes by dragging the handle,
    /// as opposed to clicking the groove or keyboard stepping. Allows a consumer
    /// to treat continuous manipulation differently (for example, defer expensive
    /// work until release).
    pub slider_moved: Signal1<i32>,
    /// Emitted when the handle is pressed. Carries no payload.
    pub slider_pressed: GenericSignal,
    /// Emitted when the handle is released. Paired with
    /// [`Slider::slider_pressed`], so the widget's own press tracking decides
    /// when it fires rather than the pointer's position.
    pub slider_released: GenericSignal,
}
/// Tick mark position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TickPosition {
    /// No tick marks
    #[default]
    NoTicks,
    /// Tick marks above (for horizontal) or left (for vertical)
    TicksAbove,
    /// Tick marks below (for horizontal) or right (for vertical)
    TicksBelow,
    /// Tick marks on both sides
    TicksBothSides,
}

/// Edge length of the draggable handle, in logical pixels.
///
/// Named once because two places have to agree about it: `draw` centres the handle on the
/// position `value_to_pixel_pos` returns, and that function has to keep half a handle inside
/// each end. While the size was a literal in the draw path only, the travel range assumed a
/// zero-width handle and the minimum value's handle hung off the control.
const SLIDER_SIZE: f32 = 16.0;

/// Formats a [`TickPosition`] as its published token.
///
/// Local rather than imported from `capability::access` (or `coercion`) for the
/// same reason the type itself lives here: `Slider` is available in every profile
/// while those modules' `TickPosition` helpers are gated to `full_widgets`.
pub const fn tick_position_to_str(tick_position: TickPosition) -> &'static str {
    match tick_position {
        TickPosition::NoTicks => "none",
        TickPosition::TicksAbove => "above",
        TickPosition::TicksBelow => "below",
        TickPosition::TicksBothSides => "both",
    }
}

/// Parses the token [`tick_position_to_str`] publishes, plus the spellings config
/// files have historically used.
///
/// The local inverse of [`tick_position_to_str`]; `coercion::expect_tick_position`
/// would do the same job but is `full_widgets`-gated, and a `Slider` must be able
/// to answer its own contract in every profile.
fn expect_tick_position(value: CapabilityValue) -> Result<TickPosition, CapabilityAccessError> {
    let token = match value {
        CapabilityValue::String(text) => crate::widget::capability::coercion::normalize_key(&text),
        _ => return Err(CapabilityAccessError::TypeMismatch),
    };

    match token.as_str() {
        "none" | "noticks" => Ok(TickPosition::NoTicks),
        "above" | "ticksabove" | "left" => Ok(TickPosition::TicksAbove),
        "below" | "ticksbelow" | "right" => Ok(TickPosition::TicksBelow),
        "both" | "ticksbothsides" => Ok(TickPosition::TicksBothSides),
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}
impl Slider {
    /// Creates a slider with default range 0-100.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Slider, geometry, "Slider"),
            minimum: 0,
            maximum: 100,
            value: 0,
            single_step: 1,
            page_step: 10,
            orientation: Orientation::Horizontal,
            tick_position: TickPosition::NoTicks,
            tick_interval: 0,
            tracking: true,
            slider_position: 0,
            mouse_pressed: false,
            direction: crate::core::TextDirection::default(),
            value_changed: Signal1::new(),
            slider_moved: Signal1::new(),
            slider_pressed: GenericSignal::new(),
            slider_released: GenericSignal::new(),
        }
    }
    /// Returns minimum value.
    pub fn minimum(&self) -> i32 {
        self.minimum
    }
    /// Sets minimum value.
    pub fn set_minimum(&mut self, minimum: i32) {
        self.minimum = minimum;
        if self.maximum < self.minimum {
            self.maximum = self.minimum;
        }
        self.set_value(self.value); // Re-clamp
    }
    /// Returns maximum value.
    pub fn maximum(&self) -> i32 {
        self.maximum
    }
    /// Sets maximum value.
    pub fn set_maximum(&mut self, maximum: i32) {
        self.maximum = maximum;
        if self.minimum > self.maximum {
            self.minimum = self.maximum;
        }
        self.set_value(self.value); // Re-clamp
    }
    /// Sets both minimum and maximum in one call.
    /// This is a convenience writer; query bounds via `minimum()` and `maximum()`.
    pub fn set_range(&mut self, minimum: i32, maximum: i32) {
        self.minimum = minimum;
        self.maximum = maximum.max(minimum);
        self.set_value(self.value); // Re-clamp
    }
    /// Returns current value.
    pub fn value(&self) -> i32 {
        self.value
    }
    /// Sets value, clamped to valid range.
    pub fn set_value(&mut self, value: i32) {
        let clamped = ordered_clamp_i32(value, self.minimum, self.maximum);
        if self.value == clamped {
            return;
        }
        self.value = clamped;
        self.slider_position = clamped;
        self.value_changed.emit(self.value);
        self.base.request_redraw();
    }
    /// Returns single step value.
    pub fn single_step(&self) -> i32 {
        self.single_step
    }
    /// Sets single step value.
    pub fn set_single_step(&mut self, step: i32) {
        self.single_step = step.max(1);
        self.base.request_redraw();
    }
    /// Returns page step value.
    pub fn page_step(&self) -> i32 {
        self.page_step
    }
    /// Sets page step value.
    pub fn set_page_step(&mut self, step: i32) {
        self.page_step = step.max(1);
        self.base.request_redraw();
    }
    /// Returns orientation.
    pub fn orientation(&self) -> Orientation {
        self.orientation
    }
    /// Sets orientation.
    pub fn set_orientation(&mut self, orientation: Orientation) {
        self.orientation = orientation;
        self.base.request_redraw();
    }
    /// Returns tick position.
    pub fn tick_position(&self) -> TickPosition {
        self.tick_position
    }
    /// Sets tick position.
    pub fn set_tick_position(&mut self, position: TickPosition) {
        self.tick_position = position;
        self.base.request_redraw();
    }
    /// Returns tick interval.
    pub fn tick_interval(&self) -> i32 {
        self.tick_interval
    }
    /// Sets tick interval.
    pub fn set_tick_interval(&mut self, interval: i32) {
        self.tick_interval = interval.max(0);
        self.base.request_redraw();
    }
    /// Returns whether tracking is enabled.
    pub fn tracking(&self) -> bool {
        self.tracking
    }
    /// Sets tracking state.
    pub fn set_tracking(&mut self, tracking: bool) {
        self.tracking = tracking;
    }
    /// Returns slider position.
    pub fn slider_position(&self) -> i32 {
        self.slider_position
    }
    /// Sets slider position (without emitting signals).
    pub fn set_slider_position(&mut self, position: i32) {
        let new_position = ordered_clamp_i32(position, self.minimum, self.maximum);
        if self.slider_position == new_position {
            return;
        }
        self.slider_position = new_position;
        if self.tracking {
            self.set_value(self.slider_position);
        }
        self.slider_moved.emit(self.slider_position);
    }
    /// Adds single step to value.
    pub fn trigger_action(&mut self, action: SliderAction) {
        match action {
            SliderAction::SliderSingleStepAdd => {
                self.set_value(self.value + self.single_step);
            }
            SliderAction::SliderSingleStepSub => {
                self.set_value(self.value - self.single_step);
            }
            SliderAction::SliderPageStepAdd => {
                self.set_value(self.value + self.page_step);
            }
            SliderAction::SliderPageStepSub => {
                self.set_value(self.value - self.page_step);
            }
            SliderAction::SliderToMinimum => {
                self.set_value(self.minimum);
            }
            SliderAction::SliderToMaximum => {
                self.set_value(self.maximum);
            }
            SliderAction::SliderMove => {
                // Handled by mouse events
            }
        }
    }
    /// Returns value for a given pixel position.
    fn pixel_pos_to_value(&self, pos: f32) -> i32 {
        let rect = self.geometry();
        let range = (self.maximum - self.minimum) as f32;
        // The handle is `SLIDER_SIZE` wide and `value_to_pixel_pos` centres it on the position it
        // returns, so that function's travel stops half a handle short of each end. This one used the
        // **full** width, which made the two calculations non-inverses: the value 0 was drawn at
        // x = inset, and reading that same x back produced a mid-range value. In practice clicking
        // the handle did not select the value the handle was showing — and the error grew with the
        // zoom of the range, so it was not a rounding artefact. Sharing the inset makes the two
        // exact inverses, which the round-trip test pins.
        let inset = SLIDER_SIZE / 2.0;
        match self.orientation {
            Orientation::Horizontal => {
                let travel = (rect.width as f32 - inset * 2.0).max(0.0);
                if travel == 0.0 {
                    return self.minimum;
                }
                let left_fraction = ((pos - rect.x as f32 - inset) / travel).clamp(0.0, 1.0);
                // The pixel is measured from the left edge, but the value runs in reading order, so
                // the direction converts between the two frames. Without it an RTL slider reported
                // the *inverse* value of the position a user clicked — the defect this fixes.
                let relative = self.direction.left_fraction_to_begin_fraction(left_fraction);
                let value = self.minimum as f32 + range * relative;
                value.round() as i32
            }
            Orientation::Vertical => {
                let travel = (rect.height as f32 - inset * 2.0).max(0.0);
                if travel == 0.0 {
                    return self.minimum;
                }
                // Vertical runs top-to-bottom in every direction: the block axis is not mirrored, so
                // the direction must not be applied here.
                let relative = 1.0 - ((pos - rect.y as f32 - inset) / travel).clamp(0.0, 1.0);
                let value = self.minimum as f32 + range * relative;
                value.round() as i32
            }
        }
    }
    /// Returns pixel position for a given value.
    fn value_to_pixel_pos(&self, value: i32) -> f32 {
        let rect = self.geometry();
        let clamped = ordered_clamp_i32(value, self.minimum, self.maximum);
        // The handle is `SLIDER_SIZE` wide and is *centred* on the position this returns, so
        // the travel range has to stop half a handle short of each end. Using the full width
        // put the minimum value's handle centre on `rect.x`, i.e. half of the handle outside
        // the control; the raster backend clipped it and the SVG snapshot showed it hanging
        // off the left edge, which is how the defect was found.
        let inset = SLIDER_SIZE / 2.0;
        let travel = match self.orientation {
            Orientation::Horizontal => (rect.width as f32 - inset * 2.0).max(0.0),
            Orientation::Vertical => (rect.height as f32 - inset * 2.0).max(0.0),
        };
        let range = (self.maximum - self.minimum) as f32;
        if range == 0.0 {
            return match self.orientation {
                Orientation::Horizontal => rect.x as f32 + inset,
                Orientation::Vertical => rect.y as f32 + rect.height as f32 / 2.0,
            };
        }
        let relative = (clamped - self.minimum) as f32 / range;
        match self.orientation {
            Orientation::Horizontal => {
                // The value runs in reading order; the pixel is measured from the left edge. The
                // direction is the single conversion between the two, and it is the same one
                // `pixel_pos_to_value` applies in reverse — so a handle drawn at a position reads
                // back as the value that put it there.
                let left_fraction = self.direction.begin_fraction_to_left_fraction(relative);
                rect.x as f32 + inset + travel * left_fraction
            }
            Orientation::Vertical => {
                rect.y as f32 + inset + travel * (1.0 - relative) // Invert Y axis
            }
        }
    }

    /// Returns the direction the value axis runs in.
    pub fn direction(&self) -> crate::core::TextDirection {
        self.direction
    }

    /// The x coordinate the handle is centred on for the current value, in the control's own
    /// geometry.
    ///
    /// Exposed because the handle's position is the slider's visible answer to "where is the
    /// value", and a host drawing an overlay (a value bubble, a custom groove) needs to agree with
    /// the slider about it rather than recompute the mapping and drift.
    pub fn handle_centre_x(&self) -> f32 {
        self.value_to_pixel_pos(self.value)
    }

    /// The y coordinate the handle is centred on for the current value, for a vertical slider.
    pub fn handle_centre_y(&self) -> f32 {
        self.value_to_pixel_pos(self.value)
    }

    /// The value a pointer at `x` selects, in the slider's own geometry.
    ///
    /// The inverse of [`Self::handle_centre_x`], so a host forwarding a drag and a host drawing the
    /// handle use one mapping between them.
    pub fn value_at_x(&self, x: f32) -> i32 {
        self.pixel_pos_to_value(x)
    }

    /// Sets the writing direction the value axis runs in.
    ///
    /// A right-to-left slider puts the minimum at the right edge, so dragging left raises the value.
    /// Set this at construction or when the locale changes; the redraw is requested here so a host
    /// does not have to remember to.
    pub fn set_direction(&mut self, direction: crate::core::TextDirection) {
        if self.direction == direction {
            return;
        }
        self.direction = direction;
        self.base.request_redraw();
    }
}
/// Slider actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SliderAction {
    /// Move slider by single step up/right
    SliderSingleStepAdd,
    /// Move slider by single step down/left
    SliderSingleStepSub,
    /// Move slider by page step up/right
    SliderPageStepAdd,
    /// Move slider by page step down/left
    SliderPageStepSub,
    /// Move slider to minimum
    SliderToMinimum,
    /// Move slider to maximum
    SliderToMaximum,
    /// Move slider to arbitrary position
    SliderMove,
}
// Implement Widget trait
impl Widget for Slider {
    /// Resolves the published event names this control emits to their signals.
    ///
    /// | published name | signal | payload |
    /// |---|---|---|
    /// | `value_changed` | `value_changed` | `i32` |
    /// | `slider_moved` | `slider_moved` | `i32` |
    /// | `slider_pressed` | `slider_pressed` | none |
    /// | `slider_released` | `slider_released` | none |
    ///
    /// `value_changed` and `slider_moved` carry the same type but mean different things: the first
    /// fires on every value change including a programmatic setter, the second only while the handle
    /// is dragged. A designer offers both because they are both published, and the payload type it
    /// shows is the same for each.
    ///
    /// `clicked` is deliberately **not** here: the control owns that signal on its base, but its
    /// capability does not publish the name, so an arm resolving it would be dead code that looks
    /// like support. `tools/check_event_signal_dyn.py` fails on either half of that mismatch.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "value_changed" => {
                Some(EventSignalRef::mapped("value_changed", &self.value_changed, |value| {
                    CapabilityValue::Int(*value as i64)
                }))
            }
            "slider_moved" => {
                Some(EventSignalRef::mapped("slider_moved", &self.slider_moved, |value| {
                    CapabilityValue::Int(*value as i64)
                }))
            }
            "slider_pressed" => Some(EventSignalRef::unit("slider_pressed", &self.slider_pressed)),
            "slider_released" => {
                Some(EventSignalRef::unit("slider_released", &self.slider_released))
            }
            _ => None,
        }
    }

    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        match self.orientation() {
            Orientation::Horizontal => Size::new(120, 20),
            Orientation::Vertical => Size::new(20, 120),
        }
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `Slider`'s property contract.
///
/// The control names its own properties here, reads and writes them against its
/// own fields, and forwards every name it does not recognise to the shared base
/// helpers. Semantics mirror the previous centralised dispatch exactly.
impl WidgetProperties for Slider {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "minimum" => Ok(CapabilityValue::Int(self.minimum() as i64)),
            "maximum" => Ok(CapabilityValue::Int(self.maximum() as i64)),
            "value" => Ok(CapabilityValue::Int(self.value() as i64)),
            "single_step" => Ok(CapabilityValue::Int(self.single_step() as i64)),
            "page_step" => Ok(CapabilityValue::Int(self.page_step() as i64)),
            "orientation" => {
                Ok(CapabilityValue::String(orientation_to_str(self.orientation()).to_string()))
            }
            "tick_position" => {
                Ok(CapabilityValue::String(tick_position_to_str(self.tick_position()).to_string()))
            }
            "tick_interval" => Ok(CapabilityValue::Int(self.tick_interval() as i64)),
            "tracking" => Ok(CapabilityValue::Bool(self.tracking())),
            "slider_position" => Ok(CapabilityValue::Int(self.slider_position() as i64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "minimum" => {
                self.set_minimum(expect_i64(value)? as i32);
                Ok(())
            }
            "maximum" => {
                self.set_maximum(expect_i64(value)? as i32);
                Ok(())
            }
            "value" => {
                self.set_value(expect_i64(value)? as i32);
                Ok(())
            }
            "single_step" => {
                self.set_single_step(expect_i64(value)? as i32);
                Ok(())
            }
            "page_step" => {
                self.set_page_step(expect_i64(value)? as i32);
                Ok(())
            }
            "orientation" => {
                self.set_orientation(expect_orientation(value)?);
                Ok(())
            }
            "tick_position" => {
                self.set_tick_position(expect_tick_position(value)?);
                Ok(())
            }
            "tick_interval" => {
                self.set_tick_interval(expect_i64(value)? as i32);
                Ok(())
            }
            "tracking" => {
                self.set_tracking(expect_bool(value)?);
                Ok(())
            }
            "slider_position" => {
                self.set_slider_position(expect_i64(value)? as i32);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Mirrors `SLIDER_PROPERTIES`: this control's own names plus the four
        // shared ones that `base_property_get` answers.
        property_names_of![
            "minimum",
            "maximum",
            "value",
            "single_step",
            "page_step",
            "orientation",
            "tick_position",
            "tick_interval",
            "tracking",
            "slider_position",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `slider` publishes.
    ///
    /// All three assign state and need an argument a command carries none of:
    /// `set_range` the bounds, `set_value` the position and `set_slider_position` the
    /// derived handle coordinate. They are refused as
    /// [`CapabilityAccessError::OutOfRange`] so the caller is sent to the property
    /// route (`set("value", ..)`, `set("slider_position", ..)`) rather than to a
    /// different control, which is what `UnknownCommand` would say.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "set_range" | "set_value" | "set_slider_position" => {
                Err(CapabilityAccessError::OutOfRange)
            }
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for Slider {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button } if *button == 1 => {
                self.mouse_pressed = true;
                self.slider_pressed.emit();
                let pixel = match self.orientation {
                    Orientation::Horizontal => pos.x as f32,
                    Orientation::Vertical => pos.y as f32,
                };
                let value = self.pixel_pos_to_value(pixel);
                self.set_slider_position(value);
            }
            #[cfg(feature = "touch")]
            Event::TouchBegin { pos, .. } => {
                self.mouse_pressed = true;
                self.slider_pressed.emit();
                let pixel = match self.orientation {
                    Orientation::Horizontal => pos.x as f32,
                    Orientation::Vertical => pos.y as f32,
                };
                let value = self.pixel_pos_to_value(pixel);
                self.set_slider_position(value);
            }
            Event::MouseRelease { pos: _, button } if *button == 1 => {
                self.mouse_pressed = false;
                self.slider_released.emit();
                if !self.tracking {
                    self.set_value(self.slider_position);
                }
            }
            // A press whose release lands outside the widget never reaches the arm
            // above: the runtime's hit-test returns `None` for a point outside every
            // control, so no `MouseRelease` is delivered. Without this arm the flag
            // stayed set and the *next* hover moved the handle with no button held —
            // the control behaved as if permanently dragging. Cancelling is the right
            // answer rather than committing, because the user left the control.
            Event::MouseLeave { .. } if self.mouse_pressed => {
                self.mouse_pressed = false;
                self.slider_released.emit();
                if !self.tracking {
                    self.set_value(self.slider_position);
                }
            }
            #[cfg(feature = "touch")]
            Event::TouchEnd { .. } => {
                self.mouse_pressed = false;
                self.slider_released.emit();
                if !self.tracking {
                    self.set_value(self.slider_position);
                }
            }
            Event::MouseMove { pos } if self.mouse_pressed => {
                let pixel = match self.orientation {
                    Orientation::Horizontal => pos.x as f32,
                    Orientation::Vertical => pos.y as f32,
                };
                let value = self.pixel_pos_to_value(pixel);
                self.set_slider_position(value);
            }
            #[cfg(feature = "touch")]
            Event::TouchMove { pos, .. } if self.mouse_pressed => {
                let pixel = match self.orientation {
                    Orientation::Horizontal => pos.x as f32,
                    Orientation::Vertical => pos.y as f32,
                };
                let value = self.pixel_pos_to_value(pixel);
                self.set_slider_position(value);
            }
            Event::KeyPress { key, modifiers: _ } => {
                // Arrow keys name a *screen* direction, but a slider's value runs in reading order.
                // The two agree in LTR and are opposite in RTL, so the step is expressed in reading
                // order and converted through the direction once, here — instead of each arm
                // knowing about direction and risking one of them being missed.
                let step =
                    |begin_step: i32| match self.direction.begin_step_to_left_step(begin_step) {
                        // Leftward on screen.
                        step if step < 0 => SliderAction::SliderSingleStepSub,
                        _ => SliderAction::SliderSingleStepAdd,
                    };
                match *key {
                    37 => {
                        // Left arrow (or up arrow for vertical)
                        if self.orientation == Orientation::Vertical {
                            self.trigger_action(SliderAction::SliderSingleStepSub);
                        } else {
                            self.trigger_action(step(-1));
                        }
                    }
                    38 => {
                        // Up arrow (or right arrow for horizontal)
                        if self.orientation == Orientation::Vertical {
                            self.trigger_action(SliderAction::SliderSingleStepAdd);
                        } else {
                            self.trigger_action(step(1));
                        }
                    }
                    39 => {
                        // Right arrow (or down arrow for vertical)
                        if self.orientation == Orientation::Vertical {
                            self.trigger_action(SliderAction::SliderSingleStepAdd);
                        } else {
                            self.trigger_action(step(1));
                        }
                    }
                    40 => {
                        // Down arrow (or left arrow for horizontal)
                        if self.orientation == Orientation::Vertical {
                            self.trigger_action(SliderAction::SliderSingleStepSub);
                        } else {
                            self.trigger_action(step(-1));
                        }
                    }
                    33 => {
                        // Page up
                        self.trigger_action(SliderAction::SliderPageStepSub);
                    }
                    34 => {
                        // Page down
                        self.trigger_action(SliderAction::SliderPageStepAdd);
                    }
                    36 => {
                        // Home
                        self.trigger_action(SliderAction::SliderToMinimum);
                    }
                    35 => {
                        // End
                        self.trigger_action(SliderAction::SliderToMaximum);
                    }
                    _ => { /* Other keys are not relevant */ }
                }
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}
impl Draw for Slider {
    fn draw(&mut self, context: &mut RenderContext) {
        // Draw base widget
        let rect = self.geometry();
        let slider_pos = self.value_to_pixel_pos(self.value);
        let slider_size = SLIDER_SIZE;

        // Chrome colours resolve the explicit style first, then the theme's resolved style for
        // this control, and only then fall back to a literal. The groove read `style` already,
        // but the handle and the ticks were literals, so a light/dark switch left them unchanged
        // — the rendering census reported the control as theme-blind.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global manager's mutex
        // is not re-entrant.
        let style = self.style();
        // `slider` classifies as `WidgetRole::Accent`, whose resolved colours are the theme's
        // accent token and its contrasting ink. `primary` is read alongside it as the crate's
        // conventional value-indicator token, so the handle matches the rest of the library.
        let theme = crate::style::resolved_theme_style("slider");
        // A stripped device build (`mini`/`embedded`) has no theme module, so there is no
        // manager to read. The literals below are the same ones the `None` arm uses, which
        // is what keeps the two profiles rendering alike rather than inventing a palette
        // for the profile that has none (principle #37).
        #[cfg(device_profile)]
        let (window_fill, foreground, primary, accent, muted) = {
            let manager = crate::style::theme_manager();
            match manager.current_theme() {
                Some(active) => (
                    active.colors.background,
                    active.colors.foreground,
                    active.colors.primary,
                    active.colors.accent,
                    active.colors.secondary,
                ),
                None => (
                    Color::rgb(240, 240, 240),
                    Color::BLACK,
                    Color::rgb(33, 150, 243),
                    Color::rgb(255, 152, 0),
                    Color::rgb(158, 158, 158),
                ),
            }
        };
        #[cfg(not(device_profile))]
        let (window_fill, foreground, primary, accent, muted) = (
            Color::rgb(240, 240, 240),
            Color::BLACK,
            Color::rgb(33, 150, 243),
            Color::rgb(255, 152, 0),
            Color::rgb(158, 158, 158),
        );

        // The groove: a caller's own colour wins, then the theme's resolved background. A control
        // classified as `Surface` or `Accent` resolves to something that is not the window fill on
        // its own, but the filter keeps a window-coloured value from being painted as the track.
        let groove_color = style
            .background_color
            .filter(|resolved| *resolved != window_fill)
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .filter(|resolved| *resolved != window_fill)
            .unwrap_or_else(|| window_fill.blend(&foreground, 0.14));
        // The handle is the largest area the slider paints and the census measures it as the
        // dominant colour, so it is what must move with the appearance: it carries the theme's
        // primary (the value indicator) rather than a fixed blue. A caller's border colour, which
        // is what the old code used to outline the handle, is not reused for the fill — it stands
        // in for the handle's own outline below.
        let handle_color = primary;
        // Ticks are de-emphasised from the groove rather than being a fixed grey.
        let tick_color = groove_color.blend(&muted, 0.55);
        // The handle's outline, one visible step from the handle itself. `accent` is kept in the
        // read so the palette tuple stays uniform across the material controls.
        let handle_border = style
            .border_color
            .filter(|resolved| *resolved != handle_color)
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .filter(|resolved| *resolved != handle_color)
            .unwrap_or_else(|| handle_color.blend(&accent, 0.40));
        // Draw groove (track)
        match self.orientation {
            Orientation::Horizontal => {
                let groove_y = rect.y as f32 + rect.height as f32 / 2.0;
                let groove_height = 4;
                // Draw groove
                context.fill_rect(
                    Rect::from_f32(
                        rect.x as f32,
                        groove_y - groove_height as f32 / 2.0,
                        rect.width as f32,
                        groove_height as f32,
                    ),
                    groove_color,
                );
                // Draw slider handle
                context.fill_rect(
                    Rect::from_f32(
                        slider_pos - SLIDER_SIZE / 2.0,
                        rect.y as f32,
                        slider_size,
                        rect.height as f32,
                    ),
                    handle_color,
                );
                // Draw handle border
                if let Some(border_color) = style.border_color.filter(|c| *c != handle_color) {
                    context.draw_rect(
                        Rect::from_f32(
                            slider_pos - SLIDER_SIZE / 2.0,
                            rect.y as f32,
                            slider_size,
                            rect.height as f32,
                        ),
                        border_color,
                    );
                } else {
                    context.draw_rect(
                        Rect::from_f32(
                            slider_pos - SLIDER_SIZE / 2.0,
                            rect.y as f32,
                            slider_size,
                            rect.height as f32,
                        ),
                        handle_border,
                    );
                }
                // Draw ticks if enabled (capped at 100 ticks max to avoid
                // performance issues with large ranges and small intervals).
                if self.tick_position != TickPosition::NoTicks && self.tick_interval > 0 {
                    let tick_height = 6;
                    let total_ticks =
                        ((self.maximum - self.minimum) / self.tick_interval) as u32 + 1;
                    let tick_count = total_ticks.min(100);
                    for i in 0..tick_count {
                        let value = self.minimum + i as i32 * self.tick_interval;
                        let tick_x = self.value_to_pixel_pos(value);
                        if self.tick_position == TickPosition::TicksAbove
                            || self.tick_position == TickPosition::TicksBothSides
                        {
                            context.draw_line(
                                Point::from_f32(tick_x, rect.y as f32),
                                Point::from_f32(tick_x, rect.y as f32 + tick_height as f32),
                                tick_color,
                            );
                        }
                        if self.tick_position == TickPosition::TicksBelow
                            || self.tick_position == TickPosition::TicksBothSides
                        {
                            context.draw_line(
                                Point::from_f32(
                                    tick_x,
                                    rect.y as f32 + rect.height as f32 - tick_height as f32,
                                ),
                                Point::from_f32(tick_x, rect.y as f32 + rect.height as f32),
                                tick_color,
                            );
                        }
                    }
                }
            }
            Orientation::Vertical => {
                let groove_x = rect.x as f32 + rect.width as f32 / 2.0;
                let groove_width = 4;
                // Draw groove
                context.fill_rect(
                    Rect::from_f32(
                        groove_x - groove_width as f32 / 2.0,
                        rect.y as f32,
                        groove_width as f32,
                        rect.height as f32,
                    ),
                    groove_color,
                );
                // Draw slider handle
                context.fill_rect(
                    Rect::from_f32(
                        rect.x as f32,
                        slider_pos - SLIDER_SIZE / 2.0,
                        rect.width as f32,
                        slider_size,
                    ),
                    handle_color,
                );
                // Draw handle border
                if let Some(border_color) = style.border_color.filter(|c| *c != handle_color) {
                    context.draw_rect(
                        Rect::from_f32(
                            rect.x as f32,
                            slider_pos - SLIDER_SIZE / 2.0,
                            rect.width as f32,
                            slider_size,
                        ),
                        border_color,
                    );
                } else {
                    context.draw_rect(
                        Rect::from_f32(
                            rect.x as f32,
                            slider_pos - SLIDER_SIZE / 2.0,
                            rect.width as f32,
                            slider_size,
                        ),
                        handle_border,
                    );
                }
                // Draw ticks if enabled (capped at 100 ticks max to avoid
                // performance issues with large ranges and small intervals).
                if self.tick_position != TickPosition::NoTicks && self.tick_interval > 0 {
                    let tick_width = 6;
                    let total_ticks =
                        ((self.maximum - self.minimum) / self.tick_interval) as u32 + 1;
                    let tick_count = total_ticks.min(100);
                    for i in 0..tick_count {
                        let value = self.minimum + i as i32 * self.tick_interval;
                        let tick_y = self.value_to_pixel_pos(value);
                        if self.tick_position == TickPosition::TicksAbove
                            || self.tick_position == TickPosition::TicksBothSides
                        {
                            context.draw_line(
                                Point::from_f32(rect.x as f32, tick_y),
                                Point::from_f32(rect.x as f32 + tick_width as f32, tick_y),
                                tick_color,
                            );
                        }
                        if self.tick_position == TickPosition::TicksBelow
                            || self.tick_position == TickPosition::TicksBothSides
                        {
                            context.draw_line(
                                Point::from_f32(
                                    rect.x as f32 + rect.width as f32 - tick_width as f32,
                                    tick_y,
                                ),
                                Point::from_f32(rect.x as f32 + rect.width as f32, tick_y),
                                tick_color,
                            );
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Color, Orientation, Rect, Size};
    use crate::event::Event;
    use crate::style::WidgetStyle;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    /// Helper to create a default slider with a standard geometry.
    fn make_slider() -> Slider {
        Slider::new(Rect::new(0, 0, 200, 30))
    }

    // ── 0. Writing direction (BLUE21 P2-10) ──

    /// The default slider is left-to-right, so nothing about it changed.
    ///
    /// This is the assertion that makes the direction field safe to add: every existing caller
    /// keeps the exact behaviour it had, and a control only becomes direction-aware by asking.
    #[test]
    fn a_slider_defaults_to_left_to_right() {
        let s = make_slider();
        assert_eq!(s.direction(), crate::core::TextDirection::LeftToRight);
    }

    /// A right-to-left slider puts its minimum at the right edge and its maximum at the left.
    ///
    /// # The defect this pins
    ///
    /// Value and position were mapped by one identity, so in a right-to-left interface the handle
    /// moved the *opposite* way from the value it reported: dragging toward the right lowered the
    /// value. That is not a translation gap — the control was functionally wrong, which is why the
    /// plan singled RTL out as the one functional defect among the appearance findings.
    ///
    /// The assertion is deliberately about *opposition*, not about a specific pixel: what makes the
    /// control correct is that the two directions disagree about which end is the beginning.
    #[test]
    fn a_right_to_left_slider_maps_its_value_the_other_way_round() {
        let mut ltr = make_slider();
        ltr.set_range(0, 100);
        ltr.set_value(0);
        let ltr_min_x = ltr.handle_centre_x();
        ltr.set_value(100);
        let ltr_max_x = ltr.handle_centre_x();
        assert!(ltr_min_x < ltr_max_x, "left-to-right runs minimum\u{2192}maximum left to right");

        let mut rtl = make_slider();
        rtl.set_range(0, 100);
        rtl.set_direction(crate::core::TextDirection::RightToLeft);
        rtl.set_value(0);
        let rtl_min_x = rtl.handle_centre_x();
        rtl.set_value(100);
        let rtl_max_x = rtl.handle_centre_x();
        assert!(rtl_min_x > rtl_max_x, "right-to-left runs minimum\u{2192}maximum right to left");

        // And the two are exact mirrors of each other, which is stronger than mere opposition:
        // an implementation that merely swapped the ends would still get the travel wrong.
        let width = 200.0;
        assert!(
            ((ltr_min_x + rtl_min_x) - width).abs() < 1.0,
            "the minimum sits at mirrored positions: {ltr_min_x} vs {rtl_min_x}"
        );
        assert!(
            ((ltr_max_x + rtl_max_x) - width).abs() < 1.0,
            "the maximum sits at mirrored positions: {ltr_max_x} vs {rtl_max_x}"
        );
    }

    /// The drawn handle and the value read from a position are inverses in both directions.
    ///
    /// This is the property that keeps a drag from drifting away from the pointer: `draw` places
    /// the handle with one conversion and the drag handler reads a value back with the other. If
    /// they disagreed, dragging under RTL would walk the handle out from under the pointer — a bug
    /// that only appears in one direction and would be very hard to attribute later.
    #[test]
    fn the_two_directions_round_trip_a_value_through_a_position() {
        for direction in
            [crate::core::TextDirection::LeftToRight, crate::core::TextDirection::RightToLeft]
        {
            let mut s = make_slider();
            s.set_range(0, 100);
            s.set_direction(direction);
            for value in [0, 1, 25, 50, 75, 99, 100] {
                s.set_value(value);
                let x = s.handle_centre_x();
                let read_back = s.value_at_x(x);
                assert!(
                    (read_back - value).abs() <= 1,
                    "{direction:?}: value {value} at x={x} read back as {read_back}"
                );
            }
        }
    }

    /// A horizontal right-to-left slider steps toward the maximum on the left arrow.
    ///
    /// The arrow keys name a screen direction, and the value runs in reading order; those coincide
    /// in LTR and are opposite in RTL. The defect the plan records is precisely that the arrow arms
    /// did not consult direction at all, so an RTL user pressing left moved the value the way an LTR
    /// user would.
    #[test]
    fn the_left_arrow_steps_toward_the_maximum_in_a_right_to_left_slider() {
        // Left-to-right: the left arrow lowers the value.
        let mut ltr = make_slider();
        ltr.set_range(0, 100);
        ltr.set_value(50);
        ltr.handle_event(&Event::KeyPress { key: 37, modifiers: 0 });
        assert_eq!(ltr.value(), 49, "left arrow decrements in a left-to-right slider");

        // Right-to-left: the left arrow *raises* it, because left is further along the line.
        let mut rtl = make_slider();
        rtl.set_range(0, 100);
        rtl.set_direction(crate::core::TextDirection::RightToLeft);
        rtl.set_value(50);
        rtl.handle_event(&Event::KeyPress { key: 37, modifiers: 0 });
        assert_eq!(rtl.value(), 51, "left arrow increments in a right-to-left slider");

        // The right arrow is the mirror, so both directions are covered.
        rtl.handle_event(&Event::KeyPress { key: 39, modifiers: 0 });
        assert_eq!(rtl.value(), 50, "right arrow decrements in a right-to-left slider");
    }

    /// A vertical slider is not mirrored, because the block axis does not flip with direction.
    ///
    /// Vertical sliders run top-to-bottom in every writing direction; applying the direction to the
    /// vertical axis would be a second defect rather than a fix for the first.
    #[test]
    fn a_vertical_slider_ignores_the_writing_direction() {
        let mut ltr = Slider::new(Rect::new(0, 0, 30, 200));
        ltr.set_orientation(Orientation::Vertical);
        ltr.set_range(0, 100);
        ltr.set_value(0);
        let ltr_min_y = ltr.handle_centre_y();
        ltr.set_value(100);
        let ltr_max_y = ltr.handle_centre_y();

        let mut rtl = Slider::new(Rect::new(0, 0, 30, 200));
        rtl.set_orientation(Orientation::Vertical);
        rtl.set_direction(crate::core::TextDirection::RightToLeft);
        rtl.set_range(0, 100);
        rtl.set_value(0);
        let rtl_min_y = rtl.handle_centre_y();
        rtl.set_value(100);
        let rtl_max_y = rtl.handle_centre_y();

        assert_eq!(ltr_min_y, rtl_min_y, "the vertical axis is not mirrored");
        assert_eq!(ltr_max_y, rtl_max_y, "the vertical axis is not mirrored");
    }

    // ── 1. Creation defaults ──

    #[test]
    fn default_range_is_0_to_100() {
        let s = make_slider();
        assert_eq!(s.minimum(), 0);
        assert_eq!(s.maximum(), 100);
    }

    #[test]
    fn default_value_is_0() {
        let s = make_slider();
        assert_eq!(s.value(), 0);
    }

    #[test]
    fn default_orientation_horizontal() {
        let s = make_slider();
        assert_eq!(s.orientation(), Orientation::Horizontal);
    }

    #[test]
    fn default_tracking_is_true() {
        let s = make_slider();
        assert!(s.tracking());
    }

    #[test]
    fn default_tick_position_is_no_ticks() {
        let s = make_slider();
        assert_eq!(s.tick_position(), TickPosition::NoTicks);
    }

    #[test]
    fn default_single_step_is_1() {
        let s = make_slider();
        assert_eq!(s.single_step(), 1);
    }

    #[test]
    fn default_page_step_is_10() {
        let s = make_slider();
        assert_eq!(s.page_step(), 10);
    }

    // ── 2. set_value / clamping ──

    #[test]
    fn set_value_normal() {
        let mut s = make_slider();
        s.set_value(42);
        assert_eq!(s.value(), 42);
    }

    #[test]
    fn set_value_clamps_below_minimum() {
        let mut s = make_slider();
        s.set_value(-10);
        assert_eq!(s.value(), 0);
    }

    #[test]
    fn set_value_clamps_above_maximum() {
        let mut s = make_slider();
        s.set_value(200);
        assert_eq!(s.value(), 100);
    }

    #[test]
    fn set_value_same_value_does_not_emit() {
        let mut s = make_slider();
        let count = Arc::new(AtomicUsize::new(0));
        let c = Arc::clone(&count);
        s.value_changed.connect(move |_| {
            c.fetch_add(1, Ordering::SeqCst);
        });
        s.set_value(0);
        assert_eq!(count.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn set_value_emits_on_change() {
        let mut s = make_slider();
        let count = Arc::new(AtomicUsize::new(0));
        let c = Arc::clone(&count);
        s.value_changed.connect(move |_| {
            c.fetch_add(1, Ordering::SeqCst);
        });
        s.set_value(55);
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    // ── 3. set_minimum / set_maximum / set_range ──

    #[test]
    fn set_minimum_adjusts_maximum_when_crossed() {
        let mut s = make_slider();
        s.set_minimum(150);
        assert_eq!(s.minimum(), 150);
        assert_eq!(s.maximum(), 150);
    }

    #[test]
    fn set_minimum_reclamps_value() {
        let mut s = make_slider();
        s.set_value(50);
        s.set_minimum(60);
        assert_eq!(s.value(), 60);
    }

    #[test]
    fn set_maximum_adjusts_minimum_when_crossed() {
        let mut s = make_slider();
        s.set_maximum(-20);
        assert_eq!(s.maximum(), -20);
        assert_eq!(s.minimum(), -20);
    }

    #[test]
    fn set_maximum_reclamps_value() {
        let mut s = make_slider();
        s.set_value(80);
        s.set_maximum(40);
        assert_eq!(s.value(), 40);
    }

    #[test]
    fn set_range_clamps_maximum_not_below_minimum() {
        let mut s = make_slider();
        s.set_range(10, 5);
        assert_eq!(s.minimum(), 10);
        assert_eq!(s.maximum(), 10);
    }

    #[test]
    fn set_range_reclamps_value() {
        let mut s = make_slider();
        s.set_value(50);
        s.set_range(0, 30);
        assert_eq!(s.value(), 30);
    }

    // ── 4. value_changed signal emission ──

    #[test]
    fn value_changed_emits_new_value() {
        let mut s = make_slider();
        let emitted = Arc::new(AtomicUsize::new(0));
        let e = Arc::clone(&emitted);
        s.value_changed.connect(move |v| {
            e.store(*v as usize, Ordering::SeqCst);
        });
        s.set_value(77);
        assert_eq!(emitted.load(Ordering::SeqCst), 77);
    }

    #[test]
    fn value_changed_not_emitted_on_noop() {
        let mut s = make_slider();
        s.set_value(33);
        let count = Arc::new(AtomicUsize::new(0));
        let c = Arc::clone(&count);
        s.value_changed.connect(move |_| {
            c.fetch_add(1, Ordering::SeqCst);
        });
        s.set_value(33);
        assert_eq!(count.load(Ordering::SeqCst), 0);
    }

    // ── 5. slider_moved, slider_pressed, slider_released signals ──

    #[test]
    fn slider_moved_emitted_on_set_slider_position() {
        let mut s = make_slider();
        let emitted = Arc::new(AtomicUsize::new(999));
        let e = Arc::clone(&emitted);
        s.slider_moved.connect(move |v| {
            e.store(*v as usize, Ordering::SeqCst);
        });
        s.set_slider_position(30);
        assert_eq!(emitted.load(Ordering::SeqCst), 30);
    }

    #[test]
    fn slider_pressed_emitted_on_mouse_press() {
        let mut s = make_slider();
        let pressed = Arc::new(AtomicUsize::new(0));
        let p = Arc::clone(&pressed);
        s.base.set_geometry(Rect::new(0, 0, 200, 30));
        s.slider_pressed.connect(move || {
            p.fetch_add(1, Ordering::SeqCst);
        });
        s.handle_event(&Event::mouse_press(50, 15, 1));
        assert_eq!(pressed.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn slider_released_emitted_on_mouse_release() {
        let mut s = make_slider();
        let released = Arc::new(AtomicUsize::new(0));
        let r = Arc::clone(&released);
        s.base.set_geometry(Rect::new(0, 0, 200, 30));
        s.slider_released.connect(move || {
            r.fetch_add(1, Ordering::SeqCst);
        });
        s.handle_event(&Event::mouse_press(50, 15, 1));
        s.handle_event(&Event::mouse_release(50, 15, 1));
        assert_eq!(released.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn slider_moved_during_drag() {
        let mut s = make_slider();
        let moved = Arc::new(AtomicUsize::new(999));
        let m = Arc::clone(&moved);
        s.base.set_geometry(Rect::new(0, 0, 200, 30));
        s.slider_moved.connect(move |v| {
            m.store(*v as usize, Ordering::SeqCst);
        });
        s.handle_event(&Event::mouse_press(10, 15, 1));
        s.handle_event(&Event::mouse_move(100, 15));
        // After a drag the slider_moved should have been emitted.
        assert_ne!(moved.load(Ordering::SeqCst), 999);
    }

    // ── 6. single_step / page_step set/get (floor at 1) ──

    #[test]
    fn single_step_floors_at_1() {
        let mut s = make_slider();
        s.set_single_step(0);
        assert_eq!(s.single_step(), 1);
        s.set_single_step(-5);
        assert_eq!(s.single_step(), 1);
    }

    #[test]
    fn single_step_normal() {
        let mut s = make_slider();
        s.set_single_step(5);
        assert_eq!(s.single_step(), 5);
    }

    #[test]
    fn page_step_floors_at_1() {
        let mut s = make_slider();
        s.set_page_step(0);
        assert_eq!(s.page_step(), 1);
        s.set_page_step(-3);
        assert_eq!(s.page_step(), 1);
    }

    #[test]
    fn page_step_normal() {
        let mut s = make_slider();
        s.set_page_step(15);
        assert_eq!(s.page_step(), 15);
    }

    // ── 7. orientation set/get ──

    #[test]
    fn set_orientation_vertical() {
        let mut s = make_slider();
        s.set_orientation(Orientation::Vertical);
        assert_eq!(s.orientation(), Orientation::Vertical);
    }

    #[test]
    fn set_orientation_horizontal() {
        let mut s = make_slider();
        s.set_orientation(Orientation::Vertical);
        s.set_orientation(Orientation::Horizontal);
        assert_eq!(s.orientation(), Orientation::Horizontal);
    }

    // ── 8. trigger_action for all 8 SliderAction variants ──

    #[test]
    fn trigger_action_single_step_add() {
        let mut s = make_slider();
        s.set_value(5);
        s.trigger_action(SliderAction::SliderSingleStepAdd);
        assert_eq!(s.value(), 6);
    }

    #[test]
    fn trigger_action_single_step_sub() {
        let mut s = make_slider();
        s.set_value(5);
        s.trigger_action(SliderAction::SliderSingleStepSub);
        assert_eq!(s.value(), 4);
    }

    #[test]
    fn trigger_action_page_step_add() {
        let mut s = make_slider();
        s.set_value(5);
        s.trigger_action(SliderAction::SliderPageStepAdd);
        assert_eq!(s.value(), 15);
    }

    #[test]
    fn trigger_action_page_step_sub() {
        let mut s = make_slider();
        s.set_value(50);
        s.trigger_action(SliderAction::SliderPageStepSub);
        assert_eq!(s.value(), 40);
    }

    #[test]
    fn trigger_action_to_minimum() {
        let mut s = make_slider();
        s.set_value(80);
        s.trigger_action(SliderAction::SliderToMinimum);
        assert_eq!(s.value(), s.minimum());
    }

    #[test]
    fn trigger_action_to_maximum() {
        let mut s = make_slider();
        s.set_value(10);
        s.trigger_action(SliderAction::SliderToMaximum);
        assert_eq!(s.value(), s.maximum());
    }

    #[test]
    fn trigger_action_slider_move_no_op() {
        let mut s = make_slider();
        s.set_value(50);
        s.trigger_action(SliderAction::SliderMove);
        // SliderMove is a no-op handled by mouse events; value unchanged.
        assert_eq!(s.value(), 50);
    }

    #[test]
    fn trigger_action_single_step_sub_clamps_to_minimum() {
        let mut s = make_slider();
        s.set_value(0);
        s.trigger_action(SliderAction::SliderSingleStepSub);
        assert_eq!(s.value(), 0);
    }

    #[test]
    fn trigger_action_single_step_add_clamps_to_maximum() {
        let mut s = make_slider();
        s.set_value(100);
        s.trigger_action(SliderAction::SliderSingleStepAdd);
        assert_eq!(s.value(), 100);
    }

    #[test]
    fn trigger_action_page_step_sub_clamps_to_minimum() {
        let mut s = make_slider();
        s.set_value(3);
        s.trigger_action(SliderAction::SliderPageStepSub);
        assert_eq!(s.value(), 0);
    }

    #[test]
    fn trigger_action_page_step_add_clamps_to_maximum() {
        let mut s = make_slider();
        s.set_value(95);
        s.trigger_action(SliderAction::SliderPageStepAdd);
        assert_eq!(s.value(), 100);
    }

    // ── 9. set_slider_position with tracking=true/false ──

    #[test]
    fn set_slider_position_with_tracking_emits_value_changed() {
        let mut s = make_slider();
        let count = Arc::new(AtomicUsize::new(0));
        let c = Arc::clone(&count);
        s.value_changed.connect(move |_| {
            c.fetch_add(1, Ordering::SeqCst);
        });
        s.set_slider_position(60);
        // tracking=true by default → set_value called → value_changed emitted
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(s.value(), 60);
    }

    #[test]
    fn set_slider_position_without_tracking_does_not_emit_value_changed() {
        let mut s = make_slider();
        s.set_tracking(false);
        let count = Arc::new(AtomicUsize::new(0));
        let c = Arc::clone(&count);
        s.value_changed.connect(move |_| {
            c.fetch_add(1, Ordering::SeqCst);
        });
        s.set_slider_position(60);
        // tracking=false → no set_value → no value_changed
        assert_eq!(count.load(Ordering::SeqCst), 0);
        // But value is NOT updated; only slider_position is set
        assert_eq!(s.value(), 0);
        assert_eq!(s.slider_position(), 60);
    }

    #[test]
    fn set_slider_position_clamps_to_range() {
        let mut s = make_slider();
        s.set_slider_position(999);
        assert_eq!(s.slider_position(), 100);
        s.set_slider_position(-50);
        assert_eq!(s.slider_position(), 0);
    }

    #[test]
    fn set_slider_position_without_tracking_applies_on_release() {
        let mut s = make_slider();
        s.set_tracking(false);
        s.base.set_geometry(Rect::new(0, 0, 200, 30));
        s.handle_event(&Event::mouse_press(80, 15, 1));
        s.handle_event(&Event::mouse_release(80, 15, 1));
        // On release, if !tracking, set_value is called with slider_position.
        //
        // The expected value is `(80 - inset) / travel * 100` = `72 / 184 * 100` = 39.1 -> 39. It
        // used to be asserted as `80 / 200 * 100 = 40`, because the read direction divided by the
        // full width while the draw direction inset by half a handle. That made the control answer a
        // value the handle was not showing, and the two directions were not inverses; the round-trip
        // test above is what caught it, and this expectation moved with the fix rather than the fix
        // being shaped to keep a wrong number.
        assert_eq!(s.value(), 39);
    }

    // ── 10. Widget trait delegation ──

    #[test]
    fn widget_id_delegation() {
        let s = make_slider();
        // id is always non-zero for a fresh BaseWidget
        assert_ne!(s.id(), 0u64);
    }

    #[test]
    fn widget_kind_is_slider() {
        let s = make_slider();
        assert_eq!(s.kind(), WidgetKind::Slider);
    }

    #[test]
    fn widget_geometry_roundtrip() {
        let mut s = make_slider();
        let new_rect = Rect::new(10, 20, 300, 50);
        s.set_geometry(new_rect);
        assert_eq!(s.geometry(), new_rect);
    }

    #[test]
    fn widget_visibility() {
        let mut s = make_slider();
        assert!(s.is_visible());
        s.hide();
        assert!(!s.is_visible());
        s.show();
        assert!(s.is_visible());
    }

    #[test]
    fn widget_enabled() {
        let mut s = make_slider();
        assert!(s.is_enabled());
        s.set_enabled(false);
        assert!(!s.is_enabled());
        s.set_enabled(true);
        assert!(s.is_enabled());
    }

    #[test]
    fn widget_parent_roundtrip() {
        let mut s = make_slider();
        let parent = 42u64;
        s.set_parent(Some(parent));
        assert_eq!(s.parent(), Some(parent));
        s.set_parent(None);
        assert_eq!(s.parent(), None);
    }

    #[test]
    fn widget_children_delegation() {
        let mut s = make_slider();
        let child = 99u64;
        assert!(s.children().is_empty());
        s.add_child(child);
        assert_eq!(s.children().len(), 1);
        assert_eq!(s.children()[0], child);
        s.remove_child(child);
        assert!(s.children().is_empty());
    }

    #[test]
    fn widget_min_max_size() {
        let mut s = make_slider();
        assert_eq!(s.min_size(), None);
        assert_eq!(s.max_size(), None);
        let min = Size::new(50, 20);
        let max = Size::new(500, 100);
        s.set_min_size(Some(min));
        s.set_max_size(Some(max));
        assert_eq!(s.min_size(), Some(min));
        assert_eq!(s.max_size(), Some(max));
    }

    #[test]
    fn widget_tooltip_roundtrip() {
        let mut s = make_slider();
        s.set_tooltip("Volume".to_string());
        assert_eq!(s.tooltip(), "Volume");
    }

    #[test]
    fn widget_style_roundtrip() {
        let mut s = make_slider();
        let style =
            WidgetStyle { background_color: Some(Color::rgb(255, 0, 0)), ..WidgetStyle::default() };
        s.set_style(style.clone());
        assert_eq!(s.style().background_color, Some(Color::rgb(255, 0, 0)));
    }

    #[test]
    fn a_drag_that_leaves_the_control_stops_tracking_the_pointer() {
        // A press whose release lands outside the widget never delivers `MouseRelease`:
        // the runtime's hit-test answers `None` for a point outside every control. With
        // no `MouseLeave` arm `mouse_pressed` stayed set, so every later *hover* moved
        // the handle with no button held — the slider behaved as if permanently
        // dragging.
        let mut s = make_slider();
        s.set_range(0, 100);
        s.set_value(50);

        s.handle_event(&Event::MousePress { pos: Point::new(100, 15), button: 1 });
        // The pointer leaves without a release being delivered.
        s.handle_event(&Event::MouseLeave { pos: Point::new(0, 0) });

        let after_leave = s.value();
        // Hover moves must not change anything now: no button is held.
        for x in [20, 40, 60, 160, 190] {
            s.handle_event(&Event::MouseMove { pos: Point::new(x, 15) });
        }
        assert_eq!(
            s.value(),
            after_leave,
            "hovering after the pointer left must not move the handle"
        );
    }

    #[test]
    fn widget_signals_accessible() {
        let s = make_slider();
        // All signal accessors should return valid signal references without panicking.
        let _ = s.hover_signal();
        let _ = s.mouse_down_signal();
        let _ = s.mouse_up_signal();
        let _ = s.key_down_signal();
        let _ = s.key_up_signal();
        let _ = s.focus_gained_signal();
        let _ = s.focus_lost_signal();
        let _ = s.redraw_requested_signal();
        let _ = s.layout_requested_signal();
        let _ = s.connection_scope();
    }

    #[test]
    fn widget_signal_emission_hover() {
        let mut s = make_slider();
        let count = Arc::new(AtomicUsize::new(0));
        let c = Arc::clone(&count);
        s.base.hover_signal().connect(move |_| {
            c.fetch_add(1, Ordering::SeqCst);
        });
        // The base widget's handle_event emits hover_signal on MouseMove.
        s.base.handle_event(&Event::mouse_move(10, 5));
        // One emission from the event handler.
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    // ── 12. Edge cases ─────────────────────────────────────────────────
    #[test]
    fn test_slider_negative_range() {
        // Set range with min > max — should clamp max to min
        let mut s = make_slider();
        s.set_range(50, -50);
        assert_eq!(s.minimum(), 50);
        assert_eq!(s.maximum(), 50, "max should be clamped to min when max < min");
        // Value should be reclamped to valid range
        assert_eq!(s.value(), 50);

        // Setting minimum above maximum directly — max follows
        let mut s2 = make_slider();
        s2.set_minimum(200);
        assert_eq!(s2.minimum(), 200);
        assert_eq!(s2.maximum(), 200, "max should be raised to match min");

        // Setting maximum below minimum directly — min follows
        let mut s3 = make_slider();
        s3.set_maximum(-100);
        assert_eq!(s3.maximum(), -100);
        assert_eq!(s3.minimum(), -100, "min should be lowered to match max");
    }

    #[test]
    fn test_slider_step_larger_than_range() {
        let mut s = make_slider();
        // Range is 0–100, set step to 200 (> range width)
        s.set_single_step(200);
        assert_eq!(s.single_step(), 200);

        // From value=5, step sub should clamp to minimum
        s.set_value(5);
        s.trigger_action(SliderAction::SliderSingleStepSub);
        // 5 - 200 = -195, clamped to 0
        assert_eq!(s.value(), 0, "step sub should clamp to minimum when step > range");

        // From value=50, step add should clamp to maximum
        s.set_value(50);
        s.trigger_action(SliderAction::SliderSingleStepAdd);
        // 50 + 200 = 250, clamped to 100
        assert_eq!(s.value(), 100, "step add should clamp to maximum when step > range");

        // Narrow range (0–1) with step=5
        let mut s2 = make_slider();
        s2.set_range(0, 1);
        s2.set_single_step(5);
        s2.trigger_action(SliderAction::SliderSingleStepSub);
        assert_eq!(s2.value(), 0);

        s2.set_value(1);
        s2.trigger_action(SliderAction::SliderSingleStepAdd);
        assert_eq!(s2.value(), 1);
    }
}
