// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! iOS platform trait implementation.
//!
//! Implements the `Platform` contract for iOS mobile devices. This is a
//! state-driven backend: every widget is recorded in `BackendState<IosHandleKind>`
//! and the trait defaults cover anything the host cannot supply.
//!
//! # BLUE15: the host no longer builds UIKit controls
//!
//! This backend used to instantiate a real `UIView` per logical widget —
//! `UIButton`, `UILabel`, `UISwitch`, `UITextField`, `UITableView`, … — and then
//! forward every state mutation to the live object. Under the self-drawn strategy
//! that is exactly the duplication BLUE15 removes: the library paints every
//! `WidgetKind`, and the host owes the widget layer a **window** and a **drawing
//! surface** (rules #55/#56). A per-kind `create_*` here had no OS object to map
//! onto, so the control creators and the view registry that existed only to serve
//! them are gone (rule #59: delete means delete).
//!
//! What survives is the platform-facing part the library cannot replace: the
//! window lifecycle, the runtime loop, `UIPrintInteractionController`-backed facts
//! reported honestly (see `spawn_print_job`), the clipboard, IME flags,
//! accessibility names, and the injectable menu / widget-trigger queues.
//!
//! ## Menu / status-bar semantics (iOS)
//!
//! iOS has no desktop `MenuBar`/`StatusBar` chrome. These handles are modelled
//! as **in-process data**: kind-constrained parents, textual payload, and
//! injectable trigger events. They are intentionally *not* advertised as native
//! menu capability (`capabilities().native_menu == false`) and never attached to
//! a UIKit view. Apps that need UIKit contextual menus should build them from the
//! same data instead of treating these handles as native menus.

use super::types::{IosHandleKind, IosMobilePlatform};
use crate::compat::atomic::Ordering;
#[cfg(not(all(apple_uikit, feature = "ios-uikit-ffi")))]
use crate::compat::format;
use crate::compat::{lock, String, Vec};
use crate::core::PlatformFamily;
use crate::platform::clipboard::RichClipboardBackend;
use crate::platform::types::{Display, DisplayId};
use crate::platform::{Platform, PlatformCapabilities, WidgetTriggerEvent, WidgetTriggerKind};

/// The frame interval this backend's loop runs at, in milliseconds.
///
/// The same value as every other backend's twin constant, and named here for the same
/// reason: the sleep between iterations and the delta handed to [`crate::drive_frame`]
/// must be the same number, or every transition runs at the ratio between them with
/// nothing to notice the mismatch. A backend that later runs at the display's own rate
/// changes exactly this value.
const FRAME_INTERVAL_MS: u64 = crate::platform::types::FRAME_INTERVAL_MS;

impl Platform for IosMobilePlatform {
    // The uniform widget-property methods are answered once, over `self.state`, by the
    // shared expansion in `platform::state_impl` rather than re-written per backend.
    crate::impl_platform_state_properties!();

    fn as_any(&self) -> &dyn crate::compat::Any {
        self
    }

