//! Platform runtime initialization, backend selection, and lifecycle management.
//!
//! Provides `init()`, `run()`, `quit()`, `get_platform()`, and DPI scale factor
//! querying.  Platform backends are selected at compile time based on `target_os`
//! and feature flags, then cached in a global singleton.

#[cfg(not(feature = "mini"))]
use crate::compat::OnceLock;
#[cfg(target_os = "ios")]
use crate::platform::ios::IosMobilePlatform;
#[cfg(all(target_os = "linux", not(feature = "mini"), not(feature = "embedded")))]
use crate::platform::linux::LinuxPlatform;
#[cfg(all(
    target_os = "macos",
    not(feature = "embedded"),
    any(feature = "macos", feature = "macos-legacy")
))]
use crate::platform::macos::macos_bridge::SelectedMacOSPlatform;
#[cfg(all(not(feature = "embedded"), feature = "mobile-api"))]
use crate::platform::mobile;
#[cfg(not(feature = "mini"))]
pub use crate::platform::types::*;
#[cfg(all(target_os = "linux", not(feature = "embedded"), feature = "wayland-native"))]
use crate::platform::wayland::WaylandPlatform;
#[cfg(all(target_os = "windows", not(feature = "embedded")))]
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
#[cfg(all(target_os = "linux", not(feature = "embedded"), feature = "wayland-native"))]
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
#[cfg(all(not(feature = "mini"), feature = "embedded"))]
fn create_native_platform() -> Box<dyn Platform> {
    Box::new(crate::platform::stub::StubPlatform::new(
        "embedded-runtime-stub",
        crate::core::PlatformFamily::Embedded,
    ))
}

#[cfg(all(not(feature = "mini"), target_os = "windows", not(feature = "embedded")))]
fn create_native_platform() -> Box<dyn Platform> {
    Box::new(WindowsPlatform::new())
}

/// Select macOS backend via the bridge (BLUE11 R1.5).
/// The bridge dispatches to objc2 or cocoa based on feature flags.
#[cfg(all(
    not(feature = "mini"),
    target_os = "macos",
    not(feature = "embedded"),
    any(feature = "macos", feature = "macos-legacy")
))]
fn create_native_platform() -> Box<dyn Platform> {
    Box::new(SelectedMacOSPlatform::new())
}

/// macOS fallback when no macos/macos-legacy backend feature is active.
#[cfg(all(
    not(feature = "mini"),
    target_os = "macos",
    not(feature = "embedded"),
    not(any(feature = "macos", feature = "macos-legacy"))
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
    not(feature = "mini"),
    target_os = "linux",
    not(feature = "embedded"),
    feature = "wayland-native"
))]
fn create_native_platform() -> Box<dyn Platform> {
    if is_wayland_session() {
        Box::new(WaylandPlatform::new())
    } else {
        Box::new(LinuxPlatform::new())
    }
}

/// Linux without wayland-native feature → always use LinuxPlatform.
#[cfg(all(
    not(feature = "mini"),
    target_os = "linux",
    not(feature = "embedded"),
    not(feature = "wayland-native")
))]
fn create_native_platform() -> Box<dyn Platform> {
    Box::new(LinuxPlatform::new())
}

/// iOS state-backed platform backend.
#[cfg(all(not(feature = "mini"), target_os = "ios", not(feature = "embedded")))]
fn create_native_platform() -> Box<dyn Platform> {
    Box::new(IosMobilePlatform::new())
}

/// WASM backend — used on `wasm32` targets when the `wasm` feature is enabled.
/// On wasm32 the event loop is driven by `request_animation_frame`; elsewhere
/// the same backend uses a polling fallback (used for development/testing).
#[cfg(all(
    not(feature = "mini"),
    not(feature = "embedded"),
    feature = "wasm",
    target_arch = "wasm32"
))]
fn create_native_platform() -> Box<dyn Platform> {
    Box::new(crate::platform::wasm::WasmPlatform::default())
}

#[cfg(all(
    not(feature = "mini"),
    not(feature = "embedded"),
    not(all(feature = "wasm", target_arch = "wasm32")),
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

#[cfg(not(feature = "mini"))]
static PLATFORM: OnceLock<Box<dyn Platform>> = OnceLock::new();

/// Returns the process-global platform backend instance.
#[cfg(not(feature = "mini"))]
pub fn get_platform() -> &'static dyn Platform {
    PLATFORM.get_or_init(create_native_platform).as_ref()
}

/// Initializes the platform backend.
#[cfg(not(feature = "mini"))]
pub fn init() {
    get_platform().init();
}

/// Runs the platform main loop.
#[cfg(not(feature = "mini"))]
pub fn run() {
    get_platform().run();
}

/// Requests platform main loop shutdown.
#[cfg(not(feature = "mini"))]
pub fn quit() {
    get_platform().quit();
}

/// Returns runtime capabilities for the active backend.
#[cfg(not(feature = "mini"))]
pub fn capabilities() -> PlatformCapabilities {
    get_platform().capabilities()
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
#[cfg(not(feature = "mini"))]
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
#[cfg(not(feature = "mini"))]
pub fn runtime_gui_mode() -> RuntimeGuiMode {
    runtime_gui_mode_for(get_platform())
}

/// Returns logical DPI scale factor for the active backend.
#[cfg(not(feature = "mini"))]
pub fn dpi_scale_factor() -> f32 {
    get_platform().dpi_scale_factor()
}

// ---------------------------------------------------------------------------
// Mobile extension (not embedded)
// ---------------------------------------------------------------------------

/// Returns the mobile backend name.
#[cfg(feature = "mobile-api")]
pub fn mobile_backend_name() -> &'static str {
    #[cfg(not(feature = "embedded"))]
    {
        mobile::get_mobile_platform().backend_name()
    }
    #[cfg(feature = "embedded")]
    {
        "embedded"
    }
}

/// Attaches the mobile backend to a native view handle.
#[cfg(feature = "mobile-api")]
pub fn mobile_attach_to_native_view(native_handle: usize) -> bool {
    #[cfg(not(feature = "embedded"))]
    {
        mobile::get_mobile_platform().attach_to_native_view(native_handle)
    }
    #[cfg(feature = "embedded")]
    {
        let _ = native_handle;
        false
    }
}
