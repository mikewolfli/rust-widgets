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

/// Native surface for self-drawn widgets (Linux/GTK).
///
/// Compiled out for `mini`/`embedded`: those profiles have no `widget::runtime`,
/// which this module is built on.
#[cfg(all(target_os = "linux", feature = "gtk-native", widgets_unstripped))]
pub(crate) mod canvas;

/// Native WebKitGTK web engine (Linux only).
///
/// Holds every `webkit2gtk` reference in the crate so `src/web/` can drive a real
/// browser engine without importing a platform crate — see principle #36.
#[cfg(all(target_os = "linux", feature = "webkit-engine", widgets_unstripped))]
pub(crate) mod webkit_engine;

pub use types::*;
