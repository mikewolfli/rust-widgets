//! Meter widget — gauge with arc and needle indicator (BLUE13 R2.14).
//!
//! A simplified gauge/indicator widget that draws a 270° arc (starting from
//! 135°) as a background track, a colored arc up to the current value, and
//! a needle pointing at the value.

use crate::core::{deg_to_rad, Color, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::{RenderCommand, RenderContext};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Meter (gauge) widget — displays a value on an arc with a needle indicator.
pub struct Meter {
    base: BaseWidget,
    /// Current displayed value.
    value: u32,
    /// Minimum value of the range.
    min: u32,
    /// Maximum value of the range.
    max: u32,
    /// Number of tick marks.
    tick_count: u32,
}

impl Meter {
    /// Creates a new Meter widget with the given geometry.
    ///
    /// Defaults: value 0, range 0-100, 5 tick marks.
    pub fn new(rect: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Meter, rect, "Meter"),
            value: 0,
            min: 0,
            max: 100,
            tick_count: 5,
        }
    }

    /// Returns the current value.
    pub fn value(&self) -> u32 {
        self.value
    }

    /// Sets the value, clamped between min and max.
    ///
    /// Emits `changed` signal when the value actually changes.
    pub fn set_value(&mut self, v: u32) {
        let clamped = v.clamp(self.min, self.max);
        if self.value == clamped {
            return;
        }
        self.value = clamped;
        self.base.changed.emit();
    }

    /// Sets both minimum and maximum values in one call.
    ///
    /// The current value is re-clamped to the new range.
    pub fn set_range(&mut self, min: u32, max: u32) {
        self.min = min.min(max);
        self.max = max.max(min);
        let clamped = self.value.clamp(self.min, self.max);
        if self.value != clamped {
            self.value = clamped;
            self.base.changed.emit();
        }
    }

    /// Sets the number of tick marks drawn along the arc.
    pub fn set_tick_count(&mut self, count: u32) {
        self.tick_count = count.max(2);
    }

    /// Normalizes the current value to a fraction in [0.0, 1.0].
    fn normalized_value(&self) -> f32 {
        if self.max <= self.min {
            return 0.0;
        }
        (self.value - self.min) as f32 / (self.max - self.min) as f32
    }
}

impl Widget for Meter {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        Size::new(200, 200)
    }
}

impl EventHandler for Meter {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

impl Draw for Meter {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }

        let center = Point::new(rect.x + rect.width as i32 / 2, rect.y + rect.height as i32 / 2);

        // Radius is half the smaller dimension minus padding.
        let radius = rect.width.min(rect.height).saturating_sub(8) / 2;
        if radius < 10 {
            return;
        }

        // Arc angles: 270° sweep starting from 135° (top-right quadrant).
        // The offset of -90° converts from "0 = top" to "0 = 3 o'clock".
        let arc_start_deg = 135.0_f32;
        let arc_sweep_deg = 270.0_f32;
        let offset = -90.0_f32;

        let start_angle = deg_to_rad(arc_start_deg + offset);
        let end_angle = deg_to_rad(arc_start_deg + arc_sweep_deg + offset);
        let value_angle =
            deg_to_rad(arc_start_deg + arc_sweep_deg * self.normalized_value() + offset);

        // Resolve colors from style.
        let track_color = Color::rgb(230, 230, 230);
        let value_arc_color = self.style().background_color.unwrap_or(Color::rgb(0, 120, 215));
        let needle_color = self.style().text_color.unwrap_or(Color::rgb(60, 60, 60));
        let tick_color = Color::rgb(160, 160, 160);

        // Draw the background track arc (270° sweep, light gray).
        if (end_angle - start_angle).abs() > 0.001 {
            context.execute_command(RenderCommand::DrawArc {
                center,
                radius,
                start_angle,
                end_angle,
                color: track_color,
                filled: false,
            });
        }

        // Draw the value arc (colored arc from start to value position).
        if self.value > self.min && (value_angle - start_angle).abs() > 0.001 {
            context.execute_command(RenderCommand::DrawArc {
                center,
                radius,
                start_angle,
                end_angle: value_angle,
                color: value_arc_color,
                filled: false,
            });
        }

        // Draw tick marks at regular intervals along the arc.
        if self.tick_count >= 2 {
            let tick_outer = radius;
            let tick_inner = radius.saturating_sub(6).max(1);
            let tick_step = arc_sweep_deg / (self.tick_count - 1) as f32;

            for i in 0..self.tick_count {
                let tick_angle_deg = arc_start_deg + tick_step * i as f32;
                let tick_rad = deg_to_rad(tick_angle_deg + offset);

                let outer_x = center.x + (tick_outer as f32 * tick_rad.cos()) as i32;
                let outer_y = center.y + (tick_outer as f32 * tick_rad.sin()) as i32;
                let inner_x = center.x + (tick_inner as f32 * tick_rad.cos()) as i32;
                let inner_y = center.y + (tick_inner as f32 * tick_rad.sin()) as i32;

                context.draw_line_stroke(
                    Point::new(inner_x, inner_y),
                    Point::new(outer_x, outer_y),
                    tick_color,
                    1,
                );
            }
        }

