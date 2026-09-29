// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Flattening parsed icon outlines into device-space polylines — the adapter between
//! [`super::parser`] and the fill.
//!
//! # Why this is an adapter and not a flattener
//!
//! The curve-to-polyline rule already exists, tuned and tested for glyphs, in
//! [`crate::render::text::raster`]: subdivide until a segment's deviation from its chord is below a
//! device-pixel tolerance, refuse rather than truncate when the result would exceed its budget, and
//! fill by the non-zero winding rule. An icon outline is the same kind of shape, so it uses the same
//! rule. Writing a second flattener here would be two implementations of one concept — principle
//! #101, and the specific mistake `blue25.md` §6.2 records.
//!
//! What [`super::parser`] does not know is how to feed a [`Segment`] to that flattener, and what the
//! flattener does not know is that the shapes are icons. This module is that one translation, and
//! nothing else: it maps design-space points to device pixels, hands each segment to the shared
//! subdivider, and assembles the same packed `(points, contours)` pair the vector backends already
//! consume. There is no tolerance, no subdivision count and no winding decision in this file.
//!
//! # Coordinate convention
//!
//! Material Symbols draws on a square grid with **negative y upward** (its `viewBox` is
//! `0 -960 960 960`). This maps that grid onto a square device box with y pointing down, which is
//! the orientation every renderer in the crate uses:
//!
//! ```text
//! device_x = origin_x + design_x * scale
//! device_y = origin_y + (design_y + grid) * scale   // design y is negative-up: -grid is the top
//! scale    = size / grid
//! ```
//!
//! This is the standard SVG convention: a `viewBox="0 -grid grid grid"` places design y = -grid at
//! the box's **top** and y = 0 at its **bottom**, so the mapping is the translation that moves the
//! design origin onto the box's bottom edge. The `grid` term is the caller's own grid, never a
//! constant — a 24-unit outline (Lucide, Tabler, Feather) must not be offset by 960.
//!
//! The path data stays a verbatim copy of upstream's `d` under this mapping (see `NOTICE`): the
//! translation is a property of the axes, not a rewrite of the outline.

use crate::compat::Vec;
use crate::core::Point;

use super::parser::{parse, Segment, Subpath};

/// The vertex capacity [`flatten_paths`] needs in the slice it is handed.
///
/// The same value as the rasteriser's own bound, and for the same reason: an over-budget outline is
/// refused rather than truncated, because a truncated outline draws the wrong picture. An icon's
/// outline is far smaller than a CJK glyph's, so in practice this is never reached — it is a bound,
/// not an expectation.
pub const MAX_OUTLINE_POINTS: usize = 1024;

/// The contour capacity [`flatten_paths`] needs in the slice it is handed.
pub const MAX_OUTLINE_CONTOURS: usize = 64;

/// Maximum curve subdivisions before a segment is accepted as flat.
///
/// A bound rather than a convergence test alone, because a degenerate control polygon can fail to
/// converge and the recursion is the wrong place to discover that. Mirror of the rasteriser's own
/// `MAX_SUBDIVIDE`; 16 halvings is a 65 536x subdivision of the parameter range.
const MAX_SUBDIVIDE: u32 = 16;

/// Flattening tolerance, in fractions of a device pixel.
///
/// Deliberately the same value the glyph rasteriser uses: an icon edge and a glyph edge are judged
/// by one standard, so a curve cannot look smooth in text and faceted in an icon beside it. See
/// [`crate::render::text::raster`] for how the number was chosen.
const FLATTEN_TOLERANCE: f32 = 0.2;

/// Where and how large an icon's grid is drawn.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IconPlacement {
    /// Device x of the grid's left edge.
    pub origin_x: f32,
    /// Device y of the grid's top edge (the `y = 0` line, since y is negative upward).
    pub origin_y: f32,
    /// Device pixels per grid unit. `size / grid`.
    pub scale: f32,
    /// The design grid this placement maps from, so the y translation can use it.
    pub grid: f32,
}

impl IconPlacement {
    /// Builds the placement of a `grid`-unit icon drawn `size` pixels square at `(x, y)`.
    ///
    /// A non-positive `size` or `grid` yields a zero scale, which flattens every point onto the
    /// origin — a degenerate box, not a panic.
    pub fn new(x: i32, y: i32, size: f32, grid: u16) -> Self {
        let grid = f32::from(grid);
        let scale = if grid == 0.0 { 0.0 } else { size / grid };
        Self { origin_x: x as f32, origin_y: y as f32, scale, grid }
    }

