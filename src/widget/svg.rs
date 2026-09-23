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

/// Reads the **top edge of the glyph box** out of a `<text>` element's emitted `y`.
///
/// # Why this conversion exists, and why it is here
///
/// The SVG backend emits a **baseline** for `y`, not a top edge: SVG's `y` is a baseline by
/// definition, and the renderer's own contract is a top edge, so the backend adds the ascent it
/// measured (`origin.y + ascent`). That is the whole conversion, and it is deliberately the
/// only place it happens.
///
/// A test that wants to assert "this label is centred in its box" has to undo it, and the undo
/// is what makes the assertion meaningful: comparing a baseline against a box's top edge is not
/// a near-miss, it is comparing two different quantities, and the difference is a whole ascent
/// (11 px at the default font). Five tests in this crate did exactly that and reported
/// failures the moment the backend stopped emitting a legacy keyword. Rather than have each of
/// them re-derive `y - ascent` — and one of them eventually getting it wrong — the arithmetic
/// lives beside the code that performs the forward conversion.
///
/// Returns `None` when the line is not a `<text>` element or carries no numeric `y`, so a
/// caller can skip documents it does not understand instead of guessing.
///
/// ```text
/// let y = text_top_of(svg_line).expect("a text element");
/// assert_eq!(y, expected_box_top);
/// ```
pub fn text_top_of(svg_line: &str) -> Option<i32> {
    if !svg_line.contains("<text") {
        return None;
    }
    let y = attribute_i32(svg_line, "y")?;
    let size = attribute_f32(svg_line, "font-size")?;
    // The same rule `SvgPaintBackend::execute_command` applies, from the same measurement:
    // `line_height = round(font.size)` and `ascent = round(line_height * 0.8)`.
    let line_height = size.max(1.0).round();
    let ascent = (line_height as f32 * 0.8).round() as i32;
    Some(y - ascent)
}

/// The value of an integer SVG attribute, e.g. `x` in `<text x="12" ...>`.
fn attribute_i32(line: &str, name: &str) -> Option<i32> {
    let key = format!("{name}=\"");
    let at = line.find(&key)? + key.len();
    let end = line[at..].find('"')? + at;
    line[at..end].parse().ok()
}

/// The value of a float SVG attribute, e.g. `font-size`.
fn attribute_f32(line: &str, name: &str) -> Option<f32> {
    let key = format!("{name}=\"");
    let at = line.find(&key)? + key.len();
    let end = line[at..].find('"')? + at;
    line[at..end].parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Font;
    use crate::compat::MiniToString;
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

    /// The emitted SVG ink box equals the rasteriser's ink box, at every font size.
    ///
    /// This is the guarantee that makes the snapshots a trustworthy picture of the control:
    /// the software rasteriser blits an 8x8 bitmap across the **whole** glyph box
    /// (`0..height` measured down from `origin.y`), and the SVG backend emits a baseline
    /// (`origin.y + ascent`). Those two describe the same box exactly when
    ///
    /// ```text
    /// baseline - ascent == origin.y            (the top edge)
    /// baseline + descent == origin.y + height  (the bottom edge)
    /// ```
    ///
    /// and both hold because `ascent + descent == height` in the crate's own `measure_text`.
    /// The test asserts it rather than proving it on paper, because the property is the whole
    /// reason the backend adds an ascent at all and it would be silent to break: a label would
    /// simply be a few pixels off in every snapshot, which no other gate can see.
    #[test]
    fn the_emitted_baseline_reproduces_the_glyph_box_exactly() {
        for size in [11.0f32, 12.0, 13.0, 14.0, 20.0, 48.0] {
            let font = Font::new("Arial", size, false, false);
            let origin = crate::core::Point::new(10, 53);
            let mut backend = crate::render::SvgPaintBackend::new(Size::new(240, 120));
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
            let line = document
                .lines()
                .find(|line| line.contains("<text"))
                .expect("a text element was emitted");

            // The crate's own metrics, which both backends share.
            let height = font.size().max(1.0).round();
            let ascent = (height * 0.8).round() as i32;
            let descent = height as i32 - ascent;

            let baseline = attribute_i32(line, "y").expect("the element carries a y");
            assert_eq!(
                baseline - ascent,
                origin.y,
                "size {size}: the glyph box's top edge must be where the rasteriser put it"
            );
            assert_eq!(
                baseline + descent,
                origin.y + height as i32,
                "size {size}: the glyph box's bottom edge must be where the rasteriser put it"
            );
            // And the reader that undoes the conversion agrees with the forward one.
            assert_eq!(text_top_of(line), Some(origin.y));
        }
    }

    /// `text_top_of` refuses a line it cannot read, rather than inventing a top edge.
    #[test]
    fn reading_a_top_edge_from_a_non_text_line_is_none() {
        assert_eq!(text_top_of("<rect x=\"0\" y=\"5\" width=\"1\" height=\"1\" />"), None);
        assert_eq!(text_top_of("<text x=\"0\" font-size=\"14\">x</text>"), None);
        assert_eq!(text_top_of("<text x=\"0\" y=\"5\">x</text>"), None);
    }
}
