// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Platform runtime initialization, backend selection, and lifecycle management.
//!
//! Provides `init()`, `run()`, `quit()`, `get_platform()`, and DPI scale factor
//! querying.  Platform backends are selected at compile time based on `target_os`
//! and feature flags, then cached in a global singleton.

#[cfg(not(alloc_frugal))]
use crate::compat::OnceLock;
#[cfg(all(target_os = "android", not(alloc_frugal), not(embedded_surface)))]
use crate::platform::android::AndroidPlatform;
#[cfg(all(
    any(target_os = "ohos", feature = "harmony"),
    not(target_os = "android"),
    not(alloc_frugal),
    not(embedded_surface)
))]
use crate::platform::harmony::HarmonyPlatform;
#[cfg(all(not(alloc_frugal), target_os = "ios", not(embedded_surface)))]
use crate::platform::ios::IosMobilePlatform;
#[cfg(all(
    target_os = "linux",
    not(alloc_frugal),
    not(embedded_surface),
    not(feature = "harmony")
))]
use crate::platform::linux::LinuxPlatform;
#[cfg(all(
    not(alloc_frugal),
    target_os = "macos",
    not(embedded_surface),
    any(feature = "macos", feature = "macos-legacy"),
    // Must mirror the `create_native_platform` gate below exactly: this import has
    // no other user, so if the two ever diverge the build warns about an unused
    // import (or fails outright).
    not(feature = "harmony")
))]
use crate::platform::macos::macos_bridge::SelectedMacOSPlatform;
#[cfg(all(not(embedded_surface), feature = "mobile-api"))]
use crate::platform::mobile;
#[cfg(not(alloc_frugal))]
pub use crate::platform::types::*;
#[cfg(all(
    target_os = "linux",
    not(embedded_surface),
    feature = "wayland-native",
    not(feature = "harmony")
))]
use crate::platform::wayland::WaylandPlatform;
#[cfg(all(
    not(alloc_frugal),
    target_os = "windows",
    not(embedded_surface),
    // Must mirror its only user, `create_native_platform`, exactly.
    not(feature = "harmony")
))]
use crate::platform::windows::WindowsPlatform;

// ---------------------------------------------------------------------------
// Linux runtime auto-detection: Wayland vs X11/GTK
// ---------------------------------------------------------------------------

/// Returns `true` when the process is running under a Wayland display server.
///
/// Detection strategy (tiered):
///  1. `$WAYLAND_DISPLAY` environment variable is set → Wayland
///  2. `$XDG_SESSION_TYPE` equals `"wayland"` → Wayland
///  3. Otherwise → assume X11/"plain" Linux
#[cfg(all(
    target_os = "linux",
    not(embedded_surface),
    feature = "wayland-native",
    not(feature = "harmony")
))]
fn is_wayland_session() -> bool {
    std::env::var("WAYLAND_DISPLAY").is_ok()
        || std::env::var("XDG_SESSION_TYPE")
            .map(|t| t.eq_ignore_ascii_case("wayland"))
            .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Platform constructor (auto-detect on Linux)
// ---------------------------------------------------------------------------

/// Embedded: stripped-down render-engine-only runtime.
#[cfg(all(not(alloc_frugal), embedded_surface))]
fn create_native_platform() -> Box<dyn Platform> {
    Box::new(crate::platform::stub::StubPlatform::new(
        "embedded-runtime-stub",
        crate::core::PlatformFamily::Embedded,
    ))
}

#[cfg(all(
    not(alloc_frugal),
    target_os = "windows",
    not(embedded_surface),
    // Mirrors the macOS/Linux/iOS arms: the `harmony` preview backend must win on
    // any host, otherwise `--features full` defines this function twice.
    not(feature = "harmony")
))]
fn create_native_platform() -> Box<dyn Platform> {
    Box::new(WindowsPlatform::new())
}

/// Select macOS backend via the bridge (BLUE11 R1.5).
/// The bridge dispatches to objc2 or cocoa based on feature flags.
///
/// The `harmony` feature excludes this arm (as it already does for the Linux
/// arms): enabling the Harmony preview backend must select `HarmonyPlatform`,
/// otherwise `--features full` (which turns on every OS feature) would define
/// `create_native_platform` twice on a macOS host.
#[cfg(all(
    not(alloc_frugal),
    target_os = "macos",
    not(embedded_surface),
    any(feature = "macos", feature = "macos-legacy"),
    not(feature = "harmony")
))]
fn create_native_platform() -> Box<dyn Platform> {
    Box::new(SelectedMacOSPlatform::new())
}