    fn backend_name(&self) -> &'static str {
        "ios-state-backend"
    }

    fn family(&self) -> PlatformFamily {
        PlatformFamily::Mobile
    }

    /// Reads installed physical memory via `sysconf(_SC_PHYS_PAGES)` in
    /// [`crate::platform::darwin_probes`]. iOS does not mount `/proc/meminfo`.
    ///
    /// # Why the probe is gated rather than the whole module
    ///
    /// `darwin_probes` needs `libc` symbols that only exist on an Apple target, and that one fact
    /// used to cost this module everything: `platform_impl.rs` carries the `Platform` impl, so
    /// gating *it* also hid [`super::tests`] — 9 assertions that exercise only the trait interface
    /// and touch no UIKit — from every non-Apple host. Gating the three probe bodies here is the
    /// same fix `platform/ime_macos.rs` applies to its AppKit touch points, and it keeps the
    /// selection rule where it belongs (`platform/mod.rs` still requires an Apple target for the
    /// backend to be chosen).
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

    /// The main screen's logical-to-pixel scale, or `1.0` when UIKit cannot be asked.
    ///
    /// `UIScreen.scale` is the value UIKit uses for Points-to-pixels; on a 2x/3x device this is
    /// 2.0/3.0 rather than the fabricated `1.0` the trait default used to report. The value is
    /// `1.0` off the main thread (UIKit is main-thread-only) or without the UIKit FFI, and
    /// [`Self::capabilities`] reports `dpi_scaling: false` on exactly the build that has no FFI.
    fn dpi_scale_factor(&self) -> f32 {
        #[cfg(all(apple_uikit, feature = "ios-uikit-ffi"))]
        {
            super::native::main_screen_scale().unwrap_or(1.0)
        }
        #[cfg(not(all(apple_uikit, feature = "ios-uikit-ffi")))]
        {
            1.0
        }
    }

    /// The screens UIKit reports.
    ///
    /// `UIScreen` is main-thread-only: a call from another thread answers the honest absence
    /// instead of touching UIKit. Without the UIKit FFI (`ios-uikit-ffi`) there is no `UIScreen`
    /// binding at all, so the trait's empty list is the truthful answer.
    fn displays(&self) -> Vec<Display> {
        #[cfg(all(apple_uikit, feature = "ios-uikit-ffi"))]
        {
            super::native::displays()
        }
        #[cfg(not(all(apple_uikit, feature = "ios-uikit-ffi")))]
        {
            Vec::new()
        }
    }

    /// The screen UIKit placed this window on.
    ///
    /// UIKit creates the app's window with a real `UIWindow`, and `UIWindow.screen()` is the
    /// authority for which `UIScreen` it is on (winit's `current_monitor()` reads the same
    /// property). The id a host holds is the widget-registry id, so it is translated first; a
    /// caller that passes the platform id directly is honoured too. Without the UIKit FFI, off
    /// the main thread, or for an id with no recorded window, the answer is `None` rather than a
    /// guessed primary screen.
    fn display_of_window(&self, window_id: u64) -> Option<DisplayId> {
        let host = host_window_of(self, window_id)?;
        #[cfg(all(apple_uikit, feature = "ios-uikit-ffi"))]
        {
            if let Some(id) = super::native::screen_id_of_window(host) {
                return Some(DisplayId::new(id));
            }
        }
        // No UIKit FFI: `host` proved the id names a window, but there is no `UIWindow.screen()`
        // to read, so the honest answer is still `None`.
        #[cfg(not(all(apple_uikit, feature = "ios-uikit-ffi")))]
        {
            let _ = host;
        }
        None
    }

    /// The window's own safe-area insets, read from the recorded `UIWindow`.
    ///
    /// `UIView.safeAreaInsets` is the authority UIKit gives an app for "how much of my window is
    /// covered by the notch, the status bar and the home indicator" — the same fact winit exposes
    /// as `Window::safe_area()`. It is main-thread-only and needs the recorded `UIWindow`, so a
    /// window id that does not resolve, a non-window id, a call off the main thread, or a build
    /// without the UIKit FFI all answer `None` ("cannot tell") rather than the widget's default
    /// insets, which describe a portrait phone and would fabricate a notch on an iPad.
    fn safe_area_insets(&self, window_id: u64) -> Option<crate::platform::types::SafeAreaInsets> {
        let host = host_window_of(self, window_id)?;
        #[cfg(all(apple_uikit, feature = "ios-uikit-ffi"))]
        {
            return super::native::safe_area_insets(host);
        }
        #[cfg(not(all(apple_uikit, feature = "ios-uikit-ffi")))]
        {
            let _ = host;
            None
        }
    }

    /// The system appearance UIKit reports for the current traits.
    ///
    /// Read from `UITraitCollection.currentTraitCollection().userInterfaceStyle`, which is the
    /// same source winit's UIKit backend consults for `system_theme()` (it answers from the trait
    /// environment). Main-thread-only; `None` off the main thread or without the UIKit FFI.
    fn system_theme(&self) -> Option<bool> {
        #[cfg(all(apple_uikit, feature = "ios-uikit-ffi"))]
        {
            super::native::system_theme()
        }
        #[cfg(not(all(apple_uikit, feature = "ios-uikit-ffi")))]
        {
            None
        }
    }

    /// Whether the user asked for increased contrast, from UIKit.
    fn high_contrast(&self) -> Option<bool> {
        #[cfg(all(apple_uikit, feature = "ios-uikit-ffi"))]
        {
            Some(super::native::high_contrast())
        }
        #[cfg(not(all(apple_uikit, feature = "ios-uikit-ffi")))]
        {
            None
        }
    }

    /// Whether the user asked for reduced motion, from UIKit.
    fn reduce_motion(&self) -> Option<bool> {
        #[cfg(all(apple_uikit, feature = "ios-uikit-ffi"))]
        {
            Some(super::native::reduce_motion())
        }
        #[cfg(not(all(apple_uikit, feature = "ios-uikit-ffi")))]
        {
            None
        }
    }

    /// The user's locale, from `NSLocale.currentLocale`.
    fn system_locale(&self) -> Option<&'static str> {
        #[cfg(all(apple_uikit, feature = "ios-uikit-ffi"))]
        {
            super::native::system_locale()
        }
        #[cfg(not(all(apple_uikit, feature = "ios-uikit-ffi")))]
        {
            None
        }
    }

    /// The user's preferred text size, mapped from `UIContentSizeCategory`.
    ///
    /// BLUE28 B28.10: the category is a name, not a multiplier, so the mapping lives in
    /// [`super::native::text_scale`]. `None` off the main thread or without the UIKit FFI; the
    /// trait's `1.0` is the honest absence in those builds.
    fn text_scale(&self) -> f32 {
        #[cfg(all(apple_uikit, feature = "ios-uikit-ffi"))]
        {
            super::native::text_scale().unwrap_or(1.0)
        }
        #[cfg(not(all(apple_uikit, feature = "ios-uikit-ffi")))]
        {
            1.0
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

    /// The font families UIKit reports; empty without the UIKit FFI.
    fn system_fonts(&self) -> Vec<crate::platform::types::SystemFont> {
        #[cfg(all(apple_uikit, feature = "ios-uikit-ffi"))]
        {
            super::native::system_fonts()
        }
        #[cfg(not(all(apple_uikit, feature = "ios-uikit-ffi")))]
        {
            Vec::new()
        }
    }

    /// A revision of the current screen configuration.
    ///
    /// Derived from the same fields [`Self::displays`] returns, so a host that compares two
    /// readings re-reads the list exactly when a screen was added/removed or its bounds or scale
    /// changed. `0` means UIKit could not be read (off the main thread, or no UIKit FFI).
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

    /// Reports whether `pmset -g batt` says the device is drawing from its battery.
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

    /// iOS printing goes through `UIPrintInteractionController`, not a spooler
    /// command, so this state backend cannot submit a job file.
    fn spawn_print_job(&self, job_file: &std::path::Path) -> Result<(), String> {
        #[cfg(all(apple_uikit, feature = "ios-uikit-ffi"))]
        {
            super::native::spawn_print_job(job_file)
        }
        #[cfg(not(all(apple_uikit, feature = "ios-uikit-ffi")))]
        {
            Err(format!(
                "iOS printing requires the UIKit FFI; job file '{}' was not printed",
                job_file.display()
            ))
        }
    }

    /// `UIPrintInteractionController` is part of UIKit, so the capability follows the FFI build.
    fn has_print_support(&self) -> bool {
        #[cfg(all(apple_uikit, feature = "ios-uikit-ffi"))]
        {
            super::native::has_print_support()
        }
        #[cfg(not(all(apple_uikit, feature = "ios-uikit-ffi")))]
        {
            false
        }
    }

    /// iOS uses the same AppKit-style accelerator symbols as macOS (`⌘⇧Z`).
    fn shortcut_style(&self) -> crate::shortcut::PlatformShortcutStyle {
        crate::shortcut::PlatformShortcutStyle::Mac
    }

    #[cfg(feature = "mobile-api")]
    fn mobile_extension(&self) -> Option<&dyn crate::platform::types::MobilePlatformExtension> {
        Some(self)
    }

    /// The iOS host's marked-text/commit bridge (B28.12).
    ///
    /// The platform-independent `ime_macos` state machine is reused; `focus_in`/`focus_out` come
    /// from the widget runtime, and the host forwards composition and commit through
    /// `rw_ios_ime_marked_text` / `rw_ios_ime_commit`.
    fn ime_bridge(&self) -> Option<&dyn crate::platform::ime::ImeBridge> {
        Some(super::host_facts::ime_bridge())
    }

    /// Capabilities published by the iOS backend.
    ///
    /// # Why `accessibility` stays `false`
    ///
    /// The backend publishes no `UIAccessibility` element for its painted surface, so a `true`
    /// here would promise a screen-reader tree that does not exist. `set_widget_accessibility_name`
    /// is not a counter-argument: it writes a per-widget field in `BackendState`, which is a flag
    /// store, not an accessibility node. A flag names the capability, not the storage of a
    /// boolean about it.
    ///
    /// `dpi_scaling` follows the UIKit FFI because `dpi_scale_factor()` reads `UIScreen.scale`;
    /// `ime` is `true` because `ime_bridge()` is implemented over the shared marked-text state
    /// machine and the host feeds it through the C entry points (B28.12). `typed_widget_trigger`
    /// stays `true` — this backend implements the queue (`poll_widget_trigger_event` et al.).
    fn capabilities(&self) -> PlatformCapabilities {
        PlatformCapabilities {
            // The UIKit FFI can read `UIScreen.scale`; without it (a host preview build, or an
            // iOS build that did not enable the FFI) there is no measurement to promise.
            dpi_scaling: cfg!(all(apple_uikit, feature = "ios-uikit-ffi")),
            // The state machine is always compiled; the host feeds it through
            // `rw_ios_ime_marked_text`/`rw_ios_ime_commit` (B28.12).
            ime: true,
            accessibility: false,
            native_menu: false,
            typed_widget_trigger: true,
            // `UIPasteboard`, through the UIKit FFI — which is a *build-time* fact, so it is
            // asked of the feature rather than assumed. Without `ios-uikit-ffi` the clipboard
            // bodies below fall back to the in-process record, which is not a host clipboard,
            // and this reports `false` for exactly that build.
            //
            // The flag is a promise that escapes the process (`rw_platform_capabilities`), and
            // `set_clipboard_text`/`get_clipboard_text` are the methods behind it, so the pair
            // moves together: no path, no claim.
            clipboard: cfg!(all(apple_uikit, feature = "ios-uikit-ffi")),
            // No handler spawn on iOS: opening a URL goes through `UIApplication`, which needs
            // the app's own delegate, and this backend has no window scene to hang it on.
            open_external: false,
            // No picker: `UIDocumentPickerViewController` needs the same window scene the handler
            // above lacks, and this backend presents no dialog of its own.
            file_dialog: false,
            // CoreText through the UIKit FFI names each family's backing file (B28.39).
            system_fonts: cfg!(all(apple_uikit, feature = "ios-uikit-ffi")),
        }
    }

    fn init(&self) {
        let _ = self.ios_runtime_marker();
        self.runtime.initialized.store(true, Ordering::SeqCst);
    }

    fn run(&self) {
        if !self.runtime.initialized.load(Ordering::SeqCst) {
            self.init();
        }
        // iOS state backend uses polling loop for deterministic behavior.
        self.runtime.running.store(true, Ordering::SeqCst);
        // The loop is also the drain tick: the app receives resize reports through
        // `queue_resize_trigger`, and nothing else reads that queue. See
        // `crate::drain_triggers`.
        //
        // `crate::drive_frame` is what drains *and* advances the animation bus. The drain
        // alone re-ran layout but left every animation on this platform unreachable, since
        // nothing else runs the per-frame step (BLUE24 §0A.1 measurement 1).
        while self.runtime.running.load(Ordering::SeqCst) {
            let outcome = crate::drive_frame(FRAME_INTERVAL_MS as u32);
            // Block when no frame is owed instead of waking every 16 ms. A posted event, a stop
            // request, or scheduled cross-thread work wakes the loop immediately.
            crate::platform::frame_wait::pause_between_frames(outcome.needs_another_frame);
        }
    }

    fn quit(&self) -> bool {
        // The loop this backend runs reads `self.runtime.running` every tick, so a stop
        // request from any thread lands and is honoured; `true` reports that fact rather
        // than merely that the call was made.
        self.runtime.running.store(false, Ordering::SeqCst);
        // The loop may be parked in `pause_between_frames`; wake it so the stop is immediate.
        crate::platform::frame_wait::wake();
        true
    }

    fn destroy_widget(&self, widget_id: u64) -> bool {
        // The state record is the authority on whether the widget existed.
        let existed = self.state.destroy_widget(widget_id);

        // Release the UIKit handle recorded for `display_of_window`. A destroyed window must
        // not stay reachable through this backend's registry.
        #[cfg(all(apple_uikit, feature = "ios-uikit-ffi"))]
        super::native::forget_ui_window(widget_id);

        // Drop the per-widget list storage for both list-backed widgets. Each lock
        // is released at the end of its statement so no two guards are held at once.

        // Drop the menu bookkeeping that names this widget: attached menu-bar
        // ownership, membership in a parent menu's child list, and any queued
        // trigger that would otherwise fire for a widget that no longer exists.
        let mut menus = lock(&self.menus);
        menus.attached_menu_bar.retain(|_window, menu_bar| *menu_bar != widget_id);
        let destroyed_children = menus.menu_children.remove(&widget_id);
        menus.menu_children.retain(|_parent, children| {
            children.retain(|child| *child != widget_id);
            !children.is_empty()
        });
        menus.pending_menu_events.retain(|queued| *queued != widget_id);
        drop(menus);

        // A destroyed `Menu` owns child menu items that were only reachable
        // through it; cascade the teardown so those items are not orphaned.
        for child in destroyed_children.into_iter().flatten() {
            self.destroy_widget(child);
        }

        existed
    }

    // ─── Window ───

    /// Creates the host window and records it.
    ///
    /// The window is the one OS object this backend creates. Under self-drawing the
    /// *controls* are the library's job (BLUE15 #55/#56), but a window is not: iOS
    /// requires a real `UIWindow` to have a place to draw into at all, which is why
    /// `native::create_ui_window` exists.
    ///
    /// Calling it is what makes the FFI helper live rather than orphaned. It is
    /// gated on the real iOS target, because the helper is compiled only there; the
    /// backend's state machine still runs on every host so its tests stay
    /// executable, and that is why the call is inside a `cfg` rather than around it.
    /// The UIKit handle is deliberately **not** stored: no control mutator needs it,
    /// and the library paints through the surface attached via
    /// `MobilePlatformExtension::attach_to_native_view`.
    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        let id = self.insert_widget(IosHandleKind::Window, title, x, y, width, height);

        #[cfg(all(apple_uikit, feature = "ios-uikit-ffi"))]
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            // The window must exist for the surface to have a superview. `create_ui_window`
            // records the handle under the platform id so `display_of_window` can read the
            // screen UIKit actually placed it on (winit's `UIWindow.screen()` lookup).
            let _window = super::native::create_ui_window(id, mtm, title, x, y, width, height);
        }

        id
    }

    // ─── Host clipboard ───
    //
    // `UIPasteboard` when the UIKit FFI is compiled in (`ios-uikit-ffi`), the in-process
    // `BackendState` record otherwise. Each `#[cfg]` block is removed in the build it does not
    // describe, which leaves exactly one as the body's tail expression — so the method keeps
    // **one** signature for one capability, and the "which host is this" fact stays here rather
    // than in an `Option` the caller has to read.
    fn set_clipboard_text(&self, text: &str) -> bool {
        #[cfg(all(apple_uikit, feature = "ios-uikit-ffi"))]
        {
            // The pasteboard's own answer: `setString:` reports nothing, so `native` confirms the
            // write by reading it back. A refused write is reported as refused rather than
            // mirrored into the record, which is the difference between a host clipboard and a
            // convincing imitation of one.
            super::native::pasteboard_set_text(text)
        }
        #[cfg(not(all(apple_uikit, feature = "ios-uikit-ffi")))]
        {
            self.state.set_clipboard_text(text)
        }
    }

    fn get_clipboard_text(&self) -> String {
        #[cfg(all(apple_uikit, feature = "ios-uikit-ffi"))]
        {
            // `""` for "the pasteboard holds no text": the `String` accessor's contract collapses
            // that with "holds an empty string" (see its own documentation).
            super::native::pasteboard_text().unwrap_or_default()
        }
        #[cfg(not(all(apple_uikit, feature = "ios-uikit-ffi")))]
        {
            self.state.clipboard_text()
        }
    }

    /// The pasteboard's text, or `None` when it holds none.
    ///
    /// Overridden because the trait default derives it from [`Self::get_clipboard_text`], and on
    /// the host path the empty string means "the pasteboard holds text that is empty" — which the
    /// default would then report as "no text". `Some("")` is a real answer here (`hasStrings()`
    /// is satisfied), and the paste-path caller branches on the difference.
    fn clipboard_text(&self) -> Option<String> {
        #[cfg(all(apple_uikit, feature = "ios-uikit-ffi"))]
        {
            super::native::pasteboard_text()
        }
        #[cfg(not(all(apple_uikit, feature = "ios-uikit-ffi")))]
        {
            // No host path: the in-process record is the only source, and an empty one is
            // indistinguishable from "no text" — the honest reading of that source.
            let text = self.state.clipboard_text();
            if text.is_empty() {
                None
            } else {
                Some(text)
            }
        }
    }

    /// The host pasteboard backend, or `None` when this build has no UIKit path.
    ///
    /// Returning `None` is what the trait documents for a backend whose only clipboard is an
    /// in-process record: a host that asks before offering a paste action must be told the truth.
    fn clipboard_backend(&self) -> Option<&dyn RichClipboardBackend> {
        #[cfg(all(apple_uikit, feature = "ios-uikit-ffi"))]
        {
            static CLIPBOARD: std::sync::OnceLock<super::native::IosClipboard> =
                std::sync::OnceLock::new();
            Some(CLIPBOARD.get_or_init(super::native::IosClipboard::new))
        }
        #[cfg(not(all(apple_uikit, feature = "ios-uikit-ffi")))]
        {
            None
        }
    }

    // ─── Menu Bar / Menu / Menu Item ───
    //
    // iOS has no desktop menu chrome, and no native menu protocol either. These
    // handles are an in-process data model: MenuBar is owned by a Window, Menu
    // belongs to a MenuBar/Menu, and MenuItem belongs to a Menu. Triggers are
    // delivered through the injectable `pending_menu_events` queue, so the library's
    // own menu widget can report activations; `capabilities().native_menu` stays
    // false, because no OS menu object is created.
    //
    // They survive the self-drawing change for the same reason the window does: the
    // host owns the menu *identity* an OS menu surface would need, while the library
    // paints the menu *appearance*.

    fn create_menu_bar(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if !matches!(self.kind_of(parent), Some(IosHandleKind::Window)) {
            return 0;
        }
        self.insert_widget(IosHandleKind::MenuBar, "MenuBar", x, y, width, height)
    }

    fn create_menu(&self, parent: u64, text: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        // A menu hangs off a menu bar or another menu.
        if !matches!(self.kind_of(parent), Some(IosHandleKind::MenuBar) | Some(IosHandleKind::Menu))
        {
            return 0;
        }
        self.insert_widget(IosHandleKind::Menu, text, x, y, width, height)
    }

    fn attach_menu_bar_to_window(&self, window: u64, menu_bar: u64) -> bool {
        if !matches!(self.kind_of(window), Some(IosHandleKind::Window)) {
            return false;
        }
        if !matches!(self.kind_of(menu_bar), Some(IosHandleKind::MenuBar)) {
            return false;
        }
        let mut menus = lock(&self.menus);
        menus.attached_menu_bar.insert(window, menu_bar);
        true
    }

    fn menu_add_item(&self, parent_menu: u64, text: &str, _shortcut: Option<&str>) -> u64 {
        if !matches!(self.kind_of(parent_menu), Some(IosHandleKind::Menu)) {
            return 0;
        }
        let id = self.insert_widget(IosHandleKind::MenuItem, text, 0, 0, 0, 0);

        let mut menus = lock(&self.menus);
        menus.menu_children.entry(parent_menu).or_default().push(id);

        id
    }

    fn poll_menu_triggered(&self) -> Option<u64> {
        lock(&self.menus).pending_menu_events.pop_front()
    }

    fn inject_menu_trigger(&self, menu_item_id: u64) -> bool {
        if !matches!(self.kind_of(menu_item_id), Some(IosHandleKind::MenuItem)) {
            return false;
        }
        lock(&self.menus).pending_menu_events.push_back(menu_item_id);
        true
    }

    // ─── Widget Trigger Events ───

    fn poll_widget_triggered(&self) -> Option<u64> {
        self.poll_widget_trigger_event().map(|event| event.widget_id)
    }

    fn poll_widget_trigger_event(&self) -> Option<WidgetTriggerEvent> {
        self.state.pop_widget_event()
    }

    fn inject_widget_trigger_event(&self, widget_id: u64, kind: WidgetTriggerKind) -> bool {
        if self.kind_of(widget_id).is_none() {
            return false;
        }
        self.state.push_widget_event(WidgetTriggerEvent { widget_id, kind });
        true
    }

    /// Mounts a library-painted widget onto a surface this host will present.
    ///
    /// The backend keeps no native object per control (every `WidgetKind` is painted
    /// by `src/widget/`), so the surface is a record plus a repaint queue: the UIKit
    /// side owns the pixels and pulls them with the render API, and this tells it
    /// which widgets exist and when they went stale.
    fn mount_surface(&self, _parent: u64, id: u64, rect: crate::core::Rect) -> bool {
        self.state.mount_surface_record(id, rect)
    }

    /// Updates the rect of a mounted surface. `false` when `id` is not mounted.
    fn resize_surface(&self, id: u64, rect: crate::core::Rect) -> bool {
        self.state.resize_surface_record(id, rect)
    }

    /// Releases a mounted surface.
    fn unmount_surface(&self, id: u64) -> bool {
        self.state.unmount_surface_record(id)
    }

    /// The window's current client size, as last reported by the host.
    ///
    /// Falls back to the size the window was created with. `None` for an id this backend
    /// does not know, so a caller can tell "no such window" from "a size I can use".
    fn window_client_size(&self, window_id: u64) -> Option<(u32, u32)> {
        // Ask the control backend, which owns the window and is therefore the only
        // store that knows the size a resize reported.
        crate::window_client_size(window_id).or_else(|| self.state.window_size(window_id))
    }

    /// Reports a container's new client size and queues a `Resized` trigger.
    ///
    /// # Who calls this on iOS
    ///
    /// The **host**, not this backend. This platform has no resize callback of its own:
    /// a `UIWindow` fills its scene and the size change arrives at the host's view
    /// controller (`viewDidLayoutSubviews`) or at its
    /// `UIDevice.orientationDidChangeNotification` observer, neither of which the library
    /// sees — it has no window delegate and deliberately stores no UIKit handle (see
    /// `create_window`). Fabricating a subscription here would mean holding a UIKit object
    /// the module documents as not holding.
    ///
    /// So an iOS host that wants its layout to follow the screen reports the new size here
    /// (or through the crate-level [`crate::queue_resize_trigger`], which is the same
    /// call). A host that does not is not broken: its window keeps the size it was created
    /// with, which is what `window_client_size` falls back to.
    fn queue_resize_trigger(&self, window_id: u64, width: u32, height: u32) -> bool {
        // Forward to the control backend, which owns the window and the queue the app
        // polls. Writing to the platform's own state would land in a store the host
        // never reads, because `create_window` goes through the control backend.
        crate::queue_resize_trigger(window_id, width, height)
    }

    /// Queues a repaint for the host to pick up. `false` when `id` is not mounted.
    fn invalidate_surface(&self, id: u64) -> bool {
        self.state.invalidate_surface_record(id)
    }

    /// Removes and returns the next widget awaiting a repaint.
    ///
    /// The drain half of [`Self::invalidate_surface`]: without it the queue the host
    /// fills through `rw_take_pending_repaint` would always answer `None`/`0`, so a
    /// mounted widget that changed could never be asked to redraw.
    fn take_pending_repaint(&self) -> Option<u64> {
        self.state.take_pending_repaint()
    }

    /// The backend displays library-painted widgets by handing the host their frames.
    fn supports_surfaces(&self) -> bool {
        true
    }

    // ─── Tool Bar / Status Bar ───

    // ─── Message Box ───

    // ─── Dialogs (state-only on iOS) ───

    // ─── Spin Box ───

    // ─── List View ───

    // ─── Show / Hide ───

    // ─── Enabled / Visible ───
}

