// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Line widget — horizontal or vertical divider line (BLUE13 R2.13).
//!
//! A simple standalone line widget that draws a horizontal or vertical divider
//! line across the widget rectangle. Useful for visually separating sections
//! in layouts.

use crate::compat::ToString;
use crate::core::{Color, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::widget::capability::coercion::{expect_string, expect_u32};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::display_widgets::draw_line;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Orientation of the divider line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineOrientation {
    /// Horizontal line (default).
    Horizontal,
    /// Vertical line.
    Vertical,
}

/// The shading style of a divider line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineStyle {
    /// A single solid line, the historical rendering and the default.
    #[default]
    Plain,
    /// Two adjacent 1 px lines: a dark leading edge over a light trailing one, so the
    /// region reads as pressed in.
    Sunken,
    /// Two adjacent 1 px lines: a light leading edge over a dark trailing one, so the
    /// region reads as raised.
    Raised,
}

impl LineStyle {
    /// The token this style is published as.
    pub const fn token(self) -> &'static str {
        match self {
            Self::Plain => "plain",
            Self::Sunken => "sunken",
            Self::Raised => "raised",
        }
    }

    /// Parses a published token. `None` for a spelling this control does not publish.
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "plain" => Some(Self::Plain),
            "sunken" => Some(Self::Sunken),
            "raised" => Some(Self::Raised),
            _ => None,
        }
    }
}

/// A simple horizontal or vertical divider line widget.
pub struct Line {
    base: BaseWidget,
    orientation: LineOrientation,
    thickness: u32,
    color: Option<Color>,
    style: LineStyle,
}

impl Line {
    /// Creates a new Line widget with the given orientation and geometry.
    ///
    /// Defaults: thickness 2, no explicit color (uses `style().border_color`).
    pub fn new(orientation: LineOrientation, rect: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Line, rect, "Line"),
            orientation,
            thickness: 2,
            color: None,
            style: LineStyle::Plain,
        }
    }

    /// Returns the current orientation.
    pub fn orientation(&self) -> LineOrientation {
        self.orientation
    }

    /// Sets the orientation (Horizontal or Vertical).
    pub fn set_orientation(&mut self, ori: LineOrientation) {
        if self.orientation == ori {
            return;
        }
        self.orientation = ori;
        self.base.request_redraw();
    }

    /// Sets the line thickness in pixels (minimum 1).
    pub fn set_thickness(&mut self, t: u32) {
        let thickness = t.max(1);
        if self.thickness == thickness {
            return;
        }
        self.thickness = thickness;
        self.base.request_redraw();
    }

    /// Returns the line thickness in pixels.
    pub fn thickness(&self) -> u32 {
        self.thickness
    }

    /// The explicit colour override, or `None` when the line follows the theme's border colour.
    pub fn color(&self) -> Option<Color> {
        self.color
    }

    /// Sets (or clears, with `None`) the explicit line colour.
    pub fn set_color(&mut self, color: Option<Color>) {
        if self.color == color {
            return;
        }
        self.color = color;
        self.base.request_redraw();
    }

    /// The line's shading style.
    ///
    /// Note that this is the control's own style, not the resolved theme style — the latter
    /// remains available through [`Widget::style`].
    pub fn line_style(&self) -> LineStyle {
        self.style
    }

    /// Sets the shading style. [`LineStyle::Plain`] is the default and paints one line.
    pub fn set_line_style(&mut self, style: LineStyle) {
        if self.style == style {
            return;
        }
        self.style = style;
        self.base.request_redraw();
    }

    /// The colour the painter uses: the explicit override, then the style's border, then a literal.
    fn line_color(&self) -> Color {
        self.color.or_else(|| Widget::style(self).border_color).unwrap_or(Color::rgb(180, 180, 180))
    }

    /// Paints the two-line [`LineStyle::Sunken`] / [`LineStyle::Raised`] treatment.
    ///
    /// The two 1 px bands are centred in the control's rectangle. `sunken` decides which
    /// edge is dark and which is light; the two styles are the same pair in the other order.
    fn draw_shaded(&self, context: &mut RenderContext, rect: Rect, vertical: bool, sunken: bool) {
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let base = self.line_color();
        let highlight = base.blend(&Color::WHITE, 0.45);
        let shadow = base.blend(&Color::BLACK, 0.35);
        let (first, second) = if sunken { (shadow, highlight) } else { (highlight, shadow) };

        let extent = if vertical { rect.width } else { rect.height };
        if extent < 2 {
            // A one-pixel-thick control has no room for two rows; paint the leading edge so
            // the style still has visible ink rather than silently drawing nothing twice.
            draw_line(context, rect, vertical, 1, first);
            return;
        }

        if vertical {
            let middle = rect.x + rect.width as i32 / 2;
            draw_line(context, Rect::new(middle - 1, rect.y, 1, rect.height), true, 1, first);
            draw_line(context, Rect::new(middle, rect.y, 1, rect.height), true, 1, second);
        } else {
            let middle = rect.y + rect.height as i32 / 2;
            draw_line(context, Rect::new(rect.x, middle - 1, rect.width, 1), false, 1, first);
            draw_line(context, Rect::new(rect.x, middle, rect.width, 1), false, 1, second);
        }
    }
}

