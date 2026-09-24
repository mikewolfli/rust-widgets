// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! iOS mobile platform backend (state-driven).

#[cfg(all(target_os = "ios", feature = "ios-uikit-ffi"))]
pub mod native;
/// The `Platform` implementation for iOS.
///
/// **Compiled on every host**, with its three `darwin_probes` call sites individually
/// `target_vendor = "apple"`-gated. It used to carry the Apple gate itself, for the reason that its
/// memory/power/process probes need `libc` — but that also hid [`tests`], 9 assertions that use only
/// the `Platform` trait interface and touch no UIKit. A test that never compiles is a test that can
/// never fail, which is the host-invisible class `blue23.md` §0A.6 ITEM 7 exists to close.
///
/// Nothing is reachable on a non-Apple host: `platform/mod.rs` still gates the *selection* of this
/// backend, so the fallback branches in `platform_impl.rs` are never taken by real code.
pub mod platform_impl;
/// The state container the trait impl translates into.
///
/// Gated with `platform_impl` rather than left open: the container has no reader
/// once the trait impl is out (`BackendState` is only touched from
/// `platform_impl`), so compiling it unconditionally produced five
/// The state container the trait impl translates into.
///
/// The `"never constructed"/"never read"` warnings the old comment here mentions were a
/// consequence of `platform_impl` being Apple-gated: with no trait impl compiled, nothing read
/// this. Now that the impl is compiled everywhere, so is this.
pub mod types;

pub use types::IosMobilePlatform;
