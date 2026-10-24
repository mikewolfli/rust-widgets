// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Wayland backend platform implementation.
//!
//! This module implements the `Platform` trait for the Wayland backend.
//!
//! # BLUE15: the host supplies a window and a drawing surface, not controls
//!
//! This backend used to answer every `create_<control>` override by inserting a
//! per-kind `WaylandHandleKind` record — and, where the protocol allowed it, by
//! building a compositor object to go with it. Under the self-drawn strategy that
//! is the duplication BLUE15 removes: the library paints every `WidgetKind`, and
//! the host owes the widget layer a **window** and a **drawing surface** (rules
//! #55/#56). Those overrides never produced a real control that the library could
//! not paint itself, and `xdg_shell` has no vocabulary for buttons, labels or
//! dialogs, so the control creators and the per-kind handle state that existed
//! only to serve them are gone (rule #59: delete means delete).
//!
//! What survives is the platform-facing part the library cannot replace:
//!
//! - [`Platform::create_window`], including the native `xdg_toplevel` path
//!   (`wl_surface` → `xdg_surface` → `xdg_toplevel`) and the connection-wide
//!   session it keeps alive for event dispatch.
//! - The runtime lifecycle (`init`/`run`/`quit`), the fd-based event loop and
//!   `create_event_loop_pump`.
//! - Global discovery and scale handling: `wl_compositor`, `xdg_wm_base` and the
//!   `wl_output` scale event that feeds `dpi_scale_factor()`.
//! - The protocol dispatch implementations the compositor requires (registry
//!   globals, surface enter/leave, `xdg_wm_base` ping/pong, `xdg_surface`
//!   configure ack, `xdg_toplevel` configure/close).
//! - The injectable menu **data model** and the widget-trigger queue. Neither is
//!   control construction — see the menu note at `attach_menu_bar_to_window`.
//!
//! Per-widget state is still recorded in `BackendState<WaylandHandleKind>` for
//! windows, so the ordinary state operations (text, geometry, visibility, IME,
//! accessibility, clipboard, drag and drop) remain meaningful.
//!
//! ## Menu semantics (Wayland)
//!
//! Wayland defines no menu protocol: `xdg_shell` covers toplevels and popups only.
//! `MenuBar`/`Menu`/`MenuItem` are therefore an in-process data model with
//! kind-constrained parents, textual payload, and injectable trigger events — the
//! same shape of capability the iOS backend keeps (see
//! `platform/ios/platform_impl.rs`). They must not be described as native menus,
//! and `capabilities().native_menu` reports `false` accordingly.

use crate::core::ObjectId;
use crate::core::PlatformFamily;
use crate::event::EventLoop;
use crate::platform::clipboard::RichClipboardBackend as _;
use crate::platform::types::{
    Platform, PlatformCapabilities, WidgetTriggerEvent, WidgetTriggerKind,
};
use crate::platform::wayland::types::{WaylandHandleKind, WaylandPlatform};

/// The frame interval this backend advances the library by, in milliseconds.
///
/// # Why this is at file scope and not inside the loop
///
/// It was a `let`-scope `const` inside the native-fd loop, so only the `run()` loop could see
/// it — and `dispatch_native_events`, the pump-callback spelling of the same tick, referenced a
/// name that was not in *its* scope at all. The module therefore did not compile whenever the
/// native Wayland path was enabled: `cargo check --features wayland-native` failed with
/// `cannot find value FRAME_INTERVAL_MS`, which no CI job ran.
///
/// Same value and same reason as the other backends' twin constants: the number handed to
/// [`crate::drive_frame`] is what makes a transition take the time the theme said it should, so
/// it must not differ between the loop and the pump callback — nor from the other hosts.
const FRAME_INTERVAL_MS: u32 = crate::platform::types::FRAME_INTERVAL_MS as u32;

#[cfg(all(feature = "wayland-native", unix_desktop))]
use wayland_client as wl_client;

#[cfg(all(feature = "wayland-native", unix_desktop))]
use wayland_protocols as wl_protocols;

// ---------------------------------------------------------------------------
// Platform trait implementation
// ---------------------------------------------------------------------------

/// The output the compositor reported, as a [`Display`](crate::platform::Display).
///
/// A Wayland client without `xdg-output` has no logical position separate from the output's
/// `geometry` event and its current `mode`; `mode pixels / scale` is the logical size, and the
/// geometry x/y is the compositor's own placement. This is a measured derivation, not a guess:
/// `None` until a `Current` mode has been reported.
#[cfg(all(feature = "wayland-native", unix_desktop))]
fn session_output(platform: &WaylandPlatform) -> Option<crate::platform::Display> {
    let guard = platform.native_session.lock().ok()?;
    let session = guard.as_ref()?;
    let state = &session.state;
    if !state.output_ready || state.output_width == 0 || state.output_height == 0 {
        return None;
    }
    let scale =
        if state.dpi_scale.is_finite() && state.dpi_scale > 0.0 { state.dpi_scale } else { 1.0 };
    let width = (state.output_width as f32 / scale).round() as u32;
    let height = (state.output_height as f32 / scale).round() as u32;
    Some(crate::platform::Display {
        id: crate::platform::DisplayId::new(1),
        bounds: crate::core::Rect::new(state.output_x, state.output_y, width, height),
        // Wayland core has no work-area concept; the output's full mode is the only answer.
        work_area: crate::core::Rect::new(state.output_x, state.output_y, width, height),
        scale,
        is_primary: true,
    })
}