/// Translates a widget-registry window id to the platform id `BackendState` records.
///
/// The id a host holds is the widget-registry id, while `BackendState` (and the UIKit registry)
/// is keyed by the platform id. A caller that passes the platform id directly is honoured too.
/// `None` when the resolved id is not a window — the same refusal `display_of_window` and
/// `safe_area_insets` need, stated once so the two cannot disagree.
fn host_window_of(platform: &IosMobilePlatform, window_id: u64) -> Option<u64> {
    let host = if platform.state.kind_of(window_id).is_some() {
        window_id
    } else {
        #[cfg(not(alloc_frugal))]
        {
            crate::widget::runtime::host_window_for(window_id).unwrap_or(window_id)
        }
        #[cfg(alloc_frugal)]
        {
            window_id
        }
    };
    matches!(platform.state.kind_of(host), Some(IosHandleKind::Window)).then_some(host)
}

#[cfg(all(test, not(alloc_frugal)))]
mod tests {
    use super::*;

    #[test]
    fn ios_platform_window_creation() {
        let platform = IosMobilePlatform::new();
        platform.init();

        let window_id = platform.create_window("Test Window", 0, 0, 320, 568);
        assert_ne!(window_id, 0);

        assert_eq!(platform.backend_name(), "ios-state-backend");
        assert_eq!(platform.family(), PlatformFamily::Mobile);
    }

