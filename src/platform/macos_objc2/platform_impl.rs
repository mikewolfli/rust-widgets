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
use crate::compat::String;
use crate::core::ObjectId;
use crate::core::PlatformFamily;
use crate::platform::{Platform, PlatformCapabilities};
use core::time::Duration;
use std::thread;

/// The frame interval this backend's loop runs at, in milliseconds.
///
/// The same value as every other backend's twin constant, and named here for the same
/// reason: the sleep between iterations and the delta handed to [`crate::drive_frame`]
/// must be the same number, or every transition runs at the ratio between them with
/// nothing to notice the mismatch. A backend that later runs at the display's own rate
/// changes exactly this value.
const FRAME_INTERVAL_MS: u64 = 16;

impl Platform for MacOSObjc2Platform {
    // The uniform widget-property methods are answered once, over `self.state`, by the
    // shared expansion in `platform::state_impl` rather than re-written per backend.
    crate::impl_platform_state_properties!();

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

    /// The host integrations this backend actually provides.
    ///
    /// # Why all four are `false`, and not the four the legacy backend claims
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
    /// * `ime` — `ime_bridge()` is not overridden, so it inherits the `None` default.
    /// * `accessibility` — `accessibility_bridge()` is likewise not overridden.
    /// * `dpi_scaling` — `dpi_scale_factor()` is not overridden, so it answers the
    ///   trait default `1.0`. A backend that reports DPI awareness must be able to
    ///   *ask* the display; there is no display behind this type.
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
            dpi_scaling: false,
            ime: false,
            accessibility: false,
            native_menu: false,
            typed_widget_trigger: true,
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
            log::debug!(
                "[macos-objc2] NSApplication not bootstrapped (off-main or non-macOS host)"
            );
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
        while self.runtime.running.load(Ordering::SeqCst) {
            // Pump the real AppKit queue so the native window draws, receives mouse/key
            // input, resizes and closes — without this the loop ran only library frames and
            // the window was inert. The frame and a short sleep fill the rest of the tick.
            #[cfg(all(target_os = "macos", feature = "macos"))]
            {
                super::native::pump_native_event();
            }
            crate::drive_frame(FRAME_INTERVAL_MS as u32);
            thread::sleep(Duration::from_millis(FRAME_INTERVAL_MS));
        }
    }
    fn quit(&self) {
        self.runtime.running.store(false, Ordering::SeqCst);
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
        let existed = self.state.destroy_widget(widget_id);

        // Release the retained AppKit objects. A window must be closed (and taken
        // off screen) rather than merely detached: `removeFromSuperview` on an
        // `NSWindow` does nothing useful, so a logically destroyed window could
        // stay visible. `destroy_native_handle` distinguishes the two by the
        // object's own runtime class, so it does not need the pre-destroy kind, and
        // runs only on the main thread — off it, it logs and skips.
        #[cfg(all(target_os = "macos", feature = "macos"))]
        {
            super::native::destroy_native_handle(widget_id);
        }

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
    fn set_clipboard_text(&self, text: &str) -> bool {
        self.state.set_clipboard_text(text)
    }
    fn get_clipboard_text(&self) -> String {
        self.state.clipboard_text()
    }
}
