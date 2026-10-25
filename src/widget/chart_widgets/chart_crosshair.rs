// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `ChartCrosshair` — the vertical/horizontal reference lines a chart shows under the pointer.
//!
//! A chart that only paints a 3 px dot under the pointer gives the reader no way to read a value
//! off the axes: the dot says "here", not "this value". Every chart in the family needs the same
//! reference lines, the same snapping rule and the same axis chips, so they live in one companion
//! instead of being re-drawn per chart (COMP-F01).
//!
//! # Wiring
//!
//! The chart converts its pointer position (and, when snapping, the nearest data point) and calls
//! [`ChartCrosshair::show_at_data`]; `MouseLeave` calls [`ChartCrosshair::hide`]. The companion is
//! a plain widget, so a chart can also own one and paint it in its own `Draw`.

use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Which reference lines a [`ChartCrosshair`] paints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CrosshairMode {
    /// One vertical line at the pointer's x.
    Vertical,
    /// One horizontal line at the pointer's y.
    Horizontal,
    /// Both lines crossing at the pointer.
    #[default]
    Both,
}

impl CrosshairMode {
    /// The token this mode is spelled as in a saved property.
    pub fn as_str(self) -> &'static str {
        match self {
            CrosshairMode::Vertical => "vertical",
            CrosshairMode::Horizontal => "horizontal",
            CrosshairMode::Both => "both",
        }
    }

    /// Parses the token back, rejecting anything else.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "vertical" => Some(CrosshairMode::Vertical),
            "horizontal" => Some(CrosshairMode::Horizontal),
            "both" => Some(CrosshairMode::Both),
            _ => None,
        }
    }
}

/// A chart crosshair: reference lines, an optional marker and axis value chips.
pub struct ChartCrosshair {
    base: BaseWidget,
    enabled: bool,
    mode: CrosshairMode,
    snap_to_point: bool,
    line_color: Option<Color>,
    line_width: u32,
    show_axis_values: bool,
    show_marker: bool,
    /// The pixel the pointer is at, in the chart's own coordinates.
    point: Option<Point>,
    /// The data coordinate the pixel stands for, published through `crosshair_moved`.
    data: Option<(f64, f64)>,
    /// Emitted with the data coordinate on show/move and `None` on hide.
    pub crosshair_moved: Signal1<Option<(f64, f64)>>,
}

impl ChartCrosshair {
    /// Creates a crosshair covering `geometry` (the chart's plot rect).
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Panel, geometry, "ChartCrosshair"),
            enabled: true,
            mode: CrosshairMode::Both,
            snap_to_point: false,
            line_color: None,
            line_width: 1,
            show_axis_values: true,
            show_marker: true,
            point: None,
            data: None,
            crosshair_moved: Signal1::new(),
        }
    }

    /// Shows the crosshair at a pixel position without a data coordinate.
    ///
    /// Kept for a caller that has no data mapping; the `crosshair_moved` payload then stays
    /// whatever the last snapped point set, because inventing a data value from pixels would be
    /// a number nothing measured.
    pub fn show_at(&mut self, point: Point) {
        self.point = Some(point);
        if let Some(data) = self.data {
            self.crosshair_moved.emit(Some(data));
        }
        self.base.request_redraw();
    }

    /// Shows the crosshair at a pixel and publishes the data coordinate it stands for.
    pub fn show_at_data(&mut self, point: Point, data: (f64, f64)) {
        self.enabled = true;
        self.point = Some(point);
        self.data = Some(data);
        self.crosshair_moved.emit(Some(data));
        self.base.request_redraw();
    }

    /// Hides the crosshair.
    pub fn hide(&mut self) {
        if self.point.take().is_none() && self.data.take().is_none() {
            return;
        }
        self.data = None;
        self.crosshair_moved.emit(None);
        self.base.request_redraw();
    }

    /// Whether the crosshair is currently shown.
    pub fn is_visible(&self) -> bool {
        self.point.is_some()
    }

    /// Whether the crosshair is enabled at all.
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Enables or disables the crosshair.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.hide();
        }
    }

    /// The line set painted.
    pub fn mode(&self) -> CrosshairMode {
        self.mode
    }

    /// Sets the line set painted.
    pub fn set_mode(&mut self, mode: CrosshairMode) {
        self.mode = mode;
        self.base.request_redraw();
    }

    /// Whether the chart snaps the lines to the nearest data point.
    pub fn snap_to_point(&self) -> bool {
        self.snap_to_point
    }

    /// Sets whether the chart snaps to the nearest data point.
    pub fn set_snap_to_point(&mut self, snap: bool) {
        self.snap_to_point = snap;
    }

    /// The caller's line colour, or `None` while the theme outline is used.
    pub fn line_color(&self) -> Option<Color> {
        self.line_color
    }

    /// Sets the line colour.
    pub fn set_line_color(&mut self, color: Option<Color>) {
        self.line_color = color;
        self.base.request_redraw();
    }

    /// The line width in pixels.
    pub fn line_width(&self) -> u32 {
        self.line_width
    }

    /// Sets the line width.
    pub fn set_line_width(&mut self, width: u32) {
        self.line_width = width.max(1);
        self.base.request_redraw();
    }

    /// Whether the axis value chips are painted.
    pub fn show_axis_values(&self) -> bool {
        self.show_axis_values
    }

    /// Enables or disables the axis value chips.
    pub fn set_show_axis_values(&mut self, show: bool) {
        self.show_axis_values = show;
        self.base.request_redraw();
    }

    /// Whether a marker dot is painted at the crossing.
    pub fn show_marker(&self) -> bool {
        self.show_marker
    }

    /// Enables or disables the marker dot.
    pub fn set_show_marker(&mut self, show: bool) {
        self.show_marker = show;
        self.base.request_redraw();
    }

    /// The data coordinate currently published.
    pub fn data_value(&self) -> Option<(f64, f64)> {
        self.data
    }

    /// The resolved line colour: the caller's, or the theme's outline role.
    fn resolved_color(&self) -> Color {
        self.line_color.unwrap_or_else(|| {
            crate::style::palette_shared()
                .as_ref()
                .map(|colors| colors.outline)
                .unwrap_or(Color::rgba(128, 128, 128, 180))
        })
    }
}

