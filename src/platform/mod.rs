// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Platform abstraction for desktop/embedded/mobile families.
//!
//! # Reachability
//!
//! **State:** Production callers: `src/control_backend/custom/create_widgets.rs:17` and 41 files (every backend capability is resolved through the `Platform` trait).

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
/// The CoreVideo display link both macOS backends pace frames against.
///
/// Shared because the CoreVideo and mach-time half is pure C ABI: the cocoa backend adds its
/// AppKit observers on top, the objc2 backend uses it as-is.
#[cfg(all(
    target_os = "macos",
    any(all(feature = "cocoa-legacy", widgets_unstripped), feature = "macos")
))]
pub(crate) mod display_link;
/// HarmonyOS / OpenHarmony backend.
///
/// Selected either because the artifact targets OpenHarmony (`target_env = "ohos"`)
/// or because the `harmony` preview feature asked for it on a non-OpenHarmony host.
///
/// It must be `target_env`, not `target_os`: every `*-unknown-linux-ohos` target
/// reports `target_os = "linux"`, so a `cfg(target_os = "ohos")` here would never
/// match and the backend would silently be compiled out on real HarmonyOS builds.
/// See [`crate::platform::profile::is_openharmony_target`].
#[cfg(any(target_env = "ohos", feature = "harmony"))]
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
#[cfg(any(unix_desktop, doc))]
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
// The second arm must name the *legacy* selector, not repeat `macos`:
// `cargo build --features macos-legacy` turns on `cocoa-legacy` without
// `macos`, so a duplicated `feature = "macos"` would match neither arm and
// this module would silently vanish from a build that explicitly asked for a
// macOS backend. Mirrors `src/platform/runtime.rs` and `macos/macos_bridge.rs`.
//
// `target_vendor = "apple"` used to be required here because `platform_impl.rs` answered the four
// `Platform` OS probes from [`darwin_probes`], which needs `libc` and is Apple-only. Those probe
// call sites are now individually `#[cfg(target_vendor = "apple")]`-gated (see `platform_impl.rs`),
// so the module compiles on any host while the *selection* rule stays here: a build that did not ask
// for a macOS backend, or is not on Apple, still gets no `macos_objc2`.
//
// The reason the gate could not simply stay: it silently removed `tests.rs` — 10 assertions that use
// only the `Platform` trait interface and touch no AppKit at all — from every host that is not
// Apple, so they never ran and could never fail. That is the host-invisible-test class `blue23.md`
// §0A.6 ITEM 7 is about. `macos/tests.rs` (the cocoa-legacy sibling) has always compiled this way.
#[cfg(any(feature = "macos", feature = "cocoa-legacy"))]
pub mod macos_objc2;
#[cfg(feature = "mobile-api")]
pub mod mobile;
#[cfg(all(
    target_os = "redox",
    not(alloc_frugal),
    not(embedded_surface),
    not(feature = "harmony")
))]
pub mod redox;
#[cfg(all(unix_desktop, feature = "wayland-native"))]
pub mod wayland;
#[cfg(target_os = "windows")]
pub mod windows;

/// WASM/WebAssembly platform backend (state-driven).
#[cfg(feature = "wasm")]
pub mod wasm;

/// Platform accessibility bridges (macOS, Windows, Linux).
pub mod accessibility;

/// Unified platform diagnostics probe with a stable bit order.
///
/// See the module documentation for the published bit table. The probe drives the real
/// [`Platform`] methods, so a host can run the same twelve checks on every backend instead of
/// relying on source inspection.
#[cfg(not(alloc_frugal))]
pub mod diagnostics;
/// The shared TTL wrapper for the environment facts behind that revision (P-02 / P-201).
///
/// One mechanism rather than a per-backend copy: the TTL and the way a host notification
/// invalidates a reading are properties of the cache, not of the OS being asked. Gated with the
/// profile that has a platform singleton: `mini` answers every environment fact with its honest
/// absence and has no cached reading to invalidate, so the wrapper would be dead code there.
#[cfg(not(alloc_frugal))]
pub(crate) mod environment_cache;
/// The OS change-notification revision shared by every backend (G-23).
pub(crate) mod environment_notify;

