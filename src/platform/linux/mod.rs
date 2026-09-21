// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Linux backend platform (sub-module split).
//!
//! This backend owns exactly what a host must supply: the GTK toplevel and its
//! event loop, the `gtk::DrawingArea` self-drawn surface (`canvas.rs`), the
//! `/proc` and `lpr`/`lp` fact probes, and clipboard/IME/accessibility
//! forwarding. It creates **no** native controls and **no** native menu: every
//! `WidgetKind` is painted by the library itself, and the menu is an in-process
//! model the host materialises through its own GTK widgets. The control
//! construction, menu construction and control-state methods therefore keep the
//! `Platform` trait defaults (BLUE15 #55/#56).
pub mod platform_impl;
#[cfg(test)]
pub mod tests;
pub mod types;

/// Absolute child placement for the Linux/GTK backend, on a `gtk::Overlay`.
///
/// Split out because the placement rule (position, size, no size-request propagation) is
/// shared by window creation, mounting and resizing, and having one implementation is what
/// keeps those three from disagreeing.
#[cfg(all(target_os = "linux", feature = "gtk-native"))]
pub(crate) mod overlay_place;

/// Native surface for self-drawn widgets (Linux/GTK).
///
/// Compiled out for `mini`/`embedded`: those profiles have no `widget::runtime`,
/// which this module is built on.
#[cfg(all(target_os = "linux", feature = "gtk-native", widgets_unstripped))]
pub(crate) mod canvas;

// The `webkit_engine` module lived here: 76 lines wrapping `webkit2gtk::WebView` behind
// a private trait, never added to a GTK container. Deleted by BLUE20 layer 5 (ruling W1,
// 2026-09-21) because it never displayed a page. See `src/platform/types.rs` where
// `NativeWebEngine` was, and `tools/check_web_engine_honest.sh`, which fails if it returns.

pub use types::*;
