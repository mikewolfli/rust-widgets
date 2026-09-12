//! Global shortcut system for menu items and actions.
//!
//! This module provides keyboard shortcut registration, conflict detection,
//! and dispatching for menu items across the application.
mod manager;
#[cfg(test)]
mod tests;
mod types;
pub use manager::ShortcutManager;
pub use types::{
    format_shortcut_for_platform, Key, Modifiers, PlatformShortcutStyle, Shortcut, ShortcutEntry,
};
