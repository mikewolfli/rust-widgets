// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `Platform` implementation for Linux (GTK when `gtk-native` is on, otherwise a
//! state-only host).
//!
//! Two facts drive most of what is here:
//!
//! * **GTK binds a process to one main thread.** Building a widget from any other
//!   thread aborts the process, so the surface methods check
//!   `gtk::is_initialized_main_thread()` and refuse rather than crash. That refusal is
//!   reported as `false`, which callers must read as "cannot display here".
//! * **OpenHarmony also reports `target_os = "linux"`.** Every arm that selects this
//!   backend therefore excludes `target_env = "ohos"`, or an OpenHarmony build would
//!   pick a GTK host that cannot exist there.

use super::types::{LinuxHandleKind, LinuxPlatform};
use crate::compat::atomic::Ordering;
use crate::compat::OnceLock;
use crate::compat::String;
#[cfg(all(target_os = "linux", feature = "gtk-native"))]
use crate::core::MutexExt;
use crate::core::PlatformFamily;
#[cfg(target_os = "linux")]
use crate::platform::accessibility::linux::LinuxAccessibilityBridge;
#[cfg(target_os = "linux")]
use crate::platform::accessibility::AccessibilityBridge;
use crate::platform::Platform;
#[cfg(not(all(target_os = "linux", feature = "gtk-native")))]
use core::time::Duration;
#[cfg(all(target_os = "linux", feature = "gtk-native"))]
use gtk::prelude::*;
// `glib` is a `gtk` re-export rather than a declared dependency of this crate, so it is
// named through `gtk` instead of as a bare crate (the same reason `canvas.rs` aliases it).
// Gated on `gtk-native`: without GTK there is nothing to re-export it from, and this
// module compiles then too.
#[cfg(all(target_os = "linux", feature = "gtk-native"))]
use gtk::glib;
#[cfg(not(all(target_os = "linux", feature = "gtk-native")))]
use std::thread;

