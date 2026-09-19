// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Geometry handling in the software rasterizer's primitive operations.
//!
//! # Why these are separate from the pixel tests
//!
//! Every primitive in `render/pipeline/primitives.rs` derives its span from `Rect`
//! by adding the rectangle's size to its origin. That addition used to be written
//! `rect.x + rect.width as f32 as i32`, which has two defects a build cannot see:
//!
//! 1. **It overflows `i32`.** Both terms are promoted to `i32` and then added
//!    unchecked, so a rectangle positioned near `i32::MAX` panics in debug and wraps
//!    in release. `Rect::x` is a plain `i32` a caller may set to anything.
//! 2. **It loses precision above 2^24.** `f32` has a 24-bit significand, so
//!    `16_777_217u32 as f32` is `16_777_216`; the round trip through `f32` silently
//!    drops low bits of a `u32` width. Saturating integer arithmetic is exact.
//!
//! `Rect::right` / `Rect::bottom` (`saturating_add_unsigned`) are the correct
//! helpers, and the primitives now go through them. These tests pin that, so the
//! cast cannot be reintroduced without a failure.
//!
//! The surface is double-buffered, so every test draws and then calls `end_frame`
//! before reading: `frame_rgba` returns the last **presented** frame, which without
//! the present would still be the cleared front buffer.

#![cfg(all(feature = "desktop", full_widgets))]

use rust_widgets::core::{Color, Rect, Size};
use rust_widgets::render::SoftwareSurface;

/// Reads one pixel as RGBA from a surface frame.
fn pixel(rgba: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let offset = ((y * width + x) * 4) as usize;
    [rgba[offset], rgba[offset + 1], rgba[offset + 2], rgba[offset + 3]]
}

/// Presents the back buffer so `frame_rgba` observes what was just drawn.
fn present(surface: &mut SoftwareSurface) {
    surface.end_frame();
}

/// A rectangle positioned near `i32::MAX` must not overflow its span.
///
/// The old form computed `rect.x + rect.width as f32 as i32` in `i32`. With the
/// origin just below the maximum and any non-zero width, that addition overflows:
/// a panic in a debug build, a wrapped (usually negative) span in release. Either
/// way a control placed there draws wrongly or dies. `saturating_add_unsigned`
/// clamps to `i32::MAX`, and the surface bounds check then reduces the span to the
/// part that is actually visible.
#[test]
fn a_rect_near_i32_max_does_not_overflow_its_span() {
    let mut surface = SoftwareSurface::new(Size::new(4, 4), 1.0);
    // Entirely off-surface to the right, so the expected result is "nothing drawn";
    // the assertion that matters is that computing the span does not panic.
    surface.fill_rect(Rect::new(i32::MAX - 2, 0, 8, 1), Color::RED);
    surface.draw_rect(Rect::new(i32::MAX - 2, 0, 8, 1), Color::RED);
    surface.fill_rounded_rect(Rect::new(i32::MAX - 2, 0, 8, 8), 2, Color::RED);
    surface.end_frame();

    let rgba = surface.frame_rgba();
    assert!(
        rgba.chunks(4).all(|chunk| chunk == [0, 0, 0, 0]),
        "a rect entirely off the surface must draw nothing"
    );
}

/// A width above `2^24` must be measured exactly, not rounded through `f32`.
///
/// `16_777_217u32 as f32` is `16_777_216`, so the old cast lost a pixel. The test
/// places the rect so that the lost pixel is the difference between covering the
/// last column and not covering it: at origin `-16_777_216`, an exact width of
/// `16_777_217` reaches column `0` while the rounded width stops one short.
#[test]
fn a_width_above_the_f32_significand_is_not_rounded_down() {
    const WIDTH: u32 = 16_777_217; // 2^24 + 1
    let mut surface = SoftwareSurface::new(Size::new(2, 1), 1.0);
    // The span is [-16_777_216, 1) exactly; the f32-rounded form ends at [.., 0).
    surface.fill_rect(Rect::new(-(1 << 24), 0, WIDTH, 1), Color::RED);
    surface.end_frame();

    let rgba = surface.frame_rgba();
    assert_eq!(
        pixel(rgba, 2, 0, 0),
        [255, 0, 0, 255],
        "column 0 is inside the exact span; rounding the width down to 2^24 excluded it"
    );
    assert_eq!(pixel(rgba, 2, 1, 0), [0, 0, 0, 0], "column 1 is outside the span");
}

