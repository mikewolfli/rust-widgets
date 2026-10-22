// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Android platform trait implementation.
//!
//! Implements the `Platform` contract for Android mobile devices.
//! This is a state-driven backend that can be progressively enhanced
//! with native Android views via JNI bindings.
//!
//! ## JNI Integration Path (android-jni feature)
//!
//! All widget creation methods (`create_window`, `create_button`, etc.)
//! currently delegate to the state backend (`AndroidPlatform::insert_widget`)
//! which returns a monotonically increasing handle ID.
//!
//! To wire real Android Views:
//!
//! 1. Check [`AndroidPlatform::jni_available()`] — returns `true` when
//!    the `android-jni` feature is enabled and `JAVA_VM` is initialized.
//! 2. When JNI is wired, each creation method should additionally call the
//!    corresponding JNI native method to create a real Android View and
//!    register it in the view registry.
//! 3. State operations (`set_widget_text`, `set_widget_geometry`, etc.) should
//!    first perform the Rust-side mutation, then forward the call to JNI.
//! 4. All real JNI code should be feature-gated (`#[cfg(feature = "android-jni")]`)
//!    so the state-only backend remains the default for testing and CI.

use super::types::{AndroidHandleKind, AndroidPlatform};
use crate::compat::atomic::Ordering;
use crate::compat::{format, lock, String, ToString, Vec};
use crate::core::PlatformFamily;
use crate::platform::clipboard::RichClipboardBackend;
use crate::platform::types::{Display, DisplayId};
use crate::platform::{Platform, PlatformCapabilities, WidgetTriggerEvent, WidgetTriggerKind};

impl AndroidPlatform {}

/// The frame interval this backend's loop runs at, in milliseconds.
///
/// The same value as every other backend's twin constant, and named here for the same
/// reason: the sleep between iterations and the delta handed to [`crate::drive_frame`]
/// must be the same number, or every transition runs at the ratio between them with
/// nothing to notice the mismatch. A backend that later runs at the display's own rate
/// changes exactly this value.
const FRAME_INTERVAL_MS: u64 = crate::platform::types::FRAME_INTERVAL_MS;

impl Platform for AndroidPlatform {
    // The uniform widget-property methods are answered once, over `self.state`, by the
    // shared expansion in `platform::state_impl` rather than re-written per backend.
    crate::impl_platform_state_properties!();

    fn as_any(&self) -> &dyn crate::compat::Any {
        self
    }

