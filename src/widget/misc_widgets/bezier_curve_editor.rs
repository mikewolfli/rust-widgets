//! BezierCurveEditor widget — interactive cubic bezier curve editor for creating custom easing curves.
//!
//! This widget provides a visual editor for cubic bezier curves with two control points,
//! a grid background, snap-to-grid support, and drag interaction. It can be used to design
//! custom easing functions for animations.

use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Control point handle radius in pixels.
const HANDLE_RADIUS: u32 = 8;
/// Grid color alpha value.
const GRID_ALPHA: u8 = 60;

/// BezierCurveEditor widget for creating custom easing curves.
pub struct BezierCurveEditor {
    base: BaseWidget,
    /// First control point as a fraction of the widget area (0.0–1.0).
    control_point1: (f32, f32),
    /// Second control point as a fraction of the widget area (0.0–1.0).
    control_point2: (f32, f32),
    /// Grid cell size in pixels.
    grid_size: f32,
    /// Whether to draw the background grid.
    show_grid: bool,
    /// Whether control points snap to the grid.
    snap_to_grid: bool,
    /// Which handle is being dragged (None = not dragging).
    dragging: Option<DragTarget>,
    /// Emitted when the curve changes. Passes (control_point1, control_point2).
    pub curve_changed: Signal1<((f32, f32), (f32, f32))>,
}

/// Identifies which control point is being dragged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DragTarget {
    ControlPoint1,
    ControlPoint2,
}

