//! Style system primitives.

use crate::core::{Color, Font};

/// Insets for top/right/bottom/left spacing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeInsets {
    /// Top inset.
    pub top: u32,
    /// Right inset.
    pub right: u32,
    /// Bottom inset.
    pub bottom: u32,
    /// Left inset.
    pub left: u32,
}

impl EdgeInsets {
    /// Create equal inset values on all sides.
    pub fn all(value: u32) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }
}

/// Drop-shadow style token.
#[derive(Debug, Clone, PartialEq)]
pub struct Shadow {
    /// Horizontal offset.
    pub x: i32,
    /// Vertical offset.
    pub y: i32,
    /// Blur radius.
    pub blur: u32,
    /// Shadow color.
    pub color: Color,
}

/// Resolved style values applied to a widget.
#[derive(Debug, Clone, PartialEq)]
pub struct WidgetStyle {
    /// Optional background color.
    pub background_color: Option<Color>,
    /// Optional text color.
    pub text_color: Option<Color>,
    /// Optional text font.
    pub font: Option<Font>,
    /// Optional border color.
    pub border_color: Option<Color>,
    /// Border width in logical pixels.
    pub border_width: u32,
    /// Border radius in logical pixels.
    pub border_radius: u32,
    /// Inner content padding.
    pub padding: EdgeInsets,
    /// Outer widget margin.
    pub margin: EdgeInsets,
    /// Optional drop shadow.
    pub shadow: Option<Shadow>,
}

impl Default for WidgetStyle {
    fn default() -> Self {
        Self {
            background_color: None,
            text_color: None,
            font: None,
            border_color: None,
            border_width: 0,
            border_radius: 0,
            padding: EdgeInsets::all(0),
            margin: EdgeInsets::all(0),
            shadow: None,
        }
    }
}
