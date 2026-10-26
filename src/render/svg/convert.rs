// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! SVG string conversion helpers for `SvgPaintBackend`.
//!
//! Provides color formatting and XML escaping utilities
//! shared across SVG element generation.

use crate::compat::{format, String, ToString};
use crate::core::{Color, Point, Rect};

/// Format a [`Color`] as an SVG-compatible `rgba()` CSS string.
///
/// ```text
/// // color_to_rgba is internal to the svg module, not exported publicly.
/// // See tests in src/render/svg/convert.rs for usage.
/// ```
pub fn color_to_rgba(c: &Color) -> String {
    format!("rgba({},{},{},{:.2})", c.r, c.g, c.b, c.a as f32 / 255.0)
}

/// Format a [`Rect`] as SVG `x=".." y=".." width=".." height=".."` attributes.
pub fn rect_attrs(rect: &Rect) -> String {
    format!(r#"x="{}" y="{}" width="{}" height="{}""#, rect.x, rect.y, rect.width, rect.height)
}

/// Format an **outline** rectangle, inset by half the stroke width so the whole rule lies inside
/// `rect`.
///
/// # Why the outline is not the same rectangle as the fill
///
/// SVG centres a stroke on the path it follows, so a `<rect x="0" y="0" width="240" …
/// stroke-width="1">` paints half of that rule at `x = -0.5` — outside the picture. A viewer
/// clips it, and what is left is a half-width edge whose corner arc is the first thing to
/// disappear; the widget then reads as having *open* corners even though the fill is perfectly
/// round.
///
/// The software and wgpu backends both paint an outline **inside** the rectangle: their band is
/// `rect` minus `rect` inset by the stroke width. Moving the SVG path in by `width / 2` makes the
/// band it covers identical to theirs — the same `[x, x + width)` on every edge, and the same
/// corner ring (see [`outline_radius`]). A snapshot therefore shows what the screen shows, which
/// is the only reason the SVG backend exists.
///
/// A rectangle too small to hold the stroke keeps the plain attributes rather than collapsing to
/// a degenerate element.
pub fn outline_rect_attrs(rect: &Rect, width: u32) -> String {
    if !can_inset(rect, width) {
        return rect_attrs(rect);
    }
    let half = width as f64 / 2.0;
    format!(
        r#"x="{}" y="{}" width="{}" height="{}""#,
        number_attr(rect.x as f64 + half),
        number_attr(rect.y as f64 + half),
        number_attr(rect.width as f64 - width as f64),
        number_attr(rect.height as f64 - width as f64),
    )
}

/// The radius an **outline** of `width` follows, so its ring matches the raster backends.
///
/// The raster band is `rounded(rect, r) minus rounded(rect.inset(width), r - width)`; both arcs
/// share the centre `(x + r, y + r)`, which is exactly an SVG stroke of `width` on a path whose
/// radius is `r - width / 2`. Returns `radius` unchanged when the outline could not be inset.
pub fn outline_radius(rect: &Rect, radius: u32, width: u32) -> f64 {
    if !can_inset(rect, width) {
        return radius as f64;
    }
    (radius as f64 - width as f64 / 2.0).max(0.0)
}

/// The per-corner form of [`outline_radius`] (STY-01).
///
/// Same arithmetic per corner, so a non-uniform outline emits the same ring the
/// raster backends draw: each corner's radius inset by half the stroke width on
/// the inset rectangle, or the resolved radius unchanged when the outline could
/// not be inset.
pub fn outline_radii_corners(rect: &Rect, radii: [u32; 4], width: u32) -> [f64; 4] {
    if !can_inset(rect, width) {
        return [radii[0] as f64, radii[1] as f64, radii[2] as f64, radii[3] as f64];
    }
    let half = width as f64 / 2.0;
    radii.map(|r| (r as f64 - half).max(0.0))
}

/// The inset rectangle [`outline_rect_attrs`] positions, for paths that need the
/// same geometry as numbers rather than as attributes.
pub fn outline_rect(rect: &Rect, width: u32) -> Rect {
    if !can_inset(rect, width) {
        return *rect;
    }
    let half = width as f64 / 2.0;
    Rect {
        x: rect.x + half as i32,
        y: rect.y + half as i32,
        width: rect.width.saturating_sub(width),
        height: rect.height.saturating_sub(width),
    }
}

/// Formats a rounded rectangle as an SVG `d` attribute (STY-01).
///
/// `radii` are the four corner radii **already resolved for `rect`** (the CSS
/// proportional fit), in `[top_left, top_right, bottom_right, bottom_left]`
/// order. A zero-radius corner emits a line segment rather than a zero-radius
/// arc, which the SVG spec would treat as a line anyway. This is the per-corner
/// counterpart of the `<rect rx ry>` form the uniform emission uses; the
/// uniform case never routes here, so every SVG string a uniform radius ever
/// produced is unchanged.
pub fn rounded_rect_path(rect: &Rect, radii: [f64; 4]) -> String {
    let (tl, tr, br, bl) = (radii[0], radii[1], radii[2], radii[3]);
    let x = rect.x as f64;
    let y = rect.y as f64;
    let right = x + rect.width as f64;
    let bottom = y + rect.height as f64;
    let corner = |radius: f64, vx: f64, vy: f64| {
        if radius <= 0.0 {
            format!("L {},{} ", number_attr(vx), number_attr(vy))
        } else {
            format!(
                "A {},{} 0 0 1 {},{} ",
                number_attr(radius),
                number_attr(radius),
                number_attr(vx),
                number_attr(vy)
            )
        }
    };
    let mut d = format!("M {},{} ", number_attr(x + tl), number_attr(y));
    d.push_str(&format!("H {} ", number_attr(right - tr)));
    d.push_str(&corner(tr, right, y + tr));
    d.push_str(&format!("V {} ", number_attr(bottom - br)));
    d.push_str(&corner(br, right - br, bottom));
    d.push_str(&format!("H {} ", number_attr(x + bl)));
    d.push_str(&corner(bl, x, bottom - bl));
    d.push_str(&format!("V {} ", number_attr(y + tl)));
    d.push_str(&corner(tl, x + tl, y));
    d.push('Z');
    d
}

/// Whether a stroke of `width` fits inside `rect` with room to spare on both axes.
fn can_inset(rect: &Rect, width: u32) -> bool {
    width > 0 && rect.width > width * 2 && rect.height > width * 2
}

/// Formats a coordinate without a trailing `.0` (or `.50`), so an integral edge keeps the byte
/// form it had before this helper existed.
pub fn number_attr(value: f64) -> String {
    let mut text = format!("{value:.2}");
    while text.contains('.') && text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    text
}

/// Format the start point of a line as SVG `x1=".." y1=".."` attributes.
pub fn point_attrs(p: &Point) -> String {
    format!(r#"x1="{}" y1="{}""#, p.x, p.y)
}

/// Format four perimeter-ordered corners as a `points="x y x y …"` list.
///
/// Used by the quad commands an affine transform produces, which an SVG `<rect>` cannot
/// express. The corners are already integers, so no rounding happens here.
#[cfg(feature = "3d")]
pub fn quad_points(corners: &[Point; 4]) -> String {
    format!(
        "{} {} {} {} {} {} {} {}",
        corners[0].x,
        corners[0].y,
        corners[1].x,
        corners[1].y,
        corners[2].x,
        corners[2].y,
        corners[3].x,
        corners[3].y
    )
}

/// Escape special XML characters (`<`, `>`, `&`, `"`, `'`) for safe SVG output.
pub fn escape_xml(s: &str) -> String {
    s.chars()
        .map(|ch| match ch {
            '<' => "&lt;".to_string(),
            '>' => "&gt;".to_string(),
            '&' => "&amp;".to_string(),
            '"' => "&quot;".to_string(),
            '\'' => "&apos;".to_string(),
            _ => ch.to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_to_rgba_red() {
        assert_eq!(color_to_rgba(&Color::RED), "rgba(255,0,0,1.00)");
    }

    #[test]
    fn color_to_rgba_transparent() {
        let c = Color::rgba(100, 150, 200, 0);
        assert_eq!(color_to_rgba(&c), "rgba(100,150,200,0.00)");
    }

    #[test]
    fn color_to_rgba_semi_transparent() {
        let c = Color::rgba(50, 100, 150, 128);
        assert_eq!(color_to_rgba(&c), "rgba(50,100,150,0.50)");
    }

    #[test]
    fn rect_attrs_formats_correctly() {
        let r = Rect::new(10, 20, 100, 50);
        assert_eq!(rect_attrs(&r), r#"x="10" y="20" width="100" height="50""#);
    }

    /// An outline is inset by half its width, so the whole rule lies inside the rectangle. The
    /// band it covers is then `[x, x + width)` on every edge — the same band the software and
    /// wgpu backends paint, and the reason a 4 px corner no longer loses its arc to the canvas
    /// edge.
    #[test]
    fn outline_rect_attrs_insets_the_stroke() {
        let r = Rect::new(0, 0, 240, 120);
        assert_eq!(outline_rect_attrs(&r, 1), r#"x="0.5" y="0.5" width="239" height="119""#);
        assert_eq!(outline_rect_attrs(&r, 2), r#"x="1" y="1" width="238" height="118""#);
    }

    /// A rectangle too small to hold the stroke keeps the plain geometry: shrinking it would
    /// emit a degenerate element that paints nothing.
    #[test]
    fn outline_rect_attrs_leaves_a_tiny_rect_alone() {
        let r = Rect::new(5, 5, 2, 2);
        assert_eq!(outline_rect_attrs(&r, 1), rect_attrs(&r));
    }

    /// The radius follows the same half-width shift: an SVG stroke of `w` on radius `r - w/2`
    /// shares its corner centre `(x + r, y + r)` with the raster's `r` / `r - w` rings.
    #[test]
    fn outline_radius_shares_the_corner_centre() {
        let r = Rect::new(0, 0, 240, 120);
        assert_eq!(outline_radius(&r, 4, 1), 3.5);
        assert_eq!(outline_radius(&r, 4, 2), 3.0);
        assert_eq!(outline_radius(&Rect::new(0, 0, 2, 2), 4, 1), 4.0);
    }

    /// Coordinates print without a trailing `.0`, so integral edges keep the bytes they had before
    /// the inset helper existed.
    #[test]
    fn number_attr_trims_trailing_zeroes() {
        assert_eq!(number_attr(239.0), "239");
        assert_eq!(number_attr(0.5), "0.5");
        assert_eq!(number_attr(3.5), "3.5");
    }

    #[test]
    fn point_attrs_formats_correctly() {
        let p = Point::new(15, 25);
        assert_eq!(point_attrs(&p), r#"x1="15" y1="25""#);
    }

    #[test]
    fn escape_xml_handles_all_special_chars() {
        assert_eq!(
            escape_xml("<hello> & \"world\" 'test'"),
            "&lt;hello&gt; &amp; &quot;world&quot; &apos;test&apos;"
        );
    }

    #[test]
    fn escape_xml_plain_text_unchanged() {
        assert_eq!(escape_xml("Hello, World!"), "Hello, World!");
    }

    #[test]
    fn escape_xml_empty_string() {
        assert_eq!(escape_xml(""), "");
    }
}
