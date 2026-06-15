//! ProgressCircle widget — a circular progress indicator.
//!
//! The ProgressCircle widget displays a circular progress track with a filled arc
//! representing the current progress value (0.0 to 1.0). It supports both determinate
//! mode (showing actual progress) and indeterminate mode (animated spinning arc).
//! The track and progress colors, as well as stroke width, are customizable.

use crate::core::{Color, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// ProgressCircle widget — a Material-style circular progress indicator.
///
/// In determinate mode, draws a track circle and a progress arc.
/// In indeterminate mode, draws a spinning arc segment that cycles around the circle.
///
/// The widget uses the `stroke_width` to determine the thickness of both the
/// track and progress arcs. The progress arc is drawn using closely-spaced
/// line segments approximating a circular arc.
pub struct ProgressCircle {
    base: BaseWidget,
    value: f32,
    indeterminate: bool,
    track_color: Color,
    progress_color: Color,
    stroke_width: f32,
}

impl ProgressCircle {
    /// Creates a new ProgressCircle widget with the given geometry.
    ///
    /// Defaults: value 0.0, determinate, light gray track, blue progress, stroke width 4.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ProgressCircle, geometry, "ProgressCircle"),
            value: 0.0,
            indeterminate: false,
            track_color: Color::rgba(220, 220, 220, 200),
            progress_color: Color::PRIMARY,
            stroke_width: 4.0,
        }
    }

    /// Returns the current progress value (0.0 to 1.0).
    pub fn value(&self) -> f32 {
        self.value
    }

    /// Sets the progress value. Clamped to 0.0..=1.0.
    pub fn set_value(&mut self, value: f32) {
        self.value = value.clamp(0.0, 1.0);
        self.base.request_redraw();
    }

    /// Returns whether the indicator is in indeterminate (spinning) mode.
    pub fn is_indeterminate(&self) -> bool {
        self.indeterminate
    }

    /// Sets whether the indicator shows indeterminate (spinning) animation.
    pub fn set_indeterminate(&mut self, indeterminate: bool) {
        self.indeterminate = indeterminate;
        self.base.request_redraw();
    }

    /// Returns the current track color.
    pub fn track_color(&self) -> Color {
        self.track_color
    }

    /// Sets the track (background circle) color.
    pub fn set_track_color(&mut self, color: Color) {
        self.track_color = color;
        self.base.request_redraw();
    }

    /// Returns the current progress (foreground arc) color.
    pub fn progress_color(&self) -> Color {
        self.progress_color
    }

    /// Sets the progress (foreground arc) color.
    pub fn set_progress_color(&mut self, color: Color) {
        self.progress_color = color;
        self.base.request_redraw();
    }

    /// Returns the current stroke width in logical pixels.
    pub fn stroke_width(&self) -> f32 {
        self.stroke_width
    }

    /// Sets the stroke width for both track and progress arcs.
    pub fn set_stroke_width(&mut self, width: f32) {
        self.stroke_width = width.max(0.5);
        self.base.request_redraw();
    }

    /// Computes the center point and radius of the progress circle.
    fn center_and_radius(&self) -> Option<(Point, f32)> {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return None;
        }
        let cx = rect.x + (rect.width as i32) / 2;
        let cy = rect.y + (rect.height as i32) / 2;
        let radius = (rect.width.min(rect.height) as f32 / 2.0) - self.stroke_width / 2.0 - 1.0;
        if radius <= 0.0 {
            return None;
        }
        Some((Point::new(cx, cy), radius))
    }

    /// Returns a point on the circle at the given angle (in radians).
    /// Angle 0 is at 3 o'clock (right), angles increase clockwise.
    fn point_on_circle(center: Point, radius: f32, angle: f32) -> Point {
        Point::new(
            center.x + (radius * angle.cos()) as i32,
            center.y + (radius * angle.sin()) as i32,
        )
    }

    /// Draws an arc from `start_angle` to `end_angle` using line segments.
    fn draw_arc_segments(
        context: &mut RenderContext,
        center: Point,
        radius: f32,
        start_angle: f32,
        end_angle: f32,
        color: Color,
        stroke_width: u32,
    ) {
        let segments = 40; // Number of segments for a smooth arc
        let total_angle = end_angle - start_angle;
        if total_angle.abs() < 0.001 {
            return;
        }
        let step = total_angle / segments as f32;

        let mut prev = Self::point_on_circle(center, radius, start_angle);
        for i in 1..=segments {
            let angle = start_angle + step * i as f32;
            let curr = Self::point_on_circle(center, radius, angle);
            context.draw_line_stroke(prev, curr, color, stroke_width);
            prev = curr;
        }
    }
}

impl Widget for ProgressCircle {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}

