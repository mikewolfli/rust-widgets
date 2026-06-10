//! Theme system and runtime switching.
mod manager;
mod types;
pub use manager::ThemeManager;
pub use types::{Borders, Colors, Fonts, Spacing, Theme, ThemeOverrides, ThemeStyleToken};

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn theme_default() {
        let manager = ThemeManager::default();
        let theme = manager.current_theme();
        assert!(theme.is_some());
        assert_eq!(theme.unwrap().name, "default");
    }

    #[test]
    fn theme_dark_exists() {
        let dark = Theme::dark();
        assert_eq!(dark.name, "dark");
        // Background should be very dark (near black)
        assert!(dark.colors.background.r < 30);
        assert!(dark.colors.background.g < 30);
        assert!(dark.colors.background.b < 30);
        // Foreground should be very light (near white)
        assert!(dark.colors.foreground.r > 200);
        assert!(dark.colors.foreground.g > 200);
        assert!(dark.colors.foreground.b > 200);
    }
}
