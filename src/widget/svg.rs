// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! SVG rendering trait for widgets.
//!
//! # Two Approaches
//!
//! 1. **Pipeline-accurate SVG** — Use [`render_widget_to_svg()`] which routes
//!    through the actual [`Draw::draw()`] pipeline via [`SvgPaintBackend`].
//!    The SVG output is guaranteed to match the widget's real rendering.
//!
//! Most users should use [`render_widget_to_svg()`] as it is zero-maintenance
//! and guaranteed accurate.

use crate::compat::String;
use crate::core::{Rect, Size};
use crate::render::{PaintBackend, RenderContext, SvgPaintBackend};
use crate::widget::{Draw, Widget};

/// Render any widget to an SVG string using its [`Draw`] implementation.
///
/// This is the **recommended** way to generate SVG output — it routes through
/// the actual rendering pipeline via [`SvgPaintBackend`], so the SVG
/// matches the widget's real pixel output exactly.
///
/// # Usage
/// ```text
/// // render_widget_to_svg takes a `&mut impl Draw` and a geometry `Rect`:
/// let svg = render_widget_to_svg(&mut my_widget, Rect::new(0, 0, 100, 50));
/// ```
/// Convenience wrapper that auto-detects widget geometry.
/// Requires the widget to implement both [`Draw`] and [`Widget`].
pub fn render_to_svg<W: Draw + Widget>(widget: &mut W) -> String {
    let geom = widget.geometry();
    render_widget_to_svg(widget, geom)
}

/// Render any widget to an SVG string using its [`Draw`] implementation.
///
/// This is the **recommended** way to generate SVG output — it routes through
/// the actual rendering pipeline via [`SvgPaintBackend`], so the SVG
/// matches the widget's real pixel output exactly.
///
/// # Usage
/// ```text
/// // render_widget_to_svg takes a `&mut impl Draw` and a geometry `Rect`:
/// let svg = render_widget_to_svg(&mut my_widget, Rect::new(0, 0, 100, 50));
/// ```
pub fn render_widget_to_svg<T: Draw + ?Sized>(widget: &mut T, geometry: Rect) -> String {
    render_widget_to_svg_on(widget, geometry, crate::core::Color::WHITE)
}

/// Renders any widget to an SVG string, composited over `backdrop`.
///
/// # Why the backdrop is a parameter
///
/// A control is not obliged to paint a background: a `Label`, a `Separator` and several
/// others are **transparent by design**, because in a real window the surface behind them is
/// whatever their parent painted. Always filling the frame with `WHITE` therefore produced a
/// misleading picture for exactly those controls — the exporter applied the *dark* theme to a
/// label and then laid its near-white ink on white, so `<name>.svg` showed a blank rectangle
/// and `<name>.light.svg` showed the same control looking correct. The snapshot was wrong, not
/// the control.
///
/// Passing the active theme's background puts a transparent control on the surface it would
/// actually sit on, which is the picture a reviewer needs and the one P5's element bounds are
/// already measured against.
pub fn render_widget_to_svg_on<T: Draw + ?Sized>(
    widget: &mut T,
    geometry: Rect,
    backdrop: crate::core::Color,
) -> String {
    let size = Size::new(geometry.width, geometry.height);
    let mut backend = SvgPaintBackend::new(size);
    backend.begin_frame(backdrop);
    let mut ctx = RenderContext::new(&mut backend);
    widget.draw(&mut ctx);
    backend.end_frame();
    backend.finish()
}

