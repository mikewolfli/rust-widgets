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
use crate::platform::{DropEvent, Platform};
use core::time::Duration;
use std::thread;

impl Platform for MacOSObjc2Platform {
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
    /// window and a drawing surface. This method records the surface; the AppKit view
    /// that blits frames is created by `macos/canvas.rs` through the sibling cocoa
    /// backend's identical contract.
    ///
    /// Without it this backend inherited the trait's `false` default, so the preview
    /// backend reported "I cannot display a UI here" while the cocoa backend on the
    /// same machine reported `true`. Which answer a host got depended on a feature
    /// flag the host does not control, and a host that checks `supports_surfaces()`
    /// before building a UI would refuse to start for no stated reason.
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

    /// Queues a repaint for the host to pick up. `false` when `id` is not mounted.
    fn invalidate_surface(&self, id: ObjectId) -> bool {
        self.state.invalidate_surface_record(id)
    }

    /// This backend displays library-painted widgets by handing the host their frames.
    ///
    /// Must agree with the cocoa backend on the same machine — see
    /// [`Self::mount_surface`] for what went wrong when it did not.
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
        while self.runtime.running.load(Ordering::SeqCst) {
            crate::drain_triggers();
            thread::sleep(Duration::from_millis(16));
        }
    }
    fn quit(&self) {
        self.runtime.running.store(false, Ordering::SeqCst);
    }
    fn destroy_widget(&self, widget_id: ObjectId) -> bool {
        let existed = self.state.destroy_widget(widget_id);

        // Release the retained AppKit objects. `remove_native_view` releases the
        // object stored under the widget id and drops its parent-map entry.
        #[cfg(all(target_os = "macos", feature = "macos"))]
        {
            super::native::remove_native_view(widget_id);
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
            let window = super::native::create_ns_window(mtm, title, x, y, width, height);
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
    fn begin_drag(&self, source_widget_id: u64, mime: &str, payload: &[u8]) -> bool {
        self.state.begin_drag(source_widget_id, mime, payload)
    }
    fn poll_drop_event(&self) -> Option<DropEvent> {
        self.state.pop_drop_event()
    }
    fn inject_drop_event(&self, event: DropEvent) -> bool {
        self.state.inject_drop_event(event)
    }
}