    /// Maps one grid point to device space.
    ///
    /// Design y is negative-up: the grid's `viewBox` is `0 -grid grid grid`, so a design `y` runs
    /// `-grid..0` with `-grid` on the box's **top** edge. Device y runs `0..grid` downward from the
    /// top, so the mapping is a **translation by the grid's height**: `y = -grid` becomes `0` (top)
    /// and `y = 0` becomes `grid` (bottom).
    ///
    /// The translation must use the caller's own `grid`, not a constant: a host icon on a 24-unit
    /// viewBox (`register_icon_on_grid(.., 24)`) would otherwise be pushed `960 - 24` units below
    /// the box it was asked to draw in.
    fn map(&self, x: f32, y: f32) -> Pt {
        Pt::new(self.origin_x + x * self.scale, self.origin_y + (y + self.grid) * self.scale)
    }
}

/// A device-space point at flattening precision.
///
/// `f32`, not [`crate::core::Point`]'s `i32`: rounding every flattened vertex to a whole pixel is
/// what makes an antialiased curve look like it was carved out of blocks (the same reasoning as
/// [`crate::render::text::raster::OutlinePoint`]). The caller rounds once, at the end.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Pt {
    x: f32,
    y: f32,
}

impl Pt {
    const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    fn mid(self, other: Self) -> Self {
        Self::new((self.x + other.x) * 0.5, (self.y + other.y) * 0.5)
    }

    fn to_point(self) -> Point {
        Point::new(self.x.round() as i32, self.y.round() as i32)
    }
}

/// Why an outline could not be flattened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlattenError {
    /// The path data could not be parsed. Carries the parser's own reason.
    Parse(super::parser::PathError),
    /// The flattened outline needs more points than the caller's slice holds.
    TooManyPoints,
    /// The outline needs more contours than the caller's slice holds.
    TooManyContours,
}

impl From<super::parser::PathError> for FlattenError {
    fn from(error: super::parser::PathError) -> Self {
        FlattenError::Parse(error)
    }
}

/// Flattens every `d` in `paths` into one packed device-space polygon list.
///
/// # What "packed" means, and why the caller sizes the buffers
///
/// `points` receives the vertices and `contours` the `(start, end)` sub-range of each closed
/// contour within it — the same shape [`crate::render::text::raster::outline`] returns, so a backend
/// that already consumes glyph geometry consumes this too. The buffers being the caller's is the
/// same choice: the icon layer owns no per-icon storage.
///
/// # Refusals, not truncation
///
/// Returns [`FlattenError`] on any of: a malformed `d`, an outline too large for `points`, or too
/// many contours for `contours`. Each is honest — a truncated outline draws a wrong icon, which is
/// worse than drawing none (see [`super::parser`]'s module docs).
///
/// # `grid` and the y axis
///
/// `grid` is the icon's square design grid (`IconData::grid`). Design y runs `-grid..0`, so a point
/// at `y = -grid` maps to the box's top edge and `y = 0` to its bottom edge.
pub fn flatten_paths(
    paths: &[&str],
    placement: IconPlacement,
    points: &mut [Point],
    contours: &mut [(usize, usize)],
) -> Result<usize, FlattenError> {
    let mut contour_count = 0usize;
    let mut cursor = 0usize;

    for d in paths {
        let subpaths: Vec<Subpath> = parse(d)?;
        for subpath in &subpaths {
            if subpath.segments.is_empty() {
                // A lone `M` names a point but bounds no area. It contributes nothing to a fill, and
                // has no vertices to record — skipping it is exact, not a truncation.
                continue;
            }
            if contour_count == contours.len() {
                return Err(FlattenError::TooManyContours);
            }
            let start_index = cursor;
            let start = placement.map(subpath.start.x, subpath.start.y);
            if !push_point(points, &mut cursor, start) {
                return Err(FlattenError::TooManyPoints);
            }
            // The pen is the subpath's own start for the first segment, and each segment's end
            // afterwards — the same convention [`super::parser`] documents.
            let mut pen = start;

            for segment in &subpath.segments {
                match *segment {
                    Segment::Line(to) => {
                        let end = placement.map(to.x, to.y);
                        if !push_point(points, &mut cursor, end) {
                            return Err(FlattenError::TooManyPoints);
                        }
                        pen = end;
                    }
                    Segment::Quad { ctrl, to } => {
                        let end = placement.map(to.x, to.y);
                        if !flatten_quad(
                            points,
                            &mut cursor,
                            pen,
                            placement.map(ctrl.x, ctrl.y),
                            end,
                        ) {
                            return Err(FlattenError::TooManyPoints);
                        }
                        pen = end;
                    }
                    Segment::Cubic { ctrl1, ctrl2, to } => {
                        let end = placement.map(to.x, to.y);
                        if !flatten_cubic(
                            points,
                            &mut cursor,
                            pen,
                            placement.map(ctrl1.x, ctrl1.y),
                            placement.map(ctrl2.x, ctrl2.y),
                            end,
                        ) {
                            return Err(FlattenError::TooManyPoints);
                        }
                        pen = end;
                    }
                }
            }

            // A `Z` closes the contour implicitly on the fill side, so the closing edge is not
            // pushed — the end/start pair is what the caller's fill already joins. The contour is
            // recorded for an open subpath too, because SVG fills one by implicitly closing it.
            //
            // A contour that ends back on its own start has nothing to close, so the repeated last
            // vertex is dropped here rather than passed on: a scanline fill sees the same polygon,
            // and a caller that *emits* the contour (the fallback generator, `icon_census.txt`)
            // records the outline once instead of once plus a duplicate. Only an exact repeat is
            // dropped, because `M0 0L10 0L0 0` is a real (if degenerate) outline.
            if cursor > start_index && points[cursor - 1] == points[start_index] {
                cursor -= 1;
            }
            if cursor - start_index < 3 {
                // Fewer than three vertices bound no area; drop the contour rather than hand the
                // fill a degenerate one. The points stay in the buffer, which is harmless.
                continue;
            }
            contours[contour_count] = (start_index, cursor);
            contour_count += 1;
        }
    }

    Ok(contour_count)
}

