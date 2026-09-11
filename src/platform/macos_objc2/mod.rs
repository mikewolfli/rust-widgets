//! macOS objc2 migration preview backend (sub-module split).
pub mod clipboard_dnd;
pub mod dialog_creation;
pub mod menu_impl;
/// Native AppKit FFI wrappers — macOS/`objc2-macos` only (the file itself is
/// `#![cfg]`-gated, but the declaration must not resolve elsewhere).
#[cfg(all(target_os = "macos", feature = "macos"))]
pub mod native;
pub mod platform_impl;
#[cfg(test)]
pub mod tests;
pub mod types;
pub mod widget_creation;
pub mod widget_state;

pub use types::*;
