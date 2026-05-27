//! Popup window widget.
use crate::core::{ObjectId, Rect};
use crate::render::RenderContext;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
/// Popup window widget.
pub struct PopupWindow {
    base: BaseWidget,
    content_widget: Option<ObjectId>,
}
impl PopupWindow {
    /// Creates a popup window with geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::PopupWindow, geometry, "PopupWindow"),
            content_widget: None,
        }
    }
    /// Returns the content widget ID, if any.
    pub fn content_widget(&self) -> Option<ObjectId> {
        self.content_widget
    }

    /// Sets the content widget for this popup.
    pub fn set_content_widget(&mut self, widget: Option<ObjectId>) {
        if let Some(old) = self.content_widget {
            self.base.remove_child(old);
        }
        self.content_widget = widget;
        if let Some(id) = widget {
            self.base.add_child(id);
        }
        self.base.request_redraw();
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
            crate::event::Event::MousePress { pos: _, button } if *button == 1 => {
                self.base.set_mouse_pressed(true);
            }
            crate::event::Event::MouseRelease { pos: _, button } if *button == 1 => {
                self.base.set_mouse_pressed(false);
            }
            _ => {}
        }
    }
}
