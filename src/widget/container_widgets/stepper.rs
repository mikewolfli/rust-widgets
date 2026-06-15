//! Stepper widget — a numeric increment/decrement control with +/- buttons.
//!
//! The Stepper widget displays a numeric value with minus (-) and plus (+)
//! buttons on either side for incrementing or decrementing the value.
//! It supports configurable minimum, maximum, step size, and emits a
//! `value_changed` signal whenever the value changes.

use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Stepper widget for numeric increment/decrement with +/- buttons.
pub struct Stepper {
    base: BaseWidget,
    value: i32,
    min: i32,
    max: i32,
    step: i32,
    /// Emitted when the value changes.
    pub value_changed: Signal1<i32>,
}

impl Stepper {
    /// Creates a new Stepper widget with the given geometry.
    /// Default value is 0, min=0, max=100, step=1.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Stepper, geometry, "Stepper"),
            value: 0,
            min: 0,
            max: 100,
            step: 1,
            value_changed: Signal1::new(),
        }
    }

    /// Sets the current value, clamped to [min, max].
    /// Emits `value_changed` signal if the value actually changes.
    pub fn set_value(&mut self, value: i32) {
        let clamped = value.clamp(self.min, self.max);
        if self.value != clamped {
            self.value = clamped;
            self.value_changed.emit(clamped);
            self.base.request_redraw();
        }
    }

    /// Returns the current value.
    pub fn value(&self) -> i32 {
        self.value
    }

    /// Sets the minimum value (inclusive).
    pub fn set_min(&mut self, min: i32) {
        self.min = min.min(self.max);
        // Re-clamp current value to new bounds
        self.set_value(self.value);
    }

    /// Sets the maximum value (inclusive).
    pub fn set_max(&mut self, max: i32) {
        self.max = max.max(self.min);
        // Re-clamp current value to new bounds
        self.set_value(self.value);
    }

    /// Sets the step increment/decrement amount.
    pub fn set_step(&mut self, step: i32) {
        self.step = step.max(1);
    }

    /// Increments the value by the step amount, clamped to max.
    pub fn increment(&mut self) {
        self.set_value(self.value.saturating_add(self.step));
    }

    /// Decrements the value by the step amount, clamped to min.
    pub fn decrement(&mut self) {
        self.set_value(self.value.saturating_sub(self.step));
    }
}

impl Widget for Stepper {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}

impl Draw for Stepper {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let is_enabled = self.base.is_enabled();
        let btn_width = rect.height.min(rect.width / 3).max(20);
        let value_text = self.value.to_string();
        let font = crate::core::Font::default_ui();
        let text_metrics = context.measure_text(&value_text, &font);

        // Background
        let bg_color = if !is_enabled {
            Color::rgba(230, 230, 230, 200)
        } else {
            Color::rgba(240, 240, 240, 255)
        };
        context.fill_rounded_rect(rect, 4, bg_color);
        context.draw_rounded_rect_stroke(rect, 4, Color::rgba(180, 180, 180, 200), 1);

        // --- Minus button (left) ---
        let minus_rect = Rect::new(rect.x + 1, rect.y + 1, btn_width, rect.height - 2);
        let minus_color = if !is_enabled {
            Color::rgba(200, 200, 200, 200)
        } else {
            Color::rgba(220, 220, 220, 255)
        };
        context.fill_rounded_rect(minus_rect, 3, minus_color);
        context.draw_rounded_rect_stroke(minus_rect, 3, Color::rgba(160, 160, 160, 200), 1);
        // Draw "-" symbol centered in the minus button
        let minus_label = "\u{2212}";
        let minus_font = crate::core::Font::bold("Arial", 16.0);
        let minus_metrics = context.measure_text(minus_label, &minus_font);
        let minus_x = minus_rect.x + (minus_rect.width as i32 - minus_metrics.width as i32) / 2;
        let minus_y = minus_rect.y + (minus_rect.height as i32 + minus_metrics.height as i32) / 2
            - minus_metrics.descent as i32;
        context.draw_text(
            Point::new(minus_x, minus_y),
            minus_label,
            &minus_font,
            if !is_enabled {
                Color::rgba(150, 150, 150, 200)
            } else {
                Color::rgba(60, 60, 60, 255)
            },
            HorizontalAlignment::Left,
        );

        // --- Plus button (right) ---
        let plus_rect = Rect::new(
            rect.x + rect.width as i32 - btn_width as i32 - 1,
            rect.y + 1,
            btn_width,
            rect.height - 2,
        );
        let plus_color = if !is_enabled {
            Color::rgba(200, 200, 200, 200)
        } else {
            Color::rgba(220, 220, 220, 255)
        };
        context.fill_rounded_rect(plus_rect, 3, plus_color);
        context.draw_rounded_rect_stroke(plus_rect, 3, Color::rgba(160, 160, 160, 200), 1);
        // Draw "+" symbol centered in the plus button
        let plus_label = "+";
        let plus_font = crate::core::Font::bold("Arial", 16.0);
        let plus_metrics = context.measure_text(plus_label, &plus_font);
        let plus_x = plus_rect.x + (plus_rect.width as i32 - plus_metrics.width as i32) / 2;
        let plus_y = plus_rect.y + (plus_rect.height as i32 + plus_metrics.height as i32) / 2
            - plus_metrics.descent as i32;
        context.draw_text(
            Point::new(plus_x, plus_y),
            plus_label,
            &plus_font,
            if !is_enabled {
                Color::rgba(150, 150, 150, 200)
            } else {
                Color::rgba(60, 60, 60, 255)
            },
            HorizontalAlignment::Left,
        );