impl Platform for LinuxPlatform {
    fn as_any(&self) -> &dyn crate::compat::Any {
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
    /// Gated on exactly the same conditions as `canvas.rs` itself: `mini`/
    /// `embedded` have no widget registry, and a build without `gtk-native` has
    /// no GTK toplevel to put a `DrawingArea` in. In both cases the trait
    /// defaults apply and `supports_surfaces()` honestly reports `false`.
    #[cfg(all(target_os = "linux", feature = "gtk-native", widgets_unstripped))]
    fn mount_surface(
        &self,
        parent: crate::core::ObjectId,
        id: crate::core::ObjectId,
        rect: crate::core::Rect,
    ) -> bool {
        super::canvas::mount_canvas(self, parent, id, rect)
    }

    #[cfg(all(target_os = "linux", feature = "gtk-native", widgets_unstripped))]
    fn resize_surface(&self, id: crate::core::ObjectId, rect: crate::core::Rect) -> bool {
        super::canvas::resize_canvas(self, id, rect)
    }

    #[cfg(all(target_os = "linux", feature = "gtk-native", widgets_unstripped))]
    fn unmount_surface(&self, id: crate::core::ObjectId) -> bool {
        super::canvas::unmount_canvas(self, id)
    }

    /// `true` only when the widget surface exists for this profile.
    #[cfg(all(target_os = "linux", feature = "gtk-native", widgets_unstripped))]
    fn supports_surfaces(&self) -> bool {
        true
    }

    /// Shows or hides a GTK toplevel.
    ///
    /// # Why this has to exist
    ///
    /// Nothing else showed a window. The only `show_all()` in the backend sat inside
    /// `mount_canvas`, so a window became visible **as a side effect of mounting a surface
    /// onto it**: a demo that mounted something appeared, and a demo that mounted nothing
    /// never appeared at all — while its log still said the window was shown. Showing a
    /// window is its own operation, so it is implemented here rather than left to fall out
    /// of an unrelated call.
    ///
    /// Both ids are accepted: the **host** window id the platform built (what a
    /// `WindowHandle` carries) and the **widget** id of the window itself. A caller reaches
    /// here with one or the other depending on which layer it sits in, and resolving both
    /// keeps the caller from having to know which it holds.
    ///
    /// The trait's signature returns `()`, so an id that names no GTK window is ignored
    /// rather than reported. That is the common case, not an error: most ids arriving here
    /// are ordinary controls, for which the model flag is the whole story.
    #[cfg(all(target_os = "linux", feature = "gtk-native"))]
    fn set_widget_visible(&self, widget_id: crate::core::ObjectId, visible: bool) {
        // GTK widgets are main-thread-only; driving them from anywhere else would abort.
        if !gtk::is_initialized_main_thread() {
            return;
        }
        let native = self.native.lock_guard();
        let host = if native.windows.contains_key(&widget_id) {
            Some(widget_id)
        } else {
            crate::widget::runtime::host_window_for(widget_id)
        };
        let Some(window) = host.and_then(|host| native.windows.get(&host)) else {
            return;
        };
        if visible {
            // `show_all` rather than `show`: a child created before its parent was realized
            // is not shown by `show` alone, and a window's controls are created exactly
            // that way.
            window.show_all();
        } else {
            window.hide();
        }
    }

    /// Queue a redraw on the canvas's `DrawingArea`.
    #[cfg(all(target_os = "linux", feature = "gtk-native", widgets_unstripped))]
    fn invalidate_surface(&self, id: crate::core::ObjectId) -> bool {
        super::canvas::repaint_canvas(self, id)
    }
    /// Queue a redraw of one rectangle of the canvas's `DrawingArea`.
    ///
    /// Gated exactly like [`Self::invalidate_surface`]: without `gtk-native` there are
    /// no canvases to invalidate, so the trait default (`false`) is the honest answer
    /// and the caller falls back to a whole-surface repaint.
    #[cfg(all(target_os = "linux", feature = "gtk-native", widgets_unstripped))]
    fn invalidate_surface_rect(&self, id: crate::core::ObjectId, rect: crate::core::Rect) -> bool {
        super::canvas::repaint_canvas_rect(self, id, rect)
    }
    fn init(&self) {
        self.runtime.initialized.store(true, Ordering::SeqCst);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            // GTK 3 permits `gtk::init()` on exactly one thread per process and
            // **aborts the process** if another thread calls it ("Attempted to
            // initialize GTK from two different threads"). The check-then-init
            // sequence must therefore be atomic: testing `is_initialized()` first
            // and initializing second lets two threads both observe "not
            // initialized" and then both call `gtk::init()`, which is a data race
            // inside GTK (observed as intermittent panics and, under load, a
            // SIGSEGV). Holding a process-wide mutex across both steps makes the
            // first caller the GTK main thread and every later caller a no-op, so
            // `init()` is safe from any thread — which is what a test runner (one
            // worker thread per `#[test]`) and a worker thread both require.
            use std::sync::{Mutex, OnceLock};
            static GTK_INIT_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
            let lock = GTK_INIT_LOCK.get_or_init(|| Mutex::new(()));
            let _guard = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());

            if gtk::is_initialized_main_thread() {
                // Already owned by this thread; nothing to do.
            } else if gtk::is_initialized() {
                log::debug!(
                    "[linux] init: GTK is already initialized on another thread; \
                     not re-initializing (GTK permits a single main thread)"
                );
            } else if let Err(e) = gtk::init() {
                log::error!("[linux] gtk::init() failed: {:?}", e);
            }
        }
        #[cfg(not(all(target_os = "linux", feature = "gtk-native")))]
        {}
    }
    fn run(&self) {
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            // Drain the widget-trigger queue on every tick of the GTK loop.
            //
            // `gtk::main()` runs the toolkit's loop, and the toolkit's callbacks are what
            // *fill* the trigger queue: a `connect_size_allocate` handler calls
            // `queue_resize_trigger`, which pushes a `Resized` event. `gtk::main()` never
            // reads that queue back, so the event sat there and the layout was never
            // re-run — the backend reported a resize correctly and the library never acted
            // on it. See `crate::drain_triggers` for the contract.
            //
            // `glib::timeout_add_local` runs its closure on the GTK main thread, which is
            // the thread GTK requires for widget work, so dispatching from here is
            // main-thread work rather than a cross-thread call.
            glib::timeout_add_local(core::time::Duration::from_millis(16), || {
                crate::drain_triggers();
                glib::ControlFlow::Continue
            });
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
            native.content_overlay.remove(&widget_id);
            native.window_painters.remove(&widget_id);
            native.widgets.remove(&widget_id);
        }

        // The state record is the authority on whether the widget existed.
        self.state.destroy_widget(widget_id)
    }

    /// Creates a top-level window, with a real GTK toplevel when this is the GTK
    /// main thread.
    ///
    /// # Off-main callers get a state-only window
    ///
    /// GTK 3 binds every widget to one main thread: `gtk::Window::new` (and the
    /// rest of the toolkit) calls `assert_initialized_main_thread!()`, which
    /// **aborts the process** from any other thread. Off-main is a real case — the
    /// C ABI may be driven from a worker thread, and a test harness runs every
    /// `#[test]` on its own thread. Skipping the native construction there and
    /// returning a state-only handle keeps the call honouring its contract (a
    /// valid, text/geometry-consistent id) instead of taking down the process.
    ///
    /// This mirrors `CocoaPlatform::create_window`, which refuses to construct an
    /// `NSWindow` off the AppKit main thread for exactly the same reason.
    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        let id = self.insert_widget(LinuxHandleKind::Window, title, x, y, width, height);
        // A fresh GTK toplevel is restored, windowed, resizable and decorated.
        self.state.init_window_state(id, crate::platform::state::WindowStateRecord::new_window());
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            // Only the thread that owns GTK may build widgets on it.
            if !gtk::is_initialized_main_thread() {
                log::debug!(
                    "[linux] create_window: off the GTK main thread; registered a \
                     state-only window (id={id}). GTK widgets are main-thread-only."
                );
                return id;
            }
            let window = gtk::Window::new(gtk::WindowType::Toplevel);
            window.set_title(title);
            window.set_default_size(width as i32, height as i32);
            window.move_(x, y);
            let root = gtk::Box::new(gtk::Orientation::Vertical, 0);

            // The window-content painter, with the surface container above it.
            //
            // The backend creates no native controls, so a window's ordinary children —
            // buttons, check boxes, labels — have no GTK widget of their own and nothing
            // painted them: the window showed its chrome over an empty client area. This
            // `DrawingArea` closes that gap by painting the window's whole widget tree
            // itself (see `crate::widget::runtime::render_frame_tree`).
            //
            // The stacking order matters: the painter goes in first so it is the *bottom*,
            // and the overlay holding mounted surfaces sits above it, so a self-drawn
            // control mounted as a surface paints over the tree. That is what lets one
            // window mix both kinds.
            let paint_area = gtk::DrawingArea::new();
            // Expand rather than claim an absolute rectangle: the overlay resizes it with
            // the window, so a repaint after a resize covers the new extent.
            paint_area.set_hexpand(true);
            paint_area.set_vexpand(true);
            paint_area.set_has_tooltip(false);

            // The container the window layout positions into; see `content_overlay`.
            let overlay = gtk::Overlay::new();
            overlay.add(&paint_area);
            root.pack_start(&overlay, true, true, 0);
            window.add(&root);

            // Paint the window's widget tree on every expose.
            //
            // The id GTK reports here is the **platform's**, and the tree lives under the
            // widget-registry id, so the association recorded at window creation is what
            // translates between them. Painting the platform id would look up an id that
            // addresses no widget and draw nothing — a silently empty window, which is the
            // failure this path exists to remove.
            paint_area.connect_draw(move |widget, context| {
                let area_width = widget.allocated_width().max(1) as u32;
                let area_height = widget.allocated_height().max(1) as u32;
                let Some(window_widget) = crate::widget::runtime::widget_id_for_host_window(id)
                else {
                    // No widget owns this host window yet. Not an error, and nothing to
                    // paint: the clear colour already fills the area.
                    return glib::Propagation::Proceed;
                };
                match crate::widget::runtime::render_frame_tree(
                    window_widget,
                    crate::core::Size::new(area_width, area_height),
                    crate::core::Color::rgb(240, 240, 240),
                ) {
                    Some(frame) => {
                        super::canvas::blit_rgba(context, area_width, area_height, &frame);
                    }
                    None => {
                        log::debug!(
                            "[linux] window {id} produced no tree frame \
                             (no drawable children, or the window is not mounted)"
                        );
                    }
                }
                glib::Propagation::Proceed
            });

            // ── Input ─────────────────────────────────────────────────────────
            //
            // The window's tree painter also takes pointer input.
            //
            // # Why the painter is the input surface
            //
            // Painting a window's controls into one area means the area is where the
            // pointer lands, so it is the only thing that can receive a click. Without
            // this, a demo whose controls have no mounted surface of their own received
            // **no events at all**: the controls were visible and their geometry was
            // correct, so nothing looked wrong — the callbacks simply never ran.
            //
            // The router walks the window's child tree and hit-tests each control (see
            // `widget::runtime::dispatch_pointer_event`), so a click on a button reaches
            // that button. It needs the child links, which every created control records
            // (`control_backend::custom::mount_named_widget`).
            //
            // Coordinates are already in the window's space: the paint area starts at the
            // window's content origin, and controls are positioned in that same space.
            paint_area.add_events(
                gtk::gdk::EventMask::BUTTON_PRESS_MASK
                    | gtk::gdk::EventMask::BUTTON_RELEASE_MASK
                    | gtk::gdk::EventMask::POINTER_MOTION_MASK
                    | gtk::gdk::EventMask::SCROLL_MASK,
            );
            let click_area = paint_area.clone();
            paint_area.connect_button_press_event(move |widget, event| {
                let (x, y) = event.position();
                let point = crate::core::Point::new(x as i32, y as i32);
                let Some(window_widget) = crate::widget::runtime::widget_id_for_host_window(id)
                else {
                    return glib::Propagation::Proceed;
                };
                // The area is focusable so a clicked control can take the keyboard
                // afterwards; without the grab, keys would go to the window and typing
                // into a `LineEdit` would do nothing.
                widget.set_can_focus(true);
                if crate::widget::runtime::dispatch_pointer_event(
                    window_widget,
                    &crate::event::Event::MousePress { pos: point, button: 1 },
                    point,
                ) {
                    click_area.queue_draw();
                }
                glib::Propagation::Proceed
            });

            let release_area = paint_area.clone();
            paint_area.connect_button_release_event(move |widget, event| {
                let (x, y) = event.position();
                let point = crate::core::Point::new(x as i32, y as i32);
                let Some(window_widget) = crate::widget::runtime::widget_id_for_host_window(id)
                else {
                    return glib::Propagation::Proceed;
                };
                if crate::widget::runtime::dispatch_pointer_event(
                    window_widget,
                    &crate::event::Event::MouseRelease { pos: point, button: 1 },
                    point,
                ) {
                    widget.queue_draw();
                }
                // A released click is also what a click callback keys off, and the draw
                // above covers the visual half of it.
                release_area.queue_draw();
                glib::Propagation::Proceed
            });

            // Key events go to whatever the router focused, so typing reaches a field the
            // user clicked rather than always the window. Tab is forwarded too, which is
            // how focus moves between controls.
            paint_area.set_can_focus(true);
            paint_area.connect_key_press_event(move |widget, event| {
                let Some(window_widget) = crate::widget::runtime::widget_id_for_host_window(id)
                else {
                    return glib::Propagation::Proceed;
                };
                // `keyval()` is a `gdk::keys::Key`, which derefs to its numeric GDK
                // keyval — the value the widget layer's key handling expects.
                let key = *event.keyval();
                let key_event = crate::event::Event::KeyPress { key, modifiers: 0 };
                let target = crate::widget::runtime::focused_widget().unwrap_or(window_widget);
                if crate::widget::runtime::dispatch_event(target, &key_event) {
                    widget.queue_draw();
                }
                glib::Propagation::Proceed
            });

            // Report every re-allocation of the toplevel as a `Resized` trigger.
            //
            // Without this the library only learns a new window size when someone calls
            // `WindowHandle::set_geometry`, so a user dragging the window edge left every
            // child control at the geometry it had for the previous size. The signal fires
            // for programmatic resizes too, which is harmless: the caller is expected to
            // re-run its layout, and re-running it twice is idempotent.
            //
            // # Why the id is translated before reporting
            //
            // GTK hands this closure the **platform's** window id (a small number such as
            // `1`), while everything that consumes a resize — `queue_resize_trigger`,
            // `window_client_size`, `apply_window_layout` — is keyed by the
            // **widget-registry** id, which is a different and much larger number.
            // `queue_resize_trigger` validates its argument with `is_mounted` and refuses an
            // id that addresses no widget, so reporting the platform id made every resize a
            // silent no-op: measured, a window resized to 944x600 logged
            // `platform_id=1 size=944x600 widget_owner=<large> accepted=false`, and the
            // window layout never ran again.
            //
            // `widget_id_for_host_window` is the association recorded at window creation.
            // A window with no owner yet — built before any widget was associated with it —
            // reports nothing, which is correct: there is no layout that could react.
            window.connect_size_allocate(move |_, allocation| {
                let (width, height) =
                    (allocation.width().max(0) as u32, allocation.height().max(0) as u32);
                if let Some(widget_id) = crate::widget::runtime::widget_id_for_host_window(id) {
                    crate::queue_resize_trigger(widget_id, width, height);
                }
            });

            let mut native = self.native.lock_guard();
            native.windows.insert(id, window.clone());
            native.root_boxes.insert(id, root);
            native.content_overlay.insert(id, overlay.clone());
            native.window_painters.insert(id, paint_area);
            native.widgets.insert(id, window.clone().upcast::<gtk::Widget>());
        }
        id
    }

    #[cfg(target_os = "linux")]
    fn ime_bridge(&self) -> Option<&dyn crate::platform::ime::ImeBridge> {
        Some(&self.ime_bridge)
    }

    /// The window's current client size.
    ///
    /// With `gtk-native` and a real `gtk::Window` this asks GTK, which is the authority
    /// once the user has resized the window. Otherwise it falls back to the size recorded
    /// when the last resize was reported, and finally to the created geometry — each step
    /// answerable, so a caller always gets a real number rather than a guess.
    #[cfg(target_os = "linux")]
    fn window_client_size(&self, window_id: crate::core::ObjectId) -> Option<(u32, u32)> {
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            // GTK widgets are main-thread-only; asking from anywhere else would abort.
            if gtk::is_initialized_main_thread() {
                let native = self.native.lock_guard();
                if let Some(window) = native.windows.get(&window_id) {
                    let (width, height) = window.size();
                    if width > 0 && height > 0 {
                        return Some((width as u32, height as u32));
                    }
                }
            }
        }
        // Ask the control backend, which owns the window and is therefore the only
        // store that knows the size a resize reported.
        crate::window_client_size(window_id).or_else(|| self.state.window_size(window_id))
    }

    /// Reports a container's new client size and queues a `Resized` trigger.
    fn queue_resize_trigger(
        &self,
        window_id: crate::core::ObjectId,
        width: u32,
        height: u32,
    ) -> bool {
        // Forward to the control backend, which owns the window and the queue the app
        // polls. Writing to the platform's own state would land in a store the host
        // never reads, because `create_window` goes through the control backend.
        crate::queue_resize_trigger(window_id, width, height)
    }

    /// Stores the text in the backend's clipboard record.
    ///
    /// Without `gtk-native` this backend has no GDK clipboard to hand the text
    /// to, but the record is still the honest answer for the running process —
    /// and it is what every other backend's `state` delegation does
    /// (macOS/Harmony/iOS/Android/Wayland/Wasm). Inheriting the trait default
    /// here made a copy/paste inside the library a silent no-op while the
    /// identical call on every sibling backend worked.
    ///
    /// With `gtk-native` the text is also published to the display clipboard.
    /// That path is guarded by `is_initialized_main_thread()` because GDK aborts
    /// when driven from any other thread ("GDK may only be used from the main
    /// thread") — and widgets legitimately call this from worker threads, e.g. a
    /// background copy. Off the main thread the in-process record is still
    /// updated, which keeps the call useful instead of aborting the process.
    fn set_clipboard_text(&self, text: &str) -> bool {
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            // Same contract every native entry point in this backend follows
            // (see the `Send` note in `linux/types.rs`): touch GTK only from the
            // thread that called `gtk::init`.
            if gtk::is_initialized_main_thread() {
                if let Some(display) = gtk::gdk::Display::default() {
                    if let Some(clipboard) = gtk::Clipboard::default(&display) {
                        clipboard.set_text(text);
                        clipboard.store();
                    }
                }
            }
        }
        self.state.set_clipboard_text(text)
    }

    /// Reads back what [`Platform::set_clipboard_text`] stored.
    fn get_clipboard_text(&self) -> String {
        self.state.clipboard_text()
    }

    #[cfg(target_os = "linux")]
    fn accessibility_bridge(&self) -> Option<&dyn AccessibilityBridge> {
        static BRIDGE: OnceLock<LinuxAccessibilityBridge> = OnceLock::new();
        Some(BRIDGE.get_or_init(LinuxAccessibilityBridge::new))
    }
}
