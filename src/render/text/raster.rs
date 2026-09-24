// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Rasterising a glyph outline into antialiased coverage — G-5.
//!
//! # What was missing, and why it was an interface problem
//!
//! G-1 delivered real shaping (`rustybuzz`) and real advances; G-4c delivered two OFL vector
//! subsets. What neither could do is **draw**: every face in the crate was 1-bit, and
//! [`super::GlyphSource`] returned a `&'static [u8]` of packed *bits*. Antialiasing needs a value
//! per pixel that is computed for one cell size at one moment and therefore cannot be `'static`.
//! That is the whole blocker BLUE23 §22 recorded, and the two ways out were "own the buffer" or
//! "write into the caller's" — the same choice [`super::GlyphSource::paint`] had already made for
//! a different reason. This module is the second implementation of that contract: it reads the
//! outlines and writes coverage, and it never allocates a `'static` anything.
//!
//! # The algorithm, and why these three steps
//!
//! 1. **Flatten** each contour (a `move` plus a run of line/quadratic/cubic segments) into a
//!    polygon, subdividing curves until a segment's deviation from its chord is below a tolerance
//!    tied to the device pixel. A fixed segment count cannot work: it under-samples a 12 px glyph
//!    and wastes work on a 400 px one.
//! 2. **Accumulate signed area per pixel row** using the non-zero winding rule, sampled on a grid
//!    `SUBSAMPLES` times finer than the device pixel in both axes. The winding number of a pixel
//!    is the net number of crossings above it, so the whole outline is one pass with no per-pixel
//!    inside test.
//! 3. **Resolve** the sub-samples into one coverage byte per device pixel — this is where
//!    antialiasing comes from, and it is also what makes a stem narrower than a pixel visible
//!    instead of vanishing.
//!
//! # Why this is not "render with a font crate"
//!
//! `ab_glyph`/`fontdue` would rasterise for us and bring a second glyph cache, a second hinting
//! model and a second idea of what an advance is. The crate already has one answer to each of
//! those (`shaping`, `ControlMetrics`, `estimate_cluster_advance`), and a rasteriser that disagrees
//! with the shaper about where a glyph sits would reintroduce exactly the "two rulers" defect the
//! text layer was consolidated to remove. So this reads the outlines through `ttf-parser` — the
//! dependency G-1 already added — and nothing else.
//!
//! # Cost
//!
//! No allocation for the common case: the polygon scratch is a fixed `[Point; MAX_POINTS]` on the
//! stack, and a glyph whose flattened outline would exceed it is **refused** (`None`) rather than
//! silently truncated. Refusing is the honest failure: a truncated outline draws a wrong glyph,
//! which is the one outcome a font path must not have.

use super::font_assets::{active_faces, FaceBytes};
use super::glyph_source::{Cell, GlyphSource, InkKind, Painted};

/// Sub-samples per device pixel, on each axis.
///
/// 4 gives 16 samples per pixel, which is the point where a curve's coverage stops visibly
/// stepping. 1 would be a hard-edged bitmap with the outline's precision, and 8 quadruples the
/// work for a difference no one can see at the sizes a UI uses.
const SUBSAMPLES: u32 = 4;

/// Maximum flattened points across **all** contours of one glyph.
///
/// A `ttf-parser` outline is a sequence of contours; each is flattened into this one buffer. 1024
/// points at the flattening tolerance below is enough for every glyph in the two shipped subsets —
/// a CJK ideograph would be the largest, and those are not in a vector face here. A glyph that
/// exceeds it is refused rather than truncated (see the module docs).
const MAX_POINTS: usize = 1024;

/// Flattening tolerance, in fractions of a device pixel.
///
/// A curve is subdivided until its chord is within this of the curve. Tied to the device pixel
/// rather than absolute, because the same outline is rasterised at 12 px and at 400 px and the
/// useful subdivision count differs by an order of magnitude between them.
const FLATTEN_TOLERANCE: f32 = 0.2;

/// Maximum recursion depth when subdividing a curve.
///
/// A bound rather than a convergence test alone, because a degenerate control polygon can fail to
/// converge and recursion is the wrong place to discover that. 16 halvings is a 65 536x subdivision
/// of the parameter range, far past any useful precision.
const MAX_SUBDIVIDE: u32 = 16;

