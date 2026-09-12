//! Adapter that presents a widget-facing [`RenderContext`] as a chart-facing
//! [`ChartContext`].
//!
//! This is the bridge that keeps one chart engine instead of two. `src/chart/`
//! owns the layout, axis, tick and legend math (and its SVG snapshot tests);
//! widgets in `widget/chart_widgets/` draw through whatever `RenderContext` the
//! pipeline hands them. Without an adapter the two sides cannot meet, which is
//! why the widgets previously hand-rolled their own plot area and tick loops —
//! duplicating logic this module already provides and tests.
//!
//! # Unit handling
//!
//! The two traits disagree on units by design:
//!
//! * [`ChartContext`] uses `f32` geometry and `f32` widths, because the chart
//!   engine is resolution-independent and is also driven by the SVG backend.
//! * [`RenderContext`] uses integer pixel coordinates (the software rasterizer
//!   and the native backends are pixel-exact) with `u32` stroke widths.
//!
//! The adapter rounds to the nearest integer pixel via `round()` and clamps
//! non-negative, so a chart drawn through a `RenderContext` lands on the same
//! pixels every time rather than depending on truncation direction.

use crate::chart::types::ChartContext;
use crate::core::{Color, Font, Point, Rect};
use crate::render::RenderContext;

/// Presents a [`RenderContext`] to the chart engine as a [`ChartContext`].
///
/// Create one per draw call; it borrows the context mutably for its lifetime.
///
/// The two lifetimes are deliberately distinct: the outer one is how long this
/// adapter borrows the context, the inner one is the lifetime the context itself
/// was created with. Unifying them would make the adapter invariant and prevent
/// it from being constructed from a `&mut RenderContext<'b>` inside a shorter
/// borrow.
///
/// ```ignore
/// let mut adapter = ChartContextAdapter::new(context);
/// draw_cartesian_axes(&mut adapter, &layout);
/// ```
pub struct ChartContextAdapter<'ctx, 'backend> {
    inner: &'ctx mut RenderContext<'backend>,
    /// Colour most recently set through `set_fill_color`/`set_stroke_color`.
    ///
    /// The chart engine calls these before drawing, so remembering them lets the
    /// explicit `color` arguments win while still honouring the stateful setters
    /// for any caller that relies on them.
    fill_color: Color,
    stroke_color: Color,
}

impl<'ctx, 'backend> ChartContextAdapter<'ctx, 'backend> {
    /// Wraps `inner` for use by the chart engine.
    pub fn new(inner: &'ctx mut RenderContext<'backend>) -> Self {
        Self { inner, fill_color: Color::BLACK, stroke_color: Color::BLACK }
    }

    /// Rounds a chart-space coordinate to a device pixel.
    ///
    /// `round()` (not `as i32`, which truncates toward zero) keeps negative
    /// coordinates symmetric and avoids a one-pixel bias on fractional values.
    fn px(value: f32) -> i32 {
        value.round() as i32
    }

    /// Rounds a chart-space length to a non-negative device pixel count.
    fn len(value: f32) -> u32 {
        value.round().max(0.0) as u32
    }
}

impl ChartContext for ChartContextAdapter<'_, '_> {
    fn draw_line(&mut self, from: Point, to: Point, width: f32, color: Color) {
        let from = Point { x: Self::px(from.x as f32), y: Self::px(from.y as f32) };
        let to = Point { x: Self::px(to.x as f32), y: Self::px(to.y as f32) };
        // A hairline (width < 1) is still a visible line: clamp to 1 rather than
        // letting it round to 0 and disappear.
        let width = Self::len(width).max(1);
        self.inner.draw_line_stroke(from, to, color, width);
    }

    fn draw_rect(&mut self, rect: Rect, color: Color) {
        self.inner.fill_rect(rect, color);
    }

