// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Divider/Separator widget — a horizontal or vertical line separator.
//!
//! The Divider widget draws a thin line (horizontal or vertical) centered within
//! its bounding rectangle. It is used to visually separate groups of content,
//! similar to `<hr>` in HTML or `JSeparator` in Swing.

use crate::core::{Color, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::widget::capability::coercion::{expect_f32, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::display_widgets::line::LineStyle;
use crate::widget::display_widgets::{draw_line, draw_shaded_line};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Divider/Separator widget for visually separating content sections.
///
/// Supports both horizontal and vertical orientations with configurable
/// thickness and color.
pub struct Divider {
    base: BaseWidget,
    vertical: bool,
    thickness: u32,
    color: Color,
    /// Whether the colour was chosen by the caller through [`Divider::set_color`].
    ///
    /// The caller's choice must win over the active theme, but the constructor's
    /// own neutral grey must not: without this flag a theme switch could never
    /// reach a divider that was never explicitly coloured, which is exactly the
    /// hardcoded-chrome defect the rendering census reports.
    color_is_explicit: bool,
    /// The shading shape: plain, sunken or raised.
    ///
    /// `Divider` and `Line` are the same drawing primitive at different sizes, so they share
    /// [`LineStyle`] rather than each declaring a three-variant copy (principle #54). The default
    /// is `Plain`, which is exactly the single-line picture this control always painted.
    style: LineStyle,
    /// How far the drawn line is held back from each end of the control.
    ///
    /// `0` is the default and paints the full chord, which is the historical picture. A positive
    /// value leaves the same gap at the leading and trailing ends, which is what aligns a divider
    /// with the content inset of the card it separates.
    inset: u32,
}

impl Divider {
    /// Creates a new horizontal Divider with default styling (1px thick, gray).
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Divider, geometry, "Divider"),
            vertical: false,
            thickness: 1,
            color: Color::rgba(180, 180, 180, 200),
            color_is_explicit: false,
            style: LineStyle::Plain,
            inset: 0,
        }
    }

    /// Sets whether the divider is vertical (true) or horizontal (false).
    pub fn set_vertical(&mut self, vertical: bool) {
        if self.vertical == vertical {
            return;
        }
        self.vertical = vertical;
        self.base.request_redraw();
    }

    /// Returns whether the divider is oriented vertically.
    pub fn is_vertical(&self) -> bool {
        self.vertical
    }

    /// Returns the current line thickness in pixels.
    pub fn thickness(&self) -> u32 {
        self.thickness
    }

    /// Sets the line thickness in pixels.
    pub fn set_thickness(&mut self, thickness: u32) {
        if self.thickness == thickness {
            return;
        }
        self.thickness = thickness;
        self.base.request_redraw();
    }

    /// How far the line is held back from each end of the control, in pixels.
    pub fn inset(&self) -> u32 {
        self.inset
    }

    /// Sets the end inset, in pixels.
    ///
    /// The drawn line is shortened by this much at both ends — left and right for a horizontal
    /// divider, top and bottom for a vertical one. A value larger than half the span is clamped to
    /// half, so the segment collapses to nothing instead of inverting. The default is `0`, which
    /// keeps the full-width picture.
    pub fn set_inset(&mut self, inset: u32) {
        if self.inset == inset {
            return;
        }
        self.inset = inset;
        self.base.request_redraw();
    }

    /// The rectangle the line is drawn in, with the end inset applied.
    ///
    /// Split out from `draw` so the inset arithmetic has one home and a test can read the same
    /// rectangle the paint path uses. The inset is clamped to half the span, which is what keeps
    /// the segment from inverting when the caller asks for more than the control can hold.
    fn inset_rect(&self) -> Rect {
        let rect = self.geometry();
        if self.vertical {
            let inset = self.inset.min(rect.height / 2);
            Rect::new(
                rect.x,
                rect.y + inset as i32,
                rect.width,
                rect.height.saturating_sub(inset.saturating_mul(2)),
            )
        } else {
            let inset = self.inset.min(rect.width / 2);
            Rect::new(
                rect.x + inset as i32,
                rect.y,
                rect.width.saturating_sub(inset.saturating_mul(2)),
                rect.height,
            )
        }
    }

    /// The divider's shading style.
    pub fn line_style(&self) -> LineStyle {
        self.style
    }

    /// Sets the divider's shading style.
    ///
    /// [`LineStyle::Plain`] is the default and paints one solid line; `Sunken` and `Raised` paint
    /// the same two-band treatment `Line` uses, through the shared helper.
    pub fn set_line_style(&mut self, style: LineStyle) {
        if self.style == style {
            return;
        }
        self.style = style;
        self.base.request_redraw();
    }

    /// The colour the line is drawn in.
    ///
    /// This is the **painted** colour rather than the stored literal: an explicit
    /// [`Divider::set_color`] wins, then the theme's resolved border colour, then the
    /// constructor's neutral grey. Reading it through the property route therefore reports
    /// what the divider actually shows.
    pub fn color(&self) -> Color {
        self.line_color()
    }

    /// Sets the divider line color.
    ///
    /// An explicit colour wins over the active theme, so this overrides the
    /// theme-resolved line colour until it is set again.
    pub fn set_color(&mut self, color: Color) {
        if self.color_is_explicit && self.color == color {
            return;
        }
        self.color = color;
        self.color_is_explicit = true;
        self.base.request_redraw();
    }

    /// The colour the line is drawn in.
    ///
    /// Precedence is the caller's explicit colour, then the theme's resolved
    /// border colour for this control, then the constructor's neutral grey. The
    /// theme step is what makes a theme switch visible: before it, the line kept
    /// its literal grey in both appearances.
    fn line_color(&self) -> Color {
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw or across
        // another accessor — the global manager's mutex is not re-entrant.
        if self.color_is_explicit {
            return self.color;
        }
        crate::style::resolved_theme_style_shared("divider")
            .and_then(|style| style.border_color)
            // # Why a divider reads `outline_variant`
            //
            // A divider is the *weakest* separator a theme has: it structures content and must
            // recede behind it. `outline` is the stronger role — the one a focus ring and a control
            // border read — and `Colors::outline`'s own documentation separates the two for exactly
            // this reason. Before this, a divider fell back to whatever `border_color` the theme
            // resolved for it, which on several presets is the *same* colour a focused control
            // draws its ring in: the two separated quantities had one value.
            .or_else(|| {
                crate::style::palette_shared().map(|active| active.outline_variant)
            })
            .unwrap_or(self.color)
    }
}