impl Platform for WaylandPlatform {
    // The uniform widget-property methods are answered once, over `self.state`, by the
    // shared expansion in `platform::state_impl` rather than re-written per backend.
    crate::impl_platform_state_properties!();

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn backend_name(&self) -> &'static str {
        "wayland"
    }

    fn family(&self) -> PlatformFamily {
        PlatformFamily::Desktop
    }

    /// Reads `MemTotal` from `/proc/meminfo` via [`crate::platform::os_probes`].
    fn total_memory_mb(&self) -> Option<u64> {
        crate::platform::os_probes::total_memory_mb()
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

    /// Submits the job to the unix print spooler via [`crate::platform::os_probes`].
    fn spawn_print_job(&self, job_file: &std::path::Path) -> Result<(), String> {
        crate::platform::os_probes::spawn_print_job(job_file)
    }

    /// Wayland sessions on Linux use CUPS. The `lp`/`lpr` clients are the only
    /// thing consulted — no compositor capability is involved.
    fn has_print_support(&self) -> bool {
        crate::platform::types::unix_print_clients_available()
    }

    /// The host integrations this backend actually provides.
    ///
    /// # Why `ime` and `clipboard` track a live protocol
    ///
    /// [`PlatformCapabilities::ime`] is a promise about [`Platform::ime_bridge`] **answering for
    /// this compositor**: the object exists on every build, but the flag is true only when
    /// `zwp_text_input_manager_v3` was actually bound (B28.12). `clipboard` follows a live
    /// `wl_data_device` the same way (B28.04); without a session the backend still answers the
    /// in-process record, which is not a host clipboard and is not advertised as one.
    ///
    /// `accessibility` stays `false`: this backend publishes no AT-SPI tree, so a `true` would
    /// promise a screen-reader bridge that does not exist. The flag escapes the process through
    /// `rw_platform_capabilities()`, so a host does not offer an affordance that cannot answer.
    ///
    /// # Why `dpi_scaling` stays `true`
    ///
    /// It is the one of the three with an override behind it: [`Platform::dpi_scale_factor`]
    /// reads `GDK_SCALE` / `scale factor` / `RUST_WIDGETS_DPI_SCALE` and returns a real
    /// number. Its values come from the *environment* rather than from the compositor, which is
    /// weaker than the GTK backend's — but it is a measurement of the host, not a fabricated
    /// constant, so the flag is honest (principle #37 draws the line at a value that was never
    /// obtained, not at one obtained by a less direct route).
    ///
    /// # Why `native_menu` stays `false`
    ///
    /// Wayland has no menu protocol, so the menu tree this backend keeps is in-process data the
    /// host renders and feeds back as injected triggers. Advertising a native menu would be false.
    /// This was the only one of the five the backend had actually reasoned about before; the
    /// comment above it was true and the two beside it were not, which is what made the pair easy
    /// to miss.
    fn capabilities(&self) -> PlatformCapabilities {
        PlatformCapabilities {
            dpi_scaling: true,
            // A live `zwp_text_input_v3` exists only when the compositor advertises the
            // text-input manager (B28.12).
            ime: self.ime.available(),
            accessibility: false,
            native_menu: false,
            typed_widget_trigger: true,
            // A live `wl_data_device` is created from the session's `wl_seat` and
            // `wl_data_device_manager`; until that session exists there is no system clipboard and
            // the backend still answers the in-process record (B28.04).
            clipboard: self.clipboard.has_channel(),
            // `open_external` spawns `xdg-open`, the same handler the GTK backend uses.
            open_external: true,
            // No picker: this backend has no `xdg-desktop-portal` binding, and the portal is the
            // only way a Wayland client may present a file chooser.
            file_dialog: false,
            // `fc-list` is the fontconfig enumerator on a Wayland session (B28.39).
            system_fonts: true,
        }
    }

    /// Wayland does not lay system UI over a toplevel's client area.
    ///
    /// The compositor's panels and the desktop's chrome live outside the client area, so all four
    /// distances are zero — a measurement, not a fallback. `Some(zeros)` keeps the widget layer
    /// from substituting its portrait-phone default, and matches winit's Wayland backend
    /// (`PhysicalInsets::new(0, 0, 0, 0)`).
    fn safe_area_insets(
        &self,
        _window_id: ObjectId,
    ) -> Option<crate::platform::types::SafeAreaInsets> {
        Some(crate::platform::types::SafeAreaInsets { top: 0, bottom: 0, left: 0, right: 0 })
    }

    /// Hand a URL or path to `xdg-open`, the desktop's handler.
    ///
    /// The validation is shared with the other Unix backends (`OpenError::InvalidTarget` for an
    /// empty or option-shaped target), so Wayland does not grow a second rule.
    fn open_external(&self, target: &str) -> Result<(), crate::platform::OpenError> {
        crate::platform::external::spawn_handler("xdg-open", &[], target)
    }

    /// Whether `xdg-open` is on this host's `PATH`.
    fn has_external_handler(&self) -> bool {
        crate::platform::external::program_is_on_path("xdg-open")
    }

    /// No Wayland core protocol exposes a high-contrast preference.
    ///
    /// The XDG portal does (`org.freedesktop.appearance` key `contrast`), and that is what this
    /// answers when the session has one. A headless compositor has no portal, so the accessor
    /// honestly returns `None` rather than a fabricated `false`.
    fn high_contrast(&self) -> Option<bool> {
        #[cfg(all(feature = "wayland-portal", unix_desktop))]
        {
            crate::platform::wayland::portal::contrast()
        }
        #[cfg(not(all(feature = "wayland-portal", unix_desktop)))]
        {
            None
        }
    }

    /// No Wayland core protocol exposes a reduced-motion preference.
    ///
    /// The portal has no standard key for it either, so this stays `None` rather than reading a
    /// private desktop-specific name.
    fn reduce_motion(&self) -> Option<bool> {
        None
    }

    /// The desktop's colour scheme from the XDG portal (`org.freedesktop.appearance`).
    ///
    /// `Some(true)` is dark and `Some(false)` is light. A no-preference answer (`0`) and a
    /// session with no portal both return `None`.
    fn system_theme(&self) -> Option<bool> {
        #[cfg(all(feature = "wayland-portal", unix_desktop))]
        {
            crate::platform::wayland::portal::color_scheme()
        }
        #[cfg(not(all(feature = "wayland-portal", unix_desktop)))]
        {
            None
        }
    }

    /// The locale named by the process environment.
    ///
    /// Wayland has no locale protocol; the POSIX variables are the same source every other
    /// process on the session reads. The value is interned for the style layer.
    fn system_locale(&self) -> Option<&'static str> {
        crate::platform::locale::intern(crate::platform::locale::from_environment())
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

    /// The installed families from fontconfig (`fc-list`); empty when it is absent.
    #[cfg(not(alloc_frugal))]
    fn system_fonts(&self) -> Vec<crate::platform::types::SystemFont> {
        crate::platform::fonts_unix::system_fonts()
    }

    #[cfg(alloc_frugal)]
    fn system_fonts(&self) -> Vec<crate::platform::types::SystemFont> {
        Vec::new()
    }

    fn displays(&self) -> Vec<crate::platform::Display> {
        #[cfg(all(feature = "wayland-native", unix_desktop))]
        {
            if let Some(display) = session_output(self) {
                return crate::compat::vec![display];
            }
        }
        Vec::new()
    }

    /// The one output this backend tracks is the display for every window it owns.
    fn display_of_window(&self, window_id: ObjectId) -> Option<crate::platform::DisplayId> {
        if !self.state.is_kind(window_id, WaylandHandleKind::Window) {
            return None;
        }
        #[cfg(all(feature = "wayland-native", unix_desktop))]
        {
            if let Some(display) = session_output(self) {
                return Some(display.id);
            }
        }
        None
    }

    /// A revision bumped by `wl_output` geometry/mode/scale events.
    fn display_generation(&self) -> u64 {
        #[cfg(all(feature = "wayland-native", unix_desktop))]
        {
            if let Ok(guard) = self.native_session.lock() {
                if let Some(session) = guard.as_ref() {
                    return session.state.output_generation;
                }
            }
        }
        0
    }

    /// Metadata of the tracked output.
    fn display_info(
        &self,
        display: crate::platform::DisplayId,
    ) -> Option<crate::platform::types::DisplayInfo> {
        #[cfg(all(feature = "wayland-native", unix_desktop))]
        {
            let item = session_output(self)?;
            if item.id != display {
                return None;
            }
            let refresh = {
                let guard = self.native_session.lock().ok()?;
                guard.as_ref()?.state.output_refresh_mhz
            };
            Some(crate::platform::types::DisplayInfo {
                refresh_rate_mhz: refresh,
                orientation: Some(if item.bounds.width >= item.bounds.height {
                    crate::core::Orientation::Horizontal
                } else {
                    crate::core::Orientation::Vertical
                }),
                color_space: None,
                hdr: None,
            })
        }
        #[cfg(not(all(feature = "wayland-native", unix_desktop)))]
        {
            let _ = display;
            None
        }
    }

    /// The tracked output's scale, when a native session exists; environment fallback otherwise.
    fn dpi_scale_factor(&self) -> f32 {
        #[cfg(all(feature = "wayland-native", unix_desktop))]
        {
            if let Some(display) = session_output(self) {
                return display.scale;
            }
        }
        // Attempt to detect DPI from environment when wayland-client is not wired.
        // Check common environment variables set by Wayland compositors.
        if let Ok(scale_str) = std::env::var("GDK_SCALE") {
            if let Ok(scale) = scale_str.parse::<f32>() {
                if scale > 0.0 {
                    return scale;
                }
            }
        }
        if let Ok(scale_str) = std::env::var("QT_SCALE_FACTOR") {
            if let Ok(scale) = scale_str.parse::<f32>() {
                if scale > 0.0 {
                    return scale;
                }
            }
        }
        if let Ok(scale_str) = std::env::var("RUST_WIDGETS_DPI_SCALE") {
            if let Ok(scale) = scale_str.parse::<f32>() {
                if scale > 0.0 {
                    return scale;
                }
            }
        }
        // Fallback: assume 96 DPI (1.0 scale factor).
        // When wayland-native feature is active, check the native session
        // for wl_output scale factor obtained from the compositor.
        #[cfg(all(feature = "wayland-native", unix_desktop))]
        {
            if let Ok(guard) = self.native_session.lock() {
                if let Some(ref session) = *guard {
                    let scale = session.state.dpi_scale;
                    if scale > 0.0 {
                        return scale;
                    }
                }
            }
        }
        1.0
    }

    // -----------------------------------------------------------------------
    // Initialization / lifecycle
    // -----------------------------------------------------------------------

    fn init(&self) {
        self.runtime.initialized.store(true, std::sync::atomic::Ordering::SeqCst);
        log::info!("[wayland] Platform initialized (state-only backend).");
    }

    fn run(&self) {
        self.runtime.running.store(true, std::sync::atomic::Ordering::SeqCst);
        // Check environment to provide diagnostics about Wayland session state.
        let wayland_display =
            std::env::var("WAYLAND_DISPLAY").unwrap_or_else(|_| String::from("wayland-0"));
        let xdg_session =
            std::env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| String::from("unknown"));
        log::info!(
            "[wayland] Platform running: XDG_SESSION_TYPE={}, WAYLAND_DISPLAY={}",
            xdg_session,
            wayland_display
        );

        // Start EventLoop with native pump for Wayland dispatch.
        // The EventLoop runs in a background thread and calls the pump
        // on each iteration to dispatch pending native Wayland events.
        // Meanwhile, this thread blocks waiting for the quit signal.
        let mut event_loop = EventLoop::new();
        #[cfg(all(feature = "wayland-native", not(alloc_frugal), unix_desktop))]
        if let Some(pump) = create_event_loop_pump() {
            event_loop.set_native_pump(pump);
        }
        event_loop.start();

        #[cfg(all(feature = "wayland-native", not(alloc_frugal), unix_desktop))]
        {
            self.run_native_event_loop();
        }

        #[cfg(not(all(feature = "wayland-native", not(alloc_frugal), unix_desktop)))]
        while self.runtime.running.load(std::sync::atomic::Ordering::SeqCst) {
            // The library-painted path's frame loop. This used to be `sleep(16)` and nothing
            // else, so no trigger was drained and no control advanced: resizes were never
            // applied and every animation stayed frozen. Advance the frame exactly as the
            // Linux backend's state-only loop does, then block when no frame is owed.
            let outcome = crate::drive_frame(FRAME_INTERVAL_MS);
            crate::platform::frame_wait::pause_between_frames(outcome.needs_another_frame);
        }
    }

    fn quit(&self) -> bool {
        self.runtime.running.store(false, std::sync::atomic::Ordering::SeqCst);
        // The native loop blocks in `poll()` (and, when no session exists, in
        // `pause_between_frames`), so storing the flag alone would delay the stop until the
        // idle timeout expires. Waking the frame wait makes the request visible immediately.
        crate::platform::frame_wait::wake();
        log::info!("[wayland] Platform quit.");
        // The loop reads `running` each tick, so the request is honoured from any thread.
        true
    }

    /// Release every registry entry the backend holds for `widget_id`.
    ///
    /// Beyond the authoritative `BackendState` record, the Wayland backend keeps
    /// per-widget entries in the menu bookkeeping (`menus`: the attachment map,
    /// the menu tree and the queued triggers). All must be purged, otherwise a UI
    /// rebuilt in a create/destroy loop would leak one entry per discarded widget.
    /// Every lock is scoped to its own statement so no two guards are ever held at
    /// the same time.
    ///
    /// The native session (`native_session`) is a single connection-wide object
    /// shared by every window rather than a per-widget registry, so there is
    /// nothing per-widget to release there and no protocol request is issued.
    fn destroy_widget(&self, widget_id: ObjectId) -> bool {
        if let Ok(mut menus) = self.menus.lock() {
            // Both directions of the attached-bar map, the menu subtree and the queues are
            // cleared by the shared rule (PLAT-08).
            menus.purge(widget_id);
        } else {
            log::error!("[wayland] destroy_widget: menu mutex poisoned");
        }

        // The state record is the authority on whether the widget existed.
        self.state.destroy_widget(widget_id)
    }

    // -----------------------------------------------------------------------
    // Window
    // -----------------------------------------------------------------------

    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        // Try native Wayland surface creation when the wayland-native feature is active
        #[cfg(all(feature = "wayland-native", unix_desktop))]
        if let Some(id) = self.try_create_native_window(title, x, y, width, height) {
            return id;
        }
        // Fallback: state-only window
        self.insert_widget(WaylandHandleKind::Window, title, x, y, width, height)
    }

    // -----------------------------------------------------------------------
    // Menu system
    // -----------------------------------------------------------------------

    /// Creates the root of the in-process menu model and attaches nothing to the
    /// compositor.
    ///
    /// Wayland has no menu protocol, so this is a model node rather than a
    /// compositor object. Without this producer the consumers below
    /// (`attach_menu_bar_to_window`, `menu_add_item`, `inject_menu_trigger`) could
    /// never succeed: they all validate the parent's kind first, and no `MenuBar`
    /// was ever inserted, so each returned `false`/`0` unconditionally. That was an
    /// unfinished part of the BLUE15 migration rather than a Wayland limitation —
    /// the model is exactly the mechanism this backend is supposed to offer.
    fn create_menu_bar(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        // A menu bar hangs off a window; without one there is nothing to attach to.
        if !matches!(self.state.kind_of(parent), Some(WaylandHandleKind::Window)) {
            return 0;
        }
        let id = self.insert_widget(WaylandHandleKind::MenuBar, "", x, y, width, height);
        if let Ok(mut menus) = self.menus.lock() {
            menus.attached_menu_bar.insert(parent, id);
        } else {
            log::error!("[wayland] create_menu_bar: mutex poisoned");
        }
        id
    }

    /// Creates a dropdown `WaylandHandleKind::Menu` node under `parent`.
    ///
    /// `parent` may be a menu bar (a top-level menu) or another menu (a submenu),
    /// which is what makes arbitrarily nested menus expressible. The child link is
    /// recorded so a host walking the model can render the hierarchy.
    fn create_menu(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if !matches!(
            self.state.kind_of(parent),
            Some(WaylandHandleKind::MenuBar | WaylandHandleKind::Menu)
        ) {
            return 0;
        }
        let id = self.insert_widget(WaylandHandleKind::Menu, text, x, y, width, height);
        if let Ok(mut menus) = self.menus.lock() {
            menus.menu_children.entry(parent).or_default().push(id);
        } else {
            log::error!("[wayland] create_menu: mutex poisoned");
        }
        id
    }

    /// Attach a menu bar to a window.
    ///
    /// Wayland has no menu protocol at all: `xdg_shell` models toplevels and
    /// popups, and neither carries menu chrome. A `MenuBar`/`Menu`/`MenuItem` here
    /// is therefore an **in-process data model**, not a control the compositor
    /// builds — which is why this method survives the BLUE15 control-construction
    /// removal while every `create_<control>` override around it does not.
    ///
    /// This is an injectable event capability: the attachment only records which
    /// menu bar belongs to which window so the host can render the menu itself and
    /// feed activations back through [`Platform::inject_menu_trigger`]. It creates
    /// nothing in the compositor. A menu bar can only be attached in-process, so
    /// `capabilities().native_menu` is advertised as `false`; claiming otherwise
    /// would be the exact lie BLUE15 removes.
    fn attach_menu_bar_to_window(&self, window: ObjectId, menu_bar: ObjectId) -> bool {
        // Both the window and the menu bar must be live widgets of the right
        // kind, otherwise the attachment map would hold an invalid pair.
        if !matches!(self.state.kind_of(window), Some(WaylandHandleKind::Window)) {
            return false;
        }
        if !matches!(self.state.kind_of(menu_bar), Some(WaylandHandleKind::MenuBar)) {
            return false;
        }
        if let Ok(mut menus) = self.menus.lock() {
            menus.attached_menu_bar.insert(window, menu_bar);
            true
        } else {
            log::error!("[wayland] attach_menu_bar_to_window: mutex poisoned");
            false
        }
    }

    fn menu_add_item(&self, parent_menu: ObjectId, text: &str, shortcut: Option<&str>) -> ObjectId {
        if !matches!(self.state.kind_of(parent_menu), Some(WaylandHandleKind::Menu)) {
            return 0;
        }
        let display = if let Some(shortcut) = shortcut {
            format!("{}\t{}", text, shortcut)
        } else {
            text.to_string()
        };
        // The item text carries the shortcut in its display form. No accelerator is
        // registered with the compositor: Wayland has no menu protocol, so the
        // library renders the text and the host routes activations back in.
        let id = self.insert_widget(WaylandHandleKind::MenuItem, &display, 0, 0, 0, 0);
        if let Ok(mut menus) = self.menus.lock() {
            menus.menu_children.entry(parent_menu).or_default().push(id);
        }
        id
    }

    fn poll_menu_triggered(&self) -> Option<ObjectId> {
        if let Ok(mut menus) = self.menus.lock() {
            menus.pending_menu_events.pop_front()
        } else {
            log::error!("[wayland] poll_menu_triggered: mutex poisoned");
            None
        }
    }

    fn inject_menu_trigger(&self, menu_item_id: ObjectId) -> bool {
        // Only a menu item may produce a menu trigger.
        if !matches!(self.state.kind_of(menu_item_id), Some(WaylandHandleKind::MenuItem)) {
            return false;
        }
        if let Ok(mut menus) = self.menus.lock() {
            menus.pending_menu_events.push_back(menu_item_id);
            true
        } else {
            log::error!("[wayland] inject_menu_trigger: mutex poisoned");
            false
        }
    }

    // -----------------------------------------------------------------------
    // Widget trigger events
    // -----------------------------------------------------------------------

    fn poll_widget_triggered(&self) -> Option<ObjectId> {
        if let Ok(mut menus) = self.menus.lock() {
            menus.pending_widget_events.pop_front().map(|e| e.widget_id)
        } else {
            log::error!("[wayland] poll_widget_triggered: mutex poisoned");
            None
        }
    }

    fn poll_widget_trigger_event(&self) -> Option<WidgetTriggerEvent> {
        if let Ok(mut menus) = self.menus.lock() {
            menus.pending_widget_events.pop_front()
        } else {
            log::error!("[wayland] poll_widget_trigger_event: mutex poisoned");
            None
        }
    }

    fn inject_widget_trigger_event(&self, widget_id: ObjectId, kind: WidgetTriggerKind) -> bool {
        if !self.state.contains_widget(widget_id) {
            return false;
        }
        if let Ok(mut menus) = self.menus.lock() {
            menus.pending_widget_events.push_back(WidgetTriggerEvent { widget_id, kind });
            true
        } else {
            log::error!("[wayland] inject_widget_trigger_event: mutex poisoned");
            false
        }
    }

    /// The window's current client size, as last reported by the host.
    ///
    /// Falls back to the size the window was created with. `None` for an id this backend
    /// does not know, so a caller can tell "no such window" from "a size I can use".
    fn window_client_size(&self, window_id: ObjectId) -> Option<(u32, u32)> {
        // Ask the control backend, which owns the window and is therefore the only
        // store that knows the size a resize reported.
        crate::window_client_size(window_id).or_else(|| self.state.window_size(window_id))
    }

    /// Reports a container's new client size and queues a `Resized` trigger.
    fn queue_resize_trigger(&self, window_id: ObjectId, width: u32, height: u32) -> bool {
        // Forward to the control backend, which owns the window and the queue the app
        // polls. Writing to the platform's own state would land in a store the host
        // never reads, because `create_window` goes through the control backend.
        crate::queue_resize_trigger(window_id, width, height)
    }

    // -----------------------------------------------------------------------
    // Widget surfaces
    // -----------------------------------------------------------------------

    /// Whether this backend can host library-painted widgets.
    ///
    /// # Why this was `false` and is now `true`
    ///
    /// This backend inherited the trait's `false` because it implemented no surface method at
    /// all — no `mount_surface`, `resize_surface`, `unmount_surface` or `invalidate_surface`. It
    /// therefore could not display a library-painted control, even though it is otherwise a
    /// complete backend: it talks to a real compositor, creates a real surface and reports its
    /// size, and its **host** feeds pointer and key input back in through the injected-trigger
    /// channel.
    ///
    /// That last clause is deliberately not "it reports input". This backend binds no
    /// `wl_seat`, so it receives no pointer or keyboard events of its own; the host application
    /// owns them and routes them to the widget layer. Saying it "reports input" attributed to
    /// the backend a capability that lives in the host, which is the same class of claim
    /// `capabilities` above had to correct for `ime`/`accessibility`.
    ///
    /// The gap was only the per-widget table. A Wayland window's pixels come from the same
    /// record-plus-queue shape every other non-desktop backend uses ([`BackendState`]), and this
    /// backend already owns a `BackendState` — so the fix is the table, not a new mechanism.
    ///
    /// # What a host does with it
    ///
    /// Mount each widget, then each frame/redraw call [`crate::take_pending_repaint`] to learn
    /// which went stale and `widget::runtime::render_frame_cached` to get its RGBA for the
    /// `wl_shm` buffer. `true` says the mount will succeed; it promises nothing about how the
    /// compositor presents the buffer.
    ///
    /// [`BackendState`]: crate::platform::state::BackendState
    fn supports_surfaces(&self) -> bool {
        true
    }

    /// Mounts a widget onto a surface this backend will present.
    ///
    /// `false` for an id this backend did not create, so a host is told rather than recorded
    /// into a table nothing can render.
    fn mount_surface(&self, _parent: ObjectId, id: ObjectId, rect: crate::core::Rect) -> bool {
        self.state.mount_surface_record(id, rect)
    }

    /// Updates the rect of a mounted surface. `false` when `id` is not mounted.
    fn resize_surface(&self, id: ObjectId, rect: crate::core::Rect) -> bool {
        self.state.resize_surface_record(id, rect)
    }

    /// Releases a mounted surface.
    fn unmount_surface(&self, id: ObjectId) -> bool {
        self.state.unmount_surface_record(id)
    }

    /// Queues a repaint for the compositor loop to pick up. `false` when `id` is unknown.
    ///
    /// A **window** id is accepted as well as a mounted surface's: the library repaints a window
    /// to reveal the ordinary children drawn into that window's frame, and answering only for
    /// mounted surfaces made those requests silent no-ops on every record-backed backend.
    fn invalidate_surface(&self, id: ObjectId) -> bool {
        self.state.record_repaint_request(id)
    }

    /// Pops the next widget awaiting a repaint.
    ///
    /// The drain half of [`Self::invalidate_surface`]: without it the queue would grow without
    /// bound and the compositor would never learn what to redraw.
    fn take_pending_repaint(&self) -> Option<ObjectId> {
        self.state.take_pending_repaint()
    }

    // -----------------------------------------------------------------------
    // Widget lifecycle operations
    // -----------------------------------------------------------------------

    // -----------------------------------------------------------------------
    // IME / Accessibility
    // -----------------------------------------------------------------------

    // -----------------------------------------------------------------------
    // Clipboard
    // -----------------------------------------------------------------------

    fn set_clipboard_text(&self, text: &str) -> bool {
        // The record keeps the last value this process wrote, so a read before the compositor
        // hands the selection back still answers what was copied; the rich backend is what
        // actually reaches the system clipboard.
        let stored = self.state.set_clipboard_text(text);
        let published = self
            .clipboard
            .set_contents(crate::platform::clipboard::ClipboardContent::Text(text.to_string()));
        stored && published
    }

    fn get_clipboard_text(&self) -> String {
        self.clipboard.text().unwrap_or_else(|| self.state.clipboard_text())
    }

    /// The `wl_data_device` clipboard backend (B28.04).
    ///
    /// Present in every build of this module; its writes fall back to the in-process record when
    /// no live session exists, which is the `capabilities().clipboard == false` case.
    fn clipboard_backend(&self) -> Option<&dyn crate::platform::clipboard::RichClipboardBackend> {
        Some(&self.clipboard)
    }

    /// A revision bumped by selection changes and by writes through this backend.
    fn clipboard_generation(&self) -> u64 {
        self.clipboard.generation()
    }

    /// The `text-input-v3` IME bridge (B28.12).
    ///
    /// Present in every build of this module; `capabilities().ime` is true only when the
    /// compositor actually advertised `zwp_text_input_manager_v3`.
    fn ime_bridge(&self) -> Option<&dyn crate::platform::ime::ImeBridge> {
        Some(&self.ime)
    }

    /// Asks the compositor for a frame callback for `window` (B28.42).
    ///
    /// The callback is delivered to [`WlCallback`]'s dispatch impl on the loop thread; this method
    /// never starts a frame loop of its own. When the session is checked out for dispatch the
    /// request is skipped — the loop is already servicing the compositor and will schedule the
    /// next frame on its own tick.
    fn pre_present_notify(&self, window: ObjectId) {
        #[cfg(all(feature = "wayland-native", unix_desktop))]
        {
            let Ok(mut guard) = self.native_session.try_lock() else { return };
            let Some(session) = guard.as_mut() else { return };
            let Some(surface) = session.state.surfaces.get(&window).cloned() else { return };
            let qh = session.event_queue.handle();
            let _callback = surface.frame(&qh, ());
            if let Err(e) = session.event_queue.flush() {
                log::warn!("[wayland] frame request flush failed: {e}");
            }
        }
    }

    // -----------------------------------------------------------------------
    // Drag and drop
    // -----------------------------------------------------------------------
}