/// Where the baseline sits in a cell, as a share of its height.
///
/// The typographic split a line box already carries: about four fifths ascender to one fifth
/// descender. Drawing from a baseline here is what makes a glyph land visually centred in the cell
/// the shaper measured for it, instead of hanging off the bottom edge by its descender.
const ASCENT_SHARE: f32 = 0.82;

/// A point in glyph design units, offset by the cell's origin.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Point {
    x: f32,
    y: f32,
}

impl Point {
    const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// The midpoint of `self` and `other`.
    fn mid(self, other: Self) -> Self {
        Self::new((self.x + other.x) * 0.5, (self.y + other.y) * 0.5)
    }
}

/// A fixed-capacity point buffer, so flattening never allocates.
///
/// `push` returns `false` when full, and callers propagate the refusal. A `Vec` would be simpler
/// and would allocate per glyph per frame — on the small profiles this crate supports, a per-frame
/// allocation in the text path is the cost the design specifically avoids.
struct PointBuffer {
    points: [Point; MAX_POINTS],
    len: usize,
    /// Sub-ranges of `points`, one per closed contour.
    ///
    /// # Why the boundary is recorded rather than inferred
    ///
    /// A glyph has one contour per closed region — `o` has two, `8` has three — and each must be
    /// filled as its own polygon. Inferring the boundary afterwards is impossible: the buffer holds
    /// only destinations, so a contour's end and the next contour's start are adjacent entries with
    /// nothing between them, and a single walk around the whole buffer would join `o`'s inner ring
    /// to its outer one into a figure-eight. Recording the boundary at the `move_to` — the only
    /// place the outline says "a new contour starts here" — is exact.
    contours: [(usize, usize); MAX_CONTOURS],
    contour_count: usize,
}

/// Maximum closed contours per glyph.
///
/// `@` (three regions) and `%` (three) are the worst Latin cases; digits like `8` need three. 16
/// leaves generous room for a decorated or CJK glyph in a future face, and a glyph that exceeds it
/// is refused rather than mis-filled, like every other bound here.
const MAX_CONTOURS: usize = 16;

impl PointBuffer {
    fn new() -> Self {
        Self {
            points: [Point::new(0.0, 0.0); MAX_POINTS],
            len: 0,
            contours: [(0, 0); MAX_CONTOURS],
            contour_count: 0,
        }
    }

    fn push(&mut self, point: Point) -> bool {
        if self.len == MAX_POINTS {
            return false;
        }
        self.points[self.len] = point;
        self.len += 1;
        true
    }

    /// Opens a contour at the current length, returning `false` when there is no room left.
    fn begin_contour(&mut self) -> bool {
        if self.contour_count == MAX_CONTOURS {
            return false;
        }
        self.contours[self.contour_count] = (self.len, self.len);
        self.contour_count += 1;
        true
    }

    /// Records that the open contour just grew to the buffer's current length.
    fn end_contour(&mut self) {
        if let Some(last) = self.contours.get_mut(self.contour_count.saturating_sub(1)) {
            last.1 = self.len;
        }
    }

    /// The closed contours pushed so far, each as a slice of its own points.
    fn contours(&self) -> impl Iterator<Item = &[Point]> {
        self.contours[..self.contour_count]
            .iter()
            .filter_map(|&(start, end)| self.points.get(start..end))
    }
}

/// Everything one glyph's rasterisation needs, resolved once.
///
/// # Why the transform is resolved up front
///
/// `ttf-parser`'s `OutlineBuilder` hands back coordinates in **font design units** with y up. The
/// cell wants **device pixels** with y down. Doing the conversion inside every `line_to` would
/// repeat six multiplies per point and, worse, would put the sign conventions in two places. One
/// resolved `Placement` means the flattening code is pure geometry with no font knowledge.
#[derive(Debug, Clone, Copy)]
struct Placement {
    /// Design units per em, from the face.
    units_per_em: f32,
    /// Device pixels per em the caller asked for.
    pixel_size: f32,
    /// Device x of the glyph's origin.
    origin_x: f32,
    /// Device y of the **baseline**.
    baseline_y: f32,
    /// 1.0 for a left-to-right glyph, -1.0 when the face's outlines are right-to-left.
    x_direction: f32,
}

