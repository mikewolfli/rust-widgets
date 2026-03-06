use crate::core::{Color, Rect, Size};
use std::fmt::Debug;

/// Color matcher
pub trait ColorMatcher {
    fn is_close_to(&self, other: Color, tolerance: u8) -> bool;
    fn is_opaque(&self) -> bool;
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
pub trait RectMatcher {
    fn contains_point(&self, x: i32, y: i32) -> bool;
    fn contains_rect(&self, other: &Rect) -> bool;
    fn intersects(&self, other: &Rect) -> bool;
    fn has_size(&self, size: Size) -> bool;
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
    fn is_close_to(&self, other: f32, tolerance: f32) -> bool;
}

impl FloatMatcher for f32 {
    fn is_close_to(&self, other: f32, tolerance: f32) -> bool {
        (self - other).abs() <= tolerance
    }
}

/// Generic assertion helpers
pub fn assert_close(a: f32, b: f32, tolerance: f32, message: &str) {
    assert!(
        a.is_close_to(b, tolerance),
        "{}: expected {} to be close to {} (tolerance {})",
        message,
        a,
        b,
        tolerance
    );
}

pub fn assert_color_eq(a: Color, b: Color, tolerance: u8, message: &str) {
    assert!(
        a.is_close_to(b, tolerance),
        "{}: expected {:?} to be close to {:?} (tolerance {})",
        message,
        a,
        b,
        tolerance
    );
}

pub fn assert_rect_contains(container: Rect, contained: Rect, message: &str) {
    assert!(
        container.contains_rect(&contained),
        "{}: {:?} should contain {:?}",
        message,
        container,
        contained
    );
}

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
pub trait SizeMatcher {
    fn area(&self) -> u32;
    fn is_empty(&self) -> bool;
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

        assert!(rect.contains_point(50, 50));
        assert!(!rect.contains_point(150, 50));

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

        let rects = vec![
            Rect::new(0, 0, 50, 50),
            Rect::new(50, 0, 50, 50),
            Rect::new(100, 0, 50, 50),
        ];
        assert_no_overlap(&rects, "Rects should not overlap");
    }
}
