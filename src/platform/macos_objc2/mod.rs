// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! macOS objc2 migration preview backend (sub-module split).
//!
//! This backend creates no native controls: the library paints every `WidgetKind`
//! itself, so it supplies a window, a native view surface and menu/event plumbing,
//! and the `Platform` trait defaults cover control construction and control state
//! (BLUE15 #55/#56).
/// Native AppKit FFI wrappers — macOS/`objc2-macos` only (the file itself is
/// `#![cfg]`-gated, but the declaration must not resolve elsewhere).
#[cfg(all(target_os = "macos", feature = "macos"))]
pub mod native;
pub mod platform_impl;
#[cfg(test)]
pub mod tests;
pub mod types;

pub use types::*;