impl Draw for ProgressCircle {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }

        let Some((center, radius)) = self.center_and_radius() else {
            return;
        };

        let is_enabled = self.base.is_enabled();
        let stroke_w = self.stroke_width as u32;

        // Draw track (full circle)
        let track_color =
            if is_enabled { self.track_color } else { Color::rgba(200, 200, 200, 100) };

        // Draw track as a circle stroke
        context.draw_circle_stroke(center, radius as u32, track_color, stroke_w.max(1));

        if self.indeterminate {
            // In indeterminate mode, draw a single arc segment that sweeps ~135 degrees
            // The starting angle is animated using a time-based offset.
            let start_offset = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as f32
                * 0.003; // Rotation speed
            let arc_sweep = 2.4; // ~135 degrees in radians
            let start_angle = start_offset;
            let end_angle = start_angle + arc_sweep;

            let prog_color =
                if is_enabled { self.progress_color } else { Color::DISABLED_FOREGROUND };
            Self::draw_arc_segments(
                context,
                center,
                radius,
                start_angle,
                end_angle,
                prog_color,
                stroke_w.max(1),
            );
        } else if self.value > 0.0 {
            // In determinate mode, draw the progress arc from 12 o'clock
            // -PI/2 (12 o'clock) to -PI/2 + 2*PI*value
            let start_angle = -std::f32::consts::FRAC_PI_2;
            let end_angle = start_angle + 2.0 * std::f32::consts::PI * self.value;

            let prog_color =
                if is_enabled { self.progress_color } else { Color::DISABLED_FOREGROUND };
            Self::draw_arc_segments(
                context,
                center,
                radius,
                start_angle,
                end_angle,
                prog_color,
                stroke_w.max(1),
            );
        }
    }
}

impl EventHandler for ProgressCircle {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_circle_default_creation() {
        let pc = ProgressCircle::new(Rect::new(0, 0, 48, 48));
        assert_eq!(pc.kind(), WidgetKind::ProgressCircle);
        assert!((pc.value() - 0.0).abs() < f32::EPSILON);
        assert!(!pc.is_indeterminate());
        assert!((pc.stroke_width() - 4.0).abs() < f32::EPSILON);
        assert_eq!(pc.track_color(), Color::rgba(220, 220, 220, 200));
        assert_eq!(pc.progress_color(), Color::PRIMARY);
    }

    #[test]
    fn progress_circle_set_value() {
        let mut pc = ProgressCircle::new(Rect::new(0, 0, 48, 48));
        pc.set_value(0.5);
        assert!((pc.value() - 0.5).abs() < f32::EPSILON);

        pc.set_value(1.5); // Should clamp to 1.0
        assert!((pc.value() - 1.0).abs() < f32::EPSILON);

        pc.set_value(-0.5); // Should clamp to 0.0
        assert!((pc.value() - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn progress_circle_indeterminate_toggle() {
        let mut pc = ProgressCircle::new(Rect::new(0, 0, 48, 48));
        assert!(!pc.is_indeterminate());

        pc.set_indeterminate(true);
        assert!(pc.is_indeterminate());

        pc.set_indeterminate(false);
        assert!(!pc.is_indeterminate());
    }

    #[test]
    fn progress_circle_colors_and_stroke() {
        let mut pc = ProgressCircle::new(Rect::new(0, 0, 48, 48));

        pc.set_track_color(Color::LIGHT_GRAY);
        assert_eq!(pc.track_color(), Color::LIGHT_GRAY);

        pc.set_progress_color(Color::SUCCESS);
        assert_eq!(pc.progress_color(), Color::SUCCESS);

        pc.set_stroke_width(6.0);
        assert!((pc.stroke_width() - 6.0).abs() < f32::EPSILON);

        pc.set_stroke_width(-1.0); // Should clamp to minimum
        assert!((pc.stroke_width() - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn progress_circle_svg_output_determinate() {
        let mut pc = ProgressCircle::new(Rect::new(0, 0, 48, 48));
        pc.set_value(0.75);

        let svg = crate::widget::svg::render_to_svg(&mut pc);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }

    #[test]
    fn progress_circle_svg_output_indeterminate() {
        let mut pc = ProgressCircle::new(Rect::new(0, 0, 48, 48));
        pc.set_indeterminate(true);

        let svg = crate::widget::svg::render_to_svg(&mut pc);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }

    #[test]
    fn progress_circle_zero_geometry_no_crash() {
        let mut pc = ProgressCircle::new(Rect::new(0, 0, 0, 0));
        // Should not panic
        let svg = crate::widget::svg::render_to_svg(&mut pc);
        assert!(svg.starts_with("<svg"));
    }

    #[test]
    fn progress_circle_event_forwarding() {
        let mut pc = ProgressCircle::new(Rect::new(0, 0, 48, 48));
        // Should not panic
        pc.handle_event(&Event::MouseMove { pos: Point::new(10, 10) });
        pc.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1 });
    }
}
