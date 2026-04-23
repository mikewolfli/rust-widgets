//! Platform abstraction for desktop/embedded/mobile families.

// Platform backend implementations (one per target)
pub mod harmony;
pub mod linux;
#[cfg(all(target_os = "macos", not(feature = "objc2-macos")))]
pub mod macos;
#[cfg(all(target_os = "macos", feature = "objc2-macos"))]
pub mod macos_objc2;
#[cfg(feature = "mobile-api")]
pub mod mobile;
#[cfg(target_os = "windows")]
pub mod windows;

// Internal sub-modules (split from monolithic mod.rs)
mod state;
mod stub;
pub mod types;
mod runtime;
mod contract;

// Re-exports: everything that was previously defined directly in mod.rs
pub use crate::platform::types::*;
pub use crate::platform::stub::StubPlatform;
pub use crate::platform::runtime::{get_platform, init, run, quit, capabilities};
pub use crate::platform::runtime::{runtime_gui_mode, runtime_gui_mode_for, dpi_scale_factor};
pub use crate::platform::runtime::{RuntimeGuiMode, mobile_backend_name, mobile_attach_to_native_view};
pub use crate::platform::contract::{negotiate_capability_contract, CapabilityContract};
pub use crate::platform::contract::{NativeCapabilityContract, EmbeddedCapabilityContract};

#[cfg(test)]
mod tests;