    /// A control id must never address something the host did not create.
    ///
    /// This replaces a test that asserted the opposite — that a `create_button` under
    /// a valid window produced a live id — because the host no longer builds
    /// controls. Asserting `0` for both a bad *and* a good parent is what makes the
    /// answer meaningful: the id is absent, not merely parent-sensitive.
    #[test]
    fn control_members_report_absence_for_every_parent() {
        let platform = IosMobilePlatform::new();
        platform.init();

        assert_eq!(platform.create_button(999, "Button", 0, 0, 80, 44), 0);

        let window_id = platform.create_window("Window", 0, 0, 320, 568);
        assert_ne!(window_id, 0);
        assert_eq!(platform.create_button(window_id, "Button", 0, 0, 80, 44), 0);
    }

    /// The list/combo data members are gone with the controls that backed them, so
    /// their trait defaults must report failure rather than accept writes into a
    /// model nothing can read.
    #[test]
    fn list_and_combo_data_members_report_failure() {
        let platform = IosMobilePlatform::new();
        platform.init();

        let window_id = platform.create_window("Window", 0, 0, 320, 568);
        assert!(!platform.list_box_add_item(window_id, "Item 1"));
        assert!(!platform.list_box_clear_items(window_id));
        assert!(!platform.combo_box_add_item(window_id, "Item 1"));
        assert_eq!(platform.combo_box_item_count(window_id), 0);
    }

