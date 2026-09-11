//! Platform abstraction for desktop/embedded/mobile families.

// Platform backend implementations (one per target)
/// Android platform backend (state-driven, JNI bridge behind `android-jni`).
///
/// The widget state machine is platform-independent, so the module is compiled
/// on every host to keep its unit tests executable. All JNI touch points are
/// `#[cfg(feature = "android-jni")]`-gated internally; on other hosts the
/// backend runs in pure state mode.
pub mod android;
/// Android JNI bridge (native view creation via JNI, feature-gated).
#[cfg(feature = "android-jni")]
pub mod android_jni;
#[cfg(any(target_os = "ohos", feature = "harmony"))]
pub mod harmony;
/// iOS mobile backend (state-driven, UIKit bridge behind `ios-uikit-ffi`).
///
/// The widget state machine is platform-independent, so the module is compiled
/// on every host to keep its unit tests executable. All UIKit (`objc2`) touch
/// points are `#[cfg(feature = "ios-uikit-ffi")]`-gated internally, and that
/// feature transitively requires the iOS target (`ios = ["dep:objc2", ...]`);
/// on other hosts the backend runs in pure state mode.
pub mod ios;
#[cfg(any(target_os = "linux", doc))]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;
/// macOS objc2 migration preview backend (state-driven).
///
/// The widget state machine is platform-independent, so the module is compiled
/// on every host to keep its unit tests executable. The real AppKit FFI lives
/// in the `native` sub-module, which is `#[cfg(all(target_os = "macos",
/// feature = "objc2-macos"))]`-gated; elsewhere the backend runs in pure state
/// mode. See BLUE14 D-1 for the precedent (`ime_windows`).
#[cfg(any(feature = "macos", feature = "objc2-macos"))]
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
///
/// The composition/marked-text state machine is platform-independent, so the
/// module is compiled on every host to keep its unit tests executable. All
/// AppKit (`objc2` `msg_send!`) touch points are `#[cfg(all(target_os =
/// "macos", feature = "objc2-macos"))]`-gated internally; on other hosts the
/// bridge runs in pure state-machine mode.
pub mod ime_macos;
/// Platform-specific IME stubs (macOS, Windows).
pub mod ime_stubs;
/// Real Windows IME bridge (TSF integration).
///
/// The composition/marked-text state machine is platform-independent, so the
/// module is compiled on every host to keep its unit tests executable. All TSF
/// (`msctf.dll`/`winapi`) touch points are `#[cfg(target_os = "windows")]`-gated
/// internally; on other hosts `TsfThreadMgr::try_create` reports TSF as
/// unavailable and the bridge runs in pure state-machine mode.
pub mod ime_windows;
pub(crate) mod runtime;
pub mod state;
mod stub;
pub mod types;
/// Pure Win32 notification-code semantics (host-compilable, no OS calls).
pub mod windows_notify;

#[cfg(all(test, not(feature = "mini")))]
mod tests;
/// Virtual keyboard controller for touch text input (BLUE8 P4-7).
pub mod virtual_keyboard;

// Re-exports: everything that was previously defined directly in mod.rs
pub use crate::platform::contract::{negotiate_capability_contract, CapabilityContract};
pub use crate::platform::contract::{EmbeddedCapabilityContract, NativeCapabilityContract};
pub use crate::platform::runtime::RuntimeGuiMode;
#[cfg(not(feature = "mini"))]
pub use crate::platform::runtime::{backend_name, capabilities, get_platform, init, quit, run};
#[cfg(not(feature = "mini"))]
pub use crate::platform::runtime::{dpi_scale_factor, runtime_gui_mode, runtime_gui_mode_for};
#[cfg(feature = "mobile-api")]
pub use crate::platform::runtime::{mobile_attach_to_native_view, mobile_backend_name};
pub use crate::platform::stub::StubPlatform;
pub use crate::platform::types::*;

pub mod a11y_wiring;

#[cfg(not(feature = "mini"))]
pub use a11y_wiring::wire_focus_manager_to_a11y;