        // --- Value text (center) ---
        let text_x = rect.x + (rect.width as i32 - text_metrics.width as i32) / 2;
        let text_y = rect.y + (rect.height as i32 + text_metrics.height as i32) / 2
            - text_metrics.descent as i32;
        let text_color = if !is_enabled {
            Color::rgba(150, 150, 150, 200)
        } else {
            Color::rgba(30, 30, 30, 255)
        };
        context.draw_text(
            Point::new(text_x, text_y),
            &value_text,
            &font,
            text_color,
            HorizontalAlignment::Left,
        );
    }
}

impl EventHandler for Stepper {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button } => {
                if *button != 1 {
                    return;
                }
                let rect = self.geometry();
                let btn_width = rect.height.min(rect.width / 3).max(20);

                // Minus button area (left)
                let minus_rect = Rect::new(rect.x + 1, rect.y + 1, btn_width, rect.height - 2);
                // Plus button area (right)
                let plus_rect = Rect::new(
                    rect.x + rect.width as i32 - btn_width as i32 - 1,
                    rect.y + 1,
                    btn_width,
                    rect.height - 2,
                );

                if minus_rect.contains(*pos) {
                    self.decrement();
                } else if plus_rect.contains(*pos) {
                    self.increment();
                }
            }
            _ => {
                self.base.handle_event(event);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Point;
    use std::sync::{Arc, Mutex};

    #[test]
    fn stepper_default_values() {
        let s = Stepper::new(Rect::new(0, 0, 120, 30));
        assert_eq!(s.value(), 0);
        assert_eq!(s.kind(), WidgetKind::Stepper);
    }

    #[test]
    fn stepper_set_value_clamps_to_min() {
        let mut s = Stepper::new(Rect::new(0, 0, 120, 30));
        s.set_value(-10);
        assert_eq!(s.value(), 0); // clamped to min=0
    }

    #[test]
    fn stepper_set_value_clamps_to_max() {
        let mut s = Stepper::new(Rect::new(0, 0, 120, 30));
        s.set_value(200);
        assert_eq!(s.value(), 100); // clamped to max=100
    }

    #[test]
    fn stepper_set_value_emits_signal() {
        let mut s = Stepper::new(Rect::new(0, 0, 120, 30));
        let captured = Arc::new(Mutex::new(None));
        s.value_changed.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<i32>| {
                *captured.lock().unwrap() = Some(*val);
            }
        });

        s.set_value(42);
        assert_eq!(s.value(), 42);
        assert_eq!(*captured.lock().unwrap(), Some(42));
    }

    #[test]
    fn stepper_increment_decrement() {
        let mut s = Stepper::new(Rect::new(0, 0, 120, 30));
        s.increment();
        assert_eq!(s.value(), 1);
        s.increment();
        assert_eq!(s.value(), 2);
        s.decrement();
        assert_eq!(s.value(), 1);
    }

    #[test]
    fn stepper_increment_clamped_to_max() {
        let mut s = Stepper::new(Rect::new(0, 0, 120, 30));
        s.set_value(100);
        s.increment();
        assert_eq!(s.value(), 100);
    }

    #[test]
    fn stepper_decrement_clamped_to_min() {
        let mut s = Stepper::new(Rect::new(0, 0, 120, 30));
        s.set_value(0);
        s.decrement();
        assert_eq!(s.value(), 0);
    }

    #[test]
    fn stepper_set_min_max() {
        let mut s = Stepper::new(Rect::new(0, 0, 120, 30));
        s.set_min(10);
        s.set_max(50);
        // Current value should be re-clamped
        assert_eq!(s.value(), 10);
        s.set_value(30);
        assert_eq!(s.value(), 30);
        s.set_value(5);
        assert_eq!(s.value(), 10);
        s.set_value(100);
        assert_eq!(s.value(), 50);
    }

    #[test]
    fn stepper_set_step() {
        let mut s = Stepper::new(Rect::new(0, 0, 120, 30));
        s.set_step(5);
        s.increment();
        assert_eq!(s.value(), 5);
        s.increment();
        assert_eq!(s.value(), 10);
        s.decrement();
        assert_eq!(s.value(), 5);
    }

    #[test]
    fn stepper_mouse_press_minus_decrements() {
        let mut s = Stepper::new(Rect::new(0, 0, 120, 30));
        // Set value to 5 first, then click in minus area (left side)
        s.set_value(5);
        // The minus button is btn_width wide, starting at x=0
        s.handle_event(&Event::MousePress { pos: Point::new(2, 15), button: 1 });
        assert_eq!(s.value(), 4);
    }

    #[test]
    fn stepper_mouse_press_plus_increments() {
        let mut s = Stepper::new(Rect::new(0, 0, 120, 30));
        // The plus button is btn_width wide on the right side
        // btn_width = min(30, 120/3).max(20) = min(30, 40).max(20) = 30
        // plus_rect starts at x = 0 + 120 - 30 - 1 = 89
        s.handle_event(&Event::MousePress { pos: Point::new(100, 15), button: 1 });
        assert_eq!(s.value(), 1);
    }

    #[test]
    fn stepper_disabled_blocks_events() {
        let mut s = Stepper::new(Rect::new(0, 0, 120, 30));
        s.set_enabled(false);
        s.handle_event(&Event::MousePress { pos: Point::new(100, 15), button: 1 });
        assert_eq!(s.value(), 0);
    }

    #[test]
    fn stepper_svg_output() {
        let mut s = Stepper::new(Rect::new(0, 0, 120, 30));
        let svg = crate::widget::svg::render_to_svg(&mut s);
        assert!(svg.starts_with("<svg"));
    }
}
