//! macOS platform backend implementation using Cocoa (legacy).
//!
//! This module is compiled only when the `cocoa-legacy` feature is enabled.
//! The default objc2 backend lives in `src/platform/macos_objc2/`.

pub mod macos_bridge;

/// Accelerator parsing shared by the objc2 and cocoa macOS backends.
///
/// Ungated on purpose: both backends need it, so it must not depend on
/// `cocoa-legacy` (see the module docs for the failure that caused).
#[cfg(target_os = "macos")]
pub(crate) mod accelerator;

/// Cocoa 0.24 platform implementation (legacy, behind `cocoa-legacy` feature).
#[cfg(feature = "cocoa-legacy")]
mod platform_impl;

/// CoreGraphics FFI used to blit self-drawn frames.
///
/// Gated with `canvas.rs`, its only consumer.
#[cfg(all(feature = "cocoa-legacy", not(any(feature = "mini", feature = "embedded"))))]
pub(crate) mod cg;

/// Native surface for self-drawn widgets.
///
/// Compiled out for `mini`/`embedded`: those profiles have no `widget::runtime`
/// (see `src/widget/mod.rs`), and this module is built on it.
#[cfg(all(feature = "cocoa-legacy", not(any(feature = "mini", feature = "embedded"))))]
pub(crate) mod canvas;

/// Cocoa 0.24 types and helpers (legacy, behind `cocoa-legacy` feature).
#[cfg(feature = "cocoa-legacy")]
pub mod types;

#[cfg(feature = "cocoa-legacy")]
pub use crate::platform::macos::types::*;

#[cfg(all(test, feature = "cocoa-legacy"))]
mod tests;
