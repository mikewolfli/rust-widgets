// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! WebView is now an alias for `WebEngineView`.
//!
//! All functionality has been merged into `WebEngineView`, which provides
//! a superset of signals and features including JavaScript, plugins, private
//! browsing, certificate errors, console messages, downloads, and more.

/// Re-export: `WebView` is an alias for `WebEngineView`.
///
/// This type alias preserves backward compatibility — existing code that
/// references `WebView` will transparently use `WebEngineView` instead.
pub use super::web_engine::WebEngineView as WebView;