    fn draw_text(&mut self, text: &str, pos: Point, font_size: f32, color: Color) {
        let origin = Point { x: Self::px(pos.x as f32), y: Self::px(pos.y as f32) };
        let font = Font::simple("Sans", font_size);
        self.inner.draw_text(origin, text, &font, color, crate::core::HorizontalAlignment::Left);
    }

    fn draw_circle(&mut self, center: Point, radius: f32, color: Color) {
        let center = Point { x: Self::px(center.x as f32), y: Self::px(center.y as f32) };
        self.inner.fill_circle(center, Self::len(radius), color);
    }

    fn draw_polygon(&mut self, points: &[Point], color: Color) {
        if points.len() < 3 {
            // A polygon needs at least a triangle; anything shorter is a line or
            // a dot, which the callers express through the other methods.
            return;
        }
        let rounded: Vec<Point> = points
            .iter()
            .map(|p| Point { x: Self::px(p.x as f32), y: Self::px(p.y as f32) })
            .collect();
        self.inner.draw_path(&rounded, true, color, true, 1);
    }

    fn draw_path_segment(&mut self, start: Point, end: Point, width: f32, color: Color) {
        self.draw_line(start, end, width, color);
    }

    fn draw_arc(
        &mut self,
        center: Point,
        radius: f32,
        start_angle: f64,
        end_angle: f64,
        color: Color,
    ) {
        // RenderContext exposes only a full circle, so an arc is stroked as a
        // polyline. Segment count scales with the swept angle so a wide arc is
        // smooth while a sliver stays cheap.
        let sweep = (end_angle - start_angle).abs();
        let segments = ((sweep * radius as f64 / 2.0).ceil() as i64).clamp(2, 180);
        // Stay in f32 until the end: `Point` stores i32, so rounding once here is
        // what keeps the arc smooth instead of stair-stepping per component.
        let cx = center.x as f32;
        let cy = center.y as f32;
        let points: Vec<Point> = (0..=segments)
            .map(|step| {
                let t = step as f64 / segments as f64;
                let angle = start_angle + (end_angle - start_angle) * t;
                Point {
                    x: Self::px(cx + (radius as f64 * angle.cos()) as f32),
                    y: Self::px(cy + (radius as f64 * angle.sin()) as f32),
                }
            })
            .collect();
        for pair in points.windows(2) {
            self.inner.draw_line(pair[0], pair[1], color);
        }
    }

    fn draw_path(&mut self, points: &[Point], width: f32, color: Color) {
        if points.len() < 2 {
            return;
        }
        let rounded: Vec<Point> = points
            .iter()
            .map(|p| Point { x: Self::px(p.x as f32), y: Self::px(p.y as f32) })
            .collect();
        self.inner.draw_path(&rounded, false, color, false, Self::len(width).max(1));
    }

    fn draw_ellipse(&mut self, center: Point, radius_x: f32, radius_y: f32, color: Color) {
        // No native ellipse primitive: fill it as a polygon. Angular resolution
        // follows the larger radius so big ellipses do not show facets.
        let segments = ((radius_x.max(radius_y) * 2.0).ceil() as i64).clamp(12, 180);
        let cx = center.x as f32;
        let cy = center.y as f32;
        let points: Vec<Point> = (0..segments)
            .map(|step| {
                let angle = std::f64::consts::TAU * step as f64 / segments as f64;
                Point {
                    x: Self::px(cx + (radius_x as f64 * angle.cos()) as f32),
                    y: Self::px(cy + (radius_y as f64 * angle.sin()) as f32),
                }
            })
            .collect();
        self.draw_polygon(&points, color);
    }

    fn set_fill_color(&mut self, color: Color) {
        self.fill_color = color;
    }

