//! Platform capability contract negotiation.
//!
//! Determines which capabilities are available at runtime based on the
//! active platform backend and the selected `RuntimeProfile` (Full, Embedded, etc.).
//! Falls back to sensible defaults when a backend does not publish a contract.

use crate::core::RuntimeProfile;
#[cfg(not(feature = "mini"))]
use crate::platform::runtime::get_platform;
pub use crate::platform::types::*;

fn fallback_native_capability_contract() -> NativeCapabilityContract {
    NativeCapabilityContract {
        dpi_scaling: true,
        ime: true,
        accessibility: true,
        native_menu: true,
        typed_widget_trigger: true,
    }
}
fn fallback_embedded_capability_contract() -> EmbeddedCapabilityContract {
    EmbeddedCapabilityContract {
        fixed_dpi: true,
        low_memory_mode: true,
        typed_widget_trigger: true,
    }
}
/// Negotiate capabilities using profile-specific contracts with deterministic fallbacks.
#[cfg(not(feature = "mini"))]
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
#[cfg(feature = "mini")]
pub fn negotiate_capability_contract(_profile: RuntimeProfile) -> CapabilityContract {
    CapabilityContract::Native(fallback_native_capability_contract())
}
