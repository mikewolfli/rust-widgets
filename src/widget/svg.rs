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
/// ```ignore
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
/// ```ignore
/// // render_widget_to_svg takes a `&mut impl Draw` and a geometry `Rect`:
/// let svg = render_widget_to_svg(&mut my_widget, Rect::new(0, 0, 100, 50));
/// ```
pub fn render_widget_to_svg<T: Draw>(widget: &mut T, geometry: Rect) -> String {
    let size = Size::new(geometry.width, geometry.height);
    let mut backend = SvgPaintBackend::new(size);
    backend.begin_frame(crate::core::Color::WHITE);
    let mut ctx = RenderContext::new(&mut backend);
    widget.draw(&mut ctx);
    backend.end_frame();
    backend.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
