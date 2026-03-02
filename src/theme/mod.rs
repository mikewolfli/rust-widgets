//! Theme system and runtime switching.

use crate::core::{Color, Font};
use crate::style::{Margin, Padding, Shadow, WidgetStyle};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;

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

/// Theme registry and active-theme resolver.
pub struct ThemeManager {
    /// Registered themes keyed by theme name.
    themes: HashMap<String, Theme>,
    /// Active theme name.
    current_theme: String,
}

impl ThemeManager {
    /// Creates a theme manager seeded with the default theme.
    pub fn new() -> Self {
        let default = Theme::default();
        let current_theme = default.name.clone();
        let mut themes = HashMap::new();
        themes.insert(default.name.clone(), default);
        Self {
            themes,
            current_theme,
        }
    }

    /// Loads and registers a theme from a JSON file path.
    pub fn load_theme(&mut self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path)?;
        let theme: Theme = serde_json::from_str(&content)?;
        self.themes.insert(theme.name.clone(), theme);
        Ok(())
    }

    /// Registers a theme in memory.
    pub fn register_theme(&mut self, theme: Theme) {
        self.themes.insert(theme.name.clone(), theme);
    }

    /// Selects active theme by name.
    pub fn set_theme(&mut self, name: &str) -> bool {
        if self.themes.contains_key(name) {
            self.current_theme = name.to_string();
            return true;
        }
        false
    }

    /// Returns currently active theme.
    pub fn current_theme(&self) -> Option<&Theme> {
        self.themes.get(&self.current_theme)
    }

    /// Returns a registered theme by name.
    pub fn get_theme(&self, name: &str) -> Option<&Theme> {
        self.themes.get(name)
    }

    /// Resolves a widget style for a class using current theme tokens.
    pub fn resolve_style(&self, class_name: &str) -> WidgetStyle {
        let Some(theme) = self.current_theme() else {
            return WidgetStyle::default();
        };

        let mut style = WidgetStyle::default();
        style.background_color = Some(theme.colors.background);
        style.text_color = Some(theme.colors.foreground);
        style.border_color = Some(theme.colors.secondary);
        style.border_width = theme.borders.width;
        style.border_radius = theme.borders.radius;
        style.padding = Padding::all(theme.spacing.medium);
        style.margin = Margin::all(theme.spacing.small);
        if theme.borders.shadow {
            style.shadow = Some(Shadow {
                x: 0,
                y: 2,
                blur: 6,
                color: Color::rgba(0, 0, 0, 60),
            });
        }

        if class_name == "button" {
            style.background_color = Some(theme.colors.primary);
            style.text_color = Some(Color::rgba(255, 255, 255, 255));
        }
        style
    }
}

impl Default for ThemeManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Default theme
impl Default for Theme {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            colors: Colors {
                background: Color { r: 240, g: 240, b: 240, a: 255 },
                foreground: Color { r: 0, g: 0, b: 0, a: 255 },
                primary: Color { r: 33, g: 150, b: 243, a: 255 },
                secondary: Color { r: 158, g: 158, b: 158, a: 255 },
                accent: Color { r: 255, g: 152, b: 0, a: 255 },
                error: Color { r: 244, g: 67, b: 54, a: 255 },
                warning: Color { r: 255, g: 193, b: 7, a: 255 },
                success: Color { r: 76, g: 175, b: 80, a: 255 },
                disabled: Color { r: 200, g: 200, b: 200, a: 255 },
            },
            fonts: Fonts {
                regular: Font {
                    family: "Arial".to_string(),
                    size: 14.0,
                    weight: Font::REGULAR_WEIGHT,
                    bold: false,
                    italic: false,
                },
                bold: Font {
                    family: "Arial".to_string(),
                    size: 14.0,
                    weight: Font::BOLD_WEIGHT,
                    bold: true,
                    italic: false,
                },
                italic: Font {
                    family: "Arial".to_string(),
                    size: 14.0,
                    weight: Font::REGULAR_WEIGHT,
                    bold: false,
                    italic: true,
                },
                monospace: Font {
                    family: "Courier New".to_string(),
                    size: 12.0,
                    weight: Font::REGULAR_WEIGHT,
                    bold: false,
                    italic: false,
                },
            },
            spacing: Spacing {
                small: 4,
                medium: 8,
                large: 16,
                extra_large: 24,
            },
            borders: Borders {
                width: 1,
                radius: 4,
                shadow: true,
            },
        }
    }
}
