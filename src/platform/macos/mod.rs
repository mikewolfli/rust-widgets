//! macOS platform backend implementation using Cocoa.

mod platform_impl;
pub mod types;

pub use crate::platform::macos::types::*;

#[cfg(test)]
mod tests;
