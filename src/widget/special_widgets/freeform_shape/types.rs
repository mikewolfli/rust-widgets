// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The geometry a freeform shape is built from.
//!
//! # Why these are not part of the control's file
//!
//! [`ShapePath`] and its parts describe *a shape* — a closed path, its segments, and
//! which way a speech-bubble tail points. They are data: a host can build one, store
//! one, and hand it to a control, without knowing that a control exists. Keeping them
//! beside the widget made the geometry look like an implementation detail of it, and
//! made the file 900 lines of two different subjects.
//!
//! They live here rather than in `core` because they are freeform-shape vocabulary,
//! not a primitive every control speaks.

use crate::core::Point;

/// Which corner or edge of a speech bubble the tail sticks out of.
///
/// The eight variants are the four corners and the four edge midpoints; the tail
/// is drawn outside the bubble's rectangle, so the shape's drawn extent is
/// slightly larger than the widget's geometry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BubbleTailDirection {
    /// Tail out of the top-left corner.
    TopLeft,
    /// Tail out of the top-right corner.
    TopRight,
    /// Tail out of the bottom-left corner.
    BottomLeft,
    /// Tail out of the bottom-right corner.
    BottomRight,
    /// Tail out of the middle of the left edge.
    Left,
    /// Tail out of the middle of the right edge.
    Right,
    /// Tail out of the middle of the top edge.
    Top,
    /// Tail out of the middle of the bottom edge.
    Bottom,
}

/// One step of a [`ShapePath::Custom`] path, in the coordinates of the widget's
/// own rectangle: the origin is the widget's top-left corner, x grows right and
/// y grows down, and lengths are **pixels** — there is no normalisation, so a
/// path authored for one size does not scale with the widget.
///
/// Control points are absolute positions like the endpoints, not deltas, and
/// consecutive segments are implicitly joined, so no segment needs to restate
/// where the previous one ended.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PathSegment {
    /// Starts a new subpath at this point, lifting the pen. Any subpath in
    /// progress is closed off first and, once flattened, is treated as a separate
    /// region for hit-testing — so a path that never revisits its start point
    /// leaves that region open.
    MoveTo(Point),
    /// Draws a straight line from the current point to `Point`.
    LineTo(Point),
    /// Draws a cubic Bézier from the current point to the third point, using the
    /// first two as its two control points. It is flattened into straight-line
    /// segments before hit-testing, so a point tested against it is tested
    /// against the approximation, not the true curve.
    CurveTo(Point, Point, Point),
    /// Draws a quadratic Bézier from the current point to the second point, using
    /// the first as its control point. Flattened like [`PathSegment::CurveTo`].
    QuadTo(Point, Point),
    /// Joins the subpath back to its [`PathSegment::MoveTo`] point and makes that
    /// point current again.
    Close,
}

/// One step of a [`ShapePath::UnitCustom`] path, in the widget's unit square.
///
/// Coordinates are fractions of the widget's width and height: `(0.0, 0.0)` is its
/// top-left corner and `(1.0, 1.0)` its bottom-right. A path authored this way scales
/// with the widget, unlike [`PathSegment`], whose points are pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UnitPathSegment {
    /// Starts a new subpath at this point, lifting the pen.
    MoveTo(f32, f32),
    /// Draws a straight line from the current point to this point.
    LineTo(f32, f32),
    /// Draws a cubic Bézier: two control points then the endpoint.
    CurveTo((f32, f32), (f32, f32), (f32, f32)),
    /// Draws a quadratic Bézier: one control point then the endpoint.
    QuadTo((f32, f32), (f32, f32)),
    /// Joins the subpath back to its [`UnitPathSegment::MoveTo`] point.
    Close,
}

impl UnitPathSegment {
    /// Converts this unit-square step into a pixel step for a `width` × `height` shape.
    pub fn to_path_segment(self, width: f32, height: f32) -> PathSegment {
        fn map((x, y): (f32, f32), width: f32, height: f32) -> Point {
            Point::from_f32(x * width, y * height)
        }
        match self {
            Self::MoveTo(x, y) => PathSegment::MoveTo(map((x, y), width, height)),
            Self::LineTo(x, y) => PathSegment::LineTo(map((x, y), width, height)),
            Self::CurveTo(control1, control2, end) => PathSegment::CurveTo(
                map(control1, width, height),
                map(control2, width, height),
                map(end, width, height),
            ),
            Self::QuadTo(control, end) => {
                PathSegment::QuadTo(map(control, width, height), map(end, width, height))
            }
            Self::Close => PathSegment::Close,
        }
    }
}

/// The outline a `FreeformShapeWidget` paints and hit-tests against.
///
/// Written as a code span rather than a link because the widget now lives in a
/// sibling module (`shape`), and a link from public docs to an item in a private
/// module is a rustdoc error. The split is what moved it; the concept did not change.
///
/// Every variant is described in the widget's own rectangle rather than in a
/// fixed coordinate space, so the shape scales with the widget's geometry except
/// for [`ShapePath::Custom`], whose points are literal pixels.
#[derive(Debug, Clone, PartialEq)]
pub enum ShapePath {
    /// A heart, fitted to the widget's rectangle and centred in it.
    Heart,
    /// A star with the given numbers of points.
    Star {
        /// How many points the star has. Raised to a minimum of 3, so a smaller
        /// request (including 0) still draws — and hit-tests as — a triangle
        /// rather than producing a degenerate shape.
        points: u8,
        /// The inner radius as a fraction of the outer radius; the smaller it is,
        /// the spikier the star. Clamped into `0.05`..=`0.95`.
        inner_radius: f32,
    },
    /// A closed polygon through `Vec<Point>`'s vertices, in order, in widget-local
    /// **pixels**. Fewer than 3 vertices yields a shape that hit-tests as empty.
    Polygon(Vec<Point>),
    /// A rectangle with its corners rounded.
    RoundedRect {
        /// Corner radius in **pixels**. Reduced to fit — at most half the
        /// widget's smaller dimension — so an oversized radius degrades to a
        /// rectangle with the largest corners that fit instead of overflowing.
        /// `0` gives square corners.
        radius: u32,
    },
    /// A rounded speech bubble with a tapering tail.
    Bubble {
        /// Which corner or edge the tail leaves from.
        tail_direction: BubbleTailDirection,
    },
    /// An arbitrary outline built from path segments. Unlike the other variants
    /// this one does not scale with the widget's size.
    Custom(Vec<PathSegment>),
    /// An arbitrary outline in unit-square coordinates, so it scales with the
    /// widget's size. Converted to [`ShapePath::Custom`] pixels when hit-tested or
    /// painted.
    UnitCustom(Vec<UnitPathSegment>),
}