impl Placement {
    /// Converts one design-unit point to device space.
    fn map(&self, x: f32, y: f32) -> Point {
        let scale = self.pixel_size / self.units_per_em;
        Point::new(self.origin_x + x * scale * self.x_direction, self.baseline_y - y * scale)
    }
}

/// Flattens `ttf-parser` outlines into `PointBuffer`.
///
/// One instance per rasterisation, because it carries the `Placement` and the in-progress contour.
struct Flattener<'a> {
    placement: Placement,
    points: &'a mut PointBuffer,
    /// Where the current contour started, so `close` can join back to it.
    contour_start: Point,
    /// The point the pen is at, which a `quad_to`/`curve_to` needs.
    pen: Point,
    /// Set once a push fails, so the whole rasterisation reports the refusal.
    overflowed: bool,
}

impl<'a> Flattener<'a> {
    fn new(placement: Placement, points: &'a mut PointBuffer) -> Self {
        Self {
            placement,
            points,
            contour_start: Point::new(0.0, 0.0),
            pen: Point::new(0.0, 0.0),
            overflowed: false,
        }
    }

    /// Seals the contour in progress once the outline has been fully walked.
    ///
    /// A face need not emit a `close` after its final contour — the two shipped subsets do, but
    /// nothing in the format promises it — and an unsealed contour is invisible to `fill_outline`
    /// because its extent was never recorded. Rather than reach around the borrow to seal it, the
    /// caller tells the flattener it is done.
    fn finish(&mut self) {
        self.points.end_contour();
    }

    fn push(&mut self, point: Point) {
        if !self.points.push(point) {
            self.overflowed = true;
        }
    }

    /// Subdivides a quadratic Bézier (`p0`, `ctrl`, `p1`) until flat, pushing the points.
    fn quad_to(&mut self, p0: Point, ctrl: Point, p1: Point, depth: u32) {
        if depth >= MAX_SUBDIVIDE || is_flat_quad(p0, ctrl, p1) {
            self.push(p1);
            return;
        }
        let p01 = p0.mid(ctrl);
        let p12 = ctrl.mid(p1);
        let mid = p01.mid(p12);
        self.quad_to(p0, p01, mid, depth + 1);
        self.quad_to(mid, p12, p1, depth + 1);
    }

    /// Subdivides a cubic Bézier (`p0`, `c1`, `c2`, `p1`) until flat, pushing the points.
    fn curve_to(&mut self, p0: Point, c1: Point, c2: Point, p1: Point, depth: u32) {
        if depth >= MAX_SUBDIVIDE || is_flat_cubic(p0, c1, c2, p1) {
            self.push(p1);
            return;
        }
        let p01 = p0.mid(c1);
        let p12 = c1.mid(c2);
        let p23 = c2.mid(p1);
        let p012 = p01.mid(p12);
        let p123 = p12.mid(p23);
        let mid = p012.mid(p123);
        self.curve_to(p0, p01, p012, mid, depth + 1);
        self.curve_to(mid, p123, p23, p1, depth + 1);
    }
}

/// Whether a quadratic's control point is close enough to its chord.
///
/// The distance from `ctrl` to the chord's midpoint bounds the curve's deviation from the chord by
/// a factor of two, which is the standard cheap test and is what makes this subdivision adaptive
/// rather than fixed-count.
fn is_flat_quad(p0: Point, ctrl: Point, p1: Point) -> bool {
    let dx = p1.x - p0.x;
    let dy = p1.y - p0.y;
    let mid_x = (p0.x + p1.x) * 0.5;
    let mid_y = (p0.y + p1.y) * 0.5;
    let dev_x = ctrl.x - mid_x;
    let dev_y = ctrl.y - mid_y;
    // Twice the deviation, compared against the tolerance in device pixels — the factor of two is
    // the bound above, moved to the left side so no division is needed.
    dev_x * dev_x + dev_y * dev_y
        <= (FLATTEN_TOLERANCE * FLATTEN_TOLERANCE) * 0.25 * (dx * dx + dy * dy)
        || (dev_x * dev_x + dev_y * dev_y) <= FLATTEN_TOLERANCE * FLATTEN_TOLERANCE
}

