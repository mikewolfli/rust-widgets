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
//!
//! # One kernel for both spellings
//!
//! This type used to exist twice: this companion, and a second `ChartCrosshair` inside
//! `special_widgets::chart` that used a different `WidgetKind` and a separate draw/hit path.
//! `special_widgets::chart` now re-exports this type instead of defining its own, so both paths
//! paint the same lines, obey the same visibility rules, and report through the same signals.
//! The surface the embedded spelling published (`position`, `show_axes`, `follow_pointer`,
//! `position_changed`) is kept alongside the companion surface (`show_at_data`, `mode`,
//! `crosshair_moved`).

use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

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
///
/// The two reference lines are stored as independent flags (`show_x` / `show_y`) because that is
/// what the embedded spelling published; [`CrosshairMode`] is derived from them for the companion
/// spelling, so the two views cannot disagree.
pub struct ChartCrosshair {
    base: BaseWidget,
    enabled: bool,
    /// Whether the vertical reference line is painted.
    show_x: bool,
    /// Whether the horizontal reference line is painted.
    show_y: bool,
    snap_to_point: bool,
    line_color: Option<Color>,
    line_width: u32,
    show_axis_values: bool,
    show_marker: bool,
    /// The pixel the pointer is at, in the chart's own coordinates.
    point: Option<Point>,
    /// The data coordinate the pixel stands for, published through `crosshair_moved`.
    data: Option<(f64, f64)>,
    /// Whether the anchor follows pointer motion between hovers.
    follow_pointer: bool,
    /// Emitted with the anchor when the crosshair moves.
    pub position_changed: Signal1<Point>,
    /// Emitted with the data coordinate on show/move and `None` on hide.
    pub crosshair_moved: Signal1<Option<(f64, f64)>>,
}

