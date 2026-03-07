//! Core primitives and library-wide contracts.
//!
//! # Coordinate System
//!
//! This framework uses a **screen coordinate system** with the origin at the **top-left corner**:
//!
//! - **X axis**: Increases from left to right (0 → width)
//! - **Y axis**: Increases from top to bottom (0 → height)
//!
//! ## Coordinate Conventions
//!
//! ```text
//! (0, 0) ───────────────► X
//!   │
//!   │    Screen Space (pixels)
//!   │    Origin: Top-Left
//!   │
//!   ▼ Y
//! ```
//!
//! ## Coordinate Transformations
//!
//! When working with other coordinate systems, use the helper functions:
//!
//! - `to_screen_y()`: Convert from Cartesian (bottom-left origin) to screen (top-left origin)
//! - `to_cartesian_y()`: Convert from screen (top-left origin) to Cartesian (bottom-left origin)
//! - `to_pdf_y()`: Convert from screen (top-left origin) to PDF (bottom-left origin)
//!
//! ## Module-Specific Notes
//!
//! - **Charts**: Data coordinates use Cartesian system (y increases upward), automatically converted to screen coordinates
//! - **PDF**: PDF uses bottom-left origin, converted from screen coordinates when rendering
//! - **SVG**: Uses same top-left origin as screen coordinates, no conversion needed
//! - **Widgets**: All widget positioning uses screen coordinates

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

    pub fn intersects(&self, other: &Rect) -> bool {
        let self_max_x = self.x + self.width as i32;
        let self_max_y = self.y + self.height as i32;
        let other_max_x = other.x + other.width as i32;
        let other_max_y = other.y + other.height as i32;
        
        self.x < other_max_x && self_max_x > other.x &&
        self.y < other_max_y && self_max_y > other.y
    }

    pub fn contains_rect(&self, other: &Rect) -> bool {
        let self_max_x = self.x + self.width as i32;
        let self_max_y = self.y + self.height as i32;
        let other_max_x = other.x + other.width as i32;
        let other_max_y = other.y + other.height as i32;
        
        other.x >= self.x && other.y >= self.y &&
        other_max_x <= self_max_x && other_max_y <= self_max_y
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

/// RGBA color value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
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

    /// Common color constants
    pub const BLACK: Self = Self::rgb(0, 0, 0);
    pub const WHITE: Self = Self::rgb(255, 255, 255);
    pub const RED: Self = Self::rgb(255, 0, 0);
    pub const GREEN: Self = Self::rgb(0, 255, 0);
    pub const BLUE: Self = Self::rgb(0, 0, 255);
    pub const YELLOW: Self = Self::rgb(255, 255, 0);
    pub const CYAN: Self = Self::rgb(0, 255, 255);
    pub const MAGENTA: Self = Self::rgb(255, 0, 255);
    pub const GRAY: Self = Self::rgb(128, 128, 128);
    pub const LIGHT_GRAY: Self = Self::rgb(200, 200, 200);
    pub const DARK_GRAY: Self = Self::rgb(64, 64, 64);
    pub const EXTRA_LIGHT_GRAY: Self = Self::rgb(230, 230, 230);
    pub const MEDIUM_GRAY: Self = Self::rgb(160, 160, 160);
    pub const EXTRA_DARK_GRAY: Self = Self::rgb(32, 32, 32);
    pub const TRANSPARENT: Self = Self::rgba(0, 0, 0, 0);

    /// Color variants
    pub const LIGHT_RED: Self = Self::rgb(255, 100, 100);
    pub const DARK_RED: Self = Self::rgb(150, 0, 0);
    pub const LIGHT_GREEN: Self = Self::rgb(100, 255, 100);
    pub const DARK_GREEN: Self = Self::rgb(0, 150, 0);
    pub const LIGHT_BLUE: Self = Self::rgb(100, 100, 255);
    pub const DARK_BLUE: Self = Self::rgb(0, 0, 150);
    pub const LIGHT_YELLOW: Self = Self::rgb(255, 255, 150);
    pub const DARK_YELLOW: Self = Self::rgb(150, 150, 0);

    /// UI color constants
    pub const PRIMARY: Self = Self::rgb(72, 142, 246);
    pub const SECONDARY: Self = Self::rgb(120, 124, 132);
    pub const SUCCESS: Self = Self::rgb(62, 165, 82);
    pub const WARNING: Self = Self::rgb(245, 166, 35);
    pub const ERROR: Self = Self::rgb(234, 57, 67);
    pub const BACKGROUND: Self = Self::rgb(245, 246, 248);
    pub const FOREGROUND: Self = Self::rgb(26, 28, 32);
    pub const LINK: Self = Self::rgb(0, 112, 201);
    pub const LINK_HOVER: Self = Self::rgb(0, 142, 251);
    pub const BORDER: Self = Self::rgb(192, 196, 204);
    pub const DIVIDER: Self = Self::rgb(223, 225, 230);
    pub const SELECTION: Self = Self::rgb(173, 216, 230);
    pub const TOOLTIP: Self = Self::rgb(255, 255, 224);
    pub const MENU_BACKGROUND: Self = Self::rgb(255, 255, 255);
    pub const MENU_FOREGROUND: Self = Self::rgb(26, 28, 32);

    /// Semantic colors
    pub const INFO: Self = Self::rgb(66, 133, 244);
    pub const NOTIFICATION: Self = Self::rgb(103, 58, 183);
    pub const DISABLED_BACKGROUND: Self = Self::rgb(245, 245, 245);
    pub const DISABLED_FOREGROUND: Self = Self::rgb(153, 153, 153);

    /// Neutral colors
    pub const ALICE_BLUE: Self = Self::rgb(240, 248, 255);
    pub const BEIGE: Self = Self::rgb(245, 245, 220);
    pub const CORAL: Self = Self::rgb(255, 127, 80);
    pub const GOLD: Self = Self::rgb(255, 215, 0);
    pub const IVORY: Self = Self::rgb(255, 255, 240);
    pub const LAVENDER: Self = Self::rgb(230, 230, 250);
    pub const ROSE: Self = Self::rgb(255, 105, 180);
    pub const SILVER: Self = Self::rgb(192, 192, 192);
    pub const TAN: Self = Self::rgb(210, 180, 140);

    /// Additional QT-like colors
    pub const AQUA: Self = Self::rgb(0, 255, 255);
    pub const BROWN: Self = Self::rgb(165, 42, 42);
    pub const FOREST_GREEN: Self = Self::rgb(34, 139, 34);
    pub const INDIGO: Self = Self::rgb(75, 0, 130);
    pub const MAROON: Self = Self::rgb(128, 0, 0);
    pub const NAVY: Self = Self::rgb(0, 0, 128);
    pub const OLIVE: Self = Self::rgb(128, 128, 0);
    pub const ORANGE: Self = Self::rgb(255, 165, 0);
    pub const PINK: Self = Self::rgb(255, 192, 203);
    pub const PURPLE: Self = Self::rgb(128, 0, 128);
    pub const TEAL: Self = Self::rgb(0, 128, 128);

    /// Additional WX-like colors
    pub const SKY_BLUE: Self = Self::rgb(135, 206, 235);
    pub const STEEL_BLUE: Self = Self::rgb(70, 130, 180);
    pub const SLATE_GRAY: Self = Self::rgb(112, 128, 144);
    pub const DARK_SLATE_GRAY: Self = Self::rgb(47, 79, 79);
    pub const LIGHT_SLATE_GRAY: Self = Self::rgb(119, 136, 153);
    pub const LIGHT_CYAN: Self = Self::rgb(224, 255, 255);
    pub const LIGHT_GOLDENROD_YELLOW: Self = Self::rgb(250, 250, 210);
    pub const LIGHT_PINK: Self = Self::rgb(255, 182, 193);
    pub const LIGHT_SALMON: Self = Self::rgb(255, 160, 122);

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
            && self.weight.is_multiple_of(100)
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
        assert_eq!(
            HorizontalAlignment::from_alignment(Alignment::Left),
            Some(HorizontalAlignment::Left)
        );
        assert_eq!(HorizontalAlignment::from_alignment(Alignment::Top), None);
        assert_eq!(
            VerticalAlignment::from_alignment(Alignment::Bottom),
            Some(VerticalAlignment::Bottom)
        );
        assert_eq!(VerticalAlignment::from_alignment(Alignment::Right), None);
    }

    #[test]
    fn color_hex_parse_and_serialize_are_deterministic() {
        assert_eq!(
            Color::parse_hex("#112233"),
            Some(Color::rgba(0x11, 0x22, 0x33, 0xFF))
        );
        assert_eq!(
            Color::parse_hex("#11223344"),
            Some(Color::rgba(0x11, 0x22, 0x33, 0x44))
        );
        assert_eq!(
            Color::parse_hex("#abc"),
            Some(Color::rgba(0xAA, 0xBB, 0xCC, 0xFF))
        );
        assert_eq!(
            Color::parse_hex(" #AbCd "),
            Some(Color::rgba(0xAA, 0xBB, 0xCC, 0xDD))
        );
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

        let parsed_legacy: Font =
            serde_json::from_str(r#"{"family":"Sans","size":12.0,"bold":true,"italic":false}"#)
                .expect("legacy font deserialize should succeed");

        assert_eq!(parsed_legacy.weight, 700);
        assert!(parsed_legacy.bold);
    }
}

