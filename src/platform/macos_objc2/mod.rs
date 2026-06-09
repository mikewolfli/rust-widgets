//! macOS objc2 migration preview backend (sub-module split).
pub mod native;
pub mod platform_impl;
#[cfg(test)]
pub mod tests;
pub mod types;

pub use types::*;
