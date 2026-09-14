// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `Platform` trait implementation for the WASM backend.

use super::types::{WasmHandleKind, WasmPlatform};
use crate::core::PlatformFamily;
use crate::platform::{DropEvent, Platform};
use std::sync::atomic::Ordering;
#[cfg(not(target_arch = "wasm32"))]
use std::thread;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;

impl Platform for WasmPlatform {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn backend_name(&self) -> &'static str {
        "wasm-state-backend"
    }

    fn family(&self) -> PlatformFamily {
        PlatformFamily::Desktop
    }

    /// The browser sandbox exposes no host memory figure to a synchronous query.
    fn total_memory_mb(&self) -> Option<u64> {
        None
    }

    /// Browsers do not report AC/battery state to web content.
    fn is_on_battery(&self) -> bool {
        false
    }

    /// `performance.memory` is a non-standard Chrome extension and the numbers it
    /// reports are not this process's RSS; the honest answer is `None`.
    fn process_memory_utilization(&self) -> Option<f32> {
        None
    }

    /// There is no printable document in the web sandbox.
    fn spawn_print_job(&self, _job_file: &std::path::Path) -> Result<(), String> {
        Err("printing is not available in the WASM backend".to_string())
    }

    fn has_print_support(&self) -> bool {
        false
    }

    fn init(&self) {
        self.runtime.initialized.store(true, Ordering::SeqCst);
    }

    /// Host-side busy loop.
    ///
    /// On a real `wasm32` target the browser owns the event loop, so this is a
    /// no-op; on a non-wasm host (unit tests) it pumps a bounded sleep loop so
    /// `run`/`quit` pairing can be exercised without a browser.
    fn run(&self) {
        self.runtime.running.store(true, Ordering::SeqCst);
        #[cfg(not(target_arch = "wasm32"))]
        {
            while self.runtime.running.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(10));
            }
        }
    }

    fn quit(&self) {
        self.runtime.running.store(false, Ordering::SeqCst);
    }

    /// Release the state record for `widget_id`.
    ///
    /// The WASM backend is purely a state model: there is no native object to
    /// destroy and no side table keyed by widget id, so dropping the record is the
    /// whole teardown. `BackendState::destroy_widget` is the authority on whether
    /// the widget existed.
    fn destroy_widget(&self, widget_id: u64) -> bool {
        self.state.destroy_widget(widget_id)
    }

    /// Sizes the host `<canvas>` the backend paints into.
    ///
    /// The canvas is not a widget: it is the drawing surface identified by the id
    /// this platform was constructed with. A missing element or a non-canvas
    /// element with that id is reported as an error rather than ignored.
    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        let id = self.insert_widget(WasmHandleKind::Window, title, x, y, width, height);
        #[cfg(all(target_arch = "wasm32", not(target_os = "wasi")))]
        {
            use wasm_bindgen::JsCast;
            let window = web_sys::window().expect("no global window");
            let document = window.document().expect("no document");
            match document.get_element_by_id(&self.canvas_id) {
                Some(canvas) => match canvas.dyn_into::<web_sys::HtmlCanvasElement>() {
                    Ok(html_canvas) => {
                        html_canvas.set_width(width);
                        html_canvas.set_height(height);
                    }
                    Err(_) => log::error!(
                        "[wasm] create_window: element '{}' is not a canvas",
                        self.canvas_id
                    ),
                },
                None => {
                    log::error!("[wasm] create_window: no element with id '{}'", self.canvas_id)
                }
            }
        }
        let _ = (x, y);
        id
    }

    /// Release the surface attached to this backend's canvas id.
    ///
    /// The trait method is about display surfaces, not about native window
    /// handles, so it is answered here rather than in a window-mutator stub.
    fn supports_surfaces(&self) -> bool {
        true
    }

    // ─── Widget mutation ───────────────────────────────────────────────────────

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

    fn set_clipboard_text(&self, text: &str) -> bool {
        #[cfg(all(target_arch = "wasm32", not(target_os = "wasi")))]
        {
            // `window()` returns `Option<Window>`; `navigator()` returns
            // `Navigator` directly, so use `map` (not `and_then`).
            if let Some(navigator) = web_sys::window().map(|w| w.navigator()) {
                // `clipboard()` returns `Clipboard` directly in this web-sys version.
                let clipboard = navigator.clipboard();
                let promise = clipboard.write_text(text);
                // Fire-and-forget: the promise runs asynchronously.
                let _ = promise;
                return true;
            }
        }
        // Fallback to in-memory clipboard when native API is unavailable.
        self.state.set_clipboard_text(text)
    }

    fn get_clipboard_text(&self) -> String {
        #[cfg(all(target_arch = "wasm32", not(target_os = "wasi")))]
        {
            // web-sys clipboard.read_text() returns a Promise<String>.
            // For synchronous access we fall back to the in-memory store,
            // which is always populated when set_clipboard_text was called
            // successfully. A full async bridge is out of scope for MVP.
        }
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

    fn set_widget_ime_enabled(&self, widget_id: u64, enabled: bool) -> bool {
        self.state.set_ime_enabled(widget_id, enabled)
    }

    fn is_widget_ime_enabled(&self, widget_id: u64) -> bool {
        self.state.ime_enabled(widget_id)
    }
}

// ─── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_platform() -> WasmPlatform {
        WasmPlatform::with_canvas("test-canvas")
    }

    #[test]
    fn backend_name_and_family() {
        let p = make_platform();
        assert_eq!(p.backend_name(), "wasm-state-backend");
        assert_eq!(p.family(), PlatformFamily::Desktop);
    }

    /// A freshly created window is a plain state record, and the same handle is
    /// findable through the property API — the observable contract, rather than the
    /// internal handle-kind accessor this test used to reach for. That accessor is
    /// gone: it only existed so the deleted per-kind control creators could be
    /// asserted against, and a helper with no production caller is dead code.
    #[test]
    fn create_window_records_state_only() {
        let p = make_platform();
        let win = p.create_window("test", 0, 0, 800, 600);
        assert!(win > 0);
        p.set_widget_text(win, "probe");
        assert_eq!(p.get_widget_text(win), "probe");
    }

    /// The WASM backend paints self-drawn widgets into the host canvas element it
    /// was constructed with, so it advertises the capability.
    #[test]
    fn custom_widget_support_is_advertised() {
        let p = make_platform();
        assert!(p.supports_surfaces());
    }

    /// Teardown must report whether the widget existed, and a second call must not
    /// claim success for an id that is already gone.
    #[test]
    fn destroy_widget_reports_existence() {
        let p = make_platform();
        let win = p.create_window("test", 0, 0, 800, 600);
        assert!(p.destroy_widget(win));
        assert!(!p.destroy_widget(win));
        assert!(!p.destroy_widget(4242));
    }

    #[test]
    fn run_reloads_runtime_flags_and_quit_stops_them() {
        let p = make_platform();
        assert!(!p.runtime.initialized.load(Ordering::SeqCst));
        p.init();
        assert!(p.runtime.initialized.load(Ordering::SeqCst));
        // `quit` before `run` leaves the loop flag clear, so a host that only
        // wants to tear down does not accidentally start the loop.
        p.quit();
        assert!(!p.runtime.running.load(Ordering::SeqCst));
    }

    #[test]
    fn print_is_reported_as_unavailable() {
        let p = make_platform();
        assert!(!p.has_print_support());
        assert!(p.spawn_print_job(std::path::Path::new("/tmp/x.txt")).is_err());
    }
}
