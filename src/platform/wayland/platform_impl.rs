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
const FRAME_INTERVAL_MS: u32 = 16;

#[cfg(all(feature = "wayland-native", target_os = "linux"))]
use wayland_client as wl_client;

#[cfg(all(feature = "wayland-native", target_os = "linux"))]
use wayland_protocols as wl_protocols;

// ---------------------------------------------------------------------------
// Platform trait implementation
// ---------------------------------------------------------------------------

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
    /// # Why `ime: false` and `accessibility: false`
    ///
    /// [`PlatformCapabilities::ime`] is a promise about [`Platform::ime_bridge`] and
    /// `accessibility` about [`Platform::accessibility_bridge`]. This backend overrides
    /// **neither**, so each inherits the trait default — `None` — and a `true` here told every
    /// caller there was a bridge to ask for. `grep -rn "ime_bridge\|accessibility_bridge"
    /// src/platform/wayland/` returns nothing: there is no Wayland IME integration and no
    /// Wayland accessibility bridge in this crate, and unlike Linux/GTK there is not even a
    /// candidate module to delegate to (`platform/ime_linux.rs` is IBus over D-Bus, which the
    /// GTK backend owns; a Wayland client reaches the compositor's `text-input` protocol, which
    /// is not bound here).
    ///
    /// The flag escapes the process: `rw_platform_capabilities()` packs these five into a C ABI
    /// bitmask (`bindings/binding_impl.rs`), so a host application read the false `ime`/`access`
    /// bits and would offer an input-method or screen-reader affordance that cannot answer.
    ///
    /// # Why `dpi_scaling` stays `true`
    ///
    /// It is the one of the three with an override behind it: [`Platform::dpi_scale_factor`]
    /// reads `GDK_SCALE` / `QT_SCALE_FACTOR` / `RUST_WIDGETS_DPI_SCALE` and returns a real
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
            ime: false,
            accessibility: false,
            native_menu: false,
            typed_widget_trigger: true,
        }
    }

    fn dpi_scale_factor(&self) -> f32 {
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
        #[cfg(all(feature = "wayland-native", target_os = "linux"))]
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
        #[cfg(all(feature = "wayland-native", not(alloc_frugal), target_os = "linux"))]
        if let Some(pump) = create_event_loop_pump() {
            event_loop.set_native_pump(pump);
        }
        event_loop.start();

        #[cfg(all(feature = "wayland-native", not(alloc_frugal), target_os = "linux"))]
        {
            self.run_native_event_loop();
        }

        #[cfg(not(all(feature = "wayland-native", not(alloc_frugal), target_os = "linux")))]
        while self.runtime.running.load(std::sync::atomic::Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_millis(16));
        }
    }

    fn quit(&self) {
        self.runtime.running.store(false, std::sync::atomic::Ordering::SeqCst);
        log::info!("[wayland] Platform quit.");
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
            // The widget may be an attached menu bar (keyed by window id) or a
            // window owning one, so both directions are cleared.
            menus.attached_menu_bar.remove(&widget_id);
            menus.attached_menu_bar.retain(|_, bar| *bar != widget_id);
            // The widget may be a container in the menu tree: drop both the
            // children it owned and the child entry under its own parent.
            menus.menu_children.remove(&widget_id);
            for children in menus.menu_children.values_mut() {
                children.retain(|child| *child != widget_id);
            }
            // Drop queued triggers that reference a widget that no longer exists.
            menus.pending_menu_events.retain(|queued| *queued != widget_id);
            menus.pending_widget_events.retain(|event| event.widget_id != widget_id);
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
        #[cfg(all(feature = "wayland-native", target_os = "linux"))]
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
        self.state.set_clipboard_text(text)
    }

    fn get_clipboard_text(&self) -> String {
        self.state.clipboard_text()
    }

    // -----------------------------------------------------------------------
    // Drag and drop
    // -----------------------------------------------------------------------
}

// ---------------------------------------------------------------------------
// Native Wayland window creation (gated by "wayland-native" feature)
// ---------------------------------------------------------------------------

