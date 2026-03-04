//! Core primitives and library-wide contracts.

use std::fmt::Debug;

/// Stable numeric identifier used for widgets and objects.
pub type ObjectId = u64;

/// Runtime profile controlling feature and backend selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeProfile {
    /// Full desktop-oriented profile with optional advanced modules.
    Full,
    /// Reduced profile intended for constrained environments.
    Embedded,
}

/// Platform family classification for backends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformFamily {
    /// Traditional desktop runtime targets.
    Desktop,
    /// Embedded and constrained runtime targets.
    Embedded,
    /// Mobile runtime targets.
    Mobile,
}

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

    /// Decomposes the rectangle into `(position, size)`.
    pub const fn decompose(&self) -> (Point, Size) {
        (self.position(), self.size())
    }
}

/// RGBA color value.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Color {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
    /// Alpha channel.
    pub a: u8,
}

impl Color {
    /// Convenience constructor for an RGBA color.
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Returns an opaque RGB color.
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self::rgba(r, g, b, 255)
    }

    /// Parses `#RRGGBB`, `#RRGGBBAA`, `#RGB` or `#RGBA` hex color strings.
    ///
    /// The parser is intentionally strict and deterministic:
    /// - leading `#` is required
    /// - only ASCII hex digits are accepted
    /// - short notation is normalized by nibble expansion (`#abc` -> `#AABBCC`)
    pub fn parse_hex(text: &str) -> Option<Self> {
        let raw = text.trim();
        let hex = raw.strip_prefix('#')?;

        let parse_byte = |slice: &str| u8::from_str_radix(slice, 16).ok();
        let parse_nibble = |ch: char| ch.to_digit(16).map(|n| (n as u8) * 17);

        match hex.len() {
            3 => {
                let mut chars = hex.chars();
                Some(Self::rgb(
                    parse_nibble(chars.next()?)?,
                    parse_nibble(chars.next()?)?,
                    parse_nibble(chars.next()?)?,
                ))
            }
            4 => {
                let mut chars = hex.chars();
                Some(Self::rgba(
                    parse_nibble(chars.next()?)?,
                    parse_nibble(chars.next()?)?,
                    parse_nibble(chars.next()?)?,
                    parse_nibble(chars.next()?)?,
                ))
            }
            6 => Some(Self::rgb(
                parse_byte(&hex[0..2])?,
                parse_byte(&hex[2..4])?,
                parse_byte(&hex[4..6])?,
            )),
            8 => Some(Self::rgba(
                parse_byte(&hex[0..2])?,
                parse_byte(&hex[2..4])?,
                parse_byte(&hex[4..6])?,
                parse_byte(&hex[6..8])?,
            )),
            _ => None,
        }
    }

    /// Returns canonical uppercase `#RRGGBB` serialization.
    pub fn to_hex_rgb(&self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }

    /// Returns canonical uppercase `#RRGGBBAA` serialization.
    pub fn to_hex_rgba(&self) -> String {
        format!("#{:02X}{:02X}{:02X}{:02X}", self.r, self.g, self.b, self.a)
    }

    /// Packs color channels into `0xRRGGBBAA` for stable transport/serialization.
    pub const fn to_rgba_u32(&self) -> u32 {
        ((self.r as u32) << 24) | ((self.g as u32) << 16) | ((self.b as u32) << 8) | self.a as u32
    }

    /// Unpacks channels from `0xRRGGBBAA`.
    pub const fn from_rgba_u32(value: u32) -> Self {
        Self::rgba(
            ((value >> 24) & 0xFF) as u8,
            ((value >> 16) & 0xFF) as u8,
            ((value >> 8) & 0xFF) as u8,
            (value & 0xFF) as u8,
        )
    }
}

/// Font descriptor used by text rendering and themes.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(from = "FontSerde")]
pub struct Font {
    /// Font family name.
    pub family: String,
    /// Font point size.
    pub size: f32,
    /// Font weight in CSS-like scale (100..=900).
    #[serde(default = "Font::default_weight")]
    pub weight: u16,
    /// Whether bold style is requested.
    pub bold: bool,
    /// Whether italic style is requested.
    pub italic: bool,
}

impl Font {
    /// Shared regular text weight.
    pub const REGULAR_WEIGHT: u16 = 400;
    /// Shared bold text weight.
    pub const BOLD_WEIGHT: u16 = 700;

