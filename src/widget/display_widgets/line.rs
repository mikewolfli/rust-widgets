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
use crate::widget::capability::coercion::expect_string;
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

/// A simple horizontal or vertical divider line widget.
pub struct Line {
    base: BaseWidget,
    orientation: LineOrientation,
    thickness: u32,
    color: Option<Color>,
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
        }
    }

    /// Returns the current orientation.
    pub fn orientation(&self) -> LineOrientation {
        self.orientation
    }

    /// Sets the orientation (Horizontal or Vertical).
    pub fn set_orientation(&mut self, ori: LineOrientation) {
        self.orientation = ori;
        self.base.request_redraw();
    }

    /// Sets the line thickness in pixels (minimum 1).
    pub fn set_thickness(&mut self, t: u32) {
        self.thickness = t.max(1);
        self.base.request_redraw();
    }
}

impl Widget for Line {
    fn base(&self) -> &BaseWidget {
        &self.base
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
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["orientation", BASE_PROPERTY_NAMES]
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
        // then a default gray.
        let line_color =
            self.color.or_else(|| self.style().border_color).unwrap_or(Color::rgb(180, 180, 180));

        draw_line(context, rect, is_vertical, self.thickness, line_color);
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
}
