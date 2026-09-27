// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! SVG path-data (`d`) parsing — the accessor that turns an icon's path into curve segments.
//!
//! # Why this exists, and what it deliberately does not do
//!
//! An icon is a set of filled outlines; this crate stores those outlines as SVG `d` strings (see
//! `docs/plans/blue25.md` §6). Turning a `d` into pixels needs two things: **reading the commands**
//! and **flattening the curves**. This module does only the first. Flattening — subdividing a curve
//! until its chord is within a device-pixel tolerance — is already implemented, tuned and tested in
//! [`crate::render::text::raster`], so writing a second one here would be two implementations of one
//! concept (principle #101). [`Subpath`] therefore yields **segments**, and the caller hands them to
//! whatever flattener it already uses.
//!
//! This is also *not* the reader in `crate::widget::svg::path_bounds`, which is a private,
//! `M`/`L`/`H`/`V`/`Z`-only, test-side bounds reader. That one answers "where did this rectangle
//! land"; this one answers "what curves does this icon describe", and must handle `C`, `Q`, `A` and
//! every relative form.
//!
//! # Commands
//!
//! Supported: `M`/`m`, `L`/`l`, `H`/`h`, `V`/`v`, `C`/`c`, `S`/`s`, `Q`/`q`, `T`/`t`, `A`/`a`,
//! `Z`/`z`. This is the full set Material Symbols and every mainstream icon set emit — `S` and
//! `T` are not optional: the bundled `cross` outline uses `T`, and refusing it left the icon
//! unrendered. An unsupported command is an `Err`, not a skip: a `d` this module cannot read fully
//! must not paint a partial icon, because a half-drawn icon is a wrong icon. (Contrast
//! `path_bounds`, where ignoring the unknown parts is the documented, narrow behaviour.)
//!
//! `S` and `T` are the "smooth" forms: their first control point is the reflection of the previous
//! curve's last control point, so the parser tracks that point as it goes. A `S`/`T` whose previous
//! segment was not a cubic/quadratic reflects the pen itself, which is the spec's rule for the case
//! where there is no previous control point to reflect.
//!
//! # Why `A` becomes cubics rather than staying an `Arc`
//!
//! An elliptical arc is the one SVG command the flatteners in this crate take no opinion on: the
//! rasteriser's `Flattener` subdivides quadratics and cubics, and knows nothing of arc
//! parametrisation. Leaving an `Arc` in a [`Subpath`] would therefore push the conversion onto
//! every caller — and the icon path is one of those callers, so the arc would be **dropped** rather
//! than drawn. That is not a hypothesis: `Icon` accepted exactly this conversion when it was the
//! first consumer, and an `Arc` left unhandled is an icon with a missing piece (the module rule
//! above — a partial icon is a wrong icon).
//!
//! So the arc is converted here, once, to the cubic segments that approximate it (SVG
//! implementation notes F.6.5): the centre and angles are recovered from the endpoint
//! parametrisation, the sweep is split at 90-degree boundaries, and each piece becomes one cubic
//! whose error is under 0.03 % of the radius — far below a device pixel at icon sizes. The result
//! is that a [`Subpath`] carries only the curves the rasteriser's flattener already handles, and
//! there is **one** arc conversion in the crate (principle #101) instead of one per consumer.

use crate::compat::Vec;

/// Maximum on-curve points one parsed path may yield.
///
/// The same order as the rasteriser's `MAX_POINTS`: a 24-unit icon flattens to a few dozen points per
/// subpath, so this leaves ample room for the multi-subpath icons in a standard set while staying a
/// hard bound rather than a growth.
pub const MAX_PATH_POINTS: usize = 1024;

/// One segment of a subpath, with its end point.
///
/// Control points are absolute, in the path's own coordinate space. The start point of a segment is
/// the previous segment's end (or the subpath start for the first), which the caller tracks — the
/// same model the rasteriser's flattener consumes, so no conversion layer is needed between them.
///
/// # Why there is no `Arc` variant
///
/// Every variant here is a curve the rasteriser's flattener can subdivide; an `Arc` could not be,
/// so it would have to be converted before it could be drawn — by each caller, or by none of them.
/// The conversion is done once in [`parse`] and the result is cubics, so a consumer of a [`Subpath`]
/// never has to handle arcs at all. See the module docs for why that direction was chosen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Segment {
    /// A straight line to the end point.
    Line(Point),
    /// A quadratic Bézier: one control point, then the end point.
    Quad {
        /// The single control point.
        ctrl: Point,
        /// The end point.
        to: Point,
    },
    /// A cubic Bézier: two control points, then the end point.
    Cubic {
        /// The first control point.
        ctrl1: Point,
        /// The second control point.
        ctrl2: Point,
        /// The end point.
        to: Point,
    },
}

impl Segment {
    /// The point the segment ends at.
    pub fn end(&self) -> Point {
        match *self {
            Segment::Line(to) | Segment::Quad { to, .. } | Segment::Cubic { to, .. } => to,
        }
    }
}