impl BezierCurveEditor {
    /// Creates a new BezierCurveEditor with the given geometry.
    /// Default control points form a standard ease-in-out curve.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::BezierCurveEditor, geometry, "BezierCurveEditor"),
            control_point1: (0.25, 0.1),
            control_point2: (0.75, 0.9),
            grid_size: 0.2,
            show_grid: true,
            snap_to_grid: false,
            dragging: None,
            curve_changed: Signal1::new(),
        }
    }

    /// Returns the first control point as a fraction (0.0–1.0).
    pub fn control_point1(&self) -> (f32, f32) {
        self.control_point1
    }

    /// Sets the first control point with values clamped to 0.0–1.0.
    pub fn set_control_point1(&mut self, x: f32, y: f32) {
        let old = self.control_point1;
        self.control_point1 = (x.clamp(0.0, 1.0), y.clamp(0.0, 1.0));
        if self.control_point1 != old {
            self.emit_curve_changed();
            self.base.request_redraw();
        }
    }

    /// Returns the second control point as a fraction (0.0–1.0).
    pub fn control_point2(&self) -> (f32, f32) {
        self.control_point2
    }

    /// Sets the second control point with values clamped to 0.0–1.0.
    pub fn set_control_point2(&mut self, x: f32, y: f32) {
        let old = self.control_point2;
        self.control_point2 = (x.clamp(0.0, 1.0), y.clamp(0.0, 1.0));
        if self.control_point2 != old {
            self.emit_curve_changed();
            self.base.request_redraw();
        }
    }

    /// Sets whether the background grid is drawn.
    pub fn set_show_grid(&mut self, show: bool) {
        if self.show_grid != show {
            self.show_grid = show;
            self.base.request_redraw();
        }
    }

    /// Returns whether the background grid is shown.
    pub fn show_grid(&self) -> bool {
        self.show_grid
    }

    /// Sets whether control points snap to the grid.
    pub fn set_snap_to_grid(&mut self, snap: bool) {
        self.snap_to_grid = snap;
    }

    /// Returns whether control points snap to the grid.
    pub fn snap_to_grid(&self) -> bool {
        self.snap_to_grid
    }

    /// Sets the grid size (spacing between grid lines as a fraction of widget size).
    pub fn set_grid_size(&mut self, size: f32) {
        self.grid_size = size.clamp(0.05, 0.5);
        self.base.request_redraw();
    }

    /// Returns the grid size.
    pub fn grid_size(&self) -> f32 {
        self.grid_size
    }

    /// Evaluates the cubic bezier curve at parameter t (0.0–1.0).
    /// Returns the (x, y) value on the curve at that parameter.
    pub fn sample_at(&self, t: f32) -> (f32, f32) {
        let p0 = (0.0f32, 0.0f32);
        let p3 = (1.0f32, 1.0f32);
        Self::cubic_bezier(t, p0, self.control_point1, self.control_point2, p3)
    }

    /// Evaluates a cubic bezier curve at parameter t (0.0–1.0) with control points
    /// given as `(p0, p1, p2, p3)` tuples.
    pub fn cubic_bezier(
        t: f32,
        p0: (f32, f32),
        p1: (f32, f32),
        p2: (f32, f32),
        p3: (f32, f32),
    ) -> (f32, f32) {
        let (p0x, p0y) = p0;
        let (p1x, p1y) = p1;
        let (p2x, p2y) = p2;
        let (p3x, p3y) = p3;
        let u = 1.0 - t;
        let tt = t * t;
        let uu = u * u;
        let uuu = uu * u;
        let ttt = tt * t;
        let x = uuu * p0x + 3.0 * uu * t * p1x + 3.0 * u * tt * p2x + ttt * p3x;
        let y = uuu * p0y + 3.0 * uu * t * p1y + 3.0 * u * tt * p2y + ttt * p3y;
        (x, y)
    }

    /// Emits the curve_changed signal with current control points.
    fn emit_curve_changed(&self) {
        self.curve_changed.emit((self.control_point1, self.control_point2));
    }

    /// Converts a curve coordinate (0.0–1.0) to a pixel position within the widget.
    fn curve_to_pixel(&self, cx: f32, cy: f32) -> Point {
        let rect = self.geometry();
        let margin = HANDLE_RADIUS as i32 + 4;
        let draw_w = (rect.width as i32 - 2 * margin).max(1) as f32;
        let draw_h = (rect.height as i32 - 2 * margin).max(1) as f32;
        Point::new(
            (rect.x + margin) + (cx * draw_w) as i32,
            (rect.y + margin) + ((1.0 - cy) * draw_h) as i32,
        )
    }

    /// Converts a pixel position to a curve coordinate (0.0–1.0).
    fn pixel_to_curve(&self, px: i32, py: i32) -> (f32, f32) {
        let rect = self.geometry();
        let margin = HANDLE_RADIUS as i32 + 4;
        let draw_w = (rect.width as i32 - 2 * margin).max(1) as f32;
        let draw_h = (rect.height as i32 - 2 * margin).max(1) as f32;
        let cx = ((px - rect.x - margin) as f32 / draw_w).clamp(0.0, 1.0);
        let cy = 1.0 - ((py - rect.y - margin) as f32 / draw_h).clamp(0.0, 1.0);
        if self.snap_to_grid && self.grid_size > 0.0 {
            let snapped_x = (cx / self.grid_size).round() * self.grid_size;
            let snapped_y = (cy / self.grid_size).round() * self.grid_size;
            (snapped_x.clamp(0.0, 1.0), snapped_y.clamp(0.0, 1.0))
        } else {
            (cx, cy)
        }
    }

    /// Returns how close a pixel position is to a control point handle, in pixels.
    fn distance_to_handle(&self, pos: Point, cp: (f32, f32)) -> f64 {
        let hp = self.curve_to_pixel(cp.0, cp.1);
        let dx = (pos.x - hp.x) as f64;
        let dy = (pos.y - hp.y) as f64;
        (dx * dx + dy * dy).sqrt()
    }

    /// Returns which control point handle is at the given position, if any.
    fn hit_test_handle(&self, pos: Point) -> Option<DragTarget> {
        let threshold = (HANDLE_RADIUS + 4) as f64;
        let d1 = self.distance_to_handle(pos, self.control_point1);
        let d2 = self.distance_to_handle(pos, self.control_point2);
        if d1 < threshold && d1 <= d2 {
            Some(DragTarget::ControlPoint1)
        } else if d2 < threshold {
            Some(DragTarget::ControlPoint2)
        } else {
            None
        }
    }

    /// Draws a single grid line with a subtle color.
    fn draw_grid_line(&self, context: &mut RenderContext, from: Point, to: Point, color: Color) {
        context.draw_line(from, to, color);
    }
}

impl Widget for BezierCurveEditor {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(300, 200)
    }
}

impl Draw for BezierCurveEditor {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let is_enabled = self.base.is_enabled();
        let margin = HANDLE_RADIUS as i32 + 4;
        let draw_w = (rect.width as i32 - 2 * margin).max(1) as f32;
        let draw_h = (rect.height as i32 - 2 * margin).max(1) as f32;