    #[cfg(all(feature = "serde_json", feature = "serde", widgets_unstripped))]
    #[test]
    fn ios_platform_state_serialization() {
        let platform = IosMobilePlatform::new();
        platform.init();

        let _window_id = platform.create_window("Window", 0, 0, 320, 568);
        let result = platform.serialize_state();
        assert!(result.is_ok());
    }

    /// The iOS state backend's published capability set, asserted against the two documents that
    /// publish it.
    ///
    /// # Why `accessibility` and `native_menu` are `false`
    ///
    /// The flags are promises about methods. The backend overrides no
    /// `accessibility_bridge()`, and its menu tree is in-process, so neither flag may claim a
    /// bridge that does not exist. `dpi_scaling` follows the UIKit FFI gate because
    /// `dpi_scale_factor()` reads `UIScreen.scale`; `ime` is now `true` because `ime_bridge()`
    /// exists over the shared marked-text state machine (BLUE28 B28.12), fed by
    /// `rw_ios_ime_marked_text`/`rw_ios_ime_commit`.
    #[test]
    fn ios_platform_reports_explicit_mobile_capabilities() {
        let platform = IosMobilePlatform::new();
        let caps = platform.capabilities();

        assert_eq!(platform.family(), PlatformFamily::Mobile);
        assert_eq!(
            caps.dpi_scaling,
            cfg!(all(apple_uikit, feature = "ios-uikit-ffi")),
            "`dpi_scaling` must track whether `dpi_scale_factor()` can read `UIScreen.scale`; \
             without the UIKit FFI there is no measurement to promise"
        );
        assert!(
            caps.ime,
            "`ime_bridge()` is implemented over the shared state machine, and the host feeds it \
             through `rw_ios_ime_marked_text`/`rw_ios_ime_commit`"
        );
        assert!(
            !caps.accessibility,
            "no `accessibility_bridge()` override exists, so the flag would promise `None`"
        );
        assert!(!caps.native_menu, "the menu tree is kept in-process, so no OS menu is created");
        assert!(caps.typed_widget_trigger, "the backend owns and forwards the trigger queue");
    }

