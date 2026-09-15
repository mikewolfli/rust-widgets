// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use super::types::{LinuxHandleKind, LinuxPlatform};
use crate::compat::OnceLock;
#[cfg(all(target_os = "linux", feature = "gtk-native"))]
use crate::core::MutexExt;
use crate::core::PlatformFamily;
#[cfg(target_os = "linux")]
use crate::platform::accessibility::linux::LinuxAccessibilityBridge;
#[cfg(target_os = "linux")]
use crate::platform::accessibility::AccessibilityBridge;
use crate::platform::{DropEvent, Platform};
#[cfg(all(target_os = "linux", feature = "gtk-native"))]
use gtk::prelude::*;
use std::sync::atomic::Ordering;
#[cfg(not(all(target_os = "linux", feature = "gtk-native")))]
use std::thread;
#[cfg(not(all(target_os = "linux", feature = "gtk-native")))]
use std::time::Duration;

impl Platform for LinuxPlatform {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn backend_name(&self) -> &'static str {
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            "gtk"
        }
        #[cfg(not(all(target_os = "linux", feature = "gtk-native")))]
        {
            // Honest name: without the `gtk-native` feature this backend keeps
            // widget state in-process and never opens native GTK windows.
            "linux-state-backend"
        }
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

    /// `lp` or `lpr` must be present for the system print backend to work.
    fn has_print_support(&self) -> bool {
        crate::platform::types::unix_print_clients_available()
    }

    /// Hands back a real `webkit2gtk::WebView` wrapper when the `webkit-engine`
    /// feature is on and GTK can create one.
    ///
    /// `None` on a headless host or a build without the feature, which tells
    /// `src/web/` to use its simulated navigation path.
    #[cfg(all(target_os = "linux", feature = "webkit-engine", widgets_unstripped))]
    fn create_web_engine(&self) -> Option<Box<dyn crate::platform::types::NativeWebEngine>> {
        super::webkit_engine::WebKitEngine::new()
            .map(|engine| Box::new(engine) as Box<dyn crate::platform::types::NativeWebEngine>)
    }

    /// A self-drawn widget gets a `gtk::DrawingArea` inside the window's content
    /// container; its `draw` signal blits a frame from `widget::runtime`.
    /// See `linux/canvas.rs`.
    ///
    /// Gated on the same profile conditions as `canvas.rs`: `mini`/`embedded`
    /// have no widget registry, so the trait defaults apply and
    /// `supports_surfaces()` honestly reports `false`.
    #[cfg(widgets_unstripped)]
    fn mount_surface(
        &self,
        parent: crate::core::ObjectId,
        id: crate::core::ObjectId,
        rect: crate::core::Rect,
    ) -> bool {
        super::canvas::mount_canvas(self, parent, id, rect)
    }

    #[cfg(widgets_unstripped)]
    fn resize_surface(&self, id: crate::core::ObjectId, rect: crate::core::Rect) -> bool {
        super::canvas::resize_canvas(self, id, rect)
    }

    #[cfg(widgets_unstripped)]
    fn unmount_surface(&self, id: crate::core::ObjectId) -> bool {
        super::canvas::unmount_canvas(self, id)
    }

    /// `true` only when the widget surface exists for this profile.
    #[cfg(widgets_unstripped)]
    fn supports_surfaces(&self) -> bool {
        true
    }

    /// Queue a redraw on the canvas's `DrawingArea`.
    #[cfg(widgets_unstripped)]
    fn invalidate_surface(&self, id: crate::core::ObjectId) -> bool {
        super::canvas::repaint_canvas(self, id)
    }
    fn init(&self) {
        self.runtime.initialized.store(true, Ordering::SeqCst);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            if let Err(e) = gtk::init() {
                log::error!("[linux] gtk::init() failed: {:?}", e);
            }
        }
        #[cfg(not(all(target_os = "linux", feature = "gtk-native")))]
        {}
    }
    fn run(&self) {
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            gtk::main();
        }
        #[cfg(not(all(target_os = "linux", feature = "gtk-native")))]
        {
            if !self.runtime.initialized.load(Ordering::SeqCst) {
                self.init();
            }
            self.runtime.running.store(true, Ordering::SeqCst);
            while self.runtime.running.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(16));
            }
        }
    }
    fn quit(&self) {
        self.runtime.running.store(false, Ordering::SeqCst);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            gtk::main_quit();
        }
    }

    /// Release every registry entry the backend holds for `widget_id`.
    ///
    /// Beyond the authoritative `BackendState` record, the Linux backend keeps
    /// per-widget entries only in the native GTK registries in `native` (under
    /// `gtk-native`). They must be purged, otherwise a UI rebuilt in a
    /// create/destroy loop would leak one entry per discarded widget.
    ///
    /// Only the library's own bookkeeping is released here: no GTK call is made,
    /// and the native objects are dropped when their registry entries are removed
    /// (GTK keeps its own reference for objects still attached to a parent).
    fn destroy_widget(&self, widget_id: u64) -> bool {
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let mut native = self.native.lock_guard();
            native.windows.remove(&widget_id);
            native.root_boxes.remove(&widget_id);
            native.content_fixed.remove(&widget_id);
            native.widgets.remove(&widget_id);
        }

        // The state record is the authority on whether the widget existed.
        self.state.destroy_widget(widget_id)
    }

    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        let id = self.insert_widget(LinuxHandleKind::Window, title, x, y, width, height);
        // A fresh GTK toplevel is restored, windowed, resizable and decorated.
        self.state.init_window_state(id, crate::platform::state::WindowStateRecord::new_window());
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let window = gtk::Window::new(gtk::WindowType::Toplevel);
            window.set_title(title);
            window.set_default_size(width as i32, height as i32);
            window.move_(x, y);
            let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
            let fixed = gtk::Fixed::new();
            root.pack_start(&fixed, true, true, 0);
            window.add(&root);
            let mut native = self.native.lock_guard();
            native.windows.insert(id, window.clone());
            native.root_boxes.insert(id, root);
            native.content_fixed.insert(id, fixed.clone());
            native.widgets.insert(id, window.clone().upcast::<gtk::Widget>());
        }
        id
    }

    #[cfg(target_os = "linux")]
    fn ime_bridge(&self) -> Option<&dyn crate::platform::ime::ImeBridge> {
        Some(&self.ime_bridge)
    }

    #[cfg(target_os = "linux")]
    fn accessibility_bridge(&self) -> Option<&dyn AccessibilityBridge> {
        static BRIDGE: OnceLock<LinuxAccessibilityBridge> = OnceLock::new();
        Some(BRIDGE.get_or_init(LinuxAccessibilityBridge::new))
    }
}
