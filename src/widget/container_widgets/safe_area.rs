// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! SafeArea widget — insets content to avoid notches, status bars, home indicators (BLUE11 R10.14).
use crate::core::{Color, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::widget::capability::coercion::expect_f32;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Safe area insets for mobile devices.
///
/// All four values are in logical pixels and describe how much of each edge is
/// obstructed by system UI (notch, status bar, home indicator, rounded corners)
/// and therefore must stay clear of content. They are edge distances, not a
/// rectangle: the defaults describe a portrait phone with a notch and a home
/// indicator, and are wrong for tablets, landscape, or desktop — set them from
/// the platform's reported insets rather than relying on the default.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SafeAreaInsets {
    /// Space to reserve along the top edge, in logical pixels.
    pub top: u32,
    /// Space to reserve along the bottom edge, in logical pixels.
    pub bottom: u32,
    /// Space to reserve along the left edge, in logical pixels. Zero in the
    /// default, since portrait phones rarely obstruct the sides.
    pub left: u32,
    /// Space to reserve along the right edge, in logical pixels.
    pub right: u32,
}

impl Default for SafeAreaInsets {
    fn default() -> Self {
        Self { top: 44, bottom: 34, left: 0, right: 0 }
    }
}

/// SafeArea widget — wraps content with safe area insets (BLUE11 R10.14).
///
/// The widget does not reposition its children itself. It publishes
/// [`SafeArea::content_rect`] and paints the inset margins, leaving the host
/// layout to place content inside that rectangle. Children laid out against
/// [`Widget::geometry`] instead of `content_rect` will overlap system UI.
pub struct SafeArea {
    base: BaseWidget,
    insets: SafeAreaInsets,
    /// Background color for the safe area margins.
    margin_color: Color,
}

impl SafeArea {
    /// Creates a widget using the default phone-style insets
    /// ([`SafeAreaInsets::default`]) and a white margin colour.
    ///
    /// `geometry` is in parent-relative logical pixels; the size hint is
    /// 300x200.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::SafeArea, geometry, "SafeArea"),
            insets: SafeAreaInsets::default(),
            margin_color: Color::WHITE,
        }
    }
    /// Replaces all four insets at once and requests a redraw.
    ///
    /// Insets larger than the widget in a given axis are tolerated: the content
    /// rect saturates to zero extent rather than underflowing, and the margin
    /// bars may then paint outside the widget's own rectangle.
    pub fn set_insets(&mut self, insets: SafeAreaInsets) {
        self.insets = insets;
        self.base.request_redraw();
    }
    /// Returns the current insets.
    pub fn insets(&self) -> SafeAreaInsets {
        self.insets
    }
    /// Sets the colour painted in the four inset margin bars.
    ///
    /// Does not request a redraw, so a visible change needs an explicit repaint.
    pub fn set_margin_color(&mut self, color: Color) {
        self.margin_color = color;
    }
    /// Sets the top inset, preserving the other three edges.
    pub fn set_top_inset(&mut self, top: u32) {
        self.set_insets(SafeAreaInsets { top, ..self.insets });
    }
    /// Sets the bottom inset, preserving the other three edges.
    pub fn set_bottom_inset(&mut self, bottom: u32) {
        self.set_insets(SafeAreaInsets { bottom, ..self.insets });
    }
    /// Sets the left inset, preserving the other three edges.
    pub fn set_left_inset(&mut self, left: u32) {
        self.set_insets(SafeAreaInsets { left, ..self.insets });
    }
    /// Sets the right inset, preserving the other three edges.
    pub fn set_right_inset(&mut self, right: u32) {
        self.set_insets(SafeAreaInsets { right, ..self.insets });
    }
    /// Returns the rectangle content should occupy: the widget's geometry
    /// inset by the safe area on all four sides.
    ///
    /// In the same parent-relative coordinate space as [`Widget::geometry`].
    /// Width and height saturate at zero when the insets exceed the widget, so
    /// the result is never inverted.
    pub fn content_rect(&self) -> Rect {
        let g = self.geometry();
        Rect::new(
            g.x + self.insets.left as i32,
            g.y + self.insets.top as i32,
            g.width.saturating_sub(self.insets.left + self.insets.right),
            g.height.saturating_sub(self.insets.top + self.insets.bottom),
        )
    }
}

