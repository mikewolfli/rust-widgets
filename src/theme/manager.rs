use super::{Borders, Colors, Fonts, Spacing, Theme, ThemeOverrides};
use crate::compat::HashMap;
use crate::core::{Color, Font};
use crate::signal::Signal;
use crate::style::{Margin, Padding, Shadow, WidgetStyle};

/// Theme registry and active-theme resolver.
pub struct ThemeManager {
    /// Registered themes keyed by theme name.
    themes: HashMap<String, Theme>,
    /// Active theme name.
    current_theme: String,
    /// Signal emitted when the active theme changes.
    theme_changed: Signal<()>,
}

impl ThemeManager {
    /// Creates a theme manager seeded with the default theme.
    pub fn new() -> Self {
        let default = Theme::default();
        let current_theme = default.name.clone();
        let mut themes = HashMap::new();
        themes.insert(default.name.clone(), default);
        Self { themes, current_theme, theme_changed: Signal::new() }
    }

    /// Loads and registers a theme from a JSON file path.
    #[cfg(not(feature = "mini"))]
    pub fn load_theme(&mut self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let theme: Theme = serde_json::from_str(&content)?;
        self.themes.insert(theme.name.clone(), theme);
        Ok(())
    }

    /// Serializes the current active theme to a JSON file at the given path.
    #[cfg(not(feature = "mini"))]
    pub fn save_theme(&self, path: &str) -> Result<(), String> {
        let theme = self.current_theme().ok_or_else(|| "No active theme to save".to_string())?;
        let json = serde_json::to_string_pretty(theme)
            .map_err(|e| format!("Failed to serialize theme: {}", e))?;
        std::fs::write(path, &json).map_err(|e| format!("Failed to write theme file: {}", e))?;
        Ok(())
    }

    /// Registers a theme in memory.
    pub fn register_theme(&mut self, theme: Theme) {
        self.themes.insert(theme.name.clone(), theme);
    }