/// System-font enumeration snapshot shared by the C ABI and the font dialog (B28.39).
pub mod fonts;
/// CoreText-backed installed-font enumeration for the Apple backends (B28.39).
///
/// The caller on macOS is each backend's `system_fonts`; on iOS it is `ios/native.rs`, which is
/// itself `ios-uikit-ffi`-only. Including a plain `ios` build here compiled three functions that
/// nothing could call (three `never used` warnings in `check_ios_cross.sh`), so the gate names
/// the feature that actually has the caller.
#[cfg(all(
    target_vendor = "apple",
    any(feature = "macos", feature = "cocoa-legacy", feature = "ios-uikit-ffi")
))]
pub(crate) mod fonts_apple;
/// Host-reported mobile lifecycle, orientation, and the animation-clock link (BLUE28 B28.24).
pub mod lifecycle;

/// Locale interning and the POSIX environment fallback shared by the desktop backends.
///
/// `intern`/`environment_revision` are available in every profile (the iOS/state backends call them
/// even under `mini`); only the `std::env` half is gated on `not(alloc_frugal)`. A profile whose
/// selected backend has no locale source compiles the module but cannot call it, which is why the
/// dead-code lint is allowed in exactly those builds.
#[cfg_attr(
    not(any(
        feature = "cocoa-legacy",
        feature = "macos",
        unix_desktop,
        feature = "android-jni",
        feature = "wasm",
        target_env = "ohos",
        feature = "harmony"
    )),
    allow(dead_code)
)]
pub(crate) mod locale;

/// System font enumeration through `fc-list`, shared by the Unix desktop backends.
#[cfg(all(unix_desktop, not(alloc_frugal)))]
pub(crate) mod fonts_unix;

/// Darwin-kernel system probes (`sysconf`, `pmset`, `ps`) shared by the backends
/// that sit on the Apple family: macOS (cocoa and objc2) and iOS.
///
/// Kept separate from [`os_probes`] because that module parses `/proc` and `/sys`,
/// which Apple targets do not mount: forwarding to it returned the "unknown"
/// answer on every Apple host instead of a measurement.
///
/// # Why the gate is `apple` rather than `unix`
///
/// `libc` is linked for the Apple targets only (see the `[target.…]` sections of
/// `Cargo.toml`), so the `sysconf` arm of [`darwin_probes::total_memory_mb`] can
/// only resolve there. Compiling this module for a Linux-kernel target such as
/// `aarch64-unknown-linux-ohos` fails to resolve `libc` — which is exactly what a
/// bare `#[cfg(unix)]` gate did before this one was narrowed.
///
/// # Why `platform/ios/mod.rs` carries the matching gate
///
/// `src/platform/ios/` is declared unconditionally so its state machine can be
/// unit-tested on any host, but `IosMobilePlatform` is only a real backend on an
/// Apple target. Gating that module keeps "compiled everywhere" from meaning
/// "calls probes that do not exist everywhere".
#[cfg(target_vendor = "apple")]
pub mod darwin_probes;
/// Shared host-handler spawn used by the desktop backends' `open_external`.
///
/// Gated to the backends that call it — Windows, the X11/GTK and Wayland backends
/// (Linux and the BSDs, via `unix_desktop`) and the cocoa-based macOS backend. Ungated, this
/// module also compiled into every profile without a desktop backend
/// (`tablet`/`mobile`/`mini`/`embedded` on a non-Unix host, the `android-jni` set), where nothing
/// referenced it and the build reported it as dead code.
#[cfg(any(
    target_os = "windows",
    unix_desktop,
    all(target_os = "macos", feature = "cocoa-legacy")
))]
pub(crate) mod external;
/// Linux-kernel system probes (`/proc`, `/sys`) shared by the backends that sit
/// on a Linux kernel: Android, HarmonyOS, Linux/GTK, Wayland and mobile.
///
/// The BSDs also use these probes, but they do not mount `/proc` by default; every
/// function returns its honest `None`/`false` there rather than inventing a figure.
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
/// Process-group isolation for the children this library spawns itself.
pub(crate) mod child_process;
/// Rich clipboard content types and backend trait (BLUE10 R8.6).
pub mod clipboard;
/// Native rich clipboard bindings and the platform-independent CF_HTML codec (BLUE10 R8.6).
pub mod clipboard_native;
/// Shared cleanup for the menu tables the platform backends keep (PLAT-08).
pub(crate) mod menu_bookkeeping;

