// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! SVG paint backend — converts `RenderCommand`s into SVG elements.

use super::convert::{color_to_rgba, point_attrs, rect_attrs};
use crate::compat::{format, MiniToString, String, Vec};
use crate::core::{Color, Font, Point, Size};
use crate::render::core::command::{BlendMode, RenderCommand};
use crate::render::core::types::{ShapedText, TextMetrics};
use crate::render::text::{is_combining_mark, is_variation_selector};
use crate::render::{PaintBackend, SoftwareRenderConfig};
use crate::style::gradient::GradientType;

/// PaintBackend implementation that generates SVG markup from render commands.
///
/// Every [`RenderCommand`] is converted into an equivalent SVG element.
/// The resulting SVG document is produced when [`finish()`](SvgPaintBackend::finish) is called.
pub struct SvgPaintBackend {
    pub(crate) size: Size,
    dpi_scale: f32,
    pub(crate) elements: Vec<String>,
    clip_depth: u32,
    clip_path_counter: u32,
    svg_output: Option<String>,
    gradient_counter: u32,
    /// Whether a `<g style="mix-blend-mode:…">` is currently open.
    ///
    /// A blend mode applies to everything drawn until it changes, which is sequential frame state,
    /// so it is tracked here and closed by the next `SetBlendMode` or by `build_svg`. Without the
    /// flag a group left open at the end of a frame would nest every later frame's elements.
    blend_group_open: bool,
    /// Whether a `<g filter="url(#blur…)">` is currently open, for the same reason.
    blur_group_open: bool,
}

impl SvgPaintBackend {
    /// Create a new SVG backend with the given canvas size.
    pub fn new(size: Size) -> Self {
        Self {
            size,
            dpi_scale: 1.0,
            elements: Vec::new(),
            clip_depth: 0,
            clip_path_counter: 0,
            svg_output: None,
            gradient_counter: 0,
            blend_group_open: false,
            blur_group_open: false,
        }
    }

    /// Closes an open blend group, if any.
    fn close_blend_group(&mut self) {
        if self.blend_group_open {
            self.push_element("</g>".to_string());
            self.blend_group_open = false;
        }
    }

    /// Closes an open blur group, if any.
    fn close_blur_group(&mut self) {
        if self.blur_group_open {
            self.push_element("</g>".to_string());
            self.blur_group_open = false;
        }
    }

    /// Finalize and retrieve the full SVG document string.
    ///
    /// Once called, the backend is consumed and no further commands can be added.
    pub fn finish(&mut self) -> String {
        if let Some(svg) = self.svg_output.take() {
            return svg;
        }
        self.build_svg()
    }

