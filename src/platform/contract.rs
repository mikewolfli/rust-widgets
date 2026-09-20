// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Platform capability contract negotiation.
//!
//! Determines which capabilities are available at runtime based on the
//! active platform backend and the selected `RuntimeProfile` (Full, Embedded, etc.).
//! Falls back to the backend's *family-derived* defaults when a backend does not
//! publish a contract of its own.

use crate::core::RuntimeProfile;
#[cfg(not(alloc_frugal))]
use crate::platform::runtime::get_platform;
pub use crate::platform::types::*;

/// The contract a `Full`-profile backend gets when it publishes none.
///
/// # Why this is derived rather than hardcoded
///
/// It used to be a literal with all four flags `true`. `native_capability_contract()`
/// returns `None` for every non-`Desktop` family, so the only way to reach this
/// function was from a **non-desktop** backend — and the answer it gave that backend
/// was "you have DPI scaling, IME, accessibility and a native menu". In other words
/// the fallback was not a default, it was a claim, and it inverted the very check it
/// was backing up.
///
/// The family is the honest source: it is what `Platform::capabilities()`'s own
/// default keys off, and it is the one piece of platform knowledge a backend always
/// reports. Deriving from it makes this fallback behave exactly like an untouched
/// backend's default, which is the only defensible answer for "this backend told us
/// nothing".
#[cfg(not(alloc_frugal))]
fn fallback_native_capability_contract() -> NativeCapabilityContract {
    default_capabilities_for(get_platform().family())
}

/// The contract a `mini` build gets, where no backend is selectable at runtime.
///
/// `mini` is the `alloc_frugal` profile: a software surface with no OS controls, which
/// is [`PlatformFamily::Embedded`] by construction (see `platform/portable`). Deriving
/// rather than hardcoding keeps this consistent with the runtime path above.
#[cfg(alloc_frugal)]
fn fallback_native_capability_contract() -> NativeCapabilityContract {
    crate::platform::types::default_capabilities_for(crate::core::PlatformFamily::Embedded)
}
/// Fallback contract for embedded profiles without a published backend contract.
#[cfg(not(alloc_frugal))]
fn fallback_embedded_capability_contract() -> EmbeddedCapabilityContract {
    EmbeddedCapabilityContract {
        fixed_dpi: true,
        low_memory_mode: true,
        typed_widget_trigger: true,
    }
}
/// Negotiate capabilities using profile-specific contracts with deterministic fallbacks.
#[cfg(not(alloc_frugal))]
pub fn negotiate_capability_contract(profile: RuntimeProfile) -> CapabilityContract {
    match profile {
        RuntimeProfile::Full => get_platform()
            .native_capability_contract()
            .map(CapabilityContract::Native)
            .unwrap_or(CapabilityContract::Native(fallback_native_capability_contract())),
        RuntimeProfile::Embedded => get_platform()
            .embedded_capability_contract()
            .map(CapabilityContract::Embedded)
            .unwrap_or(CapabilityContract::Embedded(fallback_embedded_capability_contract())),
    }
}
/// Capability negotiation not available in mini mode.
#[cfg(alloc_frugal)]
pub fn negotiate_capability_contract(_profile: RuntimeProfile) -> CapabilityContract {
    CapabilityContract::Native(fallback_native_capability_contract())
}