    fn backend_name(&self) -> &'static str {
        "android-state-backend"
    }

    fn family(&self) -> PlatformFamily {
        PlatformFamily::Mobile
    }

    /// The capabilities this backend can honour, over the trait's honest all-`false` default.
    ///
    /// # Why Android answers for itself
    ///
    /// It has exactly one host integration — the system clipboard, through the JNI bridge — and
    /// the trait default would report it as absent. `clipboard` is a **runtime** fact rather than
    /// a build fact: the `JavaVM` and the host `Activity`'s `Context` are both supplied by the app
    /// at startup, so the flag asks [`crate::platform::android_jni::clipboard_ready`] instead of
    /// the feature. A host that has not attached its `Context` gets `false`, which is the honest
    /// answer for a process that cannot reach the clipboard yet, and a `widget-handle` caller that
    /// reads the bit acts on a fact rather than on a hope.
    fn capabilities(&self) -> PlatformCapabilities {
        #[cfg(feature = "android-jni")]
        let clipboard = crate::platform::android_jni::clipboard_ready();
        #[cfg(not(feature = "android-jni"))]
        let clipboard = false;

        PlatformCapabilities {
            clipboard,
            // The display scale is read from `DisplayMetrics.densityDpi / 160` on the same
            // `Context` the clipboard needs. Reporting `true` before the handshake would
            // promise a measurement the backend cannot make, so this follows the runtime
            // readiness fact rather than the build feature (the same rule the clipboard bit
            // above follows).
            dpi_scaling: {
                #[cfg(feature = "android-jni")]
                {
                    crate::platform::android_jni::native_view_creation_ready()
                }
                #[cfg(not(feature = "android-jni"))]
                {
                    false
                }
            },
            // The soft keyboard is controlled through the Android `InputMethodManager` (the
            // same operation winit's Android backend performs). It needs the Activity Context,
            // so the flag follows the runtime handshake rather than the build feature.
            ime: {
                #[cfg(feature = "android-jni")]
                {
                    crate::platform::android_jni::native_view_creation_ready()
                }
                #[cfg(not(feature = "android-jni"))]
                {
                    false
                }
            },
            // Android's system fonts are files under `/system/fonts` on the target; the
            // host-side preview has no such directory and reports the honest empty list.
            system_fonts: cfg!(target_os = "android"),
            // The document picker exists when the JNI bridge has a VM and an Activity Context
            // (BLUE28 B28.08). It is the **asynchronous** contract, not `choose_path`.
            file_dialog: {
                #[cfg(feature = "android-jni")]
                {
                    crate::platform::android_jni::picker_ready()
                }
                #[cfg(not(feature = "android-jni"))]
                {
                    false
                }
            },
            ..crate::platform::types::default_capabilities_for(self.family())
        }
    }

    /// Reads `MemTotal` from `/proc/meminfo` via [`crate::platform::os_probes`].
    fn total_memory_mb(&self) -> Option<u64> {
        crate::platform::os_probes::total_memory_mb()
    }

    /// Reads the display density Android reports: `densityDpi / 160`.
    ///
    /// This is the same derivation winit's Android backend uses (`AndroidApp::config().density()
    /// / 160`). It answers `1.0` when the JNI bridge has no attached `Context`, which is the
    /// honest "not measurable here" value; [`Self::capabilities`] reports `dpi_scaling: false`
    /// in exactly that state, so the constant is not a fabricated claim.
    fn dpi_scale_factor(&self) -> f32 {
        #[cfg(feature = "android-jni")]
        {
            crate::platform::android_jni::display_info().map(|info| info.scale).unwrap_or(1.0)
        }
        #[cfg(not(feature = "android-jni"))]
        {
            1.0
        }
    }

    /// The user's text-size multiplier (`Configuration.fontScale`).
    ///
    /// This is the setting Android's accessibility "Font size" control writes. The backend used to
    /// inherit the trait's `1.0`, so a user who chose "Largest" got nominal text — the same defect
    /// the Windows backend's `text_scale` override closed. `1.0` without the JNI bridge is the
    /// honest "not measurable here" value, matching `dpi_scale_factor`.
    fn text_scale(&self) -> f32 {
        #[cfg(feature = "android-jni")]
        {
            crate::platform::android_jni::font_scale().unwrap_or(1.0)
        }
        #[cfg(not(feature = "android-jni"))]
        {
            1.0
        }
    }

    /// Whether Android's night mode is on, from the current `Configuration`.
    ///
    /// `None` without an attached `Context`: the backend cannot ask the activity's configuration
    /// until the host has handed it over.
    fn system_theme(&self) -> Option<bool> {
        #[cfg(feature = "android-jni")]
        {
            crate::platform::android_jni::system_theme()
        }
        #[cfg(not(feature = "android-jni"))]
        {
            None
        }
    }

    /// Whether Android's high-text-contrast setting is on.
    fn high_contrast(&self) -> Option<bool> {
        #[cfg(feature = "android-jni")]
        {
            crate::platform::android_jni::high_contrast()
        }
        #[cfg(not(feature = "android-jni"))]
        {
            None
        }
    }

    /// Whether the animator duration scale is zero.
    fn reduce_motion(&self) -> Option<bool> {
        #[cfg(feature = "android-jni")]
        {
            crate::platform::android_jni::reduce_motion()
        }
        #[cfg(not(feature = "android-jni"))]
        {
            None
        }
    }

    /// The default locale, from `Locale.getDefault()`.
    fn system_locale(&self) -> Option<&'static str> {
        #[cfg(feature = "android-jni")]
        {
            crate::platform::locale::intern(crate::platform::android_jni::system_locale())
        }
        #[cfg(not(feature = "android-jni"))]
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

    /// The fonts installed under `/system/fonts` on the device.
    ///
    /// Android has no synchronous "list families" API reachable from the NDK; the files in
    /// `/system/fonts` are the fonts `Typeface` resolves, and each file stem is a usable family
    /// label. `source` is the file, so a caller can read it and register the bytes. A host build
    /// (the Android module compiled elsewhere for its tests) has no such directory and answers an
    /// empty list.
    fn system_fonts(&self) -> Vec<crate::platform::types::SystemFont> {
        #[cfg(target_os = "android")]
        {
            let mut fonts: Vec<crate::platform::types::SystemFont> = Vec::new();
            let Ok(entries) = std::fs::read_dir("/system/fonts") else {
                return fonts;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let extension = path.extension().and_then(|value| value.to_str()).unwrap_or("");
                if !extension.eq_ignore_ascii_case("ttf")
                    && !extension.eq_ignore_ascii_case("otf")
                    && !extension.eq_ignore_ascii_case("ttc")
                {
                    continue;
                }
                let Some(family) = path.file_stem().and_then(|value| value.to_str()) else {
                    continue;
                };
                if family.is_empty()
                    || fonts.iter().any(|known| known.family.eq_ignore_ascii_case(family))
                {
                    continue;
                }
                fonts.push(crate::platform::types::SystemFont {
                    family: family.to_string(),
                    style: String::new(),
                    source: Some(path),
                });
            }
            fonts
        }
        #[cfg(not(target_os = "android"))]
        {
            Vec::new()
        }
    }

    /// The activity window's safe-area insets, from `WindowManager.getCurrentWindowMetrics()`.
    ///
    /// `WindowInsets` are pixels; the JNI bridge converts them to logical pixels with the same
    /// `densityDpi / 160` scale `dpi_scale_factor()` reports. `None` without the JNI bridge, before
    /// the host attaches its `Context`, or on an API below 30 (the metric API does not exist
    /// there) — never the widget's portrait-phone default.
    fn safe_area_insets(
        &self,
        _window_id: crate::core::ObjectId,
    ) -> Option<crate::platform::types::SafeAreaInsets> {
        #[cfg(feature = "android-jni")]
        {
            crate::platform::android_jni::window_insets().map(|(top, bottom, left, right)| {
                crate::platform::types::SafeAreaInsets { top, bottom, left, right }
            })
        }
        #[cfg(not(feature = "android-jni"))]
        {
            None
        }
    }

    /// The default display, when the JNI bridge can read its metrics.
    ///
    /// Android's window model places the host `Activity`'s window on the default display, so one
    /// entry is the complete answer for this backend. Without an attached `Context` there is no
    /// `WindowManager` to ask, and an empty list is the honest "nothing is observable from here"
    /// answer the trait documents.
    fn displays(&self) -> Vec<Display> {
        #[cfg(feature = "android-jni")]
        {
            crate::platform::android_jni::display_info()
                .map(|info| {
                    crate::compat::vec![Display {
                        id: DisplayId::new(info.id),
                        bounds: crate::core::Rect::new(0, 0, info.width, info.height),
                        work_area: crate::core::Rect::new(0, 0, info.work_width, info.work_height),
                        scale: info.scale,
                        is_primary: info.is_primary,
                    }]
                })
                .unwrap_or_default()
        }
        #[cfg(not(feature = "android-jni"))]
        {
            Vec::new()
        }
    }

    /// Metadata for the default display.
    ///
    /// The orientation is derived from the reported pixel bounds; Android's refresh rate and HDR
    /// capabilities need `Display.Mode` / `Display.HdrCapabilities` JNI calls this bridge does not
    /// yet make, so those answer `None` rather than a fabricated value.
    fn display_info(&self, display: DisplayId) -> Option<crate::platform::types::DisplayInfo> {
        let display_item = self.displays().into_iter().find(|item| item.id == display)?;
        Some(crate::platform::types::DisplayInfo {
            refresh_rate_mhz: None,
            orientation: Some(if display_item.bounds.width >= display_item.bounds.height {
                crate::core::Orientation::Horizontal
            } else {
                crate::core::Orientation::Vertical
            }),
            color_space: None,
            hdr: None,
        })
    }

    /// The default display for a window this backend owns.
    ///
    /// There is no per-window display bookkeeping on Android: the host `Activity` owns the one
    /// window and the system places it. An id that names no window is answered `None`, so a caller
    /// cannot mistake a random control id for a placed window.
    fn display_of_window(&self, window_id: crate::core::ObjectId) -> Option<DisplayId> {
        // The id a host holds is the widget-registry id, while `state` is keyed by the
        // platform's own handle id; `host_window_for` is the association recorded at creation.
        // A caller that passes the platform id directly is honoured too.
        let host = if self.kind_of(window_id).is_some() {
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
        if !matches!(self.kind_of(host), Some(AndroidHandleKind::Window)) {
            return None;
        }
        self.displays().first().map(|display| display.id)
    }

    /// A revision of the current display metrics.
    ///
    /// Derived from the same fields [`Self::displays`] returns, so a host that compares two
    /// readings re-reads the list exactly when the metrics (size, work area, or density) changed.
    /// `0` means the bridge could not read a display at all, which the trait contract reserves for
    /// "not observable".
    fn display_generation(&self) -> u64 {
        const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
        const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
        let Some(display) = self.displays().first().copied() else {
            return 0;
        };
        let mut hash = FNV_OFFSET;
        for part in [
            display.id.get(),
            display.bounds.width as u64,
            display.bounds.height as u64,
            display.work_area.width as u64,
            display.work_area.height as u64,
            display.scale.to_bits() as u64,
            display.is_primary as u64,
        ] {
            hash ^= part;
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        hash
    }

    /// Reports whether any battery in `/sys/class/power_supply` is discharging.
    fn is_on_battery(&self) -> bool {
        crate::platform::os_probes::is_on_battery()
    }

    /// Samples RSS over VmSize for this process from `/proc/self/status`.
    fn process_memory_utilization(&self) -> Option<f32> {
        crate::platform::os_probes::process_memory_utilization()
    }

    /// CPU utilization has no reliable one-shot source here, so this reports `None` (see
    /// `os_probes::process_cpu_utilization`); the thread budget is a separate fact.
    fn process_cpu_utilization(&self) -> Option<f32> {
        crate::platform::os_probes::process_cpu_utilization()
    }

    /// Hands `job_file` to Android's `PrintManager` through the host helper (B28.27).
    ///
    /// The adapter is Java-side (`rust_widgets.RustWidgets.printFile`), because a
    /// `PrintDocumentAdapter` is a Java class this crate cannot define; the helper streams the
    /// file to the destination the system print UI chose. Without `android-jni` there is no JNI
    /// bridge to call, so the job is refused with the reason.
    fn spawn_print_job(&self, job_file: &std::path::Path) -> Result<(), String> {
        #[cfg(feature = "android-jni")]
        {
            crate::platform::android_jni::print_file(job_file)
        }
        #[cfg(not(feature = "android-jni"))]
        {
            Err(format!(
                "Android printing requires the JNI bridge (`android-jni`); job file '{}' was not \
                 printed",
                job_file.display()
            ))
        }
    }

    /// Printing is available when the JNI bridge holds the Activity context the print manager
    /// needs. That is the same condition `spawn_print_job` depends on, so the flag and the method
    /// cannot disagree.
    fn has_print_support(&self) -> bool {
        #[cfg(feature = "android-jni")]
        {
            crate::platform::android_jni::print_available()
        }
        #[cfg(not(feature = "android-jni"))]
        {
            false
        }
    }

    #[cfg(feature = "mobile-api")]
    fn mobile_extension(&self) -> Option<&dyn crate::platform::contract::MobilePlatformExtension> {
        Some(self)
    }

    fn init(&self) {
        let _ = self.android_runtime_marker();
        self.runtime.initialized.store(true, Ordering::SeqCst);
    }

    fn run(&self) {
        if !self.runtime.initialized.load(Ordering::SeqCst) {
            self.init();
        }
        // Android state backend uses polling loop for deterministic behavior.
        self.runtime.running.store(true, Ordering::SeqCst);
        // The loop is also the drain tick, so a `Resized` event the host queued reaches
        // the window layout. See `crate::drain_triggers`.
        //
        // `crate::drive_frame` additionally advances the animation bus, which the drain
        // alone did not: layout was correct and every animation was inert, because no code
        // ran the per-frame step (BLUE24 §0A.1 measurement 1).
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
        true
    }

    /// Release every registry entry the backend holds for `widget_id`.
    ///
    /// Android keeps one per-widget side table beyond the authoritative
    /// `BackendState` record: the menu bookkeeping (`menus`). It must be purged,
    /// otherwise a UI rebuilt in a create/destroy loop would leak one entry per
    /// discarded widget. The lock is scoped to its own statement so no two guards
    /// are held at the same time.
    fn destroy_widget(&self, widget_id: u64) -> bool {
        // Menu bookkeeping is shared by Android, Mobile and Wayland; see
        // `platform::menu_bookkeeping` for the rule (PLAT-08).
        lock(&self.menus).purge(widget_id);

        // The state record is the authority on whether the widget existed.
        self.state.destroy_widget(widget_id)
    }

    // ─── Widget creation ─────────────────────────────────────────────────

    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.state.count_kind(AndroidHandleKind::Window) > 0 {
            log::warn!(
                "[android] create_window: this backend hosts one Activity window; the second
                 request returns 0 (BLUE28 B28.43)"
            );
            return 0;
        }
        self.insert_widget(AndroidHandleKind::Window, title, x, y, width, height)
    }

    /// Android's host model is one Activity with one window.
    fn supports_multiple_windows(&self) -> bool {
        false
    }

    /// Starts the Android document picker for an existing file.
    ///
    /// `ACTION_OPEN_DOCUMENT` is the only picker this bridge launches, so `SaveFile` and
    /// `SelectDirectory` answer `false` rather than presenting a dialog that cannot honour the
    /// purpose. The result is delivered through [`Self::take_path_choice`].
    fn request_path(&self, purpose: crate::platform::PathPurpose, _title: &str) -> bool {
        #[cfg(feature = "android-jni")]
        {
            if !matches!(purpose, crate::platform::PathPurpose::OpenFile) {
                log::warn!(
                    "[android] request_path: only OpenFile is available; the document picker has
                     no save/directory mode in this bridge"
                );
                return false;
            }
            crate::platform::android_jni::launch_file_dialog("*/*")
        }
        #[cfg(not(feature = "android-jni"))]
        {
            let _ = purpose;
            false
        }
    }

    /// Pops the next document-picker answer delivered by the host.
    fn take_path_choice(&self) -> Option<crate::platform::PathChoice> {
        #[cfg(feature = "android-jni")]
        {
            crate::platform::android_jni::take_path_choice()
        }
        #[cfg(not(feature = "android-jni"))]
        {
            None
        }
    }

    // ─── Menu model ──────────────────────────────────────────────────────
    //
    // These are NOT control construction. Android has no standalone menu-bar or
    // menu *View*: the host Activity owns the menu and materialises it through the
    // platform's own `onCreateOptionsMenu` / `onOptionsItemSelected` callbacks. What
    // lives here is the in-process model that maps a Rust-side menu tree onto that
    // callback surface, plus an injectable trigger queue so the library's own menu
    // widget can report activations without a UI toolkit of its own.
    //
    // They therefore survive the self-drawing change, exactly as on iOS: the
    // library paints the menu *appearance*, while the host still owns the menu
    // *identity* the OS asks about. `capabilities().native_menu` stays `false`,
    // because no OS menu object is created here.

    fn create_menu_bar(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if !matches!(self.kind_of(parent), Some(AndroidHandleKind::Window)) {
            return 0;
        }
        self.insert_widget(AndroidHandleKind::MenuBar, "MenuBar", x, y, width, height)
    }

    fn create_menu(&self, parent: u64, text: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        // A menu hangs off a menu bar or another menu.
        if !matches!(
            self.kind_of(parent),
            Some(AndroidHandleKind::MenuBar) | Some(AndroidHandleKind::Menu)
        ) {
            return 0;
        }
        self.insert_widget(AndroidHandleKind::Menu, text, x, y, width, height)
    }

    /// Binds `menu_bar` to `window` as that window's menu.
    ///
    /// Both ids and their kinds are validated, so a caller cannot attach a menu bar
    /// to something that is not a window (or attach a non-menu-bar), which would make
    /// the host ask a widget for a menu it does not have.
    fn attach_menu_bar_to_window(&self, window: u64, menu_bar: u64) -> bool {
        if !matches!(self.kind_of(window), Some(AndroidHandleKind::Window)) {
            return false;
        }
        if !matches!(self.kind_of(menu_bar), Some(AndroidHandleKind::MenuBar)) {
            return false;
        }
        let mut menus = lock(&self.menus);
        menus.attached_menu_bar.insert(window, menu_bar);
        true
    }

    fn menu_add_item(&self, parent_menu: u64, text: &str, shortcut: Option<&str>) -> u64 {
        // A menu item must hang off a menu.
        if !matches!(self.kind_of(parent_menu), Some(AndroidHandleKind::Menu)) {
            return 0;
        }
        let id = self.insert_widget(AndroidHandleKind::MenuItem, text, 0, 0, 0, 0);

        let display_text = match shortcut {
            Some(shortcut) => format!("{} ({})", text, shortcut),
            None => text.to_string(),
        };
        self.state.set_text(id, &display_text);

        let mut menus = lock(&self.menus);
        menus.menu_children.entry(parent_menu).or_default().push(id);

        // Native menu items are represented through the Activity's own menu
        // resource; no standalone Android View exists for a menu entry.
        id
    }

    fn poll_menu_triggered(&self) -> Option<u64> {
        let mut menus = lock(&self.menus);
        menus.pending_menu_events.pop_front()
    }

    fn inject_menu_trigger(&self, menu_item_id: u64) -> bool {
        // Only a menu item may produce a menu trigger.
        if !matches!(self.kind_of(menu_item_id), Some(AndroidHandleKind::MenuItem)) {
            return false;
        }
        let mut menus = lock(&self.menus);
        menus.pending_menu_events.push_back(menu_item_id);
        true
    }

    fn poll_widget_triggered(&self) -> Option<u64> {
        self.state.pop_widget_trigger()
    }

    fn poll_widget_trigger_event(&self) -> Option<WidgetTriggerEvent> {
        self.state.pop_widget_trigger_event()
    }

    fn inject_widget_trigger_event(&self, widget_id: u64, kind: WidgetTriggerKind) -> bool {
        self.state.inject_widget_trigger_event(widget_id, kind)
    }

    /// Mounts a library-painted widget onto a surface this host will present.
    ///
    /// No native view is created per control (every `WidgetKind` is painted by
    /// `src/widget/`), so the surface is a record plus a repaint queue: the Activity
    /// owns the pixels and pulls them with the render API.
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

    // ─── Widget manipulation ─────────────────────────────────────────────

    // ─── Clipboard ───────────────────────────────────────────────────
    //
    // The host `ClipboardManager` when the JNI bridge is compiled in *and* the host app has run
    // the VM/`Activity` handshake; the in-process `BackendState` record otherwise. The `Option`
    // bindings below are the mechanism: each `#[cfg]` block is removed in the build it does not
    // describe, so exactly one supplies the value and the method keeps one signature.

    fn set_clipboard_text(&self, text: &str) -> bool {
        #[cfg(feature = "android-jni")]
        {
            // Three outcomes, and the middle one is why this is not a one-liner:
            //   * the bridge is ready and the write landed   → `true`
            //   * the bridge is ready and the write was refused (the platform refuses background
            //     clipboard writes without an error) → `false`, *not* a mirror write
            //   * the bridge is not ready (no `JavaVM`, or no `Activity` `Context`) → the
            //     in-process record, which is the honest answer for a process that has not
            //     finished the handshake yet
            crate::platform::android_jni::set_clipboard_text(text)
                .unwrap_or_else(|| self.state.set_clipboard_text(text))
        }
        #[cfg(not(feature = "android-jni"))]
        {
            self.state.set_clipboard_text(text)
        }
    }

    fn get_clipboard_text(&self) -> String {
        #[cfg(feature = "android-jni")]
        {
            crate::platform::android_jni::clipboard_text()
                .unwrap_or_else(|| self.state.clipboard_text())
        }
        #[cfg(not(feature = "android-jni"))]
        {
            self.state.clipboard_text()
        }
    }

    /// The primary clip's text, or `None` when it holds none.
    ///
    /// Overridden because the trait default derives it from [`Self::get_clipboard_text`], and on
    /// the host path an empty string means "the clipboard holds an empty text clip" — a real
    /// answer (`hasPrimaryClip` is satisfied) that the default would report as "no text".
    fn clipboard_text(&self) -> Option<String> {
        #[cfg(feature = "android-jni")]
        {
            crate::platform::android_jni::clipboard_text()
        }
        #[cfg(not(feature = "android-jni"))]
        {
            let text = self.state.clipboard_text();
            if text.is_empty() {
                None
            } else {
                Some(text)
            }
        }
    }

    /// The host clipboard, or `None` when this process cannot reach it.
    ///
    /// Gated on the same [`crate::platform::android_jni::clipboard_ready`] the capability flag
    /// asks, so the two answer together: a host that reads `clipboard: true` and then calls this
    /// gets a backend, and a host that reads `false` gets `None` rather than a backend whose every
    /// write is refused. The rich methods still check readiness themselves — the handshake can be
    /// undone by a configuration change — so this is the first of two gates, not the only one.
    fn clipboard_backend(&self) -> Option<&dyn RichClipboardBackend> {
        #[cfg(feature = "android-jni")]
        {
            if crate::platform::android_jni::clipboard_ready() {
                static CLIPBOARD: std::sync::OnceLock<
                    crate::platform::android_jni::AndroidClipboard,
                > = std::sync::OnceLock::new();
                Some(CLIPBOARD.get_or_init(crate::platform::android_jni::AndroidClipboard::new))
            } else {
                None
            }
        }
        #[cfg(not(feature = "android-jni"))]
        {
            None
        }
    }

    /// The soft-keyboard bridge, when the JNI handshake has supplied a Context.
    ///
    /// The bridge controls the Android `InputMethodManager` and keeps the in-process composition
    /// state; see [`crate::platform::ime_android`]. Without the feature or the Context it answers
    /// `None`, matching the `ime: false` capability bit in exactly those states.
    #[cfg(feature = "android-jni")]
    fn ime_bridge(&self) -> Option<&dyn crate::platform::ime::ImeBridge> {
        if crate::platform::android_jni::native_view_creation_ready() {
            Some(crate::platform::ime_android::AndroidImeBridge::bridge())
        } else {
            None
        }
    }

    // ─── Drag and drop ───────────────────────────────────────────────────

    // ─── IME ─────────────────────────────────────────────────────────────

    // ─── Accessibility ───────────────────────────────────────────────────
}

