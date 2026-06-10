//! Bridge that selects between objc2 and cocoa backend (BLUE11 R1.5, R2.5).
//! Priority: objc2 (default, via objc2-macos feature) → cocoa (cocoa-legacy fallback).
//!
//! This module provides a unified Platform implementation that delegates
//! to the appropriate backend based on feature flags.
//!
//! # Migration Status (R2.5)
//! - Default: objc2 backend (via `desktop` feature which includes `objc2-macos`)
//! - Legacy: cocoa 0.24 backend (via `cocoa-legacy` feature)

/// Default: objc2 backend (activated by `macos` feature or `objc2-macos` alias).
#[cfg(any(feature = "macos", feature = "objc2-macos"))]
pub use crate::platform::macos_objc2::MacOSObjc2Platform as SelectedMacOSPlatform;

/// Legacy fallback: cocoa 0.24 backend.
#[cfg(all(not(any(feature = "macos", feature = "objc2-macos")), feature = "cocoa-legacy"))]
pub use crate::platform::macos::MacOSPlatform as SelectedMacOSPlatform;
