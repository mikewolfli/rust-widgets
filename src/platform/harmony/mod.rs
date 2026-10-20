// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! HarmonyOS backend shell (sub-module split).
pub mod platform_impl;
pub mod types;

/// The ArkUI **XComponent** bridge: a real surface and real input.
///
/// Gated on `feature = "xcomponent"` because it links `libace_ndk` and needs the OpenHarmony
/// SDK's headers; a build without them must still compile. See the module docs for what is
/// bound and why.
#[cfg(all(feature = "xcomponent", not(alloc_frugal)))]
pub mod xcomponent;

/// The accessibility bridge over the XComponent's `ArkUI_AccessibilityProvider`.
///
/// Gated exactly like [`xcomponent`], because the provider is only reachable through a
/// bound `OH_NativeXComponent`: without the bridge there is no component, so there is no
/// provider to attach to.
#[cfg(all(feature = "xcomponent", not(alloc_frugal)))]
pub mod accessibility;

/// The system pasteboard (`OH_Pasteboard` over UDMF).
///
/// Compiled wherever this backend is — an OpenHarmony target, or the `harmony` preview feature on
/// a host. The FFI itself is `#[cfg(target_env = "ohos")]` inside, so a host build compiles the
/// module (and its unit tests) with the declarations replaced by an inert stand-in rather than
/// failing to link a device library. See the module docs.
pub mod pasteboard;

/// Display enumeration (`OH_NativeDisplayManager`).
///
/// Compiled wherever this backend is; the FFI is `#[cfg(target_env = "ohos")]` inside, so a host
/// build compiles the module with an inert stand-in instead of linking a device library. See the
/// module docs.
pub mod displays;

/// Host-reported environment facts (theme, font scale, safe area).
///
/// The ArkTS host calls the `rw_harmony_set_*` entry points from its own lifecycle; see the
/// module docs for why these are host-pushed rather than synchronous SDK queries.
///
/// Compiled in every profile, including `mini`: the facts are plain atomics plus the shared
/// IME state machine, and [`platform_impl`] calls them unconditionally. Gating the module on
/// `not(alloc_frugal)` made the backend reference a module that did not exist there
/// (`mini,harmony`: five `cannot find host_facts in super` errors).
pub mod host_facts;

pub use types::*;
#[cfg(test)]
pub mod tests;
