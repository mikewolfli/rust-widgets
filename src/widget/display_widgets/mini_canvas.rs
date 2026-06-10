//! MiniCanvas widget — simplified drawing surface for mini builds (BLUE13 R2.11).
use crate::core::{Color, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::{RenderCommand, RenderContext};
use crate::signal::GenericSignal;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Simplified canvas for drawing basic shapes.
pub struct MiniCanvas {
    base: BaseWidget,
    commands: Vec<RenderCommand>,
    last_mouse_pos: Point,
    /// Emitted when a click-like interaction (press then release) is detected.
    pub clicked: GenericSignal,
    /// Emitted when the mouse button is pressed over the canvas.
    pub mouse_pressed: GenericSignal,
    /// Emitted when the mouse button is released over the canvas.
    pub mouse_released: GenericSignal,
}

impl MiniCanvas {
    /// Creates a new MiniCanvas with the given geometry.
    pub fn new(rect: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::MiniCanvas, rect, "MiniCanvas"),
            commands: Vec::new(),
            last_mouse_pos: Point::new(0, 0),
            clicked: GenericSignal::new(),
            mouse_pressed: GenericSignal::new(),
            mouse_released: GenericSignal::new(),
        }
    }

    /// Add a fill rectangle command.
    pub fn fill_rect(&mut self, rect: Rect, color: Color) {
        self.commands.push(RenderCommand::FillRect { rect, color });
    }

    /// Add a draw rectangle outline command.
    pub fn draw_rect(&mut self, rect: Rect, color: Color) {
        self.commands.push(RenderCommand::DrawRect { rect, color });
    }

    /// Add a draw line command.
    pub fn draw_line(&mut self, from: Point, to: Point, color: Color) {
        self.commands.push(RenderCommand::DrawLine { from, to, color });
    }

    /// Add a fill circle command.
    pub fn fill_circle(&mut self, center: Point, radius: u32, color: Color) {
        self.commands.push(RenderCommand::FillCircle { center, radius, color });
    }

    /// Clear all commands.
    pub fn clear(&mut self) {
        self.commands.clear();
    }

    /// Clear commands and add a fill rect for the background.
    pub fn clear_with_color(&mut self, color: Color) {
        self.commands.clear();
        let rect = self.base.geometry();
        self.commands.push(RenderCommand::FillRect { rect, color });
    }

    /// Get stored commands.
    pub fn commands(&self) -> &[RenderCommand] {
        &self.commands
    }
}

impl Widget for MiniCanvas {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        Size::new(200, 200)
    }
}

impl EventHandler for MiniCanvas {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        match event {
            Event::MousePress { pos, .. } => {
                self.last_mouse_pos = *pos;
                self.mouse_pressed.emit();
            }
            Event::MouseRelease { pos, .. } => {
                self.last_mouse_pos = *pos;
                self.mouse_released.emit();
                self.clicked.emit();
            }
            _ => {}
        }
    }
}

impl Draw for MiniCanvas {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }

        // Fill background from style.
        let bg = self.style().background_color.unwrap_or(Color::from_rgb(255, 255, 255));
        context.fill_rect(rect, bg);

        // Replay all stored commands.
        for cmd in &self.commands {
            context.execute_command(cmd.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{PaintBackend, SoftwarePaintBackend};

    #[test]
    fn mini_canvas_creation() {
        let canvas = MiniCanvas::new(Rect::new(0, 0, 200, 200));
        assert_eq!(canvas.kind(), WidgetKind::MiniCanvas);
        assert_eq!(canvas.geometry(), Rect::new(0, 0, 200, 200));
        assert!(canvas.commands().is_empty());
    }

    #[test]
    fn mini_canvas_fill_rect() {
        let mut canvas = MiniCanvas::new(Rect::new(0, 0, 200, 200));
        assert!(canvas.commands().is_empty());

        canvas.fill_rect(Rect::new(10, 10, 50, 50), Color::RED);
        assert_eq!(canvas.commands().len(), 1);

        canvas.fill_rect(Rect::new(70, 70, 30, 30), Color::BLUE);
        assert_eq!(canvas.commands().len(), 2);
    }

    #[test]
    fn mini_canvas_draw_line() {
        let mut canvas = MiniCanvas::new(Rect::new(0, 0, 200, 200));
        canvas.draw_line(Point::new(0, 0), Point::new(100, 100), Color::BLACK);
        assert_eq!(canvas.commands().len(), 1);
        match &canvas.commands()[0] {
            RenderCommand::DrawLine { from, to, color } => {
                assert_eq!(*from, Point::new(0, 0));
                assert_eq!(*to, Point::new(100, 100));
                assert_eq!(*color, Color::BLACK);
            }
            _ => panic!("Expected DrawLine command"),
        }
    }

    #[test]
    fn mini_canvas_clear() {
        let mut canvas = MiniCanvas::new(Rect::new(0, 0, 200, 200));
        canvas.fill_rect(Rect::new(10, 10, 50, 50), Color::RED);
        canvas.draw_line(Point::new(0, 0), Point::new(100, 100), Color::BLACK);
        assert_eq!(canvas.commands().len(), 2);

        canvas.clear();
        assert!(canvas.commands().is_empty());
    }

    #[test]
    fn mini_canvas_draw_no_panic() {
        let mut canvas = MiniCanvas::new(Rect::new(0, 0, 200, 200));

        let mut backend = SoftwarePaintBackend::new(Size::new(200, 200), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut context = RenderContext::new(&mut backend);
        canvas.draw(&mut context);
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert!(!rgba.is_empty());
    }

    #[test]
    fn mini_canvas_clear_with_color() {
        let mut canvas = MiniCanvas::new(Rect::new(0, 0, 200, 200));
        canvas.fill_rect(Rect::new(10, 10, 50, 50), Color::RED);
        assert_eq!(canvas.commands().len(), 1);

        canvas.clear_with_color(Color::from_rgb(200, 200, 200));
        assert_eq!(canvas.commands().len(), 1);
        match &canvas.commands()[0] {
            RenderCommand::FillRect { color, .. } => {
                assert_eq!(*color, Color::from_rgb(200, 200, 200));
            }
            _ => panic!("Expected FillRect command"),
        }
    }

    #[test]
    fn mini_canvas_fill_circle() {
        let mut canvas = MiniCanvas::new(Rect::new(0, 0, 200, 200));
        canvas.fill_circle(Point::new(100, 100), 30, Color::GREEN);
        assert_eq!(canvas.commands().len(), 1);
        match &canvas.commands()[0] {
            RenderCommand::FillCircle { center, radius, color } => {
                assert_eq!(*center, Point::new(100, 100));
                assert_eq!(*radius, 30);
                assert_eq!(*color, Color::GREEN);
            }
            _ => panic!("Expected FillCircle command"),
        }
    }
}