// ─── MobilePlatformExtension ─────────────────────────────────────────────

impl crate::platform::contract::MobilePlatformExtension for AndroidPlatform {
    fn mobile_backend(&self) -> crate::platform::MobileBackend {
        crate::platform::MobileBackend::Android
    }

    fn attach_to_native_view(&self, native_handle: usize) -> bool {
        // The handle is the host Activity's Java `Context` reference. Store it as
        // a GlobalRef so a later platform request (a document picker, say) can
        // resolve an `Activity` from any thread; without it this host cannot
        // serve such requests at all.
        #[cfg(feature = "android-jni")]
        {
            if native_handle == 0 || !crate::platform::android_jni::is_initialized() {
                return false;
            }
            // `JObject` does not own the reference; the GlobalRef created by
            // `set_activity_context` is what keeps it alive.
            let context_obj: jni::objects::JObject<'_> =
                unsafe { jni::objects::JObject::from_raw(native_handle as jni::sys::jobject) };
            crate::platform::android_jni::with_jni_env(|env| {
                crate::platform::android_jni::set_activity_context(env, &context_obj)
            })
            .unwrap_or(false)
        }
        #[cfg(not(feature = "android-jni"))]
        {
            let _ = native_handle;
            false
        }
    }

    /// The orientation the host reported through `nativeOnConfigurationChanged`, or `None`
    /// before it reports one.
    ///
    /// Deriving the axis from display bounds cannot distinguish landscape-left from
    /// landscape-right, and a state-only host has no display at all; the host's
    /// `Display.getRotation()` is the real source, so this does not guess.
    fn orientation(&self) -> Option<crate::platform::ScreenOrientation> {
        crate::platform::lifecycle::orientation()
    }

