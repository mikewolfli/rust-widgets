//! Global stylesheet manager for CSS-based widget theming (BLUE13 R1.6).
//!
//! StyleSheetManager holds a global parsed stylesheet and applies matching
//! CSS rules to widget styles. This enables app-wide theming similar to
//! LVGL's `lv_theme_t` but using Rust's type-safe CSS parser.

use crate::style::css::CssParser;
use crate::style::PseudoState;
use crate::style::WidgetStyle;

use crate::compat::Mutex;
use crate::compat::MutexGuard;
use std::sync::OnceLock;

/// A registered stylesheet with its raw CSS text and priority.
struct RegisteredSheet {
    /// Name for debugging.
    name: String,
    /// Raw CSS text.
    css: String,
    /// Priority (higher overrides lower).
    priority: u8,
}

/// Global stylesheet manager for CSS-based theming.
///
/// Manages a set of named CSS stylesheets with associated priorities.
/// When applying styles, sheets are processed in ascending priority order
/// so that higher-priority sheets can override lower-priority ones.
pub struct StyleSheetManager {
    sheets: Vec<RegisteredSheet>,
}

impl StyleSheetManager {
    /// Create a new empty manager.
    pub fn new() -> Self {
        Self { sheets: Vec::new() }
    }

    /// Register a CSS stylesheet with a name and optional priority.
    /// Higher priority sheets override lower ones when rules conflict.
    pub fn register(&mut self, name: &str, css: &str, priority: u8) {
        // Remove existing sheet with the same name, then add the new one.
        self.sheets.retain(|s| s.name != name);
        self.sheets.push(RegisteredSheet {
            name: name.to_string(),
            css: css.to_string(),
            priority,
        });
    }

    /// Remove a registered stylesheet by name.
    /// Returns `true` if a sheet was removed.
    pub fn unregister(&mut self, name: &str) -> bool {
        let len_before = self.sheets.len();
        self.sheets.retain(|s| s.name != name);
        self.sheets.len() < len_before
    }

    /// Apply all matching stylesheet rules to a widget style.
    ///
    /// Rules are applied in priority order (lower first), so higher-priority
    /// sheets override lower ones via the `style.merge()` mechanism.
    pub fn apply_to(
        &self,
        kind: &str,
        class: Option<&str>,
        id: Option<&str>,
        state: Option<PseudoState>,
        style: &mut WidgetStyle,
    ) -> Result<(), String> {
        // Sort sheets by priority (ascending) so lower priority applied first.
        let mut sorted: Vec<&RegisteredSheet> = self.sheets.iter().collect();
        sorted.sort_by_key(|s| s.priority);

        for sheet in &sorted {
            // Parse and apply directly to the target style in priority order.
            // Since `parse_and_apply` sets values directly, processing lower-
            // priority sheets first lets higher-priority sheets override them.
            CssParser::parse_and_apply(&sheet.css, kind, class, id, state, style)?;
        }

        Ok(())
    }

    /// Clear all registered stylesheets.
    pub fn clear(&mut self) {
        self.sheets.clear();
    }

    /// Returns the number of registered sheets.
    pub fn len(&self) -> usize {
        self.sheets.len()
    }

    /// Returns true if no sheets are registered.
    pub fn is_empty(&self) -> bool {
        self.sheets.is_empty()
    }
}