    /// Default weight used for backward-compatible deserialization.
    pub const fn default_weight() -> u16 {
        Self::REGULAR_WEIGHT
    }

    /// Creates a font descriptor.
    ///
    /// This compatibility constructor keeps existing call sites stable and
    /// derives `weight` from `bold` (`700` when bold, otherwise `400`).
    pub fn new(family: impl Into<String>, size: f32, bold: bool, italic: bool) -> Self {
        let weight = if bold {
            Self::BOLD_WEIGHT
        } else {
            Self::REGULAR_WEIGHT
        };
        Self::with_weight(family, size, weight, italic)
    }

    /// Creates a font descriptor with explicit weight.
    pub fn with_weight(family: impl Into<String>, size: f32, weight: u16, italic: bool) -> Self {
        let normalized_weight = Self::normalize_weight(weight);
        Self {
            family: family.into(),
            size,
            weight: normalized_weight,
            bold: normalized_weight >= Self::BOLD_WEIGHT,
            italic,
        }
    }

    /// Returns the shared default UI font descriptor.
    pub fn default_ui() -> Self {
        Self::with_weight("Arial", 14.0, Self::REGULAR_WEIGHT, false)
    }

    /// Returns a shared default bold UI descriptor.
    pub fn default_ui_bold() -> Self {
        Self::with_weight("Arial", 14.0, Self::BOLD_WEIGHT, false)
    }

    /// Normalizes arbitrary weight to nearest 100 in `[100, 900]`.
    pub const fn normalize_weight(weight: u16) -> u16 {
        let clamped = if weight < 100 {
            100
        } else if weight > 900 {
            900
        } else {
            weight
        };
        ((clamped + 50) / 100) * 100
    }

    /// Returns `true` if the font descriptor has a positive size and non-empty family.
    pub fn is_valid(&self) -> bool {
        !self.family.trim().is_empty()
            && self.size > 0.0
            && self.size.is_finite()
            && self.weight >= 100
            && self.weight <= 900
            && self.weight % 100 == 0
    }
}

impl Default for Font {
    fn default() -> Self {
        Self::default_ui()
    }
}

#[derive(serde::Deserialize)]
struct FontSerde {
    family: String,
    size: f32,
    #[serde(default = "Font::default_weight")]
    weight: u16,
    #[serde(default)]
    bold: bool,
    #[serde(default)]
    italic: bool,
}

impl From<FontSerde> for Font {
    fn from(value: FontSerde) -> Self {
        let normalized_weight = if value.weight == Font::default_weight() && value.bold {
            Font::BOLD_WEIGHT
        } else {
            Font::normalize_weight(value.weight)
        };

        Font {
            family: value.family,
            size: value.size,
            weight: normalized_weight,
            bold: normalized_weight >= Font::BOLD_WEIGHT,
            italic: value.italic,
        }
    }
}

/// Generic alignment options for layout/rendering APIs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alignment {
    /// Align to left edge.
    Left,
    /// Align to center.
    Center,
    /// Align to right edge.
    Right,
    /// Align to top edge.
    Top,
    /// Align to bottom edge.
    Bottom,
}

/// Horizontal alignment options for widget and layout APIs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HorizontalAlignment {
    Left,
    Center,
    Right,
}

/// Vertical alignment options for widget and layout APIs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerticalAlignment {
    Top,
    Center,
    Bottom,
}

impl HorizontalAlignment {
    /// Maps generic alignment to horizontal alignment when possible.
    pub const fn from_alignment(alignment: Alignment) -> Option<Self> {
        match alignment {
            Alignment::Left => Some(Self::Left),
            Alignment::Center => Some(Self::Center),
            Alignment::Right => Some(Self::Right),
            Alignment::Top | Alignment::Bottom => None,
        }
    }
}

impl VerticalAlignment {
    /// Maps generic alignment to vertical alignment when possible.
    pub const fn from_alignment(alignment: Alignment) -> Option<Self> {
        match alignment {
            Alignment::Top => Some(Self::Top),
            Alignment::Center => Some(Self::Center),
            Alignment::Bottom => Some(Self::Bottom),
            Alignment::Left | Alignment::Right => None,
        }
    }
}