// ---------------------------------------------------------------------------
// Native Wayland window creation (gated by "wayland-native" feature)
// ---------------------------------------------------------------------------

#[cfg(all(feature = "wayland-native", unix_desktop))]
impl WaylandPlatform {
    /// Run the real Wayland event loop.
    ///
    /// Blocks on the connection's socket via `prepare_read()` → `poll()` so the
    /// thread sleeps until the compositor actually sends something, instead of
    /// busy-waiting. A `poll` timeout bounds how long the loop can sit idle
    /// before re-checking the `running` flag, so `quit()` is honoured promptly
    /// even with no incoming events.
    ///
    /// Falls back to the timed polling loop when no Wayland session exists
    /// (headless CI), which keeps `run()`/`quit()` semantics identical.
    #[cfg(all(feature = "wayland-native", not(alloc_frugal), unix_desktop))]
    fn run_native_event_loop(&self) {
        use std::os::fd::AsRawFd;

        // Idle timeout: bounds quit latency when the compositor is silent.
        const IDLE_TIMEOUT_MS: i32 = 50;

        log::info!("[wayland] Entering native fd-based event loop");
        loop {
            if !self.runtime.running.load(std::sync::atomic::Ordering::SeqCst) {
                break;
            }

            // Move the session out of the mutex for this iteration so the dispatch below —
            // which runs compositor callbacks, including a close handler that may open a new
            // window — can re-enter `native_session` without deadlocking on this non-recursive
            // mutex. It is returned at the end of the iteration.
            let mut session = match self
                .native_session
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .take()
            {
                Some(session) => session,
                None => {
                    // No native session yet (no window created): stay responsive to
                    // quit without spinning.
                    // The drain still runs: a trigger can be queued before a session exists
                    // (a host reporting a size for a window it manages itself), and leaving it
                    // in the queue until a session appears would delay a layout run for as long
                    // as the window is absent. See `crate::drain_triggers`.
                    //
                    // `crate::drive_frame` rather than the bare drain: the animation step has to
                    // run even in this session-less arm, or a transition that started before the
                    // window appeared would freeze instead of finishing (BLUE24 §0A.1
                    // measurement 1).
                    let outcome = crate::drive_frame(FRAME_INTERVAL_MS);
                    // Same wait entry point as every other backend: an owed frame wakes
                    // immediately, an idle loop blocks until an event or the bounded timeout.
                    // The raw `sleep(16)` was a busy-wait that also discarded the driver's
                    // "another frame owed" answer.
                    crate::platform::frame_wait::pause_between_frames(outcome.needs_another_frame);
                    continue;
                }
            };

            // Publish the checked-out session so a re-entrant `create_window` reuses this
            // connection rather than opening a second one. Cleared before the session is
            // returned to the mutex below.
            DISPATCHING_SESSION.with(|slot| {
                *slot.borrow_mut() = Some(&mut session as *mut WaylandSession);
            });

            // Flush any queued requests before waiting for the reply.
            if let Err(e) = session.event_queue.flush() {
                log::warn!("[wayland] flush failed: {e}");
            }

            // Block until either the socket is readable or the timeout expires.
            match session.event_queue.prepare_read() {
                Some(read_guard) => {
                    let fd = read_guard.connection_fd().as_raw_fd();
                    let mut pollfd = libc::pollfd { fd, events: libc::POLLIN, revents: 0 };
                    let rc = unsafe { libc::poll(&mut pollfd, 1, IDLE_TIMEOUT_MS) };
                    if rc > 0 {
                        // Data available: read it, then dispatch to handlers.
                        if let Err(e) = read_guard.read() {
                            log::warn!("[wayland] read failed: {e}");
                        }
                        if let Err(e) = session.event_queue.dispatch_pending(&mut session.state) {
                            log::warn!("[wayland] dispatch failed: {e}");
                        }
                    } else if rc < 0 {
                        let err = std::io::Error::last_os_error();
                        log::warn!("[wayland] poll failed: {err}");
                        crate::platform::frame_wait::pause_between_frames(false);
                    }
                    // rc == 0: idle timeout, loop to re-check `running`.
                }
                None => {
                    // Events are already buffered; drain them without blocking.
                    let _ = session.event_queue.dispatch_pending(&mut session.state);
                }
            }

            // Return the session before running the library frame, so a frame that
            // re-enters `native_session` (positions a widget, opens a window) never
            // deadlocks on the mutex the dispatch above has already released.
            DISPATCHING_SESSION.with(|slot| *slot.borrow_mut() = None);
            *self.native_session.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) =
                Some(session);

            // One library frame after the protocol dispatch for this iteration.
            //
            // Without the drain, a `Resized` event queued by a host never reached a window
            // layout — the protocol events were dispatched and the library's own queue was
            // never read. Without the animation step that follows it, every hover fade and
            // caret blink was inert on this backend for the same reason it was on the
            // others (BLUE24 §0A.1 measurement 1). See `crate::drive_frame`.
            crate::drive_frame(FRAME_INTERVAL_MS);
        }
        log::info!("[wayland] Native event loop exited");
    }

    /// Dispatch pending native Wayland events.
    ///
    /// Intended to be called from an `EventLoop` native pump callback
    /// on each iteration so that Wayland protocol events are dispatched
    /// without a separate blocking `run()` loop.
    #[cfg(not(alloc_frugal))]
    pub(crate) fn dispatch_native_events(&self) {
        // Move the session out for the dispatch so a compositor callback that opens a
        // window can re-enter `native_session` without deadlocking (the same rule the
        // `run()` loop follows). Returned before the frame, which is the pump-callback
        // spelling of the same tick, animation step included.
        let Some(mut session) =
            self.native_session.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).take()
        else {
            crate::drive_frame(FRAME_INTERVAL_MS);
            return;
        };
        DISPATCHING_SESSION.with(|slot| {
            *slot.borrow_mut() = Some(&mut session as *mut WaylandSession);
        });
        let _ = session.event_queue.dispatch_pending(&mut session.state);
        DISPATCHING_SESSION.with(|slot| *slot.borrow_mut() = None);
        *self.native_session.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) =
            Some(session);
        crate::drive_frame(FRAME_INTERVAL_MS);
    }

    /// Attempt to create a native Wayland xdg_toplevel for this window.
    /// Returns `Some(id)` on success, or `None` to fall back to state-only.
    fn try_create_native_window(
        &self,
        title: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> Option<ObjectId> {
        // Helper: connect to Wayland display and discover globals.
        // Returns `Some(WaylandSession)` on success or `None` on failure.
        fn connect_wayland(
            clipboard: super::clipboard::WaylandClipboard,
            ime: super::ime::WaylandIme,
        ) -> Option<WaylandSession> {
            let conn = wl_client::Connection::connect_to_env()
                .inspect_err(|e| {
                    log::error!("[wayland] Failed to connect to Wayland display: {}", e)
                })
                .ok()?;
            let display = conn.display();
            let mut event_queue = conn.new_event_queue();
            let qh = event_queue.handle();
            let _registry = display.get_registry(&qh, ());

            clipboard.clear_channel();
            ime.clear_channel();
            let mut state = WaylandSessionState {
                compositor: None,
                xdg_wm_base: None,
                seat: None,
                data_device_manager: None,
                data_device: None,
                keyboard: None,
                pointer: None,
                clipboard,
                text_input_manager: None,
                text_input: None,
                ime,
                surfaces: std::collections::HashMap::new(),
                shm: None,
                shm_surfaces: std::collections::HashMap::new(),
                frame_done_generation: 0,
                wl_output: None,
                dpi_scale: 1.0,
                output_x: 0,
                output_y: 0,
                output_width: 0,
                output_height: 0,
                output_refresh_mhz: None,
                output_ready: false,
                output_generation: 0,
            };
            event_queue
                .roundtrip(&mut state)
                .inspect_err(|e| log::error!("[wayland] Registry roundtrip failed: {}", e))
                .ok()?;

            if state.compositor.is_none() {
                log::error!("[wayland] wl_compositor global not available");
                return None;
            }
            if state.xdg_wm_base.is_none() {
                log::error!("[wayland] xdg_wm_base global not available");
                return None;
            }

            log::info!("[wayland] Connected to display; compositor and xdg_wm_base bound.");
            Some(WaylandSession { conn, event_queue, state })
        }

        // If the event loop has the session checked out for a dispatch pass, a re-entrant
        // `create_window` (e.g. from a close handler) must reuse it: locking `native_session`
        // here would deadlock, and opening a second session would create a window whose
        // events no loop ever reads.
        let reentrant = DISPATCHING_SESSION.with(|slot| *slot.borrow());
        if let Some(ptr) = reentrant {
            // SAFETY: `ptr` names the session owned by `run_native_event_loop` /
            // `dispatch_native_events` on this same thread, live for the whole dispatch
            // window (the slot is cleared before the session returns to the mutex).
            let session = unsafe { &mut *ptr };
            return self.create_toplevel(session, title, x, y, width, height);
        }

        // Obtain or create the persistent Wayland session.
        let mut guard = self.native_session.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if guard.is_none() {
            *guard = connect_wayland(self.clipboard.clone(), self.ime.clone());
        }
        let session = guard.as_mut()?;
        self.create_toplevel(session, title, x, y, width, height)
    }

    /// Creates an `xdg_toplevel` on `session` and registers it in the state backend.
    ///
    /// Split out of [`Self::try_create_native_window`] so both the lock-holding path and
    /// the re-entrant (session-checked-out) path share one toplevel construction.
    fn create_toplevel(
        &self,
        session: &mut WaylandSession,
        title: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> Option<ObjectId> {
        // 5. Create wl_surface, xdg_surface, and xdg_toplevel
        let compositor = session.state.compositor.as_ref()?;
        let xdg_wm_base = session.state.xdg_wm_base.as_ref()?;
        let qh = session.event_queue.handle();

        let surface = compositor.create_surface(&qh, ());
        let xdg_surface = xdg_wm_base.get_xdg_surface(&surface, &qh, ());
        let toplevel = xdg_surface.get_toplevel(&qh, ());

        // 6. Set window properties
        toplevel.set_title(title.to_string());
        toplevel.set_app_id("rust_widgets".to_string());
        if width > 0 && height > 0 {
            toplevel.set_min_size(width as i32, height as i32);
        }

        // 7. Commit the surface to make the compositor aware of it
        surface.commit();

        // 8. Roundtrip to process the xdg_surface.configure event
        session.event_queue.roundtrip(&mut session.state).ok()?;

        log::info!("[wayland] Created xdg_toplevel '{}' ({}x{})", title, width, height);

        let id = self.insert_widget(WaylandHandleKind::Window, title, x, y, width, height);
        // Keep the surface so `pre_present_notify` can request a `wl_surface.frame` callback for
        // this window (B28.42). A clone shares the same protocol object.
        session.state.surfaces.insert(id, surface.clone());
        session.state.attach_placeholder_buffer(&surface, &qh, id, width, height);
        log::info!("[wayland] Window {} registered with state backend", id);

        // Record which window this `xdg_toplevel`'s configure events belong to. The
        // dispatch handler is a free function with no access to the platform object, so this
        // is how a compositor-driven resize finds its way back to the layout. Keyed by the
        // proxy, so a second window cannot displace the first.
        record_configured_window(&toplevel, id);

        Some(id)
    }
}

// The window each session's `xdg_toplevel` configure events describe.
//
// Thread-local because a Wayland connection belongs to the thread that created it, and
// a compositor callback arrives on that thread.
//
// # Why a map keyed by the proxy, and not one slot
//
// This was a single `Cell<ObjectId>` that `create_window` overwrote on every call. With
// two toplevels the second `create_window` replaced the first's id, so **every** later
// configure event was reported against the most recently created window: the wrong
// window's size was recorded, the wrong window's layout re-ran with the other window's
// dimensions, and the window the compositor actually configured never re-laid-out. It was
// undetectable from the outside, because `window_client_size` answers from the recorded
// size first and the wrong record therefore looked authoritative.
//
// The `xdg_toplevel` proxy is what the compositor names in the callback, so it is the
// correct key. Keying on it is also the only version that survives a second window, which
// the single slot could not express at all.
//
// A plain comment rather than a doc comment: rustdoc generates no documentation for a
// macro invocation, so a `///` here is an unused doc comment (a denied warning).
#[cfg(all(feature = "wayland-native", unix_desktop))]
thread_local! {
    static CONFIGURED_WINDOWS: core::cell::RefCell<crate::compat::HashMap<usize, ObjectId>> =
        core::cell::RefCell::new(crate::compat::HashMap::new());
}

// The session checked out by the event loop for a dispatch pass.
//
// While `run_native_event_loop` or `dispatch_native_events` is dispatching compositor events, the
// session is moved out of `native_session` so a re-entrant `create_window` (a close handler that
// opens a new window) can reach it without deadlocking on the same non-recursive mutex. This
// thread-local lets that call reuse the checked-out session instead of opening a second connection
// whose events nobody would dispatch.
//
// Written as `//` rather than `///`: a doc comment on a `thread_local!` invocation documents
// nothing, and rustdoc says so (`unused_doc_comments`) — the note belongs in the source, not in a
// doc block attached to a macro.
#[cfg(all(feature = "wayland-native", unix_desktop))]
thread_local! {
    static DISPATCHING_SESSION: core::cell::RefCell<Option<*mut WaylandSession>> =
        const { core::cell::RefCell::new(None) };
}

/// Records that `proxy`'s configure events describe the widget `id`.
///
/// Called once per window at creation. The proxy is identified by its object address, which
/// is stable for the life of the `wl_proxy`: `wayland-client` keeps the proxy alive in the
/// connection's object map for as long as the compositor may send it events.
#[cfg(all(feature = "wayland-native", unix_desktop))]
fn record_configured_window(proxy: &impl wl_client::Proxy, id: ObjectId) {
    let key = proxy.id().protocol_id() as usize;
    CONFIGURED_WINDOWS.with(|map| {
        map.borrow_mut().insert(key, id);
    });
}

/// The window to report a configure event against, if one is known.
#[cfg(all(feature = "wayland-native", unix_desktop))]
pub(crate) fn configured_window_id(proxy: &impl wl_client::Proxy) -> Option<ObjectId> {
    let key = proxy.id().protocol_id() as usize;
    CONFIGURED_WINDOWS.with(|map| map.borrow().get(&key).copied())
}

// ---------------------------------------------------------------------------
// Wayland session state & dispatch implementations
// ---------------------------------------------------------------------------

/// Dispatch state for the Wayland session.
/// Holds the global proxies obtained from the registry.
#[cfg(all(feature = "wayland-native", unix_desktop))]
pub(crate) struct WaylandSessionState {
    pub(crate) compositor: Option<wl_client::protocol::wl_compositor::WlCompositor>,
    pub(crate) xdg_wm_base: Option<wl_protocols::xdg::shell::client::xdg_wm_base::XdgWmBase>,
    /// The seat the data device (and the serials `set_selection` needs) come from.
    pub(crate) seat: Option<wl_client::protocol::wl_seat::WlSeat>,
    /// Creates data sources and the data device.
    pub(crate) data_device_manager:
        Option<wl_client::protocol::wl_data_device_manager::WlDataDeviceManager>,
    /// The selection channel; only present when both globals above were bound.
    pub(crate) data_device: Option<wl_client::protocol::wl_data_device::WlDataDevice>,
    /// Bound to receive keyboard serials; the library does not route its events.
    #[allow(dead_code)]
    pub(crate) keyboard: Option<wl_client::protocol::wl_keyboard::WlKeyboard>,
    /// Bound to receive pointer serials; the library does not route its events.
    #[allow(dead_code)]
    pub(crate) pointer: Option<wl_client::protocol::wl_pointer::WlPointer>,
    /// The clipboard state shared with the platform and the data-device dispatch.
    pub(crate) clipboard: super::clipboard::WaylandClipboard,
    /// The text-input manager, when the compositor advertises `text-input-v3`.
    pub(crate) text_input_manager: Option<
        wl_protocols::wp::text_input::zv3::client::zwp_text_input_manager_v3::ZwpTextInputManagerV3,
    >,
    /// The seat's text-input object.
    pub(crate) text_input:
        Option<wl_protocols::wp::text_input::zv3::client::zwp_text_input_v3::ZwpTextInputV3>,
    /// The IME state shared with the platform and the text-input dispatch.
    pub(crate) ime: super::ime::WaylandIme,
    /// The `wl_surface` of every window this session created, for `pre_present_notify`.
    pub(crate) surfaces: std::collections::HashMap<u64, wl_client::protocol::wl_surface::WlSurface>,
    /// Shared-memory global, used to give a window a mapped buffer.
    ///
    /// A Wayland toplevel with no attached buffer is never mapped, so it never receives
    /// keyboard focus, so its `wl_data_device` never receives a `selection` event. That made the
    /// clipboard advertise a live channel it could not actually read from. A minimal opaque
    /// buffer makes the surface real for focus purposes; the library still paints through its
    /// control backend.
    pub(crate) shm: Option<wl_client::protocol::wl_shm::WlShm>,
    /// Placeholder buffers kept alive for as long as their window exists.
    pub(crate) shm_surfaces: std::collections::HashMap<u64, ShmSurface>,
    /// Bumped by every `wl_callback.done`; a frame callback landed.
    pub(crate) frame_done_generation: u64,
    /// wl_output proxy for DPI scale detection.
    pub(crate) wl_output: Option<wl_client::protocol::wl_output::WlOutput>,
    /// Logical-to-physical scale reported by the compositor. It scales the
    /// drawing surface and so the input-coordinate mapping, and is set by the
    /// `wl_output::scale` event; it stays 1.0 until then.
    pub(crate) dpi_scale: f32,
    /// The output's desktop position from `wl_output.geometry`.
    pub(crate) output_x: i32,
    /// The output's desktop position from `wl_output.geometry`.
    pub(crate) output_y: i32,
    /// The current mode's pixel width (`wl_output.mode`).
    pub(crate) output_width: u32,
    /// The current mode's pixel height (`wl_output.mode`).
    pub(crate) output_height: u32,
    /// The current mode's refresh rate in millihertz.
    pub(crate) output_refresh_mhz: Option<u32>,
    /// Whether a current mode has been reported, so a zero size is not mistaken for a display.
    pub(crate) output_ready: bool,
    /// A revision bumped by every geometry/mode/scale change, for `display_generation`.
    pub(crate) output_generation: u64,
}

/// A mapped placeholder buffer: the shm pool and buffer proxies plus the mapping to release.
#[cfg(all(feature = "wayland-native", unix_desktop))]
pub(crate) struct ShmSurface {
    _pool: wl_client::protocol::wl_shm_pool::WlShmPool,
    _buffer: wl_client::protocol::wl_buffer::WlBuffer,
    /// Address and length of the `mmap` that backs the pool. Stored as integers so the state
    /// stays `Send`; the memory is only ever unmapped in `Drop`.
    address: usize,
    len: usize,
}

#[cfg(all(feature = "wayland-native", unix_desktop))]
impl Drop for ShmSurface {
    fn drop(&mut self) {
        if self.address != 0 && self.len != 0 {
            // SAFETY: `address`/`len` are exactly the mapping created in
            // `attach_placeholder_buffer`, and this `Drop` runs once.
            unsafe {
                libc::munmap(self.address as *mut libc::c_void, self.len);
            }
        }
    }
}

/// A persistent Wayland session containing the connection, event queue,
/// and proxy state. Stored inside `WaylandPlatform.native_session`.
///
/// This is one object per connection, shared by every window — not a per-widget
/// registry — so no per-widget protocol object is kept here. Widgets other than the
/// window are painted by the library, so the host has nothing to create for them.
#[cfg(all(feature = "wayland-native", unix_desktop))]
pub(crate) struct WaylandSession {
    /// The Wayland connection. Must stay alive while `event_queue` is in use.
    #[allow(dead_code)]
    pub(crate) conn: wl_client::Connection,
    pub(crate) event_queue: wl_client::EventQueue<WaylandSessionState>,
    pub(crate) state: WaylandSessionState,
}

#[cfg(all(feature = "wayland-native", unix_desktop))]
impl WaylandSessionState {
    /// Attaches a minimal opaque buffer so the toplevel is mapped and can take focus.
    ///
    /// # Why this is necessary for the clipboard
    ///
    /// Wayland sends `wl_data_device.selection` only to clients with keyboard focus, and a
    /// toplevel with no buffer is never mapped, so it never gets focus. Without this the backend
    /// advertised `clipboard: true` but could never read an external selection. The buffer is
    /// deliberately a flat opaque fill: the host paints controls through the control backend, and
    /// this surface exists for focus/selection semantics rather than pixels.
    fn attach_placeholder_buffer(
        &mut self,
        surface: &wl_client::protocol::wl_surface::WlSurface,
        qh: &wl_client::QueueHandle<WaylandSessionState>,
        id: ObjectId,
        width: u32,
        height: u32,
    ) -> bool {
        use std::os::fd::{AsFd, FromRawFd, OwnedFd};

        let Some(shm) = self.shm.clone() else {
            return false;
        };
        let width = width.max(1);
        let height = height.max(1);
        let stride = (width * 4) as i32;
        let len = (stride as usize) * height as usize;

        // POSIX shared memory: create a uniquely named object, unlink it immediately (the fd is
        // the only handle), size it, then map it for the compositor to read.
        let name = format!("/rw-wl-{}-{id}", std::process::id());
        let Ok(name) = std::ffi::CString::new(name) else {
            return false;
        };
        // SAFETY: the name is a valid NUL-terminated string and the flags are a valid `open`
        // combination for `shm_open`.
        let fd = unsafe {
            libc::shm_open(name.as_ptr(), libc::O_CREAT | libc::O_EXCL | libc::O_RDWR, 0o600)
        };
        if fd < 0 {
            log::warn!("[wayland] shm_open failed for the placeholder buffer");
            return false;
        }
        // SAFETY: `fd` is our fresh descriptor; unlinking the name after creation is the usual
        // way to make it private without leaking a filesystem entry.
        unsafe {
            libc::shm_unlink(name.as_ptr());
        }
        // SAFETY: `fd` is valid and `len` is the requested size.
        if unsafe { libc::ftruncate(fd, len as libc::off_t) } != 0 {
            // SAFETY: `fd` is owned here and not wrapped anywhere yet.
            unsafe { libc::close(fd) };
            return false;
        }
        // SAFETY: `fd` is valid, `len` > 0, and the mapping is released in `ShmSurface::drop`.
        let address = unsafe {
            libc::mmap(
                core::ptr::null_mut(),
                len,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                fd,
                0,
            )
        };
        if address == libc::MAP_FAILED {
            // SAFETY: `fd` is owned here and not wrapped anywhere yet.
            unsafe { libc::close(fd) };
            return false;
        }
        // SAFETY: the mapping is `len` bytes and writable; filling it makes the surface opaque.
        unsafe {
            core::ptr::write_bytes(address as *mut u8, 0xFF, len);
        }
        // SAFETY: `fd` is a fresh descriptor whose ownership moves into `OwnedFd` exactly once.
        let owned = unsafe { OwnedFd::from_raw_fd(fd) };
        let pool = shm.create_pool(owned.as_fd(), len as i32, qh, ());
        let buffer = pool.create_buffer(
            0,
            width as i32,
            height as i32,
            stride,
            wl_client::protocol::wl_shm::Format::Argb8888,
            qh,
            (),
        );
        surface.attach(Some(&buffer), 0, 0);
        surface.damage(0, 0, width as i32, height as i32);
        surface.commit();
        self.shm_surfaces.insert(
            id,
            ShmSurface { _pool: pool, _buffer: buffer, address: address as usize, len },
        );
        log::debug!("[wayland] attached placeholder buffer {width}x{height} for window {id}");
        true
    }

    /// Creates the `wl_data_device` once both the seat and the manager are known.
    ///
    /// The registry may deliver either global first, so both arms call this after binding; it is
    /// idempotent and does nothing until the pair exists.
    fn ensure_data_device(
        &mut self,
        qh: &wl_client::QueueHandle<WaylandSessionState>,
        conn: &wl_client::Connection,
    ) {
        if self.data_device.is_some() {
            return;
        }
        let (Some(manager), Some(seat)) = (self.data_device_manager.as_ref(), self.seat.as_ref())
        else {
            return;
        };
        let device = manager.get_data_device(seat, qh, ());
        self.clipboard.attach_channel(manager.clone(), device.clone(), qh.clone(), conn.clone());
        self.data_device = Some(device);
        log::info!("[wayland] clipboard: wl_data_device created");
    }

    /// Binds the seat's input objects so their serials can gate `set_selection`.
    fn bind_seat_input(
        &mut self,
        qh: &wl_client::QueueHandle<WaylandSessionState>,
        capabilities: wl_client::WEnum<wl_client::protocol::wl_seat::Capability>,
    ) {
        let wl_client::WEnum::Value(capabilities) = capabilities else {
            return;
        };
        let Some(seat) = self.seat.as_ref() else { return };
        if capabilities.contains(wl_client::protocol::wl_seat::Capability::Keyboard)
            && self.keyboard.is_none()
        {
            self.keyboard = Some(seat.get_keyboard(qh, ()));
        }
        if capabilities.contains(wl_client::protocol::wl_seat::Capability::Pointer)
            && self.pointer.is_none()
        {
            self.pointer = Some(seat.get_pointer(qh, ()));
        }
    }

    /// Creates the `zwp_text_input_v3` once the manager and seat are both known.
    fn ensure_text_input(
        &mut self,
        qh: &wl_client::QueueHandle<WaylandSessionState>,
        conn: &wl_client::Connection,
    ) {
        if self.text_input.is_some() {
            return;
        }
        let (Some(manager), Some(seat)) = (self.text_input_manager.as_ref(), self.seat.as_ref())
        else {
            return;
        };
        let text_input = super::ime::create_text_input(&self.ime, manager, seat, qh, conn);
        self.text_input = Some(text_input);
        log::info!("[wayland] IME: text-input-v3 object created");
    }
}

// ---------------------------------------------------------------------------
// EventLoop native pump integration
// ---------------------------------------------------------------------------

/// Create a native platform event pump for the Wayland backend.
///
/// Returns a closure suitable for `EventLoop::set_native_pump()`.
/// When called, it looks up the global `WaylandPlatform` singleton and
/// dispatches pending Wayland events through `dispatch_pending()`.
///
/// Returns `None` if the active platform is not Wayland.
#[cfg(all(feature = "wayland-native", not(alloc_frugal), unix_desktop))]
pub fn create_event_loop_pump() -> Option<Box<dyn Fn() + Send + Sync>> {
    let platform = crate::platform::runtime::get_platform();
    let wayland = platform.as_any().downcast_ref::<WaylandPlatform>()?;
    Some(Box::new(move || {
        wayland.dispatch_native_events();
    }))
}

// --- Registry global discovery ---

#[cfg(all(feature = "wayland-native", unix_desktop))]
impl wl_client::Dispatch<wl_client::protocol::wl_registry::WlRegistry, ()> for WaylandSessionState {
    fn event(
        state: &mut Self,
        registry: &wl_client::protocol::wl_registry::WlRegistry,
        event: wl_client::protocol::wl_registry::Event,
        _data: &(),
        _conn: &wl_client::Connection,
        _qh: &wl_client::QueueHandle<WaylandSessionState>,
    ) {
        use wl_client::protocol::wl_registry::Event;
        if let Event::Global { name, interface, version } = event {
            log::debug!(
                "[wayland] Registry global: interface='{}', version={}",
                interface,
                version
            );
            match interface.as_str() {
                "wl_compositor" => {
                    let comp = registry
                        .bind::<wl_client::protocol::wl_compositor::WlCompositor, _, _>(
                            name,
                            version.min(4),
                            _qh,
                            (),
                        );
                    state.compositor = Some(comp);
                    log::info!("[wayland] Bound wl_compositor (v{})", version);
                }
                "xdg_wm_base" => {
                    let xdg = registry
                        .bind::<wl_protocols::xdg::shell::client::xdg_wm_base::XdgWmBase, _, _>(
                            name,
                            version.min(6),
                            _qh,
                            (),
                        );
                    state.xdg_wm_base = Some(xdg);
                    log::info!("[wayland] Bound xdg_wm_base (v{})", version);
                }
                "wl_output" => {
                    let output = registry.bind::<wl_client::protocol::wl_output::WlOutput, _, _>(
                        name,
                        version.min(4),
                        _qh,
                        (),
                    );
                    state.wl_output = Some(output);
                    log::info!("[wayland] Bound wl_output (v{})", version);
                }
                "wl_seat" => {
                    let seat = registry.bind::<wl_client::protocol::wl_seat::WlSeat, _, _>(
                        name,
                        version.min(7),
                        _qh,
                        (),
                    );
                    state.seat = Some(seat);
                    state.ensure_data_device(_qh, _conn);
                    state.ensure_text_input(_qh, _conn);
                    log::info!("[wayland] Bound wl_seat (v{})", version);
                }
                "wl_shm" => {
                    let shm = registry.bind::<wl_client::protocol::wl_shm::WlShm, _, _>(
                        name,
                        version.min(1),
                        _qh,
                        (),
                    );
                    state.shm = Some(shm);
                    log::info!("[wayland] Bound wl_shm (v{})", version);
                }
                "wl_data_device_manager" => {
                    let manager = registry
                        .bind::<wl_client::protocol::wl_data_device_manager::WlDataDeviceManager, _, _>(
                            name,
                            version.min(3),
                            _qh,
                            (),
                        );
                    state.data_device_manager = Some(manager);
                    state.ensure_data_device(_qh, _conn);
                    log::info!("[wayland] Bound wl_data_device_manager (v{})", version);
                }
                "zwp_text_input_manager_v3" => {
                    let manager = registry.bind::<
                        wl_protocols::wp::text_input::zv3::client::zwp_text_input_manager_v3::ZwpTextInputManagerV3,
                        _,
                        _,
                    >(name, version.min(1), _qh, ());
                    state.text_input_manager = Some(manager);
                    state.ensure_text_input(_qh, _conn);
                    log::info!("[wayland] Bound zwp_text_input_manager_v3 (v{})", version);
                }
                _ => { /* Other globals need no special handling */ }
            }
        }
    }
}

// --- wl_compositor (no events) ---

#[cfg(all(feature = "wayland-native", unix_desktop))]
impl wl_client::Dispatch<wl_client::protocol::wl_compositor::WlCompositor, ()>
    for WaylandSessionState
{
    fn event(
        _state: &mut Self,
        _proxy: &wl_client::protocol::wl_compositor::WlCompositor,
        _event: wl_client::protocol::wl_compositor::Event,
        _data: &(),
        _conn: &wl_client::Connection,
        _qh: &wl_client::QueueHandle<WaylandSessionState>,
    ) {
        // wl_compositor has no server-to-client events.
    }
}

// --- wl_output (handle scale events for DPI detection) ---

#[cfg(all(feature = "wayland-native", unix_desktop))]
impl wl_client::Dispatch<wl_client::protocol::wl_output::WlOutput, ()> for WaylandSessionState {
    fn event(
        state: &mut Self,
        _proxy: &wl_client::protocol::wl_output::WlOutput,
        event: wl_client::protocol::wl_output::Event,
        _data: &(),
        _conn: &wl_client::Connection,
        _qh: &wl_client::QueueHandle<WaylandSessionState>,
    ) {
        use wl_client::protocol::wl_output::Event;
        match event {
            Event::Scale { factor } => {
                state.dpi_scale = factor as f32;
                state.output_generation = state.output_generation.wrapping_add(1);
                log::info!("[wayland] wl_output scale factor: {} (DPI: {})", factor, factor * 96);
            }
            Event::Geometry { x, y, .. } => {
                state.output_x = x;
                state.output_y = y;
                state.output_generation = state.output_generation.wrapping_add(1);
            }
            Event::Mode { flags, width, height, refresh } => {
                if matches!(
                    flags,
                    wayland_client::WEnum::Value(
                        wayland_client::protocol::wl_output::Mode::Current
                    )
                ) {
                    state.output_width = width.max(0) as u32;
                    state.output_height = height.max(0) as u32;
                    state.output_refresh_mhz = (refresh > 0).then_some(refresh as u32);
                    state.output_ready = true;
                    state.output_generation = state.output_generation.wrapping_add(1);
                }
            }
            Event::Done => {
                log::trace!("[wayland] wl_output done");
            }
            Event::Name { .. } | Event::Description { .. } => {
                log::trace!("[wayland] wl_output event: {:?}", event);
            }
            _ => {
                log::trace!("[wayland] wl_output unhandled event: {:?}", event);
            }
        }
    }
}

// --- wl_shm / wl_shm_pool / wl_buffer (placeholder mapping) ---

#[cfg(all(feature = "wayland-native", unix_desktop))]
impl wl_client::Dispatch<wl_client::protocol::wl_shm::WlShm, ()> for WaylandSessionState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_client::protocol::wl_shm::WlShm,
        _event: wl_client::protocol::wl_shm::Event,
        _data: &(),
        _conn: &wl_client::Connection,
        _qh: &wl_client::QueueHandle<WaylandSessionState>,
    ) {
        log::trace!("[wayland] wl_shm event: {_event:?}");
    }
}