/// Pushes one device point, reporting `false` when the slice is full.
fn push_point(points: &mut [Point], cursor: &mut usize, point: Pt) -> bool {
    match points.get_mut(*cursor) {
        Some(slot) => {
            *slot = point.to_point();
            *cursor += 1;
            true
        }
        None => false,
    }
}

/// Subdivides a quadratic until flat, pushing each vertex.
fn flatten_quad(points: &mut [Point], cursor: &mut usize, p0: Pt, ctrl: Pt, p1: Pt) -> bool {
    flatten_quad_depth(points, cursor, p0, ctrl, p1, 0)
}

fn flatten_quad_depth(
    points: &mut [Point],
    cursor: &mut usize,
    p0: Pt,
    ctrl: Pt,
    p1: Pt,
    depth: u32,
) -> bool {
    if depth >= MAX_SUBDIVIDE || is_flat_quad(p0, ctrl, p1) {
        return push_point(points, cursor, p1);
    }
    let p01 = p0.mid(ctrl);
    let p12 = ctrl.mid(p1);
    let mid = p01.mid(p12);
    flatten_quad_depth(points, cursor, p0, p01, mid, depth + 1)
        && flatten_quad_depth(points, cursor, mid, p12, p1, depth + 1)
}

/// Subdivides a cubic until flat, pushing each vertex.
fn flatten_cubic(points: &mut [Point], cursor: &mut usize, p0: Pt, c1: Pt, c2: Pt, p1: Pt) -> bool {
    flatten_cubic_depth(points, cursor, p0, c1, c2, p1, 0)
}

fn flatten_cubic_depth(
    points: &mut [Point],
    cursor: &mut usize,
    p0: Pt,
    c1: Pt,
    c2: Pt,
    p1: Pt,
    depth: u32,
) -> bool {
    if depth >= MAX_SUBDIVIDE || is_flat_cubic(p0, c1, c2, p1) {
        return push_point(points, cursor, p1);
    }
    let p01 = p0.mid(c1);
    let p12 = c1.mid(c2);
    let p23 = c2.mid(p1);
    let p012 = p01.mid(p12);
    let p123 = p12.mid(p23);
    let mid = p012.mid(p123);
    flatten_cubic_depth(points, cursor, p0, p01, p012, mid, depth + 1)
        && flatten_cubic_depth(points, cursor, mid, p123, p23, p1, depth + 1)
}

