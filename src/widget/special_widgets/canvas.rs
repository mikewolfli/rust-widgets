//! Canvas widget.
use crate::core::Rect;
use crate::render::RenderContext;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
/// Canvas widget for custom drawing.
pub struct Canvas {
    base: BaseWidget,
}
impl Canvas {
    /// Creates a new canvas widget.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Canvas, geometry, "Canvas"),
        }
    }
}
impl Widget for Canvas {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}
impl Draw for Canvas {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.base.geometry();
        use crate::core::Color;
        // Draw canvas background
        context.fill_rect(rect, Color::from_rgb(255, 255, 255));
        // Draw border to make canvas area visible
        context.draw_rect(rect, Color::from_rgb(200, 200, 200));
    }
}
impl crate::event::EventHandler for Canvas {
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
