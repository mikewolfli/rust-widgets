//! SVG string conversion helpers for [`SvgPaintBackend`].
//!
//! Provides color formatting and XML escaping utilities
//! shared across SVG element generation.

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

/// Format the start point of a line as SVG `x1=".." y1=".."` attributes.
pub fn point_attrs(p: &Point) -> String {
    format!(r#"x1="{}" y1="{}""#, p.x, p.y)
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
