// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

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
#[cfg(feature = "macos")]
pub use crate::platform::macos_objc2::MacOSObjc2Platform as SelectedMacOSPlatform;

/// Legacy fallback: cocoa 0.24 backend.
///
/// Reached only when the objc2 backend is *not* selected. The condition used to
/// repeat `feature = "macos"` twice (`any(macos, macos)`), so `--features
/// macos-legacy` on its own matched neither arm and `SelectedMacOSPlatform` did
/// not exist — a build that failed to compile rather than choosing the legacy
/// backend it had just asked for.
#[cfg(all(not(feature = "macos"), feature = "cocoa-legacy"))]
pub use crate::platform::macos::MacOSPlatform as SelectedMacOSPlatform;
