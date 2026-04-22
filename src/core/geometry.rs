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

    /// Returns the origin point `(0, 0)`.
    pub const fn origin() -> Self {
        Self::new(0, 0)
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
}
