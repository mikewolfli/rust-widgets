// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `Platform` implementation for the macOS `objc2` preview backend.
//!
//! Selected by the `macos` feature; the `cocoa-legacy` backend is the alternative
//! (`src/platform/macos/`). Both are state-driven here — the same
//! [`crate::platform::state::BackendState`] contract — with the AppKit-specific work
//! confined to the `native` sub-module.
//!
//! AppKit, like GTK, must be driven from the process main thread, so the surface
//! methods refuse rather than building a view off it.

use super::types::{MacOSObjc2Platform, MacObjc2HandleKind};
use crate::compat::atomic::Ordering;
use crate::compat::{String, Vec};
use crate::core::ObjectId;
use crate::core::PlatformFamily;
use crate::platform::types::{Display, DisplayId, SafeAreaInsets};
use crate::platform::{Platform, PlatformCapabilities};

/// Resolves the widget id a host holds to the platform window id this backend registers.
///
/// A profile with no widget runtime has only platform ids, so the id is returned unchanged.
#[cfg(all(target_os = "macos", feature = "macos"))]
fn resolve_host(window_id: u64) -> u64 {
    #[cfg(not(alloc_frugal))]
    {
        crate::widget::runtime::host_window_for(window_id).unwrap_or(window_id)
    }
    #[cfg(alloc_frugal)]
    {
        window_id
    }
}

/// The frame interval this backend's loop runs at, in milliseconds.
///
/// The same value as every other backend's twin constant, and named here for the same
/// reason: the sleep between iterations and the delta handed to [`crate::drive_frame`]
/// must be the same number, or every transition runs at the ratio between them with
/// nothing to notice the mismatch. A backend that later runs at the display's own rate
/// changes exactly this value.
const FRAME_INTERVAL_MS: u64 = crate::platform::types::FRAME_INTERVAL_MS;

impl Platform for MacOSObjc2Platform {
    // The uniform widget-property methods are answered once, over `self.state`, by the
    // shared expansion in `platform::state_impl` rather than re-written per backend. The
    // window-state methods are excluded because this backend applies them through AppKit
    // and falls back to the same record itself (BLUE28 B28.19).
    crate::impl_platform_state_properties!(without_window_state);

