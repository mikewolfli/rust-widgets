// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

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
/// AppKit's own file picker, behind [`Platform::choose_path`](crate::platform::Platform::choose_path).
///
/// # Why this is its own file
///
/// The body is AppKit message sends, and a non-Apple host cannot compile this crate for
/// `x86_64-apple-darwin` (a C dependency in the graph needs an Apple toolchain). Keeping the body
/// free of crate-internal names — it depends on `objc`/`cocoa` and on the two types
/// `super::super::{PathPurpose, PathPickerError}` and nothing else — is what lets the same file be
/// compiled on its own for the target (see `docs/log/log-20261014-1.md`, probe P-2), so the code
/// is verified by a compiler rather than by review.
///
/// # Which panel, and why one function covers all three purposes
///
/// `NSSavePanel` is the base class and the one that owns the modal loop and the path answer;
/// `NSOpenPanel` is its subclass, which adds "can choose files / directories / many". So "open a
/// file" is an `NSOpenPanel` with files enabled, "choose a directory" is the same panel with
/// directories enabled and files off, and "save a file" is the base `NSSavePanel` — the three
/// purposes differ only in those flags.
#[cfg(feature = "cocoa-legacy")]
pub(crate) mod file_picker;

/// Cocoa 0.24 platform implementation (legacy, behind `cocoa-legacy` feature).
#[cfg(feature = "cocoa-legacy")]
mod platform_impl;

/// CoreGraphics FFI used to blit self-drawn frames.
///
/// Gated with `canvas.rs`, its only consumer.
#[cfg(all(feature = "cocoa-legacy", widgets_unstripped))]
pub(crate) mod cg;

/// Native surface for self-drawn widgets.
///
/// Compiled out for `mini`/`embedded`: those profiles have no `widget::runtime`
/// (see `src/widget/mod.rs`), and this module is built on it.
#[cfg(all(feature = "cocoa-legacy", widgets_unstripped))]
pub(crate) mod canvas;

/// Cocoa 0.24 types and helpers (legacy, behind `cocoa-legacy` feature).
#[cfg(feature = "cocoa-legacy")]
pub mod types;

#[cfg(feature = "cocoa-legacy")]
pub use crate::platform::macos::types::*;

#[cfg(all(test, feature = "cocoa-legacy"))]
mod tests;
