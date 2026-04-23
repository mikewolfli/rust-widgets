//! Status bar widget.
use crate::core::{Alignment, Color, Font, ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::object::Object;
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::style::WidgetStyle;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
/// Status bar widget — shows status messages and permanent widgets.
pub struct StatusBar {
    base: BaseWidget,
    message: String,
    permanent_message: String,
    size_grip_enabled: bool,
    pub message_changed: Signal1<String>,
}
impl StatusBar {
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::StatusBar, geometry, "StatusBar"),
            message: String::new(),
            permanent_message: String::new(),
            size_grip_enabled: true,
            message_changed: Signal1::new(),
        }
    }
    pub fn message(&self) -> &str {
        &self.message
    }
    pub fn permanent_message(&self) -> &str {
        &self.permanent_message
    }
    pub fn size_grip_enabled(&self) -> bool {
        self.size_grip_enabled
    }
    /// Show a temporary status message (timeout_ms is informational; actual timeout managed externally).
    pub fn show_message(&mut self, message: impl Into<String>, _timeout_ms: u64) {
        self.message = message.into();
        self.message_changed.emit(self.message.clone());
    }
    pub fn clear_message(&mut self) {
        self.message.clear();
        self.message_changed.emit(String::new());
    }
    pub fn set_permanent_message(&mut self, msg: impl Into<String>) {
        self.permanent_message = msg.into();
    }
    pub fn set_size_grip_enabled(&mut self, enabled: bool) {
        self.size_grip_enabled = enabled;
    }
}
impl Widget for StatusBar {
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
impl EventHandler for StatusBar {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}
impl Draw for StatusBar {
    fn draw(&mut self, context: &mut RenderContext) {
        self.base.paint(context);
        let rect = self.geometry();
        // Background
        context.fill_rect(rect, Color::from_rgb(240, 240, 240));
        context.draw_line(Point::new(Point::new(rect.x, rect.y)), Point::new(Point::new(rect.x + rect.width as i32 as i32, rect.y)), Color::from_rgb(200, 200, 200),
        );
        // Temporary message (left side)
        if !self.message.is_empty() {
            context.draw_text(
                Point::new(rect.x + 6, rect.y + (rect.height as i32) / 2),
                &self.message,
                &Font::default(),
                Color::from_rgb(0, 0, 0),
            );
        }
        // Permanent message (right side, before size grip)
        if !self.permanent_message.is_empty() {
            let right_x = if self.size_grip_enabled {
                rect.x + rect.width as i32 as i32 - 20
            } else {
                rect.x + rect.width as i32 as i32 - 4
            };
            context.draw_text(
                Point::new(right_x, rect.y + (rect.height as i32) / 2),
                &self.permanent_message,
                &Font::default(),
                Color::from_rgb(80, 80, 80),
            );
        }
        // Size grip (bottom-right corner)
        if self.size_grip_enabled {
            let gx = rect.x + rect.width as i32 as i32 - 14;
            let gy = rect.y + rect.height as i32 as i32 - 14;
            for i in 0..3 {
                let offset = i * 4;
                context.draw_line(Point::new(Point::new(gx + offset, gy + 12)), Point::new(Point::new(gx + 12, gy + offset)), Color::from_rgb(160, 160, 160),
                );
            }
        }
    }
}
