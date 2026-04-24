//! Grid widget.
use crate::core::Rect;
use crate::render::RenderContext;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
/// Grid widget for layout management.
pub struct GridWidget {
    base: BaseWidget,
}
impl GridWidget {
    /// Creates a new grid widget.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Grid, geometry, "GridWidget"),
        }
    }
}
impl Widget for GridWidget {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}
impl Draw for GridWidget {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.base.geometry();
        use crate::core::Color;
        // Draw grid background
        context.fill_rect(rect, Color::from_rgb(255, 255, 255));
        // Draw border to make grid area visible
        context.draw_rect(rect, Color::from_rgb(200, 200, 200));
    }
}
impl crate::event::EventHandler for GridWidget {
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
