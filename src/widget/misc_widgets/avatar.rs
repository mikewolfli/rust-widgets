//! Avatar widget — a circular or square user avatar/image placeholder with initials fallback.
//!
//! The Avatar widget displays a colored circle (or rounded square) with centered
//! initials text, commonly used for user profile pictures, contact avatars, and
//! identity placeholders in modern UI design.

use crate::core::{Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Avatar widget for displaying user profile images or initials-based placeholders.
///
/// By default the avatar renders as a filled circle with the background color and
/// white bold initials centered inside. Setting `square` to `true` produces a
/// rounded rectangle instead.
pub struct Avatar {
    base: BaseWidget,
    /// Initials text displayed inside the avatar (typically 1-2 characters).
    text: String,
    /// Background fill color of the avatar.
    bg_color: Color,
    /// When `true`, renders as a rounded square instead of a circle.
    square: bool,
    /// Diameter (circle) or side length (square) in logical pixels.
    size: u32,
}

impl Avatar {
    /// Creates a new Avatar widget with the given geometry.
    ///
    /// Defaults to a 40×40 circle with [`Color::PRIMARY`] background and empty initials.
    /// If `geometry` has zero width or height, a default 40×40 size is used.
    pub fn new(geometry: Rect) -> Self {
        let sz = if geometry.width > 0 && geometry.height > 0 {
            geometry.width.min(geometry.height)
        } else {
            40
        };
        Self {
            base: BaseWidget::new(
                WidgetKind::Avatar,
                Rect::new(geometry.x, geometry.y, sz, sz),
                "Avatar",
            ),
            text: String::new(),
            bg_color: Color::PRIMARY,
            square: false,
            size: sz,
        }
    }

    /// Sets the initials text displayed inside the avatar.
    pub fn set_text(&mut self, text: &str) {
        self.text = text.to_string();
        self.base.request_redraw();
    }

    /// Returns the current initials text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Sets whether the avatar renders as a rounded square (instead of a circle).
    pub fn set_square(&mut self, square: bool) {
        self.square = square;
        self.base.request_redraw();
    }

    /// Returns `true` if the avatar renders as a rounded square.
    pub fn is_square(&self) -> bool {
        self.square
    }

    /// Sets the background fill color of the avatar.
    pub fn set_bg_color(&mut self, color: Color) {
        self.bg_color = color;
        self.base.request_redraw();
    }

    /// Returns the current background fill color.
    pub fn bg_color(&self) -> Color {
        self.bg_color
    }

    /// Sets the diameter (circle) or side length (square) of the avatar.
    /// Updates the widget geometry to match.
    pub fn set_size(&mut self, size: u32) {
        self.size = size;
        let rect = self.geometry();
        self.set_geometry(Rect::new(rect.x, rect.y, size, size));
        self.base.request_redraw();
    }

    /// Returns the current diameter or side length of the avatar.
    pub fn size(&self) -> u32 {
        self.size
    }
}

impl Widget for Avatar {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}

impl Draw for Avatar {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let size = rect.width.min(rect.height);
        let center = Point::new(rect.x + (size as i32) / 2, rect.y + (size as i32) / 2);

        // Draw the avatar shape (circle or rounded square)
        if self.square {
            let corner_radius = size / 4;
            context.fill_rounded_rect(rect, corner_radius, self.bg_color);
        } else {
            context.fill_circle(center, size / 2, self.bg_color);
        }

        // Draw centered initials text
        if !self.text.is_empty() {
            // Use a bold font sized relative to the avatar size
            let font_size = (size as f32 * 0.45).max(8.0);
            let font = Font::bold("Arial", font_size);

            let metrics = context.measure_text(&self.text, &font);
            let text_width = metrics.width as i32;
            let text_height = metrics.ascent as i32 + metrics.descent as i32;

            let text_x = rect.x + (rect.width as i32 - text_width) / 2;
            let text_y = rect.y + (rect.height as i32 - text_height) / 2 + metrics.ascent as i32;

            context.draw_text(
                Point::new(text_x.max(rect.x), text_y.max(rect.y)),
                &self.text,
                &font,
                Color::WHITE,
            );
        }
    }
}

impl EventHandler for Avatar {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Point;
    use crate::widget::WidgetKind;

    #[test]
    fn avatar_default_state() {
        let avatar = Avatar::new(Rect::new(0, 0, 40, 40));
        assert_eq!(avatar.text(), "");
        assert!(!avatar.is_square());
        assert_eq!(avatar.bg_color(), Color::PRIMARY);
        assert_eq!(avatar.size(), 40);
        assert_eq!(avatar.kind(), WidgetKind::Avatar);
    }

    #[test]
    fn avatar_default_size_when_zero_geometry() {
        let avatar = Avatar::new(Rect::new(10, 20, 0, 0));
        assert_eq!(avatar.size(), 40);
        assert_eq!(avatar.geometry().width, 40);
        assert_eq!(avatar.geometry().height, 40);
    }