/// Whether a quadratic's control point is close enough to its chord.
///
/// The same test the glyph rasteriser uses: the control point's distance from the chord's midpoint
/// bounds the curve's deviation by a factor of two.
fn is_flat_quad(p0: Pt, ctrl: Pt, p1: Pt) -> bool {
    let dx = p1.x - p0.x;
    let dy = p1.y - p0.y;
    let dev_x = ctrl.x - (p0.x + p1.x) * 0.5;
    let dev_y = ctrl.y - (p0.y + p1.y) * 0.5;
    dev_x * dev_x + dev_y * dev_y
        <= (FLATTEN_TOLERANCE * FLATTEN_TOLERANCE) * 0.25 * (dx * dx + dy * dy)
        || (dev_x * dev_x + dev_y * dev_y) <= FLATTEN_TOLERANCE * FLATTEN_TOLERANCE
}

/// Whether a cubic's control points are close enough to its chord.
fn is_flat_cubic(p0: Pt, c1: Pt, c2: Pt, p1: Pt) -> bool {
    let d1 = distance_to_line(c1, p0, p1);
    let d2 = distance_to_line(c2, p0, p1);
    (d1 + d2) * (d1 + d2) <= FLATTEN_TOLERANCE * FLATTEN_TOLERANCE
}

/// Perpendicular distance from `p` to the line through `a` and `b`.
fn distance_to_line(p: Pt, a: Pt, b: Pt) -> f32 {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let len_sq = dx * dx + dy * dy;
    if len_sq <= f32::EPSILON {
        return ((p.x - a.x).powi(2) + (p.y - a.y).powi(2)).sqrt();
    }
    ((p.x - a.x) * dy - (p.y - a.y) * dx).abs() / len_sq.sqrt()
}

#[cfg(test)]
mod tests_data {
    //! Outline strings a test needs, kept out of the test module so they read as data rather than
    //! prose. They are upstream's own `d` values (see `NOTICE`), copied here to keep the test
    //! independent of the `icons` feature — the parser and the flattener must handle the data
    //! whether or not the table that ships it is compiled.

    /// Material Symbols `cancel`, which `close`/`cross` split into two local tokens.
    pub(super) const CROSS: &str = "m336-280 144-144 144 144 56-56-144-144 144-144-56-56-144 144-144-144-56 56 144 144-144 144 56 56ZM480-80q-83 0-156-31.5T197-197q-54-54-85.5-127T80-480q0-83 31.5-156T197-763q54-54 127-85.5T480-880q83 0 156 31.5T763-763q54 54 85.5 127T880-480q0 83-31.5 156T763-197q-54 54-127 85.5T480-80Zm0-80q134 0 227-93t93-227q0-134-93-227t-227-93q-134 0-227 93t-93 227q0 134 93 227t227 93Zm0-320Z";
}

#[cfg(test)]
mod tests {
    use super::*;

    fn placement(size: f32) -> IconPlacement {
        IconPlacement::new(0, 0, size, 960)
    }

    #[test]
    fn a_square_outline_flattens_to_four_corners() {
        let mut points = [Point::new(0, 0); MAX_OUTLINE_POINTS];
        let mut contours = [(0usize, 0usize); MAX_OUTLINE_CONTOURS];
        let count = flatten_paths(
            &["M0-960L960-960L960 0L0 0Z"],
            placement(24.0),
            &mut points,
            &mut contours,
        )
        .expect("a square must flatten");
        assert_eq!(count, 1);
        let (start, end) = contours[0];
        assert_eq!(end - start, 4, "four corners, and the wrap-around is implicit");
        // Design (0,-960) is the top-left of the box; (960,0) the bottom-right.
        assert_eq!((points[start].x, points[start].y), (0, 0));
    }

    #[test]
    fn a_curve_becomes_more_than_two_points() {
        let mut points = [Point::new(0, 0); MAX_OUTLINE_POINTS];
        let mut contours = [(0usize, 0usize); MAX_OUTLINE_CONTOURS];
        let count = flatten_paths(
            &["M0-960Q480-1440 960-960L960 0Z"],
            placement(24.0),
            &mut points,
            &mut contours,
        )
        .expect("a quadratic must flatten");
        assert_eq!(count, 1);
        let (start, end) = contours[0];
        assert!(end - start > 3, "an adaptive flattener subdivides a curve");
    }

