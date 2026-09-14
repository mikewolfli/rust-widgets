// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Linux backend platform (sub-module split).
pub mod menu_impl;
pub mod platform_impl;
#[cfg(test)]
pub mod tests;
pub mod types;
pub mod widget_creation;
pub mod widget_state;

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
