//! Scroll bar widget.
use crate::core::{Alignment, Color, Font, ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::object::Object;
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::style::{Margin, Padding, WidgetStyle};
use crate::widget::{BaseWidget, Draw, Image, Widget, WidgetKind};
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
}
/// Scroll bar orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    /// Horizontal scroll bar (left to right)
    Horizontal,
    /// Vertical scroll bar (top to bottom)
    Vertical,
}
impl Default for Orientation {
    fn default() -> Self {
        Self::Horizontal
    }
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
    /// Sets range.
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
            return 1;
        }
        let page_size = self.page_step as f32;
        let total_range = (self.maximum - self.minimum) as f32;
        (page_size / total_range).clamp(0.1, 0.9)
    }
    /// Returns slider position as percentage.
    pub fn slider_position(&self) -> f32 {
        if self.maximum == self.minimum {
            return 0;
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
                let available_width = rect.width * (1 - slider_size);
                let relative = (pos - rect.x) / available_width;
                let value = self.minimum as f32 + range * relative.clamp(0, 1);
                value.round() as i32
            }
            Orientation::Vertical => {
                let available_height = rect.height * (1 - slider_size);
                let relative = (pos - rect.y) / available_height;
                let value = self.minimum as f32 + range * relative.clamp(0, 1);
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
        if range == 0 {
            return match self.orientation {
                Orientation::Horizontal => rect.x,
                Orientation::Vertical => rect.y,
            };
        }
        let relative = (clamped - self.minimum) as f32 / range;
        match self.orientation {
            Orientation::Horizontal => {
                let available_width = rect.width * (1 - slider_size);
                rect.x + available_width * relative
            }
            Orientation::Vertical => {
                let available_height = rect.height * (1 - slider_size);
                rect.y + available_height * relative
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
    fn id(&self) -> ObjectId {
        self.base.id()
    }
    fn kind(&self) -> WidgetKind {
        self.base.kind()
    }
    fn geometry(&self) -> Rect {
        self.base.geometry()
    }
    fn set_geometry(&mut self, geometry: Rect) {
        self.base.set_geometry(geometry);
    }
    fn min_size(&self) -> Option<Size> {
        self.base.min_size()
    }
    fn max_size(&self) -> Option<Size> {
        self.base.max_size()
    }
    fn set_min_size(&mut self, min_size: Option<Size>) {
        self.base.set_min_size(min_size);
    }
    fn set_max_size(&mut self, max_size: Option<Size>) {
        self.base.set_max_size(max_size);
    }
    fn parent(&self) -> Option<ObjectId> {
        self.base.parent()
    }
    fn set_parent(&mut self, parent: Option<ObjectId>) {
        self.base.set_parent(parent);
    }
    fn add_child(&mut self, child: ObjectId) {
        self.base.add_child(child);
    }
    fn remove_child(&mut self, child: ObjectId) {
        self.base.remove_child(child);
    }
    fn children(&self) -> &[ObjectId] {
        self.base.children()
    }
    fn show(&mut self) {
        self.base.show();
    }
    fn hide(&mut self) {
        self.base.hide();
    }
    fn is_visible(&self) -> bool {
        self.base.is_visible()
    }
    fn set_enabled(&mut self, enabled: bool) {
        self.base.set_enabled(enabled);
    }
    fn is_enabled(&self) -> bool {
        self.base.is_enabled()
    }
    fn set_tooltip(&mut self, tooltip: String) {
        self.base.set_tooltip(tooltip);
    }
    fn tooltip(&self) -> &str {
        self.base.tooltip()
    }
    fn style(&self) -> &WidgetStyle {
        self.base.style()
    }
    fn set_style(&mut self, style: WidgetStyle) {
        self.base.set_style(style);
    }
    fn connection_scope(&self) -> &ConnectionScope {
        self.base.connection_scope()
    }
    fn hover_signal(&self) -> &Signal1<Point> {
        self.base.hover_signal()
    }
    fn mouse_down_signal(&self) -> &Signal1<(Point, u32)> {
        self.base.mouse_down_signal()
    }
    fn mouse_up_signal(&self) -> &Signal1<(Point, u32)> {
        self.base.mouse_up_signal()
    }
    fn key_down_signal(&self) -> &Signal1<(u32, u32)> {
        self.base.key_down_signal()
    }
    fn key_up_signal(&self) -> &Signal1<(u32, u32)> {
        self.base.key_up_signal()
    }
    fn focus_gained_signal(&self) -> &GenericSignal {
        self.base.focus_gained_signal()
    }
    fn focus_lost_signal(&self) -> &GenericSignal {
        self.base.focus_lost_signal()
    }
    fn redraw_requested_signal(&self) -> &GenericSignal {
        self.base.redraw_requested_signal()
    }
    fn layout_requested_signal(&self) -> &GenericSignal {
        self.base.layout_requested_signal()
    }
}
impl EventHandler for ScrollBar {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button } => {
                if *button == 1 {
                    self.slider_pressed.emit();
                    let value = self.pixel_pos_to_value(pos.x);
                    self.set_value(value);
                }
            }
            Event::MouseRelease { pos: _, button } => {
                if *button == 1 {
                    self.slider_released.emit();
                }
            }
            Event::MouseMove { pos } => {
                if self.base.is_mouse_pressed() {
                    let value = self.pixel_pos_to_value(pos.x);
                    self.set_value(value);
                    self.slider_moved.emit(value);
                }
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
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            Color::from_rgb(240, 240, 240),
        );
        // Draw border
        context.draw_rect(
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            Color::from_rgb(200, 200, 200),
        );
        // Draw slider
        match self.orientation {
            Orientation::Horizontal => {
                let slider_width = rect.width * slider_size;
                context.fill_rect(
                    slider_pos,
                    rect.y,
                    slider_width,
                    rect.height,
                    Color::from_rgb(180, 180, 180),
                );
                // Draw slider border
                context.draw_rect(
                    slider_pos,
                    rect.y,
                    slider_width,
                    rect.height,
                    Color::from_rgb(150, 150, 150),
                );
                // Draw arrows
                let arrow_size = rect.height.min(rect.width * 0.2);
                // Left arrow
                context.fill_triangle(
                    rect.x + arrow_size / 2,
                    rect.y + rect.height as i32 / 2,
                    rect.x + arrow_size,
                    rect.y + rect.height as i32 / 4,
                    rect.x + arrow_size,
                    rect.y + rect.height as i32 * 3 / 4,
                    Color::from_rgb(100, 100, 100),
                );
                // Right arrow
                context.fill_triangle(
                    rect.x + rect.width as i32 - arrow_size / 2,
                    rect.y + rect.height as i32 / 2,
                    rect.x + rect.width as i32 - arrow_size,
                    rect.y + rect.height as i32 / 4,
                    rect.x + rect.width as i32 - arrow_size,
                    rect.y + rect.height as i32 * 3 / 4,
                    Color::from_rgb(100, 100, 100),
                );
            }
            Orientation::Vertical => {
                let slider_height = rect.height * slider_size;
                context.fill_rect(
                    rect.x,
                    slider_pos,
                    rect.width,
                    slider_height,
                    Color::from_rgb(180, 180, 180),
                );
                // Draw slider border
                context.draw_rect(
                    rect.x,
                    slider_pos,
                    rect.width,
                    slider_height,
                    Color::from_rgb(150, 150, 150),
                );
                // Draw arrows
                let arrow_size = rect.width.min(rect.height * 0.2);
                // Up arrow
                context.fill_triangle(
                    rect.x + rect.width as i32 / 2,
                    rect.y + arrow_size / 2,
                    rect.x + rect.width as i32 / 4,
                    rect.y + arrow_size,
                    rect.x + rect.width as i32 * 3 / 4,
                    rect.y + arrow_size,
                    Color::from_rgb(100, 100, 100),
                );
                // Down arrow
                context.fill_triangle(
                    rect.x + rect.width as i32 / 2,
                    rect.y + rect.height as i32 - arrow_size / 2,
                    rect.x + rect.width as i32 / 4,
                    rect.y + rect.height as i32 - arrow_size,
                    rect.x + rect.width as i32 * 3 / 4,
                    rect.y + rect.height as i32 - arrow_size,
                    Color::from_rgb(100, 100, 100),
                );
            }
        }
    }
}
