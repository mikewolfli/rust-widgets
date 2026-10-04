// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `Platform` trait implementation for the WASM backend.

use super::types::{WasmHandleKind, WasmPlatform};
use crate::compat::atomic::Ordering;
use crate::compat::{format, String};
use crate::core::PlatformFamily;
use crate::platform::{Platform, PlatformCapabilities};
#[cfg(not(target_arch = "wasm32"))]
use core::time::Duration;
#[cfg(not(target_arch = "wasm32"))]
use std::thread;

/// The frame interval this backend's loop runs at, in milliseconds.
///
/// The same value as every other backend's twin constant, and named here for the same
/// reason: the sleep between iterations and the delta handed to [`crate::drive_frame`]
/// must be the same number, or every transition runs at the ratio between them with
/// nothing to notice the mismatch. A backend that later runs at the display's own rate
/// changes exactly this value.
///
/// Gated with the loop that reads it: on `wasm32` the browser owns the clock and this
/// backend's `run` returns immediately instead of sleeping, so the constant has no consumer
/// there — and an ungated copy is the dead-code warning a build with `wasm` enabled produced.
#[cfg(not(target_arch = "wasm32"))]
const FRAME_INTERVAL_MS: u64 = 16;

impl Platform for WasmPlatform {
    // The uniform widget-property methods are answered once, over `self.state`, by the
    // shared expansion in `platform::state_impl` rather than re-written per backend.
    crate::impl_platform_state_properties!();

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn backend_name(&self) -> &'static str {
        "wasm-state-backend"
    }

    /// The browser sandbox has no operating system to delegate to.
    ///
    /// This is `Embedded`, not `Desktop`, and the distinction is load-bearing:
    /// `capabilities()`'s trait default keys off the family, so reporting
    /// `Desktop` made this backend claim DPI scaling, IME, accessibility and a
    /// native menu — none of which a web page can obtain. It also contradicted
    /// [`crate::platform::portable`], the other hostless backend, which reports
    /// `Embedded` for exactly this reason. `README.md`'s published matrix has
    /// always said `Embedded | ❌ | ❌ | ❌ | ❌ | ❌`.
    fn family(&self) -> PlatformFamily {
        PlatformFamily::Embedded
    }

    /// Every host-provided flag is a truthful `false`.
    ///
    /// Stated explicitly rather than inherited: see [`WasmPlatform::family`]. The
    /// family now yields the same answer, but writing it out means a future edit
    /// to `default_capabilities_for` cannot silently re-grant a capability the
    /// sandbox does not have.
    fn capabilities(&self) -> PlatformCapabilities {
        PlatformCapabilities {
            dpi_scaling: false,
            ime: false,
            accessibility: false,
            native_menu: false,
            typed_widget_trigger: true,
        }
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
            // The loop has no native message pump of its own — a browser host drives the
            // events — so this is where the library's trigger queue is drained and its
            // animation bus is advanced. Draining alone left a `Resized` event reaching its
            // window layout but every animation unreachable, because nothing ran the
            // per-frame step (BLUE24 §0A.1 measurement 1). See `crate::drive_frame`.
            while self.runtime.running.load(Ordering::SeqCst) {
                crate::drive_frame(FRAME_INTERVAL_MS as u32);
                thread::sleep(Duration::from_millis(FRAME_INTERVAL_MS));
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
    /// Pops the next typed widget-trigger event from this backend's queue.
    ///
    /// # Why this backend must delegate
    ///
    /// [`PlatformCapabilities::typed_widget_trigger`] claims this backend produces and delivers
    /// typed triggers, and it holds a `BackendState<u64>` — the shared queue — exactly like
    /// `harmony`, `android`, `ios`, `wayland`, `windows` and `mobile`, all of which write these two
    /// lines. This backend did not, so both answered the trait defaults (`None` / `false`) while the
    /// flag said `true`.
    ///
    /// The gap is reachable: `NativeControlBackend` forwards both methods to `get_platform()`, so
    /// an injected trigger was accepted and then dropped on the floor — the "reported success for
    /// something that did not happen" shape the default is documented to avoid claiming.
    fn poll_widget_trigger_event(&self) -> Option<crate::platform::WidgetTriggerEvent> {
        self.state.pop_widget_trigger_event()
    }

    /// Pushes a typed widget-trigger event, refusing ids this backend never made.
    fn inject_widget_trigger_event(
        &self,
        widget_id: u64,
        kind: crate::platform::WidgetTriggerKind,
    ) -> bool {
        self.state.inject_widget_trigger_event(widget_id, kind)
    }

    /// Pops the next pending trigger as a bare id, over the same queue as the typed view.
    fn poll_widget_triggered(&self) -> Option<u64> {
        self.poll_widget_trigger_event().map(|event| event.widget_id)
    }

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

    /// Whether this backend can host library-painted widgets.
    ///
    /// # Why this is `true` now, and was `false` before
    ///
    /// It was `false` because no surface method was implemented: the flag told a host "yes",
    /// the host built a widget tree, and the first mount came back
    /// `SurfaceMountError::RejectedByBackend` — the one thing `supports_surfaces` exists to let
    /// a host avoid. The doc on this method then said the flag would turn `true` "in the same
    /// commit as the implementation", and this is that commit.
    ///
    /// # What changed in the implementation
    ///
    /// The backend now implements the surface table over its own [`BackendState`] —
    /// `mount_surface`, `resize_surface`, `unmount_surface`, `invalidate_surface` and
    /// `take_pending_repaint` — which is the same record-plus-queue shape `harmony`, `android`,
    /// `ios` and `macos_objc2` use. That is exactly what this backend was missing: it already
    /// had a canvas to present into (`canvas_id`, the 2D context), and the trait's surface
    /// table is the per-widget half that tells the host *what* to draw.
    ///
    /// The distinction the old doc drew — "painting into its own canvas is the presentation
    /// path, while `supports_surfaces` asks about the per-widget surface table" — was correct,
    /// and it is precisely why the fix is to add the table rather than to re-argue the flag.
    ///
    /// # What a host does with it
    ///
    /// Mount each widget, then each frame call `rw_take_pending_repaint` to learn which went
    /// stale and `rw_render_surface_frame` to get its RGBA, blitting into the canvas. The
    /// browser's own `ResizeObserver` (see [`Self::observe_canvas_resize`]) keeps the size and
    /// the layout honest.
    fn supports_surfaces(&self) -> bool {
        true
    }

    /// Mounts a widget onto a surface this host will present.
    ///
    /// See [`Self::supports_surfaces`] for what changed and why the canvas alone was not
    /// enough. `false` for an id this backend did not create, so a host is told rather than
    /// recorded into a table nothing can render.
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

    /// Queues a repaint for the host to pick up. `false` when `id` is unknown.
    ///
    /// A **window** id is accepted as well as a mounted surface's, because the library repaints
    /// a window to reveal the ordinary children it draws into that window's frame — see
    /// [`Platform::invalidate_surface`]. Answering only for mounted surfaces made those requests
    /// silent no-ops on every record-backed backend.
    fn invalidate_surface(&self, id: u64) -> bool {
        self.state.record_repaint_request(id)
    }

    /// Pops the next widget awaiting a repaint, for the host page to render.
    ///
    /// This is the drain half of [`Self::invalidate_surface`]: the page learns which surface
    /// went stale and pulls its frame. Without it the queue would grow without bound and the
    /// canvas would never be told what to draw.
    fn take_pending_repaint(&self) -> Option<crate::core::ObjectId> {
        self.state.take_pending_repaint()
    }

    // ─── Widget mutation ───────────────────────────────────────────────────────

    fn set_clipboard_text(&self, text: &str) -> bool {
        #[cfg(all(target_arch = "wasm32", not(target_os = "wasi")))]
        {
            // `window()` returns `Option<Window>`; `navigator()` returns
            // `Navigator` directly, so use `map` (not `and_then`).
            if let Some(navigator) = web_sys::window().map(|w| w.navigator()) {
                // `clipboard()` returns `Clipboard` directly in this web-sys version.
                let clipboard = navigator.clipboard();
                // Update the synchronous mirror **now**, so a `get_clipboard_text()` immediately
                // after this call returns what was just written: the system write is asynchronous, but
                // the mirror is the memory the synchronous readback reads, and leaving it stale made a
                // successful write read back as the previous text (the defect the old body had, where
                // the promise was dropped and the mirror untouched).
                self.state.set_clipboard_text(text);
                let promise = clipboard.write_text(text);
                // The write can still be **rejected** (no permission, lost user activation). We cannot
                // block for the result here, so attach a rejection handler that reports it rather than
                // dropping the promise silently: a permission refusal then appears in the console
                // instead of being indistinguishable from success. The returned `true` means "the write
                // was initiated and mirrored", which is the strongest synchronous claim available —
                // and it is stated as such rather than pretending the system write already succeeded.
                wasm_bindgen_futures::spawn_local(async move {
                    if let Err(err) = wasm_bindgen_futures::JsFuture::from(promise).await {
                        log::warn!(
                            "[wasm] clipboard.writeText was rejected ({err:?}); the synchronous \
                             mirror already holds the text, but the system clipboard was not updated"
                        );
                    }
                });
                return true;
            }
        }
        // Fallback to in-memory clipboard when native API is unavailable.
        self.state.set_clipboard_text(text)
    }

    fn get_clipboard_text(&self) -> String {
        #[cfg(all(target_arch = "wasm32", not(target_os = "wasi")))]
        {
            // A **synchronous** read cannot await `clipboard.read_text()`'s promise, so this answers
            // from the mirror `set_clipboard_text` maintains. That mirror is updated at write time
            // (see `set_clipboard_text`), so a read after a successful local write returns the written
            // text rather than a stale value. A read of content written by *another* app is not
            // possible synchronously in a browser; the mirror is the honest boundary of what this API
            // can answer here.
        }
        self.state.clipboard_text()
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
        assert_eq!(p.family(), PlatformFamily::Embedded);
    }

    /// The browser sandbox must not advertise a single host capability.
    ///
    /// `family()` used to answer `Desktop`, which made the trait default claim DPI
    /// scaling, IME, accessibility and a native menu on a web page. This pins the
    /// published `README.md` row rather than the family default, so the two cannot
    /// drift apart again.
    #[test]
    fn host_capabilities_are_all_false() {
        let caps = make_platform().capabilities();
        assert!(!caps.dpi_scaling, "a browser page cannot query host DPI");
        assert!(!caps.ime, "the sandbox exposes no OS input-method bridge");
        assert!(!caps.accessibility, "the sandbox exposes no OS accessibility tree");
        assert!(!caps.native_menu, "a web page has no native menu bar");
        assert!(
            caps.typed_widget_trigger,
            "typed triggers are produced by the library, not granted by the host"
        );
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

    /// The WASM backend advertises the per-widget surface capability, and the
    /// implementation behind it is real.
    ///
    /// # Why this assertion flipped
    ///
    /// It previously asserted `false`, on the reasoning that painting into the host canvas is
    /// this backend's own presentation path while [`Platform::supports_surfaces`] asks about
    /// the *per-widget surface table*. That reasoning was right, and it named the fix rather
    /// than a reason not to make it: the backend now **has** the table
    /// (`mount_surface`/`resize_surface`/`unmount_surface`/`invalidate_surface`/
    /// `take_pending_repaint` over its `BackendState`), so the flag and the methods agree.
    ///
    /// Asserting both halves together is the point: `true` alone would also be produced by a
    /// backend that merely claims the capability, so the mount, the repaint queue and the
    /// unmount are exercised too. This is the property the old test was protecting — that a
    /// capability flag is a promise — kept while the promise became keepable.
    #[test]
    fn the_surface_capability_is_advertised_and_backed_by_a_real_implementation() {
        let p = make_platform();
        assert!(
            p.supports_surfaces(),
            "the surface table is implemented, so the flag may promise one"
        );

        let window = p.create_window("wasm", 0, 0, 320, 240);
        let rect = crate::core::Rect::new(0, 0, 40, 20);
        assert!(p.mount_surface(window, window, rect), "a known id mounts");
        assert_eq!(p.state.surface_rect(window), Some(rect));

        // Invalidating queues exactly one repaint, which the host page then drains.
        assert!(p.invalidate_surface(window));
        assert!(p.invalidate_surface(window), "a second invalidate still reports the mount");
        assert_eq!(p.state.pending_repaint_count(), 1, "repaints are coalesced");
        assert_eq!(p.take_pending_repaint(), Some(window));

        // A window id is accepted for the same reason a mounted surface is: the library
        // repaints a window to reveal the ordinary children drawn into its frame.
        let other = p.create_window("other", 0, 0, 10, 10);
        assert!(p.invalidate_surface(other), "a window the backend knows can be repainted");

        // A widget this backend never made is refused rather than recorded.
        assert!(!p.mount_surface(window, 9_999, rect));
        assert!(!p.invalidate_surface(9_999));

        assert!(p.unmount_surface(window));
        assert_eq!(p.state.surface_rect(window), None);
        assert!(!p.unmount_surface(window), "unmounting twice reports no-op");
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
