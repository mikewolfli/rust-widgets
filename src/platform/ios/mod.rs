// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! iOS mobile platform backend (state-driven).

#[cfg(all(target_os = "ios", feature = "ios-uikit-ffi"))]
pub mod native;
pub mod platform_impl;
pub mod types;

pub use types::IosMobilePlatform;
