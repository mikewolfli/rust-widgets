//! Status bar widget.
use crate::core::{Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
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
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
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
        context.draw_line(
            Point::new(rect.x, rect.y),
            Point::new(rect.x + rect.width as i32, rect.y),
            Color::from_rgb(200, 200, 200),
        );
        // Temporary message (left side)
        if !self.message.is_empty() {
            context.draw_text(
                Point::new(rect.x + 6, rect.y + rect.height as i32 / 2),
                &self.message,
                &Font::default(),
                Color::from_rgb(0, 0, 0),
            );
        }
        // Permanent message (right side, before size grip)
        if !self.permanent_message.is_empty() {
            let right_x = if self.size_grip_enabled {
                rect.x + rect.width as f32 as i32 - 20
            } else {
                rect.x + rect.width as f32 as i32 - 4
            };
            context.draw_text(
                Point::new(right_x, rect.y + rect.height as i32 / 2),
                &self.permanent_message,
                &Font::default(),
                Color::from_rgb(80, 80, 80),
            );
        }
        // Size grip (bottom-right corner)
        if self.size_grip_enabled {
            let gx = rect.x + rect.width as f32 as i32 - 14;
            let gy = rect.y + rect.height as f32 as i32 - 14;
            for i in 0..3 {
                let offset = i * 4;
                context.draw_line(
                    Point::new(gx + offset, gy + 12),
                    Point::new(gx + 12, gy + offset),
                    Color::from_rgb(160, 160, 160),
                );
            }
        }
    }
}
