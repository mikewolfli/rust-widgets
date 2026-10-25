// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! [`FreeformShapeWidget`] — the control that draws a [`ShapePath`](super::ShapePath).

use super::types::{BubbleTailDirection, PathSegment, ShapePath};
use crate::core::{Color, Point, Rect};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::undo::{UndoStack, ValueCommand};
use crate::widget::capability::coercion::{expect_string, expect_u32};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use alloc::boxed::Box;
use alloc::rc::Rc;
use core::cell::RefCell;

/// One of the eight resize handles an editable shape draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapeHandle {
    /// Top-left corner.
    TopLeft,
    /// Top edge midpoint.
    Top,
    /// Top-right corner.
    TopRight,
    /// Right edge midpoint.
    Right,
    /// Bottom-right corner.
    BottomRight,
    /// Bottom edge midpoint.
    Bottom,
    /// Bottom-left corner.
    BottomLeft,
    /// Left edge midpoint.
    Left,
    /// A polygon vertex, identified by its index in [`ShapePath::Polygon`].
    ///
    /// A polygon is edited by moving its own anchors rather than by resizing a
    /// bounding box, so its handles are vertices and not the eight box handles.
    Vertex(usize),
}

/// A non-rectangular, clickable shape.
///
/// Unlike an ordinary rectangular widget — which takes every pointer event
/// inside its geometry — this widget hit-tests the pointer against the actual
/// outline, so a press in the transparent corner *outside* a star falls through
/// to whatever is behind it. [`FreeformShapeWidget::contains`] is that test.
///
/// The outline is described by [`ShapePath`] and is drawn with a solid fill plus
/// an optional stroke; there is no image or texture. The shape scales with the
/// widget's geometry except for [`ShapePath::Custom`].
pub struct FreeformShapeWidget {
    base: BaseWidget,
    path: ShapePath,
    fill_color: Color,
    stroke_color: Option<Color>,
    /// Set once the caller assigns a fill through [`Self::set_fill_color`]. Until
    /// then the fill is the theme's, so an untouched shape follows an appearance
    /// switch; after that the colour is the caller's data and is used verbatim.
    fill_overridden: bool,
    /// The [`Self::fill_overridden`] counterpart for the outline.
    stroke_overridden: bool,
    stroke_width: u32,
    /// Whether the shape's own **outline** is under the pointer.
    ///
    /// Named `hovered_item` rather than `hovered` on purpose: this is not the control's
    /// widget-level hover (which [`BaseWidget`] owns) but a hit test against the *drawn
    /// path*, which is what a freeform shape treats as its content. A caller comparing
    /// the two names can see at a glance that they answer different questions.
    hovered_item: bool,
    /// Whether a primary press landed inside the outline and has not been released.
    ///
    /// The outline-level analogue of [`Self::hovered_item`]: a shape is only pressed
    /// when the press hit its path, which a plain widget-level `pressed` cannot express.
    pressed_item: bool,
    /// Whether the eight resize handles are drawn and draggable.
    editable: bool,
    /// Grid step in pixels for handle movement; zero disables snapping.
    snap_step: u32,
    /// The handle being dragged, with the pointer and geometry at the press.
    drag_handle: Option<(ShapeHandle, Point, Rect)>,
    /// The shape's stacking hint for a container that honours it.
    z_index: i32,
    /// Whether a press inside the outline drags the whole shape.
    movable: bool,
    /// An in-progress body drag: the pointer at the press and the geometry then.
    body_drag: Option<(Point, Rect)>,
    /// The vertex position at the press, so dragging is not cumulative.
    drag_vertex_start: Option<Point>,
    /// Geometry history; a whole drag is one undo step.
    history: UndoStack,
    /// The value the geometry commands write; mirrors the widget's geometry.
    history_geometry: Rc<RefCell<Rect>>,
    /// Emitted with the new geometry after a handle drag.
    pub geometry_changed: Signal1<Rect>,
    /// Emitted with the new top-left position while the shape body is dragged.
    pub moved: Signal1<Point>,
    /// Emitted when a polygon vertex is added, removed or moved.
    pub vertex_changed: Signal1<(usize, Point)>,
    /// Emitted with `(undo steps, redo steps)` after every history change.
    pub history_changed: Signal1<(usize, usize)>,
    /// Emitted when the shape is clicked, i.e. pressed and released while the
    /// pointer is still inside the outline.
    pub clicked: GenericSignal,
    /// Emitted when the pointer enters or leaves the outline, with the new
    /// state. Fires from a pointer move that crosses the outline, not on every
    /// move.
    pub hovered_changed: Signal1<bool>,
    /// Emitted when the primary button is pressed or released inside the
    /// outline, with the new state. A press that begins outside the shape and
    /// drags in never emits it.
    pub pressed_changed: Signal1<bool>,
}