    /// The clipboard flag, the backend and the host path must tell one story.
    ///
    /// Three separate code paths answer "does this process have a clipboard": the flag in
    /// `capabilities()` (which escapes the process through `rw_platform_capabilities`), the
    /// backend `clipboard_backend()` hands out, and the `UIPasteboard` binding in `super::native`.
    /// All three are produced from the same `ios-uikit-ffi` gate, and this asserts that they still
    /// are — a flag of `true` with no backend, or a backend with no FFI behind it, is exactly the
    /// defect the capability contract forbids.
    #[test]
    fn the_clipboard_flag_matches_its_backend() {
        let platform = IosMobilePlatform::new();
        let claimed = platform.capabilities().clipboard;
        assert_eq!(
            claimed,
            cfg!(all(apple_uikit, feature = "ios-uikit-ffi")),
            "`clipboard` must track the UIKit FFI gate, not a literal"
        );
        assert_eq!(
            claimed,
            platform.clipboard_backend().is_some(),
            "the flag and `clipboard_backend()` must agree in both directions"
        );
    }

    /// The host must create **no** control: the library paints every `WidgetKind`,
    /// so a `create_*` that answered with a live id would announce a capability the
    /// backend does not have (BLUE15 #55/#56). The trait defaults report `0`, and
    /// this pins that — a regression here would mean a control was reintroduced.
    #[test]
    fn host_creates_no_controls_only_a_window() {
        let platform = IosMobilePlatform::new();
        platform.init();

        let window = platform.create_window("Window", 0, 0, 320, 568);
        assert_ne!(window, 0, "the window is the one primitive the host owns");
        assert_eq!(platform.kind_of(window), Some(IosHandleKind::Window));

        // Every control member now inherits the trait default, which reports that
        // this host provides no such primitive rather than inventing an id.
        assert_eq!(platform.create_button(window, "OK", 0, 0, 80, 44), 0);
        assert_eq!(platform.create_label(window, "hi", 0, 0, 80, 44), 0);
        assert_eq!(platform.create_list_box(window, 0, 0, 320, 120), 0);
        assert_eq!(platform.create_combo_box(window, 0, 0, 160, 44), 0);
    }