        // Draw the needle line from center outward to the value angle.
        let needle_length = radius.saturating_sub(8).max(1);
        let needle_x = center.x + (needle_length as f32 * value_angle.cos()) as i32;
        let needle_y = center.y + (needle_length as f32 * value_angle.sin()) as i32;
        context.draw_line_stroke(center, Point::new(needle_x, needle_y), needle_color, 2);

        // Draw a small filled circle at the center as the needle pivot.
        context.fill_circle(center, 4, needle_color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Color, Rect, Size};
    use crate::render::{PaintBackend, SoftwarePaintBackend};

    #[test]
    fn meter_creation() {
        let meter = Meter::new(Rect::new(0, 0, 200, 200));
        assert_eq!(meter.value(), 0);
        assert_eq!(meter.min, 0);
        assert_eq!(meter.max, 100);
        assert_eq!(meter.tick_count, 5);
        assert_eq!(meter.kind(), WidgetKind::Meter);
    }

    #[test]
    fn meter_set_value() {
        let mut meter = Meter::new(Rect::new(0, 0, 200, 200));
        meter.set_value(50);
        assert_eq!(meter.value(), 50);

        // Above max should clamp to 100.
        meter.set_value(200);
        assert_eq!(meter.value(), 100);

        // Below min should clamp to 0.
        meter.set_value(0);
        assert_eq!(meter.value(), 0);
    }

    #[test]
    fn meter_set_range() {
        let mut meter = Meter::new(Rect::new(0, 0, 200, 200));
        meter.set_range(10, 50);
        assert_eq!(meter.min, 10);
        assert_eq!(meter.max, 50);

        // Value should be re-clamped to the new range.
        meter.set_value(30);
        assert_eq!(meter.value(), 30);

        // Value below new min clamps up.
        meter.set_value(5);
        assert_eq!(meter.value(), 10);

        // Value above new max clamps down.
        meter.set_value(60);
        assert_eq!(meter.value(), 50);
    }

    #[test]
    fn meter_draw_no_panic() {
        let mut meter = Meter::new(Rect::new(0, 0, 200, 200));
        meter.set_value(65);

        let mut backend = SoftwarePaintBackend::new(Size::new(200, 200), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut context = RenderContext::new(&mut backend);
        meter.draw(&mut context);
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert!(!rgba.is_empty());
    }

    #[test]
    fn meter_draw_zero_value_no_panic() {
        let mut meter = Meter::new(Rect::new(0, 0, 200, 200));
        meter.set_value(0);

        let mut backend = SoftwarePaintBackend::new(Size::new(200, 200), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut context = RenderContext::new(&mut backend);
        meter.draw(&mut context);
        backend.end_frame();
    }

    #[test]
    fn meter_draw_zero_geometry_no_panic() {
        let mut meter = Meter::new(Rect::new(0, 0, 0, 0));
        let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut context = RenderContext::new(&mut backend);
        meter.draw(&mut context);
        backend.end_frame();
    }

    #[test]
    fn meter_set_tick_count() {
        let mut meter = Meter::new(Rect::new(0, 0, 200, 200));
        assert_eq!(meter.tick_count, 5);

        meter.set_tick_count(10);
        assert_eq!(meter.tick_count, 10);

        // Minimum is 2.
        meter.set_tick_count(0);
        assert_eq!(meter.tick_count, 2);
    }

    #[test]
    fn meter_normalized_value() {
        let meter = Meter::new(Rect::new(0, 0, 200, 200));
        assert!((meter.normalized_value() - 0.0).abs() < f32::EPSILON);

        let mut meter = Meter::new(Rect::new(0, 0, 200, 200));
        meter.set_value(50);
        assert!((meter.normalized_value() - 0.5).abs() < f32::EPSILON);

        meter.set_value(100);
        assert!((meter.normalized_value() - 1.0).abs() < f32::EPSILON);

        // Empty range returns 0.
        meter.set_range(50, 50);
        assert!((meter.normalized_value() - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn meter_size_hint() {
        let meter = Meter::new(Rect::new(0, 0, 100, 100));
        assert_eq!(meter.size_hint(), Size::new(200, 200));
    }

    #[test]
    fn meter_geometry_delegation() {
        let mut meter = Meter::new(Rect::new(0, 0, 200, 200));
        meter.set_geometry(Rect::new(10, 10, 150, 150));
        assert_eq!(meter.geometry(), Rect::new(10, 10, 150, 150));
    }

    #[test]
    fn meter_event_delegation() {
        let mut meter = Meter::new(Rect::new(0, 0, 200, 200));
        // Handle event without panicking.
        meter.handle_event(&Event::KeyDown((37, 0)));
    }
}