mod contract;
pub(crate) mod frame_wait;
/// Device class detection and adaptive layout support (BLUE8 P4-6).
/// Laser holographic keyboard detector (BLUE8 P4-5a, experimental).
#[cfg(feature = "holographic")]
pub mod holographic;
/// IME bridge trait, types, and mock implementation.
pub mod ime;
/// Real Android IME bridge (soft keyboard over JNI).
///
/// Compiled only with the `android-jni` feature, because its focus/commit methods call the JNI
/// bridge; without it the Android backend reports `ime_bridge() == None` and the capability bit
/// stays false.
#[cfg(feature = "android-jni")]
pub mod ime_android;
/// Real Linux/BSD IME bridge (IBus integration).
#[cfg(unix_desktop)]
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
/// Work any thread may hand to the loop's own thread, run by the same frame tick.
#[cfg(not(alloc_frugal))]
pub mod main_thread;
pub(crate) mod runtime;
pub mod state;
/// The stop request any thread may make and the frame tick that reads it.
pub mod stop;
/// The uniform widget-property `Platform` methods, shared by every backend whose widget
/// state lives in [`state::BackendState`]. See the module docs for the defect it removes.
#[macro_use]
pub(crate) mod state_impl;
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
// Re-exports: everything that was previously defined directly in mod.rs.
//
// `contract` publishes only what it *defines* — the negotiation entry point — plus the
// extension-trait path the Android/iOS backends use. The contract *types* are owned by
// `platform::types`, so they are re-exported from there rather than laundered through
// `contract` (HX-07: a module must not republish another module's surface).
pub use crate::platform::contract::negotiate_capability_contract;
pub use crate::platform::portable::{FrameBuffer, SurfaceGeometry};
pub use crate::platform::runtime::RuntimeGuiMode;
#[cfg(not(alloc_frugal))]
pub use crate::platform::runtime::{backend_name, capabilities, get_platform, init, quit, run};
#[cfg(not(alloc_frugal))]
pub use crate::platform::runtime::{
    display_generation, display_of_window, displays, dpi_scale_factor,
};
#[cfg(feature = "mobile-api")]
pub use crate::platform::runtime::{mobile_attach_to_native_view, mobile_backend_name};
#[cfg(not(alloc_frugal))]
pub use crate::platform::runtime::{runtime_gui_mode, runtime_gui_mode_for};
/// The invalidation recorder and its override, for tests that need to observe what the
/// runtime asked a backend to do. Public because the runtime's own tests live in
/// another module; nothing outside tests installs an override.
#[cfg(not(alloc_frugal))]
pub use crate::platform::runtime::{
    with_platform, with_recorded_invalidations, RecordingInvalidations,
};
pub use crate::platform::stub::StubPlatform;
pub use crate::platform::types::*;
pub use crate::platform::types::{
    CapabilityContract, EmbeddedCapabilityContract, HostedCapabilityContract,
};

pub mod a11y_wiring;

#[cfg(not(alloc_frugal))]
pub use a11y_wiring::wire_focus_manager_to_a11y;

mod facts;
pub use facts::platform_facts;
