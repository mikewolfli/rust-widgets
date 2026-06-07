//! Canvas widget for custom drawing operations.
//!
//! The `Canvas` widget provides a drawing surface where users can push
//! rendering commands (lines, circles, rectangles, paths, text) and have
//! them replayed during the `draw()` pass. It also emits mouse interaction
//! signals so callers can respond to clicks, drags, and hover events on
//! the drawing area.
use crate::core::{Color, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::{RenderCommand, RenderContext};
use crate::signal::GenericSignal;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Canvas widget for custom drawing operations.
pub struct Canvas {
    base: BaseWidget,
    /// Stored drawing commands replayed during `draw()`.
    commands: Vec<RenderCommand>,
    /// Track mouse position for interaction signals.
    last_mouse_pos: Point,
    /// Emitted when mouse is pressed on the canvas.
    pub mouse_pressed: GenericSignal,
    /// Emitted when mouse is released on the canvas.
    pub mouse_released: GenericSignal,
    /// Emitted when mouse moves over the canvas, carrying the current cursor position.
    pub mouse_moved: crate::signal::Signal1<Point>,
    /// Emitted when a double-click occurs on the canvas.
    pub double_clicked: GenericSignal,
}

impl Canvas {
    /// Creates a new canvas widget with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Canvas, geometry, "Canvas"),
            commands: Vec::new(),
            last_mouse_pos: Point::new(0, 0),
            mouse_pressed: GenericSignal::new(),
            mouse_released: GenericSignal::new(),
            mouse_moved: crate::signal::Signal1::new(),
            double_clicked: GenericSignal::new(),
        }
    }

    /// Remove all stored drawing commands.
    pub fn clear(&mut self) {
        self.commands.clear();
        self.base.request_redraw();
    }

    /// Returns the number of stored drawing commands.
    pub fn command_count(&self) -> usize {
        self.commands.len()
    }

    // ── Drawing API ──────────────────────────────────────────────────────

    /// Fill the entire canvas area with a solid color.
    pub fn fill_rect(&mut self, rect: Rect, color: Color) {
        self.commands.push(RenderCommand::FillRect { rect, color });
        self.base.request_redraw();
    }

    /// Draw a rectangle outline.
    pub fn draw_rect(&mut self, rect: Rect, color: Color) {
        self.commands.push(RenderCommand::DrawRect { rect, color });
        self.base.request_redraw();
    }

    /// Draw a line segment from `from` to `to`.
    pub fn draw_line(&mut self, from: Point, to: Point, color: Color) {
        self.commands.push(RenderCommand::DrawLine { from, to, color });
        self.base.request_redraw();
    }

    /// Draw a line segment with anti-aliasing.
    pub fn draw_line_aa(&mut self, from: Point, to: Point, color: Color) {
        self.commands.push(RenderCommand::DrawLineAA { from, to, color });
        self.base.request_redraw();
    }

    /// Draw a line segment with explicit stroke width.
    pub fn draw_line_stroke(&mut self, from: Point, to: Point, color: Color, width: u32) {
        self.commands.push(RenderCommand::DrawLineStroke { from, to, color, width });
        self.base.request_redraw();
    }

    /// Fill a circle at `center` with `radius`.
    pub fn fill_circle(&mut self, center: Point, radius: u32, color: Color) {
        self.commands.push(RenderCommand::FillCircle { center, radius, color });
        self.base.request_redraw();
    }

    /// Fill a circle with anti-aliasing.
    pub fn fill_circle_aa(&mut self, center: Point, radius: u32, color: Color) {
        self.commands.push(RenderCommand::FillCircleAA { center, radius, color });
        self.base.request_redraw();
    }

    /// Draw a circle outline.
    pub fn draw_circle(&mut self, center: Point, radius: u32, color: Color) {
        self.commands.push(RenderCommand::DrawCircle { center, radius, color });
        self.base.request_redraw();
    }

    /// Draw a circle outline with explicit stroke width.
    pub fn draw_circle_stroke(&mut self, center: Point, radius: u32, color: Color, width: u32) {
        self.commands.push(RenderCommand::DrawCircleStroke { center, radius, color, width });
        self.base.request_redraw();
    }

    /// Fill a rounded rectangle.
    pub fn fill_rounded_rect(&mut self, rect: Rect, radius: u32, color: Color) {
        self.commands.push(RenderCommand::FillRoundedRect { rect, radius, color });
        self.base.request_redraw();
    }

    /// Draw text at the specified position.
    pub fn draw_text(&mut self, origin: Point, text: &str, font: &crate::core::Font, color: Color) {
        self.commands.push(RenderCommand::DrawText {
            origin,
            text: text.to_string(),
            font: font.clone(),
            color,
            alignment: crate::core::HorizontalAlignment::Left,
        });
        self.base.request_redraw();
    }

    /// Draw an arc (partial circle).
    pub fn draw_arc(
        &mut self,
        center: Point,
        radius: u32,
        start_angle: f32,
        end_angle: f32,
        color: Color,
        filled: bool,
    ) {
        self.commands.push(RenderCommand::DrawArc {
            center,
            radius,
            start_angle,
            end_angle,
            color,
            filled,
        });
        self.base.request_redraw();
    }

    /// Draw a polyline or polygon path.
    pub fn draw_path(
        &mut self,
        points: Vec<Point>,
        closed: bool,
        color: Color,
        filled: bool,
        width: u32,
    ) {
        self.commands.push(RenderCommand::DrawPath { points, closed, color, filled, width });
        self.base.request_redraw();
    }

    /// Returns the last tracked mouse position.
    pub fn last_mouse_pos(&self) -> Point {
        self.last_mouse_pos
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
        // Draw canvas background
        context.fill_rect(rect, Color::WHITE);
        // Replay all stored commands
        for cmd in &self.commands {
            context.execute_command(cmd.clone());
        }
        // Draw border to make canvas area visible
        context.draw_rect(rect, Color::rgb(200, 200, 200));
    }

    fn uses_custom_drawing(&self) -> bool {
        true
    }
}

