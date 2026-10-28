// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! BezierCurveEditor widget — interactive cubic bezier curve editor for creating custom easing curves.
//!
//! This widget provides a visual editor for cubic bezier curves with two control points,
//! a grid background, snap-to-grid support, and drag interaction. It can be used to design
//! custom easing functions for animations.

/// A 2-D control point.
type Point2 = (f32, f32);

use crate::compat::{vec, Box, Rc, RefCell, Vec};
use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::undo::{UndoStack, ValueCommand};
use crate::widget::capability::coercion::expect_bool;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Control point handle radius in pixels.
const HANDLE_RADIUS: u32 = 8;
/// Weight, in `0.0..=1.0`, the grid is blended toward the text colour over the
/// surface. Previously an `u8` alpha (60/255 ≈ 0.24) applied to a fixed grey;
/// expressing it as a blend weight lets the grid follow the resolved surface.
const GRID_ALPHA: f32 = 0.24;

/// How many cubic steps one segment's painted polyline is sampled at.
const CURVE_SAMPLES_PER_SEGMENT: usize = 50;

/// One cubic segment of the editor's piecewise curve.
///
/// `t0..t1` is the segment's span in the curve's **global** parameter, so splitting a segment
/// preserves the original parameterisation: `sample_at(t)` answers the same point before and
/// after a split. `split_fraction` records where this segment was cut from its left neighbour,
/// which is exactly the parameter `merge_cubics` needs to invert the split.
#[derive(Debug, Clone, Copy, PartialEq)]
struct CubicSegment {
    p0: (f32, f32),
    p1: (f32, f32),
    p2: (f32, f32),
    p3: (f32, f32),
    t0: f32,
    t1: f32,
    split_fraction: f32,
}

impl CubicSegment {
    /// The editor's historical ease-in-out curve: one segment from `(0,0)` to `(1,1)`.
    fn default_ease() -> Self {
        Self {
            p0: (0.0, 0.0),
            p1: (0.25, 0.1),
            p2: (0.75, 0.9),
            p3: (1.0, 1.0),
            t0: 0.0,
            t1: 1.0,
            split_fraction: 0.0,
        }
    }
}

/// A cubic Bézier split, as the two halves de Casteljau produces.
fn split_cubic(
    p0: Point2,
    p1: Point2,
    p2: Point2,
    p3: Point2,
    t: f32,
) -> (Point2, Point2, Point2, Point2, Point2, Point2) {
    let lerp =
        |a: (f32, f32), b: (f32, f32), t: f32| (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
    let q0 = lerp(p0, p1, t);
    let q1 = lerp(p1, p2, t);
    let q2 = lerp(p2, p3, t);
    let r0 = lerp(q0, q1, t);
    let r1 = lerp(q1, q2, t);
    let s = lerp(r0, r1, t);
    (q0, r0, s, r1, q2, s)
}

/// Inverts [`split_cubic`] at the same fraction.
///
/// For a pair produced by splitting a cubic at `t`, this returns the original control points
/// exactly: `p1` is recovered from the left half's first de Casteljau point and `p2` from the
/// right half's last one. Applied to an arbitrary adjacent pair it still produces a cubic whose
/// endpoints are the pair's endpoints, which is the only well-defined general merge.
fn merge_cubics(
    p0: Point2,
    l1: Point2,
    r2: Point2,
    p3: Point2,
    t: f32,
) -> (Point2, Point2, Point2, Point2) {
    if t <= 0.0 || t >= 1.0 {
        return (p0, l1, r2, p3);
    }
    let p1 = ((l1.0 - (1.0 - t) * p0.0) / t, (l1.1 - (1.0 - t) * p0.1) / t);
    let p2 = ((r2.0 - t * p3.0) / (1.0 - t), (r2.1 - t * p3.1) / (1.0 - t));
    (p0, p1, p2, p3)
}

/// BezierCurveEditor widget for creating custom easing curves.
pub struct BezierCurveEditor {
    base: BaseWidget,
    /// The piecewise cubic the editor paints, in global-parameter order.
    ///
    /// One segment is the historical shape. `add_point` splits a segment with de Casteljau and
    /// `remove_point` merges the two neighbours back, so the curve has a real point count rather
    /// than two fixed handles.
    segments: Vec<CubicSegment>,
    /// Grid cell size in pixels.
    grid_size: f32,
    /// Whether to draw the background grid.
    show_grid: bool,
    /// Whether control points snap to the grid.
    snap_to_grid: bool,
    /// Which handle is being dragged (None = not dragging).
    dragging: Option<DragTarget>,
    /// Which handle the pointer is currently over (None = between/outside the handles).
    ///
    /// Recorded by `MouseMove` through the same `hit_test_handle` the press uses, so the
    /// handle that lights up is the handle a press would grab. `BaseWidget::is_hovered()`
    /// answers whether the *pointer is over the control*; only this answers which handle
    /// it is over, and the two are not the same fact.
    hovered_handle: Option<DragTarget>,
    /// Whether the two handles are kept mirrored through the curve's midpoint.
    symmetric_handles: bool,
    /// The two control-point readouts, formatted only when the points move (K-P11).
    ///
    /// The draw path rebuilt both `format!` strings every frame — two allocations per paint for a
    /// pair of numbers that change only while a handle is being dragged.
    cp1_label: String,
    cp2_label: String,
    /// The point pair [`Self::cp1_label`] / [`Self::cp2_label`] were formatted from.
    label_points: Option<((f32, f32), (f32, f32))>,
    /// The curve's sampled pixel polyline, flattened end to end.
    ///
    /// A sample is a pure function of the geometry and the control points, so an idle frame
    /// reuses it instead of evaluating the cubic 50 times per segment (ADV-011). The ranges
    /// below say where each segment lives inside this buffer.
    curve_samples: Vec<Point>,
    /// One `(start, end)` range per segment inside [`Self::curve_samples`].
    curve_sample_ranges: Vec<(usize, usize)>,
    /// The geometry and segments [`Self::curve_samples`] was built from; `None` before the
    /// first sample.
    curve_sample_inputs: Option<(Rect, Vec<CubicSegment>)>,
    /// The undo history of whole control-point sets (M-004).
    undo_stack: UndoStack,
    /// The point set the history commands write to.
    ///
    /// An [`UndoCommand`](crate::undo::UndoCommand) cannot reach a field of this widget, so the
    /// editor keeps the authoritative set in a shared cell the `ValueCommand` can restore — the
    /// same `Rc<RefCell<T>>` pattern the date and key-sequence editors use. `segments` stays the
    /// readable field every drawing path uses; `restore_history_segments` copies the cell back
    /// into it after an undo or redo.
    history_target: Rc<RefCell<Vec<CubicSegment>>>,
    /// Whether the current change came from a history restore and must not be recorded again.
    restoring_history: bool,
    /// The segments at the start of the active drag, so one gesture is one undo step.
    drag_origin: Option<Vec<CubicSegment>>,
    /// Emitted when the curve changes. Passes the first segment's two control handles.
    ///
    /// The payload is kept as the historical pair so the published event schema is unchanged;
    /// a multi-segment curve can carry more handles than this event can name, and
    /// `Self::segments` is the authoritative reader for those.
    pub curve_changed: Signal1<((f32, f32), (f32, f32))>,
}

/// Identifies which control point is being dragged.
///
/// The segment index is part of the identity: a multi-segment curve draws a handle pair on every
/// segment, so "control point 1" alone names one of N handles and the press could only ever grab
/// the first segment's. The payload order is `(segment, handle)` through the variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DragTarget {
    ControlPoint1(usize),
    ControlPoint2(usize),
}

