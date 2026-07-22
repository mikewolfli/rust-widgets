//! RangeSlider widget — a dual-handle slider for selecting a numeric range.
//!
//! The RangeSlider widget provides two draggable handles on a horizontal or
//! vertical track, allowing the user to select a lower and upper bound value
//! within a configurable range. The region between the handles is visually
//! highlighted.

use crate::core::{Color, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Orientation of the RangeSlider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
pub enum RangeSliderOrientation {
    /// Horizontal slider (left-to-right).
    #[default]
    Horizontal,
    /// Vertical slider (bottom-to-top).
    Vertical,
}


/// A dual-handle range slider for selecting a min-max value range.
///
/// The slider has two draggable handles: a lower handle and an upper handle.
/// The lower handle cannot exceed the upper handle, and the range between them
/// respects a configurable minimum range constraint.
pub struct RangeSlider {
    base: BaseWidget,
    min_value: f64,
    max_value: f64,
    lower_value: f64,
    upper_value: f64,
    step: f64,
    orientation: RangeSliderOrientation,
    min_range: f64,
    /// Emitted when the range (lower, upper) changes.
    pub range_changed: Signal1<(f64, f64)>,
    /// Which handle is currently being dragged: None, Some(true) for lower, Some(false) for upper.
    dragging: Option<bool>,
}

impl RangeSlider {
    /// Creates a new RangeSlider with the given geometry.
    ///
    /// Default range is 0.0 to 100.0, step 1.0, min_range 0.0.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::RangeSlider, geometry, "RangeSlider"),
            min_value: 0.0,
            max_value: 100.0,
            lower_value: 25.0,
            upper_value: 75.0,
            step: 1.0,
            orientation: RangeSliderOrientation::Horizontal,
            min_range: 0.0,
            range_changed: Signal1::new(),
            dragging: None,
        }
    }

    /// Returns the current lower value.
    pub fn lower_value(&self) -> f64 {
        self.lower_value
    }

    /// Sets the lower value, clamping it to be within bounds and respecting min_range.
    /// Emits `range_changed` if the value changes.
    pub fn set_lower_value(&mut self, value: f64) {
        let clamped = value.clamp(self.min_value, self.upper_value - self.min_range);
        let stepped = (clamped / self.step).round() * self.step;
        let stepped = stepped.max(self.min_value);
        let new_value = stepped.min(self.upper_value - self.min_range);
        if (new_value - self.lower_value).abs() > f64::EPSILON {
            self.lower_value = new_value;
            self.emit_range_changed();
            self.base.request_redraw();
        }
    }

    /// Returns the current upper value.
    pub fn upper_value(&self) -> f64 {
        self.upper_value
    }

    /// Sets the upper value, clamping it to be within bounds and respecting min_range.
    /// Emits `range_changed` if the value changes.
    pub fn set_upper_value(&mut self, value: f64) {
        let clamped = value.clamp(self.lower_value + self.min_range, self.max_value);
        let stepped = (clamped / self.step).round() * self.step;
        let stepped = stepped.min(self.max_value);
        let new_value = stepped.max(self.lower_value + self.min_range);
        if (new_value - self.upper_value).abs() > f64::EPSILON {
            self.upper_value = new_value;
            self.emit_range_changed();
            self.base.request_redraw();
        }
    }

    /// Sets both lower and upper values simultaneously, respecting all constraints.
    pub fn set_range(&mut self, lower: f64, upper: f64) {
        let lower = lower.clamp(self.min_value, self.max_value - self.min_range);
        let upper = upper.clamp(lower + self.min_range, self.max_value);
        let lower_stepped = (lower / self.step).round() * self.step;
        let upper_stepped = (upper / self.step).round() * self.step;
        let lower_stepped = lower_stepped.max(self.min_value);
        let upper_stepped = upper_stepped.max(lower_stepped + self.min_range).min(self.max_value);
        let lower_stepped = lower_stepped.min(upper_stepped - self.min_range);

        if (lower_stepped - self.lower_value).abs() > f64::EPSILON
            || (upper_stepped - self.upper_value).abs() > f64::EPSILON
        {
            self.lower_value = lower_stepped;
            self.upper_value = upper_stepped;
            self.emit_range_changed();
            self.base.request_redraw();
        }
    }

    /// Returns the minimum possible value.
    pub fn min_value(&self) -> f64 {
        self.min_value
    }

    /// Returns the maximum possible value.
    pub fn max_value(&self) -> f64 {
        self.max_value
    }

    /// Returns the current step increment.
    pub fn step(&self) -> f64 {
        self.step
    }

    /// Sets the step increment for handle movement.
    pub fn set_step(&mut self, step: f64) {
        self.step = step.max(0.001);
        self.base.request_redraw();
    }

    /// Returns the minimum allowed range (distance between lower and upper).
    pub fn min_range(&self) -> f64 {
        self.min_range
    }

    /// Sets the minimum allowed range between handles.
    pub fn set_min_range(&mut self, min_range: f64) {
        self.min_range = min_range.max(0.0);
        // Clamp current values to respect new min_range
        if self.upper_value - self.lower_value < self.min_range {
            self.upper_value = (self.lower_value + self.min_range).min(self.max_value);
            self.emit_range_changed();
        }
        self.base.request_redraw();
    }

    /// Emits the range_changed signal.
    fn emit_range_changed(&self) {
        self.range_changed.emit((self.lower_value, self.upper_value));
    }

    /// Converts a value to pixel position on the track.
    fn value_to_pixel(&self, value: f64, rect: &Rect) -> i32 {
        let handle_radius = 8i32;
        let track_start = rect.x + handle_radius;
        let track_end = rect.x + rect.width as i32 - handle_radius;
        let track_length = (track_end - track_start) as f64;
        if self.orientation == RangeSliderOrientation::Horizontal {
            if (self.max_value - self.min_value).abs() < f64::EPSILON {
                return track_start;
            }
            let ratio = (value - self.min_value) / (self.max_value - self.min_value);
            track_start + (track_length * ratio) as i32
        } else {
            // Vertical: bottom is min, top is max
            let track_start_v = rect.y + handle_radius;
            let track_end_v = rect.y + rect.height as i32 - handle_radius;
            let track_length_v = (track_end_v - track_start_v) as f64;
            if (self.max_value - self.min_value).abs() < f64::EPSILON {
                return track_end_v;
            }
            let ratio = (value - self.min_value) / (self.max_value - self.min_value);
            track_end_v - (track_length_v * ratio) as i32
        }
    }

    /// Converts a pixel position to a value on the track.
    fn pixel_to_value(&self, pos: i32, rect: &Rect) -> f64 {
        let handle_radius = 8i32;
        if self.orientation == RangeSliderOrientation::Horizontal {
            let track_start = rect.x + handle_radius;
            let track_end = rect.x + rect.width as i32 - handle_radius;
            let track_length = (track_end - track_start) as f64;
            if track_length <= 0.0 {
                return self.min_value;
            }
            let clamped_pos = pos.clamp(track_start, track_end);
            let ratio = (clamped_pos - track_start) as f64 / track_length;
            self.min_value + ratio * (self.max_value - self.min_value)
        } else {
            let track_start = rect.y + handle_radius;
            let track_end = rect.y + rect.height as i32 - handle_radius;
            let track_length = (track_end - track_start) as f64;
            if track_length <= 0.0 {
                return self.min_value;
            }
            let clamped_pos = pos.clamp(track_start, track_end);
            let ratio = (track_end - clamped_pos) as f64 / track_length;
            self.min_value + ratio * (self.max_value - self.min_value)
        }
    }

    /// Checks if a point is within a handle's hit area.
    fn is_handle_hit(&self, pos: Point, rect: &Rect, is_lower: bool) -> bool {
        let value = if is_lower { self.lower_value } else { self.upper_value };
        let handle_radius = 10i32; // hit area radius
        let cx = self.value_to_pixel(value, rect);
        let cy = if self.orientation == RangeSliderOrientation::Horizontal {
            rect.y + rect.height as i32 / 2
        } else {
            rect.x + rect.width as i32 / 2
        };

        let handle_center = if self.orientation == RangeSliderOrientation::Horizontal {
            Point::new(cx, cy)
        } else {
            Point::new(cy, cx)
        };

        let dx = pos.x - handle_center.x;
        let dy = pos.y - handle_center.y;
        (dx * dx + dy * dy) <= (handle_radius * handle_radius)
    }
}

