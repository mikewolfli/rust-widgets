//! Linux backend platform (sub-module split).
pub mod menu_impl;
pub mod platform_impl;
#[cfg(test)]
pub mod tests;
pub mod types;
pub mod widget_creation;
pub mod widget_state;

/// Native surface for self-drawn widgets (Linux/GTK).
#[cfg(all(target_os = "linux", feature = "gtk-native"))]
pub(crate) mod canvas;

pub use types::*;
