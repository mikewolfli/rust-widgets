//! Style system primitives.
pub mod animation;
pub mod animation_group;
pub mod css;
pub mod css_watcher;
pub mod gradient;
pub mod primitives;
pub mod selector;
pub mod stylesheet;
pub mod theme;
pub mod theme_state;

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
pub use primitives::*;
pub use selector::*;
pub use stylesheet::*;
pub use theme::*;
pub use theme_state::*;
