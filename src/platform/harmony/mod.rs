// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Harmony desktop backend shell (sub-module split).
pub mod platform_impl;
pub mod types;

pub use types::*;
#[cfg(test)]
pub mod tests;