    /// Selects active theme by name. Emits `theme_changed` on success.
    pub fn set_theme(&mut self, name: &str) -> bool {
        if self.themes.contains_key(name) {
            self.current_theme = name.to_string();
            self.theme_changed.emit(());
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

    /// Returns a reference to the `theme_changed` signal.
    ///
    /// Connect slots to this signal to be notified when the active theme is switched.
    pub fn on_theme_changed(&self) -> &Signal<()> {
        &self.theme_changed
    }

    /// Resolves a widget style for a class using current theme tokens.
    pub fn resolve_style(&self, class_name: &str) -> WidgetStyle {
        let Some(theme) = self.current_theme() else {
            return WidgetStyle::default();
        };
        let shadow = if theme.borders.shadow {
            Some(Shadow { x: 0, y: 2, blur: 6, color: Color::rgba(0, 0, 0, 60) })
        } else {
            None
        };
        let (background_color, text_color, border_color) = match class_name {
            "button" | "toggle" => (
                Some(theme.colors.primary),
                Some(Color::rgba(255, 255, 255, 255)),
                Some(theme.colors.primary),
            ),
            "label" => (Some(Color::rgba(0, 0, 0, 0)), Some(theme.colors.foreground), None),
            "input" | "lineedit" | "textedit" => (
                Some(Color::rgba(255, 255, 255, 255)),
                Some(theme.colors.foreground),
                Some(theme.colors.secondary),
            ),
            "slider" | "progress" => {
                (Some(theme.colors.accent), Some(Color::rgba(255, 255, 255, 255)), None)
            }
            "panel" | "window" | "dialog" => (
                Some(theme.colors.background),
                Some(theme.colors.foreground),
                Some(theme.colors.secondary),
            ),
            "checkbox" | "radio" => (
                Some(Color::rgba(255, 255, 255, 255)),
                Some(theme.colors.foreground),
                Some(theme.colors.secondary),
            ),
            _ => (
                Some(theme.colors.background),
                Some(theme.colors.foreground),
                Some(theme.colors.secondary),
            ),
        };

        // Apply class-level overrides if present in theme overrides
        let (final_bg, final_fg, final_border, final_border_width, final_radius) =
            if let Some(token) = theme.overrides.styles.get(class_name) {
                (
                    token.background.or(background_color),
                    token.foreground.or(text_color),
                    token.border.or(border_color),
                    token.border_width.unwrap_or(theme.borders.width),
                    token.radius.unwrap_or(theme.borders.radius),
                )
            } else {
                (
                    background_color,
                    text_color,
                    border_color,
                    theme.borders.width,
                    theme.borders.radius,
                )
            };

        WidgetStyle {
            background_color: final_bg,
            text_color: final_fg,
            border_color: final_border,
            border_width: final_border_width,
            border_radius: final_radius,
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
                background: Color { r: 240, g: 240, b: 240, a: 255 },
                foreground: Color { r: 0, g: 0, b: 0, a: 255 },
                primary: Color { r: 33, g: 150, b: 243, a: 255 },
                secondary: Color { r: 158, g: 158, b: 158, a: 255 },
                accent: Color { r: 255, g: 152, b: 0, a: 255 },
                error: Color { r: 244, g: 67, b: 54, a: 255 },
                warning: Color { r: 255, g: 193, b: 7, a: 255 },
                success: Color { r: 76, g: 175, b: 80, a: 255 },
                disabled: Color { r: 200, g: 200, b: 200, a: 255 },
                info: Color::INFO,
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
                caption: Font::simple("Arial", 11.0),
                body: Font::simple("Arial", 14.0),
                title: Font::bold("Arial", 16.0),
                headline: Font::bold("Arial", 20.0),
                display: Font::bold("Arial", 28.0),
            },
            spacing: Spacing { small: 4, medium: 8, large: 16, extra_large: 24 },
            borders: Borders { width: 1, radius: 4, shadow: true },
            overrides: ThemeOverrides { styles: HashMap::new() },
        }
    }
}

impl Theme {
    /// Creates a dark theme preset with Material Dark-inspired colors.
    ///
    /// This complements the light `Theme::default()` for dark/light mode switching.
    /// Fonts, spacing, and borders are identical to the default light theme.
    pub fn dark() -> Self {
        Self {
            name: "dark".to_string(),
            colors: Colors {
                background: Color { r: 18, g: 18, b: 18, a: 255 },
                foreground: Color { r: 225, g: 225, b: 225, a: 255 },
                primary: Color { r: 100, g: 181, b: 246, a: 255 },
                secondary: Color { r: 130, g: 130, b: 130, a: 255 },
                accent: Color { r: 255, g: 171, b: 64, a: 255 },
                error: Color { r: 239, g: 83, b: 80, a: 255 },
                warning: Color { r: 255, g: 213, b: 79, a: 255 },
                success: Color { r: 129, g: 199, b: 132, a: 255 },
                disabled: Color { r: 80, g: 80, b: 80, a: 255 },
                info: Color::INFO,
            },
            fonts: Fonts {
                regular: Font::simple("Arial", 14.0),
                bold: Font::bold("Arial", 14.0),
                italic: Font {
                    family: "Arial".to_string(),
                    size: 14.0,
                    weight: Font::REGULAR_WEIGHT,
                    bold: false,
                    italic: true,
                },
                monospace: Font::simple("Courier New", 12.0),
                caption: Font::simple("Arial", 11.0),
                body: Font::simple("Arial", 14.0),
                title: Font::bold("Arial", 16.0),
                headline: Font::bold("Arial", 20.0),
                display: Font::bold("Arial", 28.0),
            },
            spacing: Spacing { small: 4, medium: 8, large: 16, extra_large: 24 },
            borders: Borders { width: 1, radius: 4, shadow: true },
            overrides: ThemeOverrides { styles: HashMap::new() },
        }
    }
}
