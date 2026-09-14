// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

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

    /// Reads installed physical memory through `sysctl hw.memsize`, the
    /// documented macOS interface for the machine's total RAM.
    fn total_memory_mb(&self) -> Option<u64> {
        let output =
            std::process::Command::new("sysctl").args(["-n", "hw.memsize"]).output().ok()?;
        if !output.status.success() {
            return None;
        }
        let bytes = String::from_utf8_lossy(&output.stdout).trim().parse::<u64>().ok()?;
        Some(bytes / (1024 * 1024))
    }

    /// Reports `true` when the machine is discharging its battery.
    ///
    /// `pmset -g batt` prints `Now drawing from 'Battery Power'` while on
    /// battery and `'AC Power'` while plugged in; desktops always report AC.
    fn is_on_battery(&self) -> bool {
        let Ok(output) = std::process::Command::new("pmset").args(["-g", "batt"]).output() else {
            return false;
        };
        output.status.success() && String::from_utf8_lossy(&output.stdout).contains("Battery Power")
    }

    /// Samples this process's resident memory against total installed RAM.
    fn process_memory_utilization(&self) -> Option<f32> {
        let pid = std::process::id().to_string();
        let output =
            std::process::Command::new("ps").args(["-o", "rss=", "-p", &pid]).output().ok()?;
        if !output.status.success() {
            return None;
        }
        let rss_kb = String::from_utf8_lossy(&output.stdout).trim().parse::<f64>().ok()?;
        // Use the real machine total rather than assuming a "typical" 8 GB, so
        // the ratio stays meaningful on 16/32/64 GB systems.
        let total_kb = self.total_memory_mb()? as f64 * 1024.0;
        if total_kb <= 0.0 {
            return None;
        }
        Some(((rss_kb / total_kb) as f32).clamp(0.0, 1.0))
    }

    /// CPU load is not sampled through a stable public interface here, so this
    /// backend reports `None` and the monitor keeps its default.
    fn process_cpu_utilization(&self) -> Option<f32> {
        None
    }

    /// Submits through the CUPS `lpr` client, falling back to `lp`.
    fn spawn_print_job(&self, job_file: &std::path::Path) -> Result<(), String> {
        if let Ok(status) = std::process::Command::new("lpr").arg(job_file).status() {
            if status.success() {
                return Ok(());
            }
        }
        if let Ok(status) = std::process::Command::new("lp").arg(job_file).status() {
            if status.success() {
                return Ok(());
            }
        }
        Err("no available system print command succeeded (tried: lpr, lp)".to_string())
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