#[cfg(all(feature = "wayland-native", unix_desktop))]
impl wl_client::Dispatch<wl_client::protocol::wl_shm_pool::WlShmPool, ()> for WaylandSessionState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_client::protocol::wl_shm_pool::WlShmPool,
        _event: wl_client::protocol::wl_shm_pool::Event,
        _data: &(),
        _conn: &wl_client::Connection,
        _qh: &wl_client::QueueHandle<WaylandSessionState>,
    ) {
    }
}

#[cfg(all(feature = "wayland-native", unix_desktop))]
impl wl_client::Dispatch<wl_client::protocol::wl_buffer::WlBuffer, ()> for WaylandSessionState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_client::protocol::wl_buffer::WlBuffer,
        event: wl_client::protocol::wl_buffer::Event,
        _data: &(),
        _conn: &wl_client::Connection,
        _qh: &wl_client::QueueHandle<WaylandSessionState>,
    ) {
        log::trace!("[wayland] wl_buffer event: {event:?}");
    }
}

// --- wl_surface (surface enter/leave events) ---

#[cfg(all(feature = "wayland-native", unix_desktop))]
impl wl_client::Dispatch<wl_client::protocol::wl_surface::WlSurface, ()> for WaylandSessionState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_client::protocol::wl_surface::WlSurface,
        _event: wl_client::protocol::wl_surface::Event,
        _data: &(),
        _conn: &wl_client::Connection,
        _qh: &wl_client::QueueHandle<WaylandSessionState>,
    ) {
        // wl_surface events (enter/leave/frame/etc.) are logged but not acted upon:
        // the compositor side of a surface needs no bookkeeping here.
        log::trace!("[wayland] wl_surface event: {:?}", _event);
    }
}

