// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Platform accessibility bridges (macOS NSAccessibility, Windows UIAutomation, Linux AT-SPI).
//!
//! This module provides the foundation for OS-level accessibility integration.
//! Each platform backend can implement the `AccessibilityBridge` trait to expose
//! widget information to screen readers and other assistive technologies.
//!
//! Higher-level abstractions (`A11yProvider`, `A11yTree`) provide a unified
//! cross-platform accessibility node tree for screen reader navigation.

#[cfg(all(target_os = "macos", feature = "cocoa-legacy"))]
pub mod macos;

/// The objc2 `NSAccessibility` bridge (BLUE28 B28.13).
///
/// Compiled with the `macos` feature on every host so its state tests run; the AppKit posting
/// call inside is `target_os = "macos"`-gated.
#[cfg(feature = "macos")]
pub mod macos_objc2;

/// The Win32 `NotifyWinEvent` bridge.
///
/// Compiled on every host: the state half is platform-neutral and its tests are part of the host
/// suite (B28.16); only the `NotifyWinEvent` call is `target_os = "windows"`-gated.
pub mod windows;

#[cfg(unix_desktop)]
pub mod linux;

/// Platform-neutral accessibility node/role/state types shared by the
/// per-OS bridges and the higher-level `A11yProvider` tree.
pub mod types;

pub use types::*;
