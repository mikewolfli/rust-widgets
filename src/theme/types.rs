use crate::core::{Color, Font};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// High-level theme definition used by runtime style resolution.
#[derive(Debug, Serialize, Deserialize, Clone)]
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
}

/// Semantic color palette tokens.
#[derive(Debug, Serialize, Deserialize, Clone)]
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
}

/// Font token set used by theme consumers.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Fonts {
    /// Regular text font token.
    pub regular: Font,
    /// Bold text font token.
    pub bold: Font,
    /// Italic text font token.
    pub italic: Font,
    /// Monospace font token.
    pub monospace: Font,
}

/// Spacing scale tokens.
#[derive(Debug, Serialize, Deserialize, Clone)]
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
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Borders {
    /// Default border width.
    pub width: u32,
    /// Default corner radius.
    pub radius: u32,
    /// Whether drop shadows are enabled.
    pub shadow: bool,
}

/// Style override map used for class-level theme customization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeOverrides {
    /// Overrides keyed by style/class name.
    pub styles: HashMap<String, ThemeStyleToken>,
}

/// Optional style tokens used to override resolved widget styles.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
