// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Wayland backend platform (sub-module split).
pub mod platform_impl;
#[cfg(test)]
pub mod tests;
pub mod types;

pub use types::*;