/// The ink box of the first text run in an SVG document, as `(left, top, right, bottom)`.
///
/// This is the geometry-side entry point for a test that wants to check where text was drawn.
/// Together with [`text_subpath_count`] it replaces the `svg.contains("OK")` / `line.contains("<text")`
/// assertions the crate used before text became geometry: the string is no longer in the
/// document in any form, so a caller must either measure the ink ([`text_ink_box`]) or count it
/// ([`text_subpath_count`]).
///
/// # Why an ink box and not a single coordinate
///
/// Text leaves the renderer as **glyph geometry**, not as a `<text>` element: the SVG backend
/// emits the same `font8x8` rectangles the software rasteriser fills (see
/// `SvgPaintBackend::execute_command`, `RenderCommand::DrawText`). There is therefore no
/// `y` to read and no baseline to subtract — the document simply contains the drawing, and the
/// only way to interrogate it is geometrically.
///
/// This existed as `text_top_of`, which undid the `origin.y + ascent` baseline conversion the
/// backend used to perform for a `<text>` element. That conversion is gone: it was needed only
/// because the viewer's font engine, not this crate's, rasterised the string, so the two
/// renderers disagreed about where the ink was. Asserting against the emitted geometry is now
/// both possible and *stronger*: it pins the position **and** the extent, where a baseline
/// assertion pinned a quantity that did not exist in the output at all.
///
/// Text runs are emitted as a single `<path>` whose `d` holds one axis-aligned subpath per set
/// bitmap bit (`M{x0} {y0}h{w}v{h}h-{w}z`), so the union of those subpaths is exactly the ink.
///
/// Returns `None` for a document with no text path, or one whose `d` cannot be parsed.
///
/// ```text
/// let (left, top, _, _) = text_ink_box(&svg).expect("a text path");
/// assert_eq!(top, expected_glyph_box_top);
/// ```
pub fn text_ink_box(svg: &str) -> Option<(i32, i32, i32, i32)> {
    // Only text is a `<path>` in this backend; shapes are `<rect>`/`<circle>`/`<line>`. The
    // fill colour is the other marker, so a future non-text path does not silently satisfy a
    // text assertion.
    let mut best: Option<(i32, i32, i32, i32)> = None;
    for line in svg.lines() {
        if !line.contains("<path") {
            continue;
        }
        let Some(d) = attribute_str(line, "d") else {
            continue;
        };
        let Some(bounds) = path_bounds(d) else {
            continue;
        };
        // The first path in document order is the first text run painted; a widget paints its
        // chrome before its text, and chrome is never a path.
        best = Some(bounds);
        break;
    }
    best
}

/// The number of **subpaths** in the document's text paths.
///
/// One subpath is one set bit of one glyph bitmap, so this is a direct measure of how much ink
/// the string laid down — enough to tell `"OK"` from `""`, or to notice that a longer string
/// produced no more ink than a shorter one (which is what a lost advance looks like).
///
/// A caller that wants "this string was drawn at all" should prefer this to
/// `svg.contains("OK")`: the string is no longer in the document in any form.
pub fn text_subpath_count(svg: &str) -> usize {
    svg.lines()
        .filter(|line| line.contains("<path"))
        .filter_map(|line| attribute_str(line, "d"))
        .map(|d| d.matches('M').count())
        .sum()
}

/// The bounds of an axis-aligned `<path d>` made of `M x y h w v h h -w z` subpaths.
///
/// Unknown commands are ignored rather than misread: a `d` this cannot understand yields the
/// bounds of the parts it does, and a `d` with nothing understood yields `None`.
fn path_bounds(d: &str) -> Option<(i32, i32, i32, i32)> {
    let bytes = d.as_bytes();
    let mut i = 0usize;
    let mut bounds: Option<(i32, i32, i32, i32)> = None;
    let mut cursor: Option<(i32, i32)> = None;
    while i < bytes.len() {
        let command = bytes[i];
        i += 1;
        match command {
            b'M' => {
                let (x, y, next) = number_pair(d, i)?;
                i = next;
                cursor = Some((x, y));
                include(&mut bounds, x, y);
            }
            b'h' | b'v' => {
                let (delta, next) = number(d, i)?;
                i = next;
                let (x, y) = cursor?;
                let point = if command == b'h' { (x + delta, y) } else { (x, y + delta) };
                cursor = Some(point);
                include(&mut bounds, point.0, point.1);
            }
            b'H' | b'V' => {
                let (value, next) = number(d, i)?;
                i = next;
                let (x, y) = cursor?;
                let point = if command == b'H' { (value, y) } else { (x, value) };
                cursor = Some(point);
                include(&mut bounds, point.0, point.1);
            }
            b'z' | b'Z' | b' ' | b',' | b'\t' | b'\n' => {}
            _ => {
                // An unrecognised command: skip its numeric operand if there is one, so the
                // scan cannot desynchronise and read a coordinate as a command letter.
                if let Some((_, next)) = number(d, i) {
                    i = next;
                }
            }
        }
    }
    bounds
}