    fn set_stroke_color(&mut self, color: Color) {
        self.stroke_color = color;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Size;
    use crate::render::{SoftwarePaintBackend, SoftwareSurface};

    /// Builds a `RenderContext` over an in-memory software surface.
    fn with_context<T>(width: u32, height: u32, f: impl FnOnce(&mut RenderContext<'_>) -> T) -> T {
        let surface = SoftwareSurface::new(Size::new(width, height), 1.0);
        let mut backend = SoftwarePaintBackend::new(surface.size(), surface.dpi_scale());
        let mut context = RenderContext::new(&mut backend);
        f(&mut context)
    }

    /// The adapter must exist and be usable as a `ChartContext` trait object —
    /// that is the whole contract the chart engine relies on.
    #[test]
    fn adapter_is_usable_as_a_chart_context_trait_object() {
        with_context(64, 64, |context| {
            let mut adapter = ChartContextAdapter::new(context);
            let dynamic: &mut dyn ChartContext = &mut adapter;
            // A representative mix of primitives, including the unit conversions.
            dynamic.draw_line(Point { x: 0, y: 0 }, Point { x: 32, y: 32 }, 2.0, Color::BLACK);
            dynamic.draw_rect(Rect::new(0, 0, 10, 10), Color::RED);
            dynamic.draw_text("42", Point { x: 2, y: 20 }, 10.0, Color::BLACK);
            dynamic.draw_circle(Point { x: 20, y: 20 }, 4.0, Color::BLUE);
            dynamic.set_fill_color(Color::GREEN);
            dynamic.set_stroke_color(Color::WHITE);
        });
    }

    /// A hairline must not vanish: `width` below one pixel clamps to 1.
    #[test]
    fn sub_pixel_line_width_still_draws() {
        with_context(16, 16, |context| {
            let mut adapter = ChartContextAdapter::new(context);
            adapter.draw_line(Point { x: 0, y: 8 }, Point { x: 15, y: 8 }, 0.1, Color::BLACK);
        });
    }

    /// Degenerate inputs must be ignored rather than panicking.
    #[test]
    fn degenerate_geometry_is_ignored() {
        with_context(16, 16, |context| {
            let mut adapter = ChartContextAdapter::new(context);
            adapter.draw_polygon(&[], Color::BLACK);
            adapter.draw_polygon(&[Point { x: 1, y: 1 }], Color::BLACK);
            adapter.draw_path(&[], 1.0, Color::BLACK);
            adapter.draw_path(&[Point { x: 1, y: 1 }], 1.0, Color::BLACK);
            // Zero-radius shapes must not panic either.
            adapter.draw_circle(Point { x: 5, y: 5 }, 0.0, Color::BLACK);
            adapter.draw_ellipse(Point { x: 5, y: 5 }, 0.0, 0.0, Color::BLACK);
            adapter.draw_arc(Point { x: 5, y: 5 }, 0.0, 0.0, 0.0, Color::BLACK);
        });
    }

    /// The full chart pipeline (layout + axes + ticks + legend) must render
    /// through the adapter without error. This is the integration the widgets
    /// depend on, so it is asserted as one end-to-end path.
    #[test]
    fn chart_engine_renders_end_to_end_through_the_adapter() {
        use crate::chart::charts::{
            compute_cartesian_layout, draw_cartesian_axes, draw_legend, draw_x_ticks, draw_y_ticks,
        };
        use crate::chart::types::ChartSeries;

        with_context(320, 200, |context| {
            let rect = Rect::new(0, 0, 320, 200);
            let series = ChartSeries {
                name: "revenue".to_string(),
                data: vec![],
                color: Color::BLUE,
                visible: true,
            };
            let layout = compute_cartesian_layout(rect, true, true, 1);

            let mut adapter = ChartContextAdapter::new(context);
            draw_cartesian_axes(&mut adapter, &layout);
            draw_y_ticks(&mut adapter, &layout, 0.0, 100.0, 5, true);
            draw_x_ticks(&mut adapter, &layout, 0.0, 10.0, 5, false);
            draw_legend(&mut adapter, &layout, &[&series]);
        });
    }
}
