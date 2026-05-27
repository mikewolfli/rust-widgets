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
}
