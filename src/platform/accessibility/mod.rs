//! Platform accessibility bridges (macOS NSAccessibility, Windows UIAutomation, Linux AT-SPI).
//!
//! This module provides the foundation for OS-level accessibility integration.
//! Each platform backend can implement the `AccessibilityBridge` trait to expose
//! widget information to screen readers and other assistive technologies.
//!
//! Higher-level abstractions (`A11yProvider`, `A11yTree`) provide a unified
//! cross-platform accessibility node tree for screen reader navigation.

#[cfg(all(target_os = "macos", feature = "macos-legacy"))]
pub mod macos;

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "linux")]
pub mod linux;

pub mod types;

pub use types::*;
