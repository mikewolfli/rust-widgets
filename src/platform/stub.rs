// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Stub platform implementation for testing and demonstrations.
//!
//! # BLUE15: this backend no longer creates controls
//!
//! A host backend owes the widget layer exactly two things: a **window** and a
//! **drawing surface**. Everything else — button, label, list box, menu bar,
//! dialog — is painted by [`crate::widget`], so a per-kind `create_*` here would
//! have no OS object to map onto and would only duplicate the library's own
//! semantics in a second place.
//!
//! The stub therefore implements the `Platform` defaults for every control
//! method (honestly reporting "this host provides no such primitive", principle
//! #37) and overrides only:
//!
//! * `create_window` — the single allocation entry point.
//! * the semantic **window** operations (`set_window_state`, `window_min_size`,
//!   `window_icon`, …), which are host facts rather than widget semantics.
//! * text/geometry/enabled/visible accessors, which the [`BackendState`] record
//!   answers for any id the layer registers — including self-drawn ones.
//! * event-queue and clipboard/drag-drop plumbing used by tests.
//!
//! `self-drawn` widgets are adopted through
//! [`BackendState::register_widget_with_id`], so `get_widget_text` and friends
//! keep working for them without any per-kind code here.

use crate::compat::{format, String};
use crate::core::{ObjectId, PlatformFamily};
use crate::platform::state::{BackendState, WindowStateRecord};
use crate::platform::types::*;

/// Handle kind discriminator for stub widget records.
///
/// Only [`StubHandleKind::Window`] is ever produced: the stub host allocates
/// windows, and every other kind of widget is owned by [`crate::widget`] and the
/// library's control backend. The enum survives because `BackendState` is
/// generic over its key, which keeps the state model free of platform enums.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum StubHandleKind {
    Window,
}

/// The in-memory `Platform` implementation used for tests and demos.
///
/// This backend is an **honest fallback, not a fake**: it allocates windows and
/// keeps widget state, but it deliberately implements no per-kind control
/// creation. Every control method falls through to the `Platform` trait default
/// and reports "unsupported" rather than pretending a control exists — there is
/// no OS object to map onto, and inventing one would put a second, divergent copy
/// of the library's own widget semantics behind a code path tests could not
/// distinguish from a real backend (principle #37).
///
/// What it *does* provide: window allocation, the semantic window operations, the
/// text/geometry/enabled/visible accessors (backed by [`BackendState`] for any id
/// registered via [`StubPlatform::register_widget`]), and the event-queue and
/// clipboard/drag-drop plumbing the tests drive. See the module header above for
/// the full list and the reasoning.
pub struct StubPlatform {
    backend: &'static str,
    family: PlatformFamily,
    state: BackendState<StubHandleKind>,
    /// Platform IME bridge for testing.
    pub(crate) ime_bridge: crate::platform::ime::MockImeBridge,
}

impl StubPlatform {
    /// Creates a new in-memory stub backend for tests and demos.
    pub fn new(backend: &'static str, family: PlatformFamily) -> Self {
        Self {
            backend,
            family,
            state: BackendState::new(),
            ime_bridge: crate::platform::ime::MockImeBridge::new(),
        }
    }

    /// Adopts `widget_id` as a live widget of this host.
    ///
    /// Called by the drawing bridge when a self-drawn widget is mounted: the id
    /// originates in `widget::runtime`, which owns the widget, so the
    /// host state records it rather than allocating its own. Without this the
    /// host would have no record for a widget it is already painting, and
    /// `get_widget_text` / `is_widget_visible` would answer for an unknown id.
    pub fn register_widget(
        &self,
        widget_id: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) {
        self.state.register_widget_with_id(
            widget_id,
            StubHandleKind::Window,
            text,
            x,
            y,
            width,
            height,
        );
    }

    /// Number of live widget records. Test/diagnostic aid for teardown checks.
    pub fn widget_count(&self) -> usize {
        self.state.widget_count()
    }
}

/// The process-wide host used by every build that has no OS backend module.
///
/// One shared value rather than one per caller: the host holds the drag-and-drop
/// and widget-trigger queues, so two instances would deliver an injected event to
/// whichever one the caller happened to ask. `StubPlatform` is `Send + Sync`, which
/// is what makes a static instance sound here.
pub(crate) fn stub_platform_singleton() -> &'static StubPlatform {
    use crate::compat::OnceLock;

    static HOST: OnceLock<StubPlatform> = OnceLock::new();
    HOST.get_or_init(|| StubPlatform::new("portable", PlatformFamily::Embedded))
}