/// Common trait implemented by id-addressable core objects.
pub trait CoreObject: Debug + Send + Sync {
    /// Get stable object id.
    fn id(&self) -> ObjectId;
    /// Set stable object id (used by object system adapters).
    fn set_id(&mut self, id: ObjectId);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point_and_size_constructors_are_stable() {
        let point = Point::new(10, -3);
        let size = Size::new(80, 24);
        assert_eq!(point, Point { x: 10, y: -3 });
        assert_eq!(size, Size { width: 80, height: 24 });
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
    fn font_default_and_validation_contract() {
        let font = Font::default_ui();
        assert!(font.is_valid());
        assert_eq!(Font::default(), font);
        assert!(!Font::new("", 12.0, false, false).is_valid());
        assert!(!Font::new("Sans", 0.0, false, false).is_valid());
        assert_eq!(font.weight, Font::REGULAR_WEIGHT);
        assert_eq!(Font::default_ui_bold().weight, Font::BOLD_WEIGHT);
    }

    #[test]
    fn font_weight_normalization_contract() {
        let light = Font::with_weight("Sans", 12.0, 149, false);
        let medium = Font::with_weight("Sans", 12.0, 550, false);
        let heavy = Font::with_weight("Sans", 12.0, 2000, false);
        assert_eq!(light.weight, 100);
        assert_eq!(medium.weight, 600);
        assert_eq!(heavy.weight, 900);
        assert!(Font::with_weight("Sans", 12.0, 700, false).bold);
        assert!(!Font::with_weight("Sans", 12.0, 600, false).bold);
    }

    #[test]
    fn axis_alignment_mapping_is_explicit() {
        assert_eq!(HorizontalAlignment::from_alignment(Alignment::Left), Some(HorizontalAlignment::Left));
        assert_eq!(HorizontalAlignment::from_alignment(Alignment::Top), None);
        assert_eq!(VerticalAlignment::from_alignment(Alignment::Bottom), Some(VerticalAlignment::Bottom));
        assert_eq!(VerticalAlignment::from_alignment(Alignment::Right), None);
    }

    #[test]
    fn color_hex_parse_and_serialize_are_deterministic() {
        assert_eq!(Color::parse_hex("#112233"), Some(Color::rgba(0x11, 0x22, 0x33, 0xFF)));
        assert_eq!(Color::parse_hex("#11223344"), Some(Color::rgba(0x11, 0x22, 0x33, 0x44)));
        assert_eq!(Color::parse_hex("#abc"), Some(Color::rgba(0xAA, 0xBB, 0xCC, 0xFF)));
        assert_eq!(Color::parse_hex(" #AbCd "), Some(Color::rgba(0xAA, 0xBB, 0xCC, 0xDD)));
        assert_eq!(Color::parse_hex("112233"), None);
        assert_eq!(Color::parse_hex("#12"), None);

        let color = Color::rgba(0x0A, 0x1B, 0x2C, 0x7D);
        assert_eq!(color.to_hex_rgb(), "#0A1B2C");
        assert_eq!(color.to_hex_rgba(), "#0A1B2C7D");
    }

    #[test]
    fn color_u32_pack_roundtrip_is_stable() {
        let color = Color::rgba(0x01, 0x23, 0x45, 0x67);
        let packed = color.to_rgba_u32();
        assert_eq!(packed, 0x01234567);
        assert_eq!(Color::from_rgba_u32(packed), color);
    }

    #[test]
    fn font_bold_is_derived_from_normalized_weight() {
        let normalized_to_bold = Font::with_weight("Sans", 12.0, 650, false);
        assert_eq!(normalized_to_bold.weight, 700);
        assert!(normalized_to_bold.bold);
    }

    #[cfg(not(feature = "embedded"))]
    #[test]
    fn font_deserialize_normalizes_weight_and_bold_contract() {
        let parsed: Font = serde_json::from_str(
            r#"{"family":"Sans","size":12.0,"weight":650,"bold":false,"italic":true}"#,
        )
        .expect("font deserialize should succeed");

        assert_eq!(parsed.weight, 700);
        assert!(parsed.bold);
        assert!(parsed.italic);

        let parsed_legacy: Font = serde_json::from_str(
            r#"{"family":"Sans","size":12.0,"bold":true,"italic":false}"#,
        )
        .expect("legacy font deserialize should succeed");

        assert_eq!(parsed_legacy.weight, 700);
        assert!(parsed_legacy.bold);
    }
}
