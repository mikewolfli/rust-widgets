// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! iOS mobile platform backend (state-driven).

#[cfg(all(target_os = "ios", feature = "ios-uikit-ffi"))]
pub mod native;
/// The `Platform` implementation for iOS.
///
/// Gated to Apple targets because its memory, power and process probes come from
/// [`crate::platform::darwin_probes`], which is itself Apple-only (it needs
/// `libc`).
#[cfg(target_vendor = "apple")]
pub mod platform_impl;
/// The state container the trait impl translates into.
///
/// Gated with `platform_impl` rather than left open: the container has no reader
/// once the trait impl is out (`BackendState` is only touched from
/// `platform_impl`), so compiling it unconditionally produced five
/// "never constructed"/"never read" warnings on every non-Apple target.
#[cfg(target_vendor = "apple")]
pub mod types;

#[cfg(target_vendor = "apple")]
pub use types::IosMobilePlatform;