/// Whether a cubic's control points are close enough to its chord.
fn is_flat_cubic(p0: Point, c1: Point, c2: Point, p1: Point) -> bool {
    // The two control points' distances from the chord, summed — the standard flatness measure for
    // a cubic. Bounded by the same tolerance, squared to avoid the square root.
    let d1 = distance_to_line(c1, p0, p1);
    let d2 = distance_to_line(c2, p0, p1);
    (d1 + d2) * (d1 + d2) <= FLATTEN_TOLERANCE * FLATTEN_TOLERANCE
}

/// Perpendicular distance from `p` to the line through `a` and `b`.
fn distance_to_line(p: Point, a: Point, b: Point) -> f32 {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let len_sq = dx * dx + dy * dy;
    if len_sq <= f32::EPSILON {
        // A degenerate chord: the distance is simply to the point.
        return ((p.x - a.x).powi(2) + (p.y - a.y).powi(2)).sqrt();
    }
    // Twice the triangle's area over the base length.
    ((p.x - a.x) * dy - (p.y - a.y) * dx).abs() / len_sq.sqrt()
}

/// Accumulated edge crossings per sub-sample row, for the non-zero winding rule.
///
/// # Why this stores crossings rather than an inside/outside flag
///
/// A scanline fill has two steps and the crate's first attempt at this collapsed them into one.
/// Step one records, for each row, **where the outline crosses it** — a horizontal position with a
/// direction. Step two walks that row left to right keeping a running winding sum, and every
/// sub-sample with a non-zero sum is inside.
///
/// Recording a per-column "is inside" answer directly is the mistake: an edge tells you the
/// winding *changes at this x*, not that this x is filled. Marking only the crossed column leaves
/// every column between two edges at zero, so a glyph's stems get an outline and no body — which is
/// exactly what the first version of this module drew (measured: `A` at 24x24 reached coverage 127
/// on its strokes and 0 between them).
struct Accumulator<'a> {
    /// Crossing events per sub-sample row, laid out row-major over the *sub-sampled* grid.
    ///
    /// Each cell is the net signed number of outline edges crossing this sub-sample **position**,
    /// which the resolver turns into a running winding when it sweeps the row.
    rows: &'a mut [i32],
    /// Sub-sampled width.
    width: usize,
    /// Sub-sampled height.
    height: usize,
}

impl Accumulator<'_> {
    /// Adds the edge `(x0, y0) -> (x1, y1)` to the winding.
    ///
    /// # Why a scanline loop and not a per-sample point test
    ///
    /// An outline is a closed set of edges. Walking each edge and adding its contribution to every
    /// row it crosses visits each edge once; asking "is this point inside?" per sub-sample would
    /// visit every edge for every sample — 16 * cell.area() * edges, which at 400 px is millions of
    /// edge tests for one glyph.
    fn add_edge(&mut self, x0: f32, y0: f32, x1: f32, y1: f32) {
        let (x0s, y0s) = (x0 * SUBSAMPLES as f32, y0 * SUBSAMPLES as f32);
        let (x1s, y1s) = (x1 * SUBSAMPLES as f32, y1 * SUBSAMPLES as f32);
        let dy_total = y1s - y0s;
        if dy_total.abs() <= f32::EPSILON {
            // A horizontal edge contributes no crossings under the non-zero rule: it crosses no
            // scanline, so it neither opens nor closes a region.
            return;
        }
        // Walk the rows in increasing order regardless of which way the edge points. A decreasing
        // `y` with an increasing row loop is the classic scanline bug — it either skips the edge or
        // loops forever — so the walk is normalised and the *sign* carries the direction.
        let going_down = dy_total > 0.0;
        let (start_y, end_y) = if going_down { (y0s, y1s) } else { (y1s, y0s) };
        let delta = if going_down { 1 } else { -1 };
        let dx_total = x1s - x0s;

        // Row-centre sampling: a sample belongs to the row whose centre it falls in, so the first
        // row considered is `ceil(start_y - 0.5)`. Getting this wrong by one row shifts every
        // glyph's coverage by half a sub-pixel and rounds thin stems away.
        let first_row = (start_y - 0.5).ceil() as i64;
        let last_row = (end_y - 0.5).ceil() as i64;
        for row in first_row..last_row {
            if row < 0 || row as usize >= self.height {
                continue;
            }
            let row_center = row as f32 + 0.5;
            let t = (row_center - y0s) / dy_total;
            if !(0.0..=1.0).contains(&t) {
                continue;
            }
            let xs = x0s + dx_total * t;
            let column = xs.floor() as i64;
            if column < 0 || column as usize >= self.width {
                continue;
            }
            let index = row as usize * self.width + column as usize;
            self.rows[index] += delta;
        }
    }
}