impl Widget for SafeArea {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        crate::core::Size::new(300, 200)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `SafeArea`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_dialog.in.rs` / `access_write_dialog.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. The insets are stored as
/// `u32` internally and published as `Float` to match the legacy shape.
impl WidgetProperties for SafeArea {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        let insets = self.insets();
        match name {
            "top_inset" => Ok(CapabilityValue::Float(f64::from(insets.top))),
            "bottom_inset" => Ok(CapabilityValue::Float(f64::from(insets.bottom))),
            "left_inset" => Ok(CapabilityValue::Float(f64::from(insets.left))),
            "right_inset" => Ok(CapabilityValue::Float(f64::from(insets.right))),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "top_inset" => {
                self.set_top_inset(expect_f32(value)? as u32);
                Ok(())
            }
            "bottom_inset" => {
                self.set_bottom_inset(expect_f32(value)? as u32);
                Ok(())
            }
            "left_inset" => {
                self.set_left_inset(expect_f32(value)? as u32);
                Ok(())
            }
            "right_inset" => {
                self.set_right_inset(expect_f32(value)? as u32);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "top_inset",
            "bottom_inset",
            "left_inset",
            "right_inset",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl Draw for SafeArea {
    fn draw(&mut self, ctx: &mut RenderContext) {
        let g = self.geometry();
        let cr = self.content_rect();

        // The inset bars and the content outline resolve explicit style first, then the
        // theme's resolved style for this control, and only then a literal. SafeArea is
        // not in the role table, so it classifies as `Surface` and `apply_active_theme`
        // cannot give it a distinct fill on its own — the bars therefore derive their
        // colour one step away from the theme's foreground, which is what makes an
        // appearance switch visible.
        //
        // The theme reads take and release the global manager's lock internally, so no
        // guard is held across the draw (the mutex is not re-entrant).
        let style = self.base.style().clone();
        let theme = crate::theme::resolved_theme_style("safe_area");
        let window_fill = crate::theme::global_theme_manager()
            .current_theme()
            .map(|active| active.colors.background)
            .unwrap_or(Color::WHITE);
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        // A caller who set an explicit margin colour keeps it: `set_margin_color` is the
        // control's own override and wins over the theme, the same precedence
        // `WidgetStyle` documents. The default white it starts on is not an override, so
        // it is replaced by the themed surface.
        let margin_color =
            if self.margin_color == Color::WHITE {
                style
                    .background_color
                    .or_else(|| theme.as_ref().and_then(|t| t.background_color))
                    .map(|resolved| {
                        if resolved == window_fill {
                            window_fill.blend(&ink, 0.08)
                        } else {
                            resolved
                        }
                    })
                    .unwrap_or_else(|| window_fill.blend(&ink, 0.08))
            } else {
                self.margin_color
            };
        // The content outline is chrome too, and used to be a hardcoded grey that ignored
        // the appearance entirely.
        let border_color = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| window_fill.blend(&ink, 0.25));

        // Fill margin areas (top/bottom/left/right bars)
        if self.insets.top > 0 {
            ctx.fill_rect(Rect::new(g.x, g.y, g.width, self.insets.top), margin_color);
        }
        if self.insets.bottom > 0 {
            ctx.fill_rect(
                Rect::new(
                    g.x,
                    g.y + g.height as i32 - self.insets.bottom as i32,
                    g.width,
                    self.insets.bottom,
                ),
                margin_color,
            );
        }
        if self.insets.left > 0 {
            let left_bar_height = g.height.saturating_sub(self.insets.top + self.insets.bottom);
            ctx.fill_rect(
                Rect::new(g.x, g.y + self.insets.top as i32, self.insets.left, left_bar_height),
                margin_color,
            );
        }
        if self.insets.right > 0 {
            let right_bar_height = g.height.saturating_sub(self.insets.top + self.insets.bottom);
            ctx.fill_rect(
                Rect::new(
                    g.x + g.width as i32 - self.insets.right as i32,
                    g.y + self.insets.top as i32,
                    self.insets.right,
                    right_bar_height,
                ),
                margin_color,
            );
        }
        // Draw content area border (subtle)
        ctx.draw_rect(cr, border_color.with_alpha_f32(0.4));
    }
}

impl EventHandler for SafeArea {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Point;

    #[test]
    fn safe_area_default_insets() {
        let sa = SafeArea::new(Rect::new(0, 0, 375, 812));
        let insets = sa.insets();
        assert_eq!(insets.top, 44);
        assert_eq!(insets.bottom, 34);
        assert_eq!(insets.left, 0);
        assert_eq!(insets.right, 0);
    }

    #[test]
    fn safe_area_content_rect_computes_correctly() {
        let sa = SafeArea::new(Rect::new(0, 0, 375, 812));
        let cr = sa.content_rect();
        assert_eq!(cr.x, 0);
        assert_eq!(cr.y, 44);
        assert_eq!(cr.width, 375);
        assert_eq!(cr.height, 812 - 44 - 34);
    }

    #[test]
    fn safe_area_content_rect_with_all_insets() {
        let mut sa = SafeArea::new(Rect::new(0, 0, 375, 812));
        sa.set_insets(SafeAreaInsets { top: 50, bottom: 40, left: 16, right: 16 });
        let cr = sa.content_rect();
        assert_eq!(cr.x, 16);
        assert_eq!(cr.y, 50);
        assert_eq!(cr.width, 375 - 32);
        assert_eq!(cr.height, 812 - 90);
    }

    #[test]
    fn safe_area_set_insets_updates_content_rect() {
        let mut sa = SafeArea::new(Rect::new(0, 0, 400, 800));
        let custom = SafeAreaInsets { top: 60, bottom: 50, left: 10, right: 10 };
        sa.set_insets(custom);
        assert_eq!(sa.insets(), custom);
        let cr = sa.content_rect();
        assert_eq!(cr.x, 10);
        assert_eq!(cr.y, 60);
    }

    #[test]
    fn safe_area_set_margin_color() {
        let mut sa = SafeArea::new(Rect::new(0, 0, 375, 812));
        sa.set_margin_color(Color::BLACK);
        // No crash; color is used in draw.
        assert_eq!(sa.kind(), WidgetKind::SafeArea);
    }

    #[test]
    fn safe_area_event_delegation() {
        let mut sa = SafeArea::new(Rect::new(0, 0, 375, 812));
        // Events should not panic/crash.
        sa.handle_event(&Event::MousePress { pos: Point::new(10, 5), button: 1 });
        sa.handle_event(&Event::MouseRelease { pos: Point::new(10, 5), button: 1 });
        assert_eq!(sa.insets().top, 44);
    }

    #[test]
    fn safe_area_svg_output() {
        let mut sa = SafeArea::new(Rect::new(0, 0, 375, 812));
        let svg = crate::widget::svg::render_to_svg(&mut sa);
        assert!(svg.starts_with("<svg"));
    }

    #[test]
    fn safe_area_content_rect_saturating_overflow() {
        let mut sa = SafeArea::new(Rect::new(0, 0, 100, 100));
        let huge = SafeAreaInsets { top: 200, bottom: 200, left: 0, right: 0 };
        sa.set_insets(huge);
        // This should not panic; height saturates to 0.
        let cr = sa.content_rect();
        assert_eq!(cr.width, 100); // left/right are 0, so width unchanged
        assert_eq!(cr.height, 0);
    }
}
