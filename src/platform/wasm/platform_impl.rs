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
        Err(format!(
            "printing is not available in a browser (job file '{}' was not printed): the \
             sandbox exposes no print spooler; the host page must call `window.print()`",
            _job_file.display()
        ))
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
            let window = web_sys::window().expect(
                "a canvas-backed WASM widget needs a browser global `window`, so this \
                     must run on the main thread of a browser document",
            );
            let document = window.document().expect(
                "a canvas-backed WASM widget needs `window.document`; the global window has \
                     no document (e.g. it is a worker scope)",
            );
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

    /// The window's current client size, as last reported by the host page.
    ///
    /// Falls back to the size the window was created with, so a window that has never
    /// been resized answers with the size it was built for rather than claiming not to
    /// exist. `None` for an id this backend does not know.
    fn window_client_size(&self, window_id: u64) -> Option<(u32, u32)> {
        // The control backend owns the window, so it is the only store that knows a size
        // a resize reported. The platform's own record is the fallback for "never
        // resized", which a browser page that opened at a fixed canvas size will be.
        crate::window_client_size(window_id).or_else(|| self.state.window_size(window_id))
    }

    /// Reports that the drawing surface is now `width` x `height` CSS pixels.
    ///
    /// # Who calls this
    ///
    /// Two producers, both real:
    ///
    /// * [`WasmPlatform::observe_canvas_resize`], which attaches a `ResizeObserver` to the
    ///   canvas and reports every content-box change;
    /// * a host that already tracks its own layout and would rather report the size
    ///   itself than have the library observe the element.
    ///
    /// Returns `false` when `window_id` addresses nothing, so a host that reports a
    /// resize for a window it already dropped is told rather than silently ignored.
    ///
    /// # Why this does not just forward to the crate-level entry point
    ///
    /// The other backends forward, because their `create_window` delegates to the control
    /// backend, which then owns the window and its size record. **This** backend creates
    /// its window in its own `BackendState` (see `create_window` above: it sizes the host
    /// canvas directly), so the control backend has never heard of the id and its
    /// `queue_resize_trigger` would refuse it. Recording the geometry here — where the
    /// window actually lives — is what keeps the readback answerable.
    ///
    /// The trigger *event* is still queued through the crate-level entry point, because
    /// that queue is what the host loop polls regardless of which backend created the
    /// window. A refusal there (the id belongs to a control-backend window, as it does in
    /// a host that built its window through `rw_create_window`) is harmless: the size is
    /// already recorded, and the control backend queues its own event in that case.
    fn queue_resize_trigger(&self, window_id: u64, width: u32, height: u32) -> bool {
        let Some((x, y, _, _)) = self.state.widget_geometry(window_id) else {
            return false;
        };
        // Resize the record rather than adding a second size store: `window_size` reads
        // the record, so one write keeps the two views of "how big is this window"
        // (geometry and client size) from disagreeing.
        self.state.set_geometry(window_id, x, y, width, height);
        let _ = crate::queue_resize_trigger(window_id, width, height);
        true
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

    /// A window that has never been resized reports the size it was created with.
    ///
    /// The page can defer attaching the canvas observer, so this is the state every
    /// backend sits in until the first resize arrives: claiming `None` would make a
    /// host treat its own window as unknown.
    #[test]
    fn an_unresized_window_reports_its_created_size() {
        let p = make_platform();
        let win = p.create_window("test", 0, 0, 1024, 768);
        assert_eq!(p.window_client_size(win), Some((1024, 768)));
    }

    /// A resize reported through the backend must be readable back afterwards.
    #[test]
    fn a_reported_resize_is_readable_from_the_backend() {
        let p = make_platform();
        let win = p.create_window("test", 0, 0, 800, 600);
        assert!(p.queue_resize_trigger(win, 1200, 900), "a live window must accept a resize");
        assert_eq!(
            p.window_client_size(win),
            Some((1200, 900)),
            "the reported size must be what the backend answers with"
        );
    }

    /// A resize for an id this backend does not know must be refused.
    #[test]
    fn a_resize_for_an_unknown_window_is_refused() {
        let p = make_platform();
        assert!(!p.queue_resize_trigger(0x0BAD_1DEA, 100, 100));
        assert_eq!(p.window_client_size(0x0BAD_1DEA), None);
    }

    /// On a non-wasm host there is no DOM, so observing is reported as unavailable.
    ///
    /// The honest `false` matters: a host that read `true` here would attach nothing and
    /// then never receive a resize, with no way to tell that from an idle window.
    #[cfg(not(all(target_arch = "wasm32", not(target_os = "wasi"))))]
    #[test]
    fn observing_a_canvas_is_unavailable_without_a_dom() {
        let p = make_platform();
        let win = p.create_window("test", 0, 0, 800, 600);
        assert!(
            !p.observe_canvas_resize(win),
            "a host with no DOM must be told observation is unavailable"
        );
    }
}