/// An absolute point in the path's coordinate space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    /// Horizontal coordinate.
    pub x: f32,
    /// Vertical coordinate.
    pub y: f32,
}

impl Point {
    /// Creates a point.
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// One closed or open contour: where it started, and the segments after that start.
#[derive(Debug, Clone, PartialEq)]
pub struct Subpath {
    /// The point the first segment starts from (the `M`/`m` destination).
    pub start: Point,
    /// The segments that follow the start point, in order.
    pub segments: Vec<Segment>,
    /// Whether the contour was closed with `Z`.
    ///
    /// A closed contour is filled as a ring — the flattener adds the closing edge. An open one is
    /// still filled by the non-zero rule (SVG fills open subpaths by implicitly closing them), so
    /// this flag changes the geometry only when a caller wants to stroke instead of fill.
    pub closed: bool,
}

/// Why a `d` could not be parsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathError {
    /// A command letter this parser does not support.
    UnknownCommand(char),
    /// A number was expected and the input ended or held something else.
    ExpectedNumber,
    /// A subpath's segments appeared before any `M`/`m` established a start point.
    SegmentsBeforeMove,
    /// The path would need more than [`MAX_PATH_POINTS`] points.
    TooManyPoints,
}

/// Parses SVG path data into subpaths.
///
/// Returns `Err` on the first problem rather than painting a partial icon; see the module docs for
/// why partial is worse than absent here.
///
/// `input` is the raw `d` attribute. Whitespace and commas are both separators, and a command letter
/// may be followed by repeated argument sets (`M0 0 L1 1 2 2` is two line segments) — both are part
/// of the SVG grammar and both appear in real icon data.
pub fn parse(input: &str) -> Result<Vec<Subpath>, PathError> {
    let mut parser = Parser { bytes: input.as_bytes(), at: 0 };
    let mut subpaths: Vec<Subpath> = Vec::new();
    let mut current: Option<Subpath> = None;
    // The pen, and the subpath's own start (for closing and for the relative-to-start commands).
    let mut pen = Point::new(0.0, 0.0);
    let mut subpath_start = Point::new(0.0, 0.0);
    let mut total_points = 0usize;
    // The reflection state the smooth commands (`S`/`s`, `T`/`t`) read: the previous curve's last
    // control point, and whether the previous segment was of the matching kind. SVG defines `S` as
    // "reflect the previous cubic's second control point", and the reflection falls back to the pen
    // when the previous segment was not a cubic — so the kind of the previous segment is state too.
    let mut last_cubic_ctrl: Option<Point> = None;
    let mut last_quad_ctrl: Option<Point> = None;

    parser.skip_separators();
    // A `Z` is the only command with no arguments, so it does not enter the argument loop.
    while !parser.at_end() {
        let command = parser.read_command()?;
        let relative = command.is_ascii_lowercase();

        match command.to_ascii_uppercase() {
            'Z' => {
                let Some(subpath) = current.as_mut() else {
                    return Err(PathError::SegmentsBeforeMove);
                };
                subpath.closed = true;
                // `Z` returns the pen to the subpath start; a following relative command is
                // measured from there.
                pen = subpath_start;
                parser.skip_separators();
                continue;
            }
            'M' => {
                // The first argument set is the move itself; a repeated one is a line (SVG's rule
                // for the one command whose repetition changes meaning).
                if let Some(done) = current.take() {
                    subpaths.push(done);
                }
                let point = parser.read_point()?;
                pen = if relative { Point::new(pen.x + point.x, pen.y + point.y) } else { point };
                subpath_start = pen;
                // A move breaks the reflection chain: there is no previous curve to reflect.
                last_cubic_ctrl = None;
                last_quad_ctrl = None;
                current = Some(Subpath { start: pen, segments: Vec::new(), closed: false });
                total_points += 1;
                if total_points > MAX_PATH_POINTS {
                    return Err(PathError::TooManyPoints);
                }
                // Any further argument sets of this `M` are implicit lines.
                while parser.number_pending() {
                    let segments = parse_segments('L', &mut parser, relative, pen, pen)?;
                    total_points += segments.len();
                    if total_points > MAX_PATH_POINTS {
                        return Err(PathError::TooManyPoints);
                    }
                    let subpath = current.as_mut().expect("a move opened a subpath");
                    for segment in segments {
                        pen = segment.end();
                        // An implicit line after `M` reflects as a line would: nothing to reflect.
                        last_cubic_ctrl = None;
                        last_quad_ctrl = None;
                        subpath.segments.push(segment);
                    }
                }
                parser.skip_separators();
                continue;
            }
            _ => {}
        }

        if current.is_none() {
            return Err(PathError::SegmentsBeforeMove);
        }
        // One argument set is mandatory (the command letter alone is malformed); any further sets
        // repeat it, which is SVG's rule. An `A` produces several segments, so the loop consumes a
        // run rather than one.
        loop {
            let segments = parse_segments_with_reflection(
                command,
                &mut parser,
                relative,
                pen,
                last_cubic_ctrl,
                last_quad_ctrl,
            )?;
            total_points += segments.len();
            if total_points > MAX_PATH_POINTS {
                return Err(PathError::TooManyPoints);
            }
            let subpath = current.as_mut().expect("checked above");
            for segment in segments {
                pen = segment.end();
                match segment {
                    Segment::Cubic { ctrl2, .. } => last_cubic_ctrl = Some(ctrl2),
                    Segment::Quad { ctrl, .. } => last_quad_ctrl = Some(ctrl),
                    Segment::Line(_) => {}
                }
                // A line, or a curve of the other kind, breaks its counterpart's reflection chain.
                if !matches!(segment, Segment::Cubic { .. }) {
                    last_cubic_ctrl = None;
                }
                if !matches!(segment, Segment::Quad { .. }) {
                    last_quad_ctrl = None;
                }
                subpath.segments.push(segment);
            }
            if !parser.number_pending() {
                break;
            }
            // A repeated argument set repeats the command only when the *next* token is a number
            // and not a new command letter. SVG's grammar makes this explicit: the repeat form
            // applies "if the next token is not a command, the previous command is repeated".
            // Without this rule a path like `m1 1 2 2` — a relative move followed by a relative
            // line, two commands with no letter between them — was read as four arguments of one
            // `m`, which is a different shape. Real icon data uses the form freely, so the rule is
            // load-bearing rather than a nicety. `min` is meaningless here because both are cubic.
            if parser.command_pending() {
                break;
            }
        }
        parser.skip_separators();
    }

    if let Some(done) = current.take() {
        subpaths.push(done);
    }
    Ok(subpaths)
}

