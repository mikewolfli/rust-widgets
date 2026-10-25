// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `KeyTipBadge` — the small "press these keys" chip a key-tip mode paints over an addressable
//! control.
//!
//! # Why this is its own control
//!
//! The badge was drawn inline inside `ribbon_bar`'s item loop: a filled 18x14 box at the item's
//! top-left corner with its tip centred in it. Every other control a key-tip mode addresses — a tab
//! strip's pages, a segmented button's segments — had no way to paint the same chip without copying
//! that arithmetic, and a copy would drift from the ribbon's own picture.
//!
//! # What it is not
//!
//! The badge takes no events and owns no interaction: it is a badge, and the control underneath it
//! is what receives the key. A host positions it (or asks for [`KeyTipBadge::size_hint`] and lets
//! the layout place it) and paints it over the control it annotates.

use crate::core::{Color, Font, HorizontalAlignment, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::widget::capability::coercion::expect_string;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// The corner a badge is pinned to when a host does not position it explicitly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KeyTipCorner {
    /// Above the control's top-left corner; the ribbon's own placement.
    #[default]
    TopLeft,
    /// Above the control's top-right corner.
    TopRight,
    /// Below the control's bottom-left corner.
    BottomLeft,
    /// Below the control's bottom-right corner.
    BottomRight,
}

impl KeyTipCorner {
    /// The published token: `"top_left"`, `"top_right"`, `"bottom_left"`, `"bottom_right"`.
    pub fn as_str(self) -> &'static str {
        match self {
            KeyTipCorner::TopLeft => "top_left",
            KeyTipCorner::TopRight => "top_right",
            KeyTipCorner::BottomLeft => "bottom_left",
            KeyTipCorner::BottomRight => "bottom_right",
        }
    }

    /// Parses [`Self::as_str`].
    // An inherent token parser returning `Option`; part of the published API surface.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(token: &str) -> Option<Self> {
        match token.trim().to_ascii_lowercase().as_str() {
            "top_left" | "topleft" => Some(KeyTipCorner::TopLeft),
            "top_right" | "topright" => Some(KeyTipCorner::TopRight),
            "bottom_left" | "bottomleft" => Some(KeyTipCorner::BottomLeft),
            "bottom_right" | "bottomright" => Some(KeyTipCorner::BottomRight),
            _ => None,
        }
    }
}

/// A key-tip badge: a small filled chip with a one- or two-character tip.
pub struct KeyTipBadge {
    base: BaseWidget,
    /// The tip text, one or two characters (`"F"`, `"H1"`).
    tip_text: String,
    /// The chip's fill, or `None` while it follows the active theme's accent.
    badge_color: Option<Color>,
    /// The tip's ink, or `None` while it follows the chip's contrast colour.
    ink: Option<Color>,
    /// The corner this badge is anchored to, for a host that positions it by corner.
    corner: KeyTipCorner,
    /// The size of the chip, in logical pixels.
    ///
    /// The ribbon paints an 18x14 chip; that is the default here, and a caller that needs a
    /// different one states it rather than deriving it from its own row height.
    size: Size,
}

impl KeyTipBadge {
    /// Creates a badge with the given tip text.
    ///
    /// The chip is sized 18x14 by default — the ribbon's own chip.
    pub fn new(tip_text: impl Into<String>) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Label, Rect::new(0, 0, 18, 14), "KeyTipBadge"),
            tip_text: tip_text.into(),
            badge_color: None,
            ink: None,
            corner: KeyTipCorner::TopLeft,
            size: Size::new(18, 14),
        }
    }

    /// The tip text.
    pub fn tip_text(&self) -> &str {
        &self.tip_text
    }

    /// Replaces the tip text and repaints.
    ///
    /// The text is shown as given; a caller that wants a strict one/two-character limit should
    /// enforce it where the tip is generated, because truncating here would silently change the key
    /// the user is told to press.
    pub fn set_tip_text(&mut self, text: impl Into<String>) {
        let text = text.into();
        if self.tip_text == text {
            return;
        }
        self.tip_text = text;
        self.base.request_redraw();
    }

    /// The chip size a host should reserve.
    pub fn size_hint(&self) -> Size {
        self.size
    }

    /// Sets the chip size and resizes the badge to match.
    pub fn set_size(&mut self, size: Size) {
        if self.size == size {
            return;
        }
        self.size = size;
        let rect = self.geometry();
        self.base.set_geometry(Rect::new(rect.x, rect.y, size.width, size.height));
        self.base.request_redraw();
    }

    /// The explicit chip fill, if the caller set one.
    pub fn badge_color(&self) -> Option<Color> {
        self.badge_color
    }

    /// Sets the chip fill; `None` returns it to the theme's accent.
    pub fn set_badge_color(&mut self, color: Option<Color>) {
        if self.badge_color == color {
            return;
        }
        self.badge_color = color;
        self.base.request_redraw();
    }

    /// The explicit tip ink, if the caller set one.
    pub fn ink(&self) -> Option<Color> {
        self.ink
    }

    /// Sets the tip ink; `None` returns it to the chip's contrast colour.
    pub fn set_ink(&mut self, color: Option<Color>) {
        if self.ink == color {
            return;
        }
        self.ink = color;
        self.base.request_redraw();
    }

    /// The corner this badge is anchored to.
    pub fn corner(&self) -> KeyTipCorner {
        self.corner
    }

    /// Sets the corner; the caller applies it when it positions the badge.
    pub fn set_corner(&mut self, corner: KeyTipCorner) {
        if self.corner == corner {
            return;
        }
        self.corner = corner;
        self.base.request_redraw();
    }

    /// The box this badge occupies when anchored to `target` at its configured corner.
    ///
    /// A host that wants the chip over the control it annotates calls this instead of re-deriving
    /// the offsets, which is how a badge on a tab page and one on a ribbon item land in the same
    /// relation to their control.
    pub fn rect_for(&self, target: Rect) -> Rect {
        let (x, y) = match self.corner {
            KeyTipCorner::TopLeft => (target.x + 2, target.y + 2),
            KeyTipCorner::TopRight => {
                (target.x + target.width as i32 - self.size.width as i32 - 2, target.y + 2)
            }
            KeyTipCorner::BottomLeft => {
                (target.x + 2, target.y + target.height as i32 - self.size.height as i32 - 2)
            }
            KeyTipCorner::BottomRight => (
                target.x + target.width as i32 - self.size.width as i32 - 2,
                target.y + target.height as i32 - self.size.height as i32 - 2,
            ),
        };
        Rect::new(x, y, self.size.width, self.size.height)
    }

    /// Paints the badge in `rect`.
    ///
    /// This is the entry point a host calls when it owns the position but not the widget: the same
    /// fill, contrast ink and centred tip the ribbon draws inline, so the two cannot differ.
    pub fn paint(&self, context: &mut RenderContext, rect: Rect) {
        let (chip, ink) = self.resolve_colors();
        context.fill_rect(rect, chip);
        if self.tip_text.is_empty() {
            return;
        }
        let font = Font::bold("Arial", (rect.height as f32 * 0.8).clamp(7.0, 12.0));
        context.draw_text_fitted_cow(rect, &self.tip_text, &font, ink, HorizontalAlignment::Center);
    }

    /// The chip fill and the tip ink.
    ///
    /// Precedence: the caller's colour, then the theme's accent (`primary`), then the control's own
    /// fallback blue. The ink defaults to the chip's contrast colour, so a caller that sets only the
    /// fill still gets a readable tip.
    fn resolve_colors(&self) -> (Color, Color) {
        let chip = self
            .badge_color
            .or_else(|| crate::style::palette_shared().map(|colors| colors.primary))
            .unwrap_or(Color::PRIMARY);
        let ink = self.ink.unwrap_or_else(|| chip.contrast_color());
        (chip, ink)
    }
}

