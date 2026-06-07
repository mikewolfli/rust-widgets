//! Dial (knob) widget.
use crate::core::{Color, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
/// Dial (rotary knob) widget.
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
    pub value_changed: Signal1<i32>,
    pub slider_moved: Signal1<i32>,
    pub slider_pressed: GenericSignal,
    pub slider_released: GenericSignal,
}
impl Dial {
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
            slider_moved: Signal1::new(),
            slider_pressed: GenericSignal::new(),
            slider_released: GenericSignal::new(),
        }
    }
    pub fn minimum(&self) -> i32 {
        self.minimum
    }
    pub fn maximum(&self) -> i32 {
        self.maximum
    }
    pub fn value(&self) -> i32 {
        self.value
    }
    pub fn single_step(&self) -> i32 {
        self.single_step
    }
    pub fn page_step(&self) -> i32 {
        self.page_step
    }
    pub fn notches_visible(&self) -> bool {
        self.notches_visible
    }
    pub fn notch_target(&self) -> f64 {
        self.notch_target
    }
    pub fn wrapping(&self) -> bool {
        self.wrapping
    }
    pub fn set_minimum(&mut self, min: i32) {
        self.minimum = min;
        self.set_value(self.value);
    }
    pub fn set_maximum(&mut self, max: i32) {
        self.maximum = max;
        self.set_value(self.value);
    }
    /// Sets both minimum and maximum in one call.
    /// This is a convenience writer; query bounds via `minimum()` and `maximum()`.
    pub fn set_range(&mut self, min: i32, max: i32) {
        self.minimum = min;
        self.maximum = max.max(min);
        self.set_value(self.value);
    }
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
        }
    }
    pub fn set_single_step(&mut self, step: i32) {
        self.single_step = step.max(1);
    }
    pub fn set_page_step(&mut self, step: i32) {
        self.page_step = step.max(1);
    }
    pub fn set_notches_visible(&mut self, visible: bool) {
        self.notches_visible = visible;
    }
    pub fn set_notch_target(&mut self, target: f64) {
        self.notch_target = target;
    }
    pub fn set_wrapping(&mut self, wrapping: bool) {
        self.wrapping = wrapping;
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
                _ => {}
            },
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
        context.fill_circle(center, radius, Color::from_rgb(230, 230, 230));
        context.draw_circle(center, radius, Color::from_rgb(150, 150, 150));
        // Draw a simple value needle.
        let angle = self.value_angle();
        let needle_len = (radius as f32 * 0.7) as i32;
        let to = Point {
            x: center.x + (needle_len as f32 * angle.cos() as f32) as i32,
            y: center.y + (needle_len as f32 * angle.sin() as f32) as i32,
        };
        context.draw_line(center, to, Color::from_rgb(0, 0, 0));
        context.fill_circle(center, 3, Color::from_rgb(80, 80, 80));
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
        let _ = &d.slider_moved;
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
