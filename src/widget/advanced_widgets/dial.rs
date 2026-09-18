// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Dial (knob) widget.
use crate::core::{Color, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::capability::coercion::{expect_bool, expect_f64, expect_i64};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
/// Dial (rotary knob) widget.
///
/// Holds an integer value in an inclusive `minimum ..= maximum` range and
/// renders it as a needle on a circle. The widget has no drag handling of its
/// own: it changes value in response to keyboard events and to explicit
/// [`Dial::set_value`] calls from the caller.
///
pub struct Dial {
    base: BaseWidget,
    minimum: i32,
    maximum: i32,
    value: i32,
    single_step: i32,
    page_step: i32,
    notches_visible: bool,
    notch_target: f64,
    wrapping: bool,
    /// Emitted with the new value whenever [`Dial::set_value`] actually changes
    /// it. Redundant sets do not fire it.
    pub value_changed: Signal1<i32>,
    /// Emitted when the primary mouse button is pressed while the dial is
    /// enabled. Purely a notification — the press does not change the value.
    pub slider_pressed: GenericSignal,
    /// Emitted when the primary mouse button is released while the dial is
    /// enabled. Like `slider_pressed`, it does not change the value.
    pub slider_released: GenericSignal,
}
impl Dial {
    /// Creates a dial ranging over `0 ..= 99` with value `0`, a single step of
    /// `1`, a page step of `10`, notches hidden, wrapping off, and a notch
    /// target of `3.7`.
    ///
    /// `geometry` is in parent-relative logical pixels; the size hint is 64x64.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Dial, geometry, "Dial"),
            minimum: 0,
            maximum: 99,
            value: 0,
            single_step: 1,
            page_step: 10,
            notches_visible: false,
            notch_target: 3.7,
            wrapping: false,
            value_changed: Signal1::new(),
            slider_pressed: GenericSignal::new(),
            slider_released: GenericSignal::new(),
        }
    }
    /// Returns the inclusive lower bound. Defaults to `0`.
    pub fn minimum(&self) -> i32 {
        self.minimum
    }
    /// Returns the inclusive upper bound. Defaults to `99`.
    pub fn maximum(&self) -> i32 {
        self.maximum
    }
    /// Returns the current value, always inside the configured range (modulo
    /// wrapping, which is still range-mapped).
    pub fn value(&self) -> i32 {
        self.value
    }
    /// Returns the increment applied by one arrow-key press (in value units).
    /// Always at least `1`; defaults to `1`.
    pub fn single_step(&self) -> i32 {
        self.single_step
    }
    /// Returns the increment applied by Page Up / Page Down (in value units).
    /// Always at least `1`; defaults to `10`.
    pub fn page_step(&self) -> i32 {
        self.page_step
    }
    /// Returns whether notches are drawn. Defaults to `false`.
    ///
    /// Note that [`Dial::draw`] currently paints only the body and needle, so
    /// this flag has no visible effect yet.
    pub fn notches_visible(&self) -> bool {
        self.notches_visible
    }
    /// Returns the notch target angle, in **degrees**, used by the notch
    /// geometry. Defaults to `3.7`.
    ///
    /// The value is stored verbatim and currently not consumed by rendering.
    pub fn notch_target(&self) -> f64 {
        self.notch_target
    }
    /// Returns whether the value wraps around the range instead of clamping.
    /// Defaults to `false`.
    pub fn wrapping(&self) -> bool {
        self.wrapping
    }
    /// Sets the lower bound and re-applies it to the current value through
    /// [`Dial::set_value`], so the value will be clamped or wrapped into the
    /// new range and `value_changed` may fire.
    ///
    /// `min` above the current `maximum` leaves an inverted range in which the
    /// clamp saturates unpredictably; use [`Dial::set_range`] instead, which
    /// keeps `maximum >= minimum`.
    pub fn set_minimum(&mut self, min: i32) {
        self.minimum = min;
        self.set_value(self.value);
        self.base.request_redraw();
    }
    /// Sets the upper bound and re-applies it to the current value through
    /// [`Dial::set_value`]. See [`Dial::set_minimum`] for range caveats.
    pub fn set_maximum(&mut self, max: i32) {
        self.maximum = max;
        self.set_value(self.value);
        self.base.request_redraw();
    }
    /// Sets both minimum and maximum in one call, raising `maximum` to `min`
    /// if it is lower, so the range is never inverted. The current value is
    /// then re-applied through [`Dial::set_value`].
    ///
    /// This is a convenience writer; query bounds via `minimum()` and `maximum()`.
    pub fn set_range(&mut self, min: i32, max: i32) {
        self.minimum = min;
        self.maximum = max.max(min);
        self.set_value(self.value);
        self.base.request_redraw();
    }
    /// Sets the value, clamping into `minimum ..= maximum` — or wrapping
    /// modulo the range when [`Dial::wrapping`] is on, in which case the value
    /// is mapped back into the range rather than rejected.
    ///
    /// A no-op when the resulting value equals the current one: no signal and
    /// no redraw.
    pub fn set_value(&mut self, value: i32) {
        let clamped = if self.wrapping {
            let range = self.maximum - self.minimum + 1;
            if range <= 0 {
                self.minimum
            } else {
                (value - self.minimum).rem_euclid(range) + self.minimum
            }
        } else {
            value.clamp(self.minimum, self.maximum)
        };
        if self.value != clamped {
            self.value = clamped;
            self.value_changed.emit(clamped);
            self.base.request_redraw();
        }
    }
    /// Sets the arrow-key increment, floored at `1` so the value can always
    /// move. Requests a redraw.
    pub fn set_single_step(&mut self, step: i32) {
        self.single_step = step.max(1);
        self.base.request_redraw();
    }
    /// Sets the Page Up / Page Down increment, floored at `1`. Requests a
    /// redraw.
    pub fn set_page_step(&mut self, step: i32) {
        self.page_step = step.max(1);
        self.base.request_redraw();
    }
    /// Toggles notch rendering. See [`Dial::notches_visible`] — currently has
    /// no visual effect. Requests a redraw.
    pub fn set_notches_visible(&mut self, visible: bool) {
        self.notches_visible = visible;
        self.base.request_redraw();
    }
    /// Sets the notch target in degrees, stored verbatim. See
    /// [`Dial::notch_target`]. Requests a redraw.
    pub fn set_notch_target(&mut self, target: f64) {
        self.notch_target = target;
        self.base.request_redraw();
    }
    /// Enables or disables wrap-around behaviour.
    ///
    /// Takes effect on the *next* call to [`Dial::set_value`]; the current
    /// value is not re-mapped. Turning wrapping off therefore leaves a value
    /// that was produced by wrapping in place, which is fine because wrapped
    /// values are always inside the range. Requests a redraw.
    pub fn set_wrapping(&mut self, wrapping: bool) {
        self.wrapping = wrapping;
        self.base.request_redraw();
    }
    /// Returns value as angle in radians (from -135° to +135°, or full circle if wrapping).
    fn value_angle(&self) -> f64 {
        let range = (self.maximum - self.minimum) as f64;
        if range == 0.0 {
            return -std::f64::consts::PI * 0.75;
        }
        let ratio = (self.value - self.minimum) as f64 / range;
        if self.wrapping {
            ratio * 2.0 * std::f64::consts::PI - std::f64::consts::PI
        } else {
            -std::f64::consts::PI * 0.75 + ratio * std::f64::consts::PI * 1.5
        }
    }
}
impl Widget for Dial {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        Size::new(64, 64)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `Dial`'s property contract.
impl WidgetProperties for Dial {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "minimum" => Ok(CapabilityValue::Int(self.minimum() as i64)),
            "maximum" => Ok(CapabilityValue::Int(self.maximum() as i64)),
            "value" => Ok(CapabilityValue::Int(self.value() as i64)),
            "single_step" => Ok(CapabilityValue::Int(self.single_step() as i64)),
            "page_step" => Ok(CapabilityValue::Int(self.page_step() as i64)),
            "notches_visible" => Ok(CapabilityValue::Bool(self.notches_visible())),
            "notch_target" => Ok(CapabilityValue::Float(self.notch_target())),
            "wrapping" => Ok(CapabilityValue::Bool(self.wrapping())),
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
            "notches_visible" => {
                self.set_notches_visible(expect_bool(value)?);
                Ok(())
            }
            "notch_target" => {
                self.set_notch_target(expect_f64(value)?);
                Ok(())
            }
            "wrapping" => {
                self.set_wrapping(expect_bool(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Mirrors `DIAL_PROPERTIES`.
        property_names_of![
            "minimum",
            "maximum",
            "value",
            "single_step",
            "page_step",
            "notches_visible",
            "notch_target",
            "wrapping",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl EventHandler for Dial {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { button, .. } if *button == 1 => {
                self.slider_pressed.emit();
            }
            Event::MouseRelease { button, .. } if *button == 1 => {
                self.slider_released.emit();
            }
            Event::KeyPress { key, .. } => match *key {
                37 | 40 => self.set_value(self.value - self.single_step), // Left/Down
                38 | 39 => self.set_value(self.value + self.single_step), // Up/Right
                33 => self.set_value(self.value - self.page_step),
                34 => self.set_value(self.value + self.page_step),
                36 => self.set_value(self.minimum),
                35 => self.set_value(self.maximum),
                // Unknown key; ignore
                _ => {}
            },
            // Other events are not relevant for this widget
            _ => {}
        }
    }
}
impl Draw for Dial {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let center = Point {
            x: rect.x + rect.width as f32 as i32 / 2,
            y: rect.y + rect.height as f32 as i32 / 2,
        };
        let radius = (rect.width.min(rect.height) / 2).saturating_sub(4);
        context.fill_circle(center, radius, Color::rgb(230, 230, 230));
        context.draw_circle(center, radius, Color::rgb(150, 150, 150));
        // Draw a simple value needle.
        let angle = self.value_angle();
        let needle_len = (radius as f32 * 0.7) as i32;
        let to = Point {
            x: center.x + (needle_len as f32 * angle.cos() as f32) as i32,
            y: center.y + (needle_len as f32 * angle.sin() as f32) as i32,
        };
        context.draw_line(center, to, Color::rgb(0, 0, 0));
        context.fill_circle(center, 3, Color::rgb(80, 80, 80));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Rect;

    #[test]
    fn dial_creation_defaults() {
        let d = Dial::new(Rect::new(0, 0, 64, 64));
        assert_eq!(d.minimum(), 0);
        assert_eq!(d.maximum(), 99);
        assert_eq!(d.value(), 0);
        assert_eq!(d.single_step(), 1);
        assert_eq!(d.page_step(), 10);
        assert!(!d.notches_visible());
        assert!(!d.wrapping());
    }

    #[test]
    fn dial_set_value_clamps() {
        let mut d = Dial::new(Rect::new(0, 0, 64, 64));
        d.set_value(50);
        assert_eq!(d.value(), 50);
        d.set_value(200);
        assert_eq!(d.value(), 99);
        d.set_value(-10);
        assert_eq!(d.value(), 0);
    }

    #[test]
    fn dial_set_range_reclamps_value() {
        let mut d = Dial::new(Rect::new(0, 0, 64, 64));
        d.set_value(50);
        d.set_range(60, 80);
        assert_eq!(d.value(), 60);
        assert_eq!(d.minimum(), 60);
        assert_eq!(d.maximum(), 80);
    }

    #[test]
    fn dial_wrapping() {
        let mut d = Dial::new(Rect::new(0, 0, 64, 64));
        d.set_range(0, 9);
        d.set_wrapping(true);
        assert!(d.wrapping());
        d.set_value(9);
        assert_eq!(d.value(), 9);
        d.set_value(10);
        // wrapping: 10 % 10 = 0
        assert_eq!(d.value(), 0);
    }

    #[test]
    fn dial_steps() {
        let mut d = Dial::new(Rect::new(0, 0, 64, 64));
        d.set_single_step(5);
        assert_eq!(d.single_step(), 5);
        d.set_single_step(0);
        assert_eq!(d.single_step(), 1); // floors at 1
        d.set_page_step(25);
        assert_eq!(d.page_step(), 25);
        d.set_page_step(0);
        assert_eq!(d.page_step(), 1); // floors at 1
    }

    #[test]
    fn dial_notches_visible() {
        let mut d = Dial::new(Rect::new(0, 0, 64, 64));
        assert!(!d.notches_visible());
        d.set_notches_visible(true);
        assert!(d.notches_visible());
        d.set_notches_visible(false);
        assert!(!d.notches_visible());
    }

    #[test]
    fn dial_notch_target() {
        let mut d = Dial::new(Rect::new(0, 0, 64, 64));
        assert!((d.notch_target() - 3.7).abs() < 1e-9);
        d.set_notch_target(5.0);
        assert!((d.notch_target() - 5.0).abs() < 1e-9);
    }

    #[test]
    fn dial_keyboard_navigation() {
        let mut d = Dial::new(Rect::new(0, 0, 64, 64));
        d.set_value(50);
        // Left arrow (key 37) decreases by single step
        d.handle_event(&Event::KeyPress { key: 37, modifiers: 0 });
        assert_eq!(d.value(), 49);
        // Right arrow (key 39) increases by single step
        d.handle_event(&Event::KeyPress { key: 39, modifiers: 0 });
        assert_eq!(d.value(), 50);
        // PageUp (key 33) decreases by page step
        d.handle_event(&Event::KeyPress { key: 33, modifiers: 0 });
        assert_eq!(d.value(), 40);
        // PageDown (key 34) increases by page step
        d.handle_event(&Event::KeyPress { key: 34, modifiers: 0 });
        assert_eq!(d.value(), 50);
        // Home (key 36) goes to min
        d.handle_event(&Event::KeyPress { key: 36, modifiers: 0 });
        assert_eq!(d.value(), 0);
        // End (key 35) goes to max
        d.handle_event(&Event::KeyPress { key: 35, modifiers: 0 });
        assert_eq!(d.value(), 99);
    }

    #[test]
    fn dial_mouse_events() {
        let mut d = Dial::new(Rect::new(0, 0, 64, 64));
        d.handle_event(&Event::MousePress { pos: Point::new(32, 32), button: 1 });
        // value should not change; only signal emitted
        assert_eq!(d.value(), 0);
        d.handle_event(&Event::MouseRelease { pos: Point::new(32, 32), button: 1 });
        assert_eq!(d.value(), 0);
    }

    #[test]
    fn dial_signal_accessors() {
        let d = Dial::new(Rect::new(0, 0, 64, 64));
        let _ = &d.value_changed;
        let _ = &d.slider_pressed;
        let _ = &d.slider_released;
    }

    #[test]
    fn dial_geometry_delegation() {
        let mut d = Dial::new(Rect::new(0, 0, 64, 64));
        d.set_geometry(Rect::new(10, 10, 80, 80));
        assert_eq!(d.geometry(), Rect::new(10, 10, 80, 80));
    }

    #[test]
    fn dial_disabled_blocks_events() {
        let mut d = Dial::new(Rect::new(0, 0, 64, 64));
        d.set_value(50);
        d.set_enabled(false);
        d.handle_event(&Event::KeyPress { key: 39, modifiers: 0 });
        // Should stay unchanged because disabled
        assert_eq!(d.value(), 50);
    }
}
