//! Popup window widget.
use crate::core::Rect;
use crate::render::RenderContext;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
/// Popup window widget.
pub struct PopupWindow {
    base: BaseWidget,
}
impl PopupWindow {
    /// Creates a popup window with geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::PopupWindow, geometry, "PopupWindow"),
        }
    }
}
impl Widget for PopupWindow {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}
impl Draw for PopupWindow {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.base.geometry();
        use crate::core::Color;
        // Draw popup background with semi-transparent effect
        context.fill_rect(rect, Color::from_rgb(255, 255, 255));
        // Draw border
        context.draw_rect(rect, Color::from_rgb(120, 120, 120));
    }
}
impl crate::event::EventHandler for PopupWindow {
    fn handle_event(&mut self, event: &crate::event::Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            crate::event::Event::MousePress { pos: _, button } => {
                if *button == 1 {
                    self.base.set_mouse_pressed(true);
                }
            }
            crate::event::Event::MouseRelease { pos: _, button } => {
                if *button == 1 {
                    self.base.set_mouse_pressed(false);
                }
            }
            _ => {}
        }
    }
}
