use std::collections::HashMap;
use std::fs;

use crate::core::{Color, Font};
use crate::style::{Margin, Padding, Shadow, WidgetStyle};

use super::{Borders, Colors, Fonts, Spacing, Theme};

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

        let shadow = if theme.borders.shadow {
            Some(Shadow {
                x: 0,
                y: 2,
                blur: 6,
                color: Color::rgba(0, 0, 0, 60),
            })
        } else {
            None
        };

        let (background_color, text_color) = if class_name == "button" {
            (
                Some(theme.colors.primary),
                Some(Color::rgba(255, 255, 255, 255)),
            )
        } else {
            (Some(theme.colors.background), Some(theme.colors.foreground))
        };

        WidgetStyle {
            background_color,
            text_color,
            border_color: Some(theme.colors.secondary),
            border_width: theme.borders.width,
            border_radius: theme.borders.radius,
            padding: Padding::all(theme.spacing.medium),
            margin: Margin::all(theme.spacing.small),
            shadow,
            ..Default::default()
        }
    }
}

impl Default for ThemeManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            colors: Colors {
                background: Color {
                    r: 240,
                    g: 240,
                    b: 240,
                    a: 255,
                },
                foreground: Color {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 255,
                },
                primary: Color {
                    r: 33,
                    g: 150,
                    b: 243,
                    a: 255,
                },
                secondary: Color {
                    r: 158,
                    g: 158,
                    b: 158,
                    a: 255,
                },
                accent: Color {
                    r: 255,
                    g: 152,
                    b: 0,
                    a: 255,
                },
                error: Color {
                    r: 244,
                    g: 67,
                    b: 54,
                    a: 255,
                },
                warning: Color {
                    r: 255,
                    g: 193,
                    b: 7,
                    a: 255,
                },
                success: Color {
                    r: 76,
                    g: 175,
                    b: 80,
                    a: 255,
                },
                disabled: Color {
                    r: 200,
                    g: 200,
                    b: 200,
                    a: 255,
                },
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
