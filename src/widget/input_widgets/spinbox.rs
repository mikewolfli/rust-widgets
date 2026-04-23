//! Spin box widget for numeric input.
use crate::core::{Alignment, Color, Font, ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::object::Object;
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::style::{Margin, Padding, WidgetStyle};
use crate::widget::{BaseWidget, Draw, Image, Widget, WidgetKind};
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
    /// Sets range.
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
        let mut text = format!("{}{}{}", self.prefix, self.value, self.suffix);
        text
    }
}
// Implement Widget trait
impl Widget for SpinBox {
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
                    // Check if click is on up/down buttons
                    if pos.x >= rect.x + rect.width as f32 - button_width * 2 {
                        if pos.x < rect.x + rect.width as f32 - button_width {
                            // Down button
                            self.step_down();
                        } else {
                            // Up button
                            self.step_up();
                        }
                        self.base.clicked.emit();
                    }
                }
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
        let text_y = rect.y + rect.height as f32 / 2;
        // Draw background
        context.fill_rect(
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            Color::from_rgb(255, 255, 255),
        );
        // Draw border
        context.draw_rect(
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            Color::from_rgb(200, 200, 200),
        );
        // Draw up/down buttons
        let down_button_x = rect.x + rect.width as f32 - button_width * 2;
        let up_button_x = rect.x + rect.width as f32 - button_width;
        // Down button
        context.fill_rect(
            down_button_x,
            rect.y,
            button_width,
            rect.height,
            Color::from_rgb(240, 240, 240),
        );
        context.draw_rect(
            down_button_x,
            rect.y,
            button_width,
            rect.height,
            Color::from_rgb(200, 200, 200),
        );
        // Down arrow
        let down_arrow_x = down_button_x + button_width / 2;
        let down_arrow_y = rect.y + rect.height as f32 / 2;
        let arrow_size = 4;
        context.draw_line(Point::new(down_arrow_x - arrow_size as f32, down_arrow_y - arrow_size / 2 as f32), Point::new(down_arrow_x + arrow_size as f32, down_arrow_y - arrow_size / 2 as f32), Color::from_rgb(100, 100, 100),
        );
        context.draw_line(Point::new(down_arrow_x + arrow_size as f32, down_arrow_y - arrow_size / 2 as f32), Point::new(down_arrow_x as f32, down_arrow_y + arrow_size / 2 as f32), Color::from_rgb(100, 100, 100),
        );
        context.draw_line(Point::new(down_arrow_x as f32, down_arrow_y + arrow_size / 2 as f32), Point::new(down_arrow_x - arrow_size as f32, down_arrow_y - arrow_size / 2 as f32), Color::from_rgb(100, 100, 100),
        );
        // Up button
        context.fill_rect(
            up_button_x,
            rect.y,
            button_width,
            rect.height,
            Color::from_rgb(240, 240, 240),
        );
        context.draw_rect(
            up_button_x,
            rect.y,
            button_width,
            rect.height,
            Color::from_rgb(200, 200, 200),
        );
        // Up arrow
        let up_arrow_x = up_button_x + button_width / 2;
        let up_arrow_y = rect.y + rect.height as f32 / 2;
        context.draw_line(Point::new(up_arrow_x - arrow_size as f32, up_arrow_y + arrow_size / 2 as f32), Point::new(up_arrow_x + arrow_size as f32, up_arrow_y + arrow_size / 2 as f32), Color::from_rgb(100, 100, 100),
        );
        context.draw_line(Point::new(up_arrow_x + arrow_size as f32, up_arrow_y + arrow_size / 2 as f32), Point::new(up_arrow_x as f32, up_arrow_y - arrow_size / 2 as f32), Color::from_rgb(100, 100, 100),
        );
        context.draw_line(Point::new(up_arrow_x as f32, up_arrow_y - arrow_size / 2 as f32), Point::new(up_arrow_x - arrow_size as f32, up_arrow_y + arrow_size / 2 as f32), Color::from_rgb(100, 100, 100),
        );
        // Draw text
        let display_text = self.display_text();
        if !display_text.is_empty() {
            context.draw_text(
                text_x,
                text_y,
                &display_text,
                &Font::default(),
                Color::from_rgb(0, 0, 0),
                Alignment::Left,
            );
        }
    }
}