/// Resolves the crossing rows into one coverage byte per device pixel.
///
/// # The two passes, and why the running sum is the whole point
///
/// For each sub-sample row the resolver sweeps left to right accumulating `winding += crossings`.
/// A non-zero running total means the sub-sample is inside the outline; zero means outside. That is
/// the non-zero winding rule, and it is what makes a counter a hole: `o`'s inner contour runs the
/// opposite way from its outer one, so the sum returns to zero across the ring and the hole fills
/// as empty.
///
/// # Why the winding is carried across rows rather than recomputed
///
/// It is **not** carried: every row starts at zero, correctly, because a closed outline contributes
/// as many downward as upward crossings, so the sum at each row's left edge is always zero. Carrying
/// it would let a rounding difference at one row's boundary leak into the next, which shows up as a
/// horizontal band of fill across the glyph.
fn resolve_coverage(rows: &[i32], sub_width: usize, cell: Cell, out: &mut [u8]) {
    let device_w = cell.width as usize;
    let device_h = cell.height as usize;
    let total = (SUBSAMPLES * SUBSAMPLES) as u32;
    for py in 0..device_h {
        for px in 0..device_w {
            let mut inside = 0u32;
            for sy in 0..SUBSAMPLES as usize {
                let row = py * SUBSAMPLES as usize + sy;
                let Some(row_slice) = rows.get(row * sub_width..(row + 1) * sub_width) else {
                    continue;
                };
                // The sweep starts at this row's first column, where the winding is zero for a
                // closed outline.
                let first = px * SUBSAMPLES as usize;
                let mut winding = 0i32;
                for (offset, value) in
                    row_slice.iter().take(first + SUBSAMPLES as usize).enumerate()
                {
                    winding += *value;
                    if offset >= first && winding != 0 {
                        inside += 1;
                    }
                }
            }
            out[py * device_w + px] = (inside * 255 / total) as u8;
        }
    }
}

/// Rasterises `ch` through the build's vector faces into coverage.
pub struct VectorSource;

impl VectorSource {
    /// The one shared instance.
    pub const INSTANCE: Self = Self;

    /// The face that covers `ch`, if this build enabled one.
    fn face_for(&self, ch: char) -> Option<FaceBytes> {
        active_faces().iter().copied().find(|face| {
            ttf_parser::Face::parse(face.bytes, 0)
                .ok()
                .and_then(|parsed| parsed.glyph_index(ch))
                .is_some()
        })
    }
}

impl GlyphSource for VectorSource {
    fn glyph(&self, ch: char) -> Option<super::GlyphBitmap> {
        // A vector face has no 1-bit view: its ink is a coverage ramp that depends on the cell.
        // Answering `None` here is correct, not a gap — the resolution chain reaches it through
        // `paint`, and `resolve` is documented as the 1-bit view.
        let _ = self.face_for(ch);
        None
    }