impl Platform for StubPlatform {
    // The uniform widget-property methods — value/range/step/checked/tristate/
    // selected-index/selection/placeholder/echo-mode/read-only/max-length/
    // orientation/scroll/indeterminate/ime-enabled/accessibility-name, the window
    // state trio, and the text/geometry/enabled/visible quartet — are all answered
    // by this backend's `BackendState` record. They have exactly **one** definition
    // (`platform::state_impl`'s macro) rather than a copy per backend, so the answer
    // cannot drift between the portable host and the ten backends sharing the store.
    crate::impl_platform_state_properties!();

    // ── Widget surfaces ─────────────────────────────────────────────────────
    //
    // # Why this host above all others must carry these
    //
    // The portable host's whole definition is "the host supplies a window and a drawing surface and
    // the library paints into it" (`platform/portable/mod.rs`). It is selected for `mini`, for
    // `embedded` on a host with no backend, for a target with no backend module, and for the two
    // macOS fallbacks. For every one of those, the surface *is* the contract — there is no toolkit
    // object to fall back on and no other layer that can supply it.
    //
    // It nevertheless inherited the trait defaults, so `supports_surfaces()` answered `false` and
    // `mount_surface` was absent: a `mini`/`embedded` host was told it could not display the
    // widgets the library had just built for it, and the only thing it exists to provide was the
    // one thing it could not. `linux`, `windows`, `macos`, `android`, `ios`, `harmony`, `wasm` and
    // `wayland` all answer these over the same shared `BackendState` tables; this host holds the
    // same record and simply had not been wired.
    //
    // The five methods below are therefore the **same delegations** every other record-backed
    // backend makes, not a second implementation. `false` for an id this host did not create, so a
    // host is told rather than recorded into a table nothing can render.

    /// This host can display library-painted widgets: it is what it exists for.
    fn supports_surfaces(&self) -> bool {
        true
    }

    /// Records a widget as mounted on a surface this host will present.
    fn mount_surface(
        &self,
        _parent: crate::core::ObjectId,
        id: crate::core::ObjectId,
        rect: crate::core::Rect,
    ) -> bool {
        self.state.mount_surface_record(id, rect)
    }

    /// Updates the rect of a mounted surface. `false` when `id` is not mounted.
    fn resize_surface(&self, id: crate::core::ObjectId, rect: crate::core::Rect) -> bool {
        self.state.resize_surface_record(id, rect)
    }

    /// Releases a mounted surface.
    fn unmount_surface(&self, id: crate::core::ObjectId) -> bool {
        self.state.unmount_surface_record(id)
    }

    /// Queues a repaint for the host to pick up. `false` when `id` is unknown.
    ///
    /// A **window** id is accepted as well as a mounted surface's, for the reason
    /// [`Platform::invalidate_surface`] gives: repainting a window reveals the ordinary children
    /// drawn into its frame, and answering only for mounted surfaces made those requests silent
    /// no-ops on every record-backed backend.
    fn invalidate_surface(&self, id: crate::core::ObjectId) -> bool {
        self.state.record_repaint_request(id)
    }

    /// Pops the next widget awaiting a repaint, for the host to render.
    ///
    /// The drain half of [`Self::invalidate_surface`]: without it the queue grows without bound
    /// and the host is never told what to draw.
    fn take_pending_repaint(&self) -> Option<crate::core::ObjectId> {
        self.state.take_pending_repaint()
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }

