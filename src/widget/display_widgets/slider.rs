//! Slider widget.
use crate::core::{Color, Orientation, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
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
    pub value_changed: Signal1<i32>,
    pub slider_moved: Signal1<i32>,
    pub slider_pressed: GenericSignal,
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
        let clamped = value.clamp(self.minimum, self.maximum);
        if self.value == clamped {
            return;
        }
        self.value = clamped;
        self.slider_position = clamped;
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
    /// Returns tick position.
    pub fn tick_position(&self) -> TickPosition {
        self.tick_position
    }
    /// Sets tick position.
    pub fn set_tick_position(&mut self, position: TickPosition) {
        self.tick_position = position;
    }
    /// Returns tick interval.
    pub fn tick_interval(&self) -> i32 {
        self.tick_interval
    }
    /// Sets tick interval.
    pub fn set_tick_interval(&mut self, interval: i32) {
        self.tick_interval = interval.max(0);
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
        let new_position = position.clamp(self.minimum, self.maximum);
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
        match self.orientation {
            Orientation::Horizontal => {
                let relative = (pos - rect.x as f32) / rect.width as f32;
                let value = self.minimum as f32 + range * relative.clamp(0.0, 1.0);
                value.round() as i32
            }
            Orientation::Vertical => {
                let relative = 1.0 - (pos - rect.y as f32) / rect.height as f32; // Invert Y axis
                let value = self.minimum as f32 + range * relative.clamp(0.0, 1.0);
                value.round() as i32
            }
        }
    }
    /// Returns pixel position for a given value.
    fn value_to_pixel_pos(&self, value: i32) -> f32 {
        let rect = self.geometry();
        let clamped = value.clamp(self.minimum, self.maximum);
        let range = (self.maximum - self.minimum) as f32;
        if range == 0.0 {
            return match self.orientation {
                Orientation::Horizontal => rect.x as f32,
                Orientation::Vertical => rect.y as f32 + rect.height as f32 / 2.0,
            };
        }
        let relative = (clamped - self.minimum) as f32 / range;
        match self.orientation {
            Orientation::Horizontal => rect.x as f32 + rect.width as f32 * relative,
            Orientation::Vertical => rect.y as f32 + rect.height as f32 * (1.0 - relative), // Invert Y axis
        }
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
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        match self.orientation() {
            Orientation::Horizontal => Size::new(100, 28),
            Orientation::Vertical => Size::new(28, 100),
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
                match *key {
                    37 => {
                        // Left arrow (or up arrow for vertical)
                        self.trigger_action(SliderAction::SliderSingleStepSub);
                    }
                    38 => {
                        // Up arrow (or right arrow for horizontal)
                        if self.orientation == Orientation::Vertical {
                            self.trigger_action(SliderAction::SliderSingleStepAdd);
                        } else {
                            self.trigger_action(SliderAction::SliderSingleStepSub);
                        }
                    }
                    39 => {
                        // Right arrow (or down arrow for vertical)
                        self.trigger_action(SliderAction::SliderSingleStepAdd);
                    }
                    40 => {
                        // Down arrow (or left arrow for horizontal)
                        if self.orientation == Orientation::Vertical {
                            self.trigger_action(SliderAction::SliderSingleStepSub);
                        } else {
                            self.trigger_action(SliderAction::SliderSingleStepAdd);
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
        let slider_size = 16;
        let style = self.style();
        let groove_color = style.background_color.unwrap_or(Color::rgb(200, 200, 200));
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
                        slider_pos - slider_size as f32 / 2.0,
                        rect.y as f32,
                        slider_size as f32,
                        rect.height as f32,
                    ),
                    Color::rgb(0, 120, 215),
                );
                // Draw handle border
                if let Some(border_color) = style.border_color {
                    context.draw_rect(
                        Rect::from_f32(
                            slider_pos - slider_size as f32 / 2.0,
                            rect.y as f32,
                            slider_size as f32,
                            rect.height as f32,
                        ),
                        border_color,
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
                                Color::rgb(100, 100, 100),
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
                                Color::rgb(100, 100, 100),
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
                        slider_pos - slider_size as f32 / 2.0,
                        rect.width as f32,
                        slider_size as f32,
                    ),
                    Color::rgb(0, 120, 215),
                );
                // Draw handle border
                if let Some(border_color) = style.border_color {
                    context.draw_rect(
                        Rect::from_f32(
                            rect.x as f32,
                            slider_pos - slider_size as f32 / 2.0,
                            rect.width as f32,
                            slider_size as f32,
                        ),
                        border_color,
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
                                Color::rgb(100, 100, 100),
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
                                Color::rgb(100, 100, 100),
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
        // On release, if !tracking, set_value is called with slider_position
        assert_eq!(s.value(), 40); // 80/200 * 100 = 40
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
        let style = WidgetStyle {
            background_color: Some(Color::rgb(255, 0, 0)),
            ..WidgetStyle::default()
        };
        s.set_style(style.clone());
        assert_eq!(s.style().background_color, Some(Color::rgb(255, 0, 0)));
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