/// A rectangle wider than `i32::MAX` must clamp rather than collapse.
#[test]
fn an_oversized_rect_fills_the_whole_surface_instead_of_wrapping_to_nothing() {
    let mut surface = SoftwareSurface::new(Size::new(8, 4), 1.0);
    surface.fill_rect(Rect::new(0, 0, u32::MAX, u32::MAX), Color::RED);
    present(&mut surface);

    let rgba = surface.frame_rgba();
    assert_eq!(
        pixel(rgba, 8, 0, 0),
        [255, 0, 0, 255],
        "a rect far larger than the surface must fill it; the truncating cast left it empty"
    );
    assert_eq!(pixel(rgba, 8, 7, 3), [255, 0, 0, 255], "the fill must reach the last pixel");
}

/// A rectangle flush with the surface edge must keep its last row and column.
///
/// This is the boundary the `- 1` in the inclusive spans depends on: `right() - 1`
/// is the last covered column. An off-by-one here silently drops a row or column
/// from every filled control.
#[test]
fn a_rect_flush_with_the_surface_edge_keeps_its_last_row_and_column() {
    let mut surface = SoftwareSurface::new(Size::new(4, 4), 1.0);
    surface.fill_rect(Rect::new(0, 0, 4, 4), Color::BLUE);
    present(&mut surface);

    let rgba = surface.frame_rgba();
    for y in 0..4 {
        for x in 0..4 {
            assert_eq!(
                pixel(rgba, 4, x, y),
                [0, 0, 255, 255],
                "pixel ({x},{y}) of a flush fill is missing"
            );
        }
    }
}

/// A stroked rectangle covering the surface must draw all four edges and leave the
/// interior clear.
///
/// `draw_rect_with_width` computes its endpoints as `right() - 1` / `bottom() - 1`;
/// if those were computed from the wrapped form, the right and bottom edges would
/// land outside the surface and be clipped away, so the frame would be missing two
/// sides.
#[test]
fn a_stroke_flush_with_the_surface_edge_draws_every_side() {
    let mut surface = SoftwareSurface::new(Size::new(6, 6), 1.0);
    surface.draw_rect(Rect::new(0, 0, 6, 6), Color::GREEN);
    present(&mut surface);

    let rgba = surface.frame_rgba();
    for x in 0..6 {
        assert_eq!(pixel(rgba, 6, x, 0), [0, 255, 0, 255], "top edge at x={x}");
        assert_eq!(pixel(rgba, 6, x, 5), [0, 255, 0, 255], "bottom edge at x={x}");
    }
    for y in 0..6 {
        assert_eq!(pixel(rgba, 6, 0, y), [0, 255, 0, 255], "left edge at y={y}");
        assert_eq!(pixel(rgba, 6, 5, y), [0, 255, 0, 255], "right edge at y={y}");
    }
    assert_eq!(pixel(rgba, 6, 3, 3), [0, 0, 0, 0], "the interior must stay clear");
}

/// A rectangle starting outside the surface must be clipped to the visible area.
#[test]
fn a_negative_origin_is_clipped_to_the_visible_area() {
    let mut surface = SoftwareSurface::new(Size::new(4, 4), 1.0);
    surface.fill_rect(Rect::new(-2, -2, 4, 4), Color::WHITE);
    present(&mut surface);

    let rgba = surface.frame_rgba();
    // The visible quarter is (0,0)..(1,1).
    assert_eq!(pixel(rgba, 4, 0, 0), [255, 255, 255, 255]);
    assert_eq!(pixel(rgba, 4, 1, 1), [255, 255, 255, 255]);
    assert_eq!(pixel(rgba, 4, 2, 2), [0, 0, 0, 0], "outside the rect must stay clear");
    assert_eq!(pixel(rgba, 4, 3, 3), [0, 0, 0, 0]);
}

/// A zero-sized rectangle must draw nothing rather than panic or bleed.
#[test]
fn a_zero_sized_rect_draws_nothing() {
    let mut surface = SoftwareSurface::new(Size::new(4, 4), 1.0);
    surface.fill_rect(Rect::new(1, 1, 0, 0), Color::RED);
    surface.draw_rect(Rect::new(1, 1, 0, 0), Color::RED);
    present(&mut surface);

    let rgba = surface.frame_rgba();
    assert!(
        rgba.chunks(4).all(|chunk| chunk == [0, 0, 0, 0]),
        "a zero-sized rect must leave the surface untouched"
    );
}
