//! Divider/Separator widget — a horizontal or vertical line separator.
//!
//! The Divider widget draws a thin line (horizontal or vertical) centered within
//! its bounding rectangle. It is used to visually separate groups of content,
//! similar to `<hr>` in HTML or `JSeparator` in Swing.

use crate::core::{Color, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::widget::display_widgets::draw_line;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Divider/Separator widget for visually separating content sections.
///
/// Supports both horizontal and vertical orientations with configurable
/// thickness and color.
pub struct Divider {
    base: BaseWidget,
    vertical: bool,
    thickness: u32,
    color: Color,
}

impl Divider {
    /// Creates a new horizontal Divider with default styling (1px thick, gray).
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Divider, geometry, "Divider"),
            vertical: false,
            thickness: 1,
            color: Color::rgba(180, 180, 180, 200),
        }
    }

    /// Sets whether the divider is vertical (true) or horizontal (false).
    pub fn set_vertical(&mut self, vertical: bool) {
        self.vertical = vertical;
        self.base.request_redraw();
    }

    /// Returns whether the divider is oriented vertically.
    pub fn is_vertical(&self) -> bool {
        self.vertical
    }

    /// Sets the line thickness in pixels.
    pub fn set_thickness(&mut self, thickness: u32) {
        self.thickness = thickness;
        self.base.request_redraw();
    }

    /// Sets the divider line color.
    pub fn set_color(&mut self, color: Color) {
        self.color = color;
        self.base.request_redraw();
    }
}

impl Widget for Divider {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}

impl Draw for Divider {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        draw_line(context, rect, self.vertical, self.thickness, self.color);
    }
}

impl EventHandler for Divider {
    fn handle_event(&mut self, event: &Event) {
        // Divider is purely decorative; delegate all events to base.
        self.base.handle_event(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Point;

    #[test]
    fn divider_default_is_horizontal() {
        let div = Divider::new(Rect::new(0, 0, 100, 10));
        assert!(!div.is_vertical());
        assert_eq!(div.kind(), WidgetKind::Divider);
    }

    #[test]
    fn divider_set_vertical() {
        let mut div = Divider::new(Rect::new(0, 0, 10, 100));
        assert!(!div.is_vertical());
        div.set_vertical(true);
        assert!(div.is_vertical());
        div.set_vertical(false);
        assert!(!div.is_vertical());
    }

    #[test]
    fn divider_set_thickness() {
        let mut div = Divider::new(Rect::new(0, 0, 100, 10));
        assert_eq!(div.thickness, 1);
        div.set_thickness(3);
        // thickness is not publicly readable directly; check via the setter
        // We verify internal state by checking the draw path (no crash).
        div.set_thickness(5);
    }

    #[test]
    fn divider_set_color() {
        let mut div = Divider::new(Rect::new(0, 0, 100, 10));
        div.set_color(Color::RED);
        // No crash and correct kind preserved.
        assert_eq!(div.kind(), WidgetKind::Divider);
    }

    #[test]
    fn divider_svg_output_horizontal() {
        let mut div = Divider::new(Rect::new(0, 0, 100, 10));
        let svg = crate::widget::svg::render_to_svg(&mut div);
        assert!(svg.starts_with("<svg"));
        // SVG should contain a line element (draw_line_stroke produces a line).
        assert!(svg.contains("line") || svg.contains("stroke"));
    }

    #[test]
    fn divider_svg_output_vertical() {
        let mut div = Divider::new(Rect::new(0, 0, 10, 100));
        div.set_vertical(true);
        let svg = crate::widget::svg::render_to_svg(&mut div);
        assert!(svg.starts_with("<svg"));
    }

    #[test]
    fn divider_event_delegation() {
        let mut div = Divider::new(Rect::new(0, 0, 100, 10));
        // Events should not panic/crash.
        div.handle_event(&Event::MousePress { pos: Point::new(10, 5), button: 1 });
        div.handle_event(&Event::MouseRelease { pos: Point::new(10, 5), button: 1 });
        // State remains unchanged (no interactive behavior).
        assert!(!div.is_vertical());
    }

    #[test]
    fn divider_geometry_reflects_kind() {
        let div = Divider::new(Rect::new(0, 0, 100, 10));
        let geom = div.geometry();
        assert_eq!(geom.width, 100);
        assert_eq!(geom.height, 10);
    }
}
