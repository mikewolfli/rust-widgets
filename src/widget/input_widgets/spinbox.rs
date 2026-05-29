//! Spin box widget for numeric input.
use crate::core::{Color, Font, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};

use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
/// Spin box widget for integer input.
pub struct SpinBox {
    base: BaseWidget,
    value: i32,
    minimum: i32,
    maximum: i32,
    single_step: i32,
    prefix: String,
    suffix: String,
    special_value_text: Option<String>,
    wrapping: bool,
    pub value_changed: Signal1<i32>,
    pub editing_finished: GenericSignal,
}
impl SpinBox {
    /// Creates a spin box with default range 0-99.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::SpinBox, geometry, "SpinBox"),
            value: 0,
            minimum: 0,
            maximum: 99,
            single_step: 1,
            prefix: String::new(),
            suffix: String::new(),
            special_value_text: None,
            wrapping: false,
            value_changed: Signal1::new(),
            editing_finished: GenericSignal::new(),
        }
    }
    /// Returns current value.
    pub fn value(&self) -> i32 {
        self.value
    }
    /// Sets value, clamped to valid range.
    pub fn set_value(&mut self, value: i32) {
        let clamped = value.clamp(self.minimum, self.maximum);
        if self.value == clamped {
            return;
        }
        self.value = clamped;
        self.value_changed.emit(self.value);
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
    /// Returns single step value.
    pub fn single_step(&self) -> i32 {
        self.single_step
    }
    /// Sets single step value.
    pub fn set_single_step(&mut self, step: i32) {
        self.single_step = step.max(1);
    }
    /// Returns prefix text.
    pub fn prefix(&self) -> &str {
        &self.prefix
    }
    /// Sets prefix text.
    pub fn set_prefix(&mut self, prefix: String) {
        self.prefix = prefix;
    }
    /// Returns suffix text.
    pub fn suffix(&self) -> &str {
        &self.suffix
    }
    /// Sets suffix text.
    pub fn set_suffix(&mut self, suffix: String) {
        self.suffix = suffix;
    }
    /// Returns special value text.
    pub fn special_value_text(&self) -> Option<&str> {
        self.special_value_text.as_deref()
    }
    /// Sets special value text.
    pub fn set_special_value_text(&mut self, text: Option<String>) {
        self.special_value_text = text;
    }
    /// Returns whether wrapping is enabled.
    pub fn wrapping(&self) -> bool {
        self.wrapping
    }
    /// Sets wrapping state.
    pub fn set_wrapping(&mut self, wrapping: bool) {
        self.wrapping = wrapping;
    }
    /// Increments value by single step.
    pub fn step_up(&mut self) {
        let mut new_value = self.value + self.single_step;
        if new_value > self.maximum {
            if self.wrapping {
                new_value = self.minimum;
            } else {
                new_value = self.maximum;
            }
        }
        self.set_value(new_value);
    }
    /// Decrements value by single step.
    pub fn step_down(&mut self) {
        let mut new_value = self.value - self.single_step;
        if new_value < self.minimum {
            if self.wrapping {
                new_value = self.maximum;
            } else {
                new_value = self.minimum;
            }
        }
        self.set_value(new_value);
    }
    /// Returns display text.
    fn display_text(&self) -> String {
        if let Some(special) = &self.special_value_text {
            if self.value == 0 {
                return special.clone();
            }
        }
        let text = format!("{}{}{}", self.prefix, self.value, self.suffix);
        text
    }
}
// Implement Widget trait
impl Widget for SpinBox {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        // Value digits + up/down buttons (~20px)
        let val_w = format!("{}", self.value()).len() as u32 * 10 + 25;
        Size::new(val_w.max(60), 24)
    }
}
impl SpinBox {
    /// Handle click/tap on increment/decrement buttons.
    fn handle_button_click(&mut self, pos: &Point, rect: Rect, button_width: u32) {
        if pos.x as f32 >= rect.x as f32 + rect.width as f32 - button_width as f32 * 2.0 {
            if (pos.x as f32) < rect.x as f32 + rect.width as f32 - button_width as f32 {
                self.step_down();
            } else {
                self.step_up();
            }
            self.base.clicked.emit();
        }
    }
}
impl EventHandler for SpinBox {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button } => {
                let rect = self.geometry();
                let button_width = 20;
                if *button == 1 {
                    self.handle_button_click(pos, rect, button_width);
                }
            }
            #[cfg(feature = "touch")]
            Event::TouchBegin { pos, .. } => {
                let rect = self.geometry();
                let button_width = 20;
                self.handle_button_click(pos, rect, button_width);
            }
            Event::KeyPress { key, modifiers: _ } => {
                match *key {
                    38 => {
                        // Up arrow
                        self.step_up();
                    }
                    40 => {
                        // Down arrow
                        self.step_down();
                    }
                    13 => {
                        // Enter
                        self.editing_finished.emit();
                    }
                    27 => {
                        // Escape
                        self.editing_finished.emit();
                    }
                    _ => {}
                }
            }
            Event::FocusLost => {
                self.editing_finished.emit();
            }
            _ => {}
        }
    }
}
impl Draw for SpinBox {
    fn draw(&mut self, context: &mut RenderContext) {
        // Draw base widget
        let rect = self.geometry();
        let padding = 4;
        let button_width = 20;
        let text_x = rect.x + padding;
        let text_y = rect.y as f32 + rect.height as f32 / 2.0;
        // Draw background
        context.fill_rect(
            Rect::new(rect.x, rect.y, rect.width, rect.height),
            Color::from_rgb(255, 255, 255),
        );
        // Draw border
        context.draw_rect(
            Rect::new(rect.x, rect.y, rect.width, rect.height),
            Color::from_rgb(200, 200, 200),
        );
        // Draw up/down buttons
        let down_button_x_f = rect.x as f32 + rect.width as f32 - button_width as f32 * 2.0;
        let up_button_x_f = rect.x as f32 + rect.width as f32 - button_width as f32;
        let button_width_f = button_width as f32;
        let rect_y_f = rect.y as f32;
        let rect_height_f = rect.height as f32;
        // Down button
        context.fill_rect(
            Rect::from_f32(down_button_x_f, rect_y_f, button_width_f, rect_height_f),
            Color::from_rgb(240, 240, 240),
        );
        context.draw_rect(
            Rect::from_f32(down_button_x_f, rect_y_f, button_width_f, rect_height_f),
            Color::from_rgb(200, 200, 200),
        );
        // Down arrow
        let down_arrow_x_f = down_button_x_f + button_width_f / 2.0;
        let down_arrow_y_f = rect_y_f + rect_height_f / 2.0;
        let arrow_size = 4;
        let arrow_size_f = arrow_size as f32;
        context.draw_line(
            Point::from_f32(
                down_arrow_x_f - arrow_size_f,
                down_arrow_y_f - arrow_size_f / 2.0,
            ),
            Point::from_f32(
                down_arrow_x_f + arrow_size_f,
                down_arrow_y_f - arrow_size_f / 2.0,
            ),
            Color::from_rgb(100, 100, 100),
        );
        context.draw_line(
            Point::from_f32(
                down_arrow_x_f + arrow_size_f,
                down_arrow_y_f - arrow_size_f / 2.0,
            ),
            Point::from_f32(down_arrow_x_f, down_arrow_y_f + arrow_size_f / 2.0),
            Color::from_rgb(100, 100, 100),
        );
        context.draw_line(
            Point::from_f32(down_arrow_x_f, down_arrow_y_f + arrow_size_f / 2.0),
            Point::from_f32(
                down_arrow_x_f - arrow_size_f,
                down_arrow_y_f - arrow_size_f / 2.0,
            ),
            Color::from_rgb(100, 100, 100),
        );
        // Up button
        context.fill_rect(
            Rect::from_f32(up_button_x_f, rect_y_f, button_width_f, rect_height_f),
            Color::from_rgb(240, 240, 240),
        );
        context.draw_rect(
            Rect::from_f32(up_button_x_f, rect_y_f, button_width_f, rect_height_f),
            Color::from_rgb(200, 200, 200),
        );
        // Up arrow
        let up_arrow_x_f = up_button_x_f + button_width_f / 2.0;
        let up_arrow_y_f = rect_y_f + rect_height_f / 2.0;
        context.draw_line(
            Point::from_f32(
                up_arrow_x_f - arrow_size_f,
                up_arrow_y_f + arrow_size_f / 2.0,
            ),
            Point::from_f32(
                up_arrow_x_f + arrow_size_f,
                up_arrow_y_f + arrow_size_f / 2.0,
            ),
            Color::from_rgb(100, 100, 100),
        );
        context.draw_line(
            Point::from_f32(
                up_arrow_x_f + arrow_size_f,
                up_arrow_y_f + arrow_size_f / 2.0,
            ),
            Point::from_f32(up_arrow_x_f, up_arrow_y_f - arrow_size_f / 2.0),
            Color::from_rgb(100, 100, 100),
        );
        context.draw_line(
            Point::from_f32(up_arrow_x_f, up_arrow_y_f - arrow_size_f / 2.0),
            Point::from_f32(
                up_arrow_x_f - arrow_size_f,
                up_arrow_y_f + arrow_size_f / 2.0,
            ),
            Color::from_rgb(100, 100, 100),
        );
        // Draw text
        let display_text = self.display_text();
        if !display_text.is_empty() {
            context.draw_text(
                Point::new(text_x, text_y as i32),
                &display_text,
                &Font::default(),
                Color::from_rgb(0, 0, 0),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Rect;

    #[test]
    fn spinbox_creation_defaults() {
        let sb = SpinBox::new(Rect::new(0, 0, 100, 24));
        assert_eq!(sb.value(), 0);
        assert_eq!(sb.minimum(), 0);
        assert_eq!(sb.maximum(), 99);
        assert_eq!(sb.single_step(), 1);
        assert!(sb.prefix().is_empty());
        assert!(sb.suffix().is_empty());
        assert!(!sb.wrapping());
        assert!(sb.special_value_text().is_none());
    }

    #[test]
    fn spinbox_set_value() {
        let mut sb = SpinBox::new(Rect::new(0, 0, 100, 24));
        sb.set_value(50);
        assert_eq!(sb.value(), 50);
        sb.set_value(200); // clamp to max
        assert_eq!(sb.value(), 99);
        sb.set_value(-10); // clamp to min
        assert_eq!(sb.value(), 0);
    }

    #[test]
    fn spinbox_set_range() {
        let mut sb = SpinBox::new(Rect::new(0, 0, 100, 24));
        sb.set_minimum(-50);
        sb.set_maximum(200);
        assert_eq!(sb.minimum(), -50);
        assert_eq!(sb.maximum(), 200);
    }

    #[test]
    fn spinbox_set_range_reclamps_value() {
        let mut sb = SpinBox::new(Rect::new(0, 0, 100, 24));
        sb.set_value(50);
        sb.set_range(60, 100);
        assert_eq!(sb.value(), 60);
    }

    #[test]
    fn spinbox_prefix_suffix() {
        let mut sb = SpinBox::new(Rect::new(0, 0, 100, 24));
        sb.set_prefix("$".to_string());
        assert_eq!(sb.prefix(), "$");
        sb.set_suffix(" USD".to_string());
        assert_eq!(sb.suffix(), " USD");
        sb.set_prefix(String::new());
        assert!(sb.prefix().is_empty());
    }

    #[test]
    fn spinbox_single_step() {
        let mut sb = SpinBox::new(Rect::new(0, 0, 100, 24));
        sb.set_single_step(5);
        assert_eq!(sb.single_step(), 5);
        sb.set_single_step(0); // floors at 1
        assert_eq!(sb.single_step(), 1);
    }

    #[test]
    fn spinbox_wrapping() {
        let mut sb = SpinBox::new(Rect::new(0, 0, 100, 24));
        assert!(!sb.wrapping());
        sb.set_wrapping(true);
        assert!(sb.wrapping());
        sb.set_wrapping(false);
        assert!(!sb.wrapping());
    }

    #[test]
    fn spinbox_step_up() {
        let mut sb = SpinBox::new(Rect::new(0, 0, 100, 24));
        sb.set_value(50);
        sb.step_up();
        assert_eq!(sb.value(), 51);
    }

    #[test]
    fn spinbox_step_up_clamps() {
        let mut sb = SpinBox::new(Rect::new(0, 0, 100, 24));
        sb.set_value(99);
        sb.step_up();
        assert_eq!(sb.value(), 99); // clamped to max
    }

    #[test]
    fn spinbox_step_up_wraps() {
        let mut sb = SpinBox::new(Rect::new(0, 0, 100, 24));
        sb.set_wrapping(true);
        sb.set_value(99);
        sb.step_up();
        assert_eq!(sb.value(), 0); // wraps to min
    }

    #[test]
    fn spinbox_step_down() {
        let mut sb = SpinBox::new(Rect::new(0, 0, 100, 24));
        sb.set_value(50);
        sb.step_down();
        assert_eq!(sb.value(), 49);
    }

    #[test]
    fn spinbox_step_down_clamps() {
        let mut sb = SpinBox::new(Rect::new(0, 0, 100, 24));
        sb.set_value(0);
        sb.step_down();
        assert_eq!(sb.value(), 0); // clamped to min
    }

    #[test]
    fn spinbox_step_down_wraps() {
        let mut sb = SpinBox::new(Rect::new(0, 0, 100, 24));
        sb.set_wrapping(true);
        sb.set_value(0);
        sb.step_down();
        assert_eq!(sb.value(), 99); // wraps to max
    }

    #[test]
    fn spinbox_special_value_text() {
        let mut sb = SpinBox::new(Rect::new(0, 0, 100, 24));
        assert!(sb.special_value_text().is_none());
        sb.set_special_value_text(Some("Zero".to_string()));
        assert_eq!(sb.special_value_text(), Some("Zero"));
        sb.set_special_value_text(None);
        assert!(sb.special_value_text().is_none());
    }

    #[test]
    fn spinbox_geometry_delegation() {
        let mut sb = SpinBox::new(Rect::new(0, 0, 100, 24));
        sb.set_geometry(Rect::new(10, 10, 200, 30));
        assert_eq!(sb.geometry(), Rect::new(10, 10, 200, 30));
    }

    #[test]
    fn spinbox_visibility() {
        let mut sb = SpinBox::new(Rect::new(0, 0, 100, 24));
        assert!(sb.is_visible());
        sb.hide();
        assert!(!sb.is_visible());
        sb.show();
        assert!(sb.is_visible());
    }

    #[test]
    fn spinbox_enabled() {
        let mut sb = SpinBox::new(Rect::new(0, 0, 100, 24));
        assert!(sb.is_enabled());
        sb.set_enabled(false);
        assert!(!sb.is_enabled());
        sb.set_enabled(true);
        assert!(sb.is_enabled());
    }

    #[test]
    fn spinbox_id_kind() {
        let sb_a = SpinBox::new(Rect::new(0, 0, 100, 24));
        let sb_b = SpinBox::new(Rect::new(0, 0, 100, 24));
        assert_ne!(sb_a.id(), sb_b.id());
        assert_eq!(sb_a.kind(), WidgetKind::SpinBox);
        assert_eq!(sb_b.kind(), WidgetKind::SpinBox);
    }

    #[test]
    fn spinbox_signal_accessors() {
        let sb = SpinBox::new(Rect::new(0, 0, 100, 24));
        let _value_changed = &sb.value_changed;
        let _editing_finished = &sb.editing_finished;
    }
}
