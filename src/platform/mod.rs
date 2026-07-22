//! Platform abstraction for desktop/embedded/mobile families.

// Platform backend implementations (one per target)
/// Android platform backend (state-driven).
#[cfg(target_os = "android")]
pub mod android;
/// Android JNI bridge (native view creation via JNI, feature-gated).
#[cfg(feature = "android-jni")]
pub mod android_jni;
#[cfg(any(target_os = "ohos", feature = "harmony"))]
pub mod harmony;
#[cfg(target_os = "ios")]
pub mod ios;
#[cfg(any(target_os = "linux", doc))]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(all(target_os = "macos", any(feature = "macos", feature = "objc2-macos")))]
pub mod macos_objc2;
#[cfg(feature = "mobile-api")]
pub mod mobile;
#[cfg(all(target_os = "linux", feature = "wayland-native"))]
pub mod wayland;
#[cfg(target_os = "windows")]
pub mod windows;

/// WASM/WebAssembly platform backend (state-driven).
#[cfg(feature = "wasm")]
pub mod wasm;

/// Platform accessibility bridges (macOS, Windows, Linux).
pub mod accessibility;

// Internal sub-modules (split from monolithic mod.rs)
/// Rich clipboard content types and backend trait (BLUE10 R8.6).
pub mod clipboard;
/// Platform-specific rich clipboard stubs (BLUE10 R8.6).
pub mod clipboard_stubs;
mod contract;
/// Device class detection and adaptive layout support (BLUE8 P4-6).
pub mod detector;
/// Laser holographic keyboard detector (BLUE8 P4-5a, experimental).
#[cfg(feature = "holographic")]
pub mod holographic;
/// IME bridge trait, types, and mock implementation.
pub mod ime;
/// Real Linux IME bridge (IBus integration).
#[cfg(target_os = "linux")]
pub mod ime_linux;
/// Real macOS IME bridge (NSTextInputContext integration).
#[cfg(target_os = "macos")]
pub mod ime_macos;
/// Platform-specific IME stubs (macOS, Windows).
pub mod ime_stubs;
/// Real Windows IME bridge (TSF integration).
#[cfg(target_os = "windows")]
pub mod ime_windows;
pub(crate) mod runtime;
pub mod state;
mod stub;
pub mod types;
/// Virtual keyboard controller for touch text input (BLUE8 P4-7).
pub mod virtual_keyboard;

// Re-exports: everything that was previously defined directly in mod.rs
pub use crate::platform::contract::{negotiate_capability_contract, CapabilityContract};
pub use crate::platform::contract::{EmbeddedCapabilityContract, NativeCapabilityContract};
pub use crate::platform::runtime::RuntimeGuiMode;
#[cfg(not(feature = "mini"))]
pub use crate::platform::runtime::{capabilities, get_platform, init, quit, run};
#[cfg(not(feature = "mini"))]
pub use crate::platform::runtime::{dpi_scale_factor, runtime_gui_mode, runtime_gui_mode_for};
#[cfg(feature = "mobile-api")]
pub use crate::platform::runtime::{mobile_attach_to_native_view, mobile_backend_name};
pub use crate::platform::stub::StubPlatform;
pub use crate::platform::types::*;

pub mod a11y_wiring;

#[cfg(not(feature = "mini"))]
pub use a11y_wiring::wire_focus_manager_to_a11y;