    /// The display revision also changes on rotation, and so does the reported rotation.
    fn orientation_generation(&self) -> u64 {
        self.display_generation().wrapping_add(crate::platform::lifecycle::rotation_generation())
    }

    /// The phase the host last reported through `nativeOnLifecycleChanged`.
    fn lifecycle_state(&self) -> Option<crate::platform::MobileLifecycleState> {
        crate::platform::lifecycle::lifecycle_state()
    }

    /// Stops the animation clock; the Activity calls this from `onPause`.
    fn suspend(&self) -> bool {
        crate::platform::lifecycle::set_state(crate::platform::MobileLifecycleState::Background)
    }

    /// Restarts the animation clock; the Activity calls this from `onResume`.
    fn resume(&self) -> bool {
        crate::platform::lifecycle::set_state(crate::platform::MobileLifecycleState::Active)
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn android_platform_window_creation() {
        let platform = AndroidPlatform::new();
        platform.init();

        let window_id = platform.create_window("Test Window", 0, 0, 320, 568);
        assert_ne!(window_id, 0);

        assert_eq!(platform.backend_name(), "android-state-backend");
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
        let platform = AndroidPlatform::new();
        platform.init();

        assert_eq!(platform.create_button(999, "Button", 0, 0, 80, 44), 0);

        let window_id = platform.create_window("Window", 0, 0, 320, 568);
        assert_ne!(window_id, 0);
        assert_eq!(platform.create_button(window_id, "Button", 0, 0, 80, 44), 0);
    }

    /// The per-control list storage went with the controls that backed it, so these
    /// members must report absence rather than accept writes into a model nothing
    /// can read.
    #[test]
    fn combo_box_data_members_report_absence() {
        let platform = AndroidPlatform::new();
        platform.init();

        let window_id = platform.create_window("Window", 0, 0, 320, 568);
        assert_eq!(platform.create_combo_box(window_id, 0, 0, 200, 40), 0);
        assert!(!platform.combo_box_add_item(window_id, "Item 1"));
        assert_eq!(platform.combo_box_item_count(window_id), 0);
        assert!(!platform.combo_box_clear_items(window_id));
    }

    #[test]
    fn android_menu_requires_correct_parent_kind() {
        let platform = AndroidPlatform::new();
        platform.init();
        let window = platform.create_window("Window", 0, 0, 320, 568);
        let button = platform.create_button(window, "Button", 0, 0, 80, 44);

        // MenuBar requires a window.
        assert_eq!(platform.create_menu_bar(button, 0, 0, 320, 24), 0);
        let menu_bar = platform.create_menu_bar(window, 0, 0, 320, 24);
        assert_ne!(menu_bar, 0);

        // A menu requires a menu bar or another menu.
        assert_eq!(platform.create_menu(window, "Bad", 0, 0, 80, 24), 0);
        assert_eq!(platform.create_menu(button, "Bad", 0, 0, 80, 24), 0);
        let menu = platform.create_menu(menu_bar, "File", 0, 0, 80, 24);
        assert_ne!(menu, 0);
        let submenu = platform.create_menu(menu, "Recent", 0, 0, 80, 24);
        assert_ne!(submenu, 0, "a menu may nest under another menu");

        // A menu item requires a menu.
        assert_eq!(platform.menu_add_item(window, "Bad", None), 0);
        assert_eq!(platform.menu_add_item(menu_bar, "Bad", None), 0);
        let item = platform.menu_add_item(menu, "Open", Some("Ctrl+O"));
        assert_ne!(item, 0);

        // Only a menu item may be injected as a menu trigger.
        assert!(!platform.inject_menu_trigger(window));
        assert!(!platform.inject_menu_trigger(menu_bar));
        assert!(platform.inject_menu_trigger(item));
        assert_eq!(platform.poll_menu_triggered(), Some(item));

        // attach_menu_bar_to_window validates both ids and their kinds.
        assert!(!platform.attach_menu_bar_to_window(9999, menu_bar));
        assert!(!platform.attach_menu_bar_to_window(window, item));
        assert!(platform.attach_menu_bar_to_window(window, menu_bar));
    }

    #[test]
    fn android_menu_item_retains_shortcut() {
        let platform = AndroidPlatform::new();
        platform.init();
        let window = platform.create_window("Window", 0, 0, 320, 568);
        let menu_bar = platform.create_menu_bar(window, 0, 0, 320, 24);
        let menu = platform.create_menu(menu_bar, "File", 0, 0, 80, 24);

        let plain = platform.menu_add_item(menu, "Open", None);
        assert_eq!(platform.get_widget_text(plain), "Open");

        let with_shortcut = platform.menu_add_item(menu, "Save", Some("Ctrl+S"));
        assert_eq!(platform.get_widget_text(with_shortcut), "Save (Ctrl+S)");
    }

    /// The backend must host library-painted widgets, and each step must work:
    /// a bare `true` from `supports_surfaces()` would be a claim, not a capability.
    ///
    /// The enqueue→coalesce→consume chain is exercised through the **public**
    /// [`Platform`] trait (as `&dyn Platform`), not the private `state` field: that
    /// is the path a host draining `rw_take_pending_repaint` actually uses, and the
    /// one that used to answer `None` because this backend never overrode the trait's
    /// default.
    #[test]
    fn android_hosts_widget_surfaces_and_queues_repaints() {
        let platform = AndroidPlatform::new();
        let dyn_platform: &dyn Platform = &platform;
        assert!(dyn_platform.supports_surfaces());

        let window = dyn_platform.create_window("Window", 0, 0, 412, 915);
        let rect = crate::core::Rect::new(0, 0, 120, 44);
        assert!(dyn_platform.mount_surface(window, window, rect));
        assert_eq!(platform.state.surface_rect(window), Some(rect));

        assert!(dyn_platform.invalidate_surface(window));
        assert_eq!(dyn_platform.take_pending_repaint(), Some(window));
        assert_eq!(dyn_platform.take_pending_repaint(), None, "draining empties the queue");

        let moved = crate::core::Rect::new(8, 8, 200, 80);
        assert!(dyn_platform.resize_surface(window, moved));
        assert_eq!(platform.state.surface_rect(window), Some(moved));

        assert!(dyn_platform.unmount_surface(window));
        assert_eq!(platform.state.surface_rect(window), None);
    }

    /// A surface for a widget this backend never made must be refused.
    #[test]
    fn android_refuses_a_surface_for_an_unknown_widget() {
        let platform = AndroidPlatform::new();
        assert!(!platform.mount_surface(1, 9_999, crate::core::Rect::new(0, 0, 10, 10)));
        assert!(!platform.invalidate_surface(9_999));
        assert_eq!(platform.state.mounted_surface_count(), 0);
    }
}