        // ── Background ──
        let bg = if !is_enabled {
            Color::rgba(245, 245, 245, 100)
        } else {
            Color::rgba(245, 245, 245, 255)
        };
        context.fill_rect(rect, bg);

        // ── Grid ──
        if self.show_grid && is_enabled {
            let grid_color = Color::rgba(180, 180, 180, GRID_ALPHA);
            let steps = (1.0 / self.grid_size) as u32;
            for i in 0..=steps {
                let frac = i as f32 * self.grid_size;
                let x = (rect.x + margin) + (frac * draw_w) as i32;
                let y = (rect.y + margin) + ((1.0 - frac) * draw_h) as i32;
                // Vertical grid line.
                self.draw_grid_line(
                    context,
                    Point::new(x, rect.y + margin),
                    Point::new(x, rect.y + margin + draw_h as i32),
                    grid_color,
                );
                // Horizontal grid line.
                self.draw_grid_line(
                    context,
                    Point::new(rect.x + margin, y),
                    Point::new(rect.x + margin + draw_w as i32, y),
                    grid_color,
                );
            }

            // ── Axis lines ──
            let axis_color = Color::rgba(100, 100, 100, 150);
            // X-axis (bottom edge).
            context.draw_line(
                Point::new(rect.x + margin, rect.y + margin + draw_h as i32),
                Point::new(rect.x + margin + draw_w as i32, rect.y + margin + draw_h as i32),
                axis_color,
            );
            // Y-axis (left edge).
            context.draw_line(
                Point::new(rect.x + margin, rect.y + margin),
                Point::new(rect.x + margin, rect.y + margin + draw_h as i32),
                axis_color,
            );
        }

        if !is_enabled {
            return;
        }

        // ── Control polygon (lines from endpoints to control points) ──
        let p0 = self.curve_to_pixel(0.0, 0.0);
        let p3 = self.curve_to_pixel(1.0, 1.0);
        let cp1_pixel = self.curve_to_pixel(self.control_point1.0, self.control_point1.1);
        let cp2_pixel = self.curve_to_pixel(self.control_point2.0, self.control_point2.1);

        let poly_color = Color::rgba(150, 150, 150, 180);
        context.draw_line(p0, cp1_pixel, poly_color);
        context.draw_line(p3, cp2_pixel, poly_color);

        // ── Bezier curve ──
        let curve_color = Color::rgb(33, 118, 210); // Material blue
        let segments = 50;
        let mut prev = self.curve_to_pixel(0.0, 0.0);
        for i in 1..=segments {
            let t = i as f32 / segments as f32;
            let (cx, cy) = self.sample_at(t);
            let cur = self.curve_to_pixel(cx, cy);
            context.draw_line_stroke(prev, cur, curve_color, 2);
            prev = cur;
        }

        // ── Control point handles ──
        // CP1 handle.
        context.fill_circle(cp1_pixel, HANDLE_RADIUS, Color::rgba(33, 118, 210, 200));
        context.draw_circle_stroke(cp1_pixel, HANDLE_RADIUS, Color::rgb(255, 255, 255), 2);

        // CP2 handle.
        context.fill_circle(cp2_pixel, HANDLE_RADIUS, Color::rgba(76, 175, 80, 200));
        context.draw_circle_stroke(cp2_pixel, HANDLE_RADIUS, Color::rgb(255, 255, 255), 2);

        // ── Labels ──
        let font = crate::core::Font::default();
        let cp1_label =
            format!("CP1: ({:.2}, {:.2})", self.control_point1.0, self.control_point1.1);
        let cp2_label =
            format!("CP2: ({:.2}, {:.2})", self.control_point2.0, self.control_point2.1);

        context.draw_text(
            Point::new(rect.x + 4, rect.y + 12),
            &cp1_label,
            &font,
            Color::rgba(33, 118, 210, 200),
            HorizontalAlignment::Left,
        );
        context.draw_text(
            Point::new(rect.x + 4, rect.y + 26),
            &cp2_label,
            &font,
            Color::rgba(76, 175, 80, 200),
            HorizontalAlignment::Left,
        );

        // ── Endpoint markers ──
        context.fill_circle(p0, 4, Color::rgba(0, 0, 0, 150));
        context.fill_circle(p3, 4, Color::rgba(0, 0, 0, 150));
    }
}