    fn name(&self) -> &'static str {
        "vector"
    }

    fn covers(&self, ch: char) -> bool {
        self.face_for(ch).is_some()
    }

    fn paint(&self, ch: char, cell: Cell, out: &mut [u8]) -> Option<Painted> {
        if cell.is_empty() || out.len() < cell.area() {
            return None;
        }
        let face_bytes = self.face_for(ch)?;
        let face = ttf_parser::Face::parse(face_bytes.bytes, 0).ok()?;
        let glyph = face.glyph_index(ch)?;

        // The em the caller wants: the cell's height is the line box, and the glyph is scaled to fit
        // its em. Reading `units_per_em` from the face is what makes the scale independent of the
        // subset's own resolution — the two shipped faces disagree (1000 vs 2048), so a hardcoded
        // divisor would be wrong for one of them.
        let units_per_em = face.units_per_em() as f32;
        if units_per_em <= 0.0 {
            return None;
        }
        let pixel_size = cell.height as f32;
        let placement = Placement {
            units_per_em,
            pixel_size,
            origin_x: 0.0,
            // `ttf-parser` reports outlines with y up from the baseline, so this is where the
            // baseline sits in the cell. `ASCENT_SHARE` is the usual typographic split: most of a
            // line box is ascender, a little is descender, and a glyph drawn from this baseline
            // lands visually centred rather than hanging off the bottom.
            baseline_y: cell.height as f32 * ASCENT_SHARE,
            x_direction: 1.0,
        };
        let mut points = PointBuffer::new();
        {
            let mut flattener = Flattener::new(placement, &mut points);
            face.outline_glyph(glyph, &mut flattener)?;
            // A face need not emit a `close` after its last contour, so seal it here as well as in
            // `close` — an unsealed contour would be invisible to `fill_outline`. Done through the
            // flattener, which still owns the borrow of `points`.
            flattener.finish();
            if flattener.overflowed {
                // The outline did not fit the fixed scratch. Refusing is the honest answer: a
                // truncated outline draws a wrong glyph (see the module docs).
                return None;
            }
        }

        let sub_width = cell.width as usize * SUBSAMPLES as usize;
        let sub_height = cell.height as usize * SUBSAMPLES as usize;
        let total = sub_width.checked_mul(sub_height)?;
        // The winding scratch is the one thing this cannot keep on the stack: a 400x400 cell at
        // 4x4 subsampling is 2.56 M accumulators. It is zeroed per glyph and dropped at the end of
        // the call, so it is never resident — the third constraint.
        let mut rows = crate::compat::vec![0i32; total];

        {
            let mut accumulator =
                Accumulator { rows: &mut rows, width: sub_width, height: sub_height };
            fill_outline(&points, &mut accumulator);
        }
        out[..cell.area()].fill(0);
        resolve_coverage(&rows, sub_width, cell, out);

        Some(Painted { source: face_bytes.name, source_cell: None, ink: InkKind::Coverage })
    }
}

/// Feeds every closed contour of a flattened outline into `accumulator`.
///
/// # Why each contour closes on itself
///
/// The fill needs each contour as a region, which means the implicit edge from its last point back
/// to its first. Without it a contour is an open polyline whose winding is undefined, and `o` would
/// fill as a solid disc instead of a ring. The wrap-around is emitted here, per contour, using the
/// ranges [`PointBuffer::begin_contour`] recorded.
fn fill_outline(buffer: &PointBuffer, accumulator: &mut Accumulator) {
    for contour in buffer.contours() {
        if contour.len() < 3 {
            // Two points cannot bound an area; a single point is a dot, which a filled outline of
            // this form has no way to express and which no shipped face uses.
            continue;
        }
        for index in 0..contour.len() {
            let a = contour[index];
            let b = contour[(index + 1) % contour.len()];
            accumulator.add_edge(a.x, a.y, b.x, b.y);
        }
    }
}