/// Coordinate transformation utilities for converting between different coordinate systems.
///
/// The framework uses screen coordinates (top-left origin), but some modules
/// (charts, PDF) use different coordinate systems internally.
pub mod coords {
    use super::{Point, Rect};

    /// Converts a Y coordinate from Cartesian (bottom-left origin) to screen (top-left origin).
    ///
    /// # Arguments
    /// * `cartesian_y` - Y coordinate in Cartesian system (increases upward)
    /// * `height` - Total height of the coordinate space
    ///
    /// # Returns
    /// Y coordinate in screen system (increases downward)
    ///
    /// # Example
    /// ```
    /// # use rust_widgets::core::coords::to_screen_y;
    /// let screen_y = to_screen_y(10.0, 100.0); // Returns 90.0
    /// ```
    #[inline]
    pub fn to_screen_y(cartesian_y: f32, height: f32) -> f32 {
        height - cartesian_y
    }

    /// Converts a Y coordinate from screen (top-left origin) to Cartesian (bottom-left origin).
    ///
    /// # Arguments
    /// * `screen_y` - Y coordinate in screen system (increases downward)
    /// * `height` - Total height of the coordinate space
    ///
    /// # Returns
    /// Y coordinate in Cartesian system (increases upward)
    ///
    /// # Example
    /// ```
    /// # use rust_widgets::core::coords::to_cartesian_y;
    /// let cartesian_y = to_cartesian_y(90.0, 100.0); // Returns 10.0
    /// ```
    #[inline]
    pub fn to_cartesian_y(screen_y: f32, height: f32) -> f32 {
        height - screen_y
    }

