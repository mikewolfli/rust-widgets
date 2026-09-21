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
    /// Whether the caller ever chose a text colour through [`ImePreedit::set_text_color`].
    ///
    /// The field is only ever *written* by that setter, so its constructed value cannot be told
    /// from a caller's write by looking at the field alone. The flag records the difference, which
    /// is what lets `draw` fall back to the theme's foreground for a colour nobody chose instead of
    /// keeping the black the constructor happened to install.
    text_color_set: bool,
    /// Whether the caller ever chose an underline colour through
    /// [`ImePreedit::set_underline_color`]. Same reason as [`Self::text_color_set`].
    underline_color_set: bool,
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
            text_color_set: false,
            underline_color_set: false,
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
        self.text_color_set = true;
        self.base.request_redraw();
    }

    /// Sets the underline color.
    pub fn set_underline_color(&mut self, color: Color) {
        self.underline_color = color;
        self.underline_color_set = true;
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

    /// The text colour `draw` should actually use.
    ///
    /// Precedence is the crate-wide one: a colour the caller set wins, otherwise the caller's
    /// style colour (theme-applied or explicit), otherwise the theme's foreground passed in as
    /// `fallback`. The field's constructed `Color::BLACK` is deliberately not consulted, because
    /// it is a constructor default rather than a choice, and treating it as one would keep the
    /// control black on a dark theme.
    fn effective_text_color(&self, fallback: Color) -> Color {
        if self.text_color_set {
            return self.text_color;
        }
        self.base.style().text_color.unwrap_or(fallback)
    }

    /// The underline colour `draw` should actually use. Same precedence as
    /// [`Self::effective_text_color`].
    fn effective_underline_color(&self, fallback: Color) -> Color {
        if self.underline_color_set {
            return self.underline_color;
        }
        self.base.style().border_color.unwrap_or(fallback)
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

        // Chrome colours resolve the explicit style first, then the theme's resolved style for
        // this control, and only then fall back to a literal. The widget's own field defaults were
        // both `Color::BLACK`, so a light/dark switch left the composition text and its underline
        // unchanged — the rendering census reported the control as theme-blind.
        //
        // `ime_preedit` classifies as `WidgetRole::Text`, whose resolved background is `None`;
        // the theme's own `background` is read instead and tinted, which is the surface the text
        // would sit on in a real edit host. The theme read is a separate manager lock, taken and
        // released inside the accessor, so it is not held across the draw — the global manager's
        // mutex is not re-entrant.
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("ime_preedit");
        // Read as its own lock acquisition and copied out as values, so the guard is dropped
        // before anything else touches the theme.
        let (window_fill, foreground) = {
            let manager = crate::style::theme_manager();
            match manager.current_theme() {
                Some(active) => (active.colors.background, active.colors.foreground),
                None => (Color::rgb(240, 240, 240), Color::BLACK),
            }
        };

        // The surface the composition sits on: one step from the window fill toward the text
        // colour, so the preedit is a distinct element on a light theme and on a dark one rather
        // than a bare black on black. A caller's own colour still wins.
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(foreground);
        let surface = window_fill.blend(&ink, 0.08);
        context.fill_rect(rect, surface);

        // The preedit text: a caller's own colour wins, then the theme's foreground, and the
        // widget's constructed default is dropped in favour of that foreground rather than a
        // fixed black the theme cannot override.
        let text_color = self.effective_text_color(ink);
        // Draw the preedit text, fitted to the control's own width.
        //
        // The composition string is authored by the input method, so the control neither
        // chooses nor bounds its length: a long candidate run advanced `font.size()` pixels
        // per character straight past the control's right edge. Drawing it at the origin
        // clipped it on the raster backends but left it in the SVG snapshot as text leaving
        // the picture, and the underline below was sized from the *unfitted* string, so the
        // rule that marks the composition pointed somewhere the text no longer reached.
        // `draw_text_fitted` returns what it drew, which is what keeps the underline under
        // the glyphs that are actually on screen.
        let fitted = context.draw_text_fitted(
            rect,
            &self.text,
            &self.font,
            text_color,
            HorizontalAlignment::Left,
        );

        // Draw composition underline below the text. The advance is measured from the drawn
        // string rather than estimated as `len() × font.size()`: the two differ once the text
        // is truncated, and the estimate also overstated every non-ASCII cluster.
        let font_size = self.font.size() as i32;
        let text_width = context.measure_text(&fitted, &self.font).width as i32;
        let underline_y = rect.y + font_size + 2;
        let underline_x_end = (rect.x + text_width).min(rect.x + rect.width as i32);

        // The underline marks the composition as uncommitted, so it carries the theme's primary
        // when the caller has not chosen one of its own.
        let underline_color = self.effective_underline_color(
            crate::style::semantic_color(crate::style::SemanticColor::Info).unwrap_or(ink),
        );
        context.draw_line_stroke(
            Point::new(rect.x, underline_y),
            Point::new(underline_x_end, underline_y),
            underline_color,
            self.underline_thickness,
        );
    }
}

impl EventHandler for ImePreedit {
    /// Typing edits the preedit buffer only while the control is enabled.
    ///
    /// The disabled state was not consulted, so a host that disables the control — the usual way
    /// to take a text entry out of play — still had characters appended to it by keystrokes meant
    /// for whatever the user was actually looking at.
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
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
    /// A disabled preedit control ignores keystrokes.
    ///
    /// The handler never consulted `enabled`, so a host that disabled the control — the usual way
    /// to take a text entry out of play — still had characters appended to it by keystrokes meant
    /// for whatever the user was actually looking at.
    #[test]
    fn ime_preedit_ignores_keys_when_disabled() {
        use crate::event::Event;
        use crate::widget::Widget;

        let mut preedit = ImePreedit::new(Rect::new(0, 0, 200, 30));
        preedit.handle_event(&Event::KeyPress { key: 65, modifiers: 0 }); // 'A'
        assert_eq!(preedit.text(), "A", "an enabled control accepts typed text");

        preedit.set_enabled(false);
        preedit.handle_event(&Event::KeyPress { key: 66, modifiers: 0 }); // 'B'
        assert_eq!(preedit.text(), "A", "a disabled control must not accept text");

        // Backspace is refused too, so the buffer is frozen rather than editable one way.
        preedit.set_enabled(true);
        preedit.handle_event(&Event::KeyPress { key: 67, modifiers: 0 }); // 'C'
        assert_eq!(preedit.text(), "AC");
        preedit.set_enabled(false);
        preedit.handle_event(&Event::KeyPress { key: 8, modifiers: 0 }); // backspace
        assert_eq!(preedit.text(), "AC", "a disabled control must not accept backspace either");
    }
}
