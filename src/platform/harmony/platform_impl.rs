// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use super::super::{DropEvent, Platform};
use super::types::*;
use crate::core::PlatformFamily;

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

    /// Reads `MemTotal` from `/proc/meminfo`; HarmonyOS runs on a Linux-derived
    /// kernel that provides it.
    fn total_memory_mb(&self) -> Option<u64> {
        let content = std::fs::read_to_string("/proc/meminfo").ok()?;
        for line in content.lines() {
            let Some(rest) = line.strip_prefix("MemTotal:") else {
                continue;
            };
            let kb = rest.trim().trim_end_matches("kB").trim().parse::<u64>().ok()?;
            return Some(kb / 1024);
        }
        None
    }

    /// Walks `/sys/class/power_supply` for a discharging battery.
    fn is_on_battery(&self) -> bool {
        let Ok(entries) = std::fs::read_dir("/sys/class/power_supply") else {
            return false;
        };
        for entry in entries.flatten() {
            let status_path = entry.path().join("status");
            if let Ok(status) = std::fs::read_to_string(&status_path) {
                if status.trim() == "Discharging" {
                    return true;
                }
            }
        }
        false
    }

    /// Samples RSS/VmSize for this process from `/proc/self/status`.
    fn process_memory_utilization(&self) -> Option<f32> {
        let status = std::fs::read_to_string("/proc/self/status").ok()?;
        let mut vmrss_kb: u64 = 0;
        let mut vmsize_kb: u64 = 0;
        for line in status.lines() {
            if let Some(rest) = line.strip_prefix("VmRSS:") {
                vmrss_kb = rest.trim().trim_end_matches("kB").trim().parse().unwrap_or(0);
            } else if let Some(rest) = line.strip_prefix("VmSize:") {
                vmsize_kb = rest.trim().trim_end_matches("kB").trim().parse().unwrap_or(0);
            }
        }
        if vmsize_kb == 0 {
            return None;
        }
        Some((vmrss_kb as f32 / vmsize_kb as f32).clamp(0.0, 1.0))
    }

    /// Estimates CPU load as thread count over twice the available cores.
    fn process_cpu_utilization(&self) -> Option<f32> {
        let status = std::fs::read_to_string("/proc/self/status").ok()?;
        for line in status.lines() {
            let Some(rest) = line.strip_prefix("Threads:") else {
                continue;
            };
            let threads = rest.trim().parse::<f32>().ok()?;
            let cores = std::thread::available_parallelism().map(|n| n.get() as f32).unwrap_or(4.0);
            return Some((threads / (cores * 2.0)).clamp(0.0, 1.0));
        }
        None
    }

    /// HarmonyOS printing is served by the ArkUI print service, which this state
    /// backend does not bind; it reports the gap instead of faking success.
    fn spawn_print_job(&self, _job_file: &std::path::Path) -> Result<(), String> {
        Err("HarmonyOS printing requires the ArkUI print service (not bound)".to_string())
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
    /// The Harmony backend has no native-control surface any more, so this reports
    /// `false` until an ArkUI Canvas bridge is bound.
    fn supports_custom_widgets(&self) -> bool {
        false
    }
}
