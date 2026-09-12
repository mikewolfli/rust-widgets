//! macOS platform backend implementation using Cocoa (legacy).
//!
//! This module is compiled only when the `cocoa-legacy` feature is enabled.
//! The default objc2 backend lives in `src/platform/macos_objc2/`.

pub mod macos_bridge;

/// Cocoa 0.24 platform implementation (legacy, behind `cocoa-legacy` feature).
#[cfg(feature = "cocoa-legacy")]
mod platform_impl;

/// CoreGraphics FFI used to blit self-drawn frames.
#[cfg(feature = "cocoa-legacy")]
pub(crate) mod cg;

/// Native surface for self-drawn widgets.
#[cfg(feature = "cocoa-legacy")]
pub(crate) mod canvas;

/// Cocoa 0.24 types and helpers (legacy, behind `cocoa-legacy` feature).
#[cfg(feature = "cocoa-legacy")]
pub mod types;

#[cfg(feature = "cocoa-legacy")]
pub use crate::platform::macos::types::*;

#[cfg(all(test, feature = "cocoa-legacy"))]
mod tests;
