//! Global shortcut system for menu items and actions.
//!
//! This module provides keyboard shortcut registration, conflict detection,
//! and dispatching for menu items across the application.
mod manager;
mod types;
#[cfg(test)]
mod tests;
pub use manager::ShortcutManager;
pub use types::{Key, Modifiers, Shortcut, ShortcutEntry};