/// macOS fallback when no macos/macos-legacy backend feature is active.
#[cfg(all(
    not(alloc_frugal),
    target_os = "macos",
    not(embedded_surface),
    not(any(feature = "macos", feature = "macos-legacy")),
    not(feature = "harmony")
))]
fn create_native_platform() -> Box<dyn Platform> {
    Box::new(crate::platform::stub::StubPlatform::new(
        "macos-fallback-stub",
        crate::core::PlatformFamily::Desktop,
    ))
}

/// Linux runtime auto-detection:
///   - Wayland session → WaylandPlatform (when wayland-native feature enabled)
///   - Otherwise → LinuxPlatform (GTK or state-backed)
#[cfg(all(
    not(alloc_frugal),
    target_os = "linux",
    not(embedded_surface),
    feature = "wayland-native",
    not(feature = "harmony")
))]
fn create_native_platform() -> Box<dyn Platform> {
    if is_wayland_session() {
        Box::new(WaylandPlatform::new())
    } else {
        Box::new(LinuxPlatform::new())
    }
}

/// Linux without wayland-native feature → always use LinuxPlatform.
///
/// The `harmony` feature excludes this arm: enabling the Harmony preview
/// backend on a Linux host must select `HarmonyPlatform`, matching the
/// `any(target_os = "ohos", feature = "harmony")` arm below.
#[cfg(all(
    not(alloc_frugal),
    target_os = "linux",
    not(embedded_surface),
    not(feature = "wayland-native"),
    not(feature = "harmony")
))]
fn create_native_platform() -> Box<dyn Platform> {
    Box::new(LinuxPlatform::new())
}

/// Android platform backend (state-driven, optionally JNI-backed).
#[cfg(all(
    not(alloc_frugal),
    target_os = "android",
    not(embedded_surface),
    // Mirrors the HarmonyOS arm's own `not(target_os = "android")` exclusion, and
    // keeps the two mutually exclusive when the feature is set on an Android host.
    not(feature = "harmony")
))]
fn create_native_platform() -> Box<dyn Platform> {
    Box::new(AndroidPlatform::new())
}

/// HarmonyOS (OpenHarmony `ohos` target, or the `harmony` preview feature on
/// any host). Falls back to the state-backed Harmony backend.
#[cfg(all(
    any(target_os = "ohos", feature = "harmony"),
    not(target_os = "android"),
    not(alloc_frugal),
    not(embedded_surface)
))]
fn create_native_platform() -> Box<dyn Platform> {
    Box::new(HarmonyPlatform::new())
}

/// iOS state-backed platform backend.
#[cfg(all(not(alloc_frugal), target_os = "ios", not(embedded_surface), not(feature = "harmony")))]
fn create_native_platform() -> Box<dyn Platform> {
    Box::new(IosMobilePlatform::new())
}

/// WASM backend — used on `wasm32` targets when the `wasm` feature is enabled.
/// On wasm32 the event loop is driven by `request_animation_frame`; elsewhere
/// the same backend uses a polling fallback (used for development/testing).
#[cfg(all(not(alloc_frugal), not(embedded_surface), feature = "wasm", target_arch = "wasm32"))]
fn create_native_platform() -> Box<dyn Platform> {
    Box::new(crate::platform::wasm::WasmPlatform::default())
}

#[cfg(all(
    not(alloc_frugal),
    not(embedded_surface),
    not(all(feature = "wasm", target_arch = "wasm32")),
    not(target_os = "android"),
    not(any(target_os = "ohos", feature = "harmony")),
    not(any(target_os = "windows", target_os = "macos", target_os = "linux", target_os = "ios"))
))]
fn create_native_platform() -> Box<dyn Platform> {
    Box::new(crate::platform::stub::StubPlatform::new(
        "unknown-runtime-stub",
        crate::core::PlatformFamily::Desktop,
    ))
}

// ---------------------------------------------------------------------------
// Global platform singleton
// ---------------------------------------------------------------------------

#[cfg(not(alloc_frugal))]
static PLATFORM: OnceLock<Box<dyn Platform>> = OnceLock::new();

/// Returns the process-global platform backend instance.
#[cfg(not(alloc_frugal))]
pub fn get_platform() -> &'static dyn Platform {
    PLATFORM.get_or_init(create_native_platform).as_ref()
}