impl ChartCrosshair {
    /// Creates a crosshair covering `geometry` (the chart's plot rect).
    ///
    /// The kind is [`WidgetKind::ChartTooltip`] because a crosshair is a chart overlay, exactly
    /// like the tooltip bubble it sits beside. The embedded spelling used this kind while the
    /// companion used `Panel`; the two are now one control, so the overlay kind wins.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ChartTooltip, geometry, "ChartCrosshair"),
            enabled: true,
            show_x: true,
            show_y: true,
            snap_to_point: false,
            line_color: None,
            line_width: 1,
            show_axis_values: true,
            show_marker: true,
            point: None,
            data: None,
            follow_pointer: true,
            position_changed: Signal1::new(),
            crosshair_moved: Signal1::new(),
        }
    }

    /// The anchor, or `None` when the crosshair is hidden.
    pub fn position(&self) -> Option<Point> {
        self.point
    }

    /// Moves the anchor, or hides the crosshair with `None`.
    pub fn set_position(&mut self, position: Option<Point>) {
        if self.point == position {
            return;
        }
        self.point = position;
        match position {
            Some(point) => {
                self.position_changed.emit(point);
                if let Some(data) = self.data {
                    self.crosshair_moved.emit(Some(data));
                }
            }
            None => {
                // Withdrawing the anchor also withdraws the data coordinate: a value left
                // published after the lines are gone would claim a reading nothing shows.
                if self.data.take().is_some() {
                    self.crosshair_moved.emit(None);
                }
            }
        }
        self.base.request_redraw();
    }

    /// Shows the crosshair at a pixel position without a data coordinate.
    ///
    /// Kept for a caller that has no data mapping; the `crosshair_moved` payload then stays
    /// whatever the last snapped point set, because inventing a data value from pixels would be
    /// a number nothing measured.
    pub fn show_at(&mut self, point: Point) {
        self.set_position(Some(point));
    }

    /// Shows the crosshair at a pixel and publishes the data coordinate it stands for.
    pub fn show_at_data(&mut self, point: Point, data: (f64, f64)) {
        self.enabled = true;
        let moved = self.point != Some(point);
        self.point = Some(point);
        self.data = Some(data);
        if moved {
            self.position_changed.emit(point);
        }
        self.crosshair_moved.emit(Some(data));
        self.base.request_redraw();
    }

    /// Hides the crosshair.
    pub fn hide(&mut self) {
        let was_visible = self.point.is_some();
        let had_data = self.data.is_some();
        if !was_visible && !had_data {
            return;
        }
        self.point = None;
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
    ///
    /// The degenerate `show_axes() == (false, false)` state has no [`CrosshairMode`] spelling, so
    /// it reports [`CrosshairMode::Both`]; nothing is painted while both flags are false.
    pub fn mode(&self) -> CrosshairMode {
        match (self.show_x, self.show_y) {
            (true, false) => CrosshairMode::Vertical,
            (false, true) => CrosshairMode::Horizontal,
            _ => CrosshairMode::Both,
        }
    }

    /// Sets the line set painted.
    pub fn set_mode(&mut self, mode: CrosshairMode) {
        let (show_x, show_y) = match mode {
            CrosshairMode::Vertical => (true, false),
            CrosshairMode::Horizontal => (false, true),
            CrosshairMode::Both => (true, true),
        };
        self.set_show_axes(show_x, show_y);
    }

    /// Whether the vertical line is painted.
    pub fn show_axes(&self) -> (bool, bool) {
        (self.show_x, self.show_y)
    }

    /// Enables or disables each reference line independently.
    pub fn set_show_axes(&mut self, show_x: bool, show_y: bool) {
        if self.show_x == show_x && self.show_y == show_y {
            return;
        }
        self.show_x = show_x;
        self.show_y = show_y;
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

    /// The embedded spelling of [`Self::snap_to_point`].
    pub fn snap_to_data_point(&self) -> bool {
        self.snap_to_point
    }

    /// The embedded spelling of [`Self::set_snap_to_point`].
    pub fn set_snap_to_data_point(&mut self, snap: bool) {
        self.set_snap_to_point(snap);
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

    /// Whether the anchor follows pointer motion between hovers.
    pub fn follow_pointer(&self) -> bool {
        self.follow_pointer
    }

    /// Enables or disables pointer following.
    pub fn set_follow_pointer(&mut self, follow_pointer: bool) {
        self.follow_pointer = follow_pointer;
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
    impl_widget_property_hooks!();
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

        if self.show_x {
            context.draw_line_stroke_aa(
                Point::new(point.x, top),
                Point::new(point.x, bottom),
                color,
                width,
            );
        }
        if self.show_y {
            context.draw_line_stroke_aa(
                Point::new(left, point.y),
                Point::new(right, point.y),
                color,
                width,
            );
        }
        // A marker on its own would not explain which axes it belongs to, so it is only painted
        // while at least one reference line is visible.
        if self.show_marker && (self.show_x || self.show_y) {
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

impl EventHandler for ChartCrosshair {
    fn handle_event(&mut self, event: &Event) {
        // A host may drive the crosshair itself (the charts do), but a standalone crosshair
        // follows the pointer, which is what the embedded spelling always did. When the host
        // supplies an already-snapped anchor, `follow_pointer` should be switched off.
        self.base.handle_event(event);
        match event {
            Event::MouseMove { pos } | Event::PointerMove { pos, .. } => {
                if self.follow_pointer && self.point.is_some() {
                    self.set_position(Some(*pos));
                }
            }
            Event::MouseLeave { .. } => {
                if self.follow_pointer {
                    self.hide();
                }
            }
            Event::KeyPress { key: 27, .. } => self.hide(),
            _ => {}
        }
    }
}

impl WidgetProperties for ChartCrosshair {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            // `Null` is the published spelling of "hidden": a point a designer can express, rather
            // than a separate visibility flag that could disagree with it.
            "position" => Ok(match self.point {
                Some(point) => CapabilityValue::Tuple(crate::compat::Vec::from([
                    CapabilityValue::Int(i64::from(point.x)),
                    CapabilityValue::Int(i64::from(point.y)),
                ])),
                None => CapabilityValue::Null,
            }),
            "snap_to_data_point" => Ok(CapabilityValue::Bool(self.snap_to_point)),
            "show_x" => Ok(CapabilityValue::Bool(self.show_x)),
            "show_y" => Ok(CapabilityValue::Bool(self.show_y)),
            "follow_pointer" => Ok(CapabilityValue::Bool(self.follow_pointer)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "position" => {
                let position = match value {
                    CapabilityValue::Null => None,
                    CapabilityValue::Tuple(items) if items.len() == 2 => {
                        let mut items = items.into_iter();
                        let x = match items.next() {
                            Some(CapabilityValue::Int(x)) => x as i32,
                            _ => return Err(CapabilityAccessError::TypeMismatch),
                        };
                        let y = match items.next() {
                            Some(CapabilityValue::Int(y)) => y as i32,
                            _ => return Err(CapabilityAccessError::TypeMismatch),
                        };
                        Some(Point::new(x, y))
                    }
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_position(position);
                Ok(())
            }
            "snap_to_data_point" => {
                let snap = match value {
                    CapabilityValue::Bool(snap) => snap,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_snap_to_point(snap);
                Ok(())
            }
            "show_x" | "show_y" => {
                let CapabilityValue::Bool(flag) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                if name == "show_x" {
                    self.set_show_axes(flag, self.show_y);
                } else {
                    self.set_show_axes(self.show_x, flag);
                }
                Ok(())
            }
            "follow_pointer" => {
                let CapabilityValue::Bool(follow) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                self.set_follow_pointer(follow);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "position",
            "snap_to_data_point",
            "show_x",
            "show_y",
            "follow_pointer",
            BASE_PROPERTY_NAMES
        ]
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

    /// The embedded spelling's surface is kept on the one kernel: an anchor read-back, an
    /// independent two-axis switch and the `position_changed` signal.
    #[test]
    fn the_embedded_anchor_surface_survives_the_merge() {
        let mut crosshair = ChartCrosshair::new(Rect::new(0, 0, 200, 100));
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = std::sync::Arc::clone(&seen);
        crosshair.position_changed.connect(move |point| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*point);
            }
        });
        crosshair.show_at(Point::new(20, 40));
        assert_eq!(crosshair.position(), Some(Point::new(20, 40)));
        assert_eq!(*seen.lock().expect("lock"), vec![Point::new(20, 40)]);

        crosshair.set_show_axes(false, true);
        assert_eq!(crosshair.show_axes(), (false, true));
        assert_eq!(crosshair.mode(), CrosshairMode::Horizontal);
    }
}
