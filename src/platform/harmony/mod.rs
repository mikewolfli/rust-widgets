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

pub use types::*;
#[cfg(test)]
pub mod tests;