/// Widens `bounds` to cover `(x, y)`.
fn include(bounds: &mut Option<(i32, i32, i32, i32)>, x: i32, y: i32) {
    *bounds = Some(match *bounds {
        None => (x, y, x, y),
        Some((left, top, right, bottom)) => (left.min(x), top.min(y), right.max(x), bottom.max(y)),
    });
}

/// Reads a run of digits (and an optional leading `-`) starting at `at`.
fn number(text: &str, at: usize) -> Option<(i32, usize)> {
    let bytes = text.as_bytes();
    let mut i = at;
    while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b',') {
        i += 1;
    }
    let start = i;
    if i < bytes.len() && (bytes[i] == b'-' || bytes[i] == b'+') {
        i += 1;
    }
    let digits_start = i;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i == digits_start {
        return None;
    }
    text[start..i].parse().ok().map(|value| (value, i))
}

/// Reads two numbers separated by whitespace or a comma.
fn number_pair(text: &str, at: usize) -> Option<(i32, i32, usize)> {
    let (first, after_first) = number(text, at)?;
    let (second, after_second) = number(text, after_first)?;
    Some((first, second, after_second))
}

/// The value of a string SVG attribute, e.g. `d` in `<path d="..." />`.
fn attribute_str<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    let key = format!("{name}=\"");
    let at = line.find(&key)? + key.len();
    let end = line[at..].find('"')? + at;
    Some(&line[at..end])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::MiniToString;
    use crate::core::Font;
    use crate::core::Rect;
    use crate::widget::Button;

    #[test]
    fn render_widget_to_svg_produces_valid_svg() {
        let mut btn = Button::new("OK".to_string(), Rect::new(0, 0, 80, 30));
        let svg = render_widget_to_svg(&mut btn, Rect::new(0, 0, 80, 30));
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        assert!(svg.contains("width=\"80\""));
        assert!(svg.contains("height=\"30\""));
    }

    #[test]
    fn render_widget_to_svg_contains_elements() {
        let mut btn = Button::new("Click Me".to_string(), Rect::new(0, 0, 120, 40));
        let svg = render_widget_to_svg(&mut btn, Rect::new(0, 0, 120, 40));
        // Should contain at least one fill/stroke/rect element
        assert!(svg.contains("fill=") || svg.contains("stroke="));
    }

    #[test]
    fn render_to_svg_wrapper_works() {
        let mut btn = Button::new("OK".to_string(), Rect::new(0, 0, 80, 30));
        let svg = render_to_svg(&mut btn);
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("width=\"80\""));
    }

    /// The emitted SVG ink box equals the rasteriser's ink box, exactly.
    ///
    /// This is the guarantee that makes the snapshots a trustworthy picture of the control: the
    /// software rasteriser blits an 8x8 bitmap across the **whole** glyph box (`0..height`
    /// measured down from `origin.y`), and the SVG backend emits those same rectangles as
    /// `<path>` subpaths. Both read `glyph_rects`, so the test asserts the property the shared
    /// derivation exists to provide — and it would be silent to break, because a label would
    /// simply be a few pixels off in every snapshot, which no other gate can see.
    ///
    /// The old form of this test compared an emitted baseline against `origin.y + ascent`,
    /// which was a check on the *legacy* `<text>` path. There is no baseline any more, and the
    /// replacement is stronger: the top-left of the ink is asserted to be the glyph box's
    /// top-left, and the box is asserted to be `line_height` tall with `"Sample"`'s own width.
    #[test]
    fn the_emitted_path_reproduces_the_glyph_box_exactly() {
        use crate::render::estimate_cluster_advance;
        for size in [11.0f32, 12.0, 13.0, 14.0, 20.0, 48.0] {
            let font = Font::new("Arial", size, false, false);
            let origin = crate::core::Point::new(10, 53);
            let mut backend = crate::render::SvgPaintBackend::new(Size::new(400, 160));
            {
                use crate::render::RenderContext;
                let mut context = RenderContext::new(&mut backend);
                context.draw_text(
                    origin,
                    "Sample",
                    &font,
                    crate::core::Color::BLACK,
                    crate::core::HorizontalAlignment::Left,
                );
            }
            let document = backend.finish();

            let (left, top, right, bottom) =
                text_ink_box(&document).expect("a text path was emitted");

            // The glyph box's top edge is where the rasteriser put it. The bottom edge is one
            // line height down, because the bitmap is stretched across the whole box.
            let height = font.size().max(1.0).round() as i32;
            assert_eq!(top, origin.y, "size {size}: the glyph box top edge");
            assert_eq!(bottom, origin.y + height, "size {size}: the glyph box bottom edge");
            // And the ink starts at the left edge, because `Left` alignment anchors there and
            // `S`'s bitmap has its leftmost set bit in column 0.
            assert_eq!(left, origin.x, "size {size}: left-aligned ink starts at the origin");
            // The ink cannot be wider than the string's own advance. `estimate_cluster_advance`
            // charges one cluster at a time, so the run's advance is the sum over `"Sample"`'s
            // six clusters — the same sum `shape_text` performs.
            let advance: i32 = "Sample"
                .chars()
                .map(|ch| estimate_cluster_advance(&ch.to_string(), size, 1.0).round() as i32)
                .sum();
            assert!(
                right - left <= advance,
                "size {size}: ink width {} exceeds the {advance}px advance",
                right - left
            );
            assert!(right > left, "size {size}: the string drew no ink at all");
        }
    }

    /// Every rendered glyph contributes subpaths, so `text_subpath_count` separates "drew a
    /// string" from "drew nothing" — the assertion the deleted `svg.contains("OK")` form used
    /// to make, before text stopped being a `<text>` element.
    #[test]
    fn text_subpath_count_separates_a_drawn_string_from_an_empty_one() {
        let font = Font::new("Arial", 14.0, false, false);
        let paint = |text: &str| {
            let mut backend = crate::render::SvgPaintBackend::new(Size::new(200, 60));
            {
                use crate::render::RenderContext;
                let mut context = RenderContext::new(&mut backend);
                context.draw_text(
                    crate::core::Point::new(4, 4),
                    text,
                    &font,
                    crate::core::Color::BLACK,
                    crate::core::HorizontalAlignment::Left,
                );
            }
            backend.finish()
        };
        assert_eq!(text_subpath_count(&paint("")), 0);
        assert_eq!(text_subpath_count(&paint(" ")), 0);
        let one = text_subpath_count(&paint("O"));
        let two = text_subpath_count(&paint("OO"));
        assert!(one > 0, "a drawn glyph has set bits");
        // Two identical glyphs a pen apart double the ink. This is the property a lost
        // per-cluster advance would break, and it is invisible to a bounding-box assertion.
        assert_eq!(two, one * 2, "the pen advanced so the second O is a second glyph");
    }

    /// The readers refuse a document they cannot understand, rather than inventing an answer.
    #[test]
    fn reading_text_geometry_from_a_document_without_text_is_none() {
        assert_eq!(text_ink_box(r#"<rect x="0" y="5" width="1" height="1" />"#), None);
        assert_eq!(text_ink_box(r##"<path d="" fill="#000" />"##), None);
        assert_eq!(text_ink_box(r##"<path fill="#000" />"##), None);
        assert_eq!(text_subpath_count(r#"<rect x="0" y="1" width="2" height="3" />"#), 0);
        // One subpath: `M0 0h1v1h-1z` is a unit square at the top-left of the box.
        assert_eq!(text_ink_box("<path d=\"M0 0h1v1h-1z\" fill=\"#000\" />"), Some((0, 0, 1, 1)));
    }
}