impl FreeformShapeWidget {
    /// Creates a shape in `geometry` with the given outline.
    ///
    /// The fill and outline start as the theme's colours, so the shape follows an
    /// appearance switch until a caller sets one of its own; the stroke is two
    /// pixels wide. It starts neither hovered nor pressed.
    pub fn new(geometry: Rect, path: ShapePath) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::FreeformShape, geometry, "FreeformShapeWidget"),
            path,
            fill_color: Color::rgb(200, 220, 255),
            stroke_color: Some(Color::rgb(80, 120, 200)),
            fill_overridden: false,
            stroke_overridden: false,
            stroke_width: 2,
            hovered_item: false,
            pressed_item: false,
            editable: false,
            snap_step: 0,
            drag_handle: None,
            z_index: 0,
            movable: true,
            body_drag: None,
            drag_vertex_start: None,
            history: UndoStack::new(),
            history_geometry: Rc::new(RefCell::new(geometry)),
            geometry_changed: Signal1::new(),
            moved: Signal1::new(),
            vertex_changed: Signal1::new(),
            history_changed: Signal1::new(),
            clicked: GenericSignal::new(),
            hovered_changed: Signal1::new(),
            pressed_changed: Signal1::new(),
        }
    }

    /// The outline this shape is drawn and hit-tested as.
    pub fn path(&self) -> &ShapePath {
        &self.path
    }
    /// Replaces the outline and repaints. The previous shape's hover and press
    /// state is kept as-is, so a shape swapped out from under the pointer may be
    /// hovered while the pointer is somewhere else.
    pub fn set_path(&mut self, path: ShapePath) {
        self.path = path;
        self.base.request_redraw();
    }
    /// The colour the shape's interior is painted in.
    ///
    /// This is the colour a caller configured, and the two defaults this control
    /// ships with until one is set. While neither [`Self::set_fill_color`] nor a
    /// theme has moved it, the *rendered* fill is the theme's resolved foreground
    /// — the theme's colour is only knowable at draw time, so it is not baked in
    /// here — and this reports the ship-time default instead.
    pub fn fill_color(&self) -> Color {
        self.fill_color
    }
    /// Sets the fill colour and repaints. The alpha channel is honoured, so a
    /// translucent colour makes the shape see-through while still clickable. Once
    /// set, the colour is used verbatim and no longer follows the theme.
    pub fn set_fill_color(&mut self, color: Color) {
        self.fill_color = color;
        self.fill_overridden = true;
        self.base.request_redraw();
    }
    /// The outline colour, or `None` when the shape is drawn with no outline.
    pub fn stroke_color(&self) -> Option<Color> {
        self.stroke_color
    }
    /// Sets the outline colour, or removes the outline when given `None`, and
    /// repaints. Once set, the colour is used verbatim and no longer follows the
    /// theme.
    pub fn set_stroke_color(&mut self, color: Option<Color>) {
        self.stroke_color = color;
        self.stroke_overridden = true;
        self.base.request_redraw();
    }
    /// The outline width in **pixels**. Meaningless while
    /// [`Self::stroke_color`] is `None`.
    pub fn stroke_width(&self) -> u32 {
        self.stroke_width
    }
    /// Sets the outline width in **pixels** and repaints.
    ///
    /// The value is stored as given and is **not** clamped: `0` draws no visible
    /// outline, and a very large width draws a band much wider than the shape's
    /// edges rather than being scaled down to fit.
    pub fn set_stroke_width(&mut self, width: u32) {
        if self.stroke_width == width {
            return;
        }
        self.stroke_width = width;
        self.base.request_redraw();
    }

    /// Shows the eight resize handles, making the shape editable with the pointer.
    pub fn set_editable(&mut self, editable: bool) {
        if self.editable == editable {
            return;
        }
        self.editable = editable;
        self.drag_handle = None;
        self.base.request_redraw();
    }

    /// Returns whether the resize handles are shown.
    pub fn is_editable(&self) -> bool {
        self.editable
    }

    /// Sets the grid step handles snap to; zero disables snapping.
    pub fn set_snap_step(&mut self, step: u32) {
        if self.snap_step == step {
            return;
        }
        self.snap_step = step;
        self.base.request_redraw();
    }

    /// Returns the handle snap step in pixels.
    pub fn snap_step(&self) -> u32 {
        self.snap_step
    }

    /// The shape's stacking hint. The widget cannot reorder its siblings; a container that
    /// supports z-order reads this rather than guessing from the child list.
    pub fn z_index(&self) -> i32 {
        self.z_index
    }

    /// Sets the stacking hint.
    pub fn set_z_index(&mut self, z_index: i32) {
        if self.z_index == z_index {
            return;
        }
        self.z_index = z_index;
    }

    /// Returns whether a press inside the outline drags the whole shape.
    pub fn is_movable(&self) -> bool {
        self.movable
    }

    /// Enables or disables dragging the shape body.
    pub fn set_movable(&mut self, movable: bool) {
        if self.movable == movable {
            return;
        }
        self.movable = movable;
        if !movable {
            self.body_drag = None;
        }
    }

    /// Returns whether there is a geometry change to undo.
    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    /// Returns whether there is an undone geometry change to redo.
    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    /// Reverts the last committed geometry change.
    pub fn undo(&mut self) -> bool {
        if self.history.undo().is_err() {
            return false;
        }
        let geometry = *self.history_geometry.borrow();
        self.drag_handle = None;
        self.drag_vertex_start = None;
        self.body_drag = None;
        self.base.set_geometry(geometry);
        self.geometry_changed.emit(geometry);
        self.emit_history();
        self.base.request_redraw();
        true
    }

    /// Re-applies the geometry change most recently removed by [`Self::undo`].
    pub fn redo(&mut self) -> bool {
        if self.history.redo().is_err() {
            return false;
        }
        let geometry = *self.history_geometry.borrow();
        self.drag_handle = None;
        self.drag_vertex_start = None;
        self.body_drag = None;
        self.base.set_geometry(geometry);
        self.geometry_changed.emit(geometry);
        self.emit_history();
        self.base.request_redraw();
        true
    }

    /// Records an already-applied geometry change as one undo step.
    fn record_geometry(&mut self, before: Rect, after: Rect) {
        if before == after {
            return;
        }
        *self.history_geometry.borrow_mut() = after;
        self.history.push(Box::new(ValueCommand::new(
            Rc::clone(&self.history_geometry),
            before,
            after,
            "Move or resize shape",
            "geometry",
        )));
        self.emit_history();
    }

    /// Publishes the current history depth.
    fn emit_history(&self) {
        self.history_changed.emit((self.history.undo_count(), self.history.redo_count()));
    }

    /// Inserts a polygon vertex at `index`, clamped to the vertex count.
    ///
    /// Returns `false` when the path is not a polygon.
    pub fn add_vertex(&mut self, index: usize, point: Point) -> bool {
        let point = self.snapped(point);
        let ShapePath::Polygon(vertices) = &mut self.path else { return false };
        let index = index.min(vertices.len());
        vertices.insert(index, point);
        self.vertex_changed.emit((index, point));
        self.base.request_redraw();
        true
    }

    /// Removes the polygon vertex at `index`, returning whether it was removed.
    ///
    /// A polygon is kept at three vertices so it remains a closed area.
    pub fn remove_vertex(&mut self, index: usize) -> bool {
        let ShapePath::Polygon(vertices) = &mut self.path else { return false };
        if vertices.len() <= 3 || index >= vertices.len() {
            return false;
        }
        let removed = vertices.remove(index);
        self.vertex_changed.emit((index, removed));
        self.base.request_redraw();
        true
    }

    /// Replaces one vertex of a polygon path.
    ///
    /// Returns `false` when the path is not a polygon or the index is out of range. Anchor
    /// editing is what makes a freeform outline something a user can adjust rather than a
    /// fixed decoration.
    pub fn set_polygon_vertex(&mut self, index: usize, point: Point) -> bool {
        let point = self.snapped(point);
        match &mut self.path {
            ShapePath::Polygon(vertices) => {
                if index >= vertices.len() {
                    return false;
                }
                vertices[index] = point;
                self.vertex_changed.emit((index, point));
                self.base.request_redraw();
                true
            }
            _ => false,
        }
    }

    /// Rounds `point` to the snap grid when one is set.
    fn snapped(&self, point: Point) -> Point {
        if self.snap_step == 0 {
            return point;
        }
        let step = self.snap_step as i32;
        Point::new(
            (point.x as f32 / step as f32).round() as i32 * step,
            (point.y as f32 / step as f32).round() as i32 * step,
        )
    }

    /// The editable handles' rectangles, in window coordinates.
    ///
    /// A polygon exposes one handle per vertex, because its own anchors are what a
    /// user adjusts; every other path shape exposes the eight box handles.
    pub fn handle_rects(&self) -> Vec<(ShapeHandle, Rect)> {
        let rect = self.base.geometry();
        const SIDE: u32 = 8;
        let half = SIDE as i32 / 2;
        if let ShapePath::Polygon(vertices) = &self.path {
            return vertices
                .iter()
                .enumerate()
                .map(|(index, vertex)| {
                    (
                        ShapeHandle::Vertex(index),
                        Rect::new(rect.x + vertex.x - half, rect.y + vertex.y - half, SIDE, SIDE),
                    )
                })
                .collect();
        }
        let positions = [
            (ShapeHandle::TopLeft, rect.x, rect.y),
            (ShapeHandle::Top, rect.x + rect.width as i32 / 2, rect.y),
            (ShapeHandle::TopRight, rect.x + rect.width as i32, rect.y),
            (ShapeHandle::Right, rect.x + rect.width as i32, rect.y + rect.height as i32 / 2),
            (ShapeHandle::BottomRight, rect.x + rect.width as i32, rect.y + rect.height as i32),
            (ShapeHandle::Bottom, rect.x + rect.width as i32 / 2, rect.y + rect.height as i32),
            (ShapeHandle::BottomLeft, rect.x, rect.y + rect.height as i32),
            (ShapeHandle::Left, rect.x, rect.y + rect.height as i32 / 2),
        ];
        positions
            .into_iter()
            .map(|(handle, x, y)| (handle, Rect::new(x - half, y - half, SIDE, SIDE)))
            .collect()
    }

    /// The handle under `pos`, if the shape is editable.
    pub fn handle_at(&self, pos: Point) -> Option<ShapeHandle> {
        if !self.editable {
            return None;
        }
        self.handle_rects()
            .into_iter()
            .find(|(_, rect)| rect.contains_point(pos))
            .map(|(handle, _)| handle)
    }

    /// Applies a handle drag to the geometry at `pos`.
    fn drag_to(&mut self, pos: Point) {
        let Some((handle, origin, start)) = self.drag_handle else {
            return;
        };
        if let ShapeHandle::Vertex(index) = handle {
            let Some(vertex_start) = self.drag_vertex_start else { return };
            let target = self.snapped(Point::new(
                vertex_start.x + (pos.x - origin.x),
                vertex_start.y + (pos.y - origin.y),
            ));
            self.set_polygon_vertex(index, target);
            return;
        }
        let pos = self.snapped(pos);
        let dx = pos.x - origin.x;
        let dy = pos.y - origin.y;
        let mut left = start.x;
        let mut top = start.y;
        let mut right = start.x + start.width as i32;
        let mut bottom = start.y + start.height as i32;
        match handle {
            ShapeHandle::TopLeft => {
                left += dx;
                top += dy;
            }
            ShapeHandle::Top => top += dy,
            ShapeHandle::TopRight => {
                right += dx;
                top += dy;
            }
            ShapeHandle::Right => right += dx,
            ShapeHandle::BottomRight => {
                right += dx;
                bottom += dy;
            }
            ShapeHandle::Bottom => bottom += dy,
            ShapeHandle::BottomLeft => {
                left += dx;
                bottom += dy;
            }
            ShapeHandle::Left => left += dx,
            // Handled above: a vertex drag never reaches the rectangle math.
            ShapeHandle::Vertex(_) => return,
        }
        // A handle drag cannot invert the rectangle: the edges stay ordered, so a caller
        // always receives a valid `Rect`.
        let width = (right - left).max(1) as u32;
        let height = (bottom - top).max(1) as u32;
        self.base.set_geometry(Rect::new(left.min(right - 1), top.min(bottom - 1), width, height));
        self.geometry_changed.emit(self.base.geometry());
        self.base.request_redraw();
    }

    /// Whether `point`, in screen coordinates, falls inside the drawn outline.
    ///
    /// This is the test the widget uses to decide whether a press reaches it, and
    /// it is the reason the shape behaves as a non-rectangular target: a point
    /// inside `geometry()` but outside the outline reports `false`.
    ///
    /// The outline is evaluated in fractional detail — Bézier segments are
    /// flattened and curved variants are approximated — so a point exactly on or
    /// very near the boundary may be reported either way. A widget with a zero
    /// width or height contains nothing.
    pub fn contains(&self, point: Point) -> bool {
        let rect = self.base.geometry();
        if !rect.contains_point(point) {
            return false;
        }
        let local = Point::new(point.x - rect.x, point.y - rect.y);
        let w = rect.width as i32;
        let h = rect.height as i32;
        if w <= 0 || h <= 0 {
            return false;
        }
        match &self.path {
            ShapePath::Heart => self.contains_heart(local, w, h),
            ShapePath::Star { points, inner_radius } => {
                self.contains_star(local, w, h, *points, *inner_radius)
            }
            ShapePath::Polygon(v) => self.contains_polygon(local, v),
            ShapePath::RoundedRect { radius } => self.contains_rounded_rect(local, w, h, *radius),
            ShapePath::Bubble { tail_direction } => {
                self.contains_bubble(local, w, h, *tail_direction)
            }
            ShapePath::Custom(s) => self.contains_custom(local, s),
            ShapePath::UnitCustom(s) => {
                let pixels: Vec<PathSegment> =
                    s.iter().map(|segment| segment.to_path_segment(w as f32, h as f32)).collect();
                self.contains_custom(local, &pixels)
            }
        }
    }

    fn contains_heart(&self, local: Point, w: i32, h: i32) -> bool {
        let cx = w as f32 / 2.0;
        let cy = h as f32 / 2.0;
        let scale = (h as f32).max(1.0) / 2.0;
        let x = (local.x as f32 - cx) / scale;
        let y = (local.y as f32 - cy) / scale;
        (x * x + y * y - 1.0).powi(3) - x * x * (y * y * y) <= 0.0
    }

    fn contains_star(
        &self,
        local: Point,
        w: i32,
        h: i32,
        num_points: u8,
        inner_ratio: f32,
    ) -> bool {
        let n = num_points.max(3) as usize;
        let cx = w as f32 / 2.0;
        let cy = h as f32 / 2.0;
        let outer_r = (cx.min(cy)).max(1.0);
        let inner_r = outer_r * inner_ratio.clamp(0.05, 0.95);
        let mut vertices = Vec::with_capacity(n * 2);
        let step = std::f32::consts::PI / n as f32;
        for i in 0..n {
            let angle =
                i as f32 * 2.0 * std::f32::consts::PI / n as f32 - std::f32::consts::FRAC_PI_2;
            vertices.push(Point::new(
                (cx + outer_r * angle.cos()).round() as i32,
                (cy + outer_r * angle.sin()).round() as i32,
            ));
            let ia = angle + step;
            vertices.push(Point::new(
                (cx + inner_r * ia.cos()).round() as i32,
                (cy + inner_r * ia.sin()).round() as i32,
            ));
        }
        self.ray_cast(local, &vertices)
    }

    fn contains_polygon(&self, local: Point, vertices: &[Point]) -> bool {
        self.ray_cast(local, vertices)
    }

    fn contains_rounded_rect(&self, local: Point, w: i32, h: i32, radius: u32) -> bool {
        let r = (radius as i32).min(w / 2).min(h / 2);
        if r <= 0 {
            return local.x >= 0 && local.y >= 0 && local.x < w && local.y < h;
        }
        let (x, y) = (local.x, local.y);
        if x < 0 || y < 0 || x >= w || y >= h {
            return false;
        }
        if x < r && y < r {
            return (x - r).pow(2) + (y - r).pow(2) <= r * r;
        }
        if x >= w - r && y < r {
            return (x - (w - r)).pow(2) + (y - r).pow(2) <= r * r;
        }
        if x < r && y >= h - r {
            return (x - r).pow(2) + (y - (h - r)).pow(2) <= r * r;
        }
        if x >= w - r && y >= h - r {
            return (x - (w - r)).pow(2) + (y - (h - r)).pow(2) <= r * r;
        }
        true
    }

    fn contains_bubble(&self, local: Point, w: i32, h: i32, td: BubbleTailDirection) -> bool {
        let ts = 12i32;
        let (x, y) = (local.x, local.y);
        let (x_min, y_min, x_max, y_max) = match td {
            BubbleTailDirection::Left => (-ts, 0, w, h),
            BubbleTailDirection::Right => (0, 0, w + ts, h),
            BubbleTailDirection::Top => (0, -ts, w, h),
            BubbleTailDirection::Bottom => (0, 0, w, h + ts),
            BubbleTailDirection::TopLeft => (-ts, -ts, w, h),
            BubbleTailDirection::TopRight => (0, -ts, w + ts, h),
            BubbleTailDirection::BottomLeft => (-ts, 0, w, h + ts),
            BubbleTailDirection::BottomRight => (0, 0, w + ts, h + ts),
        };
        if x < x_min || y < y_min || x >= x_max || y >= y_max {
            return false;
        }
        let r = 8u32.min(w as u32 / 2).min(h as u32 / 2).max(1);
        if x >= 0 && x < w && y >= 0 && y < h {
            return self.contains_rounded_rect(local, w, h, r);
        }
        let (tcx, tcy, tdx, tdy) = match td {
            BubbleTailDirection::Left => (0, h / 2, -1, 0),
            BubbleTailDirection::Right => (w - 1, h / 2, 1, 0),
            BubbleTailDirection::Top => (w / 2, 0, 0, -1),
            BubbleTailDirection::Bottom => (w / 2, h - 1, 0, 1),
            BubbleTailDirection::TopLeft => (0, 0, -1, -1),
            BubbleTailDirection::TopRight => (w - 1, 0, 1, -1),
            BubbleTailDirection::BottomLeft => (0, h - 1, -1, 1),
            BubbleTailDirection::BottomRight => (w - 1, h - 1, 1, 1),
        };
        let (bdx, bdy) = match td {
            BubbleTailDirection::Left | BubbleTailDirection::Right => (0, ts / 2),
            BubbleTailDirection::Top | BubbleTailDirection::Bottom => (ts / 2, 0),
            BubbleTailDirection::TopLeft | BubbleTailDirection::BottomRight => (ts / 2, -ts / 2),
            BubbleTailDirection::TopRight | BubbleTailDirection::BottomLeft => (ts / 2, ts / 2),
        };
        let tip = Point::new(tcx + tdx * ts, tcy + tdy * ts);
        let b1 = Point::new(tcx + bdx, tcy + bdy);
        let b2 = Point::new(tcx - bdx, tcy - bdy);
        self.point_in_triangle(local, tip, b1, b2)
    }

    fn contains_custom(&self, local: Point, segments: &[PathSegment]) -> bool {
        if segments.is_empty() {
            return false;
        }
        let mut pv = Vec::new();
        let mut cur = Point::new(0, 0);
        let mut start = Point::new(0, 0);
        for seg in segments {
            match seg {
                PathSegment::MoveTo(p) => {
                    if pv.len() > 1 {
                        pv.push(start);
                    }
                    start = *p;
                    cur = *p;
                    pv.push(*p);
                }
                PathSegment::LineTo(p) => {
                    cur = *p;
                    pv.push(*p);
                }
                PathSegment::CurveTo(c1, c2, p) => {
                    self.flatten_cubic_to(&mut pv, cur, *c1, *c2, *p);
                    cur = *p;
                }
                PathSegment::QuadTo(c, p) => {
                    self.flatten_quad_to(&mut pv, cur, *c, *p);
                    cur = *p;
                }
                PathSegment::Close => {
                    if pv.last() != Some(&start) {
                        pv.push(start);
                    }
                    cur = start;
                }
            }
        }
        if pv.len() > 1 && pv.last() != Some(&start) {
            pv.push(start);
        }
        if pv.len() < 3 {
            return false;
        }
        self.ray_cast(local, &pv)
    }

    fn point_in_triangle(&self, pt: Point, a: Point, b: Point, c: Point) -> bool {
        let s = |px: f32, py: f32, x1: f32, y1: f32, x2: f32, y2: f32| -> f32 {
            (px - x2) * (y1 - y2) - (x1 - x2) * (py - y2)
        };
        let (ax, ay, bx, by, cx, cy, px, py) = (
            a.x as f32,
            a.y as f32,
            b.x as f32,
            b.y as f32,
            c.x as f32,
            c.y as f32,
            pt.x as f32,
            pt.y as f32,
        );
        let d1 = s(px, py, ax, ay, bx, by);
        let d2 = s(px, py, bx, by, cx, cy);
        let d3 = s(px, py, cx, cy, ax, ay);
        let has_neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
        let has_pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
        !(has_neg && has_pos)
    }

    fn flatten_cubic_to(
        &self,
        vertices: &mut Vec<Point>,
        p0: Point,
        p1: Point,
        p2: Point,
        p3: Point,
    ) {
        let flatness_sq = 4.0;
        let dx = p3.x - p0.x;
        let dy = p3.y - p0.y;
        let d2 = ((p1.x - p3.x) * dy - (p1.y - p3.y) * dx).abs() as f32;
        let d3 = ((p2.x - p3.x) * dy - (p2.y - p3.y) * dx).abs() as f32;
        if (d2 + d3).powi(2) <= flatness_sq * (dx * dx + dy * dy) as f32 + 0.0001 {
            vertices.push(p3);
            return;
        }
        let lerp = |a: i32, b: i32| -> i32 { ((a as f32 + b as f32) / 2.0).round() as i32 };
        let mid1 = Point::new(lerp(p0.x, p1.x), lerp(p0.y, p1.y));
        let mid2 = Point::new(lerp(p1.x, p2.x), lerp(p1.y, p2.y));
        let mid3 = Point::new(lerp(p2.x, p3.x), lerp(p2.y, p3.y));
        let mid12 = Point::new(lerp(mid1.x, mid2.x), lerp(mid1.y, mid2.y));
        let mid23 = Point::new(lerp(mid2.x, mid3.x), lerp(mid2.y, mid3.y));
        let mid = Point::new(lerp(mid12.x, mid23.x), lerp(mid12.y, mid23.y));
        self.flatten_cubic_to(vertices, p0, mid1, mid12, mid);
        self.flatten_cubic_to(vertices, mid, mid23, mid3, p3);
    }

    fn flatten_quad_to(&self, vertices: &mut Vec<Point>, p0: Point, p1: Point, p2: Point) {
        let flatness_sq = 4.0;
        let dx = p2.x - p0.x;
        let dy = p2.y - p0.y;
        let d = ((p1.x - p2.x) * dy - (p1.y - p2.y) * dx).abs() as f32;
        if d * d <= flatness_sq * (dx * dx + dy * dy) as f32 + 0.0001 {
            vertices.push(p2);
            return;
        }
        let lerp = |a: i32, b: i32| -> i32 { ((a as f32 + b as f32) / 2.0).round() as i32 };
        let mid1 = Point::new(lerp(p0.x, p1.x), lerp(p0.y, p1.y));
        let mid2 = Point::new(lerp(p1.x, p2.x), lerp(p1.y, p2.y));
        let mid = Point::new(lerp(mid1.x, mid2.x), lerp(mid1.y, mid2.y));
        self.flatten_quad_to(vertices, p0, mid1, mid);
        self.flatten_quad_to(vertices, mid, mid2, p2);
    }

    /// Draws the shape with an explicit fill/outline pair.
    ///
    /// The shape helpers read `self.fill_color` / `self.stroke_color`, so the
    /// resolved pair is installed on the widget for the duration of the draw. The
    /// appearance-derived colours therefore never become the widget's stored state —
    /// [`Self::fill_color`] still reports the caller's colour — and no shape helper
    /// needs two extra arguments. The previous pair is restored by a guard, so it
    /// survives an early return as well as the normal path.
    fn draw_filled_shape(
        &mut self,
        context: &mut RenderContext,
        fill: Color,
        stroke: Option<Color>,
    ) {
        let restore = RestoreShapePalette {
            fill: std::mem::replace(&mut self.fill_color, fill),
            stroke: std::mem::replace(&mut self.stroke_color, stroke),
        };
        self.draw_shape(context);
        restore.apply(self);
    }

    fn draw_shape(&self, context: &mut RenderContext) {
        let rect = self.base.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        match &self.path {
            ShapePath::Heart => self.draw_heart(context, rect),
            ShapePath::Star { points, inner_radius } => {
                self.draw_star(context, rect, *points, *inner_radius)
            }
            ShapePath::Polygon(v) => self.draw_polygon(context, rect, v),
            ShapePath::RoundedRect { radius } => self.draw_rounded_rect(context, rect, *radius),
            ShapePath::Bubble { tail_direction } => {
                self.draw_bubble(context, rect, *tail_direction)
            }
            ShapePath::Custom(s) => self.draw_custom(context, rect, s),
            ShapePath::UnitCustom(s) => {
                let pixels: Vec<PathSegment> = s
                    .iter()
                    .map(|segment| segment.to_path_segment(rect.width as f32, rect.height as f32))
                    .collect();
                self.draw_custom(context, rect, &pixels);
            }
        }
    }

    fn draw_heart(&self, context: &mut RenderContext, rect: Rect) {
        let cx = rect.x + rect.width as i32 / 2;
        let cy = rect.y + rect.height as i32 / 2;
        let r = (rect.width.min(rect.height) as i32 / 10).max(3);
        let (lcx, lcy) = (cx - r * 2, cy - r);
        let (rcx, rcy) = (cx + r * 2, cy - r);
        context.fill_circle(Point::new(lcx, lcy), r as u32, self.fill_color);
        context.fill_circle(Point::new(rcx, rcy), r as u32, self.fill_color);
        let tip_y = cy + 2 * r;
        let (blx, brx) = (cx - 3 * r, cx + 3 * r);
        for x in blx..=brx {
            let t = if brx > blx { (x - blx) as f32 / (brx - blx) as f32 } else { 0.5 };
            let y0 = (lcy as f32 + (tip_y - lcy) as f32 * (1.0 - (t - 0.5).abs() * 2.0)) as i32;
            if y0 <= tip_y {
                context.fill_rect(Rect::new(x, y0, 1, (tip_y - y0 + 1) as u32), self.fill_color);
            }
        }
        if let Some(sc) = self.stroke_color {
            let sw = self.stroke_width;
            context.draw_circle_stroke(Point::new(lcx, lcy), r as u32, sc, sw);
            context.draw_circle_stroke(Point::new(rcx, rcy), r as u32, sc, sw);
            context.draw_line_stroke(Point::new(blx, lcy), Point::new(cx, tip_y), sc, sw);
            context.draw_line_stroke(Point::new(brx, lcy), Point::new(cx, tip_y), sc, sw);
        }
    }

    fn draw_star(&self, context: &mut RenderContext, rect: Rect, num_points: u8, inner_ratio: f32) {
        let cx = rect.x + rect.width as i32 / 2;
        let cy = rect.y + rect.height as i32 / 2;
        let outer_r = (rect.width.min(rect.height) as i32 / 2).max(1) as f32;
        let inner_r = outer_r * inner_ratio.clamp(0.05, 0.95);
        let n = num_points.max(3) as usize;
        let mut vertices = Vec::with_capacity(n * 2);
        let step = std::f32::consts::PI / n as f32;
        for i in 0..n {
            let angle =
                i as f32 * 2.0 * std::f32::consts::PI / n as f32 - std::f32::consts::FRAC_PI_2;
            vertices.push(Point::new(
                (cx as f32 + outer_r * angle.cos()).round() as i32,
                (cy as f32 + outer_r * angle.sin()).round() as i32,
            ));
            let ia = angle + step;
            vertices.push(Point::new(
                (cx as f32 + inner_r * ia.cos()).round() as i32,
                (cy as f32 + inner_r * ia.sin()).round() as i32,
            ));
        }
        for i in 0..vertices.len() {
            let j = (i + 1) % vertices.len();
            self.fill_triangle_approx(context, Point::new(cx, cy), vertices[i], vertices[j]);
        }
        if let Some(sc) = self.stroke_color {
            for i in 0..vertices.len() {
                let j = (i + 1) % vertices.len();
                context.draw_line_stroke(vertices[i], vertices[j], sc, self.stroke_width);
            }
        }
    }

    fn draw_polygon(&self, context: &mut RenderContext, rect: Rect, vertices: &[Point]) {
        if vertices.len() < 3 {
            return;
        }
        let centroid = Point::new(
            vertices.iter().map(|v| v.x).sum::<i32>() / vertices.len() as i32,
            vertices.iter().map(|v| v.y).sum::<i32>() / vertices.len() as i32,
        );
        let vs: Vec<Point> =
            vertices.iter().map(|v| Point::new(rect.x + v.x, rect.y + v.y)).collect();
        for i in 0..vs.len() {
            let j = (i + 1) % vs.len();
            self.fill_triangle_approx(context, centroid, vs[i], vs[j]);
        }
        if let Some(sc) = self.stroke_color {
            for i in 0..vs.len() {
                let j = (i + 1) % vs.len();
                context.draw_line_stroke(vs[i], vs[j], sc, self.stroke_width);
            }
        }
    }

    fn draw_rounded_rect(&self, context: &mut RenderContext, rect: Rect, radius: u32) {
        let r = radius.min(rect.width / 2).min(rect.height / 2);
        if r > 0 {
            context.fill_rounded_rect(rect, r, self.fill_color);
        } else {
            context.fill_rect(rect, self.fill_color);
        }
        if let Some(sc) = self.stroke_color {
            if r > 0 {
                context.draw_rounded_rect_stroke(rect, r, sc, self.stroke_width);
            } else {
                context.draw_rect_stroke(rect, sc, self.stroke_width);
            }
        }
    }

    fn draw_bubble(&self, context: &mut RenderContext, rect: Rect, td: BubbleTailDirection) {
        let r = 8u32.min(rect.width / 2).min(rect.height / 2).max(1);
        let ts = 12i32;
        context.fill_rounded_rect(rect, r, self.fill_color);
        let (tcx, tcy, tdx, tdy) = match td {
            BubbleTailDirection::Left => (rect.x, rect.y + rect.height as i32 / 2, -1, 0),
            BubbleTailDirection::Right => {
                (rect.x + rect.width as i32 - 1, rect.y + rect.height as i32 / 2, 1, 0)
            }
            BubbleTailDirection::Top => (rect.x + rect.width as i32 / 2, rect.y, 0, -1),
            BubbleTailDirection::Bottom => {
                (rect.x + rect.width as i32 / 2, rect.y + rect.height as i32 - 1, 0, 1)
            }
            BubbleTailDirection::TopLeft => (rect.x, rect.y, -1, -1),
            BubbleTailDirection::TopRight => (rect.x + rect.width as i32 - 1, rect.y, 1, -1),
            BubbleTailDirection::BottomLeft => (rect.x, rect.y + rect.height as i32 - 1, -1, 1),
            BubbleTailDirection::BottomRight => {
                (rect.x + rect.width as i32 - 1, rect.y + rect.height as i32 - 1, 1, 1)
            }
        };
        let tip = Point::new(tcx + tdx * ts, tcy + tdy * ts);
        let (bdx, bdy) = match td {
            BubbleTailDirection::Left | BubbleTailDirection::Right => (0, ts / 2),
            BubbleTailDirection::Top | BubbleTailDirection::Bottom => (ts / 2, 0),
            BubbleTailDirection::TopLeft | BubbleTailDirection::BottomRight => (ts / 2, -ts / 2),
            BubbleTailDirection::TopRight | BubbleTailDirection::BottomLeft => (ts / 2, ts / 2),
        };
        let b1 = Point::new(tcx + bdx, tcy + bdy);
        let b2 = Point::new(tcx - bdx, tcy - bdy);
        self.fill_triangle_approx(context, tip, b1, b2);
        if let Some(sc) = self.stroke_color {
            context.draw_rounded_rect_stroke(rect, r, sc, self.stroke_width);
            context.draw_line_stroke(b1, tip, sc, self.stroke_width);
            context.draw_line_stroke(b2, tip, sc, self.stroke_width);
        }
    }

    fn draw_custom(&self, context: &mut RenderContext, rect: Rect, segments: &[PathSegment]) {
        let mut pv = Vec::new();
        let mut cur = Point::new(0, 0);
        let mut start = Point::new(0, 0);
        for seg in segments {
            match seg {
                PathSegment::MoveTo(p) => {
                    if pv.len() > 1 {
                        pv.push(start);
                    }
                    start = *p;
                    cur = *p;
                    pv.push(*p);
                }
                PathSegment::LineTo(p) => {
                    cur = *p;
                    pv.push(*p);
                }
                PathSegment::CurveTo(c1, c2, p) => {
                    self.flatten_cubic_to(&mut pv, cur, *c1, *c2, *p);
                    cur = *p;
                }
                PathSegment::QuadTo(c, p) => {
                    self.flatten_quad_to(&mut pv, cur, *c, *p);
                    cur = *p;
                }
                PathSegment::Close => {
                    if pv.last() != Some(&start) {
                        pv.push(start);
                    }
                    cur = start;
                }
            }
        }
        if pv.len() > 1 && pv.last() != Some(&start) {
            pv.push(start);
        }
        if pv.len() < 3 {
            return;
        }
        let vs: Vec<Point> = pv.iter().map(|v| Point::new(rect.x + v.x, rect.y + v.y)).collect();
        let centroid = Point::new(
            vs.iter().map(|v| v.x).sum::<i32>() / vs.len() as i32,
            vs.iter().map(|v| v.y).sum::<i32>() / vs.len() as i32,
        );
        for i in 0..vs.len() {
            let j = (i + 1) % vs.len();
            self.fill_triangle_approx(context, centroid, vs[i], vs[j]);
        }
        if let Some(sc) = self.stroke_color {
            for i in 0..vs.len() {
                let j = (i + 1) % vs.len();
                context.draw_line_stroke(vs[i], vs[j], sc, self.stroke_width);
            }
        }
    }

    fn fill_triangle_approx(&self, context: &mut RenderContext, v0: Point, v1: Point, v2: Point) {
        let mut vs = [v0, v1, v2];
        vs.sort_by_key(|v| v.y);
        let [a, b, c] = vs;
        if a.y == c.y {
            let min_x = a.x.min(b.x).min(c.x);
            let max_x = a.x.max(b.x).max(c.x);
            if max_x > min_x {
                context
                    .fill_rect(Rect::new(min_x, a.y, (max_x - min_x) as u32, 1), self.fill_color);
            }
            return;
        }
        let total_height = c.y - a.y;
        if total_height <= 0 {
            return;
        }
        let lerp_x =
            |p1: Point, p2: Point, t: f32| -> f32 { p1.x as f32 + (p2.x - p1.x) as f32 * t };
        for y in a.y..=c.y {
            let (x1, x2) = if y < b.y {
                let sub_h = b.y - a.y;
                if sub_h == 0 {
                    continue;
                }
                (
                    lerp_x(a, c, (y - a.y) as f32 / total_height as f32),
                    lerp_x(a, b, (y - a.y) as f32 / sub_h as f32),
                )
            } else {
                let sub_h = c.y - b.y;
                if sub_h == 0 {
                    continue;
                }
                (
                    lerp_x(a, c, (y - a.y) as f32 / total_height as f32),
                    lerp_x(b, c, (y - b.y) as f32 / sub_h as f32),
                )
            };
            let x_start = x1.min(x2).round() as i32;
            let x_end = x1.max(x2).round() as i32;
            if x_end > x_start {
                context
                    .fill_rect(Rect::new(x_start, y, (x_end - x_start) as u32, 1), self.fill_color);
            }
        }
    }

    fn ray_cast(&self, point: Point, vertices: &[Point]) -> bool {
        let n = vertices.len();
        if n < 3 {
            return false;
        }
        let mut inside = false;
        let mut j = n - 1;
        for i in 0..n {
            let vi = vertices[i];
            let vj = vertices[j];
            if ((vi.y > point.y) != (vj.y > point.y))
                && (point.x as i64)
                    < ((vj.x as i64 - vi.x as i64) * (point.y as i64 - vi.y as i64)
                        / (vj.y as i64 - vi.y as i64)
                        + vi.x as i64)
            {
                inside = !inside;
            }
            j = i;
        }
        inside
    }
}

