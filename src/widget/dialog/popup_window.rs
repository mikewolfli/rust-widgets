//! Popup window widget.
use crate::core::Rect;
use crate::widget::base::{BaseWidget, Widget, WidgetKind};
use crate::render::RenderContext;
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
impl crate::widget::base::Draw for PopupWindow {
    fn draw(&mut self, context: &mut RenderContext) {
        // Default drawing implementation
        // Popup window is drawn by the renderer
    }
}
impl crate::event::EventHandler for PopupWindow {
    fn handle_event(&mut self, event: &crate::event::Event) {
        // Default event handling
        let _ = event;
    }
}