impl DragTarget {
    /// The segment this handle belongs to.
    fn segment(self) -> usize {
        match self {
            DragTarget::ControlPoint1(index) | DragTarget::ControlPoint2(index) => index,
        }
    }
}

impl BezierCurveEditor {
    /// Creates a new BezierCurveEditor with the given geometry.
    /// Default control points form a standard ease-in-out curve.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::BezierCurveEditor, geometry, "BezierCurveEditor"),
            segments: vec![CubicSegment::default_ease()],
            grid_size: 0.2,
            show_grid: true,
            snap_to_grid: false,
            dragging: None,
            hovered_handle: None,
            symmetric_handles: false,
            cp1_label: String::new(),
            cp2_label: String::new(),
            label_points: None,
            curve_samples: Vec::new(),
            curve_sample_ranges: Vec::new(),
            curve_sample_inputs: None,
            undo_stack: UndoStack::new(),
            history_target: Rc::new(RefCell::new(vec![CubicSegment::default_ease()])),
            restoring_history: false,
            drag_origin: None,
            curve_changed: Signal1::new(),
        }
    }

    /// The first segment, which is the one the two historical handles address.
    fn first_segment(&self) -> CubicSegment {
        self.segments.first().copied().unwrap_or_else(CubicSegment::default_ease)
    }

    /// The first segment's two control handles, the historical "the curve's handles" pair.
    fn current_handles(&self) -> ((f32, f32), (f32, f32)) {
        let segment = self.first_segment();
        (segment.p1, segment.p2)
    }

    /// Returns the first control point as a fraction (0.0–1.0).
    pub fn control_point1(&self) -> (f32, f32) {
        self.first_segment().p1
    }

    /// Sets the first control point with values clamped to 0.0–1.0.
    ///
    /// While [`Self::symmetric_handles`] is set, the second point is mirrored through the
    /// curve's midpoint, so the curve keeps its S shape instead of bending towards one side.
    pub fn set_control_point1(&mut self, x: f32, y: f32) {
        let old = self.current_handles();
        let next = (x.clamp(0.0, 1.0), y.clamp(0.0, 1.0));
        if let Some(segment) = self.segments.first_mut() {
            segment.p1 = next;
            if self.symmetric_handles {
                segment.p2 = (1.0 - next.0, 1.0 - next.1);
            }
        }
        if self.current_handles() != old {
            self.emit_curve_changed();
            self.base.request_redraw();
        }
    }

    /// Returns the second control point as a fraction (0.0–1.0).
    pub fn control_point2(&self) -> (f32, f32) {
        self.first_segment().p2
    }

    /// Sets the second control point with values clamped to 0.0–1.0.
    ///
    /// While [`Self::symmetric_handles`] is set, the first point is mirrored through the
    /// curve's midpoint.
    pub fn set_control_point2(&mut self, x: f32, y: f32) {
        let old = self.current_handles();
        let next = (x.clamp(0.0, 1.0), y.clamp(0.0, 1.0));
        if let Some(segment) = self.segments.first_mut() {
            segment.p2 = next;
            if self.symmetric_handles {
                segment.p1 = (1.0 - next.0, 1.0 - next.1);
            }
        }
        if self.current_handles() != old {
            self.emit_curve_changed();
            self.base.request_redraw();
        }
    }

    /// The number of cubic segments the curve is made of.
    pub fn segment_count(&self) -> usize {
        self.segments.len()
    }

    /// The curve's control points as a token string.
    ///
    /// # Format
    ///
    /// One group per segment, groups separated by `|`, the four points of a segment separated by
    /// `;`, and each point's two coordinates separated by `,` — so the historical single-segment
    /// ease is `"0,0;0.25,0.1;0.75,0.9;1,1"`. This is the shape a document can store and a tool can
    /// diff; it is deliberately not a `Debug` dump, which would carry the parameterisation too.
    pub fn curve_points_token(&self) -> String {
        let mut out = String::new();
        for (index, segment) in self.segments.iter().enumerate() {
            if index > 0 {
                out.push('|');
            }
            for (point_index, point) in
                [segment.p0, segment.p1, segment.p2, segment.p3].iter().enumerate()
            {
                if point_index > 0 {
                    out.push(';');
                }
                let _ = core::fmt::Write::write_fmt(
                    &mut out,
                    format_args!("{:.4},{:.4}", point.0, point.1),
                );
            }
        }
        out
    }

    /// Replaces the curve from a [`Self::curve_points_token`] string.
    ///
    /// Every group must name exactly four points; a malformed token is refused as
    /// [`CapabilityAccessError::TypeMismatch`] and leaves the curve untouched, so a bad write can
    /// never leave a half-replaced curve. Segment parameters are redistributed evenly over the new
    /// segment count, which is the parameterisation a freshly built piecewise curve has.
    fn set_curve_points_token(&mut self, token: &str) -> Result<(), CapabilityAccessError> {
        let mut segments: Vec<CubicSegment> = Vec::new();
        for group in token.split('|') {
            let group = group.trim();
            if group.is_empty() {
                return Err(CapabilityAccessError::TypeMismatch);
            }
            let mut points: Vec<(f32, f32)> = Vec::new();
            for pair in group.split(';') {
                let (x, y) = pair.split_once(',').ok_or(CapabilityAccessError::TypeMismatch)?;
                let x: f32 = x.trim().parse().map_err(|_| CapabilityAccessError::TypeMismatch)?;
                let y: f32 = y.trim().parse().map_err(|_| CapabilityAccessError::TypeMismatch)?;
                if !x.is_finite() || !y.is_finite() {
                    return Err(CapabilityAccessError::TypeMismatch);
                }
                points.push((x.clamp(0.0, 1.0), y.clamp(0.0, 1.0)));
            }
            if points.len() != 4 {
                return Err(CapabilityAccessError::TypeMismatch);
            }
            segments.push(CubicSegment {
                p0: points[0],
                p1: points[1],
                p2: points[2],
                p3: points[3],
                t0: 0.0,
                t1: 1.0,
                split_fraction: 0.0,
            });
        }
        if segments.is_empty() {
            return Err(CapabilityAccessError::TypeMismatch);
        }
        let count = segments.len() as f32;
        for (index, segment) in segments.iter_mut().enumerate() {
            segment.t0 = index as f32 / count;
            segment.t1 = (index + 1) as f32 / count;
        }
        if self.segments == segments {
            return Ok(());
        }
        self.segments = segments;
        self.emit_curve_changed();
        self.base.request_redraw();
        Ok(())
    }

    /// The number of on-curve points: one more than the segment count.
    pub fn point_count(&self) -> usize {
        self.segments.len() + 1
    }

    /// The four control points of `index`'s segment, or `None` when it does not exist.
    pub fn segment_control_points(&self, index: usize) -> Option<[(f32, f32); 4]> {
        self.segments.get(index).map(|segment| [segment.p0, segment.p1, segment.p2, segment.p3])
    }

    /// Resets the curve to the editor's default single-segment ease-in-out shape.
    ///
    /// Emits `curve_changed` once when the shape actually changes.
    pub fn reset(&mut self) {
        let old_handles = self.current_handles();
        let old_len = self.segments.len();
        self.segments.clear();
        self.segments.push(CubicSegment::default_ease());
        if old_len != 1 || old_handles != self.current_handles() {
            self.emit_curve_changed();
            self.base.request_redraw();
        }
    }

    /// Replaces the curve with one cubic segment carrying these two control handles.
    ///
    /// A single cubic is defined by exactly two control points, so any other length is
    /// refused with [`CapabilityAccessError::OutOfRange`] rather than silently truncated.
    /// This also collapses a multi-segment curve back to that one segment; use
    /// [`Self::add_point`] to split it again.
    pub fn set_points(&mut self, points: &[(f32, f32)]) -> Result<(), CapabilityAccessError> {
        if points.len() != 2 {
            return Err(CapabilityAccessError::OutOfRange);
        }
        let old_handles = self.current_handles();
        let old_len = self.segments.len();
        self.segments.clear();
        self.segments.push(CubicSegment {
            p0: (0.0, 0.0),
            p1: (points[0].0.clamp(0.0, 1.0), points[0].1.clamp(0.0, 1.0)),
            p2: (points[1].0.clamp(0.0, 1.0), points[1].1.clamp(0.0, 1.0)),
            p3: (1.0, 1.0),
            t0: 0.0,
            t1: 1.0,
            split_fraction: 0.0,
        });
        if old_len != 1 || old_handles != self.current_handles() {
            self.emit_curve_changed();
            self.base.request_redraw();
        }
        Ok(())
    }

    /// Adds an on-curve point at global parameter `t` by splitting the containing cubic.
    ///
    /// De Casteljau splits one cubic into two that together trace exactly the original, so the
    /// painted curve is unchanged while [`Self::point_count`] grows by one. `t` must be strictly
    /// inside `0.0..1.0` and strictly inside one segment (not on an existing join); anything else
    /// returns `false` without changing the curve.
    pub fn add_point(&mut self, t: f32) -> bool {
        let t = t.clamp(0.0, 1.0);
        if t <= 0.0 || t >= 1.0 {
            return false;
        }
        let Some(index) = self.segments.iter().position(|segment| t > segment.t0 && t < segment.t1)
        else {
            return false;
        };
        let segment = self.segments[index];
        let span = segment.t1 - segment.t0;
        if span <= 0.0 {
            return false;
        }
        let u = (t - segment.t0) / span;
        if !(u > 0.0 && u < 1.0) {
            return false;
        }
        let (q0, r0, join, r1, q2, _) =
            split_cubic(segment.p0, segment.p1, segment.p2, segment.p3, u);
        let before = self.segments.clone();
        let left = CubicSegment {
            p0: segment.p0,
            p1: q0,
            p2: r0,
            p3: join,
            t0: segment.t0,
            t1: t,
            split_fraction: segment.split_fraction,
        };
        let right = CubicSegment {
            p0: join,
            p1: r1,
            p2: q2,
            p3: segment.p3,
            t0: t,
            t1: segment.t1,
            split_fraction: u,
        };
        self.segments[index] = left;
        self.segments.insert(index + 1, right);
        self.record_history(before);
        self.emit_curve_changed();
        self.base.request_redraw();
        true
    }

    /// Removes the interior on-curve point at `point`, merging its two neighbouring segments.
    ///
    /// Point `0` and the last point are the curve's endpoints and are not removable. The merge
    /// is the exact inverse of [`Self::add_point`] at the recorded split fraction, so
    /// split-then-remove restores the original cubic; an adjacent pair that was not produced by
    /// a split still merges into the cubic with the same endpoints.
    pub fn remove_point(&mut self, point: usize) -> bool {
        if point == 0 || point >= self.segments.len() {
            return false;
        }
        let left = self.segments[point - 1];
        let right = self.segments[point];
        let t = right.split_fraction;
        if !(t > 0.0 && t < 1.0) {
            return false;
        }
        let (p0, p1, p2, p3) = merge_cubics(left.p0, left.p1, right.p2, right.p3, t);
        let before = self.segments.clone();
        self.segments[point - 1] = CubicSegment {
            p0,
            p1,
            p2,
            p3,
            t0: left.t0,
            t1: right.t1,
            split_fraction: left.split_fraction,
        };
        self.segments.remove(point);
        self.record_history(before);
        self.emit_curve_changed();
        self.base.request_redraw();
        true
    }

    /// Enables or disables mirrored control handles.
    ///
    /// Symmetry is opt-in: an editor for an asymmetric easing curve must be able to place
    /// the two handles independently, which is why the default leaves them free.
    pub fn set_symmetric_handles(&mut self, symmetric: bool) {
        if self.symmetric_handles == symmetric {
            return;
        }
        self.symmetric_handles = symmetric;
        if symmetric {
            let p1 = self.control_point1();
            if let Some(segment) = self.segments.first_mut() {
                segment.p2 = (1.0 - p1.0, 1.0 - p1.1);
            }
            self.emit_curve_changed();
        }
        self.base.request_redraw();
    }

    /// Returns whether the handles are kept mirrored.
    pub fn symmetric_handles(&self) -> bool {
        self.symmetric_handles
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
        let size = size.clamp(0.05, 0.5);
        if self.grid_size == size {
            return;
        }
        self.grid_size = size;
        self.base.request_redraw();
    }

    /// Returns the grid size.
    pub fn grid_size(&self) -> f32 {
        self.grid_size
    }

    /// Undoes the most recent control-point change (drag commit, add or remove).
    ///
    /// Returns `false` when the history is empty. The restored set is exact: the command stores
    /// the whole `segments` vector, parameterisation included, so a split undone by this returns
    /// to the cubic it came from rather than to a re-merged approximation.
    pub fn undo(&mut self) -> bool {
        if self.undo_stack.undo().is_err() {
            return false;
        }
        self.restore_history_segments();
        true
    }

    /// Redoes the most recently undone control-point change.
    pub fn redo(&mut self) -> bool {
        if self.undo_stack.redo().is_err() {
            return false;
        }
        self.restore_history_segments();
        true
    }

    /// Whether there is a control-point change to undo.
    pub fn can_undo(&self) -> bool {
        self.undo_stack.can_undo()
    }

    /// Whether there is an undone control-point change to redo.
    pub fn can_redo(&self) -> bool {
        self.undo_stack.can_redo()
    }

    /// Records a completed change to the control-point set (M-004).
    ///
    /// The before set is mirrored into [`Self::history_target`] first, because undo/redo reach the
    /// value only through that shared cell. A change that moved nothing — a press with no motion,
    /// or a rejected split — records no entry.
    fn record_history(&mut self, before: Vec<CubicSegment>) {
        if self.restoring_history || before == self.segments {
            return;
        }
        let after = self.segments.clone();
        *self.history_target.borrow_mut() = before.clone();
        self.undo_stack.push(Box::new(ValueCommand::new(
            self.history_target.clone(),
            before,
            after,
            "Edit bezier curve",
            "bezier_curve_editor",
        )));
    }

    /// Copies the history target back into the painted point set after an undo or redo.
    fn restore_history_segments(&mut self) {
        self.restoring_history = true;
        self.segments = self.history_target.borrow().clone();
        self.restoring_history = false;
        // A drag that was in flight started from a point set the restore has just replaced, so
        // its release must not record a stale `before`.
        self.drag_origin = None;
        self.emit_curve_changed();
        self.base.request_redraw();
    }

    /// Records the drag that has just ended as one undo step.
    fn commit_drag(&mut self) {
        if let Some(before) = self.drag_origin.take() {
            self.record_history(before);
        }
    }

    /// Evaluates the cubic bezier curve at parameter t (0.0–1.0).
    /// Returns the (x, y) value on the curve at that parameter.
    pub fn sample_at(&self, t: f32) -> (f32, f32) {
        let t = t.clamp(0.0, 1.0);
        let Some(segment) = self
            .segments
            .iter()
            .find(|segment| t >= segment.t0 && t <= segment.t1)
            .or_else(|| self.segments.last())
        else {
            return (0.0, 0.0);
        };
        let span = segment.t1 - segment.t0;
        let local = if span > 0.0 { ((t - segment.t0) / span).clamp(0.0, 1.0) } else { 0.0 };
        Self::cubic_bezier(local, segment.p0, segment.p1, segment.p2, segment.p3)
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

    /// Rebuilds the sampled curve when the geometry or a control point moved (ADV-011).
    ///
    /// Called at the top of `draw`, before it borrows `self` through the style, so the rest of
    /// the frame reads the cached polyline instead of recomputing the cubic.
    fn refresh_curve_samples(&mut self, rect: Rect) {
        let unchanged = self.curve_sample_inputs.as_ref().is_some_and(|(cached_rect, segments)| {
            *cached_rect == rect && *segments == self.segments
        });
        if unchanged {
            return;
        }
        self.curve_samples.clear();
        self.curve_sample_ranges.clear();
        for index in 0..self.segments.len() {
            let segment = self.segments[index];
            let start = self.curve_samples.len();
            self.curve_samples.push(self.curve_to_pixel(segment.p0.0, segment.p0.1));
            for step in 1..=CURVE_SAMPLES_PER_SEGMENT {
                let u = step as f32 / CURVE_SAMPLES_PER_SEGMENT as f32;
                let (x, y) = Self::cubic_bezier(u, segment.p0, segment.p1, segment.p2, segment.p3);
                self.curve_samples.push(self.curve_to_pixel(x, y));
            }
            self.curve_sample_ranges.push((start, self.curve_samples.len()));
        }
        self.curve_sample_inputs = Some((rect, self.segments.clone()));
    }

    /// Emits the curve_changed signal with current control points.
    fn emit_curve_changed(&self) {
        self.curve_changed.emit(self.current_handles());
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
    ///
    /// Every segment's two handles are candidates, and the nearest one within the threshold wins.
    /// The single-segment form of this only ever answered for the first segment, so a curve with
    /// more points drew handles that could not be grabbed.
    fn hit_test_handle(&self, pos: Point) -> Option<DragTarget> {
        let threshold = (HANDLE_RADIUS + 4) as f64;
        let mut best: Option<(f64, DragTarget)> = None;
        for (index, segment) in self.segments.iter().enumerate() {
            for (distance, target) in [
                (self.distance_to_handle(pos, segment.p1), DragTarget::ControlPoint1(index)),
                (self.distance_to_handle(pos, segment.p2), DragTarget::ControlPoint2(index)),
            ] {
                if distance >= threshold {
                    continue;
                }
                if best.is_none_or(|(best_distance, _)| distance < best_distance) {
                    best = Some((distance, target));
                }
            }
        }
        best.map(|(_, target)| target)
    }

    /// Moves one segment's handle, applying the symmetry rule and announcing the change.
    ///
    /// The historical setters only address the first segment; a dragged handle belongs to
    /// whichever segment it was drawn on, so the move goes through here. `curve_changed` keeps its
    /// published payload (the first segment's pair), and the authoritative reader for the rest is
    /// [`Self::segments`].
    fn set_segment_handle(&mut self, target: DragTarget, x: f32, y: f32) {
        let next = (x.clamp(0.0, 1.0), y.clamp(0.0, 1.0));
        let index = target.segment();
        let changed;
        {
            let Some(segment) = self.segments.get_mut(index) else {
                return;
            };
            match target {
                DragTarget::ControlPoint1(_) => {
                    changed = segment.p1 != next;
                    segment.p1 = next;
                    if self.symmetric_handles {
                        segment.p2 = (1.0 - next.0, 1.0 - next.1);
                    }
                }
                DragTarget::ControlPoint2(_) => {
                    changed = segment.p2 != next;
                    segment.p2 = next;
                    if self.symmetric_handles {
                        segment.p1 = (1.0 - next.0, 1.0 - next.1);
                    }
                }
            }
        }
        if changed {
            self.emit_curve_changed();
            self.base.request_redraw();
        }
    }

    /// Rebuilds the two control-point labels when the points moved (K-P11).
    ///
    /// Called at the top of `draw`, before it borrows `self` through the style — the buffer needs a
    /// mutable borrow, and the format is the only work this saves.
    fn refresh_control_point_labels(&mut self) {
        let points = (self.control_point1(), self.control_point2());
        if self.label_points == Some(points) {
            return;
        }
        self.cp1_label.clear();
        let _ = core::fmt::Write::write_fmt(
            &mut self.cp1_label,
            format_args!("CP1: ({:.2}, {:.2})", points.0 .0, points.0 .1),
        );
        self.cp2_label.clear();
        let _ = core::fmt::Write::write_fmt(
            &mut self.cp2_label,
            format_args!("CP2: ({:.2}, {:.2})", points.1 .0, points.1 .1),
        );
        self.label_points = Some(points);
    }

    /// Draws a single grid line with a subtle color.
    fn draw_grid_line(&self, context: &mut RenderContext, from: Point, to: Point, color: Color) {
        context.draw_line(from, to, color);
    }

    /// Draws one control-point handle, with an emphasis ring when it is hovered or dragged.
    ///
    /// # Why the two emphasis weights differ
    ///
    /// A hover is "this is the one you would grab"; a drag is "you are holding it". The
    /// stronger weight on the drag is what tells a user mid-gesture that the pointer is
    /// still attached to the handle, which is the feedback a curve editor lives on. The
    /// two are the same pair of weights the theme's own state overlays use (0.08 hover,
    /// 0.12 pressed), so a handle does not invent a third visual language.
    ///
    /// The two flags are named for what they *draw* (`under_pointer`, `held`) rather than
    /// `hovered`/`dragging`, because `hovered` is the name of a control-level fact owned by
    /// `BaseWidget` — and a parameter that shadows that name is exactly what the
    /// state-source gate exists to flag.
    fn draw_handle(
        context: &mut RenderContext,
        center: Point,
        fill: Color,
        surface: Color,
        held: bool,
        under_pointer: bool,
    ) {
        const HOVER_HALO_RADIUS: u32 = HANDLE_RADIUS + 3;
        const DRAG_HALO_RADIUS: u32 = HANDLE_RADIUS + 5;
        if held {
            context.fill_circle(center, DRAG_HALO_RADIUS, fill.with_alpha(64));
        } else if under_pointer {
            context.fill_circle(center, HOVER_HALO_RADIUS, fill.with_alpha(40));
        }
        context.fill_circle(center, HANDLE_RADIUS, fill);
        context.draw_circle_stroke(center, HANDLE_RADIUS, surface, 2);
    }
}

impl Widget for BezierCurveEditor {
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
        crate::core::Size::new(
            crate::widget::metrics::dimensions::BEZIER_EDITOR_DEFAULT_WIDTH,
            crate::widget::metrics::dimensions::BEZIER_EDITOR_DEFAULT_HEIGHT,
        )
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
            "curve_changed" => {
                Some(EventSignalRef::mapped("curve_changed", &self.curve_changed, |v| {
                    CapabilityValue::Tuple(alloc::vec![
                        CapabilityValue::Float(v.0 .0 as f64),
                        CapabilityValue::Float(v.0 .1 as f64),
                        CapabilityValue::Float(v.1 .0 as f64),
                        CapabilityValue::Float(v.1 .1 as f64),
                    ])
                }))
            }
            _ => None,
        }
    }
}