/// Parses one argument set of a segment-producing command.
///
/// `cursor_ctrl` is the control point a smooth command (`S`/`T`) reflects — the caller resolves it,
/// because only the caller knows the previous segment. For every other command it is unused.
fn parse_segment(
    command: char,
    parser: &mut Parser<'_>,
    relative: bool,
    pen: Point,
    cursor_ctrl: Point,
) -> Result<Segment, PathError> {
    // A coordinate is relative to the pen for a lower-case command, and absolute for an upper-case
    // one. `H`/`V` take a single number, so they are read before the pair helper.
    let pair = |parser: &mut Parser<'_>| -> Result<Point, PathError> {
        let point = parser.read_point()?;
        if relative {
            Ok(Point::new(pen.x + point.x, pen.y + point.y))
        } else {
            Ok(point)
        }
    };
    match command.to_ascii_uppercase() {
        'L' => Ok(Segment::Line(pair(parser)?)),
        'H' => {
            let value = parser.read_number()?;
            Ok(Segment::Line(Point::new(if relative { pen.x + value } else { value }, pen.y)))
        }
        'V' => {
            let value = parser.read_number()?;
            Ok(Segment::Line(Point::new(pen.x, if relative { pen.y + value } else { value })))
        }
        'C' => {
            let ctrl1 = pair(parser)?;
            let ctrl2 = pair(parser)?;
            let to = pair(parser)?;
            Ok(Segment::Cubic { ctrl1, ctrl2, to })
        }
        'S' => {
            // `S` carries **one** control point: the second. The first is the reflection of the
            // previous cubic's second control point about the pen, or the pen itself when the
            // previous segment was not a cubic (reflection state is passed in through `ctrl1`).
            let ctrl1 = cursor_ctrl;
            let ctrl2 = pair(parser)?;
            let to = pair(parser)?;
            Ok(Segment::Cubic { ctrl1, ctrl2, to })
        }
        'Q' => {
            let ctrl = pair(parser)?;
            let to = pair(parser)?;
            Ok(Segment::Quad { ctrl, to })
        }
        'T' => {
            // `T` carries **no** control point: it is the quadratic analogue of `S`, with the
            // control point reflected about the pen (or the pen itself when the previous segment
            // was not a quadratic).
            let ctrl = cursor_ctrl;
            let to = pair(parser)?;
            Ok(Segment::Quad { ctrl, to })
        }
        // `A` never reaches this routine: arcs fan out into several cubics, so `parse_segments`
        // handles them before delegating the single-segment commands here. Reaching this arm would
        // mean a command letter changed meaning between the two functions.
        'A' => Err(PathError::UnknownCommand('A')),
        other => Err(PathError::UnknownCommand(other)),
    }
}

/// [`parse_segments`], resolving the reflection a smooth command needs.
///
/// `S`/`T` are defined against the previous curve, so the two control points are reduced to the one
/// this segment reflects: the previous cubic's second control point mirrored through the pen, or the
/// previous quadratic's control point likewise. When the previous segment was of the other kind (or
/// a line, or nothing), the reflection is the **pen**, which is the spec's own rule — not a special
/// case invented here.
fn parse_segments_with_reflection(
    command: char,
    parser: &mut Parser<'_>,
    relative: bool,
    pen: Point,
    last_cubic_ctrl: Option<Point>,
    last_quad_ctrl: Option<Point>,
) -> Result<Vec<Segment>, PathError> {
    let cursor_ctrl = match command.to_ascii_uppercase() {
        'S' => reflect(last_cubic_ctrl, pen),
        'T' => reflect(last_quad_ctrl, pen),
        // Not a smooth command, so nothing reflects. The value is never read.
        _ => pen,
    };
    parse_segments(command, parser, relative, pen, cursor_ctrl)
}