impl Widget for Divider {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        // A vertical divider is the horizontal one with its axes swapped: the length is the
        // hint's long side and the line's own thickness is the short one. Reporting the
        // horizontal shape for a vertical line left a layout no way to reserve its column.
        let short = self.thickness.max(2);
        if self.vertical {
            crate::core::Size::new(short, 100)
        } else {
            crate::core::Size::new(100, short)
        }
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `Divider`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` / `access_write_other.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. `orientation` is published as
/// the `horizontal` / `vertical` token pair and `thickness` as the `Float` the
/// schema declares, even though the widget stores it as a pixel count.
impl WidgetProperties for Divider {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "orientation" => Ok(CapabilityValue::String(
                if self.is_vertical() { "vertical" } else { "horizontal" }.to_string(),
            )),
            "thickness" => Ok(CapabilityValue::Float(self.thickness() as f64)),
            "color" => Ok(CapabilityValue::Color(self.color())),
            "line_style" => Ok(CapabilityValue::String(self.line_style().token().to_string())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "orientation" => {
                let orientation = expect_string(value)?;
                match orientation.as_str() {
                    "horizontal" => self.set_vertical(false),
                    "vertical" => self.set_vertical(true),
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                }
                Ok(())
            }
            "thickness" => {
                let thickness = expect_f32(value)?;
                if !thickness.is_finite() || thickness < 0.0 {
                    return Err(CapabilityAccessError::TypeMismatch);
                }
                self.set_thickness(thickness as u32);
                Ok(())
            }
            "color" => {
                // The declared kind is `Color`; a CSS string is still accepted because the
                // JSON and C surfaces carry colours as text (the same allowance every other
                // colour property makes).
                let color = match value {
                    CapabilityValue::Color(color) => color,
                    CapabilityValue::String(raw) => match crate::core::Color::parse_hex(&raw) {
                        Some(color) => color,
                        None => return Err(CapabilityAccessError::TypeMismatch),
                    },
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_color(color);
                Ok(())
            }
            "line_style" => {
                let token = expect_string(value)?;
                let style =
                    LineStyle::from_token(&token).ok_or(CapabilityAccessError::TypeMismatch)?;
                self.set_line_style(style);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["orientation", "thickness", "color", "line_style", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `divider` publishes.
    ///
    /// `set_orientation` takes an orientation token, `set_thickness` a pixel count and
    /// `set_color` a colour, so all three assign state through the property path rather than
    /// performing an action. They are refused as [`CapabilityAccessError::OutOfRange`] so a
    /// caller discovers the names are valid and the invocation needs a payload, not
    /// [`CapabilityAccessError::UnknownCommand`].
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "set_orientation" | "set_thickness" | "set_color" => {
                Err(CapabilityAccessError::OutOfRange)
            }
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl Draw for Divider {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.inset_rect();
        let color = self.line_color();
        match self.style {
            LineStyle::Plain => draw_line(context, rect, self.vertical, self.thickness, color),
            LineStyle::Sunken => draw_shaded_line(context, rect, self.vertical, color, true),
            LineStyle::Raised => draw_shaded_line(context, rect, self.vertical, color, false),
        }
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
        div.handle_event(&Event::MousePress { pos: Point::new(10, 5), button: 1, modifiers: 0 });
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

    /// D-004: the inset is zero by default, shrinks both ends, and clamps before inverting.
    #[test]
    fn the_inset_shortens_the_line_at_both_ends_and_clamps() {
        let mut div = Divider::new(Rect::new(0, 0, 100, 10));
        assert_eq!(div.inset(), 0, "the default paints the full chord");
        assert_eq!(div.inset_rect(), div.geometry());

        div.set_inset(10);
        let horizontal = div.inset_rect();
        assert_eq!(horizontal.x, 10, "the left end is held back");
        assert_eq!(horizontal.width, 80, "both ends lose the inset");

        let mut vertical = Divider::new(Rect::new(0, 0, 10, 100));
        vertical.set_vertical(true);
        vertical.set_inset(10);
        let rect = vertical.inset_rect();
        assert_eq!(rect.y, 10, "the top end is held back");
        assert_eq!(rect.height, 80, "both ends lose the inset");

        // More inset than the span allows collapses the segment instead of inverting it.
        vertical.set_inset(1000);
        let collapsed = vertical.inset_rect();
        assert_eq!(collapsed.height, 0, "the segment never inverts");
        assert_eq!(collapsed.y, 50, "the inset is clamped to half the span");

        div.set_inset(0);
        assert_eq!(div.inset_rect(), div.geometry(), "clearing restores the full chord");
    }

    /// The divider publishes the same three-state shading vocabulary as `Line`, and the two
    /// shaded states paint one more stroke than the plain one.
    #[test]
    fn a_shaded_divider_reuses_the_line_style_vocabulary() {
        let mut div = Divider::new(Rect::new(0, 0, 100, 10));
        assert_eq!(div.line_style(), LineStyle::Plain, "plain is the historical picture");
        let plain = crate::widget::svg::render_to_svg(&mut div);
        let plain_lines = plain.matches("<line").count();

        div.set_line_style(LineStyle::Sunken);
        assert_eq!(div.line_style(), LineStyle::Sunken);
        let sunken = crate::widget::svg::render_to_svg(&mut div);
        assert_eq!(
            sunken.matches("<line").count(),
            plain_lines + 1,
            "a shaded divider paints the second band"
        );

        // The token round-trips through the property route.
        assert_eq!(div.get("line_style").unwrap(), CapabilityValue::String("sunken".to_string()));
        div.set("line_style", CapabilityValue::String("raised".to_string())).unwrap();
        assert_eq!(div.line_style(), LineStyle::Raised);
        assert!(div.set("line_style", CapabilityValue::String("nope".to_string())).is_err());
    }
}
