//! Platform abstraction for desktop/embedded/mobile families.

// Platform backend implementations (one per target)
pub mod harmony;
#[cfg(target_os = "ios")]
pub mod ios;
pub mod linux;
#[cfg(all(target_os = "macos", not(feature = "objc2-macos")))]
pub mod macos;
#[cfg(all(target_os = "macos", feature = "objc2-macos"))]
pub mod macos_objc2;
#[cfg(feature = "mobile-api")]
pub mod mobile;
#[cfg(all(target_os = "linux", feature = "wayland-native"))]
pub mod wayland;
#[cfg(target_os = "windows")]
pub mod windows;

/// Platform accessibility bridges (macOS, Windows, Linux).
pub mod accessibility;

// Internal sub-modules (split from monolithic mod.rs)
mod contract;
/// Device class detection and adaptive layout support (BLUE8 P4-6).
pub mod detector;
/// Laser holographic keyboard detector (BLUE8 P4-5a, experimental).
#[cfg(feature = "holographic")]
pub mod holographic;
mod runtime;
pub mod state;
mod stub;
pub mod types;
/// Virtual keyboard controller for touch text input (BLUE8 P4-7).
pub mod virtual_keyboard;

// Re-exports: everything that was previously defined directly in mod.rs
pub use crate::platform::contract::{negotiate_capability_contract, CapabilityContract};
pub use crate::platform::contract::{EmbeddedCapabilityContract, NativeCapabilityContract};
pub use crate::platform::runtime::RuntimeGuiMode;
pub use crate::platform::runtime::{capabilities, get_platform, init, quit, run};
pub use crate::platform::runtime::{dpi_scale_factor, runtime_gui_mode, runtime_gui_mode_for};
#[cfg(feature = "mobile-api")]
pub use crate::platform::runtime::{mobile_attach_to_native_view, mobile_backend_name};
pub use crate::platform::stub::StubPlatform;
pub use crate::platform::types::*;

#[cfg(test)]
mod tests;