// --- wl_seat (bind keyboard/pointer for serials) ---

#[cfg(all(feature = "wayland-native", unix_desktop))]
impl wl_client::Dispatch<wl_client::protocol::wl_seat::WlSeat, ()> for WaylandSessionState {
    fn event(
        state: &mut Self,
        _proxy: &wl_client::protocol::wl_seat::WlSeat,
        event: wl_client::protocol::wl_seat::Event,
        _data: &(),
        _conn: &wl_client::Connection,
        qh: &wl_client::QueueHandle<WaylandSessionState>,
    ) {
        use wl_client::protocol::wl_seat::Event;
        match event {
            Event::Capabilities { capabilities } => state.bind_seat_input(qh, capabilities),
            Event::Name { name } => log::debug!("[wayland] seat name: {name}"),
            _ => log::trace!("[wayland] wl_seat event: {:?}", event),
        }
    }
}

// --- wl_keyboard (serial only; the host owns input routing) ---

#[cfg(all(feature = "wayland-native", unix_desktop))]
impl wl_client::Dispatch<wl_client::protocol::wl_keyboard::WlKeyboard, ()> for WaylandSessionState {
    fn event(
        state: &mut Self,
        _proxy: &wl_client::protocol::wl_keyboard::WlKeyboard,
        event: wl_client::protocol::wl_keyboard::Event,
        _data: &(),
        _conn: &wl_client::Connection,
        _qh: &wl_client::QueueHandle<WaylandSessionState>,
    ) {
        use wl_client::protocol::wl_keyboard::Event;
        match event {
            Event::Key { serial, .. } | Event::Enter { serial, .. } => {
                state.clipboard.note_serial(serial);
            }
            _ => log::trace!("[wayland] wl_keyboard event: {:?}", event),
        }
    }
}

