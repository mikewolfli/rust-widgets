use super::{Point, Rect};
/// Converts a Y coordinate from Cartesian (bottom-left origin) to screen (top-left origin).
#[inline]
pub fn to_screen_y(cartesian_y: f32, height: f32) -> f32 {
    height - cartesian_y
}
/// Converts a Y coordinate from Cartesian (bottom-left origin) to screen (top-left origin) for i32.
#[inline]
pub fn to_screen_y_i32(cartesian_y: i32, height: i32) -> i32 {
    height - cartesian_y
}
/// Converts a Y coordinate from screen (top-left origin) to Cartesian (bottom-left origin).
#[inline]
pub fn to_cartesian_y(screen_y: f32, height: f32) -> f32 {
    height - screen_y
}
/// Converts a Y coordinate from screen (top-left origin) to Cartesian (bottom-left origin) for i32.
#[inline]
pub fn to_cartesian_y_i32(screen_y: i32, height: i32) -> i32 {
    height - screen_y
}
/// Converts a Y coordinate from screen (top-left origin) to PDF (bottom-left origin).
#[inline]
pub fn to_pdf_y(screen_y: f32, height: f32) -> f32 {
    height - screen_y
}
/// Converts a Y coordinate from PDF (bottom-left origin) to screen (top-left origin).
#[inline]
pub fn from_pdf_y(pdf_y: f32, height: f32) -> f32 {
    height - pdf_y
}
/// Converts a point from Cartesian to screen coordinates.
#[inline]
pub fn point_to_screen(point: Point, height: i32) -> Point {
    Point::new(point.x, height - point.y)
}
/// Converts a point from Cartesian (f32) to screen coordinates.
#[inline]
pub fn point_to_screen_f32(x: f32, y: f32, height: f32) -> (f32, f32) {
    (x, height - y)
}
/// Converts a point from screen to Cartesian coordinates.
#[inline]
pub fn point_to_cartesian(point: Point, height: i32) -> Point {
    Point::new(point.x, height - point.y)
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
        height - rect.y - rect.height as i32,
        rect.width,
        rect.height,
    )
}
/// Converts a rectangle from screen to Cartesian coordinates.
#[inline]
pub fn rect_to_cartesian(rect: Rect, height: i32) -> Rect {
    Rect::new(
        rect.x,
        height - rect.y - rect.height as i32,
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
    Point::new(point.x, height - point.y)
}
/// Flips a rectangle's Y coordinates around the center of a given height.
#[inline]
pub fn flip_rect_y(rect: Rect, height: i32) -> Rect {
    Rect::new(
        rect.x,
        height - rect.y - rect.height as i32,
        rect.width,
        rect.height,
    )
}
/// Converts a rectangle from Cartesian to screen coordinates (f32).
#[inline]
pub fn rect_to_screen_f32(x: f32, y: f32, width: f32, height: f32, container_height: f32) -> (f32, f32, f32, f32) {
    (x, container_height - y - height, width, height)
}
/// Converts a rectangle from screen to Cartesian coordinates (f32).
#[inline]
pub fn rect_to_cartesian_f32(x: f32, y: f32, width: f32, height: f32, container_height: f32) -> (f32, f32, f32, f32) {
    (x, container_height - y - height, width, height)
}
/// Converts a rectangle from Cartesian to screen coordinates (f64).
#[inline]
pub fn rect_to_screen_f64(x: f64, y: f64, width: f64, height: f64, container_height: f64) -> (f64, f64, f64, f64) {
    (x, container_height - y - height, width, height)
}
/// Converts a rectangle from screen to Cartesian coordinates (f64).
#[inline]
pub fn rect_to_cartesian_f64(x: f64, y: f64, width: f64, height: f64, container_height: f64) -> (f64, f64, f64, f64) {
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
    (x / width, y / height)
}
/// Denormalizes coordinates from 0.0-1.0 range to pixel coordinates.
#[inline]
pub fn denormalize_coords(norm_x: f32, norm_y: f32, width: f32, height: f32) -> (f32, f32) {
    (norm_x * width, norm_y * height)
}
/// Clamps coordinates to within a rectangle.
#[inline]
pub fn clamp_point_to_rect(point: Point, rect: Rect) -> Point {
    Point::new(
        point.x.clamp(rect.x, rect.x + rect.width as i32 - 1),
        point.y.clamp(rect.y, rect.y + rect.height as i32 - 1),
    )
}
/// Clamps coordinates to within a rectangle (f32).
#[inline]
pub fn clamp_point_to_rect_f32(x: f32, y: f32, rect_x: f32, rect_y: f32, rect_width: f32, rect_height: f32) -> (f32, f32) {
    (
        x.clamp(rect_x, rect_x + rect_width - 1.0),
        y.clamp(rect_y, rect_y + rect_height - 1.0),
    )
}
/// Converts DPI-scaled coordinates to physical pixels.
#[inline]
pub fn dpi_to_pixels(value: f32, dpi_scale: f32) -> f32 {
    value * dpi_scale
}
/// Converts physical pixels to DPI-scaled coordinates.
#[inline]
pub fn pixels_to_dpi(value: f32, dpi_scale: f32) -> f32 {
    value / dpi_scale
}
/// Converts DPI-scaled coordinates to physical pixels (i32).
#[inline]
pub fn dpi_to_pixels_i32(value: i32, dpi_scale: f32) -> i32 {
    (value as f32 * dpi_scale).round() as i32
}
/// Converts physical pixels to DPI-scaled coordinates (i32).
#[inline]
pub fn pixels_to_dpi_i32(value: i32, dpi_scale: f32) -> i32 {
    (value as f32 / dpi_scale).round() as i32
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
        assert_eq!(
            point_to_cartesian(Point::new(10, 0), 100),
            Point::new(10, 100)
        );
        assert_eq!(
            point_to_cartesian(Point::new(10, 50), 100),
            Point::new(10, 50)
        );
        assert_eq!(
            point_to_cartesian(Point::new(10, 100), 100),
            Point::new(10, 0)
        );
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
}
