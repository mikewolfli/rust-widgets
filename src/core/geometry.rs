/// Two-dimensional point in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    /// Horizontal coordinate.
    pub x: i32,
    /// Vertical coordinate.
    pub y: i32,
}
impl Point {
    /// Creates a point at the provided coordinates.
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
    /// Creates a point from f32 coordinates (rounded to nearest integer).
    pub fn from_f32(x: f32, y: f32) -> Self {
        Self {
            x: x.round() as i32,
            y: y.round() as i32,
        }
    }
    /// Creates a point from f32 coordinates (truncated to integer).
    pub fn from_f32_trunc(x: f32, y: f32) -> Self {
        Self {
            x: x as i32,
            y: y as i32,
        }
    }
    /// Creates a point from u32 coordinates.
    pub const fn from_u32(x: u32, y: u32) -> Self {
        Self {
            x: x as i32,
            y: y as i32,
        }
    }
    /// Creates a point from i64 coordinates (clamped to i32 range).
    pub fn from_i64(x: i64, y: i64) -> Self {
        Self {
            x: x.clamp(i32::MIN as i64, i32::MAX as i64) as i32,
            y: y.clamp(i32::MIN as i64, i32::MAX as i64) as i32,
        }
    }
    /// Creates a point from tuple of i32.
    pub const fn from_i32_tuple((x, y): (i32, i32)) -> Self {
        Self::new(x, y)
    }
    /// Creates a point from tuple of f32 (rounded).
    pub fn from_f32_tuple((x, y): (f32, f32)) -> Self {
        Self::from_f32(x, y)
    }
    /// Creates a point from tuple of u32.
    pub const fn from_u32_tuple((x, y): (u32, u32)) -> Self {
        Self::from_u32(x, y)
    }
    /// Creates a point from f64 coordinates (rounded to nearest integer).
    pub fn from_f64(x: f64, y: f64) -> Self {
        Self {
            x: x.round() as i32,
            y: y.round() as i32,
        }
    }
    /// Creates a point from f64 coordinates (truncated to integer).
    pub fn from_f64_trunc(x: f64, y: f64) -> Self {
        Self {
            x: x as i32,
            y: y as i32,
        }
    }
    /// Creates a point from usize coordinates (clamped to i32 range).
    pub fn from_usize(x: usize, y: usize) -> Self {
        Self {
            x: x.clamp(0, i32::MAX as usize) as i32,
            y: y.clamp(0, i32::MAX as usize) as i32,
        }
    }
    /// Creates a point from isize coordinates (clamped to i32 range).
    pub fn from_isize(x: isize, y: isize) -> Self {
        Self {
            x: x.clamp(i32::MIN as isize, i32::MAX as isize) as i32,
            y: y.clamp(i32::MIN as isize, i32::MAX as isize) as i32,
        }
    }
    /// Creates a point from tuple of f64 (rounded).
    pub fn from_f64_tuple((x, y): (f64, f64)) -> Self {
        Self::from_f64(x, y)
    }
    /// Creates a point from tuple of usize (clamped).
    pub fn from_usize_tuple((x, y): (usize, usize)) -> Self {
        Self::from_usize(x, y)
    }
    /// Creates a point from tuple of isize (clamped).
    pub fn from_isize_tuple((x, y): (isize, isize)) -> Self {
        Self::from_isize(x, y)
    }
    /// Returns the origin point `(0, 0)`.
    pub const fn origin() -> Self {
        Self::new(0, 0)
    }
    /// Converts point to f32 coordinates.
    pub fn to_f32(&self) -> (f32, f32) {
        (self.x as f32, self.y as f32)
    }
    /// Converts point to f64 coordinates.
    pub fn to_f64(&self) -> (f64, f64) {
        (self.x as f64, self.y as f64)
    }
    /// Converts point to u32 coordinates (clamped to positive values).
    pub fn to_u32(&self) -> (u32, u32) {
        (
            self.x.max(0) as u32,
            self.y.max(0) as u32,
        )
    }
}
/// Width/height pair in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Size {
    /// Width in logical pixels.
    pub width: u32,
    /// Height in logical pixels.
    pub height: u32,
}
impl Size {
    /// Creates a size from width and height.
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
    /// Creates a size from f32 dimensions (rounded to nearest integer).
    pub fn from_f32(width: f32, height: f32) -> Self {
        Self {
            width: width.round().max(0.0) as u32,
            height: height.round().max(0.0) as u32,
        }
    }
    /// Creates a size from f32 dimensions (truncated to integer).
    pub fn from_f32_trunc(width: f32, height: f32) -> Self {
        Self {
            width: width.max(0.0) as u32,
            height: height.max(0.0) as u32,
        }
    }
    /// Creates a size from i32 dimensions (clamped to u32 range).
    pub fn from_i32(width: i32, height: i32) -> Self {
        Self {
            width: width.max(0) as u32,
            height: height.max(0) as u32,
        }
    }
    /// Creates a size from i64 dimensions (clamped to u32 range).
    pub fn from_i64(width: i64, height: i64) -> Self {
        Self {
            width: width.clamp(0, u32::MAX as i64) as u32,
            height: height.clamp(0, u32::MAX as i64) as u32,
        }
    }
    /// Creates a size from tuple of u32.
    pub const fn from_u32_tuple((width, height): (u32, u32)) -> Self {
        Self::new(width, height)
    }
    /// Creates a size from tuple of f32 (rounded).
    pub fn from_f32_tuple((width, height): (f32, f32)) -> Self {
        Self::from_f32(width, height)
    }
    /// Creates a size from tuple of i32 (clamped).
    pub fn from_i32_tuple((width, height): (i32, i32)) -> Self {
        Self::from_i32(width, height)
    }
    /// Creates a size from f64 dimensions (rounded to nearest integer).
    pub fn from_f64(width: f64, height: f64) -> Self {
        Self {
            width: width.round().max(0.0) as u32,
            height: height.round().max(0.0) as u32,
        }
    }
    /// Creates a size from f64 dimensions (truncated to integer).
    pub fn from_f64_trunc(width: f64, height: f64) -> Self {
        Self {
            width: width.max(0.0) as u32,
            height: height.max(0.0) as u32,
        }
    }
    /// Creates a size from usize dimensions (clamped to u32 range).
    pub fn from_usize(width: usize, height: usize) -> Self {
        Self {
            width: width.clamp(0, u32::MAX as usize) as u32,
            height: height.clamp(0, u32::MAX as usize) as u32,
        }
    }
    /// Creates a size from isize dimensions (clamped to u32 range).
    pub fn from_isize(width: isize, height: isize) -> Self {
        Self {
            width: width.clamp(0, u32::MAX as isize) as u32,
            height: height.clamp(0, u32::MAX as isize) as u32,
        }
    }
    /// Creates a size from tuple of f64 (rounded).
    pub fn from_f64_tuple((width, height): (f64, f64)) -> Self {
        Self::from_f64(width, height)
    }
    /// Creates a size from tuple of usize (clamped).
    pub fn from_usize_tuple((width, height): (usize, usize)) -> Self {
        Self::from_usize(width, height)
    }
    /// Creates a size from tuple of isize (clamped).
    pub fn from_isize_tuple((width, height): (isize, isize)) -> Self {
        Self::from_isize(width, height)
    }
    /// Converts size to f32 dimensions.
    pub fn to_f32(&self) -> (f32, f32) {
        (self.width as f32, self.height as f32)
    }
    /// Converts size to f64 dimensions.
    pub fn to_f64(&self) -> (f64, f64) {
        (self.width as f64, self.height as f64)
    }
    /// Converts size to i32 dimensions (may overflow for large sizes).
    pub fn to_i32(&self) -> (i32, i32) {
        (self.width as i32, self.height as i32)
    }
    /// Returns `true` when either axis is zero.
    pub const fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }
}
/// Axis-aligned rectangle in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    /// Left/top origin x.
    pub x: i32,
    /// Left/top origin y.
    pub y: i32,
    /// Rectangle width.
    pub width: u32,
    /// Rectangle height.
    pub height: u32,
}
impl Rect {
    /// Creates a rectangle from origin and extent.
    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
    /// Creates a rectangle from f32 coordinates (rounded to nearest integer).
    pub fn from_f32(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x: x.round() as i32,
            y: y.round() as i32,
            width: width.round().max(0.0) as u32,
            height: height.round().max(0.0) as u32,
        }
    }
    /// Creates a rectangle from f32 coordinates (truncated to integer).
    pub fn from_f32_trunc(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x: x as i32,
            y: y as i32,
            width: width.max(0.0) as u32,
            height: height.max(0.0) as u32,
        }
    }
    /// Creates a rectangle from u32 coordinates.
    pub const fn from_u32(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            x: x as i32,
            y: y as i32,
            width,
            height,
        }
    }
    /// Creates a rectangle from mixed types (i32 for position, u32 for size).
    pub const fn from_mixed(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self::new(x, y, width, height)
    }
    /// Creates a rectangle from i64 coordinates (clamped to appropriate ranges).
    pub fn from_i64(x: i64, y: i64, width: i64, height: i64) -> Self {
        Self {
            x: x.clamp(i32::MIN as i64, i32::MAX as i64) as i32,
            y: y.clamp(i32::MIN as i64, i32::MAX as i64) as i32,
            width: width.clamp(0, u32::MAX as i64) as u32,
            height: height.clamp(0, u32::MAX as i64) as u32,
        }
    }
    /// Creates a rectangle from tuple of i32, u32 (x, y, width, height).
    pub const fn from_tuple((x, y, width, height): (i32, i32, u32, u32)) -> Self {
        Self::new(x, y, width, height)
    }
    /// Creates a rectangle from tuple of f32 (rounded).
    pub fn from_f32_tuple((x, y, width, height): (f32, f32, f32, f32)) -> Self {
        Self::from_f32(x, y, width, height)
    }
    /// Creates a rectangle from tuple of u32.
    pub const fn from_u32_tuple((x, y, width, height): (u32, u32, u32, u32)) -> Self {
        Self::from_u32(x, y, width, height)
    }
    /// Creates a rectangle from f64 coordinates (rounded to nearest integer).
    pub fn from_f64(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x: x.round() as i32,
            y: y.round() as i32,
            width: width.round().max(0.0) as u32,
            height: height.round().max(0.0) as u32,
        }
    }
    /// Creates a rectangle from f64 coordinates (truncated to integer).
    pub fn from_f64_trunc(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x: x as i32,
            y: y as i32,
            width: width.max(0.0) as u32,
            height: height.max(0.0) as u32,
        }
    }
    /// Creates a rectangle from usize coordinates (clamped to appropriate ranges).
    pub fn from_usize(x: usize, y: usize, width: usize, height: usize) -> Self {
        Self {
            x: x.clamp(0, i32::MAX as usize) as i32,
            y: y.clamp(0, i32::MAX as usize) as i32,
            width: width.clamp(0, u32::MAX as usize) as u32,
            height: height.clamp(0, u32::MAX as usize) as u32,
        }
    }
    /// Creates a rectangle from isize coordinates (clamped to appropriate ranges).
    pub fn from_isize(x: isize, y: isize, width: isize, height: isize) -> Self {
        Self {
            x: x.clamp(i32::MIN as isize, i32::MAX as isize) as i32,
            y: y.clamp(i32::MIN as isize, i32::MAX as isize) as i32,
            width: width.clamp(0, u32::MAX as isize) as u32,
            height: height.clamp(0, u32::MAX as isize) as u32,
        }
    }
    /// Creates a rectangle from tuple of f64 (rounded).
    pub fn from_f64_tuple((x, y, width, height): (f64, f64, f64, f64)) -> Self {
        Self::from_f64(x, y, width, height)
    }
    /// Creates a rectangle from tuple of usize (clamped).
    pub fn from_usize_tuple((x, y, width, height): (usize, usize, usize, usize)) -> Self {
        Self::from_usize(x, y, width, height)
    }
    /// Creates a rectangle from tuple of isize (clamped).
    pub fn from_isize_tuple((x, y, width, height): (isize, isize, isize, isize)) -> Self {
        Self::from_isize(x, y, width, height)
    }
    /// Creates a rectangle from position and size.
    pub const fn from_position_size(position: Point, size: Size) -> Self {
        Self::new(position.x, position.y, size.width, size.height)
    }
    /// Returns the rectangle origin as a [`Point`].
    pub const fn position(&self) -> Point {
        Point::new(self.x, self.y)
    }
    /// Returns the rectangle extent as a [`Size`].
    pub const fn size(&self) -> Size {
        Size::new(self.width, self.height)
    }
    /// Returns `true` if width and height are both greater than zero.
    pub const fn is_valid(&self) -> bool {
        self.width > 0 && self.height > 0
    }
    /// Returns `true` if the rectangle contains the point (inclusive origin, exclusive max edge).
    pub const fn contains_point(&self, point: Point) -> bool {
        let max_x = self.x + self.width as i32;
        let max_y = self.y + self.height as i32;
        point.x >= self.x && point.y >= self.y && point.x < max_x && point.y < max_y
    }
    pub fn intersects(&self, other: &Rect) -> bool {
        let self_max_x = self.x + self.width as i32;
        let self_max_y = self.y + self.height as i32;
        let other_max_x = other.x + other.width as i32;
        let other_max_y = other.y + other.height as i32;
        self.x < other_max_x && self_max_x > other.x && self.y < other_max_y && self_max_y > other.y
    }
    pub fn contains_rect(&self, other: &Rect) -> bool {
        let self_max_x = self.x + self.width as i32;
        let self_max_y = self.y + self.height as i32;
        let other_max_x = other.x + other.width as i32;
        let other_max_y = other.y + other.height as i32;
        other.x >= self.x
            && other.y >= self.y
            && other_max_x <= self_max_x
            && other_max_y <= self_max_y
    }
    pub fn union(&self, other: &Rect) -> Rect {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        let self_max_x = self.x + self.width as i32;
        let self_max_y = self.y + self.height as i32;
        let other_max_x = other.x + other.width as i32;
        let other_max_y = other.y + other.height as i32;
        let max_x = self_max_x.max(other_max_x);
        let max_y = self_max_y.max(other_max_y);
        Rect::new(x, y, (max_x - x) as u32, (max_y - y) as u32)
    }
    pub fn intersection(&self, other: &Rect) -> Option<Rect> {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let self_max_x = self.x + self.width as i32;
        let self_max_y = self.y + self.height as i32;
        let other_max_x = other.x + other.width as i32;
        let other_max_y = other.y + other.height as i32;
        let max_x = self_max_x.min(other_max_x);
        let max_y = self_max_y.min(other_max_y);
        if max_x > x && max_y > y {
            Some(Rect::new(x, y, (max_x - x) as u32, (max_y - y) as u32))
        } else {
            None
        }
    }
    /// Decomposes the rectangle into `(position, size)`.
    pub const fn decompose(&self) -> (Point, Size) {
        (self.position(), self.size())
    }
    /// Converts rectangle to f32 coordinates.
    pub fn to_f32(&self) -> (f32, f32, f32, f32) {
        (
            self.x as f32,
            self.y as f32,
            self.width as f32,
            self.height as f32,
        )
    }
    /// Gets the right edge coordinate (exclusive).
    pub fn right(&self) -> i32 {
        self.x + self.width as i32
    }
    /// Gets the bottom edge coordinate (exclusive).
    pub fn bottom(&self) -> i32 {
        self.y + self.height as i32
    }
    /// Gets the center point of the rectangle.
    pub fn center(&self) -> Point {
        Point::new(
            self.x + (self.width as i32) / 2,
            self.y + (self.height as i32) / 2,
        )
    }
    /// Converts rectangle to f64 coordinates.
    pub fn to_f64(&self) -> (f64, f64, f64, f64) {
        (
            self.x as f64,
            self.y as f64,
            self.width as f64,
            self.height as f64,
        )
    }
    /// Converts rectangle to u32 coordinates (clamped to positive values).
    pub fn to_u32(&self) -> (u32, u32, u32, u32) {
        (
            self.x.max(0) as u32,
            self.y.max(0) as u32,
            self.width,
            self.height,
        )
    }
    /// Creates rectangle from two points (top-left and bottom-right).
    pub fn from_points(top_left: Point, bottom_right: Point) -> Self {
        let x = top_left.x.min(bottom_right.x);
        let y = top_left.y.min(bottom_right.y);
        let width = (bottom_right.x - top_left.x).abs() as u32;
        let height = (bottom_right.y - top_left.y).abs() as u32;
        Self::new(x, y, width, height)
    }
    /// Creates rectangle from center point and size.
    pub fn from_center(center: Point, size: Size) -> Self {
        Self::new(
            center.x - (size.width as i32) / 2,
            center.y - (size.height as i32) / 2,
            size.width,
            size.height,
        )
    }
    /// Creates rectangle with padding applied.
    pub fn with_padding(&self, padding: i32) -> Self {
        Self::new(
            self.x + padding,
            self.y + padding,
            (self.width as i32 - 2 * padding).max(0) as u32,
            (self.height as i32 - 2 * padding).max(0) as u32,
        )
    }
    /// Creates rectangle with margin applied.
    pub fn with_margin(&self, margin: i32) -> Self {
        Self::new(
            self.x - margin,
            self.y - margin,
            (self.width as i32 + 2 * margin).max(0) as u32,
            (self.height as i32 + 2 * margin).max(0) as u32,
        )
    }
    /// Returns `true` if the rectangle contains the point (alias for contains_point).
    pub fn contains(&self, point: Point) -> bool {
        self.contains_point(point)
    }
}
/// Direction used by directional widgets and layouts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    /// Main axis is horizontal.
    Horizontal,
    /// Main axis is vertical.
    Vertical,
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn point_and_size_constructors_are_stable() {
        let point = Point::new(10, -3);
        let size = Size::new(80, 24);
        assert_eq!(point, Point { x: 10, y: -3 });
        assert_eq!(
            size,
            Size {
                width: 80,
                height: 24
            }
        );
        assert!(!size.is_empty());
        assert!(Size::new(0, 1).is_empty());
    }
    #[test]
    fn rect_roundtrip_position_size_is_deterministic() {
        let position = Point::new(5, 7);
        let size = Size::new(120, 40);
        let rect = Rect::from_position_size(position, size);
        assert_eq!(rect.position(), position);
        assert_eq!(rect.size(), size);
        assert_eq!(rect.decompose(), (position, size));
        assert!(rect.is_valid());
        assert!(!Rect::new(0, 0, 0, 10).is_valid());
    }
    #[test]
    fn rect_contains_point_uses_exclusive_max_edge() {
        let rect = Rect::new(10, 10, 4, 4);
        assert!(rect.contains_point(Point::new(10, 10)));
        assert!(rect.contains_point(Point::new(13, 13)));
        assert!(!rect.contains_point(Point::new(14, 13)));
        assert!(!rect.contains_point(Point::new(13, 14)));
    }
    #[test]
    fn point_constructors_from_different_types() {
        let p1 = Point::from_f32(10.3, 20.7);
        assert_eq!(p1, Point::new(10, 21));
        
        let p2 = Point::from_f32(10.6, 20.4);
        assert_eq!(p2, Point::new(11, 20));
        
        let p3 = Point::from_f32_trunc(10.9, 20.1);
        assert_eq!(p3, Point::new(10, 20));
    }
    #[test]
    fn size_constructors_from_different_types() {
        let s1 = Size::from_f32(100.3, 200.7);
        assert_eq!(s1, Size::new(100, 201));
        
        let s2 = Size::from_f32(100.6, 200.4);
        assert_eq!(s2, Size::new(101, 200));
        
        let s3 = Size::from_f32_trunc(100.9, 200.1);
        assert_eq!(s3, Size::new(100, 200));
        
        let s4 = Size::from_usize(100, 200);
        assert_eq!(s4, Size::new(100, 200));
        
        let s5 = Size::from_isize(100, 200);
        assert_eq!(s5, Size::new(100, 200));
    }
    #[test]
    fn rect_constructors_from_different_types() {
        let r1 = Rect::from_f32(10.3, 20.7, 100.3, 200.7);
        assert_eq!(r1, Rect::new(10, 21, 100, 201));
        
        let r2 = Rect::from_f64(10.6, 20.4, 100.6, 200.4);
        assert_eq!(r2, Rect::new(11, 20, 101, 200));
        
        let r3 = Rect::from_usize(10, 20, 100, 200);
        assert_eq!(r3, Rect::new(10, 20, 100, 200));
        
        let r4 = Rect::from_isize(10, 20, 100, 200);
        assert_eq!(r4, Rect::new(10, 20, 100, 200));
    }
    #[test]
    fn rect_conversion_methods() {
        let rect = Rect::new(10, 20, 100, 200);
        
        let (x, y, w, h) = rect.to_f32();
        assert_eq!(x, 10.0);
        assert_eq!(y, 20.0);
        assert_eq!(w, 100.0);
        assert_eq!(h, 200.0);
        
        let (x, y, w, h) = rect.to_f64();
        assert_eq!(x, 10.0);
        assert_eq!(y, 20.0);
        assert_eq!(w, 100.0);
        assert_eq!(h, 200.0);
        
        let (x, y, w, h) = rect.to_u32();
        assert_eq!(x, 10);
        assert_eq!(y, 20);
        assert_eq!(w, 100);
        assert_eq!(h, 200);
    }
    #[test]
    fn rect_from_points_and_center() {
        let top_left = Point::new(10, 20);
        let bottom_right = Point::new(110, 220);
        let rect = Rect::from_points(top_left, bottom_right);
        assert_eq!(rect, Rect::new(10, 20, 100, 200));
        
        let center = Point::new(60, 120);
        let size = Size::new(100, 200);
        let rect2 = Rect::from_center(center, size);
        assert_eq!(rect2, Rect::new(10, 20, 100, 200));
    }
    #[test]
    fn rect_padding_and_margin() {
        let rect = Rect::new(10, 20, 100, 200);
        
        let padded = rect.with_padding(5);
        assert_eq!(padded, Rect::new(15, 25, 90, 190));
        
        let margined = rect.with_margin(5);
        assert_eq!(margined, Rect::new(5, 15, 110, 210));
        
        // Test with negative padding (should clamp to zero)
        let padded_neg = rect.with_padding(60);
        assert_eq!(padded_neg, Rect::new(70, 80, 0, 80));
    }
    #[test]
    fn rect_intersection_and_union() {
        let rect1 = Rect::new(10, 10, 100, 100);
        let rect2 = Rect::new(50, 50, 100, 100);
        let rect3 = Rect::new(200, 200, 100, 100);
        
        let intersect = rect1.intersection(&rect2);
        assert_eq!(intersect, Some(Rect::new(50, 50, 60, 60)));
        
        let intersect2 = rect1.intersection(&rect3);
        assert_eq!(intersect2, None);
        
        let union = rect1.union(&rect2);
        assert_eq!(union, Rect::new(10, 10, 140, 140));
    }
    #[test]
    fn rect_contains_and_intersects() {
        let rect1 = Rect::new(10, 10, 100, 100);
        let rect2 = Rect::new(20, 20, 50, 50);
        let rect3 = Rect::new(200, 200, 100, 100);
        
        assert!(rect1.contains_rect(&rect2));
        assert!(!rect2.contains_rect(&rect1));
        assert!(rect1.intersects(&rect2));
        assert!(!rect1.intersects(&rect3));
    }
    #[test]
    fn rect_center_and_edges() {
        let rect = Rect::new(10, 20, 100, 200);
        
        assert_eq!(rect.center(), Point::new(60, 120));
        assert_eq!(rect.right(), 110);
        assert_eq!(rect.bottom(), 220);
    }
}
