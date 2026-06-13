//! Style system primitives.
pub mod animation;
pub mod animation_group;
pub mod css;
pub mod css_watcher;
pub mod gradient;
pub mod selector;
pub mod stylesheet;
pub mod theme;
pub mod theme_state;
use crate::core::{Color, Font, Size};

// ── Style Inheritance Chain (BLUE11 R6.6) ──
//
// Widget style resolution follows this inheritance chain:
//
// 1. Global Theme defaults (ThemeManager → Theme)
// 2. ThemeOverrides per widget class (e.g., "Button", "Label")
// 3. Widget instance state (StatefulTheme → WidgetState)
// 4. Inline style overrides (future)
//
// The ThemeManager resolves: Theme → ThemeOverrides → WidgetState
// Each step falls through to the next level if unset.
pub use animation::*;
pub use animation_group::*;
pub use css::*;
pub use gradient::*;
pub use selector::*;
pub use stylesheet::*;
pub use theme::*;
pub use theme_state::*;
/// Whether the user prefers reduced motion (BLUE11 R7.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReducedMotionPreference {
    #[default]
    NoPreference,
    ReduceMotion,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
        Self { top, right, bottom, left }
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
    /// Returns self as a `Padding` value (identity conversion).
    pub const fn to_padding(&self) -> Padding {
        *self
    }
}
/// Per-side outer spacing values around a widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
        Self { top, right, bottom, left }
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
    /// Returns self as a `Padding` value (identity conversion).
    pub const fn to_padding(&self) -> Padding {
        Padding { top: self.top, right: self.right, bottom: self.bottom, left: self.left }
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
impl Shadow {
    /// Creates a new default shadow.
    pub fn new() -> Self {
        Self { x: 0, y: 0, blur: 0, color: Color::BLACK }
    }
    /// Sets the shadow offset.
    pub fn with_offset(mut self, x: i32, y: i32) -> Self {
        self.x = x;
        self.y = y;
        self
    }
    /// Sets the shadow blur radius.
    pub fn with_blur(mut self, blur: u32) -> Self {
        self.blur = blur;
        self
    }
    /// Sets the shadow color.
    pub fn with_color(mut self, c: Color) -> Self {
        self.color = c;
        self
    }
}
impl Default for Shadow {
    fn default() -> Self {
        Self::new()
    }
}
/// Minimum touch target dimensions by device class (BLUE8 P4-4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchTargetSize {
    /// PC touch screen: 32×32 pt.
    Desktop,
    /// Tablet: 44×44 pt.
    Tablet,
    /// Phone: 48×48 pt.
    Phone,
    /// Embedded: 40×40 pt.
    Embedded,
    /// Projection/remote control: 24×24 pt. (gated behind `projection` feature).
    #[cfg(feature = "projection")]
    Projection,
}

impl TouchTargetSize {
    /// Returns the minimum recommended pixel dimensions for this class.
    pub const fn dimensions(self) -> Size {
        match self {
            Self::Desktop => Size::new(32, 32),
            Self::Tablet => Size::new(44, 44),
            Self::Phone => Size::new(48, 48),
            Self::Embedded => Size::new(40, 40),
            #[cfg(feature = "projection")]
            Self::Projection => Size::new(24, 24),
        }
    }

    /// Returns the recommended spacing between interactive elements.
    pub const fn spacing(self) -> u32 {
        match self {
            Self::Desktop => 8,
            Self::Tablet => 12,
            Self::Phone => 16,
            Self::Embedded => 10,
            #[cfg(feature = "projection")]
            Self::Projection => 6,
        }
    }
}

