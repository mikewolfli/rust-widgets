//! Scroll bar widget.
use crate::core::{Color, Orientation, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
/// Scroll bar widget.
pub struct ScrollBar {
    base: BaseWidget,
    minimum: i32,
    maximum: i32,
    value: i32,
    single_step: i32,
    page_step: i32,
    orientation: Orientation,
    pub value_changed: Signal1<i32>,
    pub slider_moved: Signal1<i32>,
    pub slider_pressed: GenericSignal,
    pub slider_released: GenericSignal,
    mouse_pressed: bool,
}
impl ScrollBar {
    /// Creates a scroll bar with default range 0-100.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ScrollBar, geometry, "ScrollBar"),
            minimum: 0,
            maximum: 100,
            value: 0,
            single_step: 1,
            page_step: 10,
            orientation: Orientation::Horizontal,
            value_changed: Signal1::new(),
            slider_moved: Signal1::new(),
            slider_pressed: GenericSignal::new(),
            slider_released: GenericSignal::new(),
            mouse_pressed: false,
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
        let clamped = value.clamp(self.minimum, self.maximum);
        if self.value == clamped {
            return;
        }
        self.value = clamped;
        self.value_changed.emit(self.value);
    }
    /// Returns single step value.
    pub fn single_step(&self) -> i32 {
        self.single_step
    }
    /// Sets single step value.
    pub fn set_single_step(&mut self, step: i32) {
        self.single_step = step.max(1);
    }
    /// Returns page step value.
    pub fn page_step(&self) -> i32 {
        self.page_step
    }
    /// Sets page step value.
    pub fn set_page_step(&mut self, step: i32) {
        self.page_step = step.max(1);
    }
    /// Returns orientation.
    pub fn orientation(&self) -> Orientation {
        self.orientation
    }
    /// Sets orientation.
    pub fn set_orientation(&mut self, orientation: Orientation) {
        self.orientation = orientation;
    }
    /// Returns slider size as percentage of visible area.
    pub fn slider_size(&self) -> f32 {
        if self.maximum == self.minimum {
            return 1.0;
        }
        let page_size = self.page_step as f32;
        let total_range = (self.maximum - self.minimum) as f32;
        (page_size / total_range).clamp(0.1, 0.9)
    }
    /// Returns slider position as percentage.
    pub fn slider_position(&self) -> f32 {
        if self.maximum == self.minimum {
            return 0.0;
        }
        ((self.value - self.minimum) as f32) / ((self.maximum - self.minimum) as f32)
    }
    /// Returns value for a given pixel position.
    fn pixel_pos_to_value(&self, pos: f32) -> i32 {
        let rect = self.geometry();
        let slider_size = self.slider_size();
        let range = (self.maximum - self.minimum) as f32;
        match self.orientation {
            Orientation::Horizontal => {
                let available_width = rect.width as f32 * (1.0 - slider_size);
                let relative = (pos - rect.x as f32) / available_width;
                let value = self.minimum as f32 + range * relative.clamp(0.0, 1.0);
                value.round() as i32
            }
            Orientation::Vertical => {
                let available_height = rect.height as f32 * (1.0 - slider_size);
                let relative = (pos - rect.y as f32) / available_height;
                let value = self.minimum as f32 + range * relative.clamp(0.0, 1.0);
                value.round() as i32
            }
        }
    }
    /// Returns pixel position for a given value.
    fn value_to_pixel_pos(&self, value: i32) -> f32 {
        let rect = self.geometry();
        let clamped = value.clamp(self.minimum, self.maximum);
        let slider_size = self.slider_size();
        let range = (self.maximum - self.minimum) as f32;
        if range == 0.0 {
            return match self.orientation {
                Orientation::Horizontal => rect.x as f32,
                Orientation::Vertical => rect.y as f32,
            };
        }
        let relative = (clamped - self.minimum) as f32 / range;
        match self.orientation {
            Orientation::Horizontal => {
                let available_width = rect.width as f32 * (1.0 - slider_size);
                rect.x as f32 + available_width * relative
            }
            Orientation::Vertical => {
                let available_height = rect.height as f32 * (1.0 - slider_size);
                rect.y as f32 + available_height * relative
            }
        }
    }
    /// Triggers a scroll action.
    pub fn trigger_action(&mut self, action: ScrollBarAction) {
        match action {
            ScrollBarAction::LineUp => {
                self.set_value(self.value - self.single_step);
            }
            ScrollBarAction::LineDown => {
                self.set_value(self.value + self.single_step);
            }
            ScrollBarAction::PageUp => {
                self.set_value(self.value - self.page_step);
            }
            ScrollBarAction::PageDown => {
                self.set_value(self.value + self.page_step);
            }
            ScrollBarAction::SliderMove => {
                // Handled by mouse events
            }
            ScrollBarAction::SliderPageStepAdd => {
                self.set_value(self.value + self.page_step);
            }
            ScrollBarAction::SliderPageStepSub => {
                self.set_value(self.value - self.page_step);
            }
            ScrollBarAction::SliderToMinimum => {
                self.set_value(self.minimum);
            }
            ScrollBarAction::SliderToMaximum => {
                self.set_value(self.maximum);
            }
        }
    }
}
/// Scroll bar actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollBarAction {
    /// Move up/left by line
    LineUp,
    /// Move down/right by line
    LineDown,
    /// Move up/left by page
    PageUp,
    /// Move down/right by page
    PageDown,
    /// Move slider
    SliderMove,
    /// Move slider by page step up/right
    SliderPageStepAdd,
    /// Move slider by page step down/left
    SliderPageStepSub,
    /// Move slider to minimum
    SliderToMinimum,
    /// Move slider to maximum
    SliderToMaximum,
}
// Implement Widget trait
impl Widget for ScrollBar {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}
impl EventHandler for ScrollBar {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button } if *button == 1 => {
                self.mouse_pressed = true;
                self.slider_pressed.emit();
                let value = self.pixel_pos_to_value(pos.x as f32);
                self.set_value(value);
            }
            Event::MouseRelease { pos: _, button } if *button == 1 => {
                self.mouse_pressed = false;
                self.slider_released.emit();
            }
            Event::MouseMove { pos } if self.mouse_pressed => {
                let value = self.pixel_pos_to_value(pos.x as f32);
                self.set_value(value);
                self.slider_moved.emit(value);
            }
            Event::KeyPress { key, modifiers: _ } => {
                match *key {
                    37 => {
                        // Left arrow (or up arrow for vertical)
                        self.trigger_action(ScrollBarAction::LineUp);
                    }
                    38 => {
                        // Up arrow (or right arrow for horizontal)
                        if self.orientation == Orientation::Vertical {
                            self.trigger_action(ScrollBarAction::LineUp);
                        } else {
                            self.trigger_action(ScrollBarAction::LineDown);
                        }
                    }
                    39 => {
                        // Right arrow (or down arrow for vertical)
                        self.trigger_action(ScrollBarAction::LineDown);
                    }
                    40 => {
                        // Down arrow (or left arrow for horizontal)
                        if self.orientation == Orientation::Vertical {
                            self.trigger_action(ScrollBarAction::LineDown);
                        } else {
                            self.trigger_action(ScrollBarAction::LineUp);
                        }
                    }
                    33 => {
                        // Page up
                        self.trigger_action(ScrollBarAction::PageUp);
                    }
                    34 => {
                        // Page down
                        self.trigger_action(ScrollBarAction::PageDown);
                    }
                    36 => {
                        // Home
                        self.trigger_action(ScrollBarAction::SliderToMinimum);
                    }
                    35 => {
                        // End
                        self.trigger_action(ScrollBarAction::SliderToMaximum);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}
impl Draw for ScrollBar {
    fn draw(&mut self, context: &mut RenderContext) {
        // Draw base widget
        let rect = self.geometry();
        let slider_pos = self.value_to_pixel_pos(self.value);
        let slider_size = self.slider_size();
        // Draw background
        context.fill_rect(
            Rect::new(rect.x, rect.y, rect.width, rect.height),
            Color::from_rgb(240, 240, 240),
        );
        // Draw border
        context.draw_rect(
            Rect::new(rect.x, rect.y, rect.width, rect.height),
            Color::from_rgb(200, 200, 200),
        );
        // Draw slider
        match self.orientation {
            Orientation::Horizontal => {
                let slider_width = (rect.width as f32 * slider_size) as u32;
                context.fill_rect(
                    Rect::from_f32(
                        slider_pos,
                        rect.y as f32,
                        slider_width as f32,
                        rect.height as f32,
                    ),
                    Color::from_rgb(180, 180, 180),
                );
                // Draw slider border
                context.draw_rect(
                    Rect::from_f32(
                        slider_pos,
                        rect.y as f32,
                        slider_width as f32,
                        rect.height as f32,
                    ),
                    Color::from_rgb(150, 150, 150),
                );
                // Draw arrows using draw_line (triangles approximated)
                let arrow_size = (rect.height as f32).min(rect.width as f32 * 0.2) as u32;
                // Left arrow head
                context.draw_line(
                    Point::from_f32(
                        rect.x as f32 + arrow_size as f32 / 2.0,
                        rect.y as f32 + rect.height as f32 / 2.0,
                    ),
                    Point::from_f32(
                        rect.x as f32 + arrow_size as f32,
                        rect.y as f32 + rect.height as f32 / 4.0,
                    ),
                    Color::from_rgb(100, 100, 100),
                );
                context.draw_line(
                    Point::from_f32(
                        rect.x as f32 + arrow_size as f32 / 2.0,
                        rect.y as f32 + rect.height as f32 / 2.0,
                    ),
                    Point::from_f32(
                        rect.x as f32 + arrow_size as f32,
                        rect.y as f32 + rect.height as f32 * 3.0 / 4.0,
                    ),
                    Color::from_rgb(100, 100, 100),
                );
                // Right arrow head
                context.draw_line(
                    Point::from_f32(
                        rect.x as f32 + rect.width as f32 - arrow_size as f32 / 2.0,
                        rect.y as f32 + rect.height as f32 / 2.0,
                    ),
                    Point::from_f32(
                        rect.x as f32 + rect.width as f32 - arrow_size as f32,
                        rect.y as f32 + rect.height as f32 / 4.0,
                    ),
                    Color::from_rgb(100, 100, 100),
                );
                context.draw_line(
                    Point::from_f32(
                        rect.x as f32 + rect.width as f32 - arrow_size as f32 / 2.0,
                        rect.y as f32 + rect.height as f32 / 2.0,
                    ),
                    Point::from_f32(
                        rect.x as f32 + rect.width as f32 - arrow_size as f32,
                        rect.y as f32 + rect.height as f32 * 3.0 / 4.0,
                    ),
                    Color::from_rgb(100, 100, 100),
                );
            }
            Orientation::Vertical => {
                let slider_height = (rect.height as f32 * slider_size) as u32;
                context.fill_rect(
                    Rect::from_f32(
                        rect.x as f32,
                        slider_pos,
                        rect.width as f32,
                        slider_height as f32,
                    ),
                    Color::from_rgb(180, 180, 180),
                );
                // Draw slider border
                context.draw_rect(
                    Rect::from_f32(
                        rect.x as f32,
                        slider_pos,
                        rect.width as f32,
                        slider_height as f32,
                    ),
                    Color::from_rgb(150, 150, 150),
                );
                // Draw arrows using draw_line (triangles approximated)
                let arrow_size = (rect.width as f32).min(rect.height as f32 * 0.2) as u32;
                // Up arrow head
                context.draw_line(
                    Point::from_f32(
                        rect.x as f32 + rect.width as f32 / 2.0,
                        rect.y as f32 + arrow_size as f32 / 2.0,
                    ),
                    Point::from_f32(
                        rect.x as f32 + rect.width as f32 / 4.0,
                        rect.y as f32 + arrow_size as f32,
                    ),
                    Color::from_rgb(100, 100, 100),
                );
                context.draw_line(
                    Point::from_f32(
                        rect.x as f32 + rect.width as f32 / 2.0,
                        rect.y as f32 + arrow_size as f32 / 2.0,
                    ),
                    Point::from_f32(
                        rect.x as f32 + rect.width as f32 * 3.0 / 4.0,
                        rect.y as f32 + arrow_size as f32,
                    ),
                    Color::from_rgb(100, 100, 100),
                );
                // Down arrow head
                context.draw_line(
                    Point::from_f32(
                        rect.x as f32 + rect.width as f32 / 2.0,
                        rect.y as f32 + rect.height as f32 - arrow_size as f32 / 2.0,
                    ),
                    Point::from_f32(
                        rect.x as f32 + rect.width as f32 / 4.0,
                        rect.y as f32 + rect.height as f32 - arrow_size as f32,
                    ),
                    Color::from_rgb(100, 100, 100),
                );
                context.draw_line(
                    Point::from_f32(
                        rect.x as f32 + rect.width as f32 / 2.0,
                        rect.y as f32 + rect.height as f32 - arrow_size as f32 / 2.0,
                    ),
                    Point::from_f32(
                        rect.x as f32 + rect.width as f32 * 3.0 / 4.0,
                        rect.y as f32 + rect.height as f32 - arrow_size as f32,
                    ),
                    Color::from_rgb(100, 100, 100),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Color, Orientation, Rect};
    use crate::style::WidgetStyle;

    #[test]
    fn scrollbar_creation_defaults() {
        let sb = ScrollBar::new(Rect::new(0, 0, 200, 16));
        assert_eq!(sb.minimum(), 0);
        assert_eq!(sb.maximum(), 100);
        assert_eq!(sb.value(), 0);
        assert_eq!(sb.single_step(), 1);
        assert_eq!(sb.page_step(), 10);
        assert_eq!(sb.orientation(), Orientation::Horizontal);
    }

    #[test]
    fn scrollbar_set_value() {
        let mut sb = ScrollBar::new(Rect::new(0, 0, 200, 16));
        sb.set_value(50);
        assert_eq!(sb.value(), 50);
        sb.set_value(200); // clamp to max
        assert_eq!(sb.value(), 100);
        sb.set_value(-10); // clamp to min
        assert_eq!(sb.value(), 0);
    }

    #[test]
    fn scrollbar_set_range() {
        let mut sb = ScrollBar::new(Rect::new(0, 0, 200, 16));
        sb.set_minimum(10);
        sb.set_maximum(200);
        assert_eq!(sb.minimum(), 10);
        assert_eq!(sb.maximum(), 200);
    }

    #[test]
    fn scrollbar_set_range_reclamps_value() {
        let mut sb = ScrollBar::new(Rect::new(0, 0, 200, 16));
        sb.set_value(50);
        sb.set_range(60, 100);
        assert_eq!(sb.value(), 60);
    }

    #[test]
    fn scrollbar_single_step() {
        let mut sb = ScrollBar::new(Rect::new(0, 0, 200, 16));
        sb.set_single_step(5);
        assert_eq!(sb.single_step(), 5);
        sb.set_single_step(0); // floors at 1
        assert_eq!(sb.single_step(), 1);
    }

    #[test]
    fn scrollbar_page_step() {
        let mut sb = ScrollBar::new(Rect::new(0, 0, 200, 16));
        sb.set_page_step(25);
        assert_eq!(sb.page_step(), 25);
        sb.set_page_step(0); // floors at 1
        assert_eq!(sb.page_step(), 1);
    }

    #[test]
    fn scrollbar_orientation() {
        let mut sb = ScrollBar::new(Rect::new(0, 0, 200, 16));
        sb.set_orientation(Orientation::Vertical);
        assert_eq!(sb.orientation(), Orientation::Vertical);
        sb.set_orientation(Orientation::Horizontal);
        assert_eq!(sb.orientation(), Orientation::Horizontal);
    }

    #[test]
    fn scrollbar_slider_position() {
        let sb = ScrollBar::new(Rect::new(0, 0, 200, 16));
        assert!((sb.slider_position() - 0.0).abs() < f32::EPSILON);

        let mut sb = ScrollBar::new(Rect::new(0, 0, 200, 16));
        sb.set_value(50);
        assert!((sb.slider_position() - 0.5).abs() < f32::EPSILON);

        sb.set_value(100);
        assert!((sb.slider_position() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn scrollbar_trigger_action() {
        let mut sb = ScrollBar::new(Rect::new(0, 0, 200, 16));
        sb.set_value(50);

        sb.trigger_action(ScrollBarAction::LineUp);
        assert_eq!(sb.value(), 49);

        sb.trigger_action(ScrollBarAction::LineDown);
        assert_eq!(sb.value(), 50);

        sb.trigger_action(ScrollBarAction::PageUp);
        assert_eq!(sb.value(), 40);

        sb.trigger_action(ScrollBarAction::PageDown);
        assert_eq!(sb.value(), 50);

        sb.trigger_action(ScrollBarAction::SliderToMinimum);
        assert_eq!(sb.value(), 0);

        sb.trigger_action(ScrollBarAction::SliderToMaximum);
        assert_eq!(sb.value(), 100);
    }

    #[test]
    fn scrollbar_geometry_delegation() {
        let mut sb = ScrollBar::new(Rect::new(0, 0, 200, 16));
        sb.set_geometry(Rect::new(10, 10, 300, 30));
        assert_eq!(sb.geometry(), Rect::new(10, 10, 300, 30));
    }

    #[test]
    fn scrollbar_visibility() {
        let mut sb = ScrollBar::new(Rect::new(0, 0, 200, 16));
        assert!(sb.is_visible());
        sb.hide();
        assert!(!sb.is_visible());
        sb.show();
        assert!(sb.is_visible());
    }

    #[test]
    fn scrollbar_enabled() {
        let mut sb = ScrollBar::new(Rect::new(0, 0, 200, 16));
        assert!(sb.is_enabled());
        sb.set_enabled(false);
        assert!(!sb.is_enabled());
        sb.set_enabled(true);
        assert!(sb.is_enabled());
    }

    #[test]
    fn scrollbar_tooltip_roundtrip() {
        let mut sb = ScrollBar::new(Rect::new(0, 0, 200, 16));
        assert!(sb.tooltip().is_empty());
        sb.set_tooltip("Scroll".to_string());
        assert_eq!(sb.tooltip(), "Scroll");
        sb.set_tooltip(String::new());
        assert!(sb.tooltip().is_empty());
    }

    #[test]
    fn scrollbar_style_roundtrip() {
        let mut sb = ScrollBar::new(Rect::new(0, 0, 200, 16));
        assert_eq!(*sb.style(), WidgetStyle::default());
        let custom = WidgetStyle::default().with_background(Color::from_rgb(200, 200, 200));
        sb.set_style(custom.clone());
        assert_eq!(*sb.style(), custom);
    }

    #[test]
    fn scrollbar_id_kind() {
        let sb_a = ScrollBar::new(Rect::new(0, 0, 100, 16));
        let sb_b = ScrollBar::new(Rect::new(0, 0, 100, 16));
        assert_ne!(sb_a.id(), sb_b.id());
        assert_eq!(sb_a.kind(), WidgetKind::ScrollBar);
        assert_eq!(sb_b.kind(), WidgetKind::ScrollBar);
    }

    #[test]
    fn scrollbar_signal_accessors() {
        let sb = ScrollBar::new(Rect::new(0, 0, 100, 16));
        let _value_changed = &sb.value_changed;
        let _slider_moved = &sb.slider_moved;
        let _slider_pressed = &sb.slider_pressed;
        let _slider_released = &sb.slider_released;
    }
}
