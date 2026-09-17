// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Platform runtime initialization, backend selection, and lifecycle management.
//!
//! Provides `init()`, `run()`, `quit()`, `get_platform()`, and DPI scale factor
//! querying.  Platform backends are selected at compile time based on `target_os`
//! and feature flags, then cached in a global singleton.

#[cfg(not(alloc_frugal))]
use crate::compat::OnceLock;
// `lock_guard` is an extension trait, so it has to be in scope for the recorder's
// `Mutex` to be usable without an `unwrap` on every call. Gated with the recorder:
// the alloc-frugal profile compiles no recorder, so the trait would be unused there.
#[cfg(not(alloc_frugal))]
use crate::core::MutexExt as _;
// The recorder implements `Platform`, so the trait must be nameable here. Gated with
// the recorder itself: `mini` compiles this module but not the desktop backend set.
#[cfg(all(target_os = "android", not(alloc_frugal), not(embedded_surface)))]
use crate::platform::android::AndroidPlatform;
#[cfg(all(
    any(target_env = "ohos", feature = "harmony"),
    not(target_os = "android"),
    not(alloc_frugal),
    not(embedded_surface)
))]
use crate::platform::harmony::HarmonyPlatform;
#[cfg(all(not(alloc_frugal), target_os = "ios", not(embedded_surface)))]
use crate::platform::ios::IosMobilePlatform;
#[cfg(all(
    target_os = "linux",
    not(target_env = "ohos"),
    not(alloc_frugal),
    not(embedded_surface),
    not(feature = "harmony")
))]
use crate::platform::linux::LinuxPlatform;
#[cfg(all(
    not(alloc_frugal),
    target_os = "macos",
    not(embedded_surface),
    any(feature = "macos", feature = "cocoa-legacy"),
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
    not(target_env = "ohos"),
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
#[cfg(not(alloc_frugal))]
use crate::platform::Platform;

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
    not(target_env = "ohos"),
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
    any(feature = "macos", feature = "cocoa-legacy"),
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
    not(any(feature = "macos", feature = "cocoa-legacy")),
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
///
/// `not(target_env = "ohos")` keeps this arm off OpenHarmony, which reports
/// `target_os = "linux"` and must select `HarmonyPlatform` instead (see below).
#[cfg(all(
    not(alloc_frugal),
    target_os = "linux",
    not(target_env = "ohos"),
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
/// Both the `harmony` feature and the OpenHarmony target exclude this arm: either
/// must select `HarmonyPlatform`, matching the `any(target_env = "ohos",
/// feature = "harmony")` arm below. Without the `target_env` test the two arms
/// would both match on OpenHarmony and `create_native_platform` would be defined
/// twice — a hard error, not a silent fallback.
#[cfg(all(
    not(alloc_frugal),
    target_os = "linux",
    not(target_env = "ohos"),
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
///
/// The target test is `target_env`, not `target_os`: `*-unknown-linux-ohos`
/// targets report `target_os = "linux"`, so a `target_os = "ohos"` arm here
/// would never be reached and OpenHarmony builds would silently get
/// `LinuxPlatform` (which requires GTK, absent on OpenHarmony).
#[cfg(all(
    any(target_env = "ohos", feature = "harmony"),
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
    not(any(target_env = "ohos", feature = "harmony")),
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
///
/// A thread-local override installed by [`with_recorded_invalidations`] takes
/// precedence, so a test can observe what the runtime asked a backend to do without
/// swapping the process-global singleton.
#[cfg(not(alloc_frugal))]
pub fn get_platform() -> &'static dyn Platform {
    if let Some(overridden) = PLATFORM_OVERRIDE.try_with(|slot| *slot.borrow()).ok().flatten() {
        return overridden;
    }
    PLATFORM.get_or_init(create_native_platform).as_ref()
}