    /// Converts a Y coordinate from screen (top-left origin) to PDF (bottom-left origin).
    ///
    /// # Arguments
    /// * `screen_y` - Y coordinate in screen system (increases downward)
    /// * `height` - Total height of the PDF page
    ///
    /// # Returns
    /// Y coordinate in PDF system (increases upward)
    ///
    /// # Example
    /// ```
    /// # use rust_widgets::core::coords::to_pdf_y;
    /// let pdf_y = to_pdf_y(90.0, 100.0); // Returns 10.0
    /// ```
    #[inline]
    pub fn to_pdf_y(screen_y: f32, height: f32) -> f32 {
        height - screen_y
    }

    /// Converts a Y coordinate from PDF (bottom-left origin) to screen (top-left origin).
    ///
    /// # Arguments
    /// * `pdf_y` - Y coordinate in PDF system (increases upward)
    /// * `height` - Total height of the PDF page
    ///
    /// # Returns
    /// Y coordinate in screen system (increases downward)
    ///
    /// # Example
    /// ```
    /// # use rust_widgets::core::coords::from_pdf_y;
    /// let screen_y = from_pdf_y(10.0, 100.0); // Returns 90.0
    /// ```
    #[inline]
    pub fn from_pdf_y(pdf_y: f32, height: f32) -> f32 {
        height - pdf_y
    }

