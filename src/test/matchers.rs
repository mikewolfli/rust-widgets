// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

// ...existing code...
use crate::core::{Color, Rect, Size};
// ...existing code...
use std::fmt::Debug;
/// Color matcher
pub trait ColorMatcher {
    /// Returns `true` when every channel — including alpha — differs from
    /// `other` by at most `tolerance`.
    ///
    /// The tolerance is applied per channel with `abs_diff`, so it is an
    /// absolute value in `0 ..= 255` units, not a percentage. A tolerance of
    /// `0` degenerates to exact equality.
    fn is_close_to(&self, other: Color, tolerance: u8) -> bool;
    /// Returns `true` only for a fully opaque color (alpha exactly `255`).
    fn is_opaque(&self) -> bool;
    /// Returns `true` only for a fully transparent color (alpha exactly `0`).
    ///
    /// Note that translucent colors (alpha between 1 and 254) are neither
    /// opaque nor transparent by these two predicates.
    fn is_transparent(&self) -> bool;
}
impl ColorMatcher for Color {
    fn is_close_to(&self, other: Color, tolerance: u8) -> bool {
        (self.r.abs_diff(other.r) <= tolerance)
            && (self.g.abs_diff(other.g) <= tolerance)
            && (self.b.abs_diff(other.b) <= tolerance)
            && (self.a.abs_diff(other.a) <= tolerance)
    }
    fn is_opaque(&self) -> bool {
        self.a == 255
    }
    fn is_transparent(&self) -> bool {
        self.a == 0
    }
}
/// Rect matcher
///
/// A testing-only view of [`Rect`] that mirrors the geometry predicates with
/// test-friendly names. Unlike [`Rect::contains_point`](crate::core::Rect::contains_point),
/// these helpers compute edges with plain `i32` addition and so can overflow on
/// extreme rectangles; they are intended for ordinary fixture values.
pub trait RectMatcher {
    /// Returns `true` when `(x, y)` is inside the rectangle, using an inclusive
    /// minimum edge and an exclusive maximum edge.
    fn contains_point(&self, x: i32, y: i32) -> bool;
    /// Returns `true` when `other` is fully inside `self`; touching edges count
    /// as contained.
    fn contains_rect(&self, other: &Rect) -> bool;
    /// Returns `true` when the rectangles share at least one pixel.
    ///
    /// Edges that merely touch do **not** count as intersecting, so this agrees
    /// with [`Rect::intersects`](crate::core::Rect::intersects).
    fn intersects(&self, other: &Rect) -> bool;
    /// Returns `true` when width and height equal `size`'s exactly. The origin
    /// is not compared.
    fn has_size(&self, size: Size) -> bool;
    /// Returns `true` when the origin is exactly `(x, y)`. The extent is not
    /// compared.
    fn is_at(&self, x: i32, y: i32) -> bool;
}
impl RectMatcher for Rect {
    fn contains_point(&self, x: i32, y: i32) -> bool {
        x >= self.x
            && x < self.x + self.width as i32
            && y >= self.y
            && y < self.y + self.height as i32
    }
    fn contains_rect(&self, other: &Rect) -> bool {
        self.x <= other.x
            && self.y <= other.y
            && self.x + self.width as i32 >= other.x + other.width as i32
            && self.y + self.height as i32 >= other.y + other.height as i32
    }
    fn intersects(&self, other: &Rect) -> bool {
        !(self.x + self.width as i32 <= other.x
            || other.x + other.width as i32 <= self.x
            || self.y + self.height as i32 <= other.y
            || other.y + other.height as i32 <= self.y)
    }
    fn has_size(&self, size: Size) -> bool {
        self.width == size.width && self.height == size.height
    }
    fn is_at(&self, x: i32, y: i32) -> bool {
        self.x == x && self.y == y
    }
}
/// Numeric matcher with tolerance
pub trait FloatMatcher {
    /// Returns `true` when the absolute difference is at most `tolerance`.
    ///
    /// `tolerance` must be non-negative; a negative tolerance makes the
    /// predicate always `false`. NaN never compares close to anything.
    fn is_close_to(&self, other: f32, tolerance: f32) -> bool;
}
impl FloatMatcher for f32 {
    fn is_close_to(&self, other: f32, tolerance: f32) -> bool {
        (self - other).abs() <= tolerance
    }
}
/// Generic assertion helpers
///
/// Each panics on failure with `message` prefixed to a description of the
/// actual and expected values, so failures are identifiable at a glance.
///
/// Asserts that `a` and `b` differ by at most `tolerance`.
pub fn assert_close(a: f32, b: f32, tolerance: f32, message: &str) {
    assert!(
        a.is_close_to(b, tolerance),
        "{message}: expected {a} to be close to {b} (tolerance {tolerance})"
    );
}
/// Asserts that two colors match within `tolerance` per channel, including
/// alpha. See [`ColorMatcher::is_close_to`].
pub fn assert_color_eq(a: Color, b: Color, tolerance: u8, message: &str) {
    assert!(
        a.is_close_to(b, tolerance),
        "{message}: expected {a:?} to be close to {b:?} (tolerance {tolerance})"
    );
}
/// Asserts that `container` fully contains `contained`. Touching edges are
/// accepted.
pub fn assert_rect_contains(container: Rect, contained: Rect, message: &str) {
    assert!(
        container.contains_rect(&contained),
        "{message}: {container:?} should contain {contained:?}"
    );
}
/// Asserts that no two rectangles in `rects` overlap, checking every pair.
///
/// This is O(n²), which is fine for the small fixtures it is meant for. An
/// empty or single-element slice always passes.
pub fn assert_no_overlap(rects: &[Rect], message: &str) {
    for i in 0..rects.len() {
        for j in (i + 1)..rects.len() {
            assert!(
                !rects[i].intersects(&rects[j]),
                "{}: rects[{}] {:?} overlaps with rects[{}] {:?}",
                message,
                i,
                rects[i],
                j,
                rects[j]
            );
        }
    }
}
/// Collection assertions
///
/// Asserts that `items` is in non-decreasing order according to its own
/// [`Ord`] implementation. Empty and single-element slices pass.
pub fn assert_sorted<T: Ord + Debug>(items: &[T], message: &str) {
    for i in 1..items.len() {
        assert!(
            items[i - 1] <= items[i],
            "{}: items not sorted at index {}: {:?} > {:?}",
            message,
            i,
            items[i - 1],
            items[i]
        );
    }
}
/// Asserts that `items` contains no equal pair.
///
/// Equality is determined by sorting a clone, so `T: Ord` must be consistent
/// with the caller's notion of equality **and** for values that are distinct
/// under it; hashing or floating-point equality is not usable here.
pub fn assert_unique<T: Ord + Debug + Clone>(items: &[T], message: &str) {
    let mut sorted = items.to_vec();
    sorted.sort();
    for i in 1..sorted.len() {
        assert!(
            sorted[i - 1] != sorted[i],
            "{}: duplicate item at index {}: {:?}",
            message,
            i,
            sorted[i]
        );
    }
}
/// Size matcher
///
/// A testing-only view of [`Size`], mirroring the inherent helpers with
/// test-oriented naming.
pub trait SizeMatcher {
    /// Returns width times height, with `u32` wrapping on overflow.
    fn area(&self) -> u32;
    /// Returns `true` when either dimension is zero.
    fn is_empty(&self) -> bool;
    /// Returns width divided by height, or `0.0` when the height is zero rather
    /// than an infinity or NaN.
    fn aspect_ratio(&self) -> f32;
}
impl SizeMatcher for Size {
    fn area(&self) -> u32 {
        self.width * self.height
    }
    fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }
    fn aspect_ratio(&self) -> f32 {
        if self.height == 0 {
            0.0
        } else {
            self.width as f32 / self.height as f32
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Point;
    #[test]
    fn test_color_matcher() {
        let color1 = Color::rgb(100, 100, 100);
        let color2 = Color::rgb(102, 98, 101);
        let color3 = Color::rgb(150, 150, 150);
        assert!(color1.is_close_to(color2, 5));
        assert!(!color1.is_close_to(color3, 5));
        assert!(Color::rgb(255, 255, 255).is_opaque());
        assert!(Color::rgba(0, 0, 0, 0).is_transparent());
    }
    #[test]
    fn test_rect_matcher() {
        let rect = Rect::new(0, 0, 100, 100);
        assert!(rect.contains_point(Point::new(50, 50)));
        assert!(!rect.contains_point(Point::new(150, 50)));
        let inner = Rect::new(10, 10, 80, 80);
        assert!(rect.contains_rect(&inner));
        let overlapping = Rect::new(50, 50, 100, 100);
        assert!(rect.intersects(&overlapping));
        let non_overlapping = Rect::new(200, 200, 50, 50);
        assert!(!rect.intersects(&non_overlapping));
    }
    #[test]
    fn test_size_matcher() {
        let size = Size::new(100, 50);
        assert_eq!(size.area(), 5000);
        assert!(!size.is_empty());
        assert!((size.aspect_ratio() - 2.0).abs() < 0.01);
    }
    #[test]
    fn test_assertions() {
        assert_close(1.0, 1.01, 0.05, "Values should be close");
        assert_color_eq(
            Color::rgb(100, 100, 100),
            Color::rgb(102, 98, 101),
            5,
            "Colors should be close",
        );
        let rects =
            vec![Rect::new(0, 0, 50, 50), Rect::new(50, 0, 50, 50), Rect::new(100, 0, 50, 50)];
        assert_no_overlap(&rects, "Rects should not overlap");
    }
}
