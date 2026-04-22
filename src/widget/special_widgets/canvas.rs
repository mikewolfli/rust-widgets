//! Canvas widget.

use crate::core::Rect;
use crate::widget::base::{BaseWidget, Widget, WidgetKind};

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

impl crate::widget::base::Draw for Canvas {
    fn draw(&self, canvas: &mut dyn crate::render::Canvas) {
        // Default drawing implementation
        // Canvas is drawn by the renderer
    }
}

impl crate::event::EventHandler for Canvas {
    fn handle_event(&mut self, event: &crate::event::Event) -> bool {
        // Default event handling
        false
    }
}