/// `BezierCurveEditor`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` / `access_write_other.in.rs` dispatch.
impl WidgetProperties for BezierCurveEditor {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "snap_to_grid" => Ok(CapabilityValue::Bool(self.snap_to_grid())),
            // The segment count and the full geometry: without them a capability client could
            // not observe (or restore) the curve the control exists to edit.
            "segments" => Ok(CapabilityValue::UInt(self.segment_count() as u64)),
            "curve_points" => Ok(CapabilityValue::String(self.curve_points_token())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "snap_to_grid" => {
                self.set_snap_to_grid(expect_bool(value)?);
                Ok(())
            }
            "segments" => Err(CapabilityAccessError::ReadOnlyProperty),
            "curve_points" => {
                let token = crate::widget::capability::coercion::expect_string(value)?;
                self.set_curve_points_token(&token)
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["snap_to_grid", "segments", "curve_points", BASE_PROPERTY_NAMES]
    }
}

impl Draw for BezierCurveEditor {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        // Before anything borrows `self` for the frame, so a moved point refreshes the readouts
        // and a static curve reuses them (K-P11).
        self.refresh_control_point_labels();
        // Same rule for the sampled polyline: one evaluation of the cubic per moved frame, none
        // on an idle one (ADV-011).
        self.refresh_curve_samples(rect);
        let is_enabled = self.base.is_enabled();
        let margin = HANDLE_RADIUS as i32 + 4;
        let draw_w = (rect.width as i32 - 2 * margin).max(1) as f32;
        let draw_h = (rect.height as i32 - 2 * margin).max(1) as f32;