impl Widget for FreeformShapeWidget {
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
        crate::core::Size::new(100, 100)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why this is explicit
    ///
    /// `connect_event` validates a name against the capability table and registers a hub slot; only
    /// `event_signal_dyn` joins that name to the signal the control actually **emits**. Without an
    /// arm a published name is valid and inert, which is the silent failure
    /// `tools/check_event_signal_dyn.sh` exists to make impossible: the arm set is compared against
    /// the capability's published names, so the two cannot drift.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "clicked" => Some(EventSignalRef::unit("clicked", &self.clicked)),
            "hovered_changed" => {
                Some(EventSignalRef::mapped("hovered_changed", &self.hovered_changed, |v| {
                    CapabilityValue::Bool(*v)
                }))
            }
            "pressed_changed" => {
                Some(EventSignalRef::mapped("pressed_changed", &self.pressed_changed, |v| {
                    CapabilityValue::Bool(*v)
                }))
            }
            "geometry_changed" => {
                Some(EventSignalRef::mapped("geometry_changed", &self.geometry_changed, |v| {
                    crate::widget::capability::CapabilityValue::String(format!(
                        "{},{},{},{}",
                        v.x, v.y, v.width, v.height
                    ))
                }))
            }
            "moved" => Some(EventSignalRef::mapped("moved", &self.moved, |v| {
                CapabilityValue::String(format!("{},{}", v.x, v.y))
            })),
            "vertex_changed" => {
                Some(EventSignalRef::mapped("vertex_changed", &self.vertex_changed, |v| {
                    CapabilityValue::Tuple(crate::compat::Vec::from([
                        CapabilityValue::UInt(v.0 as u64),
                        CapabilityValue::String(format!("{},{}", v.1.x, v.1.y)),
                    ]))
                }))
            }
            "history_changed" => {
                Some(EventSignalRef::mapped("history_changed", &self.history_changed, |v| {
                    CapabilityValue::Tuple(crate::compat::Vec::from([
                        CapabilityValue::UInt(v.0 as u64),
                        CapabilityValue::UInt(v.1 as u64),
                    ]))
                }))
            }
            _ => None,
        }
    }
}

