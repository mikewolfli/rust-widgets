//! IME preedit text overlay widget — shows composition text with an underline.
//!
//! This widget displays the current IME composition string at a given position,
//! rendered with an underline style to indicate the preedit (uncommitted) state,
//! similar to how operating systems render inline IME composition text.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// IME preedit text overlay widget that renders composition text with an underline.
///
/// This widget is visually similar to a Label but draws a composition underline
/// beneath the text to indicate that it is part of an active IME composition session.
pub struct ImePreedit {
    base: BaseWidget,
    text: String,
    font: Font,
    text_color: Color,
    underline_color: Color,
    underline_thickness: u32,
}

impl ImePreedit {
    /// Creates a new ImePreedit widget at the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ImePreedit, geometry, "ImePreedit"),
            text: String::new(),
            font: Font::default(),
            text_color: Color::BLACK,
            underline_color: Color::BLACK,
            underline_thickness: 1,
        }
    }

    /// Sets the preedit composition text to display.
    pub fn set_text(&mut self, text: &str) {
        self.text = text.to_string();
        self.base.request_redraw();
    }

    /// Returns the current preedit text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Sets the font used for the preedit text.
    pub fn set_font(&mut self, font: Font) {
        self.font = font;
        self.base.request_redraw();
    }

    /// Sets the text color.
    pub fn set_text_color(&mut self, color: Color) {
        self.text_color = color;
        self.base.request_redraw();
    }

    /// Sets the underline color.
    pub fn set_underline_color(&mut self, color: Color) {
        self.underline_color = color;
        self.base.request_redraw();
    }

    /// Sets the underline thickness in pixels.
    pub fn set_underline_thickness(&mut self, thickness: u32) {
        self.underline_thickness = thickness;
        self.base.request_redraw();
    }
}

impl Widget for ImePreedit {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}

impl Draw for ImePreedit {
    fn draw(&mut self, context: &mut RenderContext) {
        if self.text.is_empty() {
            return;
        }
        let rect = self.geometry();

        // Draw the preedit text
        context.draw_text(
            Point::new(rect.x, rect.y),
            &self.text,
            &self.font,
            self.text_color,
            HorizontalAlignment::Left,
        );

        // Draw composition underline below the text
        // Estimate text width based on character count × approximate font size
        let font_size = self.font.size() as i32;
        let char_width = font_size.max(8);
        let text_width = (self.text.len() as i32) * char_width;
        let underline_y = rect.y + font_size + 2;
        let underline_x_end = rect.x + text_width.min(rect.width as i32);

        context.draw_line_stroke(
            Point::new(rect.x, underline_y),
            Point::new(underline_x_end, underline_y),
            self.underline_color,
            self.underline_thickness,
        );
    }
}

impl EventHandler for ImePreedit {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        match event {
            Event::KeyPress { key, modifiers } => {
                if *key == 8 {
                    // Backspace — remove last character
                    if !self.text.is_empty() {
                        self.text.pop();
                        self.base.request_redraw();
                    }
                } else if *key >= 32 && *key <= 126 {
                    // Printable ASCII — append to preedit text
                    let c = char::from_u32(*key).unwrap_or(' ');
                    // Respect shift for uppercase letters
                    let c = if *modifiers & 0x02 != 0 { c.to_ascii_uppercase() } else { c };
                    self.text.push(c);
                    self.base.request_redraw();
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ime_preedit_new_defaults() {
        let ip = ImePreedit::new(Rect::new(0, 0, 200, 30));
        assert_eq!(ip.text(), "");
        assert_eq!(ip.kind(), WidgetKind::ImePreedit);
    }

    #[test]
    fn ime_preedit_set_text() {
        let mut ip = ImePreedit::new(Rect::new(0, 0, 200, 30));
        ip.set_text("composition");
        assert_eq!(ip.text(), "composition");
    }

    #[test]
    fn ime_preedit_set_text_empty() {
        let mut ip = ImePreedit::new(Rect::new(0, 0, 200, 30));
        ip.set_text("hello");
        ip.set_text("");
        assert_eq!(ip.text(), "");
    }

    #[test]
    fn ime_preedit_set_font() {
        let mut ip = ImePreedit::new(Rect::new(0, 0, 200, 30));
        let font = Font::default();
        ip.set_font(font);
        // No panic expected
    }

    #[test]
    fn ime_preedit_set_colors() {
        let mut ip = ImePreedit::new(Rect::new(0, 0, 200, 30));
        ip.set_text_color(Color::RED);
        ip.set_underline_color(Color::BLUE);
    }

    #[test]
    fn ime_preedit_underline_thickness() {
        let mut ip = ImePreedit::new(Rect::new(0, 0, 200, 30));
        ip.set_underline_thickness(3);
    }

    #[test]
    fn ime_preedit_draw_empty_text() {
        let mut ip = ImePreedit::new(Rect::new(0, 0, 200, 30));
        // Should not crash when drawing empty text
        let svg = crate::widget::svg::render_to_svg(&mut ip);
        assert!(svg.starts_with("<svg"));
    }
}
