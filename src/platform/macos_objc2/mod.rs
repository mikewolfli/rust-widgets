//! macOS objc2 migration preview backend (sub-module split).
pub mod clipboard_dnd;
pub mod dialog_creation;
pub mod menu_impl;
pub mod native;
pub mod platform_impl;
#[cfg(test)]
pub mod tests;
pub mod types;
pub mod widget_creation;
pub mod widget_state;

pub use types::*;