    /// Converts a point from Cartesian to screen coordinates.
    ///
    /// # Arguments
    /// * `point` - Point in Cartesian coordinates
    /// * `height` - Total height of the coordinate space
    ///
    /// # Returns
    /// Point in screen coordinates
    #[inline]
    pub fn point_to_screen(point: Point, height: i32) -> Point {
        Point::new(point.x, height - point.y)
    }

    /// Converts a point from screen to Cartesian coordinates.
    ///
    /// # Arguments
    /// * `point` - Point in screen coordinates
    /// * `height` - Total height of the coordinate space
    ///
    /// # Returns
    /// Point in Cartesian coordinates
    #[inline]
    pub fn point_to_cartesian(point: Point, height: i32) -> Point {
        Point::new(point.x, height - point.y)
    }

    /// Converts a rectangle from Cartesian to screen coordinates.
    ///
    /// # Arguments
    /// * `rect` - Rectangle in Cartesian coordinates
    /// * `height` - Total height of the coordinate space
    ///
    /// # Returns
    /// Rectangle in screen coordinates
    ///
    /// # Note
    /// This function assumes the rectangle's position is in Cartesian coordinates.
    /// The width and height remain unchanged.
    #[inline]
    pub fn rect_to_screen(rect: Rect, height: i32) -> Rect {
        Rect::new(rect.x, height - rect.y - rect.height as i32, rect.width, rect.height)
    }

    /// Converts a rectangle from screen to Cartesian coordinates.
    ///
    /// # Arguments
    /// * `rect` - Rectangle in screen coordinates
    /// * `height` - Total height of the coordinate space
    ///
    /// # Returns
    /// Rectangle in Cartesian coordinates
    ///
    /// # Note
    /// This function assumes the rectangle's position is in screen coordinates.
    /// The width and height remain unchanged.
    #[inline]
    pub fn rect_to_cartesian(rect: Rect, height: i32) -> Rect {
        Rect::new(rect.x, height - rect.y - rect.height as i32, rect.width, rect.height)
    }

    /// Flips a Y coordinate around the center of a given height.
    ///
    /// # Arguments
    /// * `y` - Y coordinate to flip
    /// * `height` - Total height of the coordinate space
    ///
    /// # Returns
    /// Flipped Y coordinate
    #[inline]
    pub fn flip_y(y: f32, height: f32) -> f32 {
        height - y
    }

    /// Flips a point's Y coordinate around the center of a given height.
    ///
    /// # Arguments
    /// * `point` - Point to flip
    /// * `height` - Total height of the coordinate space
    ///
    /// # Returns
    /// Point with flipped Y coordinate
    #[inline]
    pub fn flip_point_y(point: Point, height: i32) -> Point {
        Point::new(point.x, height - point.y)
    }

    /// Flips a rectangle's Y coordinates around the center of a given height.
    ///
    /// # Arguments
    /// * `rect` - Rectangle to flip
    /// * `height` - Total height of the coordinate space
    ///
    /// # Returns
    /// Rectangle with flipped Y coordinates
    #[inline]
    pub fn flip_rect_y(rect: Rect, height: i32) -> Rect {
        Rect::new(rect.x, height - rect.y - rect.height as i32, rect.width, rect.height)
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
    }
}
