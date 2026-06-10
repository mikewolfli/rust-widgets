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
pub use crate::platform::runtime::{capabilities, get_platform, init, quit, run};
pub use crate::platform::runtime::{dpi_scale_factor, runtime_gui_mode, runtime_gui_mode_for};
#[cfg(feature = "mobile-api")]
pub use crate::platform::runtime::{mobile_attach_to_native_view, mobile_backend_name};
pub use crate::platform::stub::StubPlatform;
pub use crate::platform::types::*;

/// Wire a `FocusManager` to the platform's `AccessibilityBridge` if available.
///
/// When the platform has an accessibility bridge, this connects focus
/// changes to `notify_focus_changed` so screen readers can track focus.
/// This is a no-op when no bridge is available.
pub fn wire_focus_manager_to_a11y(fm: &mut crate::event::focus::FocusManager) {
    let platform = get_platform();
    if let Some(bridge) = platform.accessibility_bridge() {
        // SAFETY: The bridge reference is guaranteed to outlive the FocusManager
        // because both are owned by the application lifecycle which outlives
        // any widget tree. The raw pointer is only used within the callback
        // which fires synchronously during FocusManager operations.
        let bridge_ptr: *const dyn crate::platform::accessibility::AccessibilityBridge =
            bridge as *const dyn crate::platform::accessibility::AccessibilityBridge;
        fm.set_a11y_callback(Box::new(move |id| {
            let bridge = unsafe { &*bridge_ptr };
            bridge.notify_focus_changed(id);
        }));
    }
}

#[cfg(test)]
mod tests {
    use crate::event::focus::FocusManager;

    #[test]
    fn wire_focus_manager_to_a11y_no_panic_when_no_bridge() {
        // Verifies that wiring doesn't panic when no platform is initialized
        // (no bridge available -> should be a no-op).
        let mut fm = FocusManager::new();
        super::wire_focus_manager_to_a11y(&mut fm);
        // No assertion needed — the function should not panic
        assert!(fm.focused_widget().is_none());
    }
}
