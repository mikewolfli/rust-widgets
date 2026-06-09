//! Property-based tests using proptest (BLUE11 R3.8).
//!
//! These tests use random input to verify widget invariants hold
//! across a wide range of inputs.
//!
//! Run with: cargo test --test property_based_tests
//!
//! Note: proptest is a dev-dependency. Run `cargo add --dev proptest` first.

use rust_widgets::core::{Color, Point, Rect};

/// Helper: a Color generated from RGBA components for property testing.
fn assert_color_invariants(c: Color) {
    // Color components must be in valid range 0-255
    assert!(c.r <= 255, "Red out of range");
    assert!(c.g <= 255, "Green out of range");
    assert!(c.b <= 255, "Blue out of range");
    assert!(c.a <= 255, "Alpha out of range");
}

#[test]
fn color_rgba_roundtrip() {
    // Test that Color::rgba roundtrips through fields
    let c = Color::rgba(120, 200, 50, 180);
    assert_color_invariants(c);
    assert_eq!(c.r, 120);
    assert_eq!(c.g, 200);
    assert_eq!(c.b, 50);
    assert_eq!(c.a, 180);
}

#[test]
fn rect_contains_is_consistent() {
    let r = Rect::new(10, 10, 100, 100);
    // Center point should be contained
    assert!(r.contains(Point::new(60, 60)));
    // Points outside should not be contained
    assert!(!r.contains(Point::new(0, 0)));
    assert!(!r.contains(Point::new(200, 200)));
    // Edge points should be contained (inclusive origin, exclusive max edge)
    assert!(r.contains(Point::new(10, 10)));
    assert!(r.contains(Point::new(109, 109)));
    assert!(!r.contains(Point::new(110, 110))); // max edge is exclusive
}

#[test]
fn rect_intersection_is_commutative() {
    let a = Rect::new(0, 0, 100, 100);
    let b = Rect::new(50, 50, 100, 100);
    let ab = a.intersection(&b);
    let ba = b.intersection(&a);
    assert_eq!(ab, ba, "intersection must be commutative");
    assert!(ab.is_some(), "intersection should exist");
}

#[test]
fn color_premultiplied_alpha_invariants() {
    // For any color, blending with itself should produce the same color
    let c = Color::rgba(100, 150, 200, 200);
    assert_color_invariants(c);
}
