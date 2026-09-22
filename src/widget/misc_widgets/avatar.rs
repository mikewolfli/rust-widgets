// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Avatar widget — a circular or square user avatar/image placeholder with initials fallback.
//!
//! The Avatar widget displays a colored circle (or rounded square) with centered
//! initials text, commonly used for user profile pictures, contact avatars, and
//! identity placeholders in modern UI design.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::widget::capability::coercion::expect_string;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Avatar widget for displaying user profile images or initials-based placeholders.
///
/// By default the avatar renders as a filled circle with the background color and
/// white bold initials centered inside. Setting `square` to `true` produces a
/// rounded rectangle instead.
pub struct Avatar {
    base: BaseWidget,
    /// Initials text displayed inside the avatar (typically 1-2 characters).
    text: String,
    /// Optional source path or URL of the avatar image.
    ///
    /// The widget has no image pipeline yet, so this is carried as state and
    /// reported through the `image_source` property; the initials remain the
    /// rendered fallback until a loader is wired in.
    image_source: String,
    /// Background fill color of the avatar.
    bg_color: Color,
    /// Whether the fill was chosen by the caller through [`Avatar::set_bg_color`].
    ///
    /// The caller's choice must win over the active theme, but the colour the
    /// constructor seeds must not: without this flag a theme switch could never
    /// reach an avatar that was never explicitly coloured, which is exactly the
    /// hardcoded-chrome defect the rendering census reports.
    bg_color_is_explicit: bool,
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
            image_source: String::new(),
            bg_color: Color::PRIMARY,
            bg_color_is_explicit: false,
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

    /// Returns the configured avatar image source, or an empty string when the
    /// avatar falls back to its initials.
    pub fn image_source(&self) -> &str {
        &self.image_source
    }

    /// Sets the source path or URL of the avatar image.
    ///
    /// An empty string clears the source and restores the initials fallback.
    pub fn set_image_source(&mut self, source: &str) {
        self.image_source = source.to_string();
        self.base.request_redraw();
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
    ///
    /// An explicit colour wins over the active theme, so this overrides the
    /// theme-resolved fill until it is set again.
    pub fn set_bg_color(&mut self, color: Color) {
        self.bg_color = color;
        self.bg_color_is_explicit = true;
        self.base.request_redraw();
    }

    /// Returns the current background fill color.
    pub fn bg_color(&self) -> Color {
        self.bg_color
    }

    /// The colour the constructor seeds when the caller sets none.
    ///
    /// Named rather than inlined so [`Draw`] can tell "the caller chose this colour"
    /// (keep it, even across a theme switch) from "nothing chose it yet" (let the
    /// theme decide).
    fn default_bg() -> Color {
        Color::PRIMARY
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

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(40, 40)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `Avatar`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_dialog.in.rs` / `access_write_dialog.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. `initials` reports the
/// widget's own placeholder text and `image_source` its configured image.
impl WidgetProperties for Avatar {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "initials" => Ok(CapabilityValue::String(self.text().to_string())),
            "image_source" => Ok(CapabilityValue::String(self.image_source().to_string())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "initials" => {
                self.set_text(&expect_string(value)?);
                Ok(())
            }
            "image_source" => {
                self.set_image_source(&expect_string(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["initials", "image_source", BASE_PROPERTY_NAMES]
    }
}

impl Draw for Avatar {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let size = rect.width.min(rect.height);
        let center = Point::new(rect.x + (size as i32) / 2, rect.y + (size as i32) / 2);

        // The disc is chrome: it resolves explicit style first, then the theme's resolved
        // style for this control, and only then falls back to the constructor's brand
        // colour. Without the theme step a light/dark switch would change nothing on
        // screen, because the fill was previously hardcoded.
        //
        // The theme reads are separate manager locks, each taken and released inside
        // `resolved_theme_style`, so none is held across the draw or across another
        // accessor — the global manager's mutex is not re-entrant.
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("avatar");
        let window_background = theme.as_ref().and_then(|t| t.background_color);
        // An avatar classifies as a generic surface, so the theme resolves it to the
        // window's own colour. A disc painted in that colour is *invisible* against the
        // window it sits on — the census reports it as "painted in the background's
        // colour", which is the same defect as painting nothing. The visible choice is
        // the raised interactive surface, which the theme derives from its background and
        // foreground and which therefore differs between appearances.
        let raised =
            crate::style::resolved_theme_style("button").and_then(|button| button.background_color);
        let disc_background = style
            .background_color
            .filter(|colour| Some(*colour) != window_background)
            .or(raised)
            .or(window_background)
            .unwrap_or_else(Self::default_bg);
        // Precedence: a colour the caller set with `set_bg_color` wins, then the
        // caller's explicit style, then the theme's raised surface. So an avatar the
        // caller coloured keeps it across a theme switch, while one that was never
        // coloured follows the appearance.
        let disc_color = if self.bg_color_is_explicit { self.bg_color } else { disc_background };

        // Draw the avatar shape (circle or rounded square)
        if self.square {
            let corner_radius = size / 4;
            context.fill_rounded_rect(rect, corner_radius, disc_color);
        } else {
            context.fill_circle(center, size / 2, disc_color);
        }

        // Draw centered initials text
        if !self.text.is_empty() {
            // Use a bold font sized relative to the avatar size
            let font_size = (size as f32 * 0.45).max(8.0);
            let font = Font::bold("Arial", font_size);

            let metrics = context.measure_text(&self.text, &font);
            let text_width = metrics.width as i32;

            // The origin is the glyph box's *top* edge, so centring is half the line box:
            // the old `(size - ascent - descent)/2 + ascent` started the box half a line below
            // the disc's middle and clipped the initials against the bottom edge.
            let text_x = rect.x + (rect.width as i32 - text_width) / 2;
            let text_y = rect.y + (rect.height as i32 - metrics.height as i32) / 2;

            context.draw_text(
                Point::new(text_x.max(rect.x), text_y.max(rect.y)),
                &self.text,
                &font,
                disc_color.contrast_color(),
                HorizontalAlignment::Left,
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