/// The point `2 * pen - control`, or the pen when there is no control to reflect.
fn reflect(control: Option<Point>, pen: Point) -> Point {
    match control {
        Some(control) => Point::new(2.0 * pen.x - control.x, 2.0 * pen.y - control.y),
        None => pen,
    }
}

/// Parses one argument set, returning **every** segment it produces.
///
/// One command is one segment for every command except `A`, which becomes a run of cubics (see
/// [`arc_to_curves`]). This helper keeps [`parse`] from having to know which case it is in: the
/// segment-producing loop appends whatever it gets and counts the points the same way.
fn parse_segments(
    command: char,
    parser: &mut Parser<'_>,
    relative: bool,
    pen: Point,
    cursor_ctrl: Point,
) -> Result<Vec<Segment>, PathError> {
    match command.to_ascii_uppercase() {
        'A' => {
            let rx = parser.read_number()?;
            let ry = parser.read_number()?;
            let x_rotation = parser.read_number()?;
            let large_arc = parser.read_flag()?;
            let sweep = parser.read_flag()?;
            let to = if relative {
                let point = parser.read_point()?;
                Point::new(pen.x + point.x, pen.y + point.y)
            } else {
                parser.read_point()?
            };
            Ok(arc_to_curves(pen, rx, ry, x_rotation, large_arc, sweep, to))
        }
        _ => Ok(crate::compat::vec![parse_segment(command, parser, relative, pen, cursor_ctrl)?]),
    }
}

/// Converts one SVG elliptical arc to the cubic Béziers that approximate it.
///
/// # Why this is here and not left to a caller
///
/// The rasteriser's flattener subdivides quadratics and cubics; an arc is neither, so a caller that
/// received one would have to convert it or drop it. Converting once here is what makes an `A` in
/// an icon's `d` draw as the arc it describes rather than vanish.
///
/// # The conversion
///
/// Follows the SVG implementation notes (F.6.5): scale up degenerate radii, rotate the endpoint
/// delta into the ellipse's frame, solve for the centre from the two candidate solutions selected
/// by `large_arc`, then walk the sweep in steps of at most 90 degrees, emitting one cubic per step.
/// The maximum radial error of the 90-degree cubic approximation is about 0.027 % of the radius —
/// well under a device pixel for any icon, and under the flattener's own tolerance after scaling.
///
/// Degenerate input is handled without a panic: a zero radius, or endpoints that coincide, is a
/// straight line, which is exactly what a renderer that treats an arc as an "ellipse or line" does.
fn arc_to_curves(
    from: Point,
    rx: f32,
    ry: f32,
    x_rotation_degrees: f32,
    large_arc: bool,
    sweep: bool,
    to: Point,
) -> Vec<Segment> {
    // A degenerate arc is a line, per the SVG spec's own "if the endpoints are identical" clause
    // and for a zero radius. Returning the line rather than nothing keeps the pen at `to`.
    if from == to {
        return Vec::new();
    }
    let mut rx = rx.abs();
    let mut ry = ry.abs();
    if rx <= f32::EPSILON || ry <= f32::EPSILON {
        return crate::compat::vec![Segment::Line(to)];
    }

    let phi = x_rotation_degrees.to_radians();
    let (sin_phi, cos_phi) = phi.sin_cos();

    // F.6.6: scale the radii up until they are large enough to span the endpoints.
    let dx = (from.x - to.x) * 0.5;
    let dy = (from.y - to.y) * 0.5;
    let x1 = cos_phi * dx + sin_phi * dy;
    let y1 = -sin_phi * dx + cos_phi * dy;
    let lambda = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry);
    if lambda > 1.0 {
        let scale = lambda.sqrt();
        rx *= scale;
        ry *= scale;
    }

    // F.6.5 step 3: the centre, in the ellipse's own frame.
    let rx2 = rx * rx;
    let ry2 = ry * ry;
    let numerator = (rx2 * ry2) - (rx2 * y1 * y1) - (ry2 * x1 * x1);
    let denominator = (rx2 * y1 * y1) + (ry2 * x1 * x1);
    let mut factor =
        if denominator <= f32::EPSILON { 0.0 } else { (numerator / denominator).max(0.0).sqrt() };
    if large_arc == sweep {
        factor = -factor;
    }
    let cx1 = factor * rx * y1 / ry;
    let cy1 = -factor * ry * x1 / rx;

    // Back to user space.
    let cx = cos_phi * cx1 - sin_phi * cy1 + (from.x + to.x) * 0.5;
    let cy = sin_phi * cx1 + cos_phi * cy1 + (from.y + to.y) * 0.5;

    // F.6.5 step 4: the start angle and the sweep.
    let ux = (x1 - cx1) / rx;
    let uy = (y1 - cy1) / ry;
    let vx = (-x1 - cx1) / rx;
    let vy = (-y1 - cy1) / ry;
    let start = uy.atan2(ux);
    let mut delta = (ux * vy - uy * vx).atan2(ux * vx + uy * vy);
    if !sweep && delta > 0.0 {
        delta -= core::f32::consts::TAU;
    } else if sweep && delta < 0.0 {
        delta += core::f32::consts::TAU;
    }

    // One cubic per at most 90 degrees. `ceil` of a magnitude, so a sweep that rounds to zero
    // segments still emits one — an empty conversion would silently drop the arc.
    let steps = ((delta.abs() / core::f32::consts::FRAC_PI_2).ceil() as usize).max(1);
    let step = delta / steps as f32;
    // The magic constant that makes a 90-degree cubic match a quarter circle to 0.027 %.
    let alpha = (4.0 / 3.0) * (step * 0.25).tan();

    let mut curves = Vec::with_capacity(steps);
    let mut angle = start;
    for index in 0..steps {
        let next = angle + step;
        let (sin_a, cos_a) = angle.sin_cos();
        let (sin_b, cos_b) = next.sin_cos();
        // The two tangents, in the ellipse's frame, then rotated back to user space.
        let ctrl1 = ellipse_point(
            cx,
            cy,
            rx,
            ry,
            sin_phi,
            cos_phi,
            cos_a - alpha * sin_a,
            sin_a + alpha * cos_a,
        );
        let ctrl2 = ellipse_point(
            cx,
            cy,
            rx,
            ry,
            sin_phi,
            cos_phi,
            cos_b + alpha * sin_b,
            sin_b - alpha * cos_b,
        );
        let end = ellipse_point(cx, cy, rx, ry, sin_phi, cos_phi, cos_b, sin_b);
        // The last endpoint is the caller's exact `to`, not the parametrised one: the two agree to
        // the approximation's error, and ending on the declared point keeps the pen exact for the
        // commands that follow.
        let end = if index + 1 == steps { to } else { end };
        curves.push(Segment::Cubic { ctrl1, ctrl2, to: end });
        angle = next;
    }
    curves
}