// --- wl_pointer (serial only; the host owns input routing) ---

#[cfg(all(feature = "wayland-native", unix_desktop))]
impl wl_client::Dispatch<wl_client::protocol::wl_pointer::WlPointer, ()> for WaylandSessionState {
    fn event(
        state: &mut Self,
        _proxy: &wl_client::protocol::wl_pointer::WlPointer,
        event: wl_client::protocol::wl_pointer::Event,
        _data: &(),
        _conn: &wl_client::Connection,
        _qh: &wl_client::QueueHandle<WaylandSessionState>,
    ) {
        use wl_client::protocol::wl_pointer::Event;
        match event {
            Event::Enter { serial, .. } | Event::Button { serial, .. } => {
                state.clipboard.note_serial(serial);
            }
            _ => log::trace!("[wayland] wl_pointer event: {:?}", event),
        }
    }
}

// --- wl_data_device_manager (no events) ---

#[cfg(all(feature = "wayland-native", unix_desktop))]
impl wl_client::Dispatch<wl_client::protocol::wl_data_device_manager::WlDataDeviceManager, ()>
    for WaylandSessionState
{
    fn event(
        _state: &mut Self,
        _proxy: &wl_client::protocol::wl_data_device_manager::WlDataDeviceManager,
        _event: wl_client::protocol::wl_data_device_manager::Event,
        _data: &(),
        _conn: &wl_client::Connection,
        _qh: &wl_client::QueueHandle<WaylandSessionState>,
    ) {
        // `wl_data_device_manager` has no server-to-client events.
    }
}