        // Chrome colours resolve explicit style first, then the theme's resolved
        // style for this control, and only then a literal. The theme step is what
        // makes an appearance switch visible; previously the surface, grid, axes,
        // polygon and labels were all hardcoded, so light and dark rendered the same.
        //
        // `resolved_theme_style` and `semantic_color` each take and release the
        // global manager's lock internally, so no guard is held across the draw (the
        // mutex is not re-entrant).
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("bezier_curve_editor");
        let surface = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| surface.blend(&Color::BLACK, 0.18));
        let text_color = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);

        // ── Background ──
        // A disabled editor is the same surface, faded: derived from the resolved
        // colour rather than a second literal so it still follows the appearance.
        let bg = if !is_enabled { surface.with_alpha(100) } else { surface.with_alpha(255) };
        context.face_with_gradient_corners(
            rect,
            bg,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().resolved_corner_radii().to_array(),
            Color::BLACK,
        );

        // ── Grid ──
        if self.show_grid && is_enabled {
            let grid_color = surface.blend(&text_color, GRID_ALPHA);
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
            let axis_color = surface.blend(&text_color, 0.6);
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

        // CP1 and CP2 are the two draggable identities of the curve, so they keep a
        // distinct hue each; both are drawn *over* the themed surface and their
        // handles are outlined in the surface colour so they stay readable in either
        // appearance. The handles' outline previously hardcoded white, which vanished
        // on a light surface once the surface stopped being hardcoded too.
        let cp1_color = surface.blend(&Color::BLUE, 0.75);
        let cp2_color = surface.blend(&Color::GREEN, 0.75);
        let poly_color = surface.blend(&text_color, 0.5);
        let curve_color = crate::style::semantic_color(crate::style::SemanticColor::Info)
            .map(|token| surface.blend(&token, 0.8))
            .unwrap_or_else(|| surface.blend(&Color::BLUE, 0.8));

        let first = self.first_segment();
        let last = self.segments.last().copied().unwrap_or(first);
        let p0 = self.curve_to_pixel(first.p0.0, first.p0.1);
        let p3 = self.curve_to_pixel(last.p3.0, last.p3.1);

        // ── Control polygon, curve and handles, segment by segment ──
        //
        // A handle the pointer is over, and the one being dragged, gains a halo in its own
        // hue. Both facts come from the same `hit_test_handle` the press uses, so the handle that
        // lights up is the handle a drag would move — on every segment, not just the first.
        for (index, segment) in self.segments.iter().enumerate() {
            let seg_p0 = self.curve_to_pixel(segment.p0.0, segment.p0.1);
            let seg_p3 = self.curve_to_pixel(segment.p3.0, segment.p3.1);
            let seg_cp1 = self.curve_to_pixel(segment.p1.0, segment.p1.1);
            let seg_cp2 = self.curve_to_pixel(segment.p2.0, segment.p2.1);
            context.draw_line(seg_p0, seg_cp1, poly_color);
            context.draw_line(seg_p3, seg_cp2, poly_color);

            // The polyline the cache holds is the one the per-frame loop used to build,
            // point for point; drawing it is all that remains of the sampling work.
            let (start, end) = self.curve_sample_ranges[index];
            for pair in self.curve_samples[start..end].windows(2) {
                context.draw_line_stroke(pair[0], pair[1], curve_color, 2);
            }

            Self::draw_handle(
                context,
                seg_cp1,
                cp1_color,
                surface,
                self.dragging == Some(DragTarget::ControlPoint1(index)),
                self.hovered_handle == Some(DragTarget::ControlPoint1(index)),
            );
            Self::draw_handle(
                context,
                seg_cp2,
                cp2_color,
                surface,
                self.dragging == Some(DragTarget::ControlPoint2(index)),
                self.hovered_handle == Some(DragTarget::ControlPoint2(index)),
            );
        }

        // ── Labels ──
        // Both readouts were formatted on every frame; they are now formatted when the points
        // actually move (K-P11).
        let font = effective_font(style);

        context.draw_text(
            Point::new(rect.x + 4, rect.y + 12),
            &self.cp1_label,
            font,
            // The labels name which control point is which, so the hue is information and stays
            // — but it has to be *readable* on the surface, which the handle's own colour is not:
            // the handle is a disc with a contrasting outline, so its fill only has to be visible
            // as a shape, whereas `rgb(5,5,196)` as text on the dark surface measured 1.63:1 and
            // the green measured 1.22:1 on the light one. Same hue, guaranteed ratio.
            cp1_color.legible_on(surface, 4.5),
            HorizontalAlignment::Left,
        );
        context.draw_text(
            Point::new(rect.x + 4, rect.y + 26),
            &self.cp2_label,
            font,
            cp2_color.legible_on(surface, 4.5),
            HorizontalAlignment::Left,
        );

        // ── Endpoint markers ──
        context.fill_circle(p0, 4, surface.blend(&text_color, 0.6));
        context.fill_circle(p3, 4, surface.blend(&text_color, 0.6));

        // A frame separates the editor from whatever it is embedded in.
        crate::widget::draw::stroke_surface_border(
            context,
            rect,
            self.style().resolved_corner_radii().to_array(),
            border,
            1,
        );
    }
}