    #[test]
    fn ios_platform_capabilities_describe_a_self_drawn_host() {
        let platform = IosMobilePlatform::new();
        let caps = platform.capabilities();

        // iOS ships no native menu bar; the menu model is in-process bookkeeping,
        // which is why the capability is advertised as `false`.
        assert!(!caps.native_menu);
        assert!(caps.typed_widget_trigger);
    }

    /// The backend must host library-painted widgets, and each step must work:
    /// a bare `true` from `supports_surfaces()` would be a claim, not a capability.
    ///
    /// The enqueue→coalesce→consume chain is exercised through the **public**
    /// [`Platform`] trait (as `&dyn Platform`), not the private `state` field: that
    /// is the path a host draining `rw_take_pending_repaint` actually uses.
    #[test]
    fn ios_hosts_widget_surfaces_and_queues_repaints() {
        let platform = IosMobilePlatform::new();
        let dyn_platform: &dyn Platform = &platform;
        assert!(dyn_platform.supports_surfaces());

        let window = dyn_platform.create_window("w", 0, 0, 390, 844);
        let rect = crate::core::Rect::new(0, 0, 120, 44);
        assert!(dyn_platform.mount_surface(window, window, rect));
        assert_eq!(platform.state.surface_rect(window), Some(rect));

        assert!(dyn_platform.invalidate_surface(window));
        assert_eq!(dyn_platform.take_pending_repaint(), Some(window));
        assert_eq!(dyn_platform.take_pending_repaint(), None, "draining empties the queue");

        assert!(dyn_platform.unmount_surface(window));
        assert_eq!(platform.state.surface_rect(window), None);
    }