/// A platform override that records what it was asked to invalidate.
///
/// # Why this exists
///
/// Whether the damage tracker actually *told the platform* about a region is the whole
/// question the partial-repaint wiring has to answer, and it is invisible from outside:
/// the tracker's state says what was recorded, not what the backend was asked to do.
/// The process-global platform is a `OnceLock`, so it cannot be swapped out per test,
/// and a test that reached the real backend would either do nothing (no window) or
/// touch a display.
///
/// So the override is thread-local and consulted by [`get_platform`] before the
/// singleton. It is `#[cfg(test)]` in effect — the type is public because the runtime
/// tests live in a different module, but nothing outside tests installs one.
///
/// Gated with the trait it implements: the alloc-frugal profile has no `Platform` to
/// implement, and no invalidation path to record.
#[cfg(not(alloc_frugal))]
pub struct RecordingInvalidations {
    calls:
        crate::compat::Mutex<alloc::vec::Vec<(crate::core::ObjectId, Option<crate::core::Rect>)>>,
    /// Whether to claim the narrowed repaint succeeded.
    ///
    /// `true` models a backend that can narrow (`GTK`'s `queue_draw_area`); `false`
    /// models one that cannot, which must drive the caller's fallback.
    narrows: core::sync::atomic::AtomicBool,
}

#[cfg(not(alloc_frugal))]
impl RecordingInvalidations {
    /// A recorder that accepts narrowed repaints.
    pub fn new() -> Self {
        Self {
            calls: crate::compat::Mutex::new(alloc::vec::Vec::new()),
            narrows: core::sync::atomic::AtomicBool::new(true),
        }
    }

    /// A recorder that refuses narrowed repaints, modelling a backend without them.
    pub fn refusing_narrowing() -> Self {
        Self {
            calls: crate::compat::Mutex::new(alloc::vec::Vec::new()),
            narrows: core::sync::atomic::AtomicBool::new(false),
        }
    }

    /// What the backend was asked to invalidate, in order.
    ///
    /// An entry with `Some(rect)` is a narrowed repaint; `None` is a whole-surface
    /// one. Recording both in one list is what lets a test distinguish "narrowed" from
    /// "fell back".
    pub fn calls(&self) -> alloc::vec::Vec<(crate::core::ObjectId, Option<crate::core::Rect>)> {
        self.calls.lock_guard().clone()
    }

    /// Forgets every recorded call.
    pub fn clear(&self) {
        self.calls.lock_guard().clear();
    }
}

#[cfg(not(alloc_frugal))]
impl Default for RecordingInvalidations {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(alloc_frugal))]
impl Platform for RecordingInvalidations {
    fn backend_name(&self) -> &'static str {
        "recording-test-backend"
    }

    fn family(&self) -> crate::core::PlatformFamily {
        crate::core::PlatformFamily::Desktop
    }

    fn init(&self) {}

    fn run(&self) {}

    fn quit(&self) {}

    fn create_window(
        &self,
        _title: &str,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> crate::core::ObjectId {
        0
    }

    fn invalidate_surface(&self, id: crate::core::ObjectId) -> bool {
        self.calls.lock_guard().push((id, None));
        true
    }

    fn invalidate_surface_rect(&self, id: crate::core::ObjectId, rect: crate::core::Rect) -> bool {
        if !self.narrows.load(core::sync::atomic::Ordering::SeqCst) {
            // The contract: `false` means "I did not narrow it", and the caller falls
            // back. Recording nothing is deliberate — a backend that claims to fail
            // must not also have queued a repaint.
            return false;
        }
        self.calls.lock_guard().push((id, Some(rect)));
        true
    }
}

#[cfg(not(alloc_frugal))]
thread_local! {
    /// Per-thread platform override installed by [`with_recorded_invalidations`].
    #[allow(clippy::missing_const_for_thread_local)]
    static PLATFORM_OVERRIDE: core::cell::RefCell<Option<&'static dyn Platform>> =
        core::cell::RefCell::new(None);
}

/// Runs `f` with `recorder` as the active platform.
///
/// Restores the previous override even when `f` panics, so a failing test cannot leak
/// its recorder into whatever runs next on the same thread.
#[cfg(not(alloc_frugal))]
pub fn with_recorded_invalidations<R>(
    recorder: &'static RecordingInvalidations,
    f: impl FnOnce() -> R,
) -> R {
    struct Restore(Option<&'static dyn Platform>);
    impl Drop for Restore {
        fn drop(&mut self) {
            let previous = self.0;
            let _ = PLATFORM_OVERRIDE.try_with(|slot| *slot.borrow_mut() = previous);
        }
    }

    let previous = PLATFORM_OVERRIDE
        .try_with(|slot| {
            let previous = *slot.borrow();
            *slot.borrow_mut() = Some(recorder);
            previous
        })
        .unwrap_or(None);
    let _restore = Restore(previous);
    f()
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
