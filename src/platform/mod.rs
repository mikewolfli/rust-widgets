// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

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
/// iOS mobile backend (state-driven).
///
/// The widget state machine is platform-independent, so the module is compiled
/// on every host to keep its unit tests executable. The only UIKit (`objc2`)
/// touch point left is the window the library paints into, which is
/// `#[cfg(feature = "ios-uikit-ffi")]`-gated internally; that feature
/// transitively requires the iOS target (`ios = ["dep:objc2", ...]`), so on other
/// hosts the backend runs in pure state mode.
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
/// feature = "macos"))]`-gated; elsewhere the backend runs in pure state
/// mode. See BLUE14 D-1 for the precedent (`ime_windows`).
#[cfg(any(feature = "macos", feature = "macos"))]
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

/// Linux-kernel system probes (`/proc`, `/sys`) shared by the backends that sit
/// on a Linux kernel: Android, HarmonyOS, Linux/GTK, Wayland and mobile.
pub mod os_probes;
/// The portable host backend — a drawing surface with no operating system behind it.
///
/// This is the build target for `mini`, for `embedded` on a host without a
/// backend, and for any target that has no backend module. Those three are one
/// fact ("a surface, no OS controls"), which is why they share one backend
/// instead of three `cfg` branches. See the module docs for the reasoning.
pub mod portable;
/// Compile-time runtime-profile facts — the single gating entry point.
///
/// This is the only module in `src/` permitted to read the `mini` / `embedded`
/// feature names; everything else asks it a semantic question. See BLUE15
/// principles #57/#58 and the module docs for the reasoning.
pub mod profile;

// Internal sub-modules (split from monolithic mod.rs)
/// Rich clipboard content types and backend trait (BLUE10 R8.6).
pub mod clipboard;
/// Platform-specific rich clipboard stubs (BLUE10 R8.6).
pub mod clipboard_stubs;
mod contract;
/// Device class detection and adaptive layout support (BLUE8 P4-6).
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
/// "macos", feature = "macos"))]`-gated internally; on other hosts the
/// bridge runs in pure state-machine mode.
pub mod ime_macos;
/// Platform-specific IME stubs (macOS, Windows).
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

/// Cross-backend behavioural contract tests.
#[cfg(all(test, not(alloc_frugal)))]
mod contract_tests;
/// Widget teardown (`Platform::destroy_widget`) regression tests.
#[cfg(all(test, not(alloc_frugal)))]
mod teardown_tests;
#[cfg(all(test, not(alloc_frugal)))]
mod tests;
/// Virtual keyboard controller for touch text input (BLUE8 P4-7).
// Re-exports: everything that was previously defined directly in mod.rs
pub use crate::platform::contract::{negotiate_capability_contract, CapabilityContract};
pub use crate::platform::contract::{EmbeddedCapabilityContract, NativeCapabilityContract};
pub use crate::platform::portable::{FrameBuffer, SurfaceGeometry};
pub use crate::platform::runtime::RuntimeGuiMode;
#[cfg(not(alloc_frugal))]
pub use crate::platform::runtime::{backend_name, capabilities, get_platform, init, quit, run};
#[cfg(not(alloc_frugal))]
pub use crate::platform::runtime::{dpi_scale_factor, runtime_gui_mode, runtime_gui_mode_for};
#[cfg(feature = "mobile-api")]
pub use crate::platform::runtime::{mobile_attach_to_native_view, mobile_backend_name};
pub use crate::platform::stub::StubPlatform;
pub use crate::platform::types::*;

pub mod a11y_wiring;

#[cfg(not(alloc_frugal))]
pub use a11y_wiring::wire_focus_manager_to_a11y;

/// Platform facts accessor usable in every profile, including `mini`.
///
/// Upper layers (print, GPU adaptation, menu hardware detection) need to ask the
/// backend about OS facts — total memory, battery state, spooler availability.
/// The `mini` profile is deliberately alloc-free and has **no platform
/// singleton** (`get_platform` is `not(mini)`), so a direct call would fail to
/// compile there.
///
/// Every build's platform-facts accessor, including `mini`.
///
/// Upper layers (print, GPU adaptation, menu hardware detection) need to ask the
/// backend about OS facts — total memory, battery state, spooler availability.
/// The `mini` profile is deliberately alloc-frugal and has **no platform
/// singleton** (`get_platform` is `not(alloc_frugal)`), so a direct call would
/// fail to compile there.
///
/// Both arms return the *same kind* of value: a host that reports, for every
/// capability it does not have, the honest absence (`None` / `false` / empty)
/// rather than a made-up figure (principle #37). Outside `mini` that is whichever
/// OS backend this target has; inside `mini` it is the portable host. Call sites
/// stay free of `cfg` branching (principle #35).
pub fn platform_facts() -> &'static dyn Platform {
    #[cfg(not(alloc_frugal))]
    {
        runtime::get_platform()
    }
    #[cfg(alloc_frugal)]
    {
        crate::platform::portable::instance()
    }
}
