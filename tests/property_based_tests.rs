//! Property-based tests using proptest (BLUE11 R3.8).
//!
//! These tests use random input to verify widget invariants hold
//! across a wide range of inputs.
//!
//! Run with: cargo test --test property_based_tests

// proptest (and its rand/getrandom chain) cannot compile for wasm32.
#![cfg(not(target_arch = "wasm32"))]

use proptest::prelude::*;
use rust_widgets::core::{Color, Point, Rect};

// ── Arbitrary implementations for core types ──

proptest! {
    /// Color RGBA components are always in u8 range (guaranteed by type).
    /// Roundtrip property: creating a Color from components then reading
    /// them back yields the original values.
    #[test]
    fn color_rgba_roundtrip(r in 0u8.., g in 0u8.., b in 0u8.., a in 0u8..) {
        let c = Color::rgba(r, g, b, a);
        assert_eq!(c.r, r);
        assert_eq!(c.g, g);
        assert_eq!(c.b, b);
        assert_eq!(c.a, a);
    }

    /// For any Rect, its center point is always contained.
    #[test]
    fn rect_contains_center(w: u32, h: u32) {
        let w = w % 1000 + 1;
        let h = h % 1000 + 1;
        let r = Rect::new(0, 0, w, h);
        assert!(r.contains(Point::new((w / 2) as i32, (h / 2) as i32)));
        // Origin is always contained
        assert!(r.contains(Point::new(0, 0)));
        // Points outside width/height should NOT be contained
        assert!(!r.contains(Point::new(w as i32, 0)));
        assert!(!r.contains(Point::new(0, h as i32)));
    }

    /// Rect intersection is always commutative.
    #[test]
    fn rect_intersection_commutative(x in 0i32..1000i32, y in 0i32..1000i32, w in 1u32..500u32, h in 1u32..500u32) {
        let a = Rect::new(x, y, w, h);
        let b = Rect::new(x + (w as i32 / 2), y + (h as i32 / 2), w, h);
        let ab = a.intersection(&b);
        let ba = b.intersection(&a);
        assert_eq!(ab, ba, "intersection must be commutative");
    }

    /// `Color` stores **straight** (non-premultiplied) RGBA: a colour channel may exceed alpha,
    /// and alpha-only operations must not touch RGB.
    ///
    /// This replaces an empty body named `color_premultiplied_invariants`: the type has no
    /// premultiply operation, and `let _ = c;` could never fail.
    #[test]
    fn color_alpha_operations_preserve_straight_channels(r in 0u8.., g in 0u8.., b in 0u8.., a in 0u8..) {
        let c = Color::rgba(r, g, b, a);
        // Straight storage: alpha does not clamp the colour channels.
        assert_eq!((c.r, c.g, c.b, c.a), (r, g, b, a));

        // An alpha-only change leaves straight RGB untouched.
        let cleared = c.with_alpha(0);
        assert_eq!((cleared.r, cleared.g, cleared.b), (r, g, b));
        assert_eq!(cleared.a, 0);

        // Inversion complements RGB and preserves alpha.
        let inverted = c.invert();
        assert_eq!(
            (inverted.r, inverted.g, inverted.b, inverted.a),
            (255 - r, 255 - g, 255 - b, a)
        );

        // The unit-range conversion is where a channel count could drift past 1.0.
        let (fr, fg, fb, fa) = c.to_f32();
        for (channel, expected) in [(fr, r), (fg, g), (fb, b), (fa, a)] {
            assert!(channel.is_finite() && (0.0..=1.0).contains(&channel));
            assert!((channel - expected as f32 / 255.0).abs() < f32::EPSILON);
        }
    }
}