impl Widget for KeyTipBadge {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }
    fn size_hint(&self) -> Size {
        self.size
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

impl Draw for KeyTipBadge {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        self.paint(context, rect);
    }
}

impl EventHandler for KeyTipBadge {
    fn handle_event(&mut self, event: &Event) {
        // A badge is inert: it annotates a control the keys go to. The base is still routed so the
        // hover/press state channels exist for a host that embeds it in a live tree.
        self.base.handle_event(event);
    }
}

impl WidgetProperties for KeyTipBadge {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "tip_text" => Ok(CapabilityValue::String(self.tip_text.clone())),
            "badge_color" => Ok(match self.badge_color {
                Some(color) => CapabilityValue::Color(color),
                None => CapabilityValue::Null,
            }),
            "ink" => Ok(match self.ink {
                Some(color) => CapabilityValue::Color(color),
                None => CapabilityValue::Null,
            }),
            "corner" => Ok(CapabilityValue::String(self.corner.as_str().to_string())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "tip_text" => {
                self.set_tip_text(expect_string(value)?);
                Ok(())
            }
            "badge_color" => {
                match value {
                    CapabilityValue::Null => self.set_badge_color(None),
                    CapabilityValue::Color(color) => self.set_badge_color(Some(color)),
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                }
                Ok(())
            }
            "ink" => {
                match value {
                    CapabilityValue::Null => self.set_ink(None),
                    CapabilityValue::Color(color) => self.set_ink(Some(color)),
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                }
                Ok(())
            }
            "corner" => {
                let token = expect_string(value)?;
                let corner =
                    KeyTipCorner::from_str(&token).ok_or(CapabilityAccessError::TypeMismatch)?;
                self.set_corner(corner);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["tip_text", "badge_color", "ink", "corner", BASE_PROPERTY_NAMES]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Point;
    use crate::widget::svg::render_to_svg;

    #[test]
    fn badge_defaults_to_the_ribbon_chip() {
        let badge = KeyTipBadge::new("F");
        assert_eq!(badge.tip_text(), "F");
        assert_eq!(badge.size_hint(), Size::new(18, 14));
        assert_eq!(badge.corner(), KeyTipCorner::TopLeft);
        assert_eq!(badge.badge_color(), None);
    }

    #[test]
    fn setting_the_tip_changes_the_painted_document() {
        let mut badge = KeyTipBadge::new("F");
        badge.set_geometry(Rect::new(0, 0, 18, 14));
        let before = render_to_svg(&mut badge);
        badge.set_tip_text("H1");
        let after = render_to_svg(&mut badge);
        assert_ne!(before, after, "the tip text is what the badge paints");
    }

    #[test]
    fn a_corner_positions_the_chip_inside_its_target() {
        let mut badge = KeyTipBadge::new("A");
        badge.set_corner(KeyTipCorner::BottomRight);
        let target = Rect::new(10, 20, 100, 30);
        let rect = badge.rect_for(target);
        assert_eq!(rect.right(), target.right() - 2);
        assert_eq!(rect.bottom(), target.bottom() - 2);
        assert!(target.contains_point(Point::new(rect.x, rect.y)));
    }

    #[test]
    fn corner_tokens_round_trip() {
        for corner in [
            KeyTipCorner::TopLeft,
            KeyTipCorner::TopRight,
            KeyTipCorner::BottomLeft,
            KeyTipCorner::BottomRight,
        ] {
            assert_eq!(KeyTipCorner::from_str(corner.as_str()), Some(corner));
        }
        assert_eq!(KeyTipCorner::from_str("nowhere"), None);
    }
}