    /// A surface for a widget this backend never made must be refused.
    #[test]
    fn ios_refuses_a_surface_for_an_unknown_widget() {
        use crate::platform::Platform as _;

        let platform = IosMobilePlatform::new();
        assert!(!platform.mount_surface(1, 9_999, crate::core::Rect::new(0, 0, 10, 10)));
        assert!(!platform.invalidate_surface(9_999));
        assert_eq!(platform.state.mounted_surface_count(), 0);
    }

    /// Without the UIKit FFI there is no window geometry and no trait collection to read, so both
    /// facts are `None` — not zero insets and not a light appearance.
    ///
    /// Gated to the builds that genuinely have no UIKit: on an `apple_uikit` build with
    /// `ios-uikit-ffi` the backend *can* read them, and this test would be asserting the opposite
    /// of the contract it documents.
    #[cfg(not(all(apple_uikit, feature = "ios-uikit-ffi")))]
    #[test]
    fn ios_reports_no_window_insets_or_theme_without_uikit() {
        use crate::platform::Platform as _;

        let platform = IosMobilePlatform::new();
        let window = platform.create_window("insets", 0, 0, 390, 844);
        assert_eq!(
            platform.safe_area_insets(window),
            None,
            "no UIKit registry exists, so the honest answer is `cannot tell`, not zero"
        );
        assert_eq!(platform.system_theme(), None);
    }
}
