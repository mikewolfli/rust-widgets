// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Arc and circle drawing helpers.

use crate::core::Point;
use crate::render::RenderContext;

/// Returns a point on the circle at the given angle (in radians).
/// Angle 0 is at 3 o'clock (right), angles increase clockwise.
#[inline]
pub fn point_on_circle(center: Point, radius: f32, angle: f32) -> Point {
    Point::new(
        center.x + (radius * angle.cos()).round() as i32,
        center.y + (radius * angle.sin()).round() as i32,
    )
}

/// Draws an arc from `start_angle` to `end_angle` using line segments.
///
/// # Why the segment count is derived rather than fixed
///
/// The count used to be a constant `40` regardless of the arc's size. A fixed count is
/// only correct for one radius: the step between two samples is
/// `radius * total_angle / segments` pixels along the curve, so a small radius or a short
/// sweep makes consecutive samples land on the **same rounded pixel**, and
/// `point_on_circle` rounds each endpoint to an integer. The emitted chords are then
/// zero-length lines: they draw nothing, and a 40-sample arc collapses to the handful of
/// distinct pixels the rounding happens to produce. At the spinner's geometry
/// (`radius = 48`, sweep = 2.4 rad) the arc's true length is about 115 px, but rounding 40
/// samples over it left only **three** distinct coordinates — `snapshots/svg/spinner.svg`
/// carried a filled wedge where a smooth arc belongs, plus the zero-length `<line>`
/// elements that gave the collapse away. That is the same defect class as a sub-pixel
/// rect: a size derived for one case, silently wrong for every other.
///
/// The count is therefore `ceil(arc_length / stride)`, clamped to at least two samples
/// (one chord) and at most the old 40 (no arc needs more at these radii). The stride is
/// one pixel, so a sample is emitted at every pixel the curve crosses and no chord is
/// shorter than a pixel — which is exactly the resolution the integer target can express.
/// A degenerate chord is skipped rather than emitted, because a zero-length line is a
/// drawing command that produces no ink and breaks the "never emit a zero-extent element"
/// rule this crate's other primitives already follow.
pub fn draw_arc_segments(
    context: &mut RenderContext,
    center: Point,
    radius: f32,
    start_angle: f32,
    end_angle: f32,
    color: crate::core::Color,
    stroke_width: u32,
) {
    let mut previous: Option<Point> = None;
    for_each_arc_sample(center, radius, start_angle, end_angle, |current| {
        if let Some(previous) = previous {
            context.draw_line_stroke(previous, current, color, stroke_width);
        }
        previous = Some(current);
    });
}

/// The single arc-sampling kernel.
///
/// Emits the start point, then every subsequent sample whose rounded position differs from
/// the previous one. Both [`draw_arc_segments`] and the software backend's command-level
/// `draw_arc` walk the same sequence, so an arc is sampled identically whether it arrives as
/// a widget call or as a `DrawArc` command. A sample that rounds onto the previous pixel is
/// dropped: its chord would draw no ink, and skipping it cannot open a gap because the next
/// sample continues from the same `previous` point.
///
/// This used to be a private loop inside [`draw_arc_segments`]; the software backend had a
/// second, independent sampler (fixed `0.02` rad steps, one `Vec` allocation per call). The
/// two disagreed about precision, endpoints, and whole-turn arcs, so the loop is now shared
/// instead of duplicated.
pub(crate) fn for_each_arc_sample<F>(
    center: Point,
    radius: f32,
    start_angle: f32,
    end_angle: f32,
    mut emit: F,
) where
    F: FnMut(Point),
{
    let total_angle = end_angle - start_angle;
    if total_angle.abs() < 0.001 || radius <= 0.0 {
        return;
    }
    let segments = arc_sample_count(radius, total_angle);
    let step = total_angle / segments as f32;

    let mut previous = point_on_circle(center, radius, start_angle);
    emit(previous);
    for index in 1..=segments {
        let angle = start_angle + step * index as f32;
        let current = point_on_circle(center, radius, angle);
        if current != previous {
            emit(current);
            previous = current;
        }
    }
}

/// Turns the command-level angles into `(start, sweep)`, both non-negative in sweep.
///
/// `DrawArc` carries a start and an end angle; its contract is the forward sweep from
/// `start` to `end`, wrapping through 2π when `end` is behind `start` (350° → 10° sweeps
/// 20°, not 340°). The SVG backend normalises the same way, so this is what keeps the two
/// raster paths agreeing. One whole turn — the `Arc` control at 100 %, a `Canvas` full
/// circle — normalises to `end == start`; reading that as a zero sweep was the defect that
/// left a full-circle command drawing a single line, so it is read as one full turn.
pub(crate) fn normalized_arc_sweep(start_angle: f32, end_angle: f32) -> (f32, f32) {
    const TWO_PI: f32 = core::f32::consts::TAU;
    let normalize = |angle: f32| {
        let mut value = angle % TWO_PI;
        if value < 0.0 {
            value += TWO_PI;
        }
        value
    };
    let start = normalize(start_angle);
    let end = normalize(end_angle);
    let mut sweep = if end >= start { end - start } else { end + TWO_PI - start };
    if sweep < 0.001 && (end_angle - start_angle).abs() >= TWO_PI - 0.001 {
        sweep = TWO_PI;
    }
    (start, sweep)
}

