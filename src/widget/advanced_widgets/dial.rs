//! Dial (knob) widget.
use crate::core::{Color, Font, ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::object::Object;
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::style::WidgetStyle;
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
                let v = (value - self.minimum).rem_euclid(range) + self.minimum;
                v
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
    fn id(&self) -> ObjectId {
        self.base.id()
    }
    fn kind(&self) -> WidgetKind {
        self.base.kind()
    }
    fn geometry(&self) -> Rect {
        self.base.geometry()
    }
    fn set_geometry(&mut self, g: Rect) {
        self.base.set_geometry(g);
    }
    fn min_size(&self) -> Option<Size> {
        self.base.min_size()
    }
    fn max_size(&self) -> Option<Size> {
        self.base.max_size()
    }
    fn set_min_size(&mut self, s: Option<Size>) {
        self.base.set_min_size(s);
    }
    fn set_max_size(&mut self, s: Option<Size>) {
        self.base.set_max_size(s);
    }
    fn parent(&self) -> Option<ObjectId> {
        self.base.parent()
    }
    fn set_parent(&mut self, p: Option<ObjectId>) {
        self.base.set_parent(p);
    }
    fn add_child(&mut self, c: ObjectId) {
        self.base.add_child(c);
    }
    fn remove_child(&mut self, c: ObjectId) {
        self.base.remove_child(c);
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
    fn set_enabled(&mut self, e: bool) {
        self.base.set_enabled(e);
    }
    fn is_enabled(&self) -> bool {
        self.base.is_enabled()
    }
    fn set_tooltip(&mut self, t: String) {
        self.base.set_tooltip(t);
    }
    fn tooltip(&self) -> &str {
        self.base.tooltip()
    }
    fn style(&self) -> &WidgetStyle {
        self.base.style()
    }
    fn set_style(&mut self, s: WidgetStyle) {
        self.base.set_style(s);
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
        context.fill_circle(center, radius as u32, Color::from_rgb(230, 230, 230));
        context.draw_circle(center, radius as u32, Color::from_rgb(150, 150, 150));
        // Draw a simple value needle.
        let angle = self.value_angle();
        let needle_len = (radius as f32 * 0.7) as i32;
        let to = Point {
            x: center.x + (needle_len as f32 * angle.cos() as f32 as f32) as i32,
            y: center.y + (needle_len as f32 * angle.sin() as f32 as f32) as i32,
        };
        context.draw_line(center, to, Color::from_rgb(0, 0, 0));
        context.fill_circle(center, 3, Color::from_rgb(80, 80, 80));
    }
}
