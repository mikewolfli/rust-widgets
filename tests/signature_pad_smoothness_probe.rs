//! Evidence for BLUE23 §96: `signature_pad`'s missing time dimension.
//!
//! # The defect this pins
//!
//! `SignatureStroke` stored a bare `Vec<Point>`, and `extend_stroke` accepted a point when it
//! was far enough from the last one **by distance alone**. That rule cannot tell "moving slowly,
//! enough points already" from "moving fast, far too few", so a stroke drawn quickly was
//! sampled once per input event and every segment between two events was long: the pad drew a
//! **polygon** where the user drew a curve. The pad recorded where the pointer *was*, never
//! where it *went*.
//!
//! # Why the assertions are on the painted picture
//!
//! The claim is "a fast stroke is drawn as smoothly as a slow one". A test on
//! `stroke.len()` would pass with the defect present (both are "some points"), and a test on
//! `is_empty` would never fail at all. So the assertions count the `<line>` elements the SVG
//! backend emits — one per drawn segment — which is the quantity the control actually
//! promises. This is the same rule the M3 probes follow, and the same rule the crate has been
//! bitten by five times for skipping.
//!
//! Run with:
//!   cargo test --no-default-features --features desktop --test signature_pad_smoothness_probe
// This probe needs the full widget registry and the theme layer; the reduced
// `mini`/`embedded` profiles compile both out, so the file is gated rather than
// rewritten to avoid APIs those profiles do not have.
#![cfg(all(full_widgets, feature = "desktop"))]

use rust_widgets::core::{Color, Point, Rect};
use rust_widgets::event::{Event, EventHandler};
use rust_widgets::widget::draw_bridge::draw_of;
use rust_widgets::widget::svg::render_widget_to_svg_on;
use rust_widgets::widget::SignaturePad;

fn render(pad: &mut SignaturePad, geometry: Rect) -> String {
    let drawable = draw_of(pad).expect("draw bridge");
    render_widget_to_svg_on(drawable, geometry, Color::BLACK)
}

/// How many stroke segments the picture contains.
///
/// The `signature_pad` chrome (its face, its frame, the empty-state baseline) is drawn with
/// `draw_rect`/`draw_line_stroke`, never `draw_line_stroke_aa`, so counting only the
/// anti-aliased `<line>` elements isolates the ink from the furniture. The elements are
/// **indented** in the document, so this searches for the tag rather than anchoring to the
/// start of the line -- anchoring is what made the first version of this probe report `0`
/// segments for a picture visibly full of them.
fn ink_segments(svg: &str) -> usize {
    svg.lines().filter(|l| l.contains("<line ") && l.contains("stroke-width=\"2\"")).count()
}

/// Drives a press → moves → release, returning the pad for rendering.
fn draw_stroke_through(pad: &mut SignaturePad, points: &[Point]) {
    pad.handle_event(&Event::MousePress { pos: points[0], button: 1, modifiers: 0 });
    for p in &points[1..] {
        pad.handle_event(&Event::MouseMove { pos: *p });
    }
    let last = points[points.len() - 1];
    pad.handle_event(&Event::MouseRelease { pos: last, button: 1 });
}

#[test]
fn probe_a_fast_stroke_is_drawn_as_smoothly_as_a_slow_one() {
    rust_widgets::theme::global_theme_manager()
        .set_appearance(rust_widgets::theme::AppearanceMode::Dark);
    let geometry = Rect::new(0, 0, 320, 160);

    // A **fast** stroke: 5 input events spread across 200 px. Each hop is 50 px, which the
    // old distance-only rule accepted verbatim — 5 points, 4 segments, one long edge per hop.
    let fast: Vec<Point> = (0..5).map(|i| Point::new(20 + i * 50, 40 + i * 10)).collect();

    let mut pad = SignaturePad::new(geometry);
    draw_stroke_through(&mut pad, &fast);
    let svg = render(&mut pad, geometry);
    let segments = ink_segments(&svg);

    println!("fast stroke: {} input events -> {} drawn segments", fast.len(), segments);
    println!("stored points: {}", pad.strokes().first().map(|s| s.len()).unwrap_or(0));

    // The stroke spans 4 hops of ~51 px each (`sqrt(50^2 + 10^2)`), so at the default 1.5 px
    // resolution a smooth rendering is on the order of `200 / 1.5` segments. The assertion is
    // **not** that exact number — it is that the picture has far more segments than there were
    // input events, which is precisely what the defect made impossible: 4 segments, one per
    // hop, is the polygon.
    assert!(
        segments > fast.len() * 4,
        "a fast stroke of {} events must be drawn as many segments, not one per event ({segments} drawn). \
         Each hop was 50 px, so interpolating at the pad's own 1.5 px resolution owes ~34 segments per hop. \
         SVG:\n{svg}",
        fast.len()
    );

    // And the geometry actually reaches the far end: the last stored point is the release.
    let stored = pad.strokes().first().expect("one committed stroke");
    assert_eq!(
        stored.points().last().copied(),
        Some(*fast.last().unwrap()),
        "interpolation must not replace the real points, only fill between them"
    );
}

/// The other half of the rule: smoothing still drops jitter, so the fix did not simply
/// "accept everything".
#[test]
fn probe_jitter_is_still_dropped() {
    rust_widgets::theme::global_theme_manager()
        .set_appearance(rust_widgets::theme::AppearanceMode::Dark);
    let geometry = Rect::new(0, 0, 320, 160);
    let mut pad = SignaturePad::new(geometry);

    // A press, then twenty moves that all land on the same pixel, all within the same
    // millisecond. Both the distance rule and the interval rule reject every one of them.
    pad.handle_event(&Event::MousePress { pos: Point::new(10, 20), button: 1, modifiers: 0 });
    for _ in 0..20 {
        pad.handle_event(&Event::MouseMove { pos: Point::new(10, 20) });
    }
    pad.handle_event(&Event::MouseRelease { pos: Point::new(10, 20), button: 1 });

    let stored = pad.strokes().first().expect("one committed stroke");
    assert_eq!(
        stored.len(),
        1,
        "twenty identical moves must collapse to the press alone; the time rule must not \
         resurrect the jitter the distance rule exists to drop"
    );
}