    #[test]
    fn avatar_set_text() {
        let mut avatar = Avatar::new(Rect::new(0, 0, 40, 40));
        avatar.set_text("JD");
        assert_eq!(avatar.text(), "JD");
    }

    #[test]
    fn avatar_set_text_clears() {
        let mut avatar = Avatar::new(Rect::new(0, 0, 40, 40));
        avatar.set_text("AB");
        avatar.set_text("");
        assert_eq!(avatar.text(), "");
    }

    #[test]
    fn avatar_set_bg_color() {
        let mut avatar = Avatar::new(Rect::new(0, 0, 40, 40));
        let custom = Color::rgb(255, 0, 0);
        avatar.set_bg_color(custom);
        assert_eq!(avatar.bg_color(), custom);
    }

    #[test]
    fn avatar_set_square() {
        let mut avatar = Avatar::new(Rect::new(0, 0, 40, 40));
        assert!(!avatar.is_square());
        avatar.set_square(true);
        assert!(avatar.is_square());
        avatar.set_square(false);
        assert!(!avatar.is_square());
    }

    #[test]
    fn avatar_set_size() {
        let mut avatar = Avatar::new(Rect::new(0, 0, 40, 40));
        avatar.set_size(64);
        assert_eq!(avatar.size(), 64);
        assert_eq!(avatar.geometry().width, 64);
        assert_eq!(avatar.geometry().height, 64);
    }

    #[test]
    fn avatar_set_size_preserves_position() {
        let mut avatar = Avatar::new(Rect::new(10, 20, 40, 40));
        avatar.set_size(56);
        assert_eq!(avatar.geometry().x, 10);
        assert_eq!(avatar.geometry().y, 20);
        assert_eq!(avatar.geometry().width, 56);
        assert_eq!(avatar.geometry().height, 56);
    }

    #[test]
    fn avatar_widget_trait_geometry() {
        let mut avatar = Avatar::new(Rect::new(5, 10, 48, 48));
        assert_eq!(avatar.geometry(), Rect::new(5, 10, 48, 48));
        avatar.set_geometry(Rect::new(0, 0, 64, 64));
        assert_eq!(avatar.geometry(), Rect::new(0, 0, 64, 64));
    }

    #[test]
    fn avatar_widget_trait_kind() {
        let avatar = Avatar::new(Rect::new(0, 0, 40, 40));
        assert_eq!(avatar.kind(), WidgetKind::Avatar);
    }

    #[test]
    fn avatar_handle_event_no_panic() {
        let mut avatar = Avatar::new(Rect::new(0, 0, 40, 40));
        // EventHandler should not panic for any event type
        avatar.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1 });
        avatar.handle_event(&Event::MouseRelease { pos: Point::new(10, 10), button: 1 });
        avatar.handle_event(&Event::MouseMove { pos: Point::new(20, 20) });
        avatar.handle_event(&Event::KeyPress { key: 0x41, modifiers: 0 });
        avatar.handle_event(&Event::KeyRelease { key: 0x41, modifiers: 0 });
    }

    #[test]
    fn avatar_disabled_blocks_events() {
        let mut avatar = Avatar::new(Rect::new(0, 0, 40, 40));
        avatar.set_enabled(false);
        // Should not panic
        avatar.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1 });
        avatar.handle_event(&Event::MouseRelease { pos: Point::new(10, 10), button: 1 });
    }

    #[test]
    fn avatar_svg_output() {
        let mut avatar = Avatar::new(Rect::new(0, 0, 40, 40));
        avatar.set_text("MW");
        let svg = crate::widget::svg::render_to_svg(&mut avatar);
        assert!(svg.starts_with("<svg"), "SVG output should start with <svg, got: {svg:.80}");
    }

    #[test]
    fn avatar_square_svg_output() {
        let mut avatar = Avatar::new(Rect::new(0, 0, 48, 48));
        avatar.set_text("AB");
        avatar.set_square(true);
        let svg = crate::widget::svg::render_to_svg(&mut avatar);
        assert!(
            svg.starts_with("<svg"),
            "Square SVG output should start with <svg, got: {svg:.80}"
        );
    }

    #[test]
    fn avatar_different_sizes() {
        let small = Avatar::new(Rect::new(0, 0, 24, 24));
        assert_eq!(small.size(), 24);

        let large = Avatar::new(Rect::new(0, 0, 96, 96));
        assert_eq!(large.size(), 96);
    }

    #[test]
    fn avatar_size_min_of_width_and_height() {
        let avatar = Avatar::new(Rect::new(0, 0, 60, 40));
        // size should be min(60, 40) = 40
        assert_eq!(avatar.size(), 40);
        assert_eq!(avatar.geometry().width, 40);
        assert_eq!(avatar.geometry().height, 40);
    }
}
