//! macOS platform backend implementation using Cocoa.

pub mod types;
mod platform_impl;

pub use crate::platform::macos::types::*;

#[cfg(test)]
mod tests;