/// Initializes the platform backend.
#[cfg(not(alloc_frugal))]
pub fn init() {
    get_platform().init();
}

/// Runs the platform main loop.
#[cfg(not(alloc_frugal))]
pub fn run() {
    get_platform().run();
}

/// Requests platform main loop shutdown.
#[cfg(not(alloc_frugal))]
pub fn quit() {
    get_platform().quit();
}

/// Returns runtime capabilities for the active backend.
#[cfg(not(alloc_frugal))]
pub fn capabilities() -> PlatformCapabilities {
    get_platform().capabilities()
}

/// Returns the backend name of the active platform (e.g. `"gtk"`, `"cocoa"`,
/// `"WindowsPlatform"`, `"wasm-state-backend"`).
#[cfg(not(alloc_frugal))]
pub fn backend_name() -> &'static str {
    get_platform().backend_name()
}

/// Runtime GUI mode contract used by demos/tools to explain visible behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeGuiMode {
    /// Backend is expected to create native windows and run an interactive event loop.
    NativeInteractive,
    /// Backend is preview/stub-like and may not render native windows.
    PreviewOrStub,
}

/// Resolve GUI mode for a specific platform backend.
#[cfg(not(alloc_frugal))]
pub fn runtime_gui_mode_for(platform: &dyn Platform) -> RuntimeGuiMode {
    match platform.backend_name() {
        "cocoa" | "WindowsPlatform" => RuntimeGuiMode::NativeInteractive,
        "wayland" => {
            #[cfg(all(target_os = "linux", feature = "wayland-native"))]
            {
                RuntimeGuiMode::NativeInteractive
            }
            #[cfg(not(all(target_os = "linux", feature = "wayland-native")))]
            {
                RuntimeGuiMode::PreviewOrStub
            }
        }
        "gtk" => {
            #[cfg(all(target_os = "linux", feature = "gtk-native"))]
            {
                RuntimeGuiMode::NativeInteractive
            }
            #[cfg(not(all(target_os = "linux", feature = "gtk-native")))]
            {
                RuntimeGuiMode::PreviewOrStub
            }
        }
        "harmony-desktop"
        | "harmony-state-backend"
        | "macos-objc2-preview"
        | "macos-fallback-stub"
        | "android-state-backend"
        | "ios-mobile-stub" => RuntimeGuiMode::PreviewOrStub,
        _ => RuntimeGuiMode::PreviewOrStub,
    }
}

/// Resolve GUI mode for the active process-global backend.
#[cfg(not(alloc_frugal))]
pub fn runtime_gui_mode() -> RuntimeGuiMode {
    runtime_gui_mode_for(get_platform())
}

/// Returns logical DPI scale factor for the active backend.
#[cfg(not(alloc_frugal))]
pub fn dpi_scale_factor() -> f32 {
    get_platform().dpi_scale_factor()
}

// ---------------------------------------------------------------------------
// Mobile extension (not embedded)
// ---------------------------------------------------------------------------

/// Returns the mobile backend name.
#[cfg(feature = "mobile-api")]
pub fn mobile_backend_name() -> &'static str {
    #[cfg(not(embedded_surface))]
    {
        // Prefer the active platform's own mobile extension so the name matches
        // the backend `get_platform()` returns. Only fall back to the preview
        // singleton when the active platform is not a mobile backend (e.g. running
        // the mobile API on a desktop host for development).
        if let Some(ext) = get_platform().mobile_extension() {
            return match ext.mobile_backend() {
                crate::platform::types::MobileBackend::Android => "android-mobile",
                crate::platform::types::MobileBackend::Ios => "ios-mobile",
                crate::platform::types::MobileBackend::HarmonyMobile => "harmony-mobile",
            };
        }
        mobile::get_mobile_platform().backend_name()
    }
    #[cfg(embedded_surface)]
    {
        "embedded"
    }
}

/// Attaches the mobile backend to a native view handle.
#[cfg(feature = "mobile-api")]
pub fn mobile_attach_to_native_view(native_handle: usize) -> bool {
    #[cfg(not(embedded_surface))]
    {
        // Route to the live platform's mobile extension first: on Android/iOS
        // this is the same instance widget creation uses, so the attached view
        // and the widget state stay in one object.
        if let Some(ext) = get_platform().mobile_extension() {
            return ext.attach_to_native_view(native_handle);
        }
        mobile::get_mobile_platform().attach_to_native_view(native_handle)
    }
    #[cfg(embedded_surface)]
    {
        let _ = native_handle;
        false
    }
}