    // ---- Lifecycle & identity ----
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn backend_name(&self) -> &'static str {
        "macos-objc2-preview"
    }
    fn family(&self) -> PlatformFamily {
        PlatformFamily::Desktop
    }

    /// The system appearance AppKit resolved for the app.
    ///
    /// Read from `NSApplication.effectiveAppearance` on the main thread through the `native`
    /// module, which is the same source winit's AppKit backend uses. The legacy `cocoa` backend
    /// and a build without the `macos` feature answer the trait's `None` rather than a guess.
    fn system_theme(&self) -> Option<bool> {
        #[cfg(all(target_os = "macos", feature = "macos"))]
        {
            super::native::system_theme()
        }
        #[cfg(not(all(target_os = "macos", feature = "macos")))]
        {
            None
        }
    }

    /// The displays AppKit reports, through `+[NSScreen screens]`.
    ///
    /// This is the same source the cocoa backend daemonises in `src/platform/macos/displays.rs`.
    /// It was missing here, so a build that selected this backend (`--features macos`) could not
    /// enumerate screens even though the cocoa backend could. The vector is empty without AppKit
    /// (no `macos` feature) or off the main thread — the honest absence, not a made-up display.
    fn displays(&self) -> Vec<Display> {
        #[cfg(all(target_os = "macos", feature = "macos"))]
        {
            super::native::displays()
        }
        #[cfg(not(all(target_os = "macos", feature = "macos")))]
        {
            Vec::new()
        }
    }

    /// The main screen's scale, read from `NSScreen.backingScaleFactor`.
    ///
    /// This is the per-screen `scale` [`Self::displays`] reports, read once for the trait's global
    /// value. Without AppKit (no `macos` feature) or off the main thread the honest answer is the
    /// trait default `1.0`; [`Self::capabilities`] reports `dpi_scaling: false` in exactly that
    /// build, so a host is never told a fabricated scale is measured.
    fn dpi_scale_factor(&self) -> f32 {
        #[cfg(all(target_os = "macos", feature = "macos"))]
        {
            super::native::main_scale().unwrap_or(1.0)
        }
        #[cfg(not(all(target_os = "macos", feature = "macos")))]
        {
            1.0
        }
    }

    /// The IME bridge the canvas view's `NSTextInputClient` callbacks drive (B28.12).
    ///
    /// One bridge exists per process because one `NSTextInputContext` exists per application;
    /// the view and this accessor share it through `native::ime_bridge`.
    fn ime_bridge(&self) -> Option<&dyn crate::platform::ime::ImeBridge> {
        #[cfg(all(target_os = "macos", feature = "macos"))]
        {
            Some(super::native::ime_bridge())
        }
        #[cfg(not(all(target_os = "macos", feature = "macos")))]
        {
            None
        }
    }

    /// The `NSAccessibility` bridge the canvas view feeds (B28.13).
    ///
    /// The view answers `accessibilityRole`/`accessibilityLabel`/`accessibilityFrame` from the
    /// live widget registry; this bridge posts change notifications for the view registered
    /// against each widget id.
    fn accessibility_bridge(
        &self,
    ) -> Option<&dyn crate::platform::accessibility::AccessibilityBridge> {
        #[cfg(feature = "macos")]
        {
            Some(crate::platform::accessibility::macos_objc2::bridge())
        }
        #[cfg(not(feature = "macos"))]
        {
            None
        }
    }

    /// The screen the window is on, asked of AppKit's `-[NSWindow screen]`.
    ///
    /// Both id spaces are accepted: the platform id `create_window` returned and the widget id a
    /// host may hold (translated through `host_window_for`). `None` for an unknown id, a non-window
    /// handle, or a call off the main thread.
    fn display_of_window(&self, window_id: u64) -> Option<DisplayId> {
        #[cfg(all(target_os = "macos", feature = "macos"))]
        {
            if let Some(id) = super::native::screen_id_of_window(window_id) {
                return Some(DisplayId::new(id));
            }
            #[cfg(not(alloc_frugal))]
            if let Some(host) = crate::widget::runtime::host_window_for(window_id) {
                if let Some(id) = super::native::screen_id_of_window(host) {
                    return Some(DisplayId::new(id));
                }
            }
        }
        #[cfg(not(all(target_os = "macos", feature = "macos")))]
        {
            let _ = window_id;
        }
        None
    }

    /// A revision of the current screen configuration.
    ///
    /// Derived from the same fields [`Self::displays`] returns, so a caller that compares two
    /// readings re-reads the list exactly when a screen was added/removed or its bounds or scale
    /// changed. `0` means AppKit could not be read (off the main thread, or no `macos` feature).
    fn display_generation(&self) -> u64 {
        const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
        const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
        let displays = self.displays();
        if displays.is_empty() {
            return 0;
        }
        let mut hash = FNV_OFFSET;
        for display in displays {
            for part in [
                display.id.get(),
                display.bounds.x as i64 as u64,
                display.bounds.y as i64 as u64,
                display.bounds.width as u64,
                display.bounds.height as u64,
                display.scale.to_bits() as u64,
                display.is_primary as u64,
            ] {
                hash ^= part;
                hash = hash.wrapping_mul(FNV_PRIME);
            }
        }
        hash
    }

    /// Metadata for one AppKit screen.
    ///
    /// The orientation is derived from the reported bounds; `NSScreen` exposes no synchronous
    /// refresh-rate or colour-space accessor, so those answer `None`.
    fn display_info(&self, id: DisplayId) -> Option<crate::platform::types::DisplayInfo> {
        let display = self.displays().into_iter().find(|display| display.id == id)?;
        Some(crate::platform::types::DisplayInfo {
            refresh_rate_mhz: None,
            orientation: Some(if display.bounds.width >= display.bounds.height {
                crate::core::Orientation::Horizontal
            } else {
                crate::core::Orientation::Vertical
            }),
            color_space: None,
            hdr: None,
        })
    }

    /// Whether the user asked for increased contrast, from `NSWorkspace`.
    ///
    /// `None` without the AppKit-bound build. AppKit's shared workspace is thread-safe, so this is
    /// answerable from any thread, unlike the screen enumeration above.
    fn high_contrast(&self) -> Option<bool> {
        #[cfg(all(target_os = "macos", feature = "macos"))]
        {
            Some(super::native::high_contrast())
        }
        #[cfg(not(all(target_os = "macos", feature = "macos")))]
        {
            None
        }
    }

    /// Whether the user asked for reduced motion, from `NSWorkspace`.
    fn reduce_motion(&self) -> Option<bool> {
        #[cfg(all(target_os = "macos", feature = "macos"))]
        {
            Some(super::native::reduce_motion())
        }
        #[cfg(not(all(target_os = "macos", feature = "macos")))]
        {
            None
        }
    }

    /// Whether the user asked for translucent materials to be reduced, from `NSWorkspace`.
    fn reduce_transparency(&self) -> Option<bool> {
        #[cfg(all(target_os = "macos", feature = "macos"))]
        {
            Some(super::native::reduce_transparency())
        }
        #[cfg(not(all(target_os = "macos", feature = "macos")))]
        {
            None
        }
    }

    /// The host's current accent colour, from `NSColor.controlAccentColor`.
    ///
    /// `None` without the AppKit-bound build, which is distinct from a host that reports the
    /// default blue: only the latter is a colour the user can be said to have chosen.
    fn accent_color(&self) -> Option<crate::core::Color> {
        #[cfg(all(target_os = "macos", feature = "macos"))]
        {
            super::native::accent_color()
        }
        #[cfg(not(all(target_os = "macos", feature = "macos")))]
        {
            None
        }
    }

    /// Whether this backend paces frames against CoreVideo's presentation clock.
    ///
    /// Answered from the display link `init` starts. A host where CoreVideo could not start, and
    /// every non-AppKit build, answers `false` and keeps the fixed frame grid.
    fn supports_vsync_frame(&self) -> bool {
        #[cfg(all(target_os = "macos", feature = "macos"))]
        {
            crate::platform::display_link::is_running()
        }
        #[cfg(not(all(target_os = "macos", feature = "macos")))]
        {
            false
        }
    }

    /// The next presented-time instant published by the display-link callback.
    fn next_vsync_deadline(&self) -> Option<std::time::Instant> {
        #[cfg(all(target_os = "macos", feature = "macos"))]
        {
            crate::platform::display_link::next_deadline()
        }
        #[cfg(not(all(target_os = "macos", feature = "macos")))]
        {
            None
        }
    }

    /// The user's locale, from `NSLocale.currentLocale`.
    fn system_locale(&self) -> Option<&'static str> {
        #[cfg(all(target_os = "macos", feature = "macos"))]
        {
            super::native::system_locale()
        }
        #[cfg(not(all(target_os = "macos", feature = "macos")))]
        {
            None
        }
    }

    /// A revision of the environment facts, derived from their current values.
    fn environment_generation(&self) -> u64 {
        crate::platform::locale::generation_hash(
            self.system_theme(),
            self.high_contrast(),
            self.reduce_motion(),
            self.system_locale(),
        )
    }

    /// Applies a cursor shape through `NSCursor` (BLUE28 B28.09).
    #[cfg(all(target_os = "macos", feature = "macos"))]
    fn set_cursor(&self, _window: u64, icon: crate::platform::CursorIcon) -> bool {
        super::native::set_cursor(icon)
    }

    /// Hides or shows the pointer through `NSCursor`.
    #[cfg(all(target_os = "macos", feature = "macos"))]
    fn set_cursor_visible(&self, _window: u64, visible: bool) -> bool {
        super::native::set_cursor_visible(visible)
    }

    /// The font families AppKit reports; empty without the AppKit-bound build.
    fn system_fonts(&self) -> Vec<crate::platform::types::SystemFont> {
        #[cfg(all(target_os = "macos", feature = "macos"))]
        {
            super::native::system_fonts()
        }
        #[cfg(not(all(target_os = "macos", feature = "macos")))]
        {
            Vec::new()
        }
    }

    /// Applies a window state through AppKit, or records it when no native window exists.
    ///
    /// BLUE28 B28.19: this used to be only the state macro, so `WindowHandle::set_maximized`
    /// reported success while the window never changed. The native calls are the same ones the
    /// cocoa backend uses; a state-only handle (no `NSWindow`) still records the value, which is
    /// what the off-main-thread window contract requires.
    fn set_window_state(
        &self,
        window_id: u64,
        flag: crate::platform::WindowStateFlag,
        on: bool,
    ) -> bool {
        #[cfg(all(target_os = "macos", feature = "macos"))]
        {
            let host = resolve_host(window_id);
            if super::native::set_window_state(host, flag, on) {
                self.state.set_window_state(window_id, flag, on);
                return true;
            }
        }
        self.state.set_window_state(window_id, flag, on)
    }

    /// Reads a window state from AppKit, falling back to the recorded value.
    fn is_window_in_state(
        &self,
        window_id: u64,
        flag: crate::platform::WindowStateFlag,
    ) -> Option<bool> {
        #[cfg(all(target_os = "macos", feature = "macos"))]
        {
            let host = resolve_host(window_id);
            if let Some(answer) = super::native::window_state(host, flag) {
                return Some(answer);
            }
        }
        self.state.window_state(window_id, flag)
    }

    /// Sets the content minimum size through `NSWindow.setContentMinSize:`.
    fn set_window_min_size(&self, window_id: u64, width: u32, height: u32) -> bool {
        #[cfg(all(target_os = "macos", feature = "macos"))]
        {
            let host = resolve_host(window_id);
            if super::native::set_window_min_size(host, width, height) {
                self.state.set_window_min_size(window_id, width, height);
                return true;
            }
        }
        self.state.set_window_min_size(window_id, width, height)
    }

    /// Reads the content minimum size from AppKit, falling back to the recorded value.
    fn window_min_size(&self, window_id: u64) -> Option<(u32, u32)> {
        #[cfg(all(target_os = "macos", feature = "macos"))]
        {
            let host = resolve_host(window_id);
            if let Some(size) = super::native::window_min_size(host) {
                return Some(size);
            }
        }
        self.state.window_min_size(window_id)
    }

    /// AppKit window icons are a bundle-level fact, not a per-window one.
    ///
    /// `NSWindow` has no icon API; the Dock icon comes from `CFBundleIconFile`. Reporting `false`
    /// is the honest answer and keeps `WindowHandle::icon` from mirroring a path the OS never
    /// accepted.
    fn set_window_icon(&self, _window_id: u64, _path: &str) -> bool {
        false
    }

    /// The native `NSView` (canvas) or `NSWindow` pointer, when this backend owns one.
    fn get_native_handle(&self, widget: u64) -> Option<usize> {
        #[cfg(all(target_os = "macos", feature = "macos"))]
        {
            let host = resolve_host(widget);
            super::native::native_handle(host)
        }
        #[cfg(not(all(target_os = "macos", feature = "macos")))]
        {
            let _ = widget;
            None
        }
    }

    /// The window's content view as a `raw-window-handle` 0.6 AppKit handle.
    #[cfg(feature = "raw-window-handle")]
    fn raw_window_handle(&self, window_id: u64) -> Option<raw_window_handle::RawWindowHandle> {
        #[cfg(all(target_os = "macos", feature = "macos"))]
        {
            let host = resolve_host(window_id);
            let view = super::native::raw_view_handle(host)?;
            let ns_view = core::ptr::NonNull::new(view)?;
            return Some(raw_window_handle::RawWindowHandle::AppKit(
                raw_window_handle::AppKitWindowHandle::new(ns_view),
            ));
        }
        #[cfg(not(all(target_os = "macos", feature = "macos")))]
        {
            let _ = window_id;
            None
        }
    }

    /// The empty AppKit display handle, in the AppKit-bound build only.
    #[cfg(feature = "raw-window-handle")]
    fn raw_display_handle(&self) -> Option<raw_window_handle::RawDisplayHandle> {
        #[cfg(all(target_os = "macos", feature = "macos"))]
        {
            Some(raw_window_handle::RawDisplayHandle::AppKit(
                raw_window_handle::AppKitDisplayHandle::new(),
            ))
        }
        #[cfg(not(all(target_os = "macos", feature = "macos")))]
        {
            None
        }
    }

    /// The window's content size, read from its content view.
    fn window_client_size(&self, window_id: u64) -> Option<(u32, u32)> {
        #[cfg(all(target_os = "macos", feature = "macos"))]
        {
            let host = resolve_host(window_id);
            if let Some(size) = super::native::window_client_size(host) {
                return Some(size);
            }
        }
        self.state.window_size(window_id)
    }

    /// Reports a container's new client size and queues a `Resized` trigger.
    ///
    /// Forwarded to the control backend, which owns the window and the queue the app polls;
    /// writing to this backend's own state would land in a store the host never reads.
    fn queue_resize_trigger(&self, window_id: u64, width: u32, height: u32) -> bool {
        #[cfg(not(alloc_frugal))]
        {
            crate::queue_resize_trigger(window_id, width, height)
        }
        #[cfg(alloc_frugal)]
        {
            let _ = (window_id, width, height);
            false
        }
    }

    /// The pasteboard's change counter, from `NSPasteboard.changeCount`.
    fn clipboard_generation(&self) -> u64 {
        #[cfg(all(target_os = "macos", feature = "macos"))]
        {
            crate::platform::clipboard_native::objc2_macos::generation()
        }
        #[cfg(not(all(target_os = "macos", feature = "macos")))]
        {
            0
        }
    }

    /// The window's safe-area insets, read from AppKit's `NSView.safeAreaInsets`.
    ///
    /// For a window the value comes from its content view, which is what this backend paints into
    /// and what AppKit lays out under the title bar or a notch. `None` off the main thread, without
    /// the `macos` feature, for an unknown id, or on a pre-11 AppKit that does not implement the
    /// selector — never the widget layer's portrait-phone default.
    fn safe_area_insets(&self, window_id: u64) -> Option<SafeAreaInsets> {
        #[cfg(all(target_os = "macos", feature = "macos"))]
        {
            if let Some(insets) = super::native::safe_area_insets(window_id) {
                return Some(insets);
            }
            #[cfg(not(alloc_frugal))]
            if let Some(host) = crate::widget::runtime::host_window_for(window_id) {
                if let Some(insets) = super::native::safe_area_insets(host) {
                    return Some(insets);
                }
            }
        }
        #[cfg(not(all(target_os = "macos", feature = "macos")))]
        {
            let _ = window_id;
        }
        None
    }

    /// The host integrations this backend actually provides.
    ///
    /// # Why `native_menu`, `ime` and `accessibility` are `false`
    ///
    /// This used to answer the same all-`true` set as `cocoa`, on the reasoning that
    /// "the objc2 preview honours the same four host integrations as the legacy macOS
    /// backend". It does not, and the mismatch was invisible because a capability flag
    /// is a *promise* no test had ever checked against the methods it promises.
    ///
    /// The evidence, per flag:
    ///
    /// * `native_menu` — this backend implements **no** menu method at all:
    ///   `create_menu_bar`, `create_menu`, `menu_add_item`,
    ///   `attach_menu_bar_to_window` and `poll_menu_triggered` all fall through to the
    ///   trait defaults, and its own test suite asserts
    ///   `create_menu_bar(window, ..) == 0` (`tests.rs`).
    /// * `ime` — `ime_bridge()` is now overridden on the AppKit-bound build
    ///   (`all(target_os = "macos", feature = "macos")`). The canvas view implements
    ///   `NSTextInputClient` and `keyDown:` routes through `interpretKeyEvents:`, so the flag
    ///   has a real producer (BLUE28 B28.12).
    /// * `accessibility` — `accessibility_bridge()` is likewise not overridden.
    /// * `dpi_scaling` — BLUE28 B28.03 added `dpi_scale_factor`, read from the main screen's
    ///   `backingScaleFactor`, so this is `true` exactly on the AppKit-bound build (`macos`).
    ///
    /// What the backend genuinely does provide is the state model plus real AppKit
    /// objects where `native` is bound, and both of those are reachable without any of
    /// these flags. Reporting `false` is the honest answer (principle #37) and it is
    /// the answer the capability negotiation then acts on: a host that reads
    /// `native_menu: false` builds a library-painted menu bar instead of asking for one
    /// that will not appear.
    ///
    /// `typed_widget_trigger` stays `true` because it is the one claim with a real
    /// producer: the backend inherits `BackendState`'s trigger queue and forwards it
    /// (see [`Self::poll_widget_trigger_event`]).
    fn capabilities(&self) -> PlatformCapabilities {
        PlatformCapabilities {
            dpi_scaling: cfg!(all(target_os = "macos", feature = "macos")),
            // `NSTextInputClient` on the canvas view, backed by the shared `ime_macos` state
            // machine (B28.12). A build without AppKit has no view to be a client.
            ime: cfg!(all(target_os = "macos", feature = "macos")),
            // A real `NSAccessibility` element per canvas view (B28.13).
            accessibility: cfg!(feature = "macos"),
            native_menu: false,
            typed_widget_trigger: true,
            // `NSPasteboard`, through the objc2 bindings — a build-time fact, so it is asked of
            // the target and feature rather than assumed. The module that holds the binding is
            // `clipboard_native::objc2_macos`, gated on exactly this pair, and the trait methods
            // above call it through the same gate, so the flag and the bodies cannot drift.
            clipboard: cfg!(all(target_os = "macos", feature = "macos")),
            // No handler spawn in this preview backend.
            open_external: false,
            // This preview backend presents no picker of its own; see
            // [`Platform::choose_path`] for what the flag promises and what a `false` costs a
            // host (it draws its own list instead).
            file_dialog: false,
            // `NSFontManager.availableFontFamilies` through the AppKit-bound build.
            system_fonts: cfg!(all(target_os = "macos", feature = "macos")),
        }
    }

    /// Reads installed physical memory via `sysconf(_SC_PHYS_PAGES)` in
    /// [`crate::platform::darwin_probes`].
    ///
    /// # Why this is gated, and what the other branch means
    ///
    /// `darwin_probes` needs `libc` symbols that only exist on an Apple target. The module used to
    /// carry a `target_vendor = "apple"` gate for this one reason, which made its **entire** test
    /// suite (`tests.rs`, 10 assertions that touch only the `Platform` trait interface) invisible on
    /// every other host — tests that never run are tests that cannot fail, which is the risk this
    /// crate has a standing rule about.
    ///
    /// So the probe is gated here instead, exactly as `platform/ime_macos.rs` gates its AppKit
    /// touch points. On a non-Apple host this backend cannot be *selected* (`platform/mod.rs` still
    /// requires `target_vendor = "apple"` — a build on Linux never constructs one), so the fallback
    /// is not a behaviour any user can reach; it exists so the type and its tests compile.
    fn total_memory_mb(&self) -> Option<u64> {
        #[cfg(target_vendor = "apple")]
        {
            crate::platform::darwin_probes::total_memory_mb()
        }
        #[cfg(not(target_vendor = "apple"))]
        {
            None
        }
    }

    /// Reports whether `pmset -g batt` says the machine is drawing from its battery.
    fn is_on_battery(&self) -> bool {
        #[cfg(target_vendor = "apple")]
        {
            crate::platform::darwin_probes::is_on_battery()
        }
        #[cfg(not(target_vendor = "apple"))]
        {
            false
        }
    }

    /// Samples RSS over VSZ for this process from `ps`, in
    /// [`crate::platform::darwin_probes`].
    fn process_memory_utilization(&self) -> Option<f32> {
        #[cfg(target_vendor = "apple")]
        {
            crate::platform::darwin_probes::process_memory_utilization()
        }
        #[cfg(not(target_vendor = "apple"))]
        {
            None
        }
    }

    /// CPU tick accounting has no lock-free Darwin source here, so this reports
    /// `None` rather than a fabricated figure.
    fn process_cpu_utilization(&self) -> Option<f32> {
        None
    }

    /// Submits the job to the unix print spooler via [`crate::platform::os_probes`].
    ///
    /// The `lpr`/`lp` clients this reaches are the same on Darwin and on Linux, so
    /// sharing the spooler runner with the Linux backends is correct — unlike the
    /// `/proc`-based *probes* above, which Apple kernels cannot serve.
    fn spawn_print_job(&self, job_file: &std::path::Path) -> Result<(), String> {
        crate::platform::os_probes::spawn_print_job(job_file)
    }

    /// macOS always ships CUPS, so the `lp`/`lpr` clients are present.
    fn has_print_support(&self) -> bool {
        crate::platform::types::unix_print_clients_available()
    }

    /// Mounts a library-painted widget onto a surface this host will present.
    ///
    /// # Why this backend must answer
    ///
    /// Every `WidgetKind` is painted by `src/widget/`, so no native view is created
    /// per control and the host owes exactly two things a widget cannot provide: a
    /// window and a drawing surface.
    ///
    /// # The presentation path
    ///
    /// This used to record bookkeeping only and hand the actual AppKit view to the
    /// sibling cocoa backend — which the objc2 backend never links, so a successfully
    /// "mounted" widget produced no pixels. It now creates its own surface: a
    /// [`RustWidgetsObjc2CanvasView`](super::native::RustWidgetsObjc2CanvasView) whose
    /// `drawRect:` pulls one RGBA frame from `widget::runtime` and blits it, mirroring
    /// `macos/canvas.rs::draw_rect`. The AppKit work stays in `native`; the shared
    /// `BackendState` record is kept so a later repaint request resolves the surface.
    ///
    /// Without it this backend inherited the trait's `false` default, so the preview
    /// backend reported "I cannot display a UI here" while the cocoa backend on the
    /// same machine reported `true`. Which answer a host got depended on a feature
    /// flag the host does not control, and a host that checks `supports_surfaces()`
    /// before building a UI would refuse to start for no stated reason.
    ///
    /// # Off the main thread
    ///
    /// AppKit may only be messaged from the process main thread, so off it this
    /// falls back to the same state-only record the cocoa backend uses for
    /// `create_window` (see `is_main_thread`'s contract): the surface is remembered
    /// and answered for, and the caller — a host that owns the AppKit loop — mounts
    /// the real view when it reaches the main thread. On the main thread there is no
    /// such fallback: the real view is created or the call reports `false`.
    fn mount_surface(&self, parent: ObjectId, id: ObjectId, rect: crate::core::Rect) -> bool {
        let recorded = self.state.mount_surface_record(id, rect);
        #[cfg(all(target_os = "macos", feature = "macos"))]
        {
            if super::native::on_main_thread() {
                return recorded
                    && super::native::mount_surface_native(
                        parent,
                        id,
                        rect.x,
                        rect.y,
                        rect.width,
                        rect.height,
                    );
            }
            log::debug!(
                "[macos-objc2] mount_surface: id={id} recorded off the main thread \
                 (state-only until AppKit is reachable)"
            );
        }
        // On a build without the objc2 feature there is no AppKit path to hand `parent` to; the
        // state-only record is the whole surface, and binding `parent` here keeps the signature the
        // same in both configurations without an unused-variable warning.
        #[cfg(not(all(target_os = "macos", feature = "macos")))]
        {
            let _ = parent;
        }
        recorded
    }

    /// Resizes the mounted surface `id` and asks AppKit to redraw it.
    ///
    /// `false` when `id` is not mounted. Off the main thread the state record is
    /// still updated (see [`Self::mount_surface`]) so a caller that mounts, resizes
    /// and invalidates before reaching the main thread keeps a consistent model.
    fn resize_surface(&self, id: ObjectId, rect: crate::core::Rect) -> bool {
        let recorded = self.state.resize_surface_record(id, rect);
        #[cfg(all(target_os = "macos", feature = "macos"))]
        {
            if super::native::on_main_thread() {
                return recorded
                    && super::native::resize_surface_native(
                        id,
                        rect.x,
                        rect.y,
                        rect.width,
                        rect.height,
                    );
            }
        }
        recorded
    }

    /// Releases a mounted surface.
    ///
    /// The shared `BackendState` record is the source of truth for "was this
    /// mounted?", so a second unmount reports absence rather than claiming another
    /// release. The native view is detached as well when AppKit is reachable.
    fn unmount_surface(&self, id: ObjectId) -> bool {
        let recorded = self.state.unmount_surface_record(id);
        #[cfg(all(target_os = "macos", feature = "macos"))]
        {
            if super::native::on_main_thread()
                && !super::native::unmount_surface_native(id)
                && recorded
            {
                // The state said it was mounted but there was no native view to
                // detach — a state-only mount from before the main thread. The state
                // release above is already the honest answer, so only note it.
                log::debug!("[macos-objc2] unmount_surface: id={id} had no native view to detach");
            }
        }
        recorded
    }

    /// Queues a repaint for the host to pick up. `false` when `id` is unknown.
    ///
    /// # Why a window id is accepted
    ///
    /// The library asks for the **window** to be repainted whenever one of its
    /// ordinary children changes (`widget::runtime::request_repaint_subtree`),
    /// because a window is what draws those children. A surface-only record
    /// answered `false` for such a request, so an event could be handled and the
    /// screen still never change — silently.
    ///
    /// # What it now does
    ///
    /// On macOS the request is driven into AppKit as well as recorded: a mounted
    /// surface's view is marked `setNeedsDisplay:YES`, and a window id marks its
    /// content view, so the repaint the library asked for actually reaches the
    /// screen instead of only updating backend bookkeeping.
    fn invalidate_surface(&self, id: ObjectId) -> bool {
        let recorded = self.state.record_repaint_request(id);
        #[cfg(all(target_os = "macos", feature = "macos"))]
        if super::native::on_main_thread() {
            // A mounted surface and a window are different native objects: try the
            // surface side table first, then fall back to the window registry.
            let invalidated = super::native::invalidate_surface_native(id)
                || super::native::invalidate_window_native(id);
            if !invalidated {
                log::debug!(
                    "[macos-objc2] invalidate_surface: id={id} recorded but has no native object"
                );
            }
        }
        recorded
    }

    /// Removes and returns the next widget awaiting a repaint.
    ///
    /// The drain half of [`Self::invalidate_surface`]: without it the queue would
    /// grow without bound and a host that pulls frames would never be told what to
    /// draw. Answers from the shared [`crate::platform::state::BackendState`] queue
    /// the record half fills.
    fn take_pending_repaint(&self) -> Option<ObjectId> {
        self.state.take_pending_repaint()
    }

    /// This backend displays library-painted widgets by handing the host their frames.
    ///
    /// See [`crate::platform::Platform::supports_surfaces`] for what this does and does
    /// not promise, and [`crate::platform::Platform::invalidate_surface`] for how a
    /// repaint request for a window (as opposed to a mounted surface) is answered.
    fn supports_surfaces(&self) -> bool {
        true
    }

    /// Renders menu accelerators with AppKit symbols (`⌘⇧Z`).
    fn shortcut_style(&self) -> crate::shortcut::PlatformShortcutStyle {
        crate::shortcut::PlatformShortcutStyle::Mac
    }
    fn init(&self) {
        // Marker keeps objc2 dependency wired even before native event-loop bridging lands.
        let _ = self.objc2_runtime_marker();
        // Bootstrap the shared NSApplication so native windows/menus the backend
        // creates are tracked by AppKit (finishLaunching installs the app).
        #[cfg(all(target_os = "macos", feature = "macos"))]
        let bootstrapped = super::native::bootstrap_ns_application();
        #[cfg(not(all(target_os = "macos", feature = "macos")))]
        let bootstrapped = false;
        if bootstrapped {
            log::debug!("[macos-objc2] NSApplication bootstrapped on the main thread");
        } else {
            log::debug!("[macos-objc2] NSApplication not bootstrapped (off-main or non-macOS host)")
        }
        // The display link and the system-colour observer are process-wide, so starting them
        // here (once, at init) is what makes `supports_vsync_frame` and the accent observer
        // answer for the whole session. CoreVideo may be unavailable (a headless host); the
        // fixed frame grid then keeps running and the check reports `false`.
        #[cfg(all(target_os = "macos", feature = "macos"))]
        {
            if crate::platform::display_link::start() {
                log::debug!("[macos-objc2] CoreVideo display link started");
            } else {
                log::debug!(
                    "[macos-objc2] CoreVideo display link unavailable; the fixed frame grid paces frames"
                );
            }
            if !super::environment::install_system_color_observer() {
                log::debug!(
                    "[macos-objc2] system colour observer not installed; the accent refreshes only on explicit request"
                );
            }
        }
        self.runtime.initialized.store(true, Ordering::SeqCst);
    }
    fn run(&self) {
        if !self.runtime.initialized.load(Ordering::SeqCst) {
            self.init();
        }
        // Preview backend uses a deterministic polling loop to preserve trait-level parity.
        self.runtime.running.store(true, Ordering::SeqCst);
        // The same loop drains the trigger queue, so a backend that reports a resize
        // behaves the same here as on a native one. See `crate::drain_triggers`.
        //
        // `crate::drive_frame` drains and then advances the animation bus; the drain alone
        // re-ran layout but left every animation inert, because nothing on this backend ran
        // the per-frame step (BLUE24 §0A.1 measurement 1).
        // The loop is paced by the wait below: it blocks in the native run loop when a native
        // window is available, so an idle window no longer wakes every frame.
        while self.runtime.running.load(Ordering::SeqCst) {
            // The run-loop wait is this backend's blocking primitive: pumping here is what
            // makes the native window draw and receive input, and waiting *inside* the run loop
            // keeps that input immediate while an idle window is not polled. The shared deadline
            // grid decides how long it may block, and `pump_native_event_until` reports whether
            // it dispatched an event before the timeout -- the early wake the grid keeps the
            // deadline across. The pump still runs with a zero timeout when a frame is already
            // due, so a long frame cannot starve input.
            #[cfg(all(target_os = "macos", feature = "macos"))]
            {
                let outcome = crate::drive_frame(FRAME_INTERVAL_MS as u32);
                crate::platform::frame_wait::pause_between_frames_with(
                    outcome.needs_another_frame,
                    super::native::pump_native_event_until,
                );
            }
            #[cfg(not(all(target_os = "macos", feature = "macos")))]
            {
                let outcome = crate::drive_frame(FRAME_INTERVAL_MS as u32);
                crate::platform::frame_wait::pause_between_frames(outcome.needs_another_frame);
            }
        }
    }
    fn quit(&self) -> bool {
        // The loop this backend runs reads `self.runtime.running` every tick, so a stop
        // request from any thread lands and is honoured; `true` reports that fact rather
        // than merely that the call was made.
        //
        // # Why "is my own loop running" decides the answer
        //
        // The frame tick under test can belong to a *different* platform object: the seam admits
        // a double, and a host that binds one has a ticking loop this backend never started. In
        // that case setting this backend's own flag changes nothing anyone reads, so the request
        // is recorded on the process-wide seam instead — that is the flag every loop honours. An
        // unconditional `true` told such a host its session would end while nothing was reading
        // the answer.
        if !self.runtime.running.load(Ordering::SeqCst) {
            return crate::platform::stop::request();
        }
        self.runtime.running.store(false, Ordering::SeqCst);
        // The loop may be parked in `pause_between_frames`; wake it so the stop is immediate.
        crate::platform::frame_wait::wake();
        // The display link is process-wide and its callback thread outlives the loop otherwise.
        // Stopping it is therefore part of the teardown — but only on the thread that owns the
        // AppKit session: `CVDisplayLinkStop` waits for the callback thread and expects the main
        // thread, so a stop requested from a worker would block the very caller that asked the
        // session to end. Off the main thread the link is left running and dies with the process.
        #[cfg(all(target_os = "macos", feature = "macos"))]
        if objc2::MainThreadMarker::new().is_some() {
            crate::platform::display_link::stop();
        }
        true
    }
    /// Pops the next typed widget-trigger event from this backend's queue.
    ///
    /// # Why this backend must delegate
    ///
    /// [`PlatformCapabilities::typed_widget_trigger`] claims this preview backend produces and
    /// delivers typed triggers, and it holds a `BackendState` (the shared queue) like every other
    /// state backend — but neither method was overridden, so both answered the trait defaults while
    /// the flag said `true`. `NativeControlBackend` forwards to `get_platform()`, so an injected
    /// trigger was accepted and silently dropped.
    fn poll_widget_trigger_event(&self) -> Option<crate::platform::WidgetTriggerEvent> {
        self.state.pop_widget_trigger_event()
    }

    /// Pushes a typed widget-trigger event, refusing ids this backend never made.
    fn inject_widget_trigger_event(
        &self,
        widget_id: ObjectId,
        kind: crate::platform::WidgetTriggerKind,
    ) -> bool {
        self.state.inject_widget_trigger_event(widget_id, kind)
    }

    /// Pops the next pending trigger as a bare id, over the same queue as the typed view.
    fn poll_widget_triggered(&self) -> Option<ObjectId> {
        self.poll_widget_trigger_event().map(|event| event.widget_id)
    }

    fn destroy_widget(&self, widget_id: ObjectId) -> bool {
        // Native teardown runs **before** the logical state is dropped, so that a
        // main-thread refusal can be reported without having already lost the handle.
        //
        // A window must be closed (and taken off screen) rather than merely detached:
        // `removeFromSuperview` on an `NSWindow` does nothing useful, so a logically
        // destroyed window could stay visible. `destroy_native_handle` distinguishes the
        // two by the object's own runtime class, so it does not need the pre-destroy kind.
        //
        // # Why the result decides whether the state is dropped
        //
        // Off the AppKit main thread the native call is refused. Previously the order was
        // reversed and the refusal was ignored: the state record was deleted first and the
        // call returned `existed`, so the window stayed on screen while the caller had no
        // id left to retry with (D08-P-02). Now a refusal keeps every bookkeeping entry and
        // reports failure, so a main-thread retry can finish the teardown.
        #[cfg(all(target_os = "macos", feature = "macos"))]
        {
            match super::native::destroy_native_handle(widget_id) {
                super::native::NativeDestroyOutcome::RefusedOffMainThread => {
                    log::warn!(
                        "[macos-objc2] destroy_widget: native teardown refused off the main \
                         thread; keeping the widget {widget_id} so it can be retried there"
                    );
                    return false;
                }
                super::native::NativeDestroyOutcome::Destroyed
                | super::native::NativeDestroyOutcome::NoNativeObject => {}
            }
        }

        let existed = self.state.destroy_widget(widget_id);

        // Drop every bookkeeping entry that names this widget, including any queued
        // trigger events that would otherwise fire for a widget that no longer
        // exists.
        let mut menus = crate::compat::lock(&self.menus);
        menus.attached_menu_bar.retain(|_window, menu_bar| *menu_bar != widget_id);
        menus.menu_children.remove(&widget_id);
        menus.menu_item_shortcuts.remove(&widget_id);
        menus.pending_menu_events.retain(|queued| *queued != widget_id);
        menus.pending_widget_events.retain(|event| event.widget_id != widget_id);
        drop(menus);

        existed
    }

    // ---- Window creation ----
    /// Create a new window with the given title and geometry.
    /// This is the entry point for window lifecycle parity tests.
    /// Returns a unique window id.
    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        // Insert window widget into backend state
        let id = self.insert_widget(MacObjc2HandleKind::Window, title, x, y, width, height);

        #[cfg(all(target_os = "macos", feature = "macos"))]
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            let window = super::native::create_ns_window(mtm, id, title, x, y, width, height);
            super::native::store_native_view(id, &*window as *const _ as *mut std::ffi::c_void);
        }

        id
    }

    // ---- Clipboard & Drag-Drop ----
    //
    // `NSPasteboard` through the objc2 backend's own implementation
    // (`clipboard_native::objc2_macos`, which binds `NSPasteboard` with objc2 rather than the
    // `cocoa`/`objc` crates the legacy backend uses). Each `#[cfg]` block below is removed in the
    // build it does not describe, which leaves exactly one as the method's tail expression; on a
    // host without AppKit the in-process `BackendState` record remains the answer.
    fn set_clipboard_text(&self, text: &str) -> bool {
        #[cfg(all(target_os = "macos", feature = "macos"))]
        {
            if crate::platform::clipboard_native::objc2_macos::on_main_thread() {
                // The pasteboard's own answer, confirmed by a read-back (`writeObjects:` is the
                // system write; building the item is not).
                return crate::platform::clipboard_native::objc2_macos::write_text(text);
            }
            // Off the AppKit main thread the pasteboard may not be touched, and widgets do call
            // this from worker threads (a background copy). The in-process record keeps the call
            // useful there, which is exactly the rule the cocoa backend follows.
            self.state.set_clipboard_text(text)
        }
        #[cfg(not(all(target_os = "macos", feature = "macos")))]
        {
            self.state.set_clipboard_text(text)
        }
    }

    fn get_clipboard_text(&self) -> String {
        #[cfg(all(target_os = "macos", feature = "macos"))]
        {
            if crate::platform::clipboard_native::objc2_macos::on_main_thread() {
                return crate::platform::clipboard_native::objc2_macos::read_text()
                    .unwrap_or_default();
            }
            self.state.clipboard_text()
        }
        #[cfg(not(all(target_os = "macos", feature = "macos")))]
        {
            self.state.clipboard_text()
        }
    }

    /// The pasteboard's text, or `None` when it holds none.
    ///
    /// Overridden because the trait default derives it from [`Self::get_clipboard_text`], and an
    /// empty string on the host path means "the pasteboard holds an empty string" — a real answer
    /// the default would report as "no text". Off the main thread (where the pasteboard may not be
    /// touched) the record is the source, exactly as `get_clipboard_text` has it.
    fn clipboard_text(&self) -> Option<String> {
        #[cfg(all(target_os = "macos", feature = "macos"))]
        {
            if crate::platform::clipboard_native::objc2_macos::on_main_thread() {
                return crate::platform::clipboard_native::objc2_macos::read_text();
            }
            let text = self.state.clipboard_text();
            if text.is_empty() {
                None
            } else {
                Some(text)
            }
        }
        #[cfg(not(all(target_os = "macos", feature = "macos")))]
        {
            let text = self.state.clipboard_text();
            if text.is_empty() {
                None
            } else {
                Some(text)
            }
        }
    }

    /// The host pasteboard, or `None` when this build has no AppKit path.
    fn clipboard_backend(&self) -> Option<&dyn crate::platform::clipboard::RichClipboardBackend> {
        #[cfg(all(target_os = "macos", feature = "macos"))]
        {
            // A unit struct, so the backend needs no `OnceLock`: there is nothing to initialise.
            static CLIPBOARD: crate::platform::clipboard_native::objc2_macos::MacOsObjc2Clipboard =
                crate::platform::clipboard_native::objc2_macos::MacOsObjc2Clipboard;
            Some(&CLIPBOARD)
        }
        #[cfg(not(all(target_os = "macos", feature = "macos")))]
        {
            None
        }
    }
}