impl Widget for RangeSlider {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(200, 28)
    }
}

impl Draw for RangeSlider {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let is_enabled = self.base.is_enabled();
        let handle_radius = 8u32;

        // Track background
        let track_thickness = 6u32;
        let track_color = Color::rgba(200, 200, 200, 255);

        if self.orientation == RangeSliderOrientation::Horizontal {
            let track_y = rect.y + rect.height as i32 / 2 - track_thickness as i32 / 2;
            let track_rect = Rect::new(
                rect.x + handle_radius as i32,
                track_y,
                rect.width - handle_radius * 2,
                track_thickness,
            );
            context.fill_rounded_rect(track_rect, track_thickness / 2, track_color);

            // Highlighted range between handles
            let lower_x = self.value_to_pixel(self.lower_value, &rect);
            let upper_x = self.value_to_pixel(self.upper_value, &rect);
            if upper_x > lower_x {
                let range_rect =
                    Rect::new(lower_x, track_y, (upper_x - lower_x) as u32, track_thickness);
                let range_color = if is_enabled {
                    Color::rgba(52, 120, 246, 255)
                } else {
                    Color::rgba(180, 180, 180, 255)
                };
                context.fill_rounded_rect(range_rect, track_thickness / 2, range_color);
            }

            // Draw handles
            let center_y = rect.y + rect.height as i32 / 2;
            let handle_color =
                if is_enabled { Color::WHITE } else { Color::rgba(240, 240, 240, 255) };
            let handle_border = if is_enabled {
                Color::rgba(52, 120, 246, 255)
            } else {
                Color::rgba(180, 180, 180, 255)
            };

            for &value in &[self.lower_value, self.upper_value] {
                let cx = self.value_to_pixel(value, &rect);
                let handle_center = Point::new(cx, center_y);
                context.fill_circle(handle_center, handle_radius, handle_color);
                context.draw_circle_stroke(handle_center, handle_radius, handle_border, 2);
            }
        } else {
            // Vertical
            let track_x = rect.x + rect.width as i32 / 2 - track_thickness as i32 / 2;
            let track_rect = Rect::new(
                track_x,
                rect.y + handle_radius as i32,
                track_thickness,
                rect.height - handle_radius * 2,
            );
            context.fill_rounded_rect(track_rect, track_thickness / 2, track_color);

            // Highlighted range
            let lower_y = self.value_to_pixel(self.lower_value, &rect);
            let upper_y = self.value_to_pixel(self.upper_value, &rect);
            if upper_y < lower_y {
                let range_rect =
                    Rect::new(track_x, upper_y, track_thickness, (lower_y - upper_y) as u32);
                let range_color = if is_enabled {
                    Color::rgba(52, 120, 246, 255)
                } else {
                    Color::rgba(180, 180, 180, 255)
                };
                context.fill_rounded_rect(range_rect, track_thickness / 2, range_color);
            }

            // Draw handles
            let center_x = rect.x + rect.width as i32 / 2;
            let handle_color =
                if is_enabled { Color::WHITE } else { Color::rgba(240, 240, 240, 255) };
            let handle_border = if is_enabled {
                Color::rgba(52, 120, 246, 255)
            } else {
                Color::rgba(180, 180, 180, 255)
            };

            for &value in &[self.lower_value, self.upper_value] {
                let cy = self.value_to_pixel(value, &rect);
                let handle_center = Point::new(center_x, cy);
                context.fill_circle(handle_center, handle_radius, handle_color);
                context.draw_circle_stroke(handle_center, handle_radius, handle_border, 2);
            }
        }
    }
}

