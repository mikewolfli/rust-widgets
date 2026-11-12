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
//! Only geometry and its colour data. The caller owns the paint commands, so a window manager
//! owns the real title bar on a decorated window; these helpers exist for the borderless case,
//! where the application owns those pixels and has to draw them itself. The layout is derived
//! once and shared by the paint path and the close-button hit test, so a button cannot be
//! painted where it cannot be clicked.

use crate::core::{Color, Rect};

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
    /// boxes inside itself rather than negative rectangles. A button whose natural position would
    /// cross the trailing edge is moved to the last position it still fits in; a button that has
    /// no room left (it would overlap the one before it, or the window is narrower than one
    /// button) becomes a zero-sized box, which the paint path and hit test both skip.
    pub fn compute(window: Rect, title_bar_height: u32, button_size: u32, spacing: u32) -> Self {
        let height = title_bar_height.min(window.height);
        let title_bar = Rect::new(window.x, window.y, window.width, height);
        let size = button_size.clamp(6, 20).min(height.max(1));
        let gap = spacing.max(size + 4);
        let top = window.y + (height as i32 - size as i32) / 2;
        // The x a button may occupy while still ending inside the window. It may sit left of
        // `window.x` when the window is narrower than one button; the fit check below then skips
        // every button rather than producing a negative/overflowing rectangle.
        let right = window.x + window.width as i32 - size as i32;
        let mut previous_x: Option<i32> = None;
        let mut button = |index: u32| {
            let natural = window.x + 8 + index as i32 * gap as i32;
            let x = natural.min(right);
            // Fit means inside the window and not on top of (or before) the previous button.
            let fits = x >= window.x && previous_x.is_none_or(|previous| x > previous);
            if fits {
                previous_x = Some(x);
                Rect::new(x, top, size, size)
            } else {
                Rect::new(x.max(window.x), top, 0, 0)
            }
        };
        let close = button(0);
        let minimize = button(1);
        let maximize = button(2);
        let client = Rect::new(
            window.x,
            window.y + height as i32,
            window.width,
            window.height.saturating_sub(height),
        );
        Self { title_bar, close, minimize, maximize, client }
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Point;

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

    /// Every traffic light is inside the window, and a light with no room is not painted.
    ///
    /// # The defect this pins
    ///
    /// Only the diameter was clamped; each button's x was `window.x + 8 + index * gap` with no
    /// clamp. A 10 px-wide window therefore put minimize/maximize at x = 48/88, painted outside
    /// the window. The first button moves to the last position where it still fits, and the two
    /// that would overlap it become zero-sized boxes that the paint path and hit test skip.
    #[test]
    fn a_tiny_window_keeps_every_button_inside_and_skips_the_ones_with_no_room() {
        let window = Rect::new(0, 0, 10, 4);
        let geometry = WindowChromeGeometry::compute(window, 32, 14, 40);
        assert_eq!(geometry.close.x, 6, "the leading button moves to the trailing edge it fits in");
        assert_eq!(geometry.close.width, 4);
        for button in [geometry.close, geometry.minimize, geometry.maximize] {
            assert!(
                button.x >= window.x
                    && button.x + button.width as i32 <= window.x + window.width as i32,
                "{button:?} must stay inside {window:?}"
            );
        }
        assert_eq!(geometry.minimize.width, 0, "no room left, so nothing is painted");
        assert_eq!(geometry.maximize.width, 0);
    }
}