impl Widget for Line {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        match self.orientation {
            LineOrientation::Horizontal => Size::new(120, self.thickness.max(2)),
            LineOrientation::Vertical => Size::new(self.thickness.max(2), 120),
        }
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `Line`'s property contract.
///
/// The write path accepts exactly the two tokens the read path publishes
/// (`"horizontal"` / `"vertical"`) and rejects anything else with
/// [`CapabilityAccessError::TypeMismatch`], matching the previous dispatch.
impl WidgetProperties for Line {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "orientation" => {
                let orientation = match self.orientation() {
                    LineOrientation::Horizontal => "horizontal",
                    LineOrientation::Vertical => "vertical",
                };
                Ok(CapabilityValue::String(orientation.to_string()))
            }
            // The effective colour, because that is what a caller sees; the explicit override is
            // only one of the three rungs the painter walks.
            "color" => Ok(CapabilityValue::Color(self.line_color())),
            "thickness" => Ok(CapabilityValue::UInt(u64::from(self.thickness()))),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "orientation" => {
                let token = expect_string(value)?;
                let orientation = match token.as_str() {
                    "horizontal" => LineOrientation::Horizontal,
                    "vertical" => LineOrientation::Vertical,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_orientation(orientation);
                Ok(())
            }
            "color" => {
                // `Null` clears the override (the line follows the theme again). A CSS string is
                // still accepted, as on the other colour properties.
                match value {
                    CapabilityValue::Null => self.set_color(None),
                    CapabilityValue::Color(color) => self.set_color(Some(color)),
                    CapabilityValue::String(raw) => match Color::parse_hex(&raw) {
                        Some(color) => self.set_color(Some(color)),
                        None => return Err(CapabilityAccessError::TypeMismatch),
                    },
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                }
                Ok(())
            }
            "thickness" => {
                self.set_thickness(expect_u32(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["orientation", "thickness", "color", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `line` publishes.
    ///
    /// `set_orientation` is the only published name and it assigns one of the two
    /// orientation tokens, so it needs an argument a command carries none of. It is
    /// refused as [`CapabilityAccessError::OutOfRange`] — the name is right, the token
    /// belongs on the property route — rather than reported unknown.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "set_orientation" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for Line {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

impl Draw for Line {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let is_vertical = self.orientation == LineOrientation::Vertical;

        // Resolve line color: preferred explicit color, then style border_color,
        // then a default gray. One derivation, shared with the `color` property.
        let line_color = self.line_color();

        match self.style {
            LineStyle::Plain => {
                draw_line(context, rect, is_vertical, self.thickness, line_color);
            }
            LineStyle::Sunken => self.draw_shaded(context, rect, is_vertical, true),
            LineStyle::Raised => self.draw_shaded(context, rect, is_vertical, false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Color, Rect, Size};
    use crate::render::{PaintBackend, SoftwarePaintBackend};

    #[test]
    fn line_creation() {
        let line = Line::new(LineOrientation::Horizontal, Rect::new(0, 0, 200, 4));
        assert_eq!(line.orientation(), LineOrientation::Horizontal);
        assert_eq!(line.kind(), WidgetKind::Line);
    }

    /// The colour override is a real state, not a write-only field.
    ///
    /// The field existed but had no setter and no property, so `draw` always fell through to
    /// the style's border colour and the `Option` could never be `Some`.
    #[test]
    fn line_colour_override_round_trips() {
        use crate::widget::capability::WidgetProperties;
        let mut line = Line::new(LineOrientation::Horizontal, Rect::new(0, 0, 200, 4));
        assert_eq!(line.color(), None);

        line.set_color(Some(Color::RED));
        assert_eq!(line.color(), Some(Color::RED));
        assert_eq!(
            line.get("color"),
            Ok(CapabilityValue::Color(Color::RED)),
            "the published colour is the effective one the painter uses"
        );

        line.set_color(None);
        assert_eq!(line.color(), None, "`None` restores theme-following");
    }

    #[test]
    fn line_orientation() {
        let mut line = Line::new(LineOrientation::Horizontal, Rect::new(0, 0, 200, 4));
        assert_eq!(line.orientation(), LineOrientation::Horizontal);

        line.set_orientation(LineOrientation::Vertical);
        assert_eq!(line.orientation(), LineOrientation::Vertical);

        line.set_orientation(LineOrientation::Horizontal);
        assert_eq!(line.orientation(), LineOrientation::Horizontal);
    }

    #[test]
    fn line_set_thickness() {
        let mut line = Line::new(LineOrientation::Horizontal, Rect::new(0, 0, 200, 4));
        line.set_thickness(5);
        assert_eq!(line.thickness, 5);

        // Minimum is 1.
        line.set_thickness(0);
        assert_eq!(line.thickness, 1);
    }

    #[test]
    fn line_draw_no_panic() {
        let mut line = Line::new(LineOrientation::Horizontal, Rect::new(0, 0, 200, 4));

        let mut backend = SoftwarePaintBackend::new(Size::new(200, 10), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut context = RenderContext::new(&mut backend);
        line.draw(&mut context);
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert!(!rgba.is_empty());
    }

    #[test]
    fn line_draw_vertical_no_panic() {
        let mut line = Line::new(LineOrientation::Vertical, Rect::new(0, 0, 4, 200));

        let mut backend = SoftwarePaintBackend::new(Size::new(10, 200), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut context = RenderContext::new(&mut backend);
        line.draw(&mut context);
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert!(!rgba.is_empty());
    }

    #[test]
    fn line_draw_zero_geometry_no_panic() {
        let mut line = Line::new(LineOrientation::Horizontal, Rect::new(0, 0, 0, 0));
        let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut context = RenderContext::new(&mut backend);
        line.draw(&mut context);
        backend.end_frame();
    }

    #[test]
    fn line_size_hint() {
        let line = Line::new(LineOrientation::Horizontal, Rect::new(0, 0, 200, 4));
        assert_eq!(line.size_hint(), Size::new(120, 2));

        let mut line = Line::new(LineOrientation::Vertical, Rect::new(0, 0, 4, 200));
        line.set_thickness(6);
        assert_eq!(line.size_hint(), Size::new(6, 120));
    }

    #[test]
    fn line_geometry_delegation() {
        let mut line = Line::new(LineOrientation::Horizontal, Rect::new(0, 0, 200, 4));
        line.set_geometry(Rect::new(5, 5, 150, 6));
        assert_eq!(line.geometry(), Rect::new(5, 5, 150, 6));
    }

    #[test]
    fn line_event_delegation() {
        let mut line = Line::new(LineOrientation::Horizontal, Rect::new(0, 0, 200, 4));
        // Handle event without panicking.
        line.handle_event(&Event::KeyDown((37, 0)));
    }

    /// The style is a real state with published tokens, and `Plain` stays the default.
    #[test]
    fn line_style_tokens_round_trip() {
        assert_eq!(LineStyle::default(), LineStyle::Plain);
        assert_eq!(LineStyle::from_token("plain"), Some(LineStyle::Plain));
        assert_eq!(LineStyle::from_token("sunken"), Some(LineStyle::Sunken));
        assert_eq!(LineStyle::from_token("raised"), Some(LineStyle::Raised));
        assert_eq!(LineStyle::from_token("chartreuse"), None);
        assert_eq!(LineStyle::Sunken.token(), "sunken");

        let mut line = Line::new(LineOrientation::Horizontal, Rect::new(0, 0, 200, 4));
        assert_eq!(line.line_style(), LineStyle::Plain);
        line.set_line_style(LineStyle::Raised);
        assert_eq!(line.line_style(), LineStyle::Raised);
        line.set_line_style(LineStyle::Sunken);
        assert_eq!(line.line_style(), LineStyle::Sunken);
    }

    /// Sunken/Raised paint two adjacent strokes instead of one, and swap their edges.
    #[test]
    fn sunken_and_raised_paint_a_double_line() {
        let _theme_guard = crate::style::theme_test_guard();
        let mut line = Line::new(LineOrientation::Horizontal, Rect::new(0, 0, 200, 6));

        let plain = crate::widget::svg::render_to_svg(&mut line);
        assert_eq!(plain.matches("<line").count(), 1, "Plain paints one line: {plain}");

        line.set_line_style(LineStyle::Sunken);
        let sunken = crate::widget::svg::render_to_svg(&mut line);
        assert_eq!(sunken.matches("<line").count(), 2, "Sunken paints two lines: {sunken}");

        line.set_line_style(LineStyle::Raised);
        let raised = crate::widget::svg::render_to_svg(&mut line);
        assert_eq!(raised.matches("<line").count(), 2, "Raised paints two lines: {raised}");
        assert_ne!(sunken, raised, "the two styles swap the highlight and shadow edges");
    }

    /// A vertical sunken line shades along its width, not its length.
    #[test]
    fn a_vertical_shaded_line_stays_inside_its_rectangle() {
        let _theme_guard = crate::style::theme_test_guard();
        let mut line = Line::new(LineOrientation::Vertical, Rect::new(0, 0, 6, 120));
        line.set_line_style(LineStyle::Sunken);
        let svg = crate::widget::svg::render_to_svg(&mut line);
        assert_eq!(svg.matches("<line").count(), 2, "two edges: {svg}");
        for x in [2, 3] {
            assert!(
                svg.contains(&format!("x1=\"{x}\"")) && svg.contains(&format!("x2=\"{x}\"")),
                "the two edges are adjacent columns at x={x}: {svg}"
            );
        }
    }
}
