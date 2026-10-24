// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Painted chrome for a self-drawn (borderless) window.
//!
//! # Why this is separate from `field_decorations`
//!
//! `field_decorations` owns the five extra strings a *text field* shows — prefix, suffix,
//! helper, error and counter. It was named `decorations`, which read as window decoration and
//! sent more than one reader looking for traffic lights in a text-field layout. Window chrome
//! lives here instead, under a name that says which kind of decoration it is.
//!
//! # What is offered
//!
//! Only geometry and paint primitives. A window manager owns the real title bar on a decorated
//! window; these helpers exist for the borderless case, where the application owns those pixels
//! and has to draw them itself. The layout is derived once and shared by the paint path and the
//! close-button hit test, so a button cannot be painted where it cannot be clicked.

use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::render::RenderContext;

/// The rectangles of a self-drawn title bar and its three window buttons.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowChromeGeometry {
    /// The title strip, across the top of the window.
    pub title_bar: Rect,
    /// The close button (traffic light), at the leading edge.
    pub close: Rect,
    /// The minimise button.
    pub minimize: Rect,
    /// The maximise button.
    pub maximize: Rect,
    /// The remaining client area below the title strip.
    pub client: Rect,
}

impl WindowChromeGeometry {
    /// Derives the chrome boxes for a window of `window` at the given metrics.
    ///
    /// `button_size` is the diameter of one traffic light, `spacing` the distance from one
    /// button's leading edge to the next, and `title_bar_height` the strip's own height. All
    /// three are clamped to the window's extent, so a window smaller than its chrome still gets
    /// boxes inside itself rather than negative rectangles.
    pub fn compute(window: Rect, title_bar_height: u32, button_size: u32, spacing: u32) -> Self {
        let height = title_bar_height.min(window.height);
        let title_bar = Rect::new(window.x, window.y, window.width, height);
        let size = button_size.clamp(6, 20).min(height.max(1));
        let gap = spacing.max(size + 4);
        let top = window.y + (height as i32 - size as i32) / 2;
        let button = |index: u32| {
            let x = window.x + 8 + index as i32 * gap as i32;
            Rect::new(x, top, size, size)
        };
        let client = Rect::new(
            window.x,
            window.y + height as i32,
            window.width,
            window.height.saturating_sub(height),
        );
        Self { title_bar, close: button(0), minimize: button(1), maximize: button(2), client }
    }

    /// The colour of one of the three buttons, in the platform's order.
    ///
    /// The values are semantic, not chrome: red/yellow/green encode close/minimise/maximise on
    /// every window system that uses traffic lights, so they are kept as data rather than
    /// derived from a theme that might recolour them into meaninglessness.
    pub fn button_color(index: usize) -> Color {
        match index {
            0 => Color::rgba(255, 95, 86, 255),
            1 => Color::rgba(255, 189, 46, 255),
            _ => Color::rgba(39, 201, 63, 255),
        }
    }

    /// Paints the title strip, its three buttons and the window title.
    ///
    /// `ink` is the title's colour and `surface` the strip's fill; both come from the caller so
    /// this helper does not take a second opinion on the window's appearance.
    pub fn paint(&self, context: &mut RenderContext, surface: Color, ink: Color, title: &str) {
        context.fill_rect(self.title_bar, surface);
        for index in 0..3 {
            let button = match index {
                0 => self.close,
                1 => self.minimize,
                _ => self.maximize,
            };
            let radius = (button.width / 2).max(1);
            let center = Point::new(button.x + radius as i32, button.y + radius as i32);
            context.fill_circle_aa(center, radius, Self::button_color(index));
        }
        if !title.is_empty() {
            let font =
                crate::widget::metrics::role_font(&crate::style::WidgetStyle::default(), 13.0);
            context.draw_text_fitted_cow(
                context.text_line(self.title_bar, &font),
                title,
                &font,
                ink,
                HorizontalAlignment::Center,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_three_buttons_sit_inside_the_title_bar() {
        let geometry = WindowChromeGeometry::compute(Rect::new(0, 0, 400, 300), 32, 14, 40);
        assert_eq!(geometry.title_bar, Rect::new(0, 0, 400, 32));
        assert_eq!(geometry.client, Rect::new(0, 32, 400, 268));
        for button in [geometry.close, geometry.minimize, geometry.maximize] {
            assert!(geometry.title_bar.contains_point(Point::new(button.x, button.y)));
            assert!(button.x + button.width as i32 <= geometry.title_bar.x + 400);
        }
        assert!(geometry.close.x < geometry.minimize.x);
    }

    #[test]
    fn a_tiny_window_clamps_its_chrome_rather_than_going_negative() {
        let geometry = WindowChromeGeometry::compute(Rect::new(0, 0, 10, 4), 32, 14, 40);
        assert_eq!(geometry.title_bar.height, 4);
        assert_eq!(geometry.client.height, 0);
        assert!(geometry.close.width <= 4);
    }
}