impl EventHandler for BezierCurveEditor {
    fn handle_event(&mut self, event: &Event) {
        // The base keeps the control-level facts (`hovered`, `pressed`, `focus_reason`) and its
        // `MouseEnter`/`MouseLeave` arms are what make `widget_state()` answer `Hover` here. This
        // handler used to forward only in its catch-all arm, so the pointer state never reached
        // the base for the events it *did* consume — the theme's `"bezier_curve_editor:hover"`
        // override could not fire while dragging, which is exactly when feedback matters most.
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button, .. } => {
                if *button == 1 {
                    if let Some(target) = self.hit_test_handle(*pos) {
                        self.dragging = Some(target);
                        self.drag_origin = Some(self.segments.clone());
                        self.base.set_mouse_pressed(true);
                    }
                }
            }
            Event::MouseRelease { pos: _, button } => {
                if *button == 1 && self.dragging.is_some() {
                    self.dragging = None;
                    self.base.set_mouse_pressed(false);
                    // The whole gesture is one undo step, so the press's snapshot is recorded
                    // only now that the pointer has been lifted.
                    self.commit_drag();
                }
            }
            // Leaving the control clears both the hover emphasis and any drag the runtime
            // never delivered a release for. A press whose release lands outside the widget
            // never reaches the arm above: the runtime's hit-test returns `None` for a point
            // outside every control, so no `MouseRelease` is delivered. `dragging` (and the
            // base's pressed flag) stayed set, so the next hover kept moving a control point
            // with no button held. Cancelling on leave is the documented behaviour for this
            // pattern elsewhere (`KanbanBoard::cancel_drag`).
            Event::MouseLeave { .. } => {
                let had_drag = self.dragging.take().is_some();
                if had_drag {
                    // The moves already happened, so leaving commits the gesture rather than
                    // losing it from the history.
                    self.commit_drag();
                }
                if self.hovered_handle.take().is_some() || had_drag {
                    self.base.set_mouse_pressed(false);
                    self.base.request_redraw();
                }
            }
            Event::MouseMove { pos } => {
                if let Some(target) = self.dragging {
                    let (cx, cy) = self.pixel_to_curve(pos.x, pos.y);
                    self.set_segment_handle(target, cx, cy);
                }
                // Moving over a handle changes which one is emphasised, so the frame that
                // moved the pointer has to be repainted. The hit test is the same one the
                // press uses, so the handle that lights up is the handle a press would grab.
                let hovered = self.hit_test_handle(*pos);
                if hovered != self.hovered_handle {
                    self.hovered_handle = hovered;
                    self.base.request_redraw();
                }
            }
            _ => { /* Other events need no control-specific handling */ }
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
        assert_eq!(hit, Some(DragTarget::ControlPoint1(0)));

