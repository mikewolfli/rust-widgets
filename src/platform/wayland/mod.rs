//! Wayland backend platform (sub-module split).
pub mod platform_impl;
#[cfg(test)]
pub mod tests;
pub mod types;

pub use types::*;