impl EventHandler for BezierCurveEditor {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button } => {
                if *button == 1 {
                    if let Some(target) = self.hit_test_handle(*pos) {
                        self.dragging = Some(target);
                        self.base.set_mouse_pressed(true);
                    }
                }
            }
            Event::MouseRelease { pos: _, button } => {
                if *button == 1 && self.dragging.is_some() {
                    self.dragging = None;
                    self.base.set_mouse_pressed(false);
                }
            }
            Event::MouseMove { pos } => {
                if let Some(target) = self.dragging {
                    let (cx, cy) = self.pixel_to_curve(pos.x, pos.y);
                    match target {
                        DragTarget::ControlPoint1 => {
                            self.set_control_point1(cx, cy);
                        }
                        DragTarget::ControlPoint2 => {
                            self.set_control_point2(cx, cy);
                        }
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
    use crate::core::Point;
    use std::sync::{Arc, Mutex};

    fn default_editor() -> BezierCurveEditor {
        BezierCurveEditor::new(Rect::new(0, 0, 300, 300))
    }

    #[test]
    fn bezier_creation_defaults() {
        let editor = default_editor();
        assert_eq!(editor.control_point1(), (0.25, 0.1));
        assert_eq!(editor.control_point2(), (0.75, 0.9));
        assert!(editor.show_grid());
        assert!(!editor.snap_to_grid());
        assert!((editor.grid_size() - 0.2).abs() < 0.001);
        assert_eq!(editor.kind(), WidgetKind::BezierCurveEditor);
    }

    #[test]
    fn bezier_set_control_point1() {
        let mut editor = default_editor();
        editor.set_control_point1(0.5, 0.3);
        assert_eq!(editor.control_point1(), (0.5, 0.3));
    }

    #[test]
    fn bezier_set_control_point2() {
        let mut editor = default_editor();
        editor.set_control_point2(0.6, 0.7);
        assert_eq!(editor.control_point2(), (0.6, 0.7));
    }

    #[test]
    fn bezier_control_points_clamped() {
        let mut editor = default_editor();
        editor.set_control_point1(1.5, -0.5);
        assert_eq!(editor.control_point1(), (1.0, 0.0));

        editor.set_control_point2(2.0, 2.0);
        assert_eq!(editor.control_point2(), (1.0, 1.0));
    }

    #[test]
    fn bezier_cubic_bezier_static() {
        // Linear curve: endpoints (0,0) and (1,1), control points on the line.
        let (x, y) =
            BezierCurveEditor::cubic_bezier(0.5, (0.0, 0.0), (0.5, 0.5), (0.5, 0.5), (1.0, 1.0));
        assert!((x - 0.5).abs() < 0.01);
        assert!((y - 0.5).abs() < 0.01);
    }

    #[test]
    fn bezier_sample_at() {
        let editor = default_editor();
        // At t=0, we should be at (0,0).
        let (x0, y0) = editor.sample_at(0.0);
        assert!((x0 - 0.0).abs() < 0.01);
        assert!((y0 - 0.0).abs() < 0.01);

        // At t=1, we should be at (1,1).
        let (x1, y1) = editor.sample_at(1.0);
        assert!((x1 - 1.0).abs() < 0.01);
        assert!((y1 - 1.0).abs() < 0.01);
    }

    #[test]
    fn bezier_curve_changed_signal() {
        let mut editor = default_editor();
        let captured = Arc::new(Mutex::new(None));
        editor.curve_changed.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<((f32, f32), (f32, f32))>| {
                *captured.lock().unwrap() = Some(*val);
            }
        });

        editor.set_control_point1(0.5, 0.2);
        let signal_val = *captured.lock().unwrap();
        assert!(signal_val.is_some());
        let (cp1, _cp2) = signal_val.unwrap();
        assert_eq!(cp1, (0.5, 0.2));
    }

    #[test]
    fn bezier_set_show_grid() {
        let mut editor = default_editor();
        assert!(editor.show_grid());
        editor.set_show_grid(false);
        assert!(!editor.show_grid());
        editor.set_show_grid(true);
        assert!(editor.show_grid());
    }

    #[test]
    fn bezier_set_snap_to_grid() {
        let mut editor = default_editor();
        assert!(!editor.snap_to_grid());
        editor.set_snap_to_grid(true);
        assert!(editor.snap_to_grid());
    }

    #[test]
    fn bezier_set_grid_size_clamped() {
        let mut editor = default_editor();
        editor.set_grid_size(0.1);
        assert!((editor.grid_size() - 0.1).abs() < 0.001);

        editor.set_grid_size(0.01); // below minimum, clamped
        assert!((editor.grid_size() - 0.05).abs() < 0.001);

        editor.set_grid_size(1.0); // above maximum, clamped
        assert!((editor.grid_size() - 0.5).abs() < 0.001);
    }

    #[test]
    fn bezier_pixel_to_curve_roundtrip() {
        let editor = BezierCurveEditor::new(Rect::new(10, 10, 200, 200));
        // Test roundtrip: curve -> pixel -> curve.
        let test_points = [(0.0, 0.0), (1.0, 1.0), (0.5, 0.5), (0.25, 0.75)];
        for &(cx, cy) in &test_points {
            let pixel = editor.curve_to_pixel(cx, cy);
            let (rx, ry) = editor.pixel_to_curve(pixel.x, pixel.y);
            assert!((cx - rx).abs() < 0.02, "X roundtrip failed for ({}, {})", cx, cy);
            assert!((cy - ry).abs() < 0.02, "Y roundtrip failed for ({}, {})", cx, cy);
        }
    }

    #[test]
    fn bezier_hit_test_handle() {
        let mut editor = BezierCurveEditor::new(Rect::new(0, 0, 300, 300));
        editor.set_control_point1(0.25, 0.1);
        editor.set_control_point2(0.75, 0.9);

        // The handle positions depend on margin (12) and draw area.
        // CP1: x=12+0.25*276=81, y=12+(1-0.1)*276=12+248.4=260
        // Let's just check the function returns Some for positions near the curve.
        let cp1_pixel = editor.curve_to_pixel(0.25, 0.1);
        let hit = editor.hit_test_handle(cp1_pixel);
        assert_eq!(hit, Some(DragTarget::ControlPoint1));

        let cp2_pixel = editor.curve_to_pixel(0.75, 0.9);
        let hit = editor.hit_test_handle(cp2_pixel);
        assert_eq!(hit, Some(DragTarget::ControlPoint2));

        // Far away from any handle.
        let hit = editor.hit_test_handle(Point::new(5, 5));
        assert_eq!(hit, None);
    }

    #[test]
    fn bezier_drag_control_point1() {
        let mut editor = BezierCurveEditor::new(Rect::new(0, 0, 300, 300));
        let cp1_pixel = editor.curve_to_pixel(0.25, 0.1);

        // Mouse press on CP1.
        editor.handle_event(&Event::MousePress { pos: cp1_pixel, button: 1 });
        assert!(editor.base.is_mouse_pressed());

        // Drag to a new position.
        let new_pixel = editor.curve_to_pixel(0.5, 0.5);
        editor.handle_event(&Event::MouseMove { pos: new_pixel });

        let (cx, cy) = editor.control_point1();
        assert!((cx - 0.5).abs() < 0.02);
        assert!((cy - 0.5).abs() < 0.02);

        // Release.
        editor.handle_event(&Event::MouseRelease { pos: new_pixel, button: 1 });
        assert!(!editor.base.is_mouse_pressed());
    }

    #[test]
    fn bezier_disabled_blocks_events() {
        let mut editor = default_editor();
        editor.set_enabled(false);
        editor.handle_event(&Event::MousePress { pos: Point::new(100, 100), button: 1 });
        assert!(!editor.base.is_mouse_pressed());
    }

    #[test]
    fn bezier_cubic_bezier_endpoints() {
        // At t=0, the result must equal p0.
        let (x, y) =
            BezierCurveEditor::cubic_bezier(0.0, (0.1, 0.2), (0.3, 0.4), (0.5, 0.6), (0.7, 0.8));
        assert!((x - 0.1).abs() < 0.001);
        assert!((y - 0.2).abs() < 0.001);

        // At t=1, the result must equal p3.
        let (x, y) =
            BezierCurveEditor::cubic_bezier(1.0, (0.1, 0.2), (0.3, 0.4), (0.5, 0.6), (0.7, 0.8));
        assert!((x - 0.7).abs() < 0.001);
        assert!((y - 0.8).abs() < 0.001);
    }

    #[test]
    fn bezier_svg_output() {
        let mut editor = BezierCurveEditor::new(Rect::new(0, 0, 300, 300));
        let svg = crate::widget::svg::render_to_svg(&mut editor);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }
}
