// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Direction-aware column geometry for the table family (U2-06).
//!
//! Cell *text* already followed the bidi algorithm, but every column layout put column 0 at the
//! left edge in every locale. That is not a translation gap: in an Arabic table the first column
//! is the one a reader starts at, so it belongs on the right.
//!
//! The rule is one reflection about the middle of the content band, so the helpers here are the
//! only place that knows it:
//!
//! * a span at LTR `x` becomes `band.width - x - width`;
//! * a hit-test point `x` becomes `band.width - 1 - x`.
//!
//! Widths do not change, so scrolling, frozen prefixes and per-column widths keep working; they
//! are mirrored together with the columns they describe.

use crate::core::{Rect, TextDirection};

/// Whether the process/thread default direction is right-to-left.
///
/// Read through [`crate::core::default_text_direction`] rather than a per-control field: the i18n
/// language switch is the producer, and it writes that value. A table in an RTL page therefore
/// mirrors without every host having to call a setter.
pub(crate) fn is_rtl() -> bool {
    crate::core::default_text_direction() == TextDirection::RightToLeft
}

/// Mirrors a span's left edge inside `band`.
pub(crate) fn mirror_span(x: i32, width: u32, band: Rect) -> i32 {
    band.x + band.width as i32 - (x - band.x) - width as i32
}

/// Mirrors a point's x inside `band` (half-open pixel coordinates).
pub(crate) fn mirror_point_x(x: i32, band: Rect) -> i32 {
    band.x + band.width as i32 - 1 - (x - band.x)
}

/// Mirrors a width-relative offset inside a band of `band_width` pixels.
pub(crate) fn mirror_offset(x: i32, band_width: i32) -> i32 {
    band_width - 1 - x
}

/// The alignment a leading-edge column label uses in the current direction.
pub(crate) fn leading_alignment() -> crate::core::HorizontalAlignment {
    if is_rtl() {
        crate::core::HorizontalAlignment::Right
    } else {
        crate::core::HorizontalAlignment::Left
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Runs `f` with the default direction set to RTL, restoring it afterwards.
    ///
    /// The direction is thread-local and every test runs on its own thread, so this also keeps
    /// the table tests independent of each other.
    pub(crate) fn with_rtl<R>(f: impl FnOnce() -> R) -> R {
        let previous = crate::core::default_text_direction();
        crate::core::set_default_text_direction(TextDirection::RightToLeft);
        let result = f();
        crate::core::set_default_text_direction(previous);
        result
    }

    #[test]
    fn mirroring_twice_returns_the_original_span() {
        let band = Rect::new(10, 0, 100, 20);
        let x = mirror_span(10, 30, band);
        assert_eq!(x, 80);
        assert_eq!(mirror_span(x, 30, band), 10);
    }

    #[test]
    fn mirroring_a_point_maps_the_edges_to_each_other() {
        let band = Rect::new(0, 0, 100, 20);
        assert_eq!(mirror_point_x(0, band), 99);
        assert_eq!(mirror_point_x(99, band), 0);
    }
}
