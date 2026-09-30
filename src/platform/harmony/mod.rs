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

pub use types::*;
#[cfg(test)]
pub mod tests;