// --- wl_data_device (selection offers) ---

#[cfg(all(feature = "wayland-native", unix_desktop))]
impl wl_client::Dispatch<wl_client::protocol::wl_data_device::WlDataDevice, ()>
    for WaylandSessionState
{
    fn event(
        state: &mut Self,
        _proxy: &wl_client::protocol::wl_data_device::WlDataDevice,
        event: wl_client::protocol::wl_data_device::Event,
        _data: &(),
        _conn: &wl_client::Connection,
        _qh: &wl_client::QueueHandle<WaylandSessionState>,
    ) {
        use wl_client::protocol::wl_data_device::Event;
        match event {
            Event::DataOffer { id } => state.clipboard.note_offer(id),
            Event::Selection { id } => state.clipboard.note_selection(id),
            // Drag-and-drop has its own host-supplied `DropEvent`; the pointer-focused offers are
            // not selection changes and must not replace the clipboard.
            Event::Enter { .. } | Event::Leave | Event::Motion { .. } | Event::Drop => {
                log::trace!("[wayland] wl_data_device dnd event: {:?}", event)
            }
            _ => log::trace!("[wayland] wl_data_device event: {:?}", event),
        }
    }

    // A `selection`/`data_offer` event creates a child `wl_data_offer`. Without this override
    // the generated default panics ("Missing event_created_child specialization"), which is
    // exactly what a real compositor hit as soon as it delivered an external selection.
    wl_client::event_created_child!(WaylandSessionState, wl_client::protocol::wl_data_device::WlDataDevice, [
        0 => (wl_client::protocol::wl_data_offer::WlDataOffer, ()),
    ]);
}

