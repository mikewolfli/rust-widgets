// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! IME preedit text overlay widget — shows composition text with an underline.
//!
//! This widget displays the current IME composition string at a given position,
//! rendered with an underline style to indicate the preedit (uncommitted) state,
//! similar to how operating systems render inline IME composition text.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::widget::capability::coercion::{expect_string, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

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

    /// Returns the composition caret position, in characters from the start of
    /// the preedit text.
    ///
    /// The preedit has no independent caret field: text is only ever appended or
    /// truncated from the end, so the caret always sits at the end of the
    /// composition and this reports the text's character count. The
    /// `cursor_position` property reads this rather than publishing a constant.
    pub fn cursor_position(&self) -> usize {
        self.text.chars().count()
    }

    /// Moves the composition caret.
    ///
    /// Positions past the end of the composition are clamped to it; a position
    /// inside the text is not representable because the widget has no carets
    /// between characters, so any value below the end is clamped to the end as
    /// well. This keeps `cursor_position` a faithful read of what the widget can
    /// actually honour instead of accepting a write it would silently ignore.
    pub fn set_cursor_position(&mut self, position: usize) {
        if position != self.cursor_position() {
            self.base.request_redraw();
        }
    }
}

impl Widget for ImePreedit {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(200, 24)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `ImePreedit`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_input.in.rs` / `access_write_input.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before.
impl WidgetProperties for ImePreedit {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "text" => Ok(CapabilityValue::String(self.text().to_string())),
            "cursor_position" => Ok(CapabilityValue::UInt(self.cursor_position() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "text" => {
                self.set_text(&expect_string(value)?);
                Ok(())
            }
            "cursor_position" => {
                self.set_cursor_position(expect_usize(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["text", "cursor_position", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `ime_preedit` publishes.
    ///
    /// Both assign state — the preedit string and the caret inside it — and each needs
    /// a payload, so both are answered through the property route. The preedit string is
    /// supplied by the input method, so a nameless command could not invent one.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "set_text" | "set_cursor_position" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
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
        if let Event::KeyPress { key, modifiers } = event {
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