/// How many chords an arc of `radius` over `total_angle` is sampled with.
///
/// One sample per pixel of the arc's own length, clamped to `1..=MAX_ARC_SEGMENTS`.
///
/// # Why the count is a function and not a local
///
/// The count is a *decision* — "how finely is a curve approximated" — and it has to be
/// observable to be testable. Left as a local, the only thing a test could measure was the
/// number of chords actually emitted, and that number is dominated by the rounding of the
/// sampled points rather than by the count: a 3 px arc emits 3 or 4 chords whether it was
/// sampled 3 times or 40, because the extra samples land on the pixel already covered. That
/// made the count untestable while appearing to be tested. Naming it puts the decision
/// itself under test, and keeps the arithmetic in one place for the two callers that need
/// only the number.
pub(crate) fn arc_sample_count(radius: f32, total_angle: f32) -> u32 {
    /// One sample per pixel of arc length.
    const PIXEL_STRIDE: f32 = 1.0;
    /// The ceiling: no arc in this crate needs more, and an unbounded count would let a
    /// large radius emit thousands of chords for no visible gain.
    const MAX_ARC_SEGMENTS: u32 = 40;
    let arc_length = radius * total_angle.abs();
    (arc_length / PIXEL_STRIDE).ceil().max(1.0).min(MAX_ARC_SEGMENTS as f32) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::Vec;
    use crate::core::{Color, Font, Point, Size};
    use crate::render::{PaintBackend, RenderCommand, ShapedText, SvgPaintBackend, TextMetrics};

    /// Records every line the helper emits, so degeneracy can be asserted directly.
    struct LineRecorder {
        lines: Vec<(Point, Point)>,
    }

    impl PaintBackend for LineRecorder {
        fn begin_frame(&mut self, _clear: Color) {}
        fn end_frame(&mut self) {}
        fn execute_command(&mut self, command: &RenderCommand) {
            if let RenderCommand::DrawLineStroke { from, to, .. } = command {
                self.lines.push((*from, *to));
            }
        }
        fn size(&self) -> Size {
            Size::new(240, 120)
        }
        fn set_size(&mut self, _size: Size) {}
        fn dpi_scale(&self) -> f32 {
            1.0
        }
        fn set_dpi_scale(&mut self, _dpi_scale: f32) {}
        fn measure_text(&self, text: &str, font: &Font) -> TextMetrics {
            TextMetrics {
                width: (text.chars().count() as f32 * font.size()).round() as u32,
                height: font.size().round() as u32,
                ascent: (font.size() * 0.8).round() as u32,
                descent: 0,
            }
        }
        fn shape_text(&self, _text: &str, _font: &Font) -> crate::compat::Rc<ShapedText> {
            crate::compat::Rc::new(ShapedText { clusters: Vec::new(), advance: 0.0 })
        }
        fn frame_rgba(&self) -> &[u8] {
            &[]
        }
    }

    fn recorded(radius: f32, sweep: f32) -> Vec<(Point, Point)> {
        let mut backend = LineRecorder { lines: Vec::new() };
        let mut context = RenderContext::new(&mut backend);
        draw_arc_segments(
            &mut context,
            Point::new(120, 60),
            radius,
            -core::f32::consts::FRAC_PI_2,
            -core::f32::consts::FRAC_PI_2 + sweep,
            Color::PRIMARY,
            4,
        );
        backend.lines
    }

    #[test]
    fn an_arc_never_emits_a_zero_length_chord() {
        // The spinner's own geometry: a 48 px radius over a 2.4 rad sweep. With the fixed
        // 40-segment count this emitted 38 chords whose endpoints rounded to the same pixel,
        // leaving about three distinct coordinates on the canvas — a wedge where a smooth arc
        // belongs, plus a stream of `<line>` elements that draw nothing.
        for (radius, sweep) in
            [(48.0f32, 2.4f32), (16.0, 2.4), (48.0, 0.5), (120.0, core::f32::consts::TAU)]
        {
            let lines = recorded(radius, sweep);
            assert!(
                !lines.is_empty(),
                "radius {radius} sweep {sweep}: an arc with a visible length must emit chords"
            );
            for (from, to) in &lines {
                assert_ne!(
                    from, to,
                    "radius {radius} sweep {sweep}: a chord with identical endpoints draws no ink"
                );
            }
        }
    }

    #[test]
    fn a_small_arc_still_traces_a_continuous_curve() {
        // "No zero-length chords" must not be satisfied by emitting almost nothing: the arc
        // has to cover its own length. Consecutive chords share an endpoint by construction, so
        // the trace is continuous exactly when each chord starts where the previous one ended.
        let lines = recorded(48.0, 2.4);
        for pair in lines.windows(2) {
            assert_eq!(pair[0].1, pair[1].0, "consecutive chords must share an endpoint");
        }
        // And the whole sweep must be walked: the last point belongs on the circle at the end
        // angle, not part-way round it.
        let expected_end =
            point_on_circle(Point::new(120, 60), 48.0, -core::f32::consts::FRAC_PI_2 + 2.4);
        assert_eq!(lines.last().expect("chords were emitted").1, expected_end);
    }

    #[test]
    fn the_sample_count_follows_the_arcs_pixel_length() {
        // A *fixed* sample count is correct for exactly one radius: the step between samples is
        // `radius * sweep / count` pixels, so a short arc sampled 40 times spends most of its
        // iterations inside a single pixel. The count is therefore derived from the arc's own
        // length, with the 40-sample ceiling kept as the upper bound.
        //
        // Asserted on the count itself rather than on the emitted chords: the chord count is
        // dominated by pixel rounding — a 3 px arc emits 3 or 4 chords whether it was sampled
        // 3 times or 40 — so measuring chords cannot distinguish a derived count from a fixed
        // one. Naming the count lets the decision be pinned directly.
        assert_eq!(arc_sample_count(6.0, 0.5), 3, "about 3 px of arc is a handful of samples");
        assert_eq!(arc_sample_count(48.0, 2.4), 40, "the spinner's ~115 px arc is clamped");
        assert_eq!(
            arc_sample_count(200.0, core::f32::consts::TAU),
            40,
            "a huge arc is clamped to the ceiling"
        );
        // Degenerate input still yields a usable count, never zero — which would divide by
        // zero in the caller's `step`. The caller guards these, but the function is total.
        assert_eq!(arc_sample_count(0.0, 2.4), 1);
        assert_eq!(arc_sample_count(48.0, 0.0), 1);
    }

    #[test]
    fn an_arc_that_rounds_away_is_skipped_rather_than_drawn_degenerate() {
        // A sweep too small to move a pixel has nothing to draw. Emitting its chords would be
        // the degenerate-element defect; emitting none is the honest answer.
        assert!(recorded(48.0, 0.0).is_empty());
        assert!(recorded(0.0, 2.4).is_empty());
    }

    #[test]
    fn the_svg_backend_matches_the_raster_for_an_arc() {
        // A regression guard for the reason this helper is shared: the arc is drawn through
        // `RenderContext`, so whatever it emits must reach the SVG document as well.
        let mut svg = SvgPaintBackend::new(crate::core::Size::new(240, 120));
        svg.begin_frame(Color::WHITE);
        {
            let mut context = RenderContext::new(&mut svg);
            draw_arc_segments(
                &mut context,
                Point::new(120, 60),
                48.0,
                -core::f32::consts::FRAC_PI_2,
                -core::f32::consts::FRAC_PI_2 + 2.4,
                Color::PRIMARY,
                4,
            );
        }
        svg.end_frame();
        let document = svg.finish();
        assert!(
            document.matches("<line ").count() > 20,
            "a 115 px arc must reach the SVG document as a real curve, not three pixels"
        );
    }

    /// The command contract's wraparound case: 350° → 10° is the 20° sweep through 0, not the
    /// 340° arc the raw angle difference suggests. The SVG backend normalises the same way.
    #[test]
    fn a_wraparound_command_sweeps_the_short_way() {
        let start_deg = 350.0f32;
        let end_deg = 10.0f32;
        let (start, sweep) = normalized_arc_sweep(start_deg.to_radians(), end_deg.to_radians());
        assert!((start - start_deg.to_radians()).abs() < 1e-4);
        assert!((sweep - 20.0f32.to_radians()).abs() < 1e-4);
    }

    /// A whole turn normalises to `end == start`, which used to read as a zero sweep. It has to
    /// be one full turn: the command's own contract caps the sweep at 2π, not at "nothing".
    #[test]
    fn a_whole_turn_is_a_full_sweep() {
        let (start, sweep) = normalized_arc_sweep(0.0, core::f32::consts::TAU);
        assert_eq!(start, 0.0);
        assert!((sweep - core::f32::consts::TAU).abs() < 1e-3);
        // The same reading in the opposite direction: a whole turn is a circle either way.
        let (_, reverse) = normalized_arc_sweep(0.0, -core::f32::consts::TAU);
        assert!((reverse - core::f32::consts::TAU).abs() < 1e-3);
    }

    /// An ordinary forward arc keeps both of its endpoints; normalisation must not move them.
    #[test]
    fn a_forward_command_arc_keeps_its_endpoints() {
        let (start, sweep) = normalized_arc_sweep(0.5, 2.5);
        assert_eq!(start, 0.5);
        assert!((sweep - 2.0).abs() < 1e-6);
    }
}