// --- wl_data_offer (advertised MIME types) ---

#[cfg(all(feature = "wayland-native", unix_desktop))]
impl wl_client::Dispatch<wl_client::protocol::wl_data_offer::WlDataOffer, ()>
    for WaylandSessionState
{
    fn event(
        state: &mut Self,
        _proxy: &wl_client::protocol::wl_data_offer::WlDataOffer,
        event: wl_client::protocol::wl_data_offer::Event,
        _data: &(),
        _conn: &wl_client::Connection,
        _qh: &wl_client::QueueHandle<WaylandSessionState>,
    ) {
        use wl_client::protocol::wl_data_offer::Event;
        match event {
            Event::Offer { mime_type } => state.clipboard.note_mime(&mime_type),
            _ => log::trace!("[wayland] wl_data_offer event: {:?}", event),
        }
    }
}

// --- wl_data_source (serve the selection) ---

#[cfg(all(feature = "wayland-native", unix_desktop))]
impl wl_client::Dispatch<wl_client::protocol::wl_data_source::WlDataSource, ()>
    for WaylandSessionState
{
    fn event(
        state: &mut Self,
        _proxy: &wl_client::protocol::wl_data_source::WlDataSource,
        event: wl_client::protocol::wl_data_source::Event,
        _data: &(),
        _conn: &wl_client::Connection,
        _qh: &wl_client::QueueHandle<WaylandSessionState>,
    ) {
        use wl_client::protocol::wl_data_source::Event;
        match event {
            Event::Send { mime_type, fd } => state.clipboard.serve(&mime_type, fd),
            Event::Cancelled => {
                log::debug!("[wayland] clipboard: selection was replaced");
            }
            Event::Target { mime_type } => {
                log::trace!("[wayland] clipboard: target {mime_type:?}");
            }
            _ => log::trace!("[wayland] wl_data_source event: {:?}", event),
        }
    }
}

// --- zwp_text_input_manager_v3 (no events) ---

#[cfg(all(feature = "wayland-native", unix_desktop))]
impl
    wl_client::Dispatch<
        wl_protocols::wp::text_input::zv3::client::zwp_text_input_manager_v3::ZwpTextInputManagerV3,
        (),
    > for WaylandSessionState
{
    fn event(
        _state: &mut Self,
        _proxy: &wl_protocols::wp::text_input::zv3::client::zwp_text_input_manager_v3::ZwpTextInputManagerV3,
        _event: wl_protocols::wp::text_input::zv3::client::zwp_text_input_manager_v3::Event,
        _data: &(),
        _conn: &wl_client::Connection,
        _qh: &wl_client::QueueHandle<WaylandSessionState>,
    ) {
        // The manager has no server-to-client events.
    }
}

// --- zwp_text_input_v3 (composition and commit) ---

#[cfg(all(feature = "wayland-native", unix_desktop))]
impl
    wl_client::Dispatch<
        wl_protocols::wp::text_input::zv3::client::zwp_text_input_v3::ZwpTextInputV3,
        (),
    > for WaylandSessionState
{
    fn event(
        state: &mut Self,
        _proxy: &wl_protocols::wp::text_input::zv3::client::zwp_text_input_v3::ZwpTextInputV3,
        event: wl_protocols::wp::text_input::zv3::client::zwp_text_input_v3::Event,
        _data: &(),
        _conn: &wl_client::Connection,
        _qh: &wl_client::QueueHandle<WaylandSessionState>,
    ) {
        use wl_protocols::wp::text_input::zv3::client::zwp_text_input_v3::Event;
        match event {
            Event::Enter { .. } => state.ime.set_active(true),
            Event::Leave { .. } => state.ime.set_active(false),
            Event::PreeditString { text, cursor_begin, cursor_end } => {
                if let Some(text) = text {
                    state.ime.preedit(&text, cursor_begin, cursor_end);
                }
            }
            Event::CommitString { text } => {
                if let Some(text) = text {
                    state.ime.commit(&text);
                }
            }
            Event::DeleteSurroundingText { .. } | Event::Done { .. } => {
                log::trace!("[wayland] text-input event: {:?}", event);
            }
            _ => log::trace!("[wayland] text-input unhandled event: {:?}", event),
        }
    }
}

// --- wl_callback (frame presented) ---

#[cfg(all(feature = "wayland-native", unix_desktop))]
impl wl_client::Dispatch<wl_client::protocol::wl_callback::WlCallback, ()> for WaylandSessionState {
    fn event(
        state: &mut Self,
        _proxy: &wl_client::protocol::wl_callback::WlCallback,
        event: wl_client::protocol::wl_callback::Event,
        _data: &(),
        _conn: &wl_client::Connection,
        _qh: &wl_client::QueueHandle<WaylandSessionState>,
    ) {
        use wl_client::protocol::wl_callback::Event;
        if let Event::Done { callback_data } = event {
            state.frame_done_generation = state.frame_done_generation.wrapping_add(1);
            log::trace!("[wayland] frame presented (callback_data={callback_data})");
        }
    }
}

// --- xdg_wm_base (handle ping events) ---

#[cfg(all(feature = "wayland-native", unix_desktop))]
impl wl_client::Dispatch<wl_protocols::xdg::shell::client::xdg_wm_base::XdgWmBase, ()>
    for WaylandSessionState
{
    fn event(
        _state: &mut Self,
        proxy: &wl_protocols::xdg::shell::client::xdg_wm_base::XdgWmBase,
        event: <wl_protocols::xdg::shell::client::xdg_wm_base::XdgWmBase as wl_client::Proxy>::Event,
        _data: &(),
        _conn: &wl_client::Connection,
        _qh: &wl_client::QueueHandle<WaylandSessionState>,
    ) {
        use wl_protocols::xdg::shell::client::xdg_wm_base::Event;
        if let Event::Ping { serial } = event {
            proxy.pong(serial);
            log::trace!("[wayland] xdg_wm_base ping/pong (serial={})", serial);
        }
    }
}

// --- xdg_surface (acknowledge configure events) ---

#[cfg(all(feature = "wayland-native", unix_desktop))]
impl wl_client::Dispatch<wl_protocols::xdg::shell::client::xdg_surface::XdgSurface, ()>
    for WaylandSessionState
{
    fn event(
        _state: &mut Self,
        proxy: &wl_protocols::xdg::shell::client::xdg_surface::XdgSurface,
        event: <wl_protocols::xdg::shell::client::xdg_surface::XdgSurface as wl_client::Proxy>::Event,
        _data: &(),
        _conn: &wl_client::Connection,
        _qh: &wl_client::QueueHandle<WaylandSessionState>,
    ) {
        use wl_protocols::xdg::shell::client::xdg_surface::Event;
        if let Event::Configure { serial } = event {
            proxy.ack_configure(serial);
            log::trace!("[wayland] xdg_surface configure ack (serial={})", serial);
        }
    }
}

// --- xdg_toplevel (handle configure / close) ---

#[cfg(all(feature = "wayland-native", unix_desktop))]
impl wl_client::Dispatch<wl_protocols::xdg::shell::client::xdg_toplevel::XdgToplevel, ()>
    for WaylandSessionState
{
    fn event(
        _state: &mut Self,
        proxy: &wl_protocols::xdg::shell::client::xdg_toplevel::XdgToplevel,
        event: <wl_protocols::xdg::shell::client::xdg_toplevel::XdgToplevel as wl_client::Proxy>::Event,
        _data: &(),
        _conn: &wl_client::Connection,
        _qh: &wl_client::QueueHandle<WaylandSessionState>,
    ) {
        use wl_protocols::xdg::shell::client::xdg_toplevel::Event;
        match event {
            Event::Configure { width, height, states } => {
                log::trace!(
                    "[wayland] xdg_toplevel configure: {}x{}, states={:?}",
                    width,
                    height,
                    states
                );
                // The compositor told us the new size — this is Wayland's equivalent of
                // a user dragging the window edge, so it must reach the layout. A zero
                // dimension means "you choose", not a real size, so it is not reported.
                //
                // `proxy` is the toplevel the compositor named, so the id is looked up
                // from *this* window rather than from a single remembered one.
                if width > 0 && height > 0 {
                    if let Some(id) = super::platform_impl::configured_window_id(proxy) {
                        crate::queue_resize_trigger(id, width as u32, height as u32);
                    }
                }
            }
            Event::Close => {
                // The compositor asked this window to close (the user clicked the title-bar close
                // button, or the compositor is shutting down). This must be **honoured**, not merely
                // logged: without a consumer, clicking close left the window on screen and the
                // library running, so the close was a dead end from the user's point of view.
                //
                // The library's one close path is `crate::close_widget`, which emits the widget's
                // `closed` signal so every `on_close`/`events:{"closed":…}` handler fires, exactly
                // as the Win32 `WM_CLOSE` arm does. The id is resolved from *this* toplevel's proxy
                // rather than from a single remembered window, so a multi-window app closes the
                // window the compositor named.
                if let Some(window_id) = super::platform_impl::configured_window_id(proxy) {
                    if !crate::close_widget(window_id) {
                        log::error!(
                            "[wayland] close requested for window widget {window_id}, which is not \
                             a live widget; no close callback was run"
                        );
                    }
                } else {
                    // An unregistered toplevel: the compositor named a window this backend does not
                    // track. Reported rather than ignored, because a close that runs nothing is the
                    // silent failure this arm exists to remove.
                    log::warn!(
                        "[wayland] close requested for an unregistered xdg_toplevel; no window \
                         callback was run"
                    );
                }
            }
            Event::ConfigureBounds { .. } | Event::WmCapabilities { .. } => {
                log::trace!("[wayland] xdg_toplevel event: {:?}", event);
            }
            _ => {
                log::trace!("[wayland] xdg_toplevel unhandled event: {:?}", event);
            }
        }
    }
}
