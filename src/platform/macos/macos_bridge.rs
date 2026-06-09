//! Bridge that selects between objc2 and cocoa backend (BLUE11 R1.5).
//! Priority: objc2 (when objc2-macos feature is enabled) → cocoa (legacy).
//!
//! This module provides a unified Platform implementation that delegates
//! to the appropriate backend based on feature flags.

#[cfg(feature = "objc2-macos")]
pub use crate::platform::macos_objc2::MacOSObjc2Platform as SelectedMacOSPlatform;

#[cfg(not(feature = "objc2-macos"))]
pub use crate::platform::macos::MacOSPlatform as SelectedMacOSPlatform;
