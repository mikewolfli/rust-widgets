// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use super::{Point, Rect};
/// Converts a Y coordinate from Cartesian (bottom-left origin) to screen (top-left origin).
#[inline]
pub fn to_screen_y(cartesian_y: f32, height: f32) -> f32 {
    flip_y(cartesian_y, height)
}
/// Converts a Y coordinate from Cartesian (bottom-left origin) to screen (top-left origin) for i32.
#[inline]
pub fn to_screen_y_i32(cartesian_y: i32, height: i32) -> i32 {
    height.saturating_sub(cartesian_y)
}
/// Converts a Y coordinate from screen (top-left origin) to Cartesian (bottom-left origin).
#[inline]
pub fn to_cartesian_y(screen_y: f32, height: f32) -> f32 {
    flip_y(screen_y, height)
}
/// Converts a Y coordinate from screen (top-left origin) to Cartesian (bottom-left origin) for i32.
#[inline]
pub fn to_cartesian_y_i32(screen_y: i32, height: i32) -> i32 {
    height.saturating_sub(screen_y)
}
/// Converts a Y coordinate from screen (top-left origin) to PDF (bottom-left origin).
#[inline]
pub fn to_pdf_y(screen_y: f32, height: f32) -> f32 {
    flip_y(screen_y, height)
}
/// Converts a Y coordinate from PDF (bottom-left origin) to screen (top-left origin).
#[inline]
pub fn from_pdf_y(pdf_y: f32, height: f32) -> f32 {
    flip_y(pdf_y, height)
}
/// Converts a point from Cartesian to screen coordinates.
#[inline]
pub fn point_to_screen(point: Point, height: i32) -> Point {
    Point::new(point.x, height.saturating_sub(point.y))
}
/// Converts a point from Cartesian (f32) to screen coordinates.
#[inline]
pub fn point_to_screen_f32(x: f32, y: f32, height: f32) -> (f32, f32) {
    (x, height - y)
}
/// Converts a point from screen to Cartesian coordinates.
#[inline]
pub fn point_to_cartesian(point: Point, height: i32) -> Point {
    Point::new(point.x, height.saturating_sub(point.y))
}
/// Converts a point from screen (f32) to Cartesian coordinates.
#[inline]
pub fn point_to_cartesian_f32(x: f32, y: f32, height: f32) -> (f32, f32) {
    (x, height - y)
}
/// Converts a rectangle from Cartesian to screen coordinates.
#[inline]
pub fn rect_to_screen(rect: Rect, height: i32) -> Rect {
    Rect::new(
        rect.x,
        height.saturating_sub(rect.y).saturating_sub_unsigned(rect.height),
        rect.width,
        rect.height,
    )
}
/// Converts a rectangle from screen to Cartesian coordinates.
#[inline]
pub fn rect_to_cartesian(rect: Rect, height: i32) -> Rect {
    Rect::new(
        rect.x,
        height.saturating_sub(rect.y).saturating_sub_unsigned(rect.height),
        rect.width,
        rect.height,
    )
}
/// Flips a Y coordinate around the center of a given height.
#[inline]
pub fn flip_y(y: f32, height: f32) -> f32 {
    height - y
}
/// Flips a point's Y coordinate around the center of a given height.
#[inline]
pub fn flip_point_y(point: Point, height: i32) -> Point {
    Point::new(point.x, height.saturating_sub(point.y))
}
/// Flips a rectangle's Y coordinates around the center of a given height.
#[inline]
pub fn flip_rect_y(rect: Rect, height: i32) -> Rect {
    Rect::new(
        rect.x,
        height.saturating_sub(rect.y).saturating_sub_unsigned(rect.height),
        rect.width,
        rect.height,
    )
}
/// Converts a rectangle from Cartesian to screen coordinates (f32).
#[inline]
pub fn rect_to_screen_f32(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    container_height: f32,
) -> (f32, f32, f32, f32) {
    (x, container_height - y - height, width, height)
}
/// Converts a rectangle from screen to Cartesian coordinates (f32).
#[inline]
pub fn rect_to_cartesian_f32(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    container_height: f32,
) -> (f32, f32, f32, f32) {
    (x, container_height - y - height, width, height)
}
/// Converts a rectangle from Cartesian to screen coordinates (f64).
#[inline]
pub fn rect_to_screen_f64(
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    container_height: f64,
) -> (f64, f64, f64, f64) {
    (x, container_height - y - height, width, height)
}
/// Converts a rectangle from screen to Cartesian coordinates (f64).
#[inline]
pub fn rect_to_cartesian_f64(
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    container_height: f64,
) -> (f64, f64, f64, f64) {
    (x, container_height - y - height, width, height)
}
/// Converts a point from Cartesian to screen coordinates (f64).
#[inline]
pub fn point_to_screen_f64(x: f64, y: f64, height: f64) -> (f64, f64) {
    (x, height - y)
}
/// Converts a point from screen to Cartesian coordinates (f64).
#[inline]
pub fn point_to_cartesian_f64(x: f64, y: f64, height: f64) -> (f64, f64) {
    (x, height - y)
}
/// Converts a Y coordinate from Cartesian to screen coordinates (f64).
#[inline]
pub fn to_screen_y_f64(cartesian_y: f64, height: f64) -> f64 {
    height - cartesian_y
}
/// Converts a Y coordinate from screen to Cartesian coordinates (f64).
#[inline]
pub fn to_cartesian_y_f64(screen_y: f64, height: f64) -> f64 {
    height - screen_y
}
/// Converts a Y coordinate from Cartesian to screen coordinates (u32).
#[inline]
pub fn to_screen_y_u32(cartesian_y: u32, height: u32) -> u32 {
    height.saturating_sub(cartesian_y)
}
/// Converts a Y coordinate from screen to Cartesian coordinates (u32).
#[inline]
pub fn to_cartesian_y_u32(screen_y: u32, height: u32) -> u32 {
    height.saturating_sub(screen_y)
}
/// Normalizes coordinates to a 0.0-1.0 range.
#[inline]
pub fn normalize_coords(x: f32, y: f32, width: f32, height: f32) -> (f32, f32) {
    let w = if width.is_finite() && width > 0.0 { width } else { 1.0 };
    let h = if height.is_finite() && height > 0.0 { height } else { 1.0 };
    let normalized_x = if x.is_finite() { (x / w).clamp(0.0, 1.0) } else { 0.0 };
    let normalized_y = if y.is_finite() { (y / h).clamp(0.0, 1.0) } else { 0.0 };
    (normalized_x, normalized_y)
}
/// Denormalizes coordinates from 0.0-1.0 range to pixel coordinates.
#[inline]
pub fn denormalize_coords(norm_x: f32, norm_y: f32, width: f32, height: f32) -> (f32, f32) {
    (norm_x * width, norm_y * height)
}
/// Clamps coordinates to within a rectangle.
#[inline]
pub fn clamp_point_to_rect(point: Point, rect: Rect) -> Point {
    let max_x = rect.x.saturating_add_unsigned(rect.width.max(1) - 1);
    let max_y = rect.y.saturating_add_unsigned(rect.height.max(1) - 1);
    Point::new(point.x.clamp(rect.x, max_x), point.y.clamp(rect.y, max_y))
}
/// Clamps coordinates to within a rectangle (f32).
#[inline]
pub fn clamp_point_to_rect_f32(
    x: f32,
    y: f32,
    rect_x: f32,
    rect_y: f32,
    rect_width: f32,
    rect_height: f32,
) -> (f32, f32) {
    let max_x = rect_x + rect_width.max(1.0) - 1.0;
    let max_y = rect_y + rect_height.max(1.0) - 1.0;
    (x.clamp(rect_x, max_x), y.clamp(rect_y, max_y))
}
/// Converts DPI-scaled coordinates to physical pixels.
#[inline]
pub fn dpi_to_pixels(value: f32, dpi_scale: f32) -> f32 {
    if !value.is_finite() || !dpi_scale.is_finite() || dpi_scale <= 0.0 {
        0.0
    } else {
        (value as f64 * dpi_scale as f64).clamp(f32::MIN as f64, f32::MAX as f64) as f32
    }
}
/// Converts physical pixels to DPI-scaled coordinates.
#[inline]
pub fn pixels_to_dpi(value: f32, dpi_scale: f32) -> f32 {
    if !value.is_finite() || !dpi_scale.is_finite() || dpi_scale <= 0.0 {
        0.0
    } else {
        (value as f64 / dpi_scale as f64).clamp(f32::MIN as f64, f32::MAX as f64) as f32
    }
}
/// Converts DPI-scaled coordinates to physical pixels (i32).
#[inline]
pub fn dpi_to_pixels_i32(value: i32, dpi_scale: f32) -> i32 {
    if !dpi_scale.is_finite() || dpi_scale <= 0.0 {
        0
    } else {
        (value as f64 * dpi_scale as f64).round().clamp(i32::MIN as f64, i32::MAX as f64) as i32
    }
}
/// Converts physical pixels to DPI-scaled coordinates (i32).
#[inline]
pub fn pixels_to_dpi_i32(value: i32, dpi_scale: f32) -> i32 {
    if !dpi_scale.is_finite() || dpi_scale <= 0.0 {
        0
    } else {
        (value as f64 / dpi_scale as f64).round().clamp(i32::MIN as f64, i32::MAX as f64) as i32
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_to_screen_y() {
        assert_eq!(to_screen_y(0.0, 100.0), 100.0);
        assert_eq!(to_screen_y(50.0, 100.0), 50.0);
        assert_eq!(to_screen_y(100.0, 100.0), 0.0);
    }
    #[test]
    fn test_to_cartesian_y() {
        assert_eq!(to_cartesian_y(0.0, 100.0), 100.0);
        assert_eq!(to_cartesian_y(50.0, 100.0), 50.0);
        assert_eq!(to_cartesian_y(100.0, 100.0), 0.0);
    }
    #[test]
    fn test_to_pdf_y() {
        assert_eq!(to_pdf_y(0.0, 100.0), 100.0);
        assert_eq!(to_pdf_y(50.0, 100.0), 50.0);
        assert_eq!(to_pdf_y(100.0, 100.0), 0.0);
    }
    #[test]
    fn test_from_pdf_y() {
        assert_eq!(from_pdf_y(0.0, 100.0), 100.0);
        assert_eq!(from_pdf_y(50.0, 100.0), 50.0);
        assert_eq!(from_pdf_y(100.0, 100.0), 0.0);
    }
    #[test]
    fn test_point_to_screen() {
        assert_eq!(point_to_screen(Point::new(10, 0), 100), Point::new(10, 100));
        assert_eq!(point_to_screen(Point::new(10, 50), 100), Point::new(10, 50));
        assert_eq!(point_to_screen(Point::new(10, 100), 100), Point::new(10, 0));
    }
    #[test]
    fn test_point_to_cartesian() {
        assert_eq!(point_to_cartesian(Point::new(10, 0), 100), Point::new(10, 100));
        assert_eq!(point_to_cartesian(Point::new(10, 50), 100), Point::new(10, 50));
        assert_eq!(point_to_cartesian(Point::new(10, 100), 100), Point::new(10, 0));
    }
    #[test]
    fn test_rect_to_screen() {
        let rect = Rect::new(10, 0, 50, 30);
        let screen_rect = rect_to_screen(rect, 100);
        assert_eq!(screen_rect.x, 10);
        assert_eq!(screen_rect.y, 70);
        assert_eq!(screen_rect.width, 50);
        assert_eq!(screen_rect.height, 30);
    }
    #[test]
    fn test_rect_to_cartesian() {
        let rect = Rect::new(10, 70, 50, 30);
        let cartesian_rect = rect_to_cartesian(rect, 100);
        assert_eq!(cartesian_rect.x, 10);
        assert_eq!(cartesian_rect.y, 0);
        assert_eq!(cartesian_rect.width, 50);
        assert_eq!(cartesian_rect.height, 30);
    }
    #[test]
    fn test_flip_y() {
        assert_eq!(flip_y(0.0, 100.0), 100.0);
        assert_eq!(flip_y(50.0, 100.0), 50.0);
        assert_eq!(flip_y(100.0, 100.0), 0.0);
    }
    #[test]
    fn test_flip_point_y() {
        assert_eq!(flip_point_y(Point::new(10, 0), 100), Point::new(10, 100));
        assert_eq!(flip_point_y(Point::new(10, 50), 100), Point::new(10, 50));
        assert_eq!(flip_point_y(Point::new(10, 100), 100), Point::new(10, 0));
    }
    #[test]
    fn test_flip_rect_y() {
        let rect = Rect::new(10, 0, 50, 30);
        let flipped = flip_rect_y(rect, 100);
        assert_eq!(flipped.x, 10);
        assert_eq!(flipped.y, 70);
        assert_eq!(flipped.width, 50);
        assert_eq!(flipped.height, 30);
    }
    #[test]
    fn test_roundtrip_conversions() {
        let y = 42.0;
        let height = 100.0;
        assert_eq!(to_cartesian_y(to_screen_y(y, height), height), y);
        assert_eq!(to_screen_y(to_cartesian_y(y, height), height), y);
    }

    #[test]
    fn zero_size_f32_rect_clamps_to_origin() {
        assert_eq!(clamp_point_to_rect_f32(20.0, -20.0, 5.0, 7.0, 0.0, 0.0), (5.0, 7.0));
    }

    #[test]
    fn non_positive_dpi_scale_returns_zero() {
        assert_eq!(pixels_to_dpi(200.0, 0.0), 0.0);
        assert_eq!(pixels_to_dpi_i32(200, -1.0), 0);
        assert_eq!(dpi_to_pixels_i32(i32::MAX, f32::INFINITY), 0);
        assert_eq!(dpi_to_pixels_i32(i32::MAX, 4.0), i32::MAX);
        assert_eq!(pixels_to_dpi_i32(i32::MAX, f32::INFINITY), 0);
        assert_eq!(pixels_to_dpi_i32(i32::MAX, 0.5), i32::MAX);
        assert_eq!(dpi_to_pixels(f32::INFINITY, 2.0), 0.0);
        assert_eq!(dpi_to_pixels(2.0, 0.0), 0.0);
        assert_eq!(pixels_to_dpi(f32::NAN, 2.0), 0.0);
    }

    #[test]
    fn i32_coordinate_flips_saturate_at_limits() {
        assert_eq!(to_screen_y_i32(i32::MIN, i32::MAX), i32::MAX);
        assert_eq!(point_to_screen(Point::new(0, i32::MIN), i32::MAX).y, i32::MAX);
        let rect = Rect::new(i32::MIN, i32::MIN, u32::MAX, u32::MAX);
        assert_eq!(rect_to_screen(rect, i32::MAX).y, i32::MIN);
        assert_eq!(flip_rect_y(rect, i32::MAX).y, i32::MIN);
    }

    #[test]
    fn normalize_coords_rejects_invalid_dimensions_and_clamps_range() {
        assert_eq!(normalize_coords(-10.0, 200.0, -1.0, f32::NAN), (0.0, 1.0));
        assert_eq!(normalize_coords(50.0, 25.0, 100.0, 50.0), (0.5, 0.5));
        assert_eq!(normalize_coords(f32::NAN, f32::INFINITY, 100.0, 100.0), (0.0, 0.0));
    }
}