        let cp2_pixel = editor.curve_to_pixel(0.75, 0.9);
        let hit = editor.hit_test_handle(cp2_pixel);
        assert_eq!(hit, Some(DragTarget::ControlPoint2(0)));

        // Far away from any handle.
        let hit = editor.hit_test_handle(Point::new(5, 5));
        assert_eq!(hit, None);
    }

    #[test]
    fn bezier_drag_control_point1() {
        let mut editor = BezierCurveEditor::new(Rect::new(0, 0, 300, 300));
        let cp1_pixel = editor.curve_to_pixel(0.25, 0.1);

        // Mouse press on CP1.
        editor.handle_event(&Event::MousePress { pos: cp1_pixel, button: 1, modifiers: 0 });
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
        editor.handle_event(&Event::MousePress {
            pos: Point::new(100, 100),
            button: 1,
            modifiers: 0,
        });
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

    /// The pointer reaching a handle has to reach the *base* too, or the theme's
    /// `"bezier_curve_editor:hover"` entry is a key nothing can spell.
    ///
    /// Regression: this handler forwarded to the base only in its catch-all arm, so every
    /// event it actually consumed — press, release, move, leave — never recorded the
    /// pointer fact. `widget_state()` therefore answered `Normal` for the whole drag, which
    /// is the one moment a curve editor's feedback matters most.
    #[test]
    fn bezier_pointer_state_reaches_the_base() {
        use crate::style::WidgetState;

        let mut editor = default_editor();
        let rect = editor.geometry();
        editor.handle_event(&Event::MouseEnter { pos: Point::new(rect.x, rect.y) });
        assert_eq!(
            editor.widget_state(),
            WidgetState::Hover,
            "the base must have been told the pointer arrived"
        );
        editor.handle_event(&Event::MouseEnter { pos: Point::new(rect.x, rect.y) });
        let cp1_pixel = editor.curve_to_pixel(0.25, 0.1);
        editor.handle_event(&Event::MousePress { pos: cp1_pixel, button: 1, modifiers: 0 });
        assert_eq!(
            editor.widget_state(),
            WidgetState::Pressed,
            "a drag must outrank a hover, and the base is what decides that"
        );
        editor.handle_event(&Event::MouseLeave { pos: Point::new(rect.x, rect.y) });
        assert!(!editor.base.is_hovered(), "leaving must clear the hover fact");
    }

    /// The handle under the pointer has to be *painted* differently, or `dragging` is a
    /// field that is written and announced but never read.
    ///
    /// Assertion is a relation between two renders, not a literal colour: the frame with a
    /// pointer over a handle must differ from the frame without one, and both must still be
    /// a valid document. That is exactly what the defect (a stored-and-unread field) broke.
    #[test]
    fn the_handle_under_the_pointer_is_emphasised() {
        // Holds the crate-wide theme guard: this test renders, and a concurrent
        // test that switches the appearance would otherwise change a later frame.
        let _theme_guard = crate::style::theme_test_guard();
        let mut editor = BezierCurveEditor::new(Rect::new(0, 0, 300, 300));
        let idle = crate::widget::svg::render_to_svg(&mut editor);

        let cp1_pixel = editor.curve_to_pixel(0.25, 0.1);
        editor.handle_event(&Event::MouseMove { pos: cp1_pixel });
        assert_eq!(editor.hovered_handle, Some(DragTarget::ControlPoint1(0)));
        let hovered = crate::widget::svg::render_to_svg(&mut editor);
        assert_ne!(
            idle, hovered,
            "a handle under the pointer must render differently from an idle one"
        );

        // Away from every handle the emphasis has to clear again, or the last handle the
        // pointer crossed stays lit forever — the `hovered_row` mistake `list_view` made.
        editor.handle_event(&Event::MouseMove { pos: Point::new(5, 5) });
        assert_eq!(editor.hovered_handle, None);
        let cleared = crate::widget::svg::render_to_svg(&mut editor);
        assert_eq!(cleared, idle, "moving off every handle must restore the idle frame");
    }

    #[test]
    fn reset_and_set_points_restore_named_curve_states() {
        let mut editor = BezierCurveEditor::new(Rect::new(0, 0, 200, 200));
        editor.set_control_point1(0.1, 0.2);
        editor.reset();
        assert_eq!(editor.control_point1(), (0.25, 0.1));
        assert_eq!(editor.control_point2(), (0.75, 0.9));

        editor.set_points(&[(0.2, 0.3), (0.8, 0.7)]).expect("two points are a cubic");
        assert_eq!(editor.control_point1(), (0.2, 0.3));
        assert_eq!(editor.control_point2(), (0.8, 0.7));
        assert_eq!(editor.set_points(&[(0.5, 0.5)]), Err(CapabilityAccessError::OutOfRange));
    }

    fn assert_point_close(actual: (f32, f32), expected: (f32, f32), context: &str) {
        assert!(
            (actual.0 - expected.0).abs() < 1e-4 && (actual.1 - expected.1).abs() < 1e-4,
            "{context}: {actual:?} != {expected:?}"
        );
    }

    /// de Casteljau split followed by the inverse merge must reproduce the original cubic.
    #[test]
    fn split_then_merge_restores_the_original_cubic() {
        let mut editor = BezierCurveEditor::new(Rect::new(0, 0, 200, 200));
        editor.set_points(&[(0.25, 0.1), (0.75, 0.9)]).expect("two controls");
        let original = editor.segment_control_points(0).expect("one segment");

        assert!(editor.add_point(0.4), "0.4 is inside the only segment");
        assert_eq!(editor.segment_count(), 2);
        assert_eq!(editor.point_count(), 3);

        assert!(editor.remove_point(1), "the interior point is removable");
        assert_eq!(editor.segment_count(), 1);
        assert_eq!(editor.point_count(), 2);
        let merged = editor.segment_control_points(0).expect("the merge produced a segment");
        for index in 0..4 {
            assert_point_close(merged[index], original[index], "merge is the split inverse");
        }
    }

    /// Splitting a segment must not move the curve: only the point count changes.
    #[test]
    fn splitting_preserves_the_sampled_curve() {
        let mut editor = BezierCurveEditor::new(Rect::new(0, 0, 200, 200));
        editor.set_points(&[(0.1, 0.8), (0.9, 0.2)]).expect("two controls");
        let before: Vec<(f32, f32)> =
            (0..=40).map(|step| editor.sample_at(step as f32 / 40.0)).collect();

        assert!(editor.add_point(0.37));
        assert_eq!(editor.segment_count(), 2);
        for (step, expected) in before.iter().enumerate() {
            let actual = editor.sample_at(step as f32 / 40.0);
            assert_point_close(actual, *expected, "split preserves the parameterisation");
        }
    }

    /// The new on-curve point is exactly the join shared by the two halves.
    #[test]
    fn split_point_is_shared_by_the_two_segments() {
        let mut editor = BezierCurveEditor::new(Rect::new(0, 0, 200, 200));
        editor.set_points(&[(0.3, 0.2), (0.7, 0.8)]).expect("two controls");
        let split = editor.sample_at(0.37);

        assert!(editor.add_point(0.37));
        let left = editor.segment_control_points(0).expect("left half");
        let right = editor.segment_control_points(1).expect("right half");
        assert_point_close(left[3], split, "the left half ends at the split point");
        assert_point_close(right[0], split, "the right half starts at the split point");
        assert_point_close(editor.sample_at(0.37), split, "the split parameter did not move");
    }

    /// Endpoints are not interior points: they cannot be removed and cannot be split onto.
    #[test]
    fn add_and_remove_reject_endpoints_and_empty_joins() {
        let mut editor = BezierCurveEditor::new(Rect::new(0, 0, 200, 200));
        assert!(!editor.add_point(0.0));
        assert!(!editor.add_point(1.0));
        assert!(!editor.remove_point(0));
        assert!(!editor.remove_point(1), "a single segment has no interior point");
        assert_eq!(editor.point_count(), 2);

        assert!(editor.add_point(0.5));
        assert!(!editor.add_point(0.5), "an existing join is not split again");
    }

    /// Repeated splits and removals are a stack: removing both points restores the original.
    #[test]
    fn repeated_split_and_remove_round_trips() {
        let mut editor = BezierCurveEditor::new(Rect::new(0, 0, 200, 200));
        editor.set_points(&[(0.2, 0.4), (0.8, 0.6)]).expect("two controls");
        let original = editor.segment_control_points(0).expect("one segment");

        assert!(editor.add_point(0.25));
        assert!(editor.add_point(0.75));
        assert_eq!(editor.segment_count(), 3);
        assert_eq!(editor.point_count(), 4);

        assert!(editor.remove_point(2));
        assert!(editor.remove_point(1));
        assert_eq!(editor.segment_count(), 1);
        let merged = editor.segment_control_points(0).expect("back to one segment");
        for index in 0..4 {
            assert_point_close(merged[index], original[index], "nested round trip");
        }
    }

    /// The sampled polyline is rebuilt only when the geometry or a control point moves.
    #[test]
    fn idle_frames_reuse_the_curve_samples() {
        let mut editor = default_editor();
        let _ = crate::widget::svg::render_to_svg(&mut editor);
        let cached = editor.curve_samples.clone();
        assert_eq!(editor.curve_sample_ranges.len(), 1, "one range per segment");
        assert_eq!(
            cached.len(),
            CURVE_SAMPLES_PER_SEGMENT + 1,
            "the endpoint plus one sample per step"
        );

        let _ = crate::widget::svg::render_to_svg(&mut editor);
        assert_eq!(editor.curve_samples, cached, "an idle frame reuses the polyline");

        editor.set_control_point1(0.1, 0.2);
        let _ = crate::widget::svg::render_to_svg(&mut editor);
        assert_ne!(editor.curve_samples, cached, "a moved control point resamples");
    }

    /// A drag records one undo step when it commits, and undo/redo restore the exact point set
    /// (M-004).
    #[test]
    fn a_drag_commit_undoes_and_redoes_the_exact_points() {
        let mut editor = BezierCurveEditor::new(Rect::new(0, 0, 300, 300));
        let original = editor.segment_control_points(0).expect("one segment");
        let start = editor.curve_to_pixel(0.25, 0.1);
        let end = editor.curve_to_pixel(0.5, 0.5);

        editor.handle_event(&Event::MousePress { pos: start, button: 1, modifiers: 0 });
        editor.handle_event(&Event::MouseMove { pos: end });
        assert!(!editor.can_undo(), "the gesture is not history until it commits");
        editor.handle_event(&Event::MouseRelease { pos: end, button: 1 });
        assert!(editor.can_undo(), "the release commits one step");

        let dragged = editor.segment_control_points(0).expect("one segment");
        assert_ne!(dragged, original, "the drag moved the handle");
        assert!(editor.undo());
        assert_eq!(
            editor.segment_control_points(0),
            Some(original),
            "undo restores the exact points"
        );
        assert!(editor.can_redo());
        assert!(editor.redo());
        assert_eq!(
            editor.segment_control_points(0),
            Some(dragged),
            "redo restores the drag exactly"
        );
    }

    /// Adding and removing a point are each one undoable command, and undo returns the exact
    /// control-point set (M-004).
    #[test]
    fn adding_and_removing_a_point_is_undoable_and_exact() {
        let mut editor = BezierCurveEditor::new(Rect::new(0, 0, 200, 200));
        editor.set_points(&[(0.2, 0.3), (0.8, 0.7)]).expect("two controls");
        let original = editor.segment_control_points(0).expect("one segment");

        assert!(editor.add_point(0.4));
        let split_left = editor.segment_control_points(0).expect("left half");
        assert_eq!(editor.segment_count(), 2);
        assert!(editor.undo());
        assert_eq!(editor.segment_count(), 1);
        assert_eq!(editor.segment_control_points(0), Some(original), "the split is undone exactly");
        assert!(editor.redo());
        assert_eq!(editor.segment_control_points(0), Some(split_left));

        assert!(editor.remove_point(1));
        assert_eq!(editor.segment_count(), 1);
        assert!(editor.undo());
        assert_eq!(editor.segment_count(), 2);
        assert_eq!(
            editor.segment_control_points(0),
            Some(split_left),
            "the removal is undone exactly"
        );
    }

    /// An empty history, a press with no motion, and an edit the curve refused all record nothing
    /// (M-004 boundaries).
    #[test]
    fn an_empty_history_and_a_motionless_or_refused_edit_record_nothing() {
        let mut editor = default_editor();
        assert!(!editor.can_undo());
        assert!(!editor.undo());
        assert!(!editor.redo());

        let handle = editor.curve_to_pixel(0.25, 0.1);
        editor.handle_event(&Event::MousePress { pos: handle, button: 1, modifiers: 0 });
        editor.handle_event(&Event::MouseRelease { pos: handle, button: 1 });
        assert!(!editor.can_undo(), "a press with no motion is not an edit");

        assert!(!editor.add_point(0.0));
        assert!(!editor.add_point(1.0));
        assert!(!editor.remove_point(0));
        assert!(!editor.can_undo(), "an edit the curve refused records nothing");
    }

    /// A drag whose release lands outside the control is still committed: the moves already
    /// happened (M-004).
    #[test]
    fn leaving_mid_drag_commits_the_moves_that_happened() {
        let mut editor = BezierCurveEditor::new(Rect::new(0, 0, 300, 300));
        let start = editor.curve_to_pixel(0.25, 0.1);
        let end = editor.curve_to_pixel(0.4, 0.4);
        editor.handle_event(&Event::MousePress { pos: start, button: 1, modifiers: 0 });
        editor.handle_event(&Event::MouseMove { pos: end });
        editor.handle_event(&Event::MouseLeave { pos: Point::new(299, 299) });

        assert!(editor.can_undo(), "the moves already happened, so leaving commits them");
        assert!(editor.undo());
        assert_eq!(editor.control_point1(), (0.25, 0.1));
    }
}