/// Resolved style values applied to a widget.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct WidgetStyle {
    /// Optional background color.
    pub background_color: Option<Color>,
    /// Optional background gradient.
    pub background_gradient: Option<Gradient>,
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
    /// Optional minimum touch-target size override (BLUE8 P4-4).
    /// When set, hit testing expands the effective area to this size.
    pub touch_target: Option<Size>,
    /// Optional opacity (0.0 = transparent, 1.0 = opaque). Set via CSS `opacity`.
    pub opacity: Option<f32>,
}
impl WidgetStyle {
    /// Sets the background color.
    pub fn with_background(mut self, c: Color) -> Self {
        self.background_color = Some(c);
        self
    }
    /// Sets the text color.
    pub fn with_text_color(mut self, c: Color) -> Self {
        self.text_color = Some(c);
        self
    }
    /// Sets the font.
    pub fn with_font(mut self, f: Font) -> Self {
        self.font = Some(f);
        self
    }
    /// Sets the border.
    pub fn with_border(mut self, color: Color, width: u32, radius: u32) -> Self {
        self.border_color = Some(color);
        self.border_width = width;
        self.border_radius = radius;
        self
    }
    /// Sets the padding.
    pub fn with_padding(mut self, p: Padding) -> Self {
        self.padding = p;
        self
    }
    /// Sets the margin.
    pub fn with_margin(mut self, m: Margin) -> Self {
        self.margin = m;
        self
    }
    /// Sets the shadow.
    pub fn with_shadow(mut self, s: Shadow) -> Self {
        self.shadow = Some(s);
        self
    }
    /// Sets the minimum touch target size (BLUE8 P4-4).
    pub fn with_touch_target(mut self, target: Size) -> Self {
        self.touch_target = Some(target);
        self
    }
    /// Sets the background gradient.
    pub fn with_gradient(mut self, g: Gradient) -> Self {
        self.background_gradient = Some(g);
        self
    }
    /// Sets the opacity (CSS `opacity`).
    pub fn with_opacity(mut self, opacity: f32) -> Self {
        self.opacity = Some(opacity.clamp(0.0, 1.0));
        self
    }

    /// Create a child style by inheriting non-set properties from a parent style.
    /// Properties that are `None` in `self` fall back to the parent's values.
    pub fn inherit_from(&self, parent: &WidgetStyle) -> WidgetStyle {
        WidgetStyle {
            background_color: self.background_color.or(parent.background_color),
            background_gradient: self
                .background_gradient
                .clone()
                .or(parent.background_gradient.clone()),
            text_color: self.text_color.or(parent.text_color),
            font: self.font.clone().or(parent.font.clone()),
            border_color: self.border_color.or(parent.border_color),
            border_width: if self.border_width > 0 {
                self.border_width
            } else {
                parent.border_width
            },
            border_radius: if self.border_radius > 0 {
                self.border_radius
            } else {
                parent.border_radius
            },
            padding: self.padding,
            margin: self.margin.clone(),
            shadow: self.shadow.clone().or(parent.shadow.clone()),
            touch_target: self.touch_target.or(parent.touch_target),
            opacity: self.opacity.or(parent.opacity),
        }
    }

    /// Merge another style into this one: set each property if it's `None` (or default).
    pub fn merge(&mut self, other: &WidgetStyle) {
        if self.background_color.is_none() {
            self.background_color = other.background_color;
        }
        if self.background_gradient.is_none() {
            self.background_gradient.clone_from(&other.background_gradient);
        }
        if self.text_color.is_none() {
            self.text_color = other.text_color;
        }
        if self.font.is_none() {
            self.font.clone_from(&other.font);
        }
        if self.border_color.is_none() {
            self.border_color = other.border_color;
        }
        if self.border_width == 0 {
            self.border_width = other.border_width;
        }
        if self.border_radius == 0 {
            self.border_radius = other.border_radius;
        }
        if self.shadow.is_none() {
            self.shadow = other.shadow.clone();
        }
        if self.touch_target.is_none() {
            self.touch_target = other.touch_target;
        }
        if self.opacity.is_none() {
            self.opacity = other.opacity;
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
    #[test]
    fn inherit_from_falls_back_to_parent() {
        let parent =
            WidgetStyle::default().with_background(Color::RED).with_text_color(Color::BLUE);
        let child = WidgetStyle::default();
        let inherited = child.inherit_from(&parent);
        assert_eq!(inherited.background_color, Some(Color::RED));
        assert_eq!(inherited.text_color, Some(Color::BLUE));
    }
    #[test]
    fn inherit_from_child_takes_precedence() {
        let parent = WidgetStyle::default().with_background(Color::RED);
        let child = WidgetStyle::default().with_background(Color::GREEN);
        let inherited = child.inherit_from(&parent);
        assert_eq!(inherited.background_color, Some(Color::GREEN));
    }
    #[test]
    fn merge_updates_missing_properties() {
        let mut base = WidgetStyle::default().with_background(Color::RED);
        let overlay = WidgetStyle::default().with_text_color(Color::BLUE);
        base.merge(&overlay);
        assert_eq!(base.background_color, Some(Color::RED));
        assert_eq!(base.text_color, Some(Color::BLUE));
    }
    #[test]
    fn merge_does_not_override_existing() {
        let mut base = WidgetStyle::default().with_background(Color::RED);
        let overlay = WidgetStyle::default().with_background(Color::GREEN);
        base.merge(&overlay);
        assert_eq!(base.background_color, Some(Color::RED)); // unchanged
    }
}
