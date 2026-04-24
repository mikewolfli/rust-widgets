//! Custom drawing trait for widgets that want to render their own content.

use crate::render::RenderContext;
use super::Widget;

/// Custom drawing trait for widgets that want to render their own content.
/// Widgets implementing this trait can provide custom drawing logic instead of
/// relying solely on native platform rendering.
pub trait Draw {
    /// Draw the widget's content using the provided render context.
    /// This method is called when the widget needs to be repainted.
    fn draw(&mut self, context: &mut RenderContext);
    /// Returns true if this widget uses custom drawing, false for native rendering.
    /// This allows the rendering system to choose between native and custom paths.
    fn uses_custom_drawing(&self) -> bool {
        true
    }
    /// Optional: Request a redraw of the widget.
    /// Default implementation calls request_redraw() on the widget.
    fn request_custom_redraw(&self)
    where
        Self: Widget,
    {
        self.request_redraw();
    }
}