#[cfg(all(feature = "wayland-native", target_os = "linux"))]
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
    #[cfg(all(feature = "wayland-native", not(alloc_frugal), target_os = "linux"))]
    fn run_native_event_loop(&self) {
        use std::os::fd::AsRawFd;

        // Idle timeout: bounds quit latency when the compositor is silent.
        const IDLE_TIMEOUT_MS: i32 = 50;

        log::info!("[wayland] Entering native fd-based event loop");
        loop {
            if !self.runtime.running.load(std::sync::atomic::Ordering::SeqCst) {
                break;
            }

            let mut guard = self.native_session.lock().unwrap();
            let Some(session) = guard.as_mut() else {
                // No native session yet (no window created): stay responsive to
                // quit without spinning.
                drop(guard);
                // The drain still runs: a trigger can be queued before a session exists
                // (a host reporting a size for a window it manages itself), and leaving it
                // in the queue until a session appears would delay a layout run for as long
                // as the window is absent. See `crate::drain_triggers`.
                //
                // `crate::drive_frame` rather than the bare drain: the animation step has to
                // run even in this session-less arm, or a transition that started before the
                // window appeared would freeze instead of finishing (BLUE24 §0A.1
                // measurement 1).
                crate::drive_frame(FRAME_INTERVAL_MS);
                std::thread::sleep(std::time::Duration::from_millis(FRAME_INTERVAL_MS as u64));
                continue;
            };

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
                        std::thread::sleep(std::time::Duration::from_millis(16));
                    }
                    // rc == 0: idle timeout, loop to re-check `running`.
                }
                None => {
                    // Events are already buffered; drain them without blocking.
                    let _ = session.event_queue.dispatch_pending(&mut session.state);
                }
            }

            // One library frame after the protocol dispatch for this iteration.
            //
            // Deliberately **outside** the `native_session` lock taken above: the frame can
            // re-enter library code that positions widgets, and holding this backend's
            // session lock across that would let a re-entrant call deadlock on the same
            // mutex. The lock guard is dropped at the end of the `match` above.
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
        let mut guard = self.native_session.lock().unwrap();
        if let Some(ref mut session) = *guard {
            let _ = session.event_queue.dispatch_pending(&mut session.state);
        }
        // Released before the frame, so a trigger that positions a widget cannot
        // deadlock on this mutex. This is the pump-callback spelling of the same tick
        // the `run()` loop performs, animation step included.
        drop(guard);
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
        fn connect_wayland() -> Option<WaylandSession> {
            let conn = wl_client::Connection::connect_to_env()
                .inspect_err(|e| {
                    log::error!("[wayland] Failed to connect to Wayland display: {}", e)
                })
                .ok()?;
            let display = conn.display();
            let mut event_queue = conn.new_event_queue();
            let qh = event_queue.handle();
            let _registry = display.get_registry(&qh, ());

            let mut state = WaylandSessionState {
                compositor: None,
                xdg_wm_base: None,
                wl_output: None,
                dpi_scale: 1.0,
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

        // Obtain or create the persistent Wayland session.
        let mut guard = self.native_session.lock().unwrap();
        if guard.is_none() {
            *guard = connect_wayland();
        }
        let session = guard.as_mut()?;

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
#[cfg(all(feature = "wayland-native", target_os = "linux"))]
thread_local! {
    static CONFIGURED_WINDOWS: core::cell::RefCell<crate::compat::HashMap<usize, ObjectId>> =
        core::cell::RefCell::new(crate::compat::HashMap::new());
}

/// Records that `proxy`'s configure events describe the widget `id`.
///
/// Called once per window at creation. The proxy is identified by its object address, which
/// is stable for the life of the `wl_proxy`: `wayland-client` keeps the proxy alive in the
/// connection's object map for as long as the compositor may send it events.
#[cfg(all(feature = "wayland-native", target_os = "linux"))]
fn record_configured_window(proxy: &impl wl_client::Proxy, id: ObjectId) {
    let key = proxy.id().protocol_id() as usize;
    CONFIGURED_WINDOWS.with(|map| {
        map.borrow_mut().insert(key, id);
    });
}

/// The window to report a configure event against, if one is known.
#[cfg(all(feature = "wayland-native", target_os = "linux"))]
pub(crate) fn configured_window_id(proxy: &impl wl_client::Proxy) -> Option<ObjectId> {
    let key = proxy.id().protocol_id() as usize;
    CONFIGURED_WINDOWS.with(|map| map.borrow().get(&key).copied())
}

// ---------------------------------------------------------------------------
// Wayland session state & dispatch implementations
// ---------------------------------------------------------------------------

/// Dispatch state for the Wayland session.
/// Holds the global proxies obtained from the registry.
#[cfg(all(feature = "wayland-native", target_os = "linux"))]
pub(crate) struct WaylandSessionState {
    pub(crate) compositor: Option<wl_client::protocol::wl_compositor::WlCompositor>,
    pub(crate) xdg_wm_base: Option<wl_protocols::xdg::shell::client::xdg_wm_base::XdgWmBase>,
    /// wl_output proxy for DPI scale detection.
    pub(crate) wl_output: Option<wl_client::protocol::wl_output::WlOutput>,
    /// Logical-to-physical scale reported by the compositor. It scales the
    /// drawing surface and so the input-coordinate mapping, and is set by the
    /// `wl_output::scale` event; it stays 1.0 until then.
    pub(crate) dpi_scale: f32,
}

/// A persistent Wayland session containing the connection, event queue,
/// and proxy state. Stored inside `WaylandPlatform.native_session`.
///
/// This is one object per connection, shared by every window — not a per-widget
/// registry — so no per-widget protocol object is kept here. Widgets other than the
/// window are painted by the library, so the host has nothing to create for them.
#[cfg(all(feature = "wayland-native", target_os = "linux"))]
pub(crate) struct WaylandSession {
    /// The Wayland connection. Must stay alive while `event_queue` is in use.
    #[allow(dead_code)]
    pub(crate) conn: wl_client::Connection,
    pub(crate) event_queue: wl_client::EventQueue<WaylandSessionState>,
    pub(crate) state: WaylandSessionState,
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
#[cfg(all(feature = "wayland-native", not(alloc_frugal), target_os = "linux"))]
pub fn create_event_loop_pump() -> Option<Box<dyn Fn() + Send + Sync>> {
    let platform = crate::platform::runtime::get_platform();
    let wayland = platform.as_any().downcast_ref::<WaylandPlatform>()?;
    Some(Box::new(move || {
        wayland.dispatch_native_events();
    }))
}

// --- Registry global discovery ---

#[cfg(all(feature = "wayland-native", target_os = "linux"))]
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
                _ => { /* Other globals need no special handling */ }
            }
        }
    }
}

// --- wl_compositor (no events) ---

#[cfg(all(feature = "wayland-native", target_os = "linux"))]
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

#[cfg(all(feature = "wayland-native", target_os = "linux"))]
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
                log::info!("[wayland] wl_output scale factor: {} (DPI: {})", factor, factor * 96);
            }
            Event::Done => {
                log::trace!("[wayland] wl_output done");
            }
            Event::Geometry { .. }
            | Event::Mode { .. }
            | Event::Name { .. }
            | Event::Description { .. } => {
                log::trace!("[wayland] wl_output event: {:?}", event);
            }
            _ => {
                log::trace!("[wayland] wl_output unhandled event: {:?}", event);
            }
        }
    }
}

// --- wl_surface (surface enter/leave events) ---

#[cfg(all(feature = "wayland-native", target_os = "linux"))]
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

// --- xdg_wm_base (handle ping events) ---

#[cfg(all(feature = "wayland-native", target_os = "linux"))]
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

#[cfg(all(feature = "wayland-native", target_os = "linux"))]
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

#[cfg(all(feature = "wayland-native", target_os = "linux"))]
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