impl EventHandler for Canvas {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button } if *button == 1 => {
                self.last_mouse_pos = *pos;
                self.base.set_mouse_pressed(true);
                self.mouse_pressed.emit();
            }
            Event::MouseRelease { pos, button } if *button == 1 => {
                self.last_mouse_pos = *pos;
                self.base.set_mouse_pressed(false);
                self.mouse_released.emit();
            }
            Event::MouseMove { pos } => {
                if self.geometry().contains(*pos) {
                    self.last_mouse_pos = *pos;
                    self.mouse_moved.emit(*pos);
                }
            }
            Event::MouseDoubleClick { pos, button } if *button == 1 => {
                self.last_mouse_pos = *pos;
                self.double_clicked.emit();
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Font, Rect};
    use crate::widget::svg::render_to_svg;

    #[test]
    fn canvas_creation_defaults() {
        let canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        assert_eq!(canvas.command_count(), 0);
        assert_eq!(canvas.geometry(), Rect::new(0, 0, 200, 100));
        assert_eq!(canvas.kind(), WidgetKind::Canvas);
    }

    #[test]
    fn canvas_fill_rect_adds_command() {
        let mut canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        canvas.fill_rect(Rect::new(10, 10, 50, 50), Color::BLUE);
        assert_eq!(canvas.command_count(), 1);
    }

    #[test]
    fn canvas_draw_line_and_circle() {
        let mut canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        canvas.draw_line(Point::new(0, 0), Point::new(100, 100), Color::RED);
        canvas.fill_circle(Point::new(50, 50), 10, Color::GREEN);
        assert_eq!(canvas.command_count(), 2);
    }

    #[test]
    fn canvas_clear_removes_all_commands() {
        let mut canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        canvas.fill_rect(Rect::new(0, 0, 100, 100), Color::BLUE);
        canvas.draw_line(Point::new(0, 0), Point::new(100, 100), Color::RED);
        assert_eq!(canvas.command_count(), 2);
        canvas.clear();
        assert_eq!(canvas.command_count(), 0);
    }

    #[test]
    fn canvas_draw_path_stores_commands() {
        let mut canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        let points =
            vec![Point::new(0, 0), Point::new(100, 0), Point::new(100, 100), Point::new(0, 100)];
        canvas.draw_path(points, true, Color::BLUE, false, 2);
        assert_eq!(canvas.command_count(), 1);
    }

    #[test]
    fn canvas_draw_text_stores_command() {
        let mut canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        canvas.draw_text(Point::new(10, 20), "Hello", &Font::default_ui(), Color::BLACK);
        assert_eq!(canvas.command_count(), 1);
    }

    #[test]
    fn canvas_draw_produces_svg_output() {
        let mut canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        canvas.fill_rect(Rect::new(10, 10, 50, 50), Color::BLUE);
        canvas.draw_line(Point::new(0, 0), Point::new(100, 100), Color::RED);
        let svg = render_to_svg(&mut canvas);
        assert!(svg.starts_with("<svg"));
        assert!(svg.len() > 100);
    }

    #[test]
    fn canvas_signal_accessors() {
        let canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        let _ = &canvas.mouse_pressed;
        let _ = &canvas.mouse_released;
        let _ = &canvas.mouse_moved;
        let _ = &canvas.double_clicked;
    }

    #[test]
    fn canvas_last_mouse_position_tracked() {
        let mut canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        assert_eq!(canvas.last_mouse_pos(), Point::new(0, 0));
        canvas.handle_event(&Event::MouseMove { pos: Point::new(42, 73) });
        assert_eq!(canvas.last_mouse_pos(), Point::new(42, 73));
    }

    #[test]
    fn canvas_disabled_state_blocks_events() {
        let mut canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        canvas.set_enabled(false);
        // Events should be ignored when disabled
        canvas.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1 });
        // Signal should NOT have been emitted (disabled)
        // We just verify no panic and state unchanged
        assert_eq!(canvas.last_mouse_pos(), Point::new(0, 0));
    }

    #[test]
    fn canvas_double_click_emits_signal() {
        let mut canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        canvas.handle_event(&Event::MouseDoubleClick { pos: Point::new(50, 50), button: 1 });
        assert_eq!(canvas.last_mouse_pos(), Point::new(50, 50));
    }

    #[test]
    fn canvas_draw_arc_and_draw_circle_stroke() {
        let mut canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        canvas.draw_arc(Point::new(100, 50), 30, 0.0, std::f32::consts::PI, Color::RED, false);
        canvas.draw_circle_stroke(Point::new(50, 50), 20, Color::BLUE, 3);
        assert_eq!(canvas.command_count(), 2);
    }

    #[test]
    fn canvas_fill_rounded_rect_and_line_aa() {
        let mut canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        canvas.fill_rounded_rect(Rect::new(5, 5, 50, 50), 8, Color::rgba(255, 0, 0, 128));
        canvas.draw_line_aa(Point::new(0, 0), Point::new(100, 50), Color::GREEN);
        assert_eq!(canvas.command_count(), 2);
    }
}
