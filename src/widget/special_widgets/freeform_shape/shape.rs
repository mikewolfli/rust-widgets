// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! [`FreeformShapeWidget`] — the control that draws a [`ShapePath`](super::ShapePath).

use super::types::{BubbleTailDirection, PathSegment, ShapePath};
use crate::core::{Color, ObjectId, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::undo::{UndoStack, ValueCommand};
use crate::widget::capability::coercion::{expect_bool, expect_string, expect_u32};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use alloc::boxed::Box;
use alloc::rc::Rc;
use core::cell::{Ref, RefCell};

/// The chrome colours a freeform shape paints with.
///
/// Resolved once per canvas paint and handed to every hosted shape, so a board of `n` shapes no
/// longer takes `n` theme-manager locks per frame (SP-P57). A standalone shape resolves its own
/// palette through [`FreeformShapeWidget::palette`], which is the same derivation.
pub(crate) struct ShapePalette {
    /// The plate a shape sits on, derived one step off the resolved background.
    pub(crate) background: Color,
    /// The ink hover and press states step toward.
    pub(crate) text_color: Color,
}

/// A flattened outline in shape-local coordinates, with the inputs it was built from.
///
/// Star vertices, polygons and flattened custom paths are pure functions of the path and the
/// widget's size, yet the hit test rebuilt them on every pointer move and `draw` rebuilt them on
/// every frame (SP-P55). Both read this cache instead.
struct OutlineCache {
    /// The path revision the vertices were built for.
    revision: u64,
    /// Widget width the vertices were built for.
    width: u32,
    /// Widget height the vertices were built for.
    height: u32,
    /// The flattened outline, or empty for paths that need no vertex cache.
    vertices: Vec<Point>,
}

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
    pub hover_changed: Signal1<bool>,
    /// Emitted when the primary button is pressed or released inside the
    /// outline, with the new state. A press that begins outside the shape and
    /// drags in never emits it.
    pub pressed_changed: Signal1<bool>,
    /// Bumped whenever the path changes, so [`Self::outline_vertices`] can tell a stale
    /// cache entry from a current one.
    path_revision: u64,
    /// Cached flattened outline; see [`OutlineCache`].
    outline_cache: RefCell<Option<OutlineCache>>,
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
            hover_changed: Signal1::new(),
            pressed_changed: Signal1::new(),
            path_revision: 1,
            outline_cache: RefCell::new(None),
        }
    }

    /// Compatibility spelling of [`Self::hover_changed`] (W-21): the same emitted signal.
    pub fn hovered_changed(&self) -> &Signal1<bool> {
        &self.hover_changed
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
        self.path_revision = self.path_revision.wrapping_add(1);
        self.outline_cache = RefCell::new(None);
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
        self.path_revision = self.path_revision.wrapping_add(1);
        self.outline_cache = RefCell::new(None);
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
        self.path_revision = self.path_revision.wrapping_add(1);
        self.outline_cache = RefCell::new(None);
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
                self.path_revision = self.path_revision.wrapping_add(1);
                self.outline_cache = RefCell::new(None);
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
            ShapePath::Star { .. }
            | ShapePath::Polygon(_)
            | ShapePath::Custom(_)
            | ShapePath::UnitCustom(_) => {
                // Every one of these is a flattened polygon: the star and the polygon already are
                // one, and the custom paths are curve-flattened. Reading the shared cache is what
                // keeps a pointer move from re-flattening the same outline (SP-P55).
                let vertices = self.outline_vertices(w, h);
                self.ray_cast(local, &vertices)
            }
            ShapePath::RoundedRect { radius } => self.contains_rounded_rect(local, w, h, *radius),
            ShapePath::Bubble { tail_direction } => {
                self.contains_bubble(local, w, h, *tail_direction)
            }
        }
    }

    /// The flattened outline in shape-local coordinates, built once per path revision and size.
    fn outline_vertices(&self, w: i32, h: i32) -> Ref<'_, Vec<Point>> {
        let fresh = self.outline_cache.borrow().as_ref().is_some_and(|cache| {
            cache.revision == self.path_revision
                && cache.width == w as u32
                && cache.height == h as u32
        });
        if !fresh {
            let vertices = self.build_outline(w, h);
            *self.outline_cache.borrow_mut() = Some(OutlineCache {
                revision: self.path_revision,
                width: w as u32,
                height: h as u32,
                vertices,
            });
        }
        Ref::map(self.outline_cache.borrow(), |cache| {
            &cache.as_ref().expect("the outline cache was just populated").vertices
        })
    }

    /// Flattens the path into shape-local vertices; empty for shapes that need no polygon.
    fn build_outline(&self, w: i32, h: i32) -> Vec<Point> {
        match &self.path {
            ShapePath::Star { points, inner_radius } => {
                self.star_vertices(w, h, *points, *inner_radius)
            }
            ShapePath::Polygon(vertices) => vertices.clone(),
            ShapePath::Custom(segments) => self.flatten_segments(segments),
            ShapePath::UnitCustom(segments) => {
                let pixels: Vec<PathSegment> = segments
                    .iter()
                    .map(|segment| segment.to_path_segment(w as f32, h as f32))
                    .collect();
                self.flatten_segments(&pixels)
            }
            ShapePath::Heart | ShapePath::RoundedRect { .. } | ShapePath::Bubble { .. } => {
                Vec::new()
            }
        }
    }

    /// The star's alternating outer/inner vertices, in shape-local coordinates.
    fn star_vertices(&self, w: i32, h: i32, num_points: u8, inner_ratio: f32) -> Vec<Point> {
        let n = num_points.max(3) as usize;
        let cx = w as f32 / 2.0;
        let cy = h as f32 / 2.0;
        let outer_r = cx.min(cy).max(1.0);
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
        vertices
    }

    /// Flattens path segments into one polyline in shape-local coordinates.
    fn flatten_segments(&self, segments: &[PathSegment]) -> Vec<Point> {
        let mut vertices = Vec::new();
        let mut current = Point::new(0, 0);
        let mut start = Point::new(0, 0);
        for segment in segments {
            match segment {
                PathSegment::MoveTo(point) => {
                    if vertices.len() > 1 {
                        vertices.push(start);
                    }
                    start = *point;
                    current = *point;
                    vertices.push(*point);
                }
                PathSegment::LineTo(point) => {
                    current = *point;
                    vertices.push(*point);
                }
                PathSegment::CurveTo(c1, c2, point) => {
                    self.flatten_cubic_to(&mut vertices, current, *c1, *c2, *point);
                    current = *point;
                }
                PathSegment::QuadTo(control, point) => {
                    self.flatten_quad_to(&mut vertices, current, *control, *point);
                    current = *point;
                }
                PathSegment::Close => {
                    if vertices.last() != Some(&start) {
                        vertices.push(start);
                    }
                    current = start;
                }
            }
        }
        if vertices.len() > 1 && vertices.last() != Some(&start) {
            vertices.push(start);
        }
        vertices
    }

    fn contains_heart(&self, local: Point, w: i32, h: i32) -> bool {
        let cx = w as f32 / 2.0;
        let cy = h as f32 / 2.0;
        let scale = (h as f32).max(1.0) / 2.0;
        let x = (local.x as f32 - cx) / scale;
        let y = (local.y as f32 - cy) / scale;
        (x * x + y * y - 1.0).powi(3) - x * x * (y * y * y) <= 0.0
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
        // Every polygonal variant fills through **one** polygon call. The previous form painted a
        // fan of 1-pixel `fill_rect` scanlines per triangle, so the primitive count grew with the
        // shape's area — a big star cost thousands of commands per frame (SP-P56).
        match &self.path {
            ShapePath::Heart => self.draw_heart(context, rect),
            ShapePath::Star { .. }
            | ShapePath::Polygon(_)
            | ShapePath::Custom(_)
            | ShapePath::UnitCustom(_) => {
                let vertices = self.outline_vertices(rect.width as i32, rect.height as i32);
                self.draw_polygon(context, rect, &vertices);
            }
            ShapePath::RoundedRect { radius } => self.draw_rounded_rect(context, rect, *radius),
            ShapePath::Bubble { tail_direction } => {
                self.draw_bubble(context, rect, *tail_direction)
            }
        }
    }

    /// Draws the heart from one polygon approximation of its outline.
    ///
    /// The implicit curve the hit test uses has no closed form for its outline, so the paint
    /// samples the standard heart parametrisation instead and fills the resulting polygon once.
    /// That keeps the ink and the hit region the same *shape* — both are the heart curve — while
    /// turning an area-proportional command count into a single fill.
    fn draw_heart(&self, context: &mut RenderContext, rect: Rect) {
        const SAMPLES: usize = 64;
        let cx = rect.x as f32 + rect.width as f32 / 2.0;
        let cy = rect.y as f32 + rect.height as f32 / 2.0;
        let sx = rect.width as f32 / 32.0;
        let sy = rect.height as f32 / 29.0;
        let mut points = Vec::with_capacity(SAMPLES);
        for sample in 0..SAMPLES {
            let t = sample as f32 / SAMPLES as f32 * std::f32::consts::TAU;
            let sin = t.sin();
            let x = 16.0 * sin * sin * sin;
            let y =
                13.0 * t.cos() - 5.0 * (2.0 * t).cos() - 2.0 * (3.0 * t).cos() - (4.0 * t).cos();
            // Screen y grows downward, so the curve's positive y is *above* the centre.
            points.push(Point::new((cx + x * sx).round() as i32, (cy - y * sy).round() as i32));
        }
        context.draw_path(&points, true, self.fill_color, true, 0);
        if let Some(stroke) = self.stroke_color {
            context.draw_path(&points, true, stroke, false, self.stroke_width.max(1));
        }
    }

    /// Fills and strokes one closed polygon, offset by the widget's own origin.
    fn draw_polygon(&self, context: &mut RenderContext, rect: Rect, vertices: &[Point]) {
        if vertices.len() < 3 {
            return;
        }
        let absolute: Vec<Point> = vertices
            .iter()
            .map(|vertex| Point::new(rect.x + vertex.x, rect.y + vertex.y))
            .collect();
        context.draw_path(&absolute, true, self.fill_color, true, 0);
        if let Some(stroke) = self.stroke_color {
            context.draw_path(&absolute, true, stroke, false, self.stroke_width.max(1));
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
                // A square shape keeps the square rule; only the rounded branch above needs a
                // radius, so this call states zero rather than re-reading the style.
                crate::widget::draw::stroke_surface_border(context, rect, 0, sc, self.stroke_width);
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
        // The tail is one filled triangle rather than a scanline stack (SP-P56).
        context.draw_path(&[tip, b1, b2], true, self.fill_color, true, 0);
        if let Some(sc) = self.stroke_color {
            context.draw_rounded_rect_stroke(rect, r, sc, self.stroke_width);
            context.draw_line_stroke(b1, tip, sc, self.stroke_width);
            context.draw_line_stroke(b2, tip, sc, self.stroke_width);
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
            "hover_changed" => {
                Some(EventSignalRef::mapped("hover_changed", &self.hover_changed, |v| {
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

impl FreeformShapeWidget {
    /// The palette this shape paints its own chrome with, resolved from the style and theme.
    ///
    /// A standalone shape calls this itself; a [`ShapeCanvas`] resolves it **once** and hands the
    /// same value to every hosted shape, so a board of `n` shapes does not take `n` theme-manager
    /// locks per frame (SP-P57).
    pub(crate) fn palette(&self) -> ShapePalette {
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("freeform_shape");
        // `freeform_shape` is not a control kind in the role table, so it classifies as
        // `Surface`, whose background is `theme.colors.background` — the window's own colour.
        // The plate behind the outline is therefore a step toward the foreground, so the widget's
        // extent is visible.
        let resolved = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let text_color = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        ShapePalette { background: resolved.blend(&text_color, 0.08), text_color }
    }

    /// Paints the shape using an already-resolved palette.
    ///
    /// Split out of [`Draw::draw`] so a canvas can resolve the theme once for every hosted shape.
    pub(crate) fn draw_with_palette(
        &mut self,
        context: &mut RenderContext,
        palette: &ShapePalette,
    ) {
        if self.outline_cache.borrow().is_none() {
            // Resolve the shared outline before painting, so the first frame and the hit test that
            // follows it cannot each flatten the same path.
            let rect = self.base.geometry();
            let _ = self.outline_vertices(rect.width as i32, rect.height as i32);
        }
        let rect = self.base.geometry();
        let background = palette.background;
        let text_color = palette.text_color;

        // A caller who set a fill owns it — that colour is the datum, so it is used
        // verbatim, and hover still acknowledges the pointer by stepping toward that colour's own
        // contrast. Only the *default* fill follows the theme, which is what makes
        // an untouched shape respond to an appearance switch.
        let base_fill = if self.fill_overridden { self.fill_color } else { text_color };

        // # Why the pointer state changes what is painted
        //
        // `hovered_item` and `pressed_item` were tracked, signalled (`hover_changed`,
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

impl Draw for FreeformShapeWidget {
    fn draw(&mut self, context: &mut RenderContext) {
        let palette = self.palette();
        self.draw_with_palette(context, &palette);
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
                    self.hover_changed.emit(self.hovered_item);
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
                    self.hover_changed.emit(hovered);
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
                self.hover_changed.emit(false);
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

/// One shape hosted by a [`ShapeCanvas`].
///
/// The canvas owns the shape control rather than a copy of its path and geometry: a caller that
/// already built a [`FreeformShapeWidget`] keeps its colours, stroke and undo history, and the
/// canvas paints and hit-tests the very object that was added. There is no second representation
/// to keep in step.
struct CanvasShape {
    id: ObjectId,
    widget: FreeformShapeWidget,
}

/// A board that hosts several [`FreeformShapeWidget`]s with selection and z-order.
///
/// # Why the canvas is a separate control
///
/// `FreeformShapeWidget` is one shape: it hit-tests a single outline and reports that shape's
/// clicks. A drawing surface needs the level above it — add/remove, a selection set, stacking
/// order and a rectangular selection — and none of those describe one shape. Putting them on the
/// shape would have made every shape carry a list of its siblings.
///
/// # Coordinates
///
/// Shapes keep their own geometry in the canvas's coordinate space, exactly as they were added.
/// The canvas does not offset them into its own rectangle, so a shape added at `(20, 30)` is
/// painted at `(20, 30)` whether it was added before or after the canvas moved.
pub struct ShapeCanvas {
    base: BaseWidget,
    /// Hosted shapes in paint order: the last entry is painted on top and is hit first.
    shapes: Vec<CanvasShape>,
    /// Selected shape ids, in selection order.
    selection: Vec<ObjectId>,
    /// Whether the pointer may select, drag and marquee-select.
    editable: bool,
    /// Whether drags snap to a grid.
    snap_to_grid: bool,
    /// Grid pitch in pixels; zero disables snapping even when `snap_to_grid` is set.
    grid_step: u32,
    /// Id the next added shape receives.
    next_id: ObjectId,
    /// The drag in progress: the pointer at the press and each selected shape's geometry then.
    drag: Option<(Point, Vec<(ObjectId, Rect)>)>,
    /// The two corners of the rectangle being dragged out, while a marquee is in progress.
    marquee: Option<(Point, Point)>,
    /// Emitted with the new id after a shape is added.
    pub shape_added: Signal1<ObjectId>,
    /// Emitted with the removed id after a shape is removed.
    pub shape_removed: Signal1<ObjectId>,
    /// Emitted with the new selection whenever it changes.
    pub shape_selected: Signal1<Vec<ObjectId>>,
    /// Emitted with a shape's id after its geometry or stacking order changes.
    pub shape_modified: Signal1<ObjectId>,
    /// Bumped when the selection or any hosted geometry changes.
    selection_revision: u64,
    /// Cached selection outline boxes; see [`Self::selected_rects`].
    selection_rects: RefCell<Option<(u64, Vec<Rect>)>>,
}

impl ShapeCanvas {
    /// Creates an empty, non-editable canvas.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ShapeCanvas, geometry, "ShapeCanvas"),
            shapes: Vec::new(),
            selection: Vec::new(),
            editable: false,
            snap_to_grid: false,
            grid_step: 8,
            next_id: 1,
            drag: None,
            marquee: None,
            shape_added: Signal1::new(),
            shape_removed: Signal1::new(),
            shape_selected: Signal1::new(),
            shape_modified: Signal1::new(),
            selection_revision: 1,
            selection_rects: RefCell::new(None),
        }
    }

    /// The hosted shape ids in paint order, bottom first.
    pub fn shapes(&self) -> Vec<ObjectId> {
        self.shapes.iter().map(|shape| shape.id).collect()
    }

    /// How many shapes the canvas hosts.
    pub fn shape_count(&self) -> usize {
        self.shapes.len()
    }

    /// The selected ids, in selection order.
    pub fn selection(&self) -> &[ObjectId] {
        &self.selection
    }

    /// The shape with `id`, for reading its path, colours or geometry.
    pub fn shape(&self, id: ObjectId) -> Option<&FreeformShapeWidget> {
        self.shapes.iter().find(|shape| shape.id == id).map(|shape| &shape.widget)
    }

    /// The shape with `id`, for a host that wants to change its path or colours in place.
    pub fn shape_mut(&mut self, id: ObjectId) -> Option<&mut FreeformShapeWidget> {
        self.shapes.iter_mut().find(|shape| shape.id == id).map(|shape| &mut shape.widget)
    }

    /// Adds `shape` on top and returns the id that names it from now on.
    ///
    /// The returned id is the only handle the caller gets, so it is also emitted through
    /// `shape_added`; a host storing shapes by id can subscribe once and never infer ids from
    /// insertion order.
    pub fn add_shape(&mut self, shape: FreeformShapeWidget) -> ObjectId {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        self.shapes.push(CanvasShape { id, widget: shape });
        self.shape_added.emit(id);
        self.base.request_redraw();
        id
    }

    /// Removes the shape with `id`, dropping it from the selection too.
    pub fn remove_shape(&mut self, id: ObjectId) -> bool {
        let Some(position) = self.shapes.iter().position(|shape| shape.id == id) else {
            return false;
        };
        self.shapes.remove(position);
        self.selection_revision = self.selection_revision.wrapping_add(1);
        if self.selection.contains(&id) {
            self.selection.retain(|selected| *selected != id);
            self.shape_selected.emit(self.selection.clone());
        }
        self.shape_removed.emit(id);
        self.base.request_redraw();
        true
    }

    /// Moves the shape one step up the stacking order, returning whether it moved.
    pub fn raise(&mut self, id: ObjectId) -> bool {
        let Some(position) = self.shapes.iter().position(|shape| shape.id == id) else {
            return false;
        };
        if position + 1 >= self.shapes.len() {
            return false;
        }
        self.shapes.swap(position, position + 1);
        self.shape_modified.emit(id);
        self.base.request_redraw();
        true
    }

    /// Moves the shape one step down the stacking order, returning whether it moved.
    pub fn lower(&mut self, id: ObjectId) -> bool {
        let Some(position) = self.shapes.iter().position(|shape| shape.id == id) else {
            return false;
        };
        if position == 0 {
            return false;
        }
        self.shapes.swap(position, position - 1);
        self.shape_modified.emit(id);
        self.base.request_redraw();
        true
    }

    /// The topmost shape whose outline contains `point`.
    ///
    /// The test is the shape's own [`FreeformShapeWidget::contains`], so a point in a star's
    /// transparent corner falls through to the shape behind it — the same rule the shape itself
    /// uses, not a bounding-box approximation.
    pub fn hit_test(&self, point: Point) -> Option<ObjectId> {
        self.shapes.iter().rev().find(|shape| shape.widget.contains(point)).map(|shape| shape.id)
    }

    /// Selects every shape whose box intersects `rect`, replacing the selection.
    pub fn select_in_rect(&mut self, rect: Rect) -> Vec<ObjectId> {
        let ids: Vec<ObjectId> = self
            .shapes
            .iter()
            .filter(|shape| shape.widget.base().geometry().intersects(&rect))
            .map(|shape| shape.id)
            .collect();
        self.set_selection(ids.clone());
        ids
    }

    /// Replaces the selection.
    ///
    /// Ids that name no hosted shape are dropped rather than stored, so a selection can never
    /// include a shape the canvas cannot paint.
    pub fn set_selection(&mut self, ids: Vec<ObjectId>) {
        let mut next: Vec<ObjectId> = Vec::with_capacity(ids.len());
        for id in ids {
            if self.shapes.iter().any(|shape| shape.id == id) && !next.contains(&id) {
                next.push(id);
            }
        }
        if self.selection == next {
            return;
        }
        self.selection = next;
        self.selection_revision = self.selection_revision.wrapping_add(1);
        self.shape_selected.emit(self.selection.clone());
        self.base.request_redraw();
    }

    /// The selection's outline boxes, rebuilt only when the selection or a hosted geometry moved.
    ///
    /// The paint used to collect these on every frame, and each miss re-walked the selection and
    /// every hosted shape (SP-P57).
    fn selected_rects(&self) -> Ref<'_, Vec<Rect>> {
        let fresh = self
            .selection_rects
            .borrow()
            .as_ref()
            .is_some_and(|(revision, _)| *revision == self.selection_revision);
        if !fresh {
            let boxes: Vec<Rect> = self
                .selection
                .iter()
                .filter_map(|id| self.shape(*id).map(|shape| shape.base().geometry()))
                .collect();
            *self.selection_rects.borrow_mut() = Some((self.selection_revision, boxes));
        }
        Ref::map(self.selection_rects.borrow(), |cache| {
            &cache.as_ref().expect("the selection cache was just populated").1
        })
    }

    /// Empties the selection.
    pub fn clear_selection(&mut self) {
        self.set_selection(Vec::new());
    }

    /// Whether the pointer may select and drag shapes.
    pub fn is_editable(&self) -> bool {
        self.editable
    }

    /// Enables or disables pointer interaction.
    pub fn set_editable(&mut self, editable: bool) {
        if self.editable == editable {
            return;
        }
        self.editable = editable;
        self.drag = None;
        self.marquee = None;
        self.base.request_redraw();
    }

    /// Whether drags snap to the grid.
    pub fn snap_to_grid(&self) -> bool {
        self.snap_to_grid
    }

    /// Enables or disables grid snapping.
    pub fn set_snap_to_grid(&mut self, snap: bool) {
        if self.snap_to_grid == snap {
            return;
        }
        self.snap_to_grid = snap;
        self.base.request_redraw();
    }

    /// The grid pitch in pixels.
    pub fn grid_step(&self) -> u32 {
        self.grid_step
    }

    /// Sets the grid pitch; zero disables snapping while leaving the flag as the caller set it.
    pub fn set_grid_step(&mut self, step: u32) {
        if self.grid_step == step {
            return;
        }
        self.grid_step = step;
        self.base.request_redraw();
    }

    /// Snaps one coordinate to the grid when snapping is active.
    fn snap(&self, value: i32) -> i32 {
        if !self.snap_to_grid || self.grid_step == 0 {
            return value;
        }
        let step = self.grid_step as i32;
        (value as f32 / step as f32).round() as i32 * step
    }

    /// Sets a hosted shape's geometry without recording it in the shape's own undo history.
    ///
    /// A canvas drag is one canvas-level gesture; pushing every intermediate frame into the
    /// shape's history would make one drag look like a hundred undo steps to a host that shares
    /// the shape's stack.
    fn place(&mut self, id: ObjectId, geometry: Rect) {
        if let Some(shape) = self.shapes.iter_mut().find(|shape| shape.id == id) {
            shape.widget.base_mut().set_geometry(geometry);
            self.selection_revision = self.selection_revision.wrapping_add(1);
        }
    }
}

impl Widget for ShapeCanvas {
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
        crate::core::Size::new(320, 240)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves the canvas-level event names; see `DiffViewer`'s note for why the arms are
    /// explicit rather than derived.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "shape_added" => Some(EventSignalRef::mapped("shape_added", &self.shape_added, |v| {
                CapabilityValue::UInt(*v)
            })),
            "shape_removed" => {
                Some(EventSignalRef::mapped("shape_removed", &self.shape_removed, |v| {
                    CapabilityValue::UInt(*v)
                }))
            }
            "shape_selected" => {
                Some(EventSignalRef::mapped("shape_selected", &self.shape_selected, |v| {
                    CapabilityValue::Tuple(v.iter().map(|id| CapabilityValue::UInt(*id)).collect())
                }))
            }
            "shape_modified" => {
                Some(EventSignalRef::mapped("shape_modified", &self.shape_modified, |v| {
                    CapabilityValue::UInt(*v)
                }))
            }
            _ => None,
        }
    }
}

impl WidgetProperties for ShapeCanvas {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "shapes" => Ok(CapabilityValue::Tuple(
                self.shapes().into_iter().map(CapabilityValue::UInt).collect(),
            )),
            "shape_count" => Ok(CapabilityValue::UInt(self.shape_count() as u64)),
            "selection" => Ok(CapabilityValue::Tuple(
                self.selection.iter().map(|id| CapabilityValue::UInt(*id)).collect(),
            )),
            "editable" => Ok(CapabilityValue::Bool(self.editable)),
            "snap_to_grid" => Ok(CapabilityValue::Bool(self.snap_to_grid)),
            "grid_step" => Ok(CapabilityValue::UInt(self.grid_step as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            // The id list is a derived view of the hosted controls; add/remove are the writers.
            "shapes" | "shape_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            "selection" => match value {
                CapabilityValue::Null => {
                    self.clear_selection();
                    Ok(())
                }
                CapabilityValue::Tuple(items) => {
                    let mut ids = Vec::with_capacity(items.len());
                    for item in items {
                        let id = match item {
                            CapabilityValue::UInt(id) => id,
                            _ => return Err(CapabilityAccessError::TypeMismatch),
                        };
                        ids.push(id);
                    }
                    self.set_selection(ids);
                    Ok(())
                }
                _ => Err(CapabilityAccessError::TypeMismatch),
            },
            "editable" => {
                self.set_editable(expect_bool(value)?);
                Ok(())
            }
            "snap_to_grid" => {
                self.set_snap_to_grid(expect_bool(value)?);
                Ok(())
            }
            "grid_step" => {
                self.set_grid_step(expect_u32(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "shapes",
            "shape_count",
            "selection",
            "editable",
            "snap_to_grid",
            "grid_step",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl EventHandler for ShapeCanvas {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button: 1, modifiers } if self.editable => {
                let additive = crate::shortcut::Modifiers::from_event_bits(*modifiers)
                    .contains(crate::shortcut::Modifiers::PRIMARY);
                match self.hit_test(*pos) {
                    Some(id) => {
                        if additive {
                            let mut next = self.selection.clone();
                            if let Some(position) = next.iter().position(|selected| *selected == id)
                            {
                                next.remove(position);
                            } else {
                                next.push(id);
                            }
                            self.set_selection(next);
                        } else if !self.selection.contains(&id) {
                            self.set_selection(vec![id]);
                        }
                        let starts: Vec<(ObjectId, Rect)> = self
                            .selection
                            .iter()
                            .filter_map(|selected| {
                                self.shape(*selected)
                                    .map(|shape| (*selected, shape.base().geometry()))
                            })
                            .collect();
                        self.drag = Some((*pos, starts));
                    }
                    None => {
                        if !additive {
                            self.clear_selection();
                        }
                        self.marquee = Some((*pos, *pos));
                    }
                }
            }
            Event::MouseMove { pos } => {
                if let Some((origin, starts)) = self.drag.clone() {
                    let dx = self.snap(pos.x) - self.snap(origin.x);
                    let dy = self.snap(pos.y) - self.snap(origin.y);
                    let mut changed = Vec::with_capacity(starts.len());
                    for (id, geometry) in starts {
                        let next = Rect::new(
                            geometry.x + dx,
                            geometry.y + dy,
                            geometry.width,
                            geometry.height,
                        );
                        if next != geometry {
                            self.place(id, next);
                            changed.push(id);
                        }
                    }
                    for id in changed {
                        self.shape_modified.emit(id);
                    }
                    self.base.request_redraw();
                } else if let Some((origin, _)) = self.marquee {
                    self.marquee = Some((origin, *pos));
                    self.base.request_redraw();
                }
            }
            Event::MouseRelease { button: 1, .. } => {
                self.drag = None;
                if let Some((start, end)) = self.marquee.take() {
                    let rect = Rect::from_points(start, end);
                    self.select_in_rect(rect);
                }
                self.base.request_redraw();
            }
            _ => {}
        }
    }
}

impl Draw for ShapeCanvas {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.base.geometry();
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("freeform_shape");
        let background = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| crate::core::Color::border_of(background));
        let accent = crate::style::palette_shared()
            .as_ref()
            .map(|colors| colors.primary)
            .unwrap_or(Color::PRIMARY);

        context.face_with_gradient(
            rect,
            background,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().border_radius.unwrap_or(0),
            Color::BLACK,
        );
        crate::widget::draw::stroke_surface_border(
            context,
            rect,
            self.style().border_radius.unwrap_or(0),
            border,
            1,
        );

        // One palette for every hosted shape: the per-shape theme resolution was `2n` manager
        // locks per frame (SP-P57).
        let palette = ShapePalette { background, text_color: ink };

        // The grid is drawn from the same `grid_step` a drag snaps to, so a shape can be placed
        // exactly on a line the reader can see. A zero step (or a disabled flag) paints nothing.
        if self.snap_to_grid && self.grid_step > 0 {
            let step = self.grid_step as i32;
            let grid = background.blend(&ink, 0.08);
            let mut x = rect.x;
            while x <= rect.x + rect.width as i32 {
                context.draw_line(
                    Point::new(x, rect.y),
                    Point::new(x, rect.y + rect.height as i32),
                    grid,
                );
                x += step;
            }
            let mut y = rect.y;
            while y <= rect.y + rect.height as i32 {
                context.draw_line(
                    Point::new(rect.x, y),
                    Point::new(rect.x + rect.width as i32, y),
                    grid,
                );
                y += step;
            }
        }

        for shape in &mut self.shapes {
            shape.widget.draw_with_palette(context, &palette);
        }

        // Selection outlines are painted after every shape so a selected shape underneath a
        // later one still shows where it is. The boxes come from the cache, so a still frame does
        // not re-walk the selection (SP-P57).
        let selected = self.selected_rects();
        for box_rect in selected.iter().copied() {
            context.draw_rect(box_rect, accent);
            context.draw_rect(
                Rect::new(box_rect.x - 1, box_rect.y - 1, box_rect.width + 2, box_rect.height + 2),
                accent.with_alpha(90),
            );
        }
        drop(selected);

        if let Some((start, end)) = self.marquee {
            let marquee = Rect::from_points(start, end);
            context.draw_rect(marquee, accent);
        }
    }
}

#[cfg(test)]
mod shape_canvas_tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn box_shape(x: i32, y: i32, side: u32) -> FreeformShapeWidget {
        FreeformShapeWidget::new(Rect::new(x, y, side, side), ShapePath::RoundedRect { radius: 4 })
    }

    #[test]
    fn adding_a_shape_returns_an_id_and_emits_it() {
        let mut canvas = ShapeCanvas::new(Rect::new(0, 0, 400, 300));
        let added = Arc::new(Mutex::new(Vec::new()));
        let sink = added.clone();
        canvas.shape_added.connect(move |id| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*id);
            }
        });

        let first = canvas.add_shape(box_shape(10, 10, 40));
        let second = canvas.add_shape(box_shape(60, 60, 40));

        assert_ne!(first, second);
        assert_eq!(canvas.shapes(), vec![first, second]);
        assert_eq!(*added.lock().expect("added lock"), vec![first, second]);
    }

    #[test]
    fn hit_test_picks_the_topmost_outline_not_the_box() {
        let mut canvas = ShapeCanvas::new(Rect::new(0, 0, 400, 300));
        let lower = canvas.add_shape(box_shape(0, 0, 100));
        let upper = canvas.add_shape(box_shape(0, 0, 100));

        assert_eq!(canvas.hit_test(Point::new(50, 50)), Some(upper));
        assert!(canvas.raise(lower));
        assert_eq!(canvas.hit_test(Point::new(50, 50)), Some(lower));
        assert!(canvas.lower(lower));
        assert_eq!(canvas.hit_test(Point::new(50, 50)), Some(upper));
        assert_eq!(canvas.hit_test(Point::new(350, 250)), None);
    }

    #[test]
    fn rect_selection_returns_every_touched_shape() {
        let mut canvas = ShapeCanvas::new(Rect::new(0, 0, 400, 300));
        let first = canvas.add_shape(box_shape(10, 10, 40));
        let second = canvas.add_shape(box_shape(200, 10, 40));
        let selected = canvas.select_in_rect(Rect::new(0, 0, 100, 100));
        assert_eq!(selected, vec![first]);
        assert_eq!(canvas.selection(), &[first]);

        let both = canvas.select_in_rect(Rect::new(0, 0, 400, 300));
        assert_eq!(both, vec![first, second]);
    }

    #[test]
    fn dragging_a_selected_shape_moves_it_on_the_grid_and_reports_it() {
        let mut canvas = ShapeCanvas::new(Rect::new(0, 0, 400, 300));
        canvas.set_editable(true);
        canvas.set_snap_to_grid(true);
        canvas.set_grid_step(10);
        let id = canvas.add_shape(box_shape(10, 10, 40));
        let modified = Arc::new(Mutex::new(Vec::new()));
        let sink = modified.clone();
        canvas.shape_modified.connect(move |moved| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*moved);
            }
        });

        canvas.handle_event(&Event::mouse_press(30, 30, 1));
        assert_eq!(canvas.selection(), &[id]);
        canvas.handle_event(&Event::mouse_move(47, 36));
        canvas.handle_event(&Event::mouse_release(47, 36, 1));

        let geometry = canvas.shape(id).expect("shape").base().geometry();
        assert_eq!((geometry.x, geometry.y), (30, 20));
        assert_eq!(*modified.lock().expect("modified lock"), vec![id]);
    }

    #[test]
    fn removing_a_shape_drops_it_from_the_selection() {
        let mut canvas = ShapeCanvas::new(Rect::new(0, 0, 400, 300));
        let id = canvas.add_shape(box_shape(0, 0, 50));
        canvas.set_selection(vec![id]);
        assert!(canvas.remove_shape(id));
        assert!(canvas.selection().is_empty());
        assert!(!canvas.remove_shape(id));
    }

    #[test]
    fn properties_describe_the_hosted_shapes() {
        let mut canvas = ShapeCanvas::new(Rect::new(0, 0, 400, 300));
        let id = canvas.add_shape(box_shape(0, 0, 50));
        canvas.set_editable(true);
        canvas.set_snap_to_grid(true);
        canvas.set_grid_step(12);
        canvas.set_selection(vec![id]);

        assert_eq!(canvas.get("shape_count"), Ok(CapabilityValue::UInt(1)));
        assert_eq!(canvas.get("editable"), Ok(CapabilityValue::Bool(true)));
        assert_eq!(canvas.get("snap_to_grid"), Ok(CapabilityValue::Bool(true)));
        assert_eq!(canvas.get("grid_step"), Ok(CapabilityValue::UInt(12)));
        assert_eq!(
            canvas.get("selection"),
            Ok(CapabilityValue::Tuple(vec![CapabilityValue::UInt(id)]))
        );
        assert_eq!(
            canvas.set("shape_count", CapabilityValue::UInt(3)),
            Err(CapabilityAccessError::ReadOnlyProperty)
        );
    }

    /// The container reports selection and removal itself: a host connects to the canvas, not to
    /// each shape it added.
    #[test]
    fn the_canvas_reports_selection_changes_and_removal() {
        let mut canvas = ShapeCanvas::new(Rect::new(0, 0, 400, 300));
        let selected = Arc::new(Mutex::new(Vec::new()));
        let sink = selected.clone();
        canvas.shape_selected.connect(move |ids| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(ids.as_ref().clone());
            }
        });

        let first = canvas.add_shape(box_shape(0, 0, 50));
        let second = canvas.add_shape(box_shape(80, 0, 50));
        let touched = canvas.select_in_rect(Rect::new(0, 0, 200, 60));
        assert_eq!(touched, vec![first, second]);
        assert_eq!(
            *selected.lock().expect("selected lock"),
            vec![vec![first, second]],
            "the container emits the new selection once"
        );

        assert!(canvas.remove_shape(first));
        assert_eq!(
            *selected.lock().expect("selected lock"),
            vec![vec![first, second], vec![second]],
            "removing a selected shape reports the smaller selection"
        );
    }
}