    fn backend_name(&self) -> &'static str {
        self.backend
    }

    fn family(&self) -> PlatformFamily {
        self.family
    }

    /// The stub has no OS to interrogate, so it reports an explicit `None` rather
    /// than a made-up figure. Callers fall back to their conservative default.
    fn total_memory_mb(&self) -> Option<u64> {
        None
    }

    /// A stub host is treated as mains-powered, which selects the non-throttled
    /// defaults.
    fn is_on_battery(&self) -> bool {
        false
    }

    /// No process accounting exists without an OS; `None` keeps the adaptive
    /// monitor on its default instead of pretending the process is at 0%.
    fn process_memory_utilization(&self) -> Option<f32> {
        None
    }

    /// Same reasoning as [`Platform::process_memory_utilization`].
    fn process_cpu_utilization(&self) -> Option<f32> {
        None
    }

    /// The stub cannot reach a real spooler, and saying so is the point: demos
    /// exercising the system print backend must see a truthful failure.
    fn spawn_print_job(&self, _job_file: &std::path::Path) -> Result<(), String> {
        Err(format!(
            "the stub platform has no print spooler, so job file '{}' was not printed; \
             select a real OS backend to print",
            _job_file.display()
        ))
    }

    fn init(&self) {
        log::info!("[stub] StubPlatform init (testing backend)");
    }

    fn run(&self) {
        log::info!("[stub] StubPlatform run (testing backend)");
    }

    fn quit(&self) {
        log::info!("[stub] StubPlatform quit");
    }

    /// Releases the registry entry the host holds for `widget_id`.
    ///
    /// The `BackendState` record is the authority on whether the widget existed,
    /// so its return value is the answer.
    fn destroy_widget(&self, widget_id: ObjectId) -> bool {
        self.state.destroy_widget(widget_id)
    }

    /// Allocates the one primitive this host owns.
    ///
    /// The record is seeded with the state a fresh OS window has: restored,
    /// visible, windowed, resizable and decorated.
    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        let id = self.state.create_widget(StubHandleKind::Window, title, x, y, width, height);
        self.state.init_window_state(id, WindowStateRecord::new_window());
        id
    }

    fn poll_menu_triggered(&self) -> Option<ObjectId> {
        self.state.pop_menu_event()
    }

    /// Queues a menu activation for `menu_item_id`.
    ///
    /// The stub does not create menu items itself — the widget layer owns the
    /// menu model — but it must still be able to deliver a menu event, because
    /// that is the only way the delivery path can be exercised on a host that
    /// owns no menu primitive. An unknown id is refused so the queue cannot be
    /// polluted with orphan events.
    fn inject_menu_trigger(&self, menu_item_id: ObjectId) -> bool {
        if !self.state.contains_widget(menu_item_id) {
            return false;
        }
        self.state.push_menu_event(menu_item_id);
        true
    }

    fn poll_widget_triggered(&self) -> Option<ObjectId> {
        self.poll_widget_trigger_event().map(|event| event.widget_id)
    }

    fn poll_widget_trigger_event(&self) -> Option<WidgetTriggerEvent> {
        self.state.pop_widget_event()
    }

    /// Removes the oldest queued trigger belonging to `widget_id`, leaving the rest queued.
    ///
    /// See [`crate::drain_widget_triggers_for`] for why a targeted pop has to exist beside the FIFO
    /// one: the queue is process-wide, and a consumer that knows which widget it is driving must be
    /// able to take its own events without dispatching -- or discarding -- another consumer's.
    fn pop_widget_trigger_event_for(&self, widget_id: ObjectId) -> Option<WidgetTriggerEvent> {
        self.state.pop_widget_event_for(widget_id)
    }

    fn inject_widget_trigger_event(&self, widget_id: ObjectId, kind: WidgetTriggerKind) -> bool {
        // Accept only known widget ids to keep queue semantics deterministic.
        if !self.state.contains_widget(widget_id) {
            return false;
        }
        self.state.push_widget_event(WidgetTriggerEvent { widget_id, kind });
        true
    }

    /// The window's current client size, as last reported by the host.
    ///
    /// Falls back to the size the window was created with. `None` for an id this backend
    /// does not know, so a caller can tell "no such window" from "a size I can use".
    fn window_client_size(&self, window_id: ObjectId) -> Option<(u32, u32)> {
        // Ask the control backend, which owns the window and is therefore the only
        // store that knows the size a resize reported.
        crate::window_client_size(window_id).or_else(|| self.state.window_size(window_id))
    }

    /// Reports a container's new client size and queues a `Resized` trigger.
    fn queue_resize_trigger(&self, window_id: ObjectId, width: u32, height: u32) -> bool {
        // Forward to the control backend, which owns the window and the queue the app
        // polls. Writing to the platform's own state would land in a store the host
        // never reads, because `create_window` goes through the control backend.
        crate::queue_resize_trigger(window_id, width, height)
    }

    fn set_clipboard_text(&self, text: &str) -> bool {
        self.state.set_clipboard_text(text)
    }

    fn get_clipboard_text(&self) -> String {
        self.state.clipboard_text()
    }

    fn ime_bridge(&self) -> Option<&dyn crate::platform::ime::ImeBridge> {
        Some(&self.ime_bridge)
    }
}