/// One point on the ellipse, from a unit-circle coordinate, rotated into user space.
#[allow(clippy::too_many_arguments)]
fn ellipse_point(
    cx: f32,
    cy: f32,
    rx: f32,
    ry: f32,
    sin_phi: f32,
    cos_phi: f32,
    unit_x: f32,
    unit_y: f32,
) -> Point {
    let x = rx * unit_x;
    let y = ry * unit_y;
    Point::new(cx + cos_phi * x - sin_phi * y, cy + sin_phi * x + cos_phi * y)
}

/// A cursor over the `d` bytes.
struct Parser<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Parser<'_> {
    fn at_end(&self) -> bool {
        self.at >= self.bytes.len()
    }

    /// Whether a number (as opposed to a command letter) is next, after separators.
    fn number_pending(&self) -> bool {
        let mut at = self.at;
        while let Some(byte) = self.bytes.get(at) {
            if byte.is_ascii_whitespace() || *byte == b',' {
                at += 1;
            } else {
                break;
            }
        }
        matches!(
            self.bytes.get(at),
            Some(b) if b.is_ascii_digit() || matches!(b, b'+' | b'-' | b'.')
        )
    }

    /// Whether a command letter is next, after separators.
    ///
    /// Used to stop the repeated-argument-set loop: a repeat continues only while numbers keep
    /// coming, and a new letter starts a new command. `Z`/`z` are letters too, so they end it as
    /// well, which is what makes `…Z m…` parse as two commands rather than a `Z` swallowing an `m`.
    fn command_pending(&self) -> bool {
        let mut at = self.at;
        while let Some(byte) = self.bytes.get(at) {
            if byte.is_ascii_whitespace() || *byte == b',' {
                at += 1;
            } else {
                break;
            }
        }
        matches!(self.bytes.get(at), Some(b) if b.is_ascii_alphabetic())
    }

    /// Reads the command letter at the cursor.
    fn read_command(&mut self) -> Result<char, PathError> {
        self.skip_separators();
        let byte = self.bytes.get(self.at).copied().ok_or(PathError::ExpectedNumber)?;
        let command = char::from(byte);
        if !matches!(
            command,
            'M' | 'm'
                | 'L'
                | 'l'
                | 'H'
                | 'h'
                | 'V'
                | 'v'
                | 'C'
                | 'c'
                | 'S'
                | 's'
                | 'Q'
                | 'q'
                | 'T'
                | 't'
                | 'A'
                | 'a'
                | 'Z'
                | 'z'
        ) {
            return Err(PathError::UnknownCommand(command));
        }
        self.at += 1;
        Ok(command)
    }

    /// Reads a coordinate pair.
    fn read_point(&mut self) -> Result<Point, PathError> {
        let x = self.read_number()?;
        let y = self.read_number()?;
        Ok(Point::new(x, y))
    }

    /// Skips whitespace and the optional comma SVG allows between numbers.
    fn skip_separators(&mut self) {
        while let Some(byte) = self.bytes.get(self.at) {
            if byte.is_ascii_whitespace() || *byte == b',' {
                self.at += 1;
            } else {
                break;
            }
        }
    }

    /// Reads one SVG number: an optional sign, digits, an optional fraction and an optional
    /// exponent. A leading `.` is accepted (`.5`), which SVG allows.
    fn read_number(&mut self) -> Result<f32, PathError> {
        self.skip_separators();
        let start = self.at;
        if matches!(self.bytes.get(self.at), Some(b'+') | Some(b'-')) {
            self.at += 1;
        }
        let mut saw_digit = false;
        while matches!(self.bytes.get(self.at), Some(b) if b.is_ascii_digit()) {
            self.at += 1;
            saw_digit = true;
        }
        if self.bytes.get(self.at) == Some(&b'.') {
            self.at += 1;
            while matches!(self.bytes.get(self.at), Some(b) if b.is_ascii_digit()) {
                self.at += 1;
                saw_digit = true;
            }
        }
        if !saw_digit {
            return Err(PathError::ExpectedNumber);
        }
        if matches!(self.bytes.get(self.at), Some(b'e') | Some(b'E')) {
            let exponent_start = self.at;
            self.at += 1;
            if matches!(self.bytes.get(self.at), Some(b'+') | Some(b'-')) {
                self.at += 1;
            }
            let mut exponent_digit = false;
            while matches!(self.bytes.get(self.at), Some(b) if b.is_ascii_digit()) {
                self.at += 1;
                exponent_digit = true;
            }
            if !exponent_digit {
                // Not an exponent after all; leave the `e` for the next token to reject.
                self.at = exponent_start;
            }
        }
        let text = core::str::from_utf8(&self.bytes[start..self.at])
            .map_err(|_| PathError::ExpectedNumber)?;
        text.parse::<f32>().map_err(|_| PathError::ExpectedNumber)
    }

    /// Reads an arc flag: `0` or `1`, no sign and no fraction.
    fn read_flag(&mut self) -> Result<bool, PathError> {
        self.skip_separators();
        match self.bytes.get(self.at) {
            Some(b'0') => {
                self.at += 1;
                Ok(false)
            }
            Some(b'1') => {
                self.at += 1;
                Ok(true)
            }
            _ => Err(PathError::ExpectedNumber),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::Vec;

    #[test]
    fn a_single_move_and_line_parses() {
        let subpaths = parse("M1 2 L3 4").expect("a well-formed path must parse");
        assert_eq!(subpaths.len(), 1);
        assert_eq!(subpaths[0].start, Point::new(1.0, 2.0));
        assert_eq!(subpaths[0].segments.as_slice(), &[Segment::Line(Point::new(3.0, 4.0))]);
        assert!(!subpaths[0].closed);
    }

    #[test]
    fn a_relative_command_is_offset_from_the_pen() {
        let subpaths = parse("M10 10 l5 0").expect("relative command must parse");
        assert_eq!(subpaths[0].segments.as_slice(), &[Segment::Line(Point::new(15.0, 10.0))]);
    }

    #[test]
    fn horizontal_and_vertical_keep_the_other_coordinate() {
        let subpaths = parse("M4 5 H10 V20").expect("H/V must parse");
        assert_eq!(
            subpaths[0].segments.as_slice(),
            &[Segment::Line(Point::new(10.0, 5.0)), Segment::Line(Point::new(10.0, 20.0))]
        );
    }

    #[test]
    fn repeated_argument_sets_extend_the_command() {
        // SVG's rule: `L1 1 2 2` is two lines, not one number pair followed by garbage.
        let subpaths = parse("M0 0 L1 1 2 2").expect("repeated arguments must parse");
        assert_eq!(subpaths[0].segments.len(), 2);
    }

    #[test]
    fn a_bare_number_after_a_command_starts_that_same_command() {
        // A command letter may be followed by number sets with no letter between them, and the
        // letter that repeats is the *previous* one — so this is a relative move then a relative
        // line, two commands, not one `m` with four arguments. Real icon data uses this form: the
        // bundled `cross` outline is `m336-280 144-144 …`, and reading it as one `m` collapsed the
        // whole icon to a single point.
        let subpaths = parse("m336-280 144-144").expect("a bare repeat must parse");
        assert_eq!(
            subpaths[0].segments.as_slice(),
            &[Segment::Line(Point::new(480.0, -424.0))],
            "the four numbers are a move and a line, not one move"
        );
    }

    #[test]
    fn an_argument_after_a_move_is_a_line() {
        // SVG promotes the argument set after `M`/`m` to `L`/`l`.
        let subpaths = parse("M0 0 1 1").expect("the implicit line after M must parse");
        assert_eq!(subpaths[0].segments.as_slice(), &[Segment::Line(Point::new(1.0, 1.0))]);
    }

    #[test]
    fn a_smooth_cubic_reflects_the_previous_control_point() {
        // `S` supplies only its second control point; the first is the reflection of the previous
        // cubic's second control point about the pen. This is the form the bundled outlines use, so
        // refusing it (as an earlier version did) left whole icons unrendered.
        let subpaths = parse("M0 0 C0 1 2 1 2 0 S4 -1 4 0").expect("a smooth cubic must parse");
        // The pen is (2, 0) and the previous ctrl2 is (2, 1), so the reflection is (2, -1).
        assert_eq!(
            subpaths[0].segments.as_slice(),
            &[
                Segment::Cubic {
                    ctrl1: Point::new(0.0, 1.0),
                    ctrl2: Point::new(2.0, 1.0),
                    to: Point::new(2.0, 0.0),
                },
                Segment::Cubic {
                    ctrl1: Point::new(2.0, -1.0),
                    ctrl2: Point::new(4.0, -1.0),
                    to: Point::new(4.0, 0.0),
                },
            ]
        );
    }

    #[test]
    fn a_smooth_command_without_a_previous_curve_reflects_the_pen() {
        // The spec's fallback: with nothing to reflect, the control point is the pen itself, which
        // makes the first `S` a curve with both control points at the start — not an error.
        let subpaths = parse("M0 0 S3 3 6 0").expect("a leading smooth cubic must parse");
        assert_eq!(
            subpaths[0].segments.as_slice(),
            &[Segment::Cubic {
                ctrl1: Point::new(0.0, 0.0),
                ctrl2: Point::new(3.0, 3.0),
                to: Point::new(6.0, 0.0),
            }]
        );
    }

    #[test]
    fn a_smooth_quadratic_reflects_and_returns_the_pen() {
        let subpaths = parse("M0 0 Q1 2 2 0 T4 0").expect("a smooth quadratic must parse");
        assert_eq!(
            subpaths[0].segments.as_slice(),
            &[
                Segment::Quad { ctrl: Point::new(1.0, 2.0), to: Point::new(2.0, 0.0) },
                // Reflect (1, 2) about the pen (2, 0): (3, -2).
                Segment::Quad { ctrl: Point::new(3.0, -2.0), to: Point::new(4.0, 0.0) },
            ]
        );
    }

    #[test]
    fn a_line_between_curves_breaks_the_reflection_chain() {
        // After a `L`, the previous segment is not a curve, so the smooth command reflects the pen
        // rather than the line's endpoint (which has no control point).
        let subpaths = parse("M0 0 C0 1 2 1 2 0 L2 2 S4 4 4 2").expect("mixed path must parse");
        let last = subpaths[0].segments.last().expect("a smooth cubic must follow");
        assert_eq!(
            last,
            &Segment::Cubic {
                ctrl1: Point::new(2.0, 2.0),
                ctrl2: Point::new(4.0, 4.0),
                to: Point::new(4.0, 2.0),
            }
        );
    }

    #[test]
    fn curves_carry_their_control_points() {
        let cubic = parse("M0 0 C1 1 2 2 3 3").expect("cubic must parse");
        assert_eq!(
            cubic[0].segments.as_slice(),
            &[Segment::Cubic {
                ctrl1: Point::new(1.0, 1.0),
                ctrl2: Point::new(2.0, 2.0),
                to: Point::new(3.0, 3.0),
            }]
        );
        let quad = parse("M0 0 Q1 1 2 2").expect("quadratic must parse");
        assert_eq!(
            quad[0].segments.as_slice(),
            &[Segment::Quad { ctrl: Point::new(1.0, 1.0), to: Point::new(2.0, 2.0) }]
        );
    }

    #[test]
    fn an_arc_becomes_cubics_that_end_on_the_declared_point() {
        // A quarter circle: one cubic is enough, and the endpoint is the `d`'s own, not the
        // approximation's. The previous shape carried an `Arc` through to the caller, which the
        // rasteriser's flattener cannot consume — so the arc was dropped rather than drawn.
        let subpaths = parse("M0 0 A5 5 0 0 1 10 0").expect("arc must parse");
        let segments = subpaths[0].segments.as_slice();
        assert!(!segments.is_empty(), "an arc must produce geometry");
        assert!(
            matches!(segments.last(), Some(Segment::Cubic { .. })),
            "an arc must be converted to cubics: {segments:?}"
        );
        assert_eq!(
            segments.last().map(Segment::end),
            Some(Point::new(10.0, 0.0)),
            "the last curve must end on the arc's declared endpoint"
        );
        for segment in segments {
            assert!(!matches!(segment, Segment::Line(_)), "an arc is not a line: {segment:?}");
        }
    }

    #[test]
    fn a_large_arc_is_split_into_multiple_cubics() {
        // A half-turn (180 degrees) is split at 90-degree boundaries into two cubics, so the
        // approximation error stays under a device pixel instead of collapsing to a chord.
        let subpaths = parse("M0 0 A5 5 0 1 1 0 10").expect("a large arc must parse");
        let segments = subpaths[0].segments.as_slice();
        assert_eq!(segments.len(), 2, "a 180-degree sweep is two 90-degree cubics");
        assert_eq!(segments.last().map(Segment::end), Some(Point::new(0.0, 10.0)));
    }

    #[test]
    fn an_arc_with_a_zero_radius_is_a_line() {
        // SVG's degenerate case: a radius of zero is a straight line to the endpoint, not an
        // error and not nothing — dropping it would leave the subpath open a segment short.
        let subpaths = parse("M0 0 A0 5 0 0 1 10 10").expect("a zero-radius arc must parse");
        assert_eq!(subpaths[0].segments.as_slice(), &[Segment::Line(Point::new(10.0, 10.0))]);
    }

    #[test]
    fn an_arc_between_identical_points_is_empty() {
        // Also from the spec: identical endpoints mean the arc is omitted entirely. It must not
        // emit a degenerate curve or panic on the division that would follow.
        let subpaths = parse("M5 5 A5 5 0 1 1 5 5").expect("a coincident arc must parse");
        assert!(subpaths[0].segments.is_empty());
    }

    #[test]
    fn an_arc_is_scale_invariant_in_the_number_of_segments() {
        // A relative arc must produce the same shape as its absolute equivalent, which is the
        // property that catches a frame-of-reference mistake in the conversion.
        let absolute = parse("M2 2 A5 5 0 0 1 12 2").expect("absolute arc");
        let relative = parse("M2 2 a5 5 0 0 1 10 0").expect("relative arc");
        assert_eq!(absolute[0].segments.len(), relative[0].segments.len());
        assert_eq!(absolute[0].segments.last().map(Segment::end), Some(Point::new(12.0, 2.0)));
        assert_eq!(relative[0].segments.last().map(Segment::end), Some(Point::new(12.0, 2.0)));
    }

    #[test]
    fn an_arc_that_rounds_to_a_zero_sweep_still_emits_one_curve() {
        // A tiny sweep with `large_arc = false` must not divide by a segment count of zero; the
        // `max(1)` is what this pins.
        let subpaths = parse("M0 0 A100 100 0 0 1 0.0001 0.0001").expect("tiny arc must parse");
        assert_eq!(subpaths[0].segments.len().max(1), 1);
        assert_eq!(subpaths[0].segments.last().map(Segment::end), Some(Point::new(0.0001, 0.0001)));
    }

    #[test]
    fn a_close_marks_the_subpath_and_returns_the_pen() {
        let subpaths = parse("M0 0 L10 0 Z l0 5").expect("close then relative must parse");
        assert!(subpaths[0].closed);
        // After `Z` the pen is back at the subpath start (0, 0), so `l0 5` ends at (0, 5).
        assert_eq!(subpaths[0].segments.last(), Some(&Segment::Line(Point::new(0.0, 5.0))));
    }

    #[test]
    fn a_second_move_starts_a_new_subpath() {
        let subpaths = parse("M0 0 L1 0 M5 5 L6 5").expect("two subpaths must parse");
        assert_eq!(subpaths.len(), 2);
        assert_eq!(subpaths[1].start, Point::new(5.0, 5.0));
    }

    #[test]
    fn separators_include_commas_and_redundant_whitespace() {
        let subpaths = parse("M 1,2\n  L\t3 , 4").expect("mixed separators must parse");
        assert_eq!(subpaths[0].segments.as_slice(), &[Segment::Line(Point::new(3.0, 4.0))]);
    }

    #[test]
    fn decimals_without_a_leading_zero_parse() {
        let subpaths = parse("M.5.5L1.5 2.5").expect("compact decimals must parse");
        assert_eq!(subpaths[0].start, Point::new(0.5, 0.5));
        assert_eq!(subpaths[0].segments.as_slice(), &[Segment::Line(Point::new(1.5, 2.5))]);
    }

    // ── Malformed input must be an error, never a panic ──────────────────────────────

    #[test]
    fn an_unknown_command_is_refused() {
        assert_eq!(parse("M0 0 X1 1"), Err(PathError::UnknownCommand('X')));
    }

    #[test]
    fn a_missing_number_is_refused() {
        assert_eq!(parse("M"), Err(PathError::ExpectedNumber));
        assert_eq!(parse("M0 0 L"), Err(PathError::ExpectedNumber));
    }

    #[test]
    fn a_segment_before_a_move_is_refused() {
        assert_eq!(parse("L1 1"), Err(PathError::SegmentsBeforeMove));
        assert_eq!(parse("Z"), Err(PathError::SegmentsBeforeMove));
    }

    #[test]
    fn an_arc_flag_that_is_not_zero_or_one_is_refused() {
        assert_eq!(parse("M0 0 A5 5 0 2 0 1 1"), Err(PathError::ExpectedNumber));
    }

    #[test]
    fn an_empty_path_is_an_empty_result_not_an_error() {
        // A `d` that names nothing is valid SVG; it is not the same as a malformed one.
        assert_eq!(parse("").expect("empty is valid"), Vec::new());
        assert_eq!(parse("   ").expect("blank is valid"), Vec::new());
    }

    #[test]
    fn a_path_over_the_point_budget_is_refused() {
        // One `M` plus enough `L`s to exceed the budget, without allocating a huge string by hand.
        use crate::compat::{format, String};
        let mut data = String::from("M0 0");
        for i in 0..MAX_PATH_POINTS {
            data.push_str(&format!(" L{i} 0"));
        }
        assert_eq!(parse(&data), Err(PathError::TooManyPoints));
    }
}
