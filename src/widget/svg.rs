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

#[cfg(test)]
mod tests {
    use super::*;
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
}