impl Widget for ChartCrosshair {
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
        crate::core::Size::new(120, 120)
    }
    crate::impl_draw_bridge!();
}

impl Draw for ChartCrosshair {
    fn draw(&mut self, context: &mut RenderContext) {
        let Some(point) = self.point else {
            return;
        };
        if !self.enabled {
            return;
        }
        let rect = self.base.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let color = self.resolved_color();
        let width = self.line_width.max(1);
        let left = rect.x.max(0);
        let top = rect.y.max(0);
        let right = rect.x.saturating_add(rect.width as i32);
        let bottom = rect.y.saturating_add(rect.height as i32);

        if matches!(self.mode, CrosshairMode::Vertical | CrosshairMode::Both) {
            context.draw_line_stroke_aa(
                Point::new(point.x, top),
                Point::new(point.x, bottom),
                color,
                width,
            );
        }
        if matches!(self.mode, CrosshairMode::Horizontal | CrosshairMode::Both) {
            context.draw_line_stroke_aa(
                Point::new(left, point.y),
                Point::new(right, point.y),
                color,
                width,
            );
        }
        if self.show_marker {
            context.fill_circle(point, 3, color);
        }
        if self.show_axis_values {
            if let Some((x, y)) = self.data {
                let label = std::format!("{x:.2}, {y:.2}");
                let font = crate::widget::metrics::effective_font(self.base.style());
                context.draw_text(
                    Point::new(left + 2, top + 2),
                    &label,
                    font,
                    color,
                    HorizontalAlignment::Left,
                );
            }
        }
    }
}

impl crate::event::EventHandler for ChartCrosshair {
    fn handle_event(&mut self, _event: &crate::event::Event) {
        // The owning chart routes pointer events; the companion itself has no gesture.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn show_and_hide_publish_the_data_coordinate() {
        let mut crosshair = ChartCrosshair::new(Rect::new(0, 0, 200, 100));
        assert!(!crosshair.is_visible());
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = std::sync::Arc::clone(&seen);
        crosshair.crosshair_moved.connect(move |value| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*value);
            }
        });

        crosshair.show_at_data(Point::new(20, 30), (1.5, 2.5));
        assert!(crosshair.is_visible());
        assert_eq!(crosshair.data_value(), Some((1.5, 2.5)));
        crosshair.hide();
        assert!(!crosshair.is_visible());
        assert_eq!(
            *seen.lock().expect("lock"),
            vec![Some((1.5, 2.5)), None],
            "show publishes the value, hide publishes None"
        );
    }

    #[test]
    fn mode_tokens_round_trip() {
        for mode in [CrosshairMode::Vertical, CrosshairMode::Horizontal, CrosshairMode::Both] {
            assert_eq!(CrosshairMode::from_name(mode.as_str()), Some(mode));
        }
        assert_eq!(CrosshairMode::from_name("diagonal"), None);
    }

    #[test]
    fn drawing_a_shown_crosshair_reaches_the_backend() {
        let mut crosshair = ChartCrosshair::new(Rect::new(0, 0, 200, 100));
        crosshair.show_at_data(Point::new(40, 50), (1.0, 2.0));
        let svg = crate::widget::svg::render_to_svg(&mut crosshair);
        assert!(svg.contains("line"), "the reference lines were drawn");
    }
}
