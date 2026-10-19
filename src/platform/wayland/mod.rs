// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Wayland backend platform (sub-module split).
#[cfg(all(feature = "wayland-native", unix_desktop))]
pub mod clipboard;
/// `text-input-v3` IME bridge (BLUE28 B28.12).
#[cfg(all(feature = "wayland-native", unix_desktop))]
pub mod ime;
#[cfg(all(feature = "wayland-native", unix_desktop))]
pub mod platform_impl;
#[cfg(all(feature = "wayland-portal", unix_desktop))]
pub mod portal;
#[cfg(test)]
pub mod tests;
pub mod types;

pub use types::*;
