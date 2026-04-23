//! Slider widget.
use crate::core::{Alignment, Color, Font, ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::object::Object;
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::style::{Margin, Padding, WidgetStyle};
use crate::widget::{BaseWidget, Draw, Image, Widget, WidgetKind};
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
    pub value_changed: Signal1<i32>,
    pub slider_moved: Signal1<i32>,
    pub slider_pressed: GenericSignal,
    pub slider_released: GenericSignal,
}
/// Slider orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    /// Horizontal slider (left to right)
    Horizontal,
    /// Vertical slider (bottom to top)
    Vertical,
}
impl Default for Orientation {
    fn default() -> Self {
        Self::Horizontal
    }
}
/// Tick mark position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TickPosition {
    /// No tick marks
    NoTicks,
    /// Tick marks above (for horizontal) or left (for vertical)
    TicksAbove,
    /// Tick marks below (for horizontal) or right (for vertical)
    TicksBelow,
    /// Tick marks on both sides
    TicksBothSides,
}
impl Default for TickPosition {
    fn default() -> Self {
        Self::NoTicks
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
        self.slider_position = position.clamp(self.minimum, self.maximum);
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
                let relative = (pos - rect.x) / rect.width;
                let value = self.minimum as f32 + range * relative.clamp(0, 1);
                value.round() as i32
            }
            Orientation::Vertical => {
                let relative = 1 - (pos - rect.y) / rect.height; // Invert Y axis
                let value = self.minimum as f32 + range * relative.clamp(0, 1);
                value.round() as i32
            }
        }
    }
    /// Returns pixel position for a given value.
    fn value_to_pixel_pos(&self, value: i32) -> f32 {
        let rect = self.geometry();
        let clamped = value.clamp(self.minimum, self.maximum);
        let range = (self.maximum - self.minimum) as f32;
        if range == 0 {
            return match self.orientation {
                Orientation::Horizontal => rect.x,
                Orientation::Vertical => rect.y + rect.height as f32 / 2,
            };
        }
        let relative = (clamped - self.minimum) as f32 / range;
        match self.orientation {
            Orientation::Horizontal => rect.x + rect.width as f32 * relative,
            Orientation::Vertical => rect.y + rect.height as f32 * (1 - relative), // Invert Y axis
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
impl EventHandler for Slider {
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
                    self.set_slider_position(value);
                }
            }
            Event::MouseRelease { pos: _, button } => {
                if *button == 1 {
                    self.slider_released.emit();
                    if !self.tracking {
                        self.set_value(self.slider_position);
                    }
                }
            }
            Event::MouseMove { pos } => {
                if self.base.is_mouse_pressed() {
                    let value = self.pixel_pos_to_value(pos.x);
                    self.set_slider_position(value);
                }
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
                    _ => {}
                }
            }
            _ => {}
        }
    }
}
impl Draw for Slider {
    fn draw(&mut self, context: &mut RenderContext) {
        // Draw base widget
        let rect = self.geometry();
        let slider_pos = self.value_to_pixel_pos(self.value);
        let slider_size = 16;
        // Draw groove (track)
        match self.orientation {
            Orientation::Horizontal => {
                let groove_y = rect.y + rect.height as f32 / 2;
                let groove_height = 4;
                // Draw groove
                context.fill_rect(
                    rect.x,
                    groove_y - groove_height / 2,
                    rect.width,
                    groove_height,
                    Color::from_rgb(200, 200, 200),
                );
                // Draw slider handle
                context.fill_rect(
                    slider_pos - slider_size / 2,
                    rect.y,
                    slider_size,
                    rect.height,
                    Color::from_rgb(0, 120, 215),
                );
                // Draw ticks if enabled
                if self.tick_position != TickPosition::NoTicks && self.tick_interval > 0 {
                    let tick_height = 6;
                    for value in (self.minimum..=self.maximum).step_by(self.tick_interval as usize)
                    {
                        let tick_x = self.value_to_pixel_pos(value);
                        if self.tick_position == TickPosition::TicksAbove
                            || self.tick_position == TickPosition::TicksBothSides
                        {
                            context.draw_line(Point::new(tick_x as f32, rect.y as f32), Point::new(tick_x as f32, rect.y + tick_height as f32), Color::from_rgb(100, 100, 100),
                            );
                        }
                        if self.tick_position == TickPosition::TicksBelow
                            || self.tick_position == TickPosition::TicksBothSides
                        {
                            context.draw_line(Point::new(tick_x as f32, rect.y + rect.height as f32 - tick_height as f32), Point::new(tick_x as f32, rect.y + rect.height as f32 as f32), Color::from_rgb(100, 100, 100),
                            );
                        }
                    }
                }
            }
            Orientation::Vertical => {
                let groove_x = rect.x + rect.width as f32 / 2;
                let groove_width = 4;
                // Draw groove
                context.fill_rect(
                    groove_x - groove_width / 2,
                    rect.y,
                    groove_width,
                    rect.height,
                    Color::from_rgb(200, 200, 200),
                );
                // Draw slider handle
                context.fill_rect(
                    rect.x,
                    slider_pos - slider_size / 2,
                    rect.width,
                    slider_size,
                    Color::from_rgb(0, 120, 215),
                );
                // Draw ticks if enabled
                if self.tick_position != TickPosition::NoTicks && self.tick_interval > 0 {
                    let tick_width = 6;
                    for value in (self.minimum..=self.maximum).step_by(self.tick_interval as usize)
                    {
                        let tick_y = self.value_to_pixel_pos(value);
                        if self.tick_position == TickPosition::TicksAbove
                            || self.tick_position == TickPosition::TicksBothSides
                        {
                            context.draw_line(Point::new(rect.x as f32, tick_y as f32), Point::new(rect.x + tick_width as f32, tick_y as f32), Color::from_rgb(100, 100, 100),
                            );
                        }
                        if self.tick_position == TickPosition::TicksBelow
                            || self.tick_position == TickPosition::TicksBothSides
                        {
                            context.draw_line(Point::new(rect.x + rect.width as f32 - tick_width as f32, tick_y as f32), Point::new(rect.x + rect.width as f32 as f32, tick_y as f32), Color::from_rgb(100, 100, 100),
                            );
                        }
                    }
                }
            }
        }
    }
}
