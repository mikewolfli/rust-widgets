//! Status bar widget.
use crate::core::{HorizontalAlignment, Color, Font, Point, Rect};
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
                HorizontalAlignment::Left,
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
                HorizontalAlignment::Left,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Rect;

    #[test]
    fn statusbar_creation_defaults() {
        let sb = StatusBar::new(Rect::new(0, 0, 800, 24));
        assert!(sb.message().is_empty());
        assert!(sb.permanent_message().is_empty());
        assert!(sb.size_grip_enabled());
    }

    #[test]
    fn statusbar_show_message() {
        let mut sb = StatusBar::new(Rect::new(0, 0, 800, 24));
        assert!(sb.message().is_empty());
        sb.show_message("Ready", 3000);
        assert_eq!(sb.message(), "Ready");
    }

    #[test]
    fn statusbar_clear_message() {
        let mut sb = StatusBar::new(Rect::new(0, 0, 800, 24));
        sb.show_message("Busy", 5000);
        assert_eq!(sb.message(), "Busy");
        sb.clear_message();
        assert!(sb.message().is_empty());
    }

    #[test]
    fn statusbar_permanent_message() {
        let mut sb = StatusBar::new(Rect::new(0, 0, 800, 24));
        assert!(sb.permanent_message().is_empty());
        sb.set_permanent_message("Line: 1  Col: 1");
        assert_eq!(sb.permanent_message(), "Line: 1  Col: 1");
        sb.set_permanent_message("");
        assert!(sb.permanent_message().is_empty());
    }

    #[test]
    fn statusbar_size_grip() {
        let mut sb = StatusBar::new(Rect::new(0, 0, 800, 24));
        assert!(sb.size_grip_enabled());
        sb.set_size_grip_enabled(false);
        assert!(!sb.size_grip_enabled());
        sb.set_size_grip_enabled(true);
        assert!(sb.size_grip_enabled());
    }

    #[test]
    fn statusbar_geometry_delegation() {
        let mut sb = StatusBar::new(Rect::new(0, 0, 800, 24));
        sb.set_geometry(Rect::new(0, 700, 800, 24));
        assert_eq!(sb.geometry(), Rect::new(0, 700, 800, 24));
    }

    #[test]
    fn statusbar_visibility() {
        let mut sb = StatusBar::new(Rect::new(0, 0, 800, 24));
        assert!(sb.is_visible());
        sb.hide();
        assert!(!sb.is_visible());
        sb.show();
        assert!(sb.is_visible());
    }

    #[test]
    fn statusbar_signal_accessors() {
        let sb = StatusBar::new(Rect::new(0, 0, 800, 24));
        let _ = &sb.message_changed;
    }

    #[test]
    fn statusbar_id_kind() {
        let sb_a = StatusBar::new(Rect::new(0, 0, 800, 24));
        let sb_b = StatusBar::new(Rect::new(0, 0, 800, 24));
        assert_ne!(sb_a.id(), sb_b.id());
        assert_eq!(sb_a.kind(), WidgetKind::StatusBar);
        assert_eq!(sb_b.kind(), WidgetKind::StatusBar);
    }

    #[test]
    fn statusbar_draw_produces_svg_output() {
        let mut sb = StatusBar::new(Rect::new(0, 0, 800, 24));
        sb.show_message("Ready", 3000);
        let svg = crate::widget::svg::render_to_svg(&mut sb);
        assert!(svg.starts_with("<svg"));
    }
}