/// `FreeformShapeWidget`'s property contract, published under the
/// `FreeformShape` kind.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` / `access_write_other.in.rs` dispatch, including
/// the `Null` handling of `stroke_rgba` and the `TypeMismatch` an unparsable hex
/// colour produced.
impl WidgetProperties for FreeformShapeWidget {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "path_kind" => {
                let s = match self.path() {
                    ShapePath::Heart => "heart",
                    ShapePath::Star { .. } => "star",
                    ShapePath::Polygon(_) => "polygon",
                    ShapePath::RoundedRect { .. } => "rounded_rect",
                    ShapePath::Bubble { .. } => "bubble",
                    ShapePath::Custom(_) => "custom",
                    ShapePath::UnitCustom(_) => "unit_custom",
                };
                Ok(CapabilityValue::String(s.to_string()))
            }
            "fill_rgba" => Ok(CapabilityValue::String(self.fill_color().to_hex_rgba())),
            "stroke_rgba" => match self.stroke_color() {
                Some(color) => Ok(CapabilityValue::String(color.to_hex_rgba())),
                None => Ok(CapabilityValue::Null),
            },
            "stroke_width" => Ok(CapabilityValue::UInt(self.stroke_width() as u64)),
            "movable" => Ok(CapabilityValue::Bool(self.is_movable())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "fill_rgba" => {
                let raw = expect_string(value)?;
                let Some(color) = crate::core::Color::parse_hex(&raw) else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                self.set_fill_color(color);
                Ok(())
            }
            "stroke_rgba" => {
                match value {
                    CapabilityValue::Null => self.set_stroke_color(None),
                    CapabilityValue::String(raw) => {
                        let Some(color) = crate::core::Color::parse_hex(&raw) else {
                            return Err(CapabilityAccessError::TypeMismatch);
                        };
                        self.set_stroke_color(Some(color));
                    }
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                }
                Ok(())
            }
            "stroke_width" => {
                self.set_stroke_width(expect_u32(value)?);
                Ok(())
            }
            "movable" => {
                self.set_movable(crate::widget::capability::coercion::expect_bool(value)?);
                Ok(())
            }
            "path_kind" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "path_kind",
            "fill_rgba",
            "stroke_rgba",
            "stroke_width",
            "movable",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl Draw for FreeformShapeWidget {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.base.geometry();

        // Chrome colours resolve explicit style first, then the theme's resolved
        // style for this control, and only then a literal. The theme step is what
        // makes an appearance switch visible; previously the plate below was a
        // hardcoded white and the shape's default fill a hardcoded blue, so light
        // and dark rendered identically.
        //
        // `resolved_theme_style` takes and releases the global manager's lock
        // internally, so no guard is held across the draw (the mutex is not
        // re-entrant).
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("freeform_shape");
        // `freeform_shape` is not a control kind in the role table, so it classifies as
        // `Surface`, whose background is `theme.colors.background` — the window's own
        // colour. The plate behind the outline is therefore a step toward the
        // foreground, so the widget's extent is visible.
        let resolved = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let text_color = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        let background = resolved.blend(&text_color, 0.08);

        // A caller who set a fill owns it — that colour is the datum, so it is used
        // verbatim, and hover still acknowledges the pointer by stepping toward that colour's own
        // contrast. Only the *default* fill follows the theme, which is what makes
        // an untouched shape respond to an appearance switch.
        let base_fill = if self.fill_overridden { self.fill_color } else { text_color };

        // # Why the pointer state changes what is painted
        //
        // `hovered_item` and `pressed_item` were tracked, signalled (`hovered_changed`,
        // `pressed_changed`) and **never read by `draw`** — so a shape reported a hover to every
        // listener while looking exactly like a shape nothing was pointing at. That is the same
        // defect `segmented_control`'s teleporting pill was: the mechanism exists, the signal is
        // honest, and the pixels ignore both. A user who has to *guess* whether their pointer is
        // on a shape cannot drag it with any confidence.
        //
        // The weights come from `StateOverlay::fill_blend`, the crate's one source for "how far
        // does a hover move a surface" — it is what the preset theme's `"<kind>:hover"` key
        // derives from, so an overlaid control and a theme-driven one move by the same amount.
        // Press is the firmer step, and a press already implies the pointer is down on the shape,
        // so the two are mutually exclusive here exactly as they are in the overlay.
        let overlay =
            crate::style::StateOverlay::from_base(self.hovered_item, self.pressed_item, false);
        let fill = if overlay.is_empty() {
            base_fill
        } else {
            base_fill.blend(&base_fill.contrast_color(), overlay.fill_blend())
        };
        let stroke = match (self.stroke_overridden, self.stroke_color) {
            (true, Some(color)) => Some(color),
            // An explicit `None` from the caller stays `None`; the default derives
            // an outline from the resolved pair.
            (true, None) => None,
            (false, _) => Some(fill.blend(&background, 0.5)),
        };

        context.face_with_gradient(
            rect,
            background,
            self.base.style().background_gradient.as_ref(),
            self.base.style().surface.unwrap_or_default(),
            self.base.style().border_radius.unwrap_or(0),
            crate::core::Color::BLACK,
        );
        self.draw_filled_shape(context, fill, stroke);
        // Resize handles are chrome, drawn over the shape so they stay reachable even on an
        // opaque fill.
        if self.editable {
            for (_, handle) in self.handle_rects() {
                context.fill_rect(handle, text_color.contrast_color());
                context.draw_rect(handle, text_color);
            }
        }
        if !self.base.is_enabled() {
            // A disabled shape is a chrome state, so the veil is derived from the
            // resolved pair rather than a fixed grey.
            let overlay = background.blend(&text_color, 0.25).with_alpha(128);
            context.fill_rect(rect, overlay);
        }
    }
}

/// The shape palette that was in place before an appearance-resolved pair replaced
/// it for one draw.
///
/// Held so [`FreeformShapeWidget::draw_filled_shape`] can put the widget's own
/// colours back after painting, which keeps the theme's colours out of the
/// widget's persistent state. Restoration is explicit rather than a `Drop`
/// implementation so the borrow of the widget ends before the guard is applied.
struct RestoreShapePalette {
    fill: Color,
    stroke: Option<Color>,
}

impl RestoreShapePalette {
    /// Writes the saved pair back onto `shape`.
    fn apply(self, shape: &mut FreeformShapeWidget) {
        shape.fill_color = self.fill;
        shape.stroke_color = self.stroke;
    }
}

impl crate::event::EventHandler for FreeformShapeWidget {
    fn handle_event(&mut self, event: &crate::event::Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            crate::event::Event::MouseMove { pos } => {
                if self.drag_handle.is_some() {
                    self.drag_to(*pos);
                    return;
                }
                if let Some((origin, start)) = self.body_drag {
                    let target = self.snapped(Point::new(
                        start.x + (pos.x - origin.x),
                        start.y + (pos.y - origin.y),
                    ));
                    let moved = Rect::new(target.x, target.y, start.width, start.height);
                    self.base.set_geometry(moved);
                    self.moved.emit(Point::new(moved.x, moved.y));
                    self.geometry_changed.emit(moved);
                    self.base.request_redraw();
                    return;
                }
                let was_hovered = self.hovered_item;
                self.hovered_item = self.contains(*pos);
                if was_hovered != self.hovered_item {
                    self.hovered_changed.emit(self.hovered_item);
                    self.base.request_redraw();
                }
            }
            crate::event::Event::MouseEnter { pos } => {
                // Enter carries the position, so the outline test is the same one a
                // move makes. Unconditionally reporting hover made a pointer in the
                // transparent corner of a star claim to be on the shape.
                let hovered = self.contains(*pos);
                if hovered != self.hovered_item {
                    self.hovered_item = hovered;
                    self.hovered_changed.emit(hovered);
                    self.base.request_redraw();
                }
            }
            crate::event::Event::KeyPress { key, modifiers } => {
                let modifiers = crate::shortcut::Modifiers::from_event_bits(*modifiers);
                if modifiers.contains(crate::shortcut::Modifiers::PRIMARY) {
                    match crate::shortcut::Key::from_key_code(*key) {
                        Some(crate::shortcut::Key::Z)
                            if modifiers.contains(crate::shortcut::Modifiers::SHIFT) =>
                        {
                            self.redo();
                        }
                        Some(crate::shortcut::Key::Z) => {
                            self.undo();
                        }
                        Some(crate::shortcut::Key::Y) => {
                            self.redo();
                        }
                        _ => {}
                    }
                }
            }
            crate::event::Event::MousePress { pos, button: 1, .. }
                if self.editable && self.handle_at(*pos).is_some() =>
            {
                if let Some(handle) = self.handle_at(*pos) {
                    self.drag_vertex_start = match handle {
                        ShapeHandle::Vertex(index) => match &self.path {
                            ShapePath::Polygon(vertices) => vertices.get(index).copied(),
                            _ => None,
                        },
                        _ => None,
                    };
                    self.drag_handle = Some((handle, *pos, self.base.geometry()));
                    self.base.request_redraw();
                }
            }
            crate::event::Event::MouseRelease { button: 1, .. } if self.drag_handle.is_some() => {
                if let Some((_, _, start)) = self.drag_handle.take() {
                    let after = self.base.geometry();
                    self.record_geometry(start, after);
                }
                self.drag_vertex_start = None;
                self.base.request_redraw();
            }
            crate::event::Event::MouseLeave { .. } => {
                self.drag_handle = None;
                self.drag_vertex_start = None;
                self.body_drag = None;
                self.hovered_item = false;
                self.pressed_item = false;
                self.hovered_changed.emit(false);
                self.pressed_changed.emit(false);
                self.base.request_redraw();
            }
            crate::event::Event::MousePress { pos, button, .. }
                if *button == 1 && self.contains(*pos) =>
            {
                self.pressed_item = true;
                self.pressed_changed.emit(true);
                self.base.set_mouse_pressed(true);
                if self.movable {
                    self.body_drag = Some((*pos, self.base.geometry()));
                }
                self.base.request_redraw();
            }
            crate::event::Event::MouseRelease { pos, button } if *button == 1 => {
                if let Some((_, start)) = self.body_drag.take() {
                    let after = self.base.geometry();
                    self.record_geometry(start, after);
                }
                let was_pressed = self.pressed_item;
                self.pressed_item = false;
                self.base.set_mouse_pressed(false);
                if was_pressed && self.contains(*pos) {
                    self.clicked.emit();
                }
                self.pressed_changed.emit(false);
                self.base.request_redraw();
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}
