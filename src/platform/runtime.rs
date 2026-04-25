#[cfg(all(target_os = "linux", not(feature = "embedded")))]
use crate::platform::linux::LinuxPlatform;
#[cfg(all(target_os = "macos", not(feature = "objc2-macos")))]
use crate::platform::macos::MacOSPlatform;
#[cfg(feature = "mobile-api")]
use crate::platform::mobile;
pub use crate::platform::types::*;
#[cfg(target_os = "windows")]
use crate::platform::windows::WindowsPlatform;
use std::sync::OnceLock;

#[cfg(feature = "embedded")]
fn create_native_platform() -> Box<dyn Platform> {
    Box::new(StubPlatform::new(
        "embedded-runtime-stub",
        PlatformFamily::Embedded,
    ))
}
#[cfg(all(target_os = "windows", not(feature = "embedded")))]
fn create_native_platform() -> Box<dyn Platform> {
    Box::new(WindowsPlatform::new())
}
/// Select objc2 preview backend when migration feature is enabled on macOS.
#[cfg(all(
    target_os = "macos",
    feature = "objc2-macos",
    not(feature = "embedded")
))]
fn create_native_platform() -> Box<dyn Platform> {
    Box::new(crate::platform::macos_objc2::MacOSObjc2Platform::new())
}
/// Select legacy Cocoa backend when objc2 migration feature is disabled.
#[cfg(all(
    target_os = "macos",
    not(feature = "objc2-macos"),
    not(feature = "embedded")
))]
fn create_native_platform() -> Box<dyn Platform> {
    Box::new(MacOSPlatform::new())
}
#[cfg(all(target_os = "linux", not(feature = "embedded")))]
fn create_native_platform() -> Box<dyn Platform> {
    Box::new(LinuxPlatform::new())
}
#[cfg(all(
    not(feature = "embedded"),
    not(any(target_os = "windows", target_os = "macos", target_os = "linux"))
))]
fn create_native_platform() -> Box<dyn Platform> {
    Box::new(StubPlatform::new(
        "unknown-runtime-stub",
        PlatformFamily::Desktop,
    ))
}
static PLATFORM: OnceLock<Box<dyn Platform>> = OnceLock::new();
/// Returns the process-global platform backend instance.
pub fn get_platform() -> &'static dyn Platform {
    PLATFORM.get_or_init(create_native_platform).as_ref()
}
/// Initializes the platform backend.
pub fn init() {
    get_platform().init();
}
/// Runs the platform main loop.
pub fn run() {
    get_platform().run();
}
/// Requests platform main loop shutdown.
pub fn quit() {
    get_platform().quit();
}
/// Returns runtime capabilities for the active backend.
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
pub fn runtime_gui_mode_for(platform: &dyn Platform) -> RuntimeGuiMode {
    match platform.backend_name() {
        "cocoa" | "WindowsPlatform" => RuntimeGuiMode::NativeInteractive,
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
        "harmony-desktop" | "android-mobile" | "macos-objc2-preview" => {
            RuntimeGuiMode::PreviewOrStub
        }
        _ => RuntimeGuiMode::PreviewOrStub,
    }
}
/// Resolve GUI mode for the active process-global backend.
pub fn runtime_gui_mode() -> RuntimeGuiMode {
    runtime_gui_mode_for(get_platform())
}
/// Returns logical DPI scale factor for the active backend.
pub fn dpi_scale_factor() -> f32 {
    get_platform().dpi_scale_factor()
}
#[cfg(feature = "mobile-api")]
/// Returns the mobile backend name.
pub fn mobile_backend_name() -> &'static str {
    mobile::get_mobile_platform().backend_name()
}
#[cfg(feature = "mobile-api")]
/// Attaches the mobile backend to a native view handle.
pub fn mobile_attach_to_native_view(native_handle: usize) -> bool {
    mobile::get_mobile_platform().attach_to_native_view(native_handle)
}
// NOTE: fallback_native_capability_contract and fallback_embedded_capability_contract
// are defined in contract.rs
