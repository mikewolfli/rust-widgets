use crate::core::RuntimeProfile;
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