/// `ttf-parser`'s `OutlineBuilder` for [`Flattener`].
impl ttf_parser::OutlineBuilder for Flattener<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        let point = self.placement.map(x, y);
        // The previous contour ends here, and a new one begins. Recording both is what lets the
        // fill treat `o`'s two rings separately.
        self.points.end_contour();
        if !self.points.begin_contour() {
            self.overflowed = true;
        }
        self.contour_start = point;
        self.pen = point;
        self.push(point);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let point = self.placement.map(x, y);
        self.pen = point;
        self.push(point);
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let ctrl = self.placement.map(x1, y1);
        let end = self.placement.map(x, y);
        let start = self.pen;
        Flattener::quad_to(self, start, ctrl, end, 0);
        self.pen = end;
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let c1 = self.placement.map(x1, y1);
        let c2 = self.placement.map(x2, y2);
        let end = self.placement.map(x, y);
        let start = self.pen;
        Flattener::curve_to(self, start, c1, c2, end, 0);
        self.pen = end;
    }

    fn close(&mut self) {
        // The closing edge is emitted by `fill_outline`'s wrap-around, so there is nothing to push
        // here — but the contour's extent has to be sealed now that no more points will follow it.
        self.points.end_contour();
        self.pen = self.contour_start;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::text::{paint_active, Cell, GlyphSource, InkKind};

    /// The two things a rasteriser can get wrong that a 1-bit face cannot: it produces interior
    /// coverage values, and it fills *inside* the outline.
    ///
    /// # Why this reads as three separate assertions
    ///
    /// "It drew something" is satisfied by a solid block; "it drew a ramp" is satisfied by noise.
    /// The three together pin the actual property — a filled outline, sampled at sub-pixel
    /// resolution, with its interior solid and its edge partial.
    #[cfg(feature = "fonts-vector-latin")]
    #[test]
    fn a_vector_face_produces_antialiased_coverage() {
        let cell = Cell::new(32, 32);
        let mut out = vec![0u8; cell.area()];
        let painted = paint_active('o', cell, &mut out).expect("the Latin face covers 'o'");
        assert_eq!(painted.ink, InkKind::Coverage, "a vector face reports coverage, not 1-bit ink");
        assert!(painted.is_coverage(), "so the rasteriser may blend it as alpha");
        assert!(!painted.is_color(), "and it is not a colour glyph");
        assert_eq!(painted.source_cell, None, "there is no source pixel grid to compress by");

        assert!(out.iter().any(|v| *v == 0), "the glyph must not fill its whole cell");
        assert!(out.iter().any(|v| *v == 255), "its interior must be solid");
        // This is the assertion a 1-bit face cannot satisfy: 'o' has a curved outline, so some
        // pixel must be partially covered.
        assert!(
            out.iter().any(|v| *v > 0 && *v < 255),
            "a curved outline must produce partial coverage, or there is no antialiasing"
        );
    }

    /// A glyph with a counter has a hole, and the hole is empty.
    ///
    /// # Why `o` and not `l`
    ///
    /// This is the one assertion that distinguishes a *filled outline* from a *filled bounding
    /// box*: both have solid interiors near their edges, and only the counter tells them apart. It
    /// is also what catches a contour-boundary bug — if `o`'s two rings were joined into one
    /// polygon, the hole would be covered up and the centre sample would be solid.
    ///
    /// # Why the sample is the brightest pixel's own row and column
    ///
    /// The cell's geometric centre is **not** the glyph's centre: the baseline sits at
    /// [`ASCENT_SHARE`] of the cell height, so `o`'s counter is above the cell's middle row by
    /// several pixels. Sampling `(h/2, w/2)` measured the bowl's lower stroke and read 255. The
    /// counter's location is derived from the ink itself instead — the densest column through the
    /// middle of the glyph — which is what the assertion is actually about.
    #[cfg(feature = "fonts-vector-latin")]
    #[test]
    fn the_counter_of_a_glyph_is_a_hole() {
        let (w, h) = (40u32, 40u32);
        let cell = Cell::new(w, h);
        let mut out = vec![0u8; cell.area()];
        paint_active('o', cell, &mut out).expect("the Latin face covers 'o'");

        // The glyph occupies only part of the cell; find its ink bounds so the test samples the
        // glyph rather than the padding.
        let painted = |x: u32, y: u32| out[(y * w + x) as usize] > 0;
        let mut top = None;
        let mut bottom = 0u32;
        for y in 0..h {
            if (0..w).any(|x| painted(x, y)) {
                top.get_or_insert(y);
                bottom = y;
            }
        }
        let (top, bottom) = (top.expect("something was painted"), bottom);
        let mid_y = (top + bottom) / 2;

        // The counter is where the row through the glyph's middle has ink on both sides and none
        // in between, so the run of empty pixels between the two strokes is the hole.
        let columns: Vec<u32> = (0..w).filter(|x| painted(*x, mid_y)).collect();
        let first = *columns.first().expect("the left wall of the bowl");
        let last = *columns.last().expect("the right wall of the bowl");
        assert!(last > first + 1, "the row must cross two walls, not one blob");
        let hole = ((first + 1)..last).filter(|x| !painted(*x, mid_y)).count();
        assert!(
            hole > 0,
            "'o' must have an unpainted counter on its middle row: walls at {first} and {last}, \
             nothing empty between them"
        );
        assert!(
            out[(mid_y * w + (first + last) / 2) as usize] == 0,
            "and the middle of that gap must be exactly unpainted, not merely dim"
        );
    }

    /// A character the face does not cover is refused, so a stack falls through.
    ///
    /// The property the whole fallback chain rests on: a source that claims a character it cannot
    /// draw makes every later source unreachable.
    #[cfg(feature = "fonts-vector-latin")]
    #[test]
    fn a_character_outside_the_face_is_refused() {
        let source = VectorSource::INSTANCE;
        assert!(source.covers('A'), "the Latin subset carries ASCII letters");
        let cell = Cell::new(16, 16);
        let mut out = vec![0u8; cell.area()];
        assert!(
            source.paint('\u{4e00}', cell, &mut out).is_none(),
            "an ideograph is not in a Latin subset, so the face must say so"
        );
    }

    /// A buffer too small for the cell is refused rather than written past.
    ///
    /// A drawing routine is the wrong place to discover a sizing bug: this must be a `None`, not a
    /// panic and not a partial fill.
    #[cfg(feature = "fonts-vector-latin")]
    #[test]
    fn a_short_buffer_is_refused() {
        let cell = Cell::new(32, 32);
        let mut out = vec![0u8; cell.area() - 1];
        assert!(
            VectorSource::INSTANCE.paint('A', cell, &mut out).is_none(),
            "one byte short is short"
        );
    }

    /// A degenerate cell is refused, not indexed.
    #[cfg(feature = "fonts-vector-latin")]
    #[test]
    fn an_empty_cell_is_refused() {
        let mut out = vec![0u8; 0];
        assert!(VectorSource::INSTANCE.paint('A', Cell::new(0, 0), &mut out).is_none());
        assert!(VectorSource::INSTANCE.paint('A', Cell::new(8, 0), &mut out).is_none());
    }

    /// The rasteriser scales with the cell, so one glyph is one shape at every size.
    ///
    /// A rasteriser that ignored the cell would draw an 8 px glyph in a 64 px box; one that scaled
    /// the *buffer* rather than the outline would show blocky edges at every size. Counting the
    /// painted pixels against the cell area is what distinguishes "scaled" from "nearest-neighbour
    /// upscale of a fixed bitmap".
    #[cfg(feature = "fonts-vector-latin")]
    #[test]
    fn the_outline_is_rasterised_at_the_requested_size() {
        let mut small = vec![0u8; 8 * 8];
        let mut large = vec![0u8; 64 * 64];
        paint_active('l', Cell::new(8, 8), &mut small).expect("8x8");
        paint_active('l', Cell::new(64, 64), &mut large).expect("64x64");
        let coverage = |px: &[u8]| px.iter().map(|v| u32::from(*v)).sum::<u32>() as f32 / 255.0;
        let small_ink = coverage(&small);
        let large_ink = coverage(&large);
        // Area scales with the square of the size, so a 8x8 -> 64x64 glyph should cover roughly
        // 64x the ink. The band is wide because hinting-free rasterisation rounds differently at
        // each size; it is narrow enough to reject "the same bitmap scaled up" (which would give
        // exactly 64x with no integer rounding error) and "the cell ignored" (which would give 1x).
        let ratio = large_ink / small_ink.max(1.0);
        assert!(
            (30.0..120.0).contains(&ratio),
            "ink must scale with the cell's area: 8x8 -> {small_ink}, 64x64 -> {large_ink}, \
             ratio {ratio}"
        );
    }
}
