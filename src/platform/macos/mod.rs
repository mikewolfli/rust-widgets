//! macOS platform backend implementation using Cocoa.

pub mod macos_bridge;
mod platform_impl;
pub mod types;

pub use crate::platform::macos::types::*;

#[cfg(test)]
mod tests;