impl Default for StyleSheetManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Get the global stylesheet manager instance (thread-safe).
///
/// This lazily initializes a singleton `StyleSheetManager` behind a `Mutex`,
/// allowing safe access from multiple threads.
pub fn global_stylesheet_manager() -> MutexGuard<'static, StyleSheetManager> {
    static MANAGER: OnceLock<Mutex<StyleSheetManager>> = OnceLock::new();
    MANAGER
        .get_or_init(|| Mutex::new(StyleSheetManager::new()))
        .lock()
        .expect("StyleSheetManager mutex poisoned")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Color;

    /// Helper: create a basic WidgetStyle with known values for testing.
    fn make_style() -> WidgetStyle {
        WidgetStyle::default()
    }

    #[test]
    fn stylesheet_register_and_apply() {
        let mut manager = StyleSheetManager::new();
        let css = r#"
            Button {
                background-color: #ff0000;
                text-color: #ffffff;
            }
        "#;
        manager.register("test-sheet", css, 0);

        let mut style = make_style();
        manager.apply_to("Button", None, None, None, &mut style).expect("apply_to should succeed");

        assert_eq!(
            style.background_color,
            Some(Color::from_rgb(255, 0, 0)),
            "background_color should be red"
        );
        assert_eq!(
            style.text_color,
            Some(Color::from_rgb(255, 255, 255)),
            "text_color should be white"
        );
    }

    #[test]
    fn stylesheet_priority() {
        let mut manager = StyleSheetManager::new();

        // Low priority: red background
        let css_low = r#"
            Button {
                background-color: #ff0000;
                text-color: #000000;
            }
        "#;
        manager.register("low", css_low, 0);

        // High priority: blue background (should override red)
        let css_high = r#"
            Button {
                background-color: #0000ff;
            }
        "#;
        manager.register("high", css_high, 100);

        let mut style = make_style();
        manager.apply_to("Button", None, None, None, &mut style).expect("apply_to should succeed");

        // background-color comes from high-priority sheet (blue overrides red)
        assert_eq!(
            style.background_color,
            Some(Color::from_rgb(0, 0, 255)),
            "high-priority blue should override low-priority red"
        );
        // text_color comes from low-priority sheet (high didn't set it)
        assert_eq!(
            style.text_color,
            Some(Color::from_rgb(0, 0, 0)),
            "text_color from low-priority sheet should be preserved"
        );
    }

    #[test]
    fn stylesheet_unregister() {
        let mut manager = StyleSheetManager::new();
        manager.register("theme", "Button { background-color: #00ff00; }", 0);
        assert!(!manager.is_empty(), "manager should not be empty after register");

        let removed = manager.unregister("theme");
        assert!(removed, "unregister should return true when sheet exists");
        assert!(manager.is_empty(), "manager should be empty after unregister");

        // Removing a non-existent sheet returns false.
        let removed_again = manager.unregister("nonexistent");
        assert!(!removed_again, "unregister should return false for unknown name");
    }

    #[test]
    fn stylesheet_clear() {
        let mut manager = StyleSheetManager::new();
        manager.register("a", "Button { background-color: #ff0000; }", 0);
        manager.register("b", "Label { text-color: #00ff00; }", 1);
        assert_eq!(manager.len(), 2, "should have 2 sheets");

        manager.clear();
        assert_eq!(manager.len(), 0, "should be empty after clear");
        assert!(manager.is_empty(), "is_empty should be true");
    }

    #[test]
    fn global_manager_thread_safe() {
        // Get the global manager and verify it works.
        {
            let mut mgr = global_stylesheet_manager();
            mgr.register("global-test", "Button { border-color: #ff00ff; }", 0);
        }

        // Verify the sheet is accessible (unregister to clean up).
        {
            let mut mgr = global_stylesheet_manager();
            let removed = mgr.unregister("global-test");
            assert!(removed, "should find the sheet registered in previous scope");
            assert!(mgr.is_empty(), "manager should be clean after unregister");
        }
    }

    #[test]
    fn stylesheet_register_replaces_existing() {
        let mut manager = StyleSheetManager::new();
        manager.register("sheet", "Button { background-color: #ff0000; }", 0);
        manager.register("sheet", "Button { background-color: #00ff00; }", 0);

        assert_eq!(manager.len(), 1, "re-registering same name should replace, not add");

        let mut style = make_style();
        manager.apply_to("Button", None, None, None, &mut style).expect("apply_to should succeed");

        assert_eq!(
            style.background_color,
            Some(Color::from_rgb(0, 255, 0)),
            "should use the latest registered CSS for the same name"
        );
    }

    #[test]
    fn stylesheet_class_selector_match() {
        let mut manager = StyleSheetManager::new();
        let css = r#"
            .primary {
                background-color: #ff8800;
            }
        "#;
        manager.register("class-test", css, 0);

        let mut style = make_style();
        manager
            .apply_to("Button", Some("primary"), None, None, &mut style)
            .expect("apply_to should succeed");

        assert_eq!(
            style.background_color,
            Some(Color::from_rgb(0xff, 0x88, 0x00)),
            "class selector .primary should match"
        );
    }

    #[test]
    fn stylesheet_no_match_does_not_apply() {
        let mut manager = StyleSheetManager::new();
        let css = r#"
            Label {
                background-color: #ff0000;
            }
        "#;
        manager.register("no-match", css, 0);

        let mut style = make_style();
        manager.apply_to("Button", None, None, None, &mut style).expect("apply_to should succeed");

        assert_eq!(style.background_color, None, "Label CSS should not apply to Button");
    }

    #[test]
    fn stylesheet_invalid_css_returns_error() {
        let mut manager = StyleSheetManager::new();
        // Missing closing brace — invalid CSS.
        manager.register("bad", "Button { background-color: #ff0000; ", 0);

        let mut style = make_style();
        let result = manager.apply_to("Button", None, None, None, &mut style);
        assert!(result.is_err(), "invalid CSS should produce an error");
    }
}
