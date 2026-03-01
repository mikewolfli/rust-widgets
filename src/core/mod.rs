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

/// Width/height pair in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Size {
    /// Width in logical pixels.
    pub width: u32,
    /// Height in logical pixels.
    pub height: u32,
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
}

/// Font descriptor used by text rendering and themes.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Font {
    /// Font family name.
    pub family: String,
    /// Font point size.
    pub size: f32,
    /// Whether bold style is requested.
    pub bold: bool,
    /// Whether italic style is requested.
    pub italic: bool,
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

/// Common trait implemented by id-addressable core objects.
pub trait CoreObject: Debug + Send + Sync {
    /// Get stable object id.
    fn id(&self) -> ObjectId;
    /// Set stable object id (used by object system adapters).
    fn set_id(&mut self, id: ObjectId);
}
