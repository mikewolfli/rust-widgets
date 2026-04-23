//! macOS objc2 migration preview backend (sub-module split).
pub mod types;
pub mod platform_impl;
#[cfg(test)]
pub mod tests;

pub use types::*;
