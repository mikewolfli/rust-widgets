// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `Platform` implementation for the HarmonyOS / OpenHarmony backend.
//!
//! The backend is state-driven: widget creation, geometry, text, visibility, menus,
//! clipboard, drag/drop and IME metadata all live in
//! [`crate::platform::state::BackendState`], and library-painted widgets are
//! displayed through the surface methods (see `super::status.md`).
//!
//! The one fact worth knowing before editing anything here: OpenHarmony targets
//! report `target_env = "ohos"` and `target_os = "linux"`, so backend selection keys
//! off `target_env` — see [`crate::platform::profile::is_openharmony_target`].

use super::super::{DropEvent, Platform};
use super::types::*;
use crate::core::PlatformFamily;
use crate::{WidgetTriggerEvent, WidgetTriggerKind};

use std::sync::atomic::Ordering;
#[cfg(not(target_arch = "wasm32"))]
use std::thread;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;

impl Platform for HarmonyPlatform {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn backend_name(&self) -> &'static str {
        "harmony-desktop"
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

    /// Estimates CPU load as thread count over twice the available cores.
    fn process_cpu_utilization(&self) -> Option<f32> {
        crate::platform::os_probes::process_cpu_utilization()
    }

    /// HarmonyOS printing is served by the ArkUI print service, which this state
    /// backend does not bind; it reports the gap instead of faking success.
    fn spawn_print_job(&self, job_file: &std::path::Path) -> Result<(), String> {
        Err(format!(
            "HarmonyOS printing requires the ArkUI print service, which is not bound in this \
             build; job file '{}' was not printed",
            job_file.display()
        ))
    }

    /// Whether ArkUI exposes a print spooler to this backend.
    ///
    /// `PrintDialog` asks this before offering the action, so a host without the
    /// service gets a truthful `false` instead of a dialog that discards the
    /// document.
    fn has_print_support(&self) -> bool {
        false
    }

    /// Capabilities published by the Harmony backend.
    ///
    /// The backend is state-only: widget state, layout and event plumbing are all
    /// modelled in-process against `BackendState`, so the flags describe the
    /// in-process contract. Every `WidgetKind` is now painted by `src/widget/`,
    /// so there is no native-control capability left to advertise.
    fn capabilities(&self) -> crate::platform::types::PlatformCapabilities {
        crate::platform::types::PlatformCapabilities {
            dpi_scaling: true,
            ime: true,
            accessibility: true,
            native_menu: false,
            typed_widget_trigger: true,
        }
    }
    fn init(&self) {
        self.runtime.initialized.store(true, Ordering::SeqCst);
    }
    fn run(&self) {
        if !self.runtime.initialized.load(Ordering::SeqCst) {
            self.init();
        }
        self.runtime.running.store(true, Ordering::SeqCst);
        while self.runtime.running.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(16));
        }
    }
    fn quit(&self) {
        self.runtime.running.store(false, Ordering::SeqCst);
    }

    /// Release the state record for `widget_id`.
    ///
    /// Harmony keeps no per-widget side table now that the native control
    /// constructors are gone: the shared list storage and the menu bookkeeping
    /// that used to need purging here no longer exist, so dropping the
    /// authoritative `BackendState` record is the whole teardown.
    fn destroy_widget(&self, widget_id: u64) -> bool {
        self.state.destroy_widget(widget_id)
    }
    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        self.insert_widget(HarmonyHandleKind::Window, title, x, y, width, height)
    }
    fn show_widget(&self, widget_id: u64) {
        self.state.set_visible(widget_id, true);
    }
    fn hide_widget(&self, widget_id: u64) {
        self.state.set_visible(widget_id, false);
    }
    fn set_widget_geometry(&self, widget_id: u64, x: i32, y: i32, width: u32, height: u32) {
        self.state.set_geometry(widget_id, x, y, width, height);
    }
    fn set_widget_text(&self, widget_id: u64, text: &str) {
        self.state.set_text(widget_id, text);
    }
    fn get_widget_text(&self, widget_id: u64) -> String {
        self.state.text(widget_id)
    }
    fn set_widget_enabled(&self, widget_id: u64, enabled: bool) {
        self.state.set_enabled(widget_id, enabled);
    }
    fn is_widget_enabled(&self, widget_id: u64) -> bool {
        self.state.enabled(widget_id)
    }
    fn set_widget_visible(&self, widget_id: u64, visible: bool) {
        self.state.set_visible(widget_id, visible);
    }
    fn is_widget_visible(&self, widget_id: u64) -> bool {
        self.state.visible(widget_id)
    }
    fn set_widget_ime_enabled(&self, widget_id: u64, enabled: bool) -> bool {
        self.state.set_ime_enabled(widget_id, enabled)
    }
    fn is_widget_ime_enabled(&self, widget_id: u64) -> bool {
        self.state.ime_enabled(widget_id)
    }
    fn set_clipboard_text(&self, text: &str) -> bool {
        self.state.set_clipboard_text(text)
    }
    fn get_clipboard_text(&self) -> String {
        self.state.clipboard_text()
    }
    fn begin_drag(&self, source_widget_id: u64, mime: &str, payload: &[u8]) -> bool {
        self.state.begin_drag(source_widget_id, mime, payload)
    }
    fn poll_drop_event(&self) -> Option<DropEvent> {
        self.state.pop_drop_event()
    }
    fn inject_drop_event(&self, event: DropEvent) -> bool {
        self.state.inject_drop_event(event)
    }

    /// Pops the next typed widget-trigger event injected into this backend.
    ///
    /// The queue lives in `BackendState` and is shared by every state-only
    /// backend (iOS/Android/mobile/stub). Harmony delegates to it like the rest;
    /// leaving this to the trait default made the pair below silently
    /// asymmetric — `inject_widget_trigger_event` returned `false` and this
    /// always returned `None`, so no injected trigger could ever be observed.
    fn poll_widget_trigger_event(&self) -> Option<WidgetTriggerEvent> {
        self.state.pop_widget_trigger_event()
    }

    /// Injects a typed widget-trigger event, refusing ids the backend never made.
    fn inject_widget_trigger_event(&self, widget_id: u64, kind: WidgetTriggerKind) -> bool {
        self.state.inject_widget_trigger_event(widget_id, kind)
    }

    /// Mounts a library-painted widget onto a surface this host will present.
    ///
    /// The backend keeps no native object per control (every `WidgetKind` is painted
    /// by `src/widget/`), so the surface is a record plus a repaint queue: the ArkTS
    /// side owns the pixels and pulls them with the render API, and this tells it
    /// which widgets exist and when they went stale.
    ///
    /// Before this the backend inherited the trait default and reported `false`, so a
    /// host that asked could not display anything even though nothing was missing but
    /// the wiring.
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
    /// # Who calls this on Harmony
    ///
    /// The **host ArkTS page**, not this backend. OpenHarmony delivers a size change to
    /// the host's `onAreaChange` callback on the component it created the surface in; the
    /// library holds no ArkUI component handle to subscribe with (this backend is a state
    /// model — see `create_window`), so there is no callback for it to attach.
    ///
    /// A host reports the new size here (or through [`crate::queue_resize_trigger`], the
    /// same call). One that does not keeps the created size, which is what
    /// `window_client_size` falls back to.
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

    /// The backend displays library-painted widgets by handing the host their frames.
    fn supports_surfaces(&self) -> bool {
        true
    }
}
