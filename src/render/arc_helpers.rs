//! Arc and circle drawing helpers.

use crate::core::Point;
use crate::render::RenderContext;

/// Returns a point on the circle at the given angle (in radians).
/// Angle 0 is at 3 o'clock (right), angles increase clockwise.
#[inline]
pub fn point_on_circle(center: Point, radius: f32, angle: f32) -> Point {
    Point::new(
        center.x + (radius * angle.cos()).round() as i32,
        center.y + (radius * angle.sin()).round() as i32,
    )
}

/// Draws an arc from `start_angle` to `end_angle` using line segments.
pub fn draw_arc_segments(
    context: &mut RenderContext,
    center: Point,
    radius: f32,
    start_angle: f32,
    end_angle: f32,
    color: crate::core::Color,
    stroke_width: u32,
) {
    let segments = 40; // Number of segments for a smooth arc
    let total_angle = end_angle - start_angle;
    if total_angle.abs() < 0.001 {
        return;
    }
    let step = total_angle / segments as f32;

    let mut prev = point_on_circle(center, radius, start_angle);
    for i in 1..=segments {
        let angle = start_angle + step * i as f32;
        let curr = point_on_circle(center, radius, angle);
        context.draw_line_stroke(prev, curr, color, stroke_width);
        prev = curr;
    }
}