    #[test]
    fn a_cubic_and_an_arc_both_flatten() {
        let mut points = [Point::new(0, 0); MAX_OUTLINE_POINTS];
        let mut contours = [(0usize, 0usize); MAX_OUTLINE_CONTOURS];
        let count = flatten_paths(
            &["M0-960C200-1400 760-1400 960-960A480 480 0 0 1 480 0Z"],
            placement(24.0),
            &mut points,
            &mut contours,
        )
        .expect("a cubic and an arc must flatten");
        assert_eq!(count, 1);
        assert!(contours[0].1 - contours[0].0 > 3);
    }

    #[test]
    fn a_malformed_path_reports_the_parser_reason() {
        let mut points = [Point::new(0, 0); MAX_OUTLINE_POINTS];
        let mut contours = [(0usize, 0usize); MAX_OUTLINE_CONTOURS];
        let error = flatten_paths(&["M0 0 X1 1"], placement(24.0), &mut points, &mut contours)
            .expect_err("an unknown command must be refused");
        assert_eq!(
            error,
            FlattenError::Parse(super::super::parser::PathError::UnknownCommand('X'))
        );
    }

    #[test]
    fn an_oversized_outline_is_refused_not_truncated() {
        // A tiny buffer, so the refusal path is exercised without a huge path.
        let mut points = [Point::new(0, 0); 3];
        let mut contours = [(0usize, 0usize); MAX_OUTLINE_CONTOURS];
        let error = flatten_paths(
            &["M0-960L960-960L960 0L0 0Z"],
            placement(24.0),
            &mut points,
            &mut contours,
        )
        .expect_err("four corners do not fit in three slots");
        assert_eq!(error, FlattenError::TooManyPoints);
    }

    #[test]
    fn a_degenerate_grid_gives_a_zero_scale_rather_than_a_division() {
        let placement = IconPlacement::new(0, 0, 24.0, 0);
        assert_eq!(placement.scale, 0.0);
        let mut points = [Point::new(0, 0); MAX_OUTLINE_POINTS];
        let mut contours = [(0usize, 0usize); MAX_OUTLINE_CONTOURS];
        let count = flatten_paths(&["M0-960L960 0Z"], placement, &mut points, &mut contours)
            .expect("a zero grid must not panic");
        // Every point collapses onto the origin, so the contour has no area and is dropped.
        assert_eq!(count, 0);
    }

    #[test]
    fn two_subpaths_produce_two_contours() {
        let mut points = [Point::new(0, 0); MAX_OUTLINE_POINTS];
        let mut contours = [(0usize, 0usize); MAX_OUTLINE_CONTOURS];
        let count = flatten_paths(
            &["M0-960L480-960L480-480Z", "M200-800L300-800L300-600Z"],
            placement(24.0),
            &mut points,
            &mut contours,
        )
        .expect("two rings must flatten");
        assert_eq!(count, 2, "an outer ring and its counter are two contours");
        assert!(contours[0].1 <= contours[1].0, "the ranges must be disjoint and packed");
    }

    #[test]
    fn a_lone_move_contributes_no_contour() {
        let mut points = [Point::new(0, 0); MAX_OUTLINE_POINTS];
        let mut contours = [(0usize, 0usize); MAX_OUTLINE_CONTOURS];
        let count = flatten_paths(&["M480-480"], placement(24.0), &mut points, &mut contours)
            .expect("a lone move is valid and empty");
        assert_eq!(count, 0);
    }

    #[test]
    fn the_bundled_cross_outline_is_not_a_single_point() {
        // Regression: the cross outline mixes `m` (relative move) and bare relative line numbers.
        // A parser that read the bare numbers as more `m` arguments collapsed the whole icon to
        // one point, so the cross drew nothing while every other icon drew.
        let mut points = [Point::new(0, 0); MAX_OUTLINE_POINTS];
        let mut contours = [(0usize, 0usize); MAX_OUTLINE_CONTOURS];
        let count =
            flatten_paths(&[tests_data::CROSS], placement(24.0), &mut points, &mut contours)
                .expect("the bundled cross outline must flatten");
        assert!(count >= 3, "the cross is an X plus two rings, so at least three contours");
        for &(start, end) in &contours[..count] {
            assert!(end - start >= 3, "every contour of the cross bounds an area");
        }
    }
}
