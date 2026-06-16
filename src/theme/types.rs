use crate::compat::HashMap;
use crate::core::{Color, Font};
#[cfg(not(feature = "mini"))]
use serde::{Deserialize, Serialize};

/// High-level theme definition used by runtime style resolution.
#[cfg_attr(not(feature = "mini"), derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct Theme {
    /// Theme unique name.
    pub name: String,
    /// Semantic color tokens.
    pub colors: Colors,
    /// Font tokens.
    pub fonts: Fonts,
    /// Spacing tokens.
    pub spacing: Spacing,
    /// Border/elevation tokens.
    pub borders: Borders,
    /// Class-level style overrides applied after base resolution.
    pub overrides: ThemeOverrides,
}

/// Semantic color palette tokens.
#[cfg_attr(not(feature = "mini"), derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct Colors {
    /// Default background color.
    pub background: Color,
    /// Default foreground/text color.
    pub foreground: Color,
    /// Primary brand/action color.
    pub primary: Color,
    /// Secondary neutral color.
    pub secondary: Color,
    /// Accent color.
    pub accent: Color,
    /// Error state color.
    pub error: Color,
    /// Warning state color.
    pub warning: Color,
    /// Success state color.
    pub success: Color,
    /// Disabled-state color.
    pub disabled: Color,
    /// Informational state color.
    #[cfg_attr(not(feature = "mini"), serde(default = "default_info_color"))]
    pub info: Color,
}

/// Default info color used for backward-compatible deserialization.
const fn default_info_color() -> Color {
    Color::INFO
}

impl Color {
    /// Parses a hex color string (`"#RRGGBB"` or `"#RRGGBBAA"`) into a `Color`.
    ///
    /// # Errors
    /// Returns an error if the hex string is malformed or missing the `#` prefix.
    pub fn from_hex(hex: &str) -> Result<Self, String> {
        Self::parse_hex(hex).ok_or_else(|| format!("Invalid hex color string: '{}'", hex))
    }

    /// Serializes the color to `"#RRGGBBAA"` hex format.
    pub fn to_hex(&self) -> String {
        self.to_hex_rgba()
    }

    /// Returns a darkened variant of this color by reducing each RGB component
    /// by the given `factor` (clamped to `[0.0, 1.0]`).
    ///
    /// A factor of `0.0` leaves the color unchanged; `1.0` produces black.
    /// Alpha is preserved unchanged.
    pub fn dark_variant(&self, factor: f32) -> Self {
        let f = factor.clamp(0.0, 1.0);
        Self::rgba(
            (self.r as f32 * (1.0 - f)).round().clamp(0.0, 255.0) as u8,
            (self.g as f32 * (1.0 - f)).round().clamp(0.0, 255.0) as u8,
            (self.b as f32 * (1.0 - f)).round().clamp(0.0, 255.0) as u8,
            self.a,
        )
    }

    /// Returns a lightened variant of this color by increasing each RGB component
    /// toward 255 by the given `factor` (clamped to `[0.0, 1.0]`).
    ///
    /// A factor of `0.0` leaves the color unchanged; `1.0` produces white.
    /// Alpha is preserved unchanged.
    pub fn light_variant(&self, factor: f32) -> Self {
        let f = factor.clamp(0.0, 1.0);
        Self::rgba(
            (self.r as f32 + (255.0 - self.r as f32) * f).round().clamp(0.0, 255.0) as u8,
            (self.g as f32 + (255.0 - self.g as f32) * f).round().clamp(0.0, 255.0) as u8,
            (self.b as f32 + (255.0 - self.b as f32) * f).round().clamp(0.0, 255.0) as u8,
            self.a,
        )
    }
}

/// Font token set used by theme consumers.
#[cfg_attr(not(feature = "mini"), derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct Fonts {
    /// Regular text font token.
    pub regular: Font,
    /// Bold text font token.
    pub bold: Font,
    /// Italic text font token.
    pub italic: Font,
    /// Monospace font token.
    pub monospace: Font,
    /// Caption / footnote font token (small, secondary text).
    #[cfg_attr(not(feature = "mini"), serde(default = "default_caption_font"))]
    pub caption: Font,
    /// Body text font token (default paragraph text).
    #[cfg_attr(not(feature = "mini"), serde(default = "default_body_font"))]
    pub body: Font,
    /// Title font token (section or widget titles).
    #[cfg_attr(not(feature = "mini"), serde(default = "default_title_font"))]
    pub title: Font,
    /// Headline font token (prominent section headings).
    #[cfg_attr(not(feature = "mini"), serde(default = "default_headline_font"))]
    pub headline: Font,
    /// Display font token (large, decorative text).
    #[cfg_attr(not(feature = "mini"), serde(default = "default_display_font"))]
    pub display: Font,
}

/// Default caption font: Arial 11px, regular.
fn default_caption_font() -> Font {
    Font::simple("Arial", 11.0)
}

/// Default body font: Arial 14px, regular.
fn default_body_font() -> Font {
    Font::simple("Arial", 14.0)
}

/// Default title font: Arial 16px, bold.
fn default_title_font() -> Font {
    Font::bold("Arial", 16.0)
}

/// Default headline font: Arial 20px, bold.
fn default_headline_font() -> Font {
    Font::bold("Arial", 20.0)
}

/// Default display font: Arial 28px, bold.
fn default_display_font() -> Font {
    Font::bold("Arial", 28.0)
}

/// Spacing scale tokens.
///
/// # Recommendation
/// For more granular spacing, consider adding additional levels such as:
/// - `extra_small: u32` — 2px for tight spacing
/// - `huge: u32` — 48px for generous layout gaps
/// - `massive: u32` — 64px for section separators
#[cfg_attr(not(feature = "mini"), derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct Spacing {
    /// Small spacing unit.
    pub small: u32,
    /// Medium spacing unit.
    pub medium: u32,
    /// Large spacing unit.
    pub large: u32,
    /// Extra-large spacing unit.
    pub extra_large: u32,
}

/// Border and elevation behavior tokens.
#[cfg_attr(not(feature = "mini"), derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct Borders {
    /// Default border width.
    pub width: u32,
    /// Default corner radius.
    pub radius: u32,
    /// Whether drop shadows are enabled.
    pub shadow: bool,
}

/// Style override map used for class-level theme customization.
#[cfg_attr(not(feature = "mini"), derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct ThemeOverrides {
    /// Overrides keyed by style/class name.
    pub styles: HashMap<String, ThemeStyleToken>,
}

/// Optional style tokens used to override resolved widget styles.
#[cfg_attr(not(feature = "mini"), derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct ThemeStyleToken {
    /// Optional background override.
    pub background: Option<Color>,
    /// Optional foreground/text override.
    pub foreground: Option<Color>,
    /// Optional border color override.
    pub border: Option<Color>,
    /// Optional border width override.
    pub border_width: Option<u32>,
    /// Optional corner radius override.
    pub radius: Option<u32>,
}