    /// Build the SVG document from the collected elements.
    fn build_svg(&self) -> String {
        let mut svg = String::from(r#"<svg xmlns="http://www.w3.org/2000/svg""#);
        svg.push_str(&format!(r#" width="{}" height="{}""#, self.size.width, self.size.height));
        svg.push_str(&format!(r#" viewBox="0 0 {} {}">"#, self.size.width, self.size.height));
        for element in &self.elements {
            svg.push('\n');
            svg.push_str("  ");
            svg.push_str(element);
        }
        // Groups left open by a `Blur`/`SetBlendMode` at the end of the frame are closed here, so
        // the document is well-formed whether or not the caller reset the mode explicitly.
        if self.blur_group_open {
            svg.push_str("\n</g>");
        }
        if self.blend_group_open {
            svg.push_str("\n</g>");
        }
        svg.push_str("\n</svg>");
        svg
    }

    /// Add a raw SVG element string to the internal list.
    fn push_element(&mut self, element: String) {
        self.elements.push(element);
    }
}

/// Maps a [`BlendMode`] to its CSS `mix-blend-mode` keyword.
///
/// Every variant has an exact counterpart: the compositing operators are defined by the CSS
/// compositing spec, which is the same definition the software backend's per-pixel arithmetic
/// implements, so the two agree by construction rather than by approximation.
fn blend_mode_to_css(mode: BlendMode) -> &'static str {
    match mode {
        BlendMode::Normal => "normal",
        BlendMode::Multiply => "multiply",
        BlendMode::Screen => "screen",
        BlendMode::Overlay => "overlay",
        BlendMode::Darken => "darken",
        BlendMode::Lighten => "lighten",
        BlendMode::ColorDodge => "color-dodge",
        BlendMode::ColorBurn => "color-burn",
        BlendMode::HardLight => "hard-light",
        BlendMode::SoftLight => "soft-light",
        BlendMode::Difference => "difference",
        BlendMode::Exclusion => "exclusion",
        BlendMode::Hue => "hue",
        BlendMode::Saturation => "saturation",
        BlendMode::Color => "color",
        BlendMode::Luminosity => "luminosity",
    }
}

/// Samples a conic ramp at the screen angle `t` (in `0.0..=1.0`, measured from straight-up).
///
/// # The convention, and why `start_angle` is applied *here*
///
/// The software backend samples pixel by pixel as
/// `pos = ((atan2(dy, dx) + PI) + start_angle) / TAU` — i.e. the caller's `start_angle` rotates the
/// **ramp index**, not the wedge's position. This helper must therefore add `start_angle` to `t`
/// itself. It previously ignored the parameter (`let _ = start_angle;`) while its one caller rotated
/// the *geometry* by the same amount, which is a different rotation: the wedge landed in a new place
/// but kept the colour it would have had where it used to sit, so an SVG snapshot disagreed with the
/// rasteriser for the same command.
fn conic_sample(stops: &[(f32, Color)], t: f32, start_angle: f32) -> Color {
    if stops.is_empty() {
        return Color::TRANSPARENT;
    }
    let t = (t + start_angle / core::f32::consts::TAU).rem_euclid(1.0);
    if t <= stops[0].0 {
        return stops[0].1;
    }
    let last = stops[stops.len() - 1];
    if t >= last.0 {
        return last.1;
    }
    let mut lo = 0usize;
    let mut hi = stops.len() - 1;
    while hi - lo > 1 {
        let mid = (lo + hi) / 2;
        if stops[mid].0 <= t {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let span = (stops[hi].0 - stops[lo].0).max(0.0001);
    let local = (t - stops[lo].0) / span;
    let a = stops[lo].1;
    let b = stops[hi].1;
    Color::rgba(
        (a.r as f32 + (b.r as f32 - a.r as f32) * local) as u8,
        (a.g as f32 + (b.g as f32 - a.g as f32) * local) as u8,
        (a.b as f32 + (b.b as f32 - a.b as f32) * local) as u8,
        (a.a as f32 + (b.a as f32 - a.a as f32) * local) as u8,
    )
}

/// Returns the distance from `centre` to the farthest corner of the `width`×`height` box whose
/// top-left is the origin, so a conic fan reaches every visible pixel.
///
/// The fan is clipped to the box, so any radius at or beyond this value covers it completely; a
/// radius that is too small leaves the far corners unpainted (the software backend paints every
/// pixel of the surface, so a short fan would draw a *different* picture). Taking the maximum over
/// the four corners is exact, where an edge-distance combination can under- or over-shoot.
fn radius_to_far_corner(centre: Point, width: u32, height: u32) -> f32 {
    let cx = centre.x as f32;
    let cy = centre.y as f32;
    let (w, h) = (width as f32, height as f32);
    let mut best: f32 = 0.0;
    for corner in [(0.0, 0.0), (w, 0.0), (0.0, h), (w, h)] {
        best = best.max((corner.0 - cx).hypot(corner.1 - cy));
    }
    best.max(1.0)
}

/// Emits a conic ramp as a fan of wedge `<path>`s centred on `centre` out to `radius`.
///
/// `angle_origin` is passed through to the sampler and `stops` are the `(position, color)` pairs in
/// ascending order. Shared by [`SvgPaintBackend`]'s two conic entry points (the standalone
/// `DrawConicGradient` command and a [`GradientType::Conic`] `DrawGradient`) so one sweep is drawn
/// one way.
/// Draws a conic (sweep) gradient as a fan of wedges.
///
/// # The angle unit is **radians**, matching the command
///
/// `angle_origin` is the sweep's start angle in radians — the unit
/// [`crate::render::RenderCommand::DrawConicGradient`] documents and the unit the software backend's
/// per-pixel `atan2` loop adds it in. This function used to call `to_degrees()` on it, i.e. treat a
/// radian value as degrees, while the *other* caller passed a degree value (from
/// `Gradient::angle`). One parameter, two units: an SVG snapshot and the software surface therefore
/// disagreed about where the sweep started. Each caller now converts at the boundary.
fn draw_conic_fan(
    backend: &mut SvgPaintBackend,
    centre: Point,
    radius: f32,
    angle_origin: f32,
    stops: &[(f32, Color)],
) {
    const WEDGES: usize = 360;
    let cx = centre.x as f32;
    let cy = centre.y as f32;
    for i in 0..WEDGES {
        let t0 = i as f32 / WEDGES as f32;
        let t1 = (i + 1) as f32 / WEDGES as f32;
        // Sample at the wedge's own screen angle; `conic_sample` applies `angle_origin` to the ramp
        // index, which is what the rasteriser does.
        let colour = conic_sample(stops, (t0 + t1) * 0.5, angle_origin);
        // `t` grows from straight-up; SVG's arc angle grows clockwise from +x. Undo the `+180°`
        // origin shift so the drawn wedge sits where the sampled angle says it does. The ramp's own
        // rotation is already carried by the sampled colour, so no `angle_origin` term belongs in
        // the geometry.
        let a0 = (t0 * 360.0 - 180.0).rem_euclid(360.0);
        let a1 = (t1 * 360.0 - 180.0).rem_euclid(360.0);
        let (s0, c0) = (a0.to_radians().sin(), a0.to_radians().cos());
        let (s1, c1) = (a1.to_radians().sin(), a1.to_radians().cos());
        let x0 = cx + radius * c0;
        let y0 = cy + radius * s0;
        let x1 = cx + radius * c1;
        let y1 = cy + radius * s1;
        let large = if (a1 - a0).rem_euclid(360.0) > 180.0 { 1 } else { 0 };
        backend.push_element(format!(
            r##"<path d="M {cx} {cy} L {x0} {y0} A {radius} {radius} 0 {large} 1 {x1} {y1} Z" fill="{}" stroke="none" />"##,
            color_to_rgba(&colour)
        ));
    }
}

/// What [`SvgPaintBackend::append_outline`] did with a glyph.
///
/// The distinction matters because "no outline face covers this character" and "a face covers it but
/// the ink clipped away" have different correct answers: the first falls through to the 1-bit bitmap
/// path (that is what the fallback is *for*), while the second is honestly **nothing** — the
/// rasteriser paints the same glyph into the same cell and writes zero pixels, so emitting a bitmap
/// there would draw ink the pixels do not have.
///
/// A plain `bool` could not express this, and collapsing the two cases to `false` is exactly the
/// defect this type removes: a narrow glyph (an `i` whose estimate-based advance rounds to one
/// pixel) reported "not covered", the caller drew the 8x8 bitmap, and a word's snapshot came out as
/// vector outlines with a bitmap letter inside it.
///
/// The type itself is **not** gated on the outline features, because the cluster loop names it on
/// every build: without an outline face the loop still has to answer "fall through to the bitmap",
/// and the answer is spelled with the same enum so the two arms cannot drift. On such a build only
/// `Uncovered` is ever constructed, so the other two variants are allowed to go unread there — with
/// a reason rather than by deleting them, because the arm that reads them is one feature flag away.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    not(any(feature = "fonts-vector-latin", feature = "fonts-complex", cjk_outline_face)),
    allow(dead_code)
)]
enum OutlineOutcome {
    /// An outline face covers the character and at least one contour survived cell clipping.
    Drawn,
    /// An outline face covers the character, but every contour clipped to a degenerate shape.
    ///
    /// The cell is the estimate-based advance, which is narrower than a real face's ink for some
    /// characters, so this is not a failure — it is the same nothing the rasteriser draws.
    ClippedAway,
    /// No outline face covers the character, so the 1-bit bitmap path should draw it.
    Uncovered,
}

impl SvgPaintBackend {
    /// Appends `ch`'s **outline** to `path`, reporting what happened.
    ///
    /// # Why the geometry comes from `text::outline_by_coverage` and not from `glyph_rects`
    ///
    /// A vector face's ink is curves, and the rasteriser antialiases those curves. Emitting the
    /// face's *bitmap* view here instead would draw a different picture from the one the pixels
    /// show, which is exactly what a snapshot must not do. `text::outline_by_coverage` gives the same
    /// flattened polygons the rasteriser fills, **from the same face** — it selects by coverage, the
    /// way `VectorSource` does.
    ///
    /// # Why not `text::outline`, which takes a family
    ///
    /// Because a family lookup and a coverage lookup answer different questions, and the pixels are
    /// the answer to the *second*. `text::outline` honours the `Font`'s family, which is right for a
    /// caller that named a face this build ships; but every theme in this crate names `"Arial"`, this
    /// crate ships `"Open Sans"`, and `face_for_family("Arial")` is `None` on every build — so this
    /// function's 1-bit fallback drew **all 377 snapshots** as 8x8 bitmap rectangles while
    /// `paint_active` on the same build reported `source=Open Sans ink=Coverage`. The file and the
    /// pixels disagreed about which control they described, which is the one thing this backend's
    /// module docs say it exists to prevent.
    ///
    /// # Why this returns an outcome rather than writing tofu itself
    ///
    /// "No outline face covers this character" is a *fall-through*, not a failure: the caller has a
    /// second path for 1-bit ink and must be allowed to take it. "Covered but clipped away" is the
    /// opposite — the caller must **not** fall through, because the bitmap it would draw is ink the
    /// pixels do not have. [`OutlineOutcome`] keeps that decision in one place (the cluster loop)
    /// instead of duplicating the fallback rule.
    ///
    /// # Why the polygons become one subpath each
    ///
    /// `fill-rule="nonzero"` on the emitted element is what makes a counter a hole: an `o`'s inner
    /// ring winds opposite to its outer one, so the non-zero rule leaves it empty. That is the same
    /// rule the rasteriser applies, so a glyph with a hole keeps it in both backends.
    ///
    /// # Why this is gated on the vector features
    ///
    /// `outline_by_coverage` only exists when a build carries an outline face, and a build that
    /// carries none has no outline ink to emit. Gating the whole function — rather than returning
    /// `false` unconditionally — is what keeps the default build's code path byte-identical: without
    /// an outline face, every glyph takes [`Self::append_bitmap_rects`] exactly as it always did.
    #[cfg(any(feature = "fonts-vector-latin", feature = "fonts-complex", cjk_outline_face))]
    fn append_outline(
        &self,
        path: &mut String,
        ch: char,
        pen_x: f32,
        origin_y: i32,
        glyph_width: u32,
        glyph_height: u32,
    ) -> OutlineOutcome {
        // The buffers live here rather than in the cluster loop so one allocation of each covers a
        // whole line, and they are the same order as the rasteriser's own scratch (`MAX_POINTS` is
        // 1024). They are dropped at the end of the call, so no glyph outline is ever resident.
        let mut points = [crate::render::text::OutlinePoint { x: 0.0, y: 0.0 };
            crate::render::text::OUTLINE_MAX_POINTS];
        let mut contours = [(0usize, 0usize); crate::render::text::OUTLINE_MAX_CONTOURS];
        // The **same cell the rasteriser uses**, so the two backends draw one picture: the cluster's
        // advance wide and the measured line box tall. `Cell::new(glyph_height, glyph_height)` was
        // the first attempt and it is wrong — a glyph 14 px wide in a 24 px square cell lands
        // outside the column its own advance reserves, which is why a dozen widget tests saw ink in
        // the wrong column.
        let cell = crate::render::text::Cell::new(glyph_width, glyph_height);
        let Some(count) =
            crate::render::text::outline_by_coverage(ch, cell, &mut points, &mut contours)
        else {
            return OutlineOutcome::Uncovered;
        };
        // # Clipping to the cell, and why it is a real clip now
        //
        // An outline is scaled to the cell's *height*, so a glyph wider than the estimate-based
        // advance reaches past the cell's right edge, and a descender (`p`, `g`) reaches below its
        // bottom. The rasteriser never writes those pixels — its loop is
        // `for py in 0..cell.height { for px in 0..cell.width }` — so emitting them here would make
        // the snapshot show ink the pixels do not have.
        //
        // # Why "drop the whole contour" was wrong
        //
        // It was the first attempt, and it discarded the *glyph*. The cell is the **estimate-based**
        // advance (0.6 em per Latin cluster) while a real face is genuinely wider, so on `"Sample"`
        // at 14 px the outlines measured:
        //
        // ```text
        // 'S': x=[0.82, 8.01] in a cell 8 wide   -> overflowed by 0.01, contour dropped
        // 'm': x=[1.37,13.52] in a cell 8 wide   -> overflowed by 5.52, contour dropped
        // 'e': x=[0.89, 8.15] in a cell 8 wide   -> overflowed by 0.15, contour dropped
        // 'p': x=[1.37, 8.90] y=[4.40,16.95]     -> overflowed both    , contour dropped
        // ```
        //
        // Four of the six glyphs therefore fell through to the 1-bit path and were drawn as 8x8 bitmap
        // rectangles **inside an otherwise vector string** — which is the mixed, cramped-looking text
        // this produced. A hundredth of a pixel of overflow is not a reason to draw a different glyph.
        //
        // # Sutherland–Hodgman, and why the winding matters
        //
        // Each contour is clipped against the cell's four edges in turn. Clipping a *closed* polygon
        // this way **preserves its orientation**, which is what makes the fix compatible with the
        // `fill-rule="nonzero"` contract: a counter such as `o`'s inner ring still winds opposite to
        // its outer one, so it stays a hole (a clip that reversed winding would fill every counter and
        // turn an `o` into a blob).
        //
        // This is the "proper polygon clipping" the earlier comment declined to write. It is ~40 lines
        // of edge function, shared by all four edges, and the alternative was a snapshot set where
        // most glyphs in a word were a different typeface from the rest.
        let clip_left = pen_x;
        let clip_right = pen_x + glyph_width as f32;
        let clip_top = origin_y as f32;
        let clip_bottom = clip_top + glyph_height as f32;
        let before = path.len();
        // Two vertex buffers, ping-ponged: one holds the contour before an edge pass, the other after.
        // Sized for the worst case, where clipping an edge can add one vertex per existing edge.
        let capacity = crate::render::text::OUTLINE_MAX_POINTS * 2;
        let mut scratch_a: Vec<(f32, f32)> = Vec::with_capacity(capacity);
        let mut scratch_b: Vec<(f32, f32)> = Vec::with_capacity(capacity);
        for (start, end) in contours.iter().take(count) {
            let Some(contour) = points.get(*start..*end) else {
                continue;
            };
            scratch_a.clear();
            scratch_a.extend(contour.iter().map(|p| (pen_x + p.x, origin_y as f32 + p.y)));
            // Four edge passes, each closing the polygon at one side of the cell. Kept as four calls
            // rather than a table of predicates because each one captures a different bound, and a
            // `fn` pointer cannot capture — the table form would push the bounds into a struct for no
            // gain in clarity.
            clip_against_x(&scratch_a, &mut scratch_b, clip_left, true);
            core::mem::swap(&mut scratch_a, &mut scratch_b);
            if !scratch_a.is_empty() {
                clip_against_x(&scratch_a, &mut scratch_b, clip_right, false);
                core::mem::swap(&mut scratch_a, &mut scratch_b);
            }
            if !scratch_a.is_empty() {
                clip_against_y(&scratch_a, &mut scratch_b, clip_top, true);
                core::mem::swap(&mut scratch_a, &mut scratch_b);
            }
            if !scratch_a.is_empty() {
                clip_against_y(&scratch_a, &mut scratch_b, clip_bottom, false);
                core::mem::swap(&mut scratch_a, &mut scratch_b);
            }
            if scratch_a.len() < 3 {
                // A degenerate result (a touch on an edge, or a contour that was entirely outside).
                // Emitting it would be an empty subpath, which the snapshot gate reads as a defect.
                continue;
            }
            // Polygon subpaths are `M` then `L`s then `Z`. Coordinates are rounded to two decimals
            // rather than to integers: an antialiased outline's whole advantage is its sub-pixel
            // precision, and rounding to whole pixels would turn every curve back into the blocks
            // the 1-bit path already draws.
            path.push_str(&format!("M{:.2} {:.2}", scratch_a[0].0, scratch_a[0].1));
            for point in scratch_a.iter().skip(1) {
                path.push_str(&format!("L{:.2} {:.2}", point.0, point.1));
            }
            path.push('Z');
        }
        // A face can report a contour whose points all coincide, which produces an empty subpath.
        // Treating that as "no geometry" sends the glyph to the bitmap path rather than emitting a
        // degenerate `Mx yZ` that draws nothing.
        if path.len() > before {
            OutlineOutcome::Drawn
        } else {
            OutlineOutcome::ClippedAway
        }
    }

    /// Appends `ch`'s 1-bit **bitmap rectangles** to `path`.
    ///
    /// # Why rectangles-per-source-pixel is the right answer here
    ///
    /// A set bit in an 8x8 or 16x16 source bitmap is one rectangle however large the cell is, so an
    /// 8x8 glyph in a 40 px box is 30 subpaths rather than 750. Compressing is not a shortcut here:
    /// the ink genuinely has no detail between the bits, so the rectangles are the face's exact
    /// geometry rather than an approximation of it.
    fn append_bitmap_rects(
        &self,
        path: &mut String,
        ch: char,
        pen_x: f32,
        origin_y: i32,
        glyph_width: u32,
        glyph_height: u32,
    ) {
        for (x0, y0, x1, y1) in crate::render::glyph_rects(
            ch,
            pen_x.round() as i32,
            origin_y,
            glyph_width,
            glyph_height,
        ) {
            // Each rectangle is one subpath. Axis-aligned subpaths that never overlap need no
            // `fill-rule`, but the element carries `nonzero` anyway for the outline path's sake.
            path.push_str(&format!("M{x0} {y0}h{}v{}h-{}z", x1 - x0, y1 - y0, x1 - x0));
        }
    }
}

// ─── Helper: Sutherland–Hodgman polygon clipping ─────────────────────────
//
// Gated with `append_outline`, which is the only caller: a build with no outline face draws glyph
// bitmaps and never clips a polygon, so compiling these there would be dead code — and the crate
// treats a dead-code warning as a defect rather than as noise.

/// Clips `input` against the half-plane `x >= limit` (`keep_greater`) or `x <= limit`.
///
/// One pair of functions per axis keeps the four call sites free of truth tables: "which side does
/// this edge keep" is a `bool` no reader can misread, whereas a generic comparator would have to be
/// spelled out at each call. See [`clip_polygon`] for the algorithm and why winding survives it.
#[cfg(any(feature = "fonts-vector-latin", feature = "fonts-complex", cjk_outline_face))]
fn clip_against_x(
    input: &[(f32, f32)],
    output: &mut Vec<(f32, f32)>,
    limit: f32,
    keep_greater: bool,
) {
    let inside = |x: f32, _y: f32| if keep_greater { x >= limit } else { x <= limit };
    clip_polygon(input, output, inside, |x, _y| x, limit);
}

/// Clips `input` against the half-plane `y >= limit` (`keep_greater`) or `y <= limit`.
#[cfg(any(feature = "fonts-vector-latin", feature = "fonts-complex", cjk_outline_face))]
fn clip_against_y(
    input: &[(f32, f32)],
    output: &mut Vec<(f32, f32)>,
    limit: f32,
    keep_greater: bool,
) {
    let inside = |_x: f32, y: f32| if keep_greater { y >= limit } else { y <= limit };
    clip_polygon(input, output, inside, |_x, y| y, limit);
}

/// Clips `input` against one half-plane, appending the result to `output`.
///
/// # The algorithm, in one paragraph
///
/// Walk the polygon's edges. For each edge from `s` to `e`: if `e` is inside, emit the crossing point
/// (when `s` was outside) and then `e`; if `e` is outside and `s` was inside, emit only the crossing
/// point. That is the whole of Sutherland–Hodgman, and applying it to the four sides of a rectangle
/// clips to that rectangle.
///
/// # Why the winding survives, which is the part that matters here
///
/// The traversal order is the input's order and nothing is reversed or re-sorted — crossing points are
/// *inserted between* the vertices they lie between. So a contour that wound one way still winds that
/// way, and the `fill-rule="nonzero"` contract that keeps an `o`'s counter a hole still holds. That
/// is not incidental: a clipper that emitted each edge segment as its own subpath would produce a
/// correct-looking outline whose counters were filled, and the glyph would read as a blob at small
/// sizes.
///
/// `varying` projects a point onto the axis this edge tests, and `limit` is the bound — so the
/// crossing is always interpolated on the same coordinate the inside test used, which is what keeps
/// a clip against `x` from computing a crossing along `y`.
#[cfg(any(feature = "fonts-vector-latin", feature = "fonts-complex", cjk_outline_face))]
fn clip_polygon(
    input: &[(f32, f32)],
    output: &mut Vec<(f32, f32)>,
    inside: impl Fn(f32, f32) -> bool,
    varying: impl Fn(f32, f32) -> f32,
    limit: f32,
) {
    output.clear();
    if input.is_empty() {
        return;
    }
    for index in 0..input.len() {
        let start = input[index];
        let end = input[(index + 1) % input.len()];
        let start_inside = inside(start.0, start.1);
        let end_inside = inside(end.0, end.1);
        if end_inside {
            if !start_inside {
                // Entering: the crossing is where the edge meets the boundary.
                if let Some(crossing) = edge_crossing(start, end, limit, &varying) {
                    output.push(crossing);
                }
            }
            output.push(end);
        } else if start_inside {
            // Leaving: emit the crossing so the polygon stays closed at the boundary.
            if let Some(crossing) = edge_crossing(start, end, limit, &varying) {
                output.push(crossing);
            }
        }
    }
}

/// Where the segment `start -> end` meets the plane whose `varying` coordinate is `limit`.
///
/// Returns `None` for a segment parallel to that plane (its two ends share the varying coordinate, so
/// it either lies in the plane or never reaches it). Emitting a vertex for such a segment would
/// duplicate a point the walk already emitted, and a duplicated vertex makes a zero-area edge —
/// harmless for filling, but it is churn in a byte-compared artifact.
#[cfg(any(feature = "fonts-vector-latin", feature = "fonts-complex", cjk_outline_face))]
fn edge_crossing(
    start: (f32, f32),
    end: (f32, f32),
    limit: f32,
    varying: &impl Fn(f32, f32) -> f32,
) -> Option<(f32, f32)> {
    let span = varying(end.0, end.1) - varying(start.0, start.1);
    if span.abs() < f32::EPSILON {
        return None;
    }
    let t = (limit - varying(start.0, start.1)) / span;
    if !(0.0..=1.0).contains(&t) {
        return None;
    }
    Some((start.0 + (end.0 - start.0) * t, start.1 + (end.1 - start.1) * t))
}

// ─── Helper: RGBA→BMP conversion ──────────────────────────────────────────

/// Convert raw RGBA pixel data into an in-memory BMP file (32-bit BGRA).
///
/// # The size is the *data's*, not the caller's
///
/// `width`/`height` are what the caller asked the image to be scaled to, and the data is only
/// `data.len() / 4` pixels. Indexing the buffer with the requested extent panicked whenever the two
/// disagreed -- which is the normal case for a scaled icon, because a 2x2 source drawn into a 16x16
/// slot is exactly "draw me at a size the pixels do not have". The BMP therefore carries the data's
/// own extent, and the `<image>` element that references it keeps the requested one: SVG scales the
/// bitmap into the box, which is what the caller asked for and what the software backend already
/// does.
fn rgba_to_bmp(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    // The extent the buffer actually holds. A zero factor means "no pixels", and the caller's
    // rectangle is then only an empty box to draw nothing into.
    let source_pixels = (rgba.len() / 4) as u32;
    let (width, height) = if source_pixels == 0 {
        (0, 0)
    } else if (width * height) as usize == source_pixels as usize && width > 0 && height > 0 {
        // The data matches what was asked for, so the caller's extent is the data's.
        (width, height)
    } else if let Some(derived_width) = source_pixels.checked_div(height) {
        // Otherwise the declared height indexes rows and the width follows from the data, which keeps
        // a mis-declared pair from producing a short or ragged row. `checked_div` rather than a bare
        // `/` guarded by the branch above: the guard and the division are one fact, so the division
        // carries it instead of a reader having to check that the branch protects it.
        (derived_width, height)
    } else {
        // No usable height: treat the data as one row, which is the only shape left that cannot lose
        // a pixel.
        (source_pixels, 1)
    };
    let row_size = width * 4; // 4 bytes/pixel, already 4-byte aligned
    let pixel_data_size = row_size * height;
    let file_size: usize = 14 + 40 + pixel_data_size as usize;
    let mut bmp = Vec::with_capacity(file_size);

    // BITMAPFILEHEADER (14 bytes)
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&(file_size as u32).to_le_bytes());
    bmp.extend_from_slice(&[0u8; 4]); // reserved
    bmp.extend_from_slice(&54u32.to_le_bytes()); // offset to pixel array

    // BITMAPINFOHEADER (40 bytes)
    bmp.extend_from_slice(&40u32.to_le_bytes()); // header size
    bmp.extend_from_slice(&width.to_le_bytes());
    bmp.extend_from_slice(&height.to_le_bytes());
    bmp.extend_from_slice(&1u16.to_le_bytes()); // color planes
    bmp.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
    bmp.extend_from_slice(&0u32.to_le_bytes()); // compression (BI_RGB)
    bmp.extend_from_slice(&pixel_data_size.to_le_bytes()); // image size
    bmp.extend_from_slice(&0i32.to_le_bytes()); // x pixels-per-meter
    bmp.extend_from_slice(&0i32.to_le_bytes()); // y pixels-per-meter
    bmp.extend_from_slice(&0u32.to_le_bytes()); // colors used
    bmp.extend_from_slice(&0u32.to_le_bytes()); // important colors

    // Pixel data: RGBA → BGRA, stored bottom-up. Bounded by the buffer rather than by the extent,
    // so a truncated payload loses its trailing pixels instead of panicking -- the same rule
    // `chunks_exact` states elsewhere in this crate for a malformed buffer.
    for y in (0..height).rev() {
        let row_off = (y * row_size) as usize;
        for x in 0..width {
            let idx = row_off + (x * 4) as usize;
            if idx + 4 > rgba.len() {
                return bmp;
            }
            bmp.push(rgba[idx + 2]); // B
            bmp.push(rgba[idx + 1]); // G
            bmp.push(rgba[idx]); // R
            bmp.push(rgba[idx + 3]); // A
        }
    }

    bmp
}

/// Minimal base64 encoder (RFC 4648) — no dependencies needed.
fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/=";
    let cap = data.len().div_ceil(3) * 4;
    let mut out = String::with_capacity(cap);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        out.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            out.push(CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(CHARS[(triple & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

// ─── PaintBackend implementation ─────────────────────────────────────────────

impl PaintBackend for SvgPaintBackend {
    fn begin_frame(&mut self, clear: Color) {
        self.elements.clear();
        self.clip_depth = 0;
        // Background fill rect matching clear color
        if clear.a > 0 {
            let fill = color_to_rgba(&clear);
            let bg_rect = crate::core::Rect::new(0, 0, self.size.width, self.size.height);
            self.push_element(format!(r#"<rect {} fill="{}" />"#, rect_attrs(&bg_rect), fill));
        }
    }

    fn end_frame(&mut self) {
        // Close any remaining clip groups
        for _ in 0..self.clip_depth {
            self.push_element("</g>".to_string());
        }
        self.clip_depth = 0;
        self.svg_output = Some(self.build_svg());
    }

    fn execute_command(&mut self, command: &RenderCommand) {
        match command {
            // ── Filled rectangles ──────────────────────────────────────
            RenderCommand::FillRect { rect, color } => {
                self.push_element(format!(
                    r#"<rect {} fill="{}" />"#,
                    rect_attrs(rect),
                    color_to_rgba(color)
                ));
            }

            RenderCommand::FillRoundedRect { rect, radius, color }
            | RenderCommand::FillRoundedRectAA { rect, radius, color } => {
                self.push_element(format!(
                    r#"<rect {} rx="{}" ry="{}" fill="{}" />"#,
                    rect_attrs(rect),
                    radius,
                    radius,
                    color_to_rgba(color)
                ));
            }

            // ── Rectangle outlines ─────────────────────────────────────
            RenderCommand::DrawRect { rect, color } => {
                self.push_element(format!(
                    r#"<rect {} fill="none" stroke="{}" stroke-width="1" />"#,
                    rect_attrs(rect),
                    color_to_rgba(color)
                ));
            }

            RenderCommand::DrawRectStroke { rect, color, width } => {
                self.push_element(format!(
                    r#"<rect {} fill="none" stroke="{}" stroke-width="{}" />"#,
                    rect_attrs(rect),
                    color_to_rgba(color),
                    width
                ));
            }

            // ── Rounded rectangle outlines ─────────────────────────────
            RenderCommand::DrawRoundedRectStroke { rect, radius, color, width }
            | RenderCommand::DrawRoundedRectStrokeAA { rect, radius, color, width } => {
                self.push_element(format!(
                    r#"<rect {} rx="{}" ry="{}" fill="none" stroke="{}" stroke-width="{}" />"#,
                    rect_attrs(rect),
                    radius,
                    radius,
                    color_to_rgba(color),
                    width
                ));
            }

            // ── Lines ──────────────────────────────────────────────────
            RenderCommand::DrawLine { from, to, color }
            | RenderCommand::DrawLineAA { from, to, color } => {
                self.push_element(format!(
                    r#"<line {} x2="{}" y2="{}" stroke="{}" stroke-width="1" />"#,
                    point_attrs(from),
                    to.x,
                    to.y,
                    color_to_rgba(color)
                ));
            }

            RenderCommand::DrawLineStroke { from, to, color, width }
            | RenderCommand::DrawLineStrokeAA { from, to, color, width } => {
                self.push_element(format!(
                    r#"<line {} x2="{}" y2="{}" stroke="{}" stroke-width="{}" />"#,
                    point_attrs(from),
                    to.x,
                    to.y,
                    color_to_rgba(color),
                    width
                ));
            }

            // ── Filled circles ─────────────────────────────────────────
            RenderCommand::FillCircle { center, radius, color }
            | RenderCommand::FillCircleAA { center, radius, color } => {
                self.push_element(format!(
                    r#"<circle cx="{}" cy="{}" r="{}" fill="{}" />"#,
                    center.x,
                    center.y,
                    radius,
                    color_to_rgba(color)
                ));
            }

            // ── Circle outlines ────────────────────────────────────────
            RenderCommand::DrawCircle { center, radius, color } => {
                self.push_element(format!(
                    r#"<circle cx="{}" cy="{}" r="{}" fill="none" stroke="{}" stroke-width="1" />"#,
                    center.x,
                    center.y,
                    radius,
                    color_to_rgba(color)
                ));
            }

            RenderCommand::DrawCircleStroke { center, radius, color, width } => {
                self.push_element(format!(
                    r#"<circle cx="{}" cy="{}" r="{}" fill="none" stroke="{}" stroke-width="{}" />"#,
                    center.x,
                    center.y,
                    radius,
                    color_to_rgba(color),
                    width
                ));
            }

            // ── Text ───────────────────────────────────────────────────
            //
            // # Why this draws glyph geometry instead of a `<text>` element
            //
            // A `<text>` element hands the string to the viewer's font engine. That is a
            // *different renderer* from this crate's, in three ways at once:
            //
            // | | software rasteriser | `<text>` element |
            // |---|---|---|
            // | glyph source | the whole font stack (`render::text`) | whatever font the viewer has |
            // | glyph shape | a filled outline, or bitmap rectangles | vector outlines |
            // | advance | `estimate_cluster_advance` (0.6 em, 1.0 em wide, 0.33 em space) | the font's own metrics |
            //
            // `snapshots/svg/` exists to be a *picture of what the control draws*, so a snapshot
            // rendered by a different font engine is a picture of a different control. This crate's
            // font is `render::text` — kept in every profile including `mini` — so the backend that
            // must change is this one.
            //
            // # The two glyph paths, and why both are needed
            //
            // The ink a face produces decides which path expresses it:
            //
            // * a **1-bit** face (the default 8x8, the CJK bitmap) is rectangles at its set source
            //   pixels — `glyph_rects`. That is the compression this backend wants: 30 subpaths for
            //   an 8x8 glyph in a 40 px box, against 750 per-destination-pixel rectangles;
            // * an **outline** face (any `fonts-vector-*` or `fonts-cjk`) is real curves, which
            //   `text::outline` hands over as device-space polygons. Emitting rectangles for it
            //   would *understate* the ink — the rasteriser antialiases the same outline — so the
            //   snapshot would disagree with the pixels by construction.
            //
            // Both paths read the same face and the same [`Placement`], so they are one drawing.
            // The choice is made per glyph by `text::outline`'s success, which is exactly the
            // question "is this character covered by an outline face?".
            //
            // A **colour** face is the case neither path can express as geometry: its ink is a
            // PNG's pixels, and no path compresses it. Those glyphs fall through to the 1-bit path,
            // which draws tofu — a deliberate, visible degradation rather than a silently wrong
            // picture. See `snapshots/svg/README.md` and the round log for why that is the ruling.
            RenderCommand::DrawText { origin, text, font, color, alignment } => {
                // `origin` is the glyph box's **top-left**, exactly as for the rasteriser.
                //
                // `HorizontalAlignment` is resolved here for the same reason it always was:
                // `origin` is the alignment's *anchor*, and the shift is the measured advance —
                // the same `shape_text` the rasteriser uses.
                let shaped = self.shape_text(text, font);
                // The same two quantities the rasteriser's `draw_text` reads: the measured glyph
                // box height, and a floating-point pen advanced by each cluster's own advance.
                let glyph_height = self.measure_text(text, font).height.max(1);
                let anchor_x = match alignment {
                    crate::core::HorizontalAlignment::Left => origin.x as f32,
                    crate::core::HorizontalAlignment::Center => {
                        origin.x as f32 - shaped.advance() / 2.0
                    }
                    crate::core::HorizontalAlignment::Right => origin.x as f32 - shaped.advance(),
                };
                let mut path = String::new();
                let mut pen_x = anchor_x;
                for cluster in shaped.clusters() {
                    let glyph_width = cluster.advance.max(1.0).round() as u32;
                    let display_char = cluster
                        .text
                        .chars()
                        .find(|ch| !is_combining_mark(*ch) && !is_variation_selector(*ch));
                    if let Some(ch) = display_char {
                        // Try the outline path first: an outline face covers ASCII (and, with
                        // `fonts-cjk`, Han and kana), while the bitmap faces cover everything the
                        // default build draws. Only one of the two ever produces geometry, so the
                        // order is a statement of preference, not a risk of double-drawing.
                        //
                        // The **bitmap fallback is taken only for `Uncovered`**, never for
                        // `ClippedAway`: a covered glyph whose ink clipped to nothing is honestly
                        // nothing, and drawing its 8x8 bitmap instead put a bitmap letter inside an
                        // otherwise vector word. The rasteriser paints the same glyph into the same
                        // cell and also writes nothing, so `ClippedAway` is the picture the pixels
                        // actually show.
                        #[cfg(any(
                            feature = "fonts-vector-latin",
                            feature = "fonts-complex",
                            cjk_outline_face
                        ))]
                        let outcome = self.append_outline(
                            &mut path,
                            ch,
                            pen_x,
                            origin.y,
                            glyph_width,
                            glyph_height,
                        );
                        // Without an outline face there is no ink to emit, so every glyph takes the
                        // 1-bit path exactly as it always did.
                        #[cfg(not(any(
                            feature = "fonts-vector-latin",
                            feature = "fonts-complex",
                            cjk_outline_face
                        )))]
                        let outcome = OutlineOutcome::Uncovered;
                        if outcome == OutlineOutcome::Uncovered {
                            self.append_bitmap_rects(
                                &mut path,
                                ch,
                                pen_x,
                                origin.y,
                                glyph_width,
                                glyph_height,
                            );
                        }
                    }
                    pen_x += cluster.advance;
                }
                if path.is_empty() {
                    // Nothing to paint — an empty string, or one whose glyphs are all blank.
                    // An empty `<path d="">` would be a drawing element that draws nothing,
                    // which the snapshot gate reads as a defect.
                    return;
                }
                // `fill-rule` and the `data-text` provenance tag are emitted only when an outline
                // face is in the build.
                //
                // The 1-bit path needs neither: axis-aligned, non-overlapping rectangles are the
                // same picture under either fill rule, and a run built from `h`/`v` is already
                // identifiable by its command letters alone (see `widget::svg::is_text_path`).
                // Adding either attribute unconditionally would rewrite all 377 committed snapshots
                // for no behavioural change, which is exactly the kind of silent churn the
                // byte-identical requirement exists to prevent.
                //
                // # Why the tag exists at all
                //
                // `data-text` is what lets a consumer tell a text run from a drawn shape **without
                // guessing**. It was guessed before — "a `d` containing `L` is a picture, not text"
                // — because the only glyph path that existed was the bitmap one, whose command set
                // is `M`/`h`/`v`/`z`. An outline's command set is `M`/`L`/`Z` with fractional
                // coordinates, which is *also* how a triangle or an elbow is written, so the guess
                // became ambiguous the moment outlines were drawn: `widget::svg::text_ink_boxes`
                // answered "no text" for every vector-rendered control, and 42 tests that assert on
                // where a label's ink landed began to fail.
                //
                // A provenance tag resolves it at the source. The producer knows a run is a run;
                // recording that fact costs 15 bytes on a path that is already hundreds, and it
                // replaces a heuristic that cannot be made correct by adding cases.
                #[cfg(any(
                    feature = "fonts-vector-latin",
                    feature = "fonts-complex",
                    cjk_outline_face
                ))]
                self.push_element(format!(
                    r#"<path d="{}" fill="{}" fill-rule="nonzero" data-text="1" />"#,
                    path,
                    color_to_rgba(color)
                ));
                #[cfg(not(any(
                    feature = "fonts-vector-latin",
                    feature = "fonts-complex",
                    cjk_outline_face
                )))]
                self.push_element(format!(
                    r#"<path d="{}" fill="{}" />"#,
                    path,
                    color_to_rgba(color)
                ));
            }

            // ── Image ──────────────────────────────────────────────────
            RenderCommand::DrawImage { x, y, width, height, data } => {
                if !data.is_empty() && *width > 0 && *height > 0 {
                    // Convert RGBA pixel data to BMP and base64-encode for embedding. The BMP holds
                    // the pixels' own extent and the `<image>` element holds the requested box, so a
                    // scaled draw scales rather than reading past the buffer -- the two were the same
                    // number until a control drew an image at a size its pixels did not have.
                    let bmp = rgba_to_bmp(*width, *height, data);
                    let b64 = base64_encode(&bmp);
                    self.push_element(format!(
                        r##"<image x="{x}" y="{y}" width="{width}" height="{height}" preserveAspectRatio="none" href="data:image/bmp;base64,{b64}" />"##
                    ));
                } else {
                    // No pixel data: render an error placeholder rectangle.
                    self.push_element(format!(
                        r##"<rect x="{x}" y="{y}" width="{width}" height="{height}" fill="#fee" stroke="#c00" stroke-width="2" />"##
                    ));
                    let cx = x + (*width as i32) / 2;
                    let cy = y + (*height as i32) / 2;
                    self.push_element(format!(
                        r##"<text x="{cx}" y="{cy}" text-anchor="middle" dominant-baseline="central" font-family="sans-serif" font-size="11" fill="#c00">Image (no data)</text>"##
                    ));
                }
            }

            // ── Clipping ───────────────────────────────────────────────
            RenderCommand::PushClip { x, y, width, height } => {
                let clip_id = format!("clip_{}", self.clip_depth);
                // Build a minimal <defs> clipPath — SVG renderers need the
                // definition available before the referencing <g> element.
                let clip_def = format!(
                    r#"<clipPath id="{clip_id}"><rect x="{x}" y="{y}" width="{width}" height="{height}" /></clipPath>"#
                );
                self.push_element(clip_def);
                self.push_element(format!(r#"<g clip-path="url(#{clip_id})">"#));
                self.clip_depth += 1;
            }

            RenderCommand::PopClip => {
                if self.clip_depth > 0 {
                    self.push_element("</g>".to_string());
                    self.clip_depth -= 1;
                }
            }

            // ── Gradient ────────────────────────────────────────────────
            RenderCommand::DrawGradient { rect, gradient } => {
                // A conic ramp cannot be expressed as an SVG paint server, so it is drawn as a fan
                // of wedges clipped to `rect` — the same construction `DrawConicGradient` uses, and
                // for the same reason (the previous form silently approximated it with a **linear**
                // gradient, which is a wrong picture rather than a missing one). The stop list is
                // converted to the `(position, color)` pairs the sampler takes.
                if gradient.gradient_type == GradientType::Conic {
                    let pairs: Vec<(f32, Color)> =
                        gradient.stops.iter().map(|s| (s.position, s.color)).collect();
                    if pairs.is_empty() {
                        return;
                    }
                    let clip_id = format!("cg{}", self.gradient_counter);
                    self.gradient_counter += 1;
                    self.push_element(format!(
                        r##"<clipPath id="{clip_id}"><rect x="{}" y="{}" width="{}" height="{}" /></clipPath>"##,
                        rect.x, rect.y, rect.width, rect.height
                    ));
                    self.push_element(format!(r##"<g clip-path="url(#{clip_id})">"##));
                    let radius = radius_to_far_corner(gradient.center, rect.width, rect.height);
                    // `Gradient::angle` is in **degrees**; `draw_conic_fan` takes radians (the unit
                    // the `DrawConicGradient` command uses). This call hard-coded `0.0` before, so an
                    // SVG snapshot dropped the caller's start angle entirely while the software
                    // backend applied it — the two drew different pictures for one command.
                    draw_conic_fan(
                        self,
                        gradient.center,
                        radius,
                        gradient.angle.to_radians(),
                        &pairs,
                    );
                    self.push_element("</g>".to_string());
                    return;
                }
                self.gradient_counter += 1;
                let gid = format!("g{}", self.gradient_counter);
                let mut def = String::new();
                match gradient.gradient_type {
                    GradientType::Linear => {
                        def.push_str(&format!(
                            r##"<linearGradient id=\"{}\" x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\">"##,
                            gid,
                            gradient.start_point.x,
                            gradient.start_point.y,
                            gradient.end_point.x,
                            gradient.end_point.y
                        ));
                    }
                    GradientType::Radial => {
                        def.push_str(&format!(
                            r##"<radialGradient id=\"{}\" cx=\"{}\" cy=\"{}\" r=\"{}\">"##,
                            gid, gradient.center.x, gradient.center.y, gradient.radius
                        ));
                    }
                    // Handled above and returned; kept for match totality.
                    GradientType::Conic => {
                        unreachable!("conic gradients return before the paint server")
                    }
                }
                for stop in &gradient.stops {
                    let hex =
                        format!("#{:02x}{:02x}{:02x}", stop.color.r, stop.color.g, stop.color.b);
                    let alpha = stop.color.a as f32 / 255.0;
                    def.push_str(&format!(
                        r##"<stop offset=\"{:.3}\" stop-color=\"{}\" stop-opacity=\"{:.3}\"/>"##,
                        stop.position, hex, alpha
                    ));
                }
                match gradient.gradient_type {
                    GradientType::Linear => {
                        def.push_str("</linearGradient>");
                    }
                    GradientType::Radial => {
                        def.push_str("</radialGradient>");
                    }
                    GradientType::Conic => unreachable!(),
                }
                self.push_element(format!("<defs>{def}</defs>"));
                self.push_element(format!(
                    r##"<rect x="{}" y="{}" width="{}" height="{}" fill="url(#{})" />"##,
                    rect.x, rect.y, rect.width, rect.height, gid
                ));
            }

            // ── Arc ─────────────────────────────────────────────────────
            RenderCommand::DrawArc { center, radius, start_angle, end_angle, color, filled } => {
                // The sweep is the **short** way round when `end <= start`: the command's own
                // contract says a `start` of 350° and an `end` of 10° sweeps 20°, not 340°.
                //
                // `large-arc-flag` used to be computed from `|end - start|` on the raw angles, so a
                // wraparound pair such as (350°, 10°) measured ~5.93 rad > π and set the flag to 1 —
                // which selects the **≥180°** arc. SVG then drew 340° where the software backend
                // (`pipeline::primitives::draw_arc`, which has an explicit wraparound branch) drew
                // 20°. The sweep is now normalised first, exactly as the rasteriser does, so the flag
                // describes the arc that is actually drawn.
                const TWO_PI: f32 = core::f32::consts::TAU;
                let norm = |angle: f32| {
                    let mut a = angle % TWO_PI;
                    if a < 0.0 {
                        a += TWO_PI;
                    }
                    a
                };
                let start = norm(*start_angle);
                let end = norm(*end_angle);
                let sweep = if end >= start { end - start } else { (end + TWO_PI) - start };
                let large_arc = if sweep > core::f32::consts::PI { 1 } else { 0 };
                // Rounded, not truncated, so the endpoints match the rasteriser's `.round()` and the
                // two backends cannot disagree by a pixel for the same command.
                let start_x = center.x + (*radius as f32 * start.cos()).round() as i32;
                let start_y = center.y + (*radius as f32 * start.sin()).round() as i32;
                let end_x = center.x + (*radius as f32 * end.cos()).round() as i32;
                let end_y = center.y + (*radius as f32 * end.sin()).round() as i32;
                let fill = if *filled { color_to_rgba(color) } else { "none".to_string() };
                let stroke = if *filled { "none".to_string() } else { color_to_rgba(color) };
                if *filled {
                    // Pie/wedge shape: center → arc start → arc → arc end → close to center
                    self.push_element(format!(
                        r##"<path d="M {} {} L {} {} A {} {} 0 {} 1 {} {} Z" fill="{}" stroke="{}" />"##,
                        center.x, center.y,
                        start_x, start_y,
                        radius, radius, large_arc, end_x, end_y,
                        fill, stroke
                    ));
                } else {
                    self.push_element(format!(
                        r##"<path d="M {start_x} {start_y} A {radius} {radius} 0 {large_arc} 1 {end_x} {end_y}" fill="{fill}" stroke="{stroke}" />"##
                    ));
                }
            }

            // ── Path ────────────────────────────────────────────────────
            RenderCommand::DrawPath { points, closed, color, filled, width } => {
                if points.is_empty() {
                    return;
                }
                let mut d = format!("M {} {}", points[0].x, points[0].y);
                for pt in &points[1..] {
                    d.push_str(&format!(" L {} {}", pt.x, pt.y));
                }
                if *closed {
                    d.push_str(" Z");
                }
                let fill = if *filled { color_to_rgba(color) } else { "none".to_string() };
                let stroke = if *filled { "none".to_string() } else { color_to_rgba(color) };
                self.push_element(format!(
                    r##"<path d="{d}" fill="{fill}" stroke="{stroke}" stroke-width="{width}" />"##
                ));
            }
            RenderCommand::BoxShadow { rect, color, offset_x, offset_y, blur_radius, spread } => {
                // The geometry and the colour both mirror the software backend exactly, so the two
                // backends draw one shadow: the rect is offset and spread by the same amounts, and
                // the colour's alpha is **halved** — the rasteriser's `(color.a * 0.5)` is the
                // shadow's weight, and emitting the un-halved alpha here made every shadow twice as
                // dark in the snapshot as on screen. The rect is square-cornered for the same
                // reason: the rasteriser fills a plain rect and blurs it, so a fixed `rx` was a
                // second, unfounded shape.
                let spread_w = (rect.width as i32 + *spread * 2).max(0) as u32;
                let spread_h = (rect.height as i32 + *spread * 2).max(0) as u32;
                let x = rect.x + offset_x - *spread;
                let y = rect.y + offset_y - *spread;
                let shadow = Color::rgba(color.r, color.g, color.b, (color.a as f32 * 0.5) as u8);
                let filter_attr = if *blur_radius > 0 {
                    let filter_id = format!("shadow_blur_{blur_radius}");
                    self.push_element(format!(
                        r##"<filter id="{filter_id}"><feGaussianBlur stdDeviation="{blur_radius}" /></filter>"##
                    ));
                    format!(r##" filter="url(#{filter_id})""##)
                } else {
                    String::new()
                };
                self.push_element(format!(
                    r##"<rect x="{}" y="{}" width="{}" height="{}" fill="{}"{} />"##,
                    x,
                    y,
                    spread_w,
                    spread_h,
                    color_to_rgba(&shadow),
                    filter_attr
                ));
            }
            RenderCommand::Blur { radius } => {
                // A blur is a property of the *drawing* it applies to, not a standalone element, so
                // this opens a filtered group that everything until the next `Blur` lands in. The
                // previous form emitted a bare `<filter>` definition that **no element referenced**,
                // so a `Blur` command drew nothing at all.
                self.close_blur_group();
                if *radius > 0 {
                    let filter_id = format!("blur_{radius}");
                    self.push_element(format!(
                        r##"<filter id="{filter_id}"><feGaussianBlur stdDeviation="{radius}" /></filter>"##
                    ));
                    self.push_element(format!(r##"<g filter="url(#{filter_id})">"##));
                    self.blur_group_open = true;
                }
            }
            RenderCommand::ClipPath { points } => {
                if !points.is_empty() {
                    let clip_id = format!("cp_{}", self.clip_path_counter);
                    self.clip_path_counter += 1;
                    let mut d = format!("M {} {}", points[0].x, points[0].y);
                    for pt in &points[1..] {
                        d.push_str(&format!(" L {} {}", pt.x, pt.y));
                    }
                    d.push_str(" Z");
                    self.push_element(format!(
                        r##"<clipPath id=\"{clip_id}\"><path d=\"{d}\" /></clipPath>"##
                    ));
                    self.push_element(format!(r##"<g clip-path=\"url(#{clip_id})\">"##));
                    self.clip_depth += 1;
                }
            }
            RenderCommand::SetBlendMode { mode } => {
                // Every `BlendMode` has an exact CSS `mix-blend-mode` counterpart, so this is a real
                // mapping rather than a lossy approximation: the software backend composites with
                // its own arithmetic and a viewer composites with the CSS operators, and the two
                // agree because SVG defines these operators to match the Porter-Duff/separable
                // formulas the rasteriser uses.
                //
                // # Why a wrapping group rather than a property on each element
                //
                // A blend mode applies to *everything drawn until it changes*, which is a
                // sequential fact. `mix-blend-mode` is a per-element style, so the mode is opened as
                // a `<g style="mix-blend-mode:…">` here and closed when the next mode arrives or at
                // `finish`. Emitting it on each element would work too, but it would rewrite every
                // element's attributes with a style that is really one piece of frame state.
                self.close_blend_group();
                if *mode != BlendMode::Normal {
                    self.push_element(format!(
                        r##"<g style="mix-blend-mode:{}">"##,
                        blend_mode_to_css(*mode)
                    ));
                    self.blend_group_open = true;
                }
            }
            RenderCommand::DrawConicGradient { center, start_angle, stops } => {
                // SVG has no native conic gradient, so the sweep is drawn as a fan of wedges, each
                // filled with the colour the ramp reaches at its own angle. `draw_conic_fan` samples
                // with the **same** `atan2` convention the software backend's per-pixel loop uses, so
                // a wedge carries the colour the rasteriser would put there.
                if stops.is_empty() {
                    return;
                }
                // The fan must reach the far corner of the canvas from the centre, or a wedge near
                // the edge would stop short of it.
                let radius = radius_to_far_corner(*center, self.size.width, self.size.height);
                draw_conic_fan(self, *center, radius, *start_angle, stops);
            }
        }
    }

    fn size(&self) -> Size {
        self.size
    }

    fn set_size(&mut self, size: Size) {
        self.size = size;
    }

    fn dpi_scale(&self) -> f32 {
        self.dpi_scale
    }

    fn set_dpi_scale(&mut self, dpi_scale: f32) {
        self.dpi_scale = dpi_scale;
    }

    fn measure_text(&self, text: &str, font: &Font) -> TextMetrics {
        // Same advance heuristic as the software rasteriser, so layout computed
        // against the SVG output agrees with the rasterised frame.
        let scale = self.dpi_scale;
        // The line box comes from `TextMetrics::for_font`, the one derivation both backends use.
        // This method used to spell it out with a `.max(1.0)` the software surface did not apply,
        // which made the two disagree about the ascent for a small leading — see that function.
        let shaped = self.shape_text(text, font);
        let width = shaped.advance().round() as u32;
        TextMetrics { width, ..TextMetrics::for_font(font, scale) }
    }

    fn shape_text(&self, text: &str, font: &Font) -> ShapedText {
        // One run of clusters in visual order, from the text layer's single derivation — the same
        // call the software surface makes, so the vector and raster backends wrap and position
        // text identically (principle #51: one derivation, not two copies that must be kept in
        // step). Reordering for a right-to-left line is applied there, once, for both.
        crate::render::text::shape_line(text, font, self.dpi_scale)
    }

    /// The SVG backend produces vector markup, not a raster surface, so it has no
    /// packed RGBA frame to hand back. Returning an empty slice is the honest
    /// answer for a vector-only backend: the output is [`SvgPaintBackend::finish`].
    fn frame_rgba(&self) -> &[u8] {
        &[]
    }

    fn apply_render_config(&mut self, _config: SoftwareRenderConfig) {}

    fn render_config(&self) -> SoftwareRenderConfig {
        SoftwareRenderConfig::default()
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};

    #[test]
    fn svg_backend_creates_valid_document() {
        let mut svg = SvgPaintBackend::new(Size::new(100, 50));
        svg.begin_frame(Color::WHITE);
        svg.execute_command(&RenderCommand::FillRect {
            rect: Rect::new(10, 10, 30, 20),
            color: Color::rgb(255, 0, 0),
        });
        svg.end_frame();
        let result = svg.finish();
        assert!(result.contains("<svg"));
        assert!(result.contains("</svg>"));
        assert!(result.contains("fill=\"rgba(255,0,0,1.00)\""));
    }

    #[test]
    fn svg_backend_text_is_glyph_geometry_not_a_text_element() {
        // The string is emitted as the set bits of its `font8x8` bitmaps, so there is nothing
        // for XML to escape and no `<text>` to hand the viewer's font engine. That is the point:
        // the SVG is the *rasteriser's* drawing, so a viewer cannot substitute a different font
        // and move the ink. `<` is the fallback "tofu" glyph, so it still produces rectangles.
        let mut svg = SvgPaintBackend::new(Size::new(100, 50));
        svg.begin_frame(Color::WHITE);
        svg.execute_command(&RenderCommand::DrawText {
            origin: Point::new(10, 20),
            text: "<hello> & world".to_string(),
            font: Font::default_ui(),
            color: Color::BLACK,
            alignment: HorizontalAlignment::Left,
        });
        svg.end_frame();
        let result = svg.finish();
        assert!(
            !result.contains("<text"),
            "a `<text>` element would be rendered by the viewer's font, not by this crate"
        );
        assert!(result.contains("<path d=\"M"), "the string must be drawn as glyph geometry");
    }

    /// A vector face is emitted as a real **outline**, and a 1-bit face as rectangles.
    ///
    /// # Why the distinction is asserted on `L` commands rather than on painted output
    ///
    /// The two glyph paths differ in *geometry*, and the difference is exactly this: a 1-bit face's
    /// subpaths are `M{x} {y}h{w}v{h}h-{w}z` — axis-aligned runs with no `L` and integer
    /// coordinates — while an outline's are `M{a.b} {c.d}L...Z` with `L`s and decimals. So counting
    /// `L`s is the cheapest honest discriminator, and it fails loudly if the outline path is ever
    /// silently bypassed (which is what happened while `text::outline` was being written: every
    /// glyph fell through to the rectangles and no coverage-level test noticed).
    ///
    /// The test is gated on a vector feature because without one there is no outline face and the
    /// rectangle path is the *correct* answer, not a fallback.
    #[cfg(any(feature = "fonts-vector-latin", cjk_outline_face))]
    #[test]
    fn svg_backend_emits_an_outline_for_a_vector_face() {
        // A Latin face this build ships. The outline path selects by `Font::family`, so the name
        // has to match a feature that is on — `fonts-complex` ships only Arabic and would correctly
        // fall through to the bitmap path for `A`, which is not what this test is about.
        let family =
            if cfg!(feature = "fonts-vector-latin") { "Open Sans" } else { "Noto Sans SC" };
        let mut svg = SvgPaintBackend::new(Size::new(120, 48));
        svg.begin_frame(Color::TRANSPARENT);
        svg.execute_command(&RenderCommand::DrawText {
            origin: Point::new(4, 4),
            text: "A".to_string(),
            // Named as `Font::family` names it: the outline path selects by family for the reason
            // `text::outline` documents.
            font: Font::new(family, 32.0, false, false),
            color: Color::WHITE,
            alignment: HorizontalAlignment::Left,
        });
        svg.end_frame();
        let result = svg.finish();
        let path = result
            .split("<path d=\"")
            .nth(1)
            .and_then(|rest| rest.split('"').next())
            .expect("the backend emitted a glyph path");
        assert!(
            path.contains('L') && path.contains('.'),
            "a vector face must be emitted as an outline (`L` commands, fractional coordinates), \
             not as bitmap rectangles; got: {path}"
        );
        // The non-zero fill rule is what keeps a counter a hole, and an outline is the only ink that
        // needs it — so its presence is part of the same claim.
        assert!(
            result.contains("fill-rule=\"nonzero\""),
            "an outline path must carry the non-zero fill rule so a counter stays a hole"
        );
    }

    /// The default build's element format is unchanged: rectangles and **no** `fill-rule`.
    ///
    /// The attribute is emitted only when an outline face exists, because adding it unconditionally
    /// would rewrite all 376 committed snapshots for no behavioural change. This pins that.
    #[cfg(not(any(feature = "fonts-vector-latin", feature = "fonts-complex", cjk_outline_face)))]
    #[test]
    fn svg_backend_emits_rectangles_without_a_fill_rule_by_default() {
        let mut svg = SvgPaintBackend::new(Size::new(120, 48));
        svg.begin_frame(Color::TRANSPARENT);
        svg.execute_command(&RenderCommand::DrawText {
            origin: Point::new(4, 4),
            text: "A".to_string(),
            // No outline face exists on this build, so any family name resolves to the bitmap face.
            font: Font::new("Arial", 32.0, false, false),
            color: Color::WHITE,
            alignment: HorizontalAlignment::Left,
        });
        svg.end_frame();
        let result = svg.finish();
        let path = result
            .split("<path d=\"")
            .nth(1)
            .and_then(|rest| rest.split('"').next())
            .expect("the backend emitted a glyph path");
        assert!(
            !path.contains('L') && !path.contains('.'),
            "the 8x8 face's ink is whole pixels, so its subpaths are integer runs with no `L`; \
             got: {path}"
        );
        assert!(
            !result.contains("fill-rule"),
            "the default build must not gain an attribute: it would rewrite every committed snapshot"
        );
    }

    #[test]
    fn svg_backend_honours_horizontal_alignment() {
        // Holds the crate-wide theme guard: this test renders, and a concurrent
        // test that switches the appearance would otherwise change a later frame.
        let _theme_guard = crate::style::theme_test_guard();
        // Rule: the two backends must put the ink in the same place. The software rasteriser
        // shifts the pen before the first glyph by the alignment (half an advance for
        // `Center`, a whole one for `Right`); this backend used to drop the field entirely,
        // so every centred or right-aligned label in the crate was left-aligned in SVG
        // output. That was visible in the committed snapshots — a wizard's "Cancel", "Back"
        // and "Finish" all started at their button's left edge — and it made the SVG
        // surface an unreliable judge of any layout work.
        let font = Font::simple("Arial", 12.0);
        let origin = Point::new(100, 20);
        let region = |alignment| {
            let mut svg = SvgPaintBackend::new(Size::new(200, 50));
            svg.begin_frame(Color::WHITE);
            svg.execute_command(&RenderCommand::DrawText {
                origin,
                text: "Cancel".to_string(),
                font: font.clone(),
                color: Color::BLACK,
                alignment,
            });
            svg.end_frame();
            svg.finish()
        };
        let width = {
            let svg = SvgPaintBackend::new(Size::new(200, 50));
            svg.measure_text("Cancel", &font).width as i32
        };
        assert!(width > 0, "the fixture must have a measurable label");

        // The leftmost `M` subpath start in the emitted `<path>` is the alignment's effect, so
        // the assertion is about where the ink actually begins rather than about an attribute.
        //
        // The coordinate is parsed as `f32` and rounded, not as `i32`: with a vector face the ink
        // is an outline whose coordinates carry fractions (`M21.45 …`), and an integer parse would
        // fail on every subpath of every glyph. Rounding here is what keeps the assertion about
        // the alignment shift, which is a whole number of device pixels, rather than about the
        // glyph's own sub-pixel placement.
        let first_ink_x = |svg: &str| -> i32 {
            let d = svg.find("<path d=\"").expect("the backend emitted a glyph path") + 9;
            let end = svg[d..].find('"').expect("the attribute is closed") + d;
            svg[d..end]
                .split('M')
                .skip(1)
                .filter_map(|sub| sub.split([' ', 'h', 'L']).next()?.parse::<f32>().ok())
                .map(|x| x.round() as i32)
                .min()
                .expect("the path has at least one subpath")
        };

        // The shift between the three alignments is what the rule is about, and it is exact in the
        // pen: the origin moves by half (centre) or all (right) of the measured **total** advance —
        // the same quantity `draw_text`'s `adjusted_origin_x` uses, not a per-glyph box.
        //
        // The *ink* the assertion reads back, though, is the outline face's own left-most drawn
        // point, which rounds to a device pixel a half-step away from the pen. Under a bitmap face
        // the ink began exactly at the pen; under an outline face it can differ by one pixel, so
        // the comparison is `within one pixel` rather than exact. The shift itself is what the
        // rule is about, and one pixel of sub-pixel placement does not change that.
        let left = first_ink_x(&region(HorizontalAlignment::Left));
        let centre = first_ink_x(&region(HorizontalAlignment::Center));
        let right = first_ink_x(&region(HorizontalAlignment::Right));
        let total_advance = {
            let svg = SvgPaintBackend::new(Size::new(200, 50));
            svg.shape_text("Cancel", &font).advance()
        };
        const SLACK: i32 = 1;
        let expected_centre = -(total_advance / 2.0).round() as i32;
        assert!(
            (centre - left - expected_centre).abs() <= SLACK,
            "centre shifts half the measured advance to the left: got {} want {expected_centre}",
            centre - left
        );
        let expected_right = -total_advance.round() as i32;
        assert!(
            (right - left - expected_right).abs() <= SLACK,
            "right shifts the whole measured advance to the left: got {} want {expected_right}",
            right - left
        );
        assert!(left >= origin.x - SLACK, "left-aligned ink starts at or after the origin");
    }

    #[test]
    fn svg_backend_all_command_types() {
        let mut svg = SvgPaintBackend::new(Size::new(200, 200));
        svg.begin_frame(Color::WHITE);

        // Test all command types compile and produce output
        svg.execute_command(&RenderCommand::FillRect {
            rect: Rect::new(0, 0, 50, 50),
            color: Color::RED,
        });
        svg.execute_command(&RenderCommand::FillRoundedRect {
            rect: Rect::new(50, 0, 50, 50),
            radius: 5,
            color: Color::GREEN,
        });
        svg.execute_command(&RenderCommand::DrawRect {
            rect: Rect::new(0, 50, 50, 50),
            color: Color::BLUE,
        });
        svg.execute_command(&RenderCommand::DrawRectStroke {
            rect: Rect::new(50, 50, 50, 50),
            color: Color::BLACK,
            width: 2,
        });
        svg.execute_command(&RenderCommand::DrawLine {
            from: Point::new(0, 100),
            to: Point::new(50, 150),
            color: Color::RED,
        });
        svg.execute_command(&RenderCommand::DrawLineStroke {
            from: Point::new(50, 100),
            to: Point::new(100, 150),
            color: Color::GREEN,
            width: 3,
        });
        svg.execute_command(&RenderCommand::FillCircle {
            center: Point::new(30, 130),
            radius: 15,
            color: Color::BLUE,
        });
        svg.execute_command(&RenderCommand::DrawCircle {
            center: Point::new(80, 130),
            radius: 15,
            color: Color::BLACK,
        });
        svg.execute_command(&RenderCommand::DrawCircleStroke {
            center: Point::new(130, 130),
            radius: 15,
            color: Color::RED,
            width: 2,
        });
        svg.execute_command(&RenderCommand::DrawText {
            origin: Point::new(10, 180),
            text: "Test".to_string(),
            font: Font::default_ui(),
            color: Color::BLACK,
            alignment: HorizontalAlignment::Left,
        });
        svg.execute_command(&RenderCommand::PushClip { x: 0, y: 0, width: 100, height: 100 });
        svg.execute_command(&RenderCommand::PopClip);
        svg.execute_command(&RenderCommand::DrawImage {
            x: 100,
            y: 0,
            width: 50,
            height: 50,
            data: vec![],
        });

        svg.end_frame();
        let result = svg.finish();
        assert!(result.contains("<svg"));
        assert!(result.contains("</svg>"));
        assert!(result.contains("stroke"));
        assert!(result.contains("fill"));
        assert!(result.contains("Image (no data)"));
        assert!(result.contains("#fee"));
        assert!(result.contains("#c00"));
    }

    #[test]
    fn svg_backend_begin_frame_clears() {
        let mut svg = SvgPaintBackend::new(Size::new(10, 10));
        svg.begin_frame(Color::rgb(200, 200, 200));
        assert!(svg.elements.len() == 1);
        svg.end_frame();
        let result = svg.finish();
        assert!(result.contains("200"));
    }

    #[test]
    fn svg_backend_draw_rounded_rect_stroke() {
        let mut svg = SvgPaintBackend::new(Size::new(100, 100));
        svg.begin_frame(Color::WHITE);
        svg.execute_command(&RenderCommand::DrawRoundedRectStroke {
            rect: Rect::new(10, 10, 80, 80),
            radius: 8,
            color: Color::BLUE,
            width: 2,
        });
        svg.end_frame();
        let result = svg.finish();
        assert!(result.contains("rx=\"8\""));
        assert!(result.contains("ry=\"8\""));
        assert!(result.contains("stroke-width=\"2\""));
    }

    #[test]
    fn svg_backend_draw_line_aa() {
        let mut svg = SvgPaintBackend::new(Size::new(100, 100));
        svg.begin_frame(Color::TRANSPARENT);
        svg.execute_command(&RenderCommand::DrawLineAA {
            from: Point::new(5, 5),
            to: Point::new(95, 95),
            color: Color::RED,
        });
        svg.end_frame();
        let result = svg.finish();
        assert!(result.contains("x1=\"5\""));
        assert!(result.contains("y1=\"5\""));
        assert!(result.contains("x2=\"95\""));
        assert!(result.contains("y2=\"95\""));
    }

    #[test]
    fn svg_backend_fill_circle_aa() {
        let mut svg = SvgPaintBackend::new(Size::new(100, 100));
        svg.begin_frame(Color::WHITE);
        svg.execute_command(&RenderCommand::FillCircleAA {
            center: Point::new(50, 50),
            radius: 25,
            color: Color::GREEN,
        });
        svg.end_frame();
        let result = svg.finish();
        assert!(result.contains("<circle"));
        assert!(result.contains("cx=\"50\""));
        assert!(result.contains("r=\"25\""));
    }

    #[test]
    fn svg_backend_multiple_frames() {
        let mut svg = SvgPaintBackend::new(Size::new(50, 50));
        svg.begin_frame(Color::WHITE);
        svg.execute_command(&RenderCommand::FillRect {
            rect: Rect::new(0, 0, 25, 25),
            color: Color::RED,
        });
        svg.end_frame();

        let frame1 = svg.finish();
        assert!(frame1.contains("rgba(255,0,0"));

        // Start second frame
        svg.begin_frame(Color::BLACK);
        // Background fill rect is pushed (BLACK has alpha > 0)
        assert!(svg.elements.len() == 1);
        svg.end_frame();
        let frame2 = svg.finish();
        assert!(frame2.contains("rgba(0,0,0"));
    }

    #[test]
    fn svg_backend_clip_nesting() {
        let mut svg = SvgPaintBackend::new(Size::new(100, 100));
        svg.begin_frame(Color::WHITE);
        svg.execute_command(&RenderCommand::PushClip { x: 10, y: 10, width: 50, height: 50 });
        svg.execute_command(&RenderCommand::PushClip { x: 20, y: 20, width: 30, height: 30 });
        svg.execute_command(&RenderCommand::PopClip);
        svg.execute_command(&RenderCommand::PopClip);
        svg.end_frame();
        let result = svg.finish();
        assert!(result.contains("clip_0"));
        assert!(result.contains("clip_1"));
    }

    #[test]
    fn svg_backend_draws_a_conic_gradient_as_wedges() {
        // A conic ramp has no SVG paint server, so the backend must draw it as geometry. This
        // guards the gap the fan closes: the previous form downgraded a conic to a **linear**
        // gradient, which is a wrong picture rather than a missing one, and the snapshot could not
        // tell the difference because nothing asserted on it.
        let mut svg = SvgPaintBackend::new(Size::new(120, 120));
        svg.begin_frame(Color::TRANSPARENT);
        svg.execute_command(&RenderCommand::DrawConicGradient {
            center: Point::new(60, 60),
            start_angle: 0.0,
            stops: vec![
                (0.0, Color::rgb(255, 0, 0)),
                (0.5, Color::rgb(0, 255, 0)),
                (1.0, Color::rgb(0, 0, 255)),
            ],
        });
        svg.end_frame();
        let result = svg.finish();
        assert!(
            !result.contains("<linearGradient"),
            "a conic must not be silently approximated by a linear gradient"
        );
        // One `<path>` per wedge, so a fan is present rather than a single shape.
        let wedges = result.matches("<path d=\"M 60 60 L").count();
        assert!(wedges >= 300, "the sweep is drawn as a wedge fan; got {wedges} wedges");
        // The ramp reached the fan: every emitted wedge colour is a blend of the stops, and the
        // reddest one (positions near 0) and bluest one (positions near 1) both appear. Sampling
        // happens at each wedge's midpoint, so the ramp never lands exactly on a stop's own
        // `rgba` — the assertion is on the fan carrying the ramp, not on an exact stop match.
        let filler = |needle: &str| result.matches(needle).count();
        assert!(filler("rgba(2") > 0, "the red end of the ramp is drawn");
        assert!(
            filler("rgba(0,0,") > 0 || result.contains("rgba(0,0,255"),
            "the blue end is drawn"
        );
    }

    #[test]
    fn a_covered_glyph_whose_ink_clips_away_draws_nothing_not_a_bitmap() {
        // A narrow glyph — `i` at a size whose estimate-based advance rounds to one pixel — has real
        // outline ink, but that ink lies outside the one-pixel cell (measured `x=[1.09, 2.79]` in a
        // `0..1` cell). The rasteriser paints the same cell and writes **zero** pixels, so the honest
        // snapshot is nothing. The defect this guards: treating "clipped away" as "not covered" fell
        // through to the 8x8 bitmap path, which drew a bitmap `i` inside an otherwise vector word.
        //
        // The build must carry an outline face for the outline path to exist at all; without one every
        // glyph is a bitmap and this is not the question being asked.
        let has_outline =
            cfg!(any(feature = "fonts-vector-latin", feature = "fonts-complex", cjk_outline_face));
        if !has_outline {
            return;
        }
        let mut svg = SvgPaintBackend::new(Size::new(40, 40));
        svg.begin_frame(Color::TRANSPARENT);
        // `i` advances a fraction of an em; at this size the rounded cell is one pixel wide.
        svg.execute_command(&RenderCommand::DrawText {
            origin: Point::new(0, 0),
            text: "i".to_string(),
            font: Font::new("Arial", 12.0, false, false),
            color: Color::BLACK,
            alignment: HorizontalAlignment::Left,
        });
        svg.end_frame();
        let document = svg.finish();
        assert!(
            !document.contains("h1v1h-1z") && !document.contains("h1v2h-1z"),
            "a covered glyph whose outline clips away must not fall back to bitmap ink; got: {document}"
        );
    }

    /// A wraparound arc must be the **short** sweep, as the command's own contract states.
    ///
    /// `DrawArc`'s docs say a `start` of 350° and an `end` of 10° sweeps 20°, not 340°. The software
    /// backend has an explicit wraparound branch for that; the SVG backend computed
    /// `large-arc-flag` from the raw `|end - start|`, so the pair measured ~5.93 rad > π and set the
    /// flag to 1 — selecting the **≥180°** arc. One `RenderCommand` therefore drew 340° in a snapshot
    /// and 20° on the rasteriser.
    #[test]
    fn a_wraparound_arc_is_the_short_sweep() {
        fn arc_path(start_deg: f32, end_deg: f32) -> String {
            let mut svg = SvgPaintBackend::new(Size::new(100, 100));
            svg.begin_frame(Color::TRANSPARENT);
            svg.execute_command(&RenderCommand::DrawArc {
                center: Point::new(50, 50),
                radius: 30,
                start_angle: start_deg.to_radians(),
                end_angle: end_deg.to_radians(),
                color: Color::BLACK,
                filled: false,
            });
            svg.end_frame();
            svg.finish().lines().find(|line| line.contains("<path")).unwrap_or_default().to_string()
        }

        // 350° -> 10° is a 20° sweep: under half a turn, so `large-arc-flag` must be 0.
        let wrap = arc_path(350.0, 10.0);
        assert!(
            wrap.contains("A 30 30 0 0 1"),
            "a 20-degree wraparound arc must not set large-arc-flag; got: {wrap}"
        );
        // The mirrored case is the same fact from the other side: it must still be the long way
        // round when the sweep really is more than half a turn.
        let long = arc_path(10.0, 350.0);
        assert!(
            long.contains("A 30 30 0 1 1"),
            "a 340-degree sweep must set large-arc-flag; got: {long}"
        );
    }

    /// The conic ramp's start angle is applied to the **sampled index**, not the wedge geometry.
    ///
    /// That is what the software backend does (`pos = ((atan2 + PI) + start) / TAU`), and matching it
    /// is what makes a snapshot agree with the rasteriser. Rotating the geometry instead — as the fan
    /// once did — moves each wedge but leaves it carrying the colour of the place it came from.
    #[test]
    fn a_conic_start_angle_rotates_the_ramp_not_the_geometry() {
        let stops = [
            (0.0, Color::rgb(255, 0, 0)),
            (0.5, Color::rgb(0, 255, 0)),
            (1.0, Color::rgb(0, 0, 255)),
        ];
        // Sampling straight-up with no offset and with a half-turn offset must differ; if the
        // parameter were ignored (as it once was) they would be identical.
        let plain = conic_sample(&stops, 0.25, 0.0);
        let turned = conic_sample(&stops, 0.25, core::f32::consts::PI);
        assert_ne!(plain, turned, "the start angle must reach the sampled ramp index");
        // A full turn is the identity, which pins the units (radians) as well.
        assert_eq!(plain, conic_sample(&stops, 0.25, core::f32::consts::TAU));
    }

    #[test]
    fn radius_to_far_corner_reaches_every_corner() {
        // A fan under-covers if the radius is short, so the helper is asserted against the true
        // farthest corner for centres inside, on, and outside the box.
        for centre in [Point::new(0, 0), Point::new(5, 7), Point::new(10, 10), Point::new(12, 3)] {
            let r = radius_to_far_corner(centre, 10, 10);
            let (cx, cy) = (centre.x as f32, centre.y as f32);
            for corner in [(0.0, 0.0), (10.0, 0.0), (0.0, 10.0), (10.0, 10.0)] {
                let d = (corner.0 - cx).hypot(corner.1 - cy);
                assert!(r >= d, "radius {r} covers corner at distance {d} from {centre:?}");
            }
        }
        // The result is the *max* corner distance, not more: a fan twice as wide as needed would
        // still be clipped, but an exact value keeps the emitted geometry tight.
        let r = radius_to_far_corner(Point::new(0, 0), 10, 10);
        assert!((r - (200.0f32).sqrt()).abs() < 0.001, "the diagonal is the farthest corner: {r}");
    }

    #[test]
    fn every_render_command_variant_reaches_the_svg_backend() {
        // The rule (BLUE20): the SVG snapshot is only a faithful picture if every command the
        // software backend can execute also produces markup here. A variant that fell through to a
        // catch-all would draw *nothing*, and a nothing in the snapshot is indistinguishable from a
        // control that legitimately has no ink.
        //
        // The guard is the `name_of` match below: it is exhaustive over `RenderCommand`, so adding
        // a variant to the enum stops this test compiling until the new variant is added to the
        // fixture list *and* given a name here. The runtime half then drives each fixture through a
        // live backend and asserts the document grew.
        fn name_of(command: &RenderCommand) -> &'static str {
            match command {
                RenderCommand::FillRect { .. } => "FillRect",
                RenderCommand::DrawRect { .. } => "DrawRect",
                RenderCommand::DrawRectStroke { .. } => "DrawRectStroke",
                RenderCommand::FillRoundedRect { .. } => "FillRoundedRect",
                RenderCommand::FillRoundedRectAA { .. } => "FillRoundedRectAA",
                RenderCommand::DrawRoundedRectStroke { .. } => "DrawRoundedRectStroke",
                RenderCommand::DrawRoundedRectStrokeAA { .. } => "DrawRoundedRectStrokeAA",
                RenderCommand::DrawLine { .. } => "DrawLine",
                RenderCommand::DrawLineAA { .. } => "DrawLineAA",
                RenderCommand::DrawLineStroke { .. } => "DrawLineStroke",
                RenderCommand::DrawLineStrokeAA { .. } => "DrawLineStrokeAA",
                RenderCommand::FillCircle { .. } => "FillCircle",
                RenderCommand::FillCircleAA { .. } => "FillCircleAA",
                RenderCommand::DrawCircle { .. } => "DrawCircle",
                RenderCommand::DrawCircleStroke { .. } => "DrawCircleStroke",
                RenderCommand::DrawText { .. } => "DrawText",
                RenderCommand::DrawImage { .. } => "DrawImage",
                RenderCommand::PushClip { .. } => "PushClip",
                RenderCommand::PopClip => "PopClip",
                RenderCommand::DrawGradient { .. } => "DrawGradient",
                RenderCommand::DrawArc { .. } => "DrawArc",
                RenderCommand::DrawPath { .. } => "DrawPath",
                RenderCommand::BoxShadow { .. } => "BoxShadow",
                RenderCommand::Blur { .. } => "Blur",
                RenderCommand::ClipPath { .. } => "ClipPath",
                RenderCommand::SetBlendMode { .. } => "SetBlendMode",
                RenderCommand::DrawConicGradient { .. } => "DrawConicGradient",
            }
        }

        let rect = Rect::new(20, 20, 60, 40);
        let point = Point::new(40, 40);
        let commands: Vec<RenderCommand> = vec![
            RenderCommand::FillRect { rect, color: Color::RED },
            RenderCommand::DrawRect { rect, color: Color::GREEN },
            RenderCommand::DrawRectStroke { rect, color: Color::BLUE, width: 2 },
            RenderCommand::FillRoundedRect { rect, radius: 6, color: Color::RED },
            RenderCommand::FillRoundedRectAA { rect, radius: 6, color: Color::GREEN },
            RenderCommand::DrawRoundedRectStroke { rect, radius: 6, color: Color::BLUE, width: 2 },
            RenderCommand::DrawRoundedRectStrokeAA { rect, radius: 6, color: Color::RED, width: 2 },
            RenderCommand::DrawLine { from: point, to: Point::new(80, 80), color: Color::BLUE },
            RenderCommand::DrawLineAA { from: point, to: Point::new(80, 80), color: Color::RED },
            RenderCommand::DrawLineStroke {
                from: point,
                to: Point::new(80, 80),
                color: Color::GREEN,
                width: 3,
            },
            RenderCommand::DrawLineStrokeAA {
                from: point,
                to: Point::new(80, 80),
                color: Color::RED,
                width: 3,
            },
            RenderCommand::FillCircle { center: point, radius: 20, color: Color::BLUE },
            RenderCommand::FillCircleAA { center: point, radius: 20, color: Color::RED },
            RenderCommand::DrawCircle { center: point, radius: 20, color: Color::GREEN },
            RenderCommand::DrawCircleStroke {
                center: point,
                radius: 20,
                color: Color::BLUE,
                width: 2,
            },
            RenderCommand::DrawText {
                origin: point,
                text: "Hi".to_string(),
                font: Font::default_ui(),
                color: Color::BLACK,
                alignment: HorizontalAlignment::Left,
            },
            RenderCommand::DrawImage {
                x: 20,
                y: 20,
                width: 40,
                height: 40,
                data: vec![1, 2, 3, 4],
            },
            RenderCommand::PushClip { x: 10, y: 10, width: 50, height: 50 },
            RenderCommand::PopClip,
            RenderCommand::DrawGradient {
                rect,
                gradient: crate::style::Gradient::linear(point, Point::new(80, 40))
                    .add_stop(0.0, Color::RED)
                    .add_stop(1.0, Color::BLUE),
            },
            RenderCommand::DrawGradient {
                rect,
                gradient: crate::style::Gradient::radial(point, 40.0)
                    .add_stop(0.0, Color::GREEN)
                    .add_stop(1.0, Color::BLUE),
            },
            RenderCommand::DrawGradient {
                rect,
                gradient: crate::style::Gradient::conic(point, 0.0)
                    .add_stop(0.0, Color::RED)
                    .add_stop(1.0, Color::BLUE),
            },
            RenderCommand::DrawArc {
                center: point,
                radius: 20,
                start_angle: 0.0,
                end_angle: 1.5,
                color: Color::RED,
                filled: false,
            },
            RenderCommand::DrawArc {
                center: point,
                radius: 20,
                start_angle: 0.0,
                end_angle: 1.5,
                color: Color::BLUE,
                filled: true,
            },
            RenderCommand::DrawPath {
                points: vec![Point::new(10, 10), Point::new(60, 20), Point::new(40, 70)],
                closed: true,
                color: Color::GREEN,
                filled: false,
                width: 2,
            },
            RenderCommand::DrawPath {
                points: vec![Point::new(10, 10), Point::new(60, 20), Point::new(40, 70)],
                closed: false,
                color: Color::GREEN,
                filled: true,
                width: 2,
            },
            RenderCommand::BoxShadow {
                rect,
                color: Color::rgba(0, 0, 0, 128),
                offset_x: 2,
                offset_y: 2,
                blur_radius: 4,
                spread: 1,
            },
            RenderCommand::Blur { radius: 3 },
            RenderCommand::ClipPath {
                points: vec![Point::new(5, 5), Point::new(95, 5), Point::new(95, 95)],
            },
            RenderCommand::SetBlendMode { mode: BlendMode::Multiply },
            RenderCommand::DrawConicGradient {
                center: point,
                start_angle: 0.0,
                stops: vec![(0.0, Color::RED), (1.0, Color::BLUE)],
            },
        ];

        // Each command is driven on its own against a fresh frame, so the assertion is about
        // *that* command's markup rather than about the frame as a whole.
        //
        // A command may be **explicitly registered** as emitting no markup of its own. The list is
        // asserted to be exactly the set of commands that need it, so an exemption cannot quietly
        // cover a command that stopped drawing: every name here must still be a real variant, and
        // the reason must be a state transition rather than missing code.
        const OUTPUTLESS: &[&str] = &[
            // `PopClip` closes the group its matching `PushClip` opened. Driven with nothing on the
            // clip stack it is correctly a no-op — the markup it removes was never emitted.
            "PopClip",
        ];
        let mut exempted: Vec<&'static str> = Vec::new();
        for command in &commands {
            let name = name_of(command);
            let mut svg = SvgPaintBackend::new(Size::new(120, 120));
            svg.begin_frame(Color::TRANSPARENT);
            let before = svg.elements.len();
            svg.execute_command(command);
            svg.end_frame();
            let document = svg.finish();
            // Balance is a property of the *document*, and it is asserted for every command below
            // whatever its output, because an unbalanced `<g>` is malformed in every case.
            assert_eq!(
                document.matches("<g").count(),
                document.matches("</g>").count(),
                "{name} left an unbalanced `<g>` group in the document"
            );
            if svg.elements.len() > before {
                continue;
            }
            assert!(
                OUTPUTLESS.contains(&name),
                "{name} emitted nothing: a command the software backend executes must reach the \
                 SVG document, or be registered in `OUTPUTLESS` with a reason"
            );
            exempted.push(name);
        }
        // The exemption list must be exactly the set of commands that needed it: a stale entry is a
        // command that either draws nothing and is not listed, or is listed but no longer a fixture.
        for name in OUTPUTLESS {
            assert!(
                exempted.contains(name),
                "`OUTPUTLESS` names {name}, but it emitted markup after all; remove the exemption"
            );
        }
        // Every variant must be represented by at least one fixture. The set of names the fixture
        // list produces is asserted against the names the exhaustive `name_of` match above can
        // return, so a variant that is legal to name but never driven would be caught here as well
        // as at compile time.
        let covered: std::collections::BTreeSet<&'static str> =
            commands.iter().map(name_of).collect();
        let expected: std::collections::BTreeSet<&'static str> = [
            "FillRect",
            "DrawRect",
            "DrawRectStroke",
            "FillRoundedRect",
            "FillRoundedRectAA",
            "DrawRoundedRectStroke",
            "DrawRoundedRectStrokeAA",
            "DrawLine",
            "DrawLineAA",
            "DrawLineStroke",
            "DrawLineStrokeAA",
            "FillCircle",
            "FillCircleAA",
            "DrawCircle",
            "DrawCircleStroke",
            "DrawText",
            "DrawImage",
            "PushClip",
            "PopClip",
            "DrawGradient",
            "DrawArc",
            "DrawPath",
            "BoxShadow",
            "Blur",
            "ClipPath",
            "SetBlendMode",
            "DrawConicGradient",
        ]
        .into_iter()
        .collect();
        assert_eq!(
            covered, expected,
            "the fixture list must drive every `RenderCommand` variant exactly once or more"
        );
    }

    #[test]
    fn svg_backend_blur_wraps_subsequent_drawing_in_a_filter() {
        // The previous form emitted a bare `<filter>` that **no element referenced**, so a `Blur`
        // drew nothing. The fix opens a filtered group; the assertion is that the group exists and
        // that the drawing which follows lands inside it.
        let mut svg = SvgPaintBackend::new(Size::new(100, 100));
        svg.begin_frame(Color::TRANSPARENT);
        svg.execute_command(&RenderCommand::Blur { radius: 4 });
        svg.execute_command(&RenderCommand::FillRect {
            rect: Rect::new(10, 10, 40, 40),
            color: Color::BLUE,
        });
        svg.end_frame();
        let result = svg.finish();
        assert!(result.contains("<filter id=\"blur_4\""), "the blur defines its filter");
        let open = result.find("<g filter=\"url(#blur_4)\">").expect("the blur opens a group");
        let rect = result.find("rgba(0,0,255").expect("the drawing is emitted");
        assert!(open < rect, "the blurred drawing is inside the filtered group");
        // A second blur closes the first group, so groups never nest without bound.
        let mut svg = SvgPaintBackend::new(Size::new(100, 100));
        svg.begin_frame(Color::TRANSPARENT);
        svg.execute_command(&RenderCommand::Blur { radius: 2 });
        svg.execute_command(&RenderCommand::Blur { radius: 3 });
        svg.end_frame();
        let result = svg.finish();
        assert_eq!(
            result.matches("<g filter=").count(),
            2,
            "both blur openings are emitted, one per `Blur` command"
        );
        assert_eq!(
            result.matches("</g>").count(),
            2,
            "each opening is closed: the second `Blur` closes the first group, and `build_svg` \
             closes the last one, so the document is balanced"
        );
    }

    #[test]
    fn svg_backend_blend_mode_opens_a_styled_group() {
        // A blend mode applies to everything until it changes, so it is frame state rather than a
        // per-element attribute. This asserts the group is opened with the CSS counterpart of the
        // mode and that `Normal` (the default) closes it instead of opening another.
        let mut svg = SvgPaintBackend::new(Size::new(100, 100));
        svg.begin_frame(Color::TRANSPARENT);
        svg.execute_command(&RenderCommand::SetBlendMode { mode: BlendMode::Multiply });
        svg.execute_command(&RenderCommand::FillRect {
            rect: Rect::new(10, 10, 40, 40),
            color: Color::RED,
        });
        svg.end_frame();
        let result = svg.finish();
        assert!(
            result.contains("<g style=\"mix-blend-mode:multiply\">"),
            "the mode maps to its CSS counterpart"
        );
        let open = result.find("mix-blend-mode:multiply").expect("the group is open");
        let rect = result.find("rgba(255,0,0").expect("the drawing is emitted");
        assert!(open < rect, "the drawing follows inside the blended group");

        // `Normal` is the default and must close the group rather than open a no-op one.
        let mut svg = SvgPaintBackend::new(Size::new(100, 100));
        svg.begin_frame(Color::TRANSPARENT);
        svg.execute_command(&RenderCommand::SetBlendMode { mode: BlendMode::Screen });
        svg.execute_command(&RenderCommand::SetBlendMode { mode: BlendMode::Normal });
        svg.end_frame();
        let result = svg.finish();
        assert_eq!(result.matches("mix-blend-mode:").count(), 1);
        assert_eq!(result.matches("</g>").count(), 1, "`Normal` closed the previous group");
    }
}
