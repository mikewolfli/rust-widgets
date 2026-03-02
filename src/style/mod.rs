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
    pub const fn all(value: u32) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }
}

/// Per-side padding values around widget content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Padding {
    /// Top padding.
    pub top: u32,
    /// Right padding.
    pub right: u32,
    /// Bottom padding.
    pub bottom: u32,
    /// Left padding.
    pub left: u32,
}

impl Padding {
    /// Creates per-side padding values.
    pub const fn new(top: u32, right: u32, bottom: u32, left: u32) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    /// Creates equal padding on all sides.
    pub const fn all(value: u32) -> Self {
        Self::new(value, value, value, value)
    }

    /// Creates symmetric padding as `(vertical, horizontal)`.
    pub const fn symmetric(vertical: u32, horizontal: u32) -> Self {
        Self::new(vertical, horizontal, vertical, horizontal)
    }

    /// Creates padding from possibly-negative values, clamping each side to `>= 0`.
    pub fn normalized(top: i32, right: i32, bottom: i32, left: i32) -> Self {
        Self::new(
            normalize_side(top),
            normalize_side(right),
            normalize_side(bottom),
            normalize_side(left),
        )
    }

    /// Converts to shared edge-insets representation.
    pub const fn to_insets(&self) -> EdgeInsets {
        EdgeInsets {
            top: self.top,
            right: self.right,
            bottom: self.bottom,
            left: self.left,
        }
    }
}

/// Per-side outer spacing values around a widget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Margin {
    /// Top margin.
    pub top: u32,
    /// Right margin.
    pub right: u32,
    /// Bottom margin.
    pub bottom: u32,
    /// Left margin.
    pub left: u32,
}

impl Margin {
    /// Creates per-side margin values.
    pub const fn new(top: u32, right: u32, bottom: u32, left: u32) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    /// Creates equal margin on all sides.
    pub const fn all(value: u32) -> Self {
        Self::new(value, value, value, value)
    }

    /// Creates symmetric margin as `(vertical, horizontal)`.
    pub const fn symmetric(vertical: u32, horizontal: u32) -> Self {
        Self::new(vertical, horizontal, vertical, horizontal)
    }

    /// Creates margin from possibly-negative values, clamping each side to `>= 0`.
    pub fn normalized(top: i32, right: i32, bottom: i32, left: i32) -> Self {
        Self::new(
            normalize_side(top),
            normalize_side(right),
            normalize_side(bottom),
            normalize_side(left),
        )
    }

    /// Converts to shared edge-insets representation.
    pub const fn to_insets(&self) -> EdgeInsets {
        EdgeInsets {
            top: self.top,
            right: self.right,
            bottom: self.bottom,
            left: self.left,
        }
    }
}

impl Default for Padding {
    fn default() -> Self {
        Self::all(0)
    }
}

impl Default for Margin {
    fn default() -> Self {
        Self::all(0)
    }
}

impl From<Padding> for EdgeInsets {
    fn from(value: Padding) -> Self {
        value.to_insets()
    }
}

impl From<Margin> for EdgeInsets {
    fn from(value: Margin) -> Self {
        value.to_insets()
    }
}

const fn normalize_side(value: i32) -> u32 {
    if value <= 0 {
        0
    } else {
        value as u32
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
    pub padding: Padding,
    /// Outer widget margin.
    pub margin: Margin,
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
            padding: Padding::default(),
            margin: Margin::default(),
            shadow: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn padding_and_margin_normalize_negative_values() {
        let padding = Padding::normalized(-1, 4, -99, 8);
        let margin = Margin::normalized(-5, 0, 3, 2);
        assert_eq!(padding, Padding::new(0, 4, 0, 8));
        assert_eq!(margin, Margin::new(0, 0, 3, 2));
    }

    #[test]
    fn padding_and_margin_support_symmetric_builders() {
        assert_eq!(Padding::symmetric(6, 2), Padding::new(6, 2, 6, 2));
        assert_eq!(Margin::symmetric(3, 5), Margin::new(3, 5, 3, 5));
    }
}
