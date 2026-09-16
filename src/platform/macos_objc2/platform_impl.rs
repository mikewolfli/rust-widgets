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
use crate::core::ObjectId;
use crate::core::PlatformFamily;
use crate::platform::{DropEvent, Platform};
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;

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

    /// Submits the job to the unix print spooler via [`crate::platform::os_probes`].
    fn spawn_print_job(&self, job_file: &std::path::Path) -> Result<(), String> {
        crate::platform::os_probes::spawn_print_job(job_file)
    }

    /// macOS always ships CUPS, so the `lp`/`lpr` clients are present.
    fn has_print_support(&self) -> bool {
        crate::platform::types::unix_print_clients_available()
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
        while self.runtime.running.load(Ordering::SeqCst) {
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
        let mut menus = self.menus.lock().expect("mac objc2 menu lock poisoned");
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