impl EventHandler for RangeSlider {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button } if *button == 1 => {
                let rect = self.geometry();
                // Check which handle is hit (upper first to give it priority)
                if self.is_handle_hit(*pos, &rect, false) {
                    self.dragging = Some(false); // upper handle
                } else if self.is_handle_hit(*pos, &rect, true) {
                    self.dragging = Some(true); // lower handle
                }
            }
            Event::MouseRelease { pos: _, button } if *button == 1 => {
                self.dragging = None;
            }
            Event::MouseMove { pos } => {
                if let Some(is_lower) = self.dragging {
                    let rect = self.geometry();
                    let raw_value = self.pixel_to_value(pos.x, &rect);
                    if is_lower {
                        self.set_lower_value(raw_value);
                    } else {
                        self.set_upper_value(raw_value);
                    }
                }
            }
            _ => {
                self.base.handle_event(event);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::svg::render_to_svg;

    #[test]
    fn range_slider_default_creation() {
        let rs = RangeSlider::new(Rect::new(0, 0, 300, 40));
        assert_eq!(rs.kind(), WidgetKind::RangeSlider);
        assert!((rs.lower_value() - 25.0).abs() < f64::EPSILON);
        assert!((rs.upper_value() - 75.0).abs() < f64::EPSILON);
        assert!((rs.min_value() - 0.0).abs() < f64::EPSILON);
        assert!((rs.max_value() - 100.0).abs() < f64::EPSILON);
        assert!((rs.step() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn range_slider_set_lower_value_respects_bounds() {
        let mut rs = RangeSlider::new(Rect::new(0, 0, 300, 40));
        rs.set_lower_value(30.0);
        assert!((rs.lower_value() - 30.0).abs() < f64::EPSILON);

        // Cannot exceed upper_value
        rs.set_lower_value(80.0);
        assert!((rs.lower_value() - 75.0).abs() < f64::EPSILON); // clamped to upper - min_range

        // Cannot go below min_value
        rs.set_lower_value(-10.0);
        assert!((rs.lower_value() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn range_slider_set_upper_value_respects_bounds() {
        let mut rs = RangeSlider::new(Rect::new(0, 0, 300, 40));
        rs.set_upper_value(90.0);
        assert!((rs.upper_value() - 90.0).abs() < f64::EPSILON);

        // Cannot go below lower_value
        rs.set_upper_value(10.0);
        assert!((rs.upper_value() - 25.0).abs() < f64::EPSILON); // clamped to lower + min_range

        // Cannot exceed max_value
        rs.set_upper_value(200.0);
        assert!((rs.upper_value() - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn range_slider_set_range() {
        let mut rs = RangeSlider::new(Rect::new(0, 0, 300, 40));
        rs.set_range(10.0, 50.0);
        assert!((rs.lower_value() - 10.0).abs() < f64::EPSILON);
        assert!((rs.upper_value() - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn range_slider_step_respected() {
        let mut rs = RangeSlider::new(Rect::new(0, 0, 300, 40));
        rs.set_step(5.0);

        rs.set_lower_value(12.0);
        assert!((rs.lower_value() - 10.0).abs() < f64::EPSILON); // rounds to nearest step (10)

        rs.set_lower_value(13.0);
        assert!((rs.lower_value() - 15.0).abs() < f64::EPSILON); // rounds to nearest step (15)
    }

    #[test]
    fn range_slider_min_range() {
        let mut rs = RangeSlider::new(Rect::new(0, 0, 300, 40));
        rs.set_min_range(20.0);
        assert!((rs.min_range() - 20.0).abs() < f64::EPSILON);

        // Enforce min_range
        rs.set_lower_value(80.0);
        assert!((rs.lower_value() - 55.0).abs() < f64::EPSILON); // 75 - 20 = 55
    }

    #[test]
    fn range_slider_range_changed_signal() {
        use std::sync::{Arc, Mutex};
        let mut rs = RangeSlider::new(Rect::new(0, 0, 300, 40));
        let captured = Arc::new(Mutex::new(None::<(f64, f64)>));
        rs.range_changed.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<(f64, f64)>| {
                *captured.lock().unwrap() = Some(*val);
            }
        });

        rs.set_lower_value(40.0);
        let result = captured.lock().unwrap();
        let (lower, upper) = result.unwrap();
        assert!((lower - 40.0).abs() < f64::EPSILON);
        assert!((upper - 75.0).abs() < f64::EPSILON);
    }

    #[test]
    fn range_slider_svg_output() {
        let mut rs = RangeSlider::new(Rect::new(0, 0, 300, 40));
        let svg = render_to_svg(&mut rs);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }
}
