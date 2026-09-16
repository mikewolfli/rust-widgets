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

use crate::core::{ObjectId, Orientation, PlatformFamily};
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

/// Returns whether `widget_id` names a control that can carry tri-state mode.
///
/// Without the widget registry (the `mini` profile strips widgets entirely) no
/// control exists, so the answer is an honest `false` rather than a stub value
/// that would let a tri-state write through for a widget that cannot be painted.
#[cfg(not(alloc_frugal))]
fn widget_is_checkable(widget_id: ObjectId) -> bool {
    crate::widget::runtime::widget_is_checkable(widget_id)
}

/// See the `not(alloc_frugal)` definition: the alloc-frugal profile has no widget
/// layer, hence no checkable control.
#[cfg(alloc_frugal)]
fn widget_is_checkable(_widget_id: ObjectId) -> bool {
    false
}

impl Platform for StubPlatform {
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
        Err("stub platform has no print spooler".to_string())
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

    fn inject_widget_trigger_event(&self, widget_id: ObjectId, kind: WidgetTriggerKind) -> bool {
        // Accept only known widget ids to keep queue semantics deterministic.
        if !self.state.contains_widget(widget_id) {
            return false;
        }
        self.state.push_widget_event(WidgetTriggerEvent { widget_id, kind });
        true
    }

    fn show_widget(&self, widget_id: ObjectId) {
        self.state.set_visible(widget_id, true);
    }

    fn hide_widget(&self, widget_id: ObjectId) {
        self.state.set_visible(widget_id, false);
    }

    fn set_widget_geometry(&self, widget_id: ObjectId, x: i32, y: i32, width: u32, height: u32) {
        self.state.set_geometry(widget_id, x, y, width, height);
    }

    fn set_widget_text(&self, widget_id: ObjectId, text: &str) {
        self.state.set_text(widget_id, text);
    }

    fn get_widget_text(&self, widget_id: ObjectId) -> String {
        self.state.text(widget_id)
    }

    fn set_widget_enabled(&self, widget_id: ObjectId, enabled: bool) {
        self.state.set_enabled(widget_id, enabled);
    }

    fn is_widget_enabled(&self, widget_id: ObjectId) -> bool {
        self.state.enabled(widget_id)
    }

    fn set_widget_visible(&self, widget_id: ObjectId, visible: bool) {
        self.state.set_visible(widget_id, visible);
    }

    fn is_widget_visible(&self, widget_id: ObjectId) -> bool {
        self.state.visible(widget_id)
    }

    fn set_widget_value(&self, widget_id: ObjectId, value: f64) -> bool {
        // A record holds a numeric value only if its creator seeded one, and each
        // self-drawn widget seeds exactly the properties its control has. That is
        // the natural per-control answer — a slider accepts a value, a button
        // does not — without any global classification table.
        if !self.state.contains_widget(widget_id) {
            return false;
        }
        self.state.set_value(widget_id, value)
    }

    fn widget_value(&self, widget_id: ObjectId) -> Option<f64> {
        self.state.value(widget_id)
    }

    fn set_widget_range(&self, widget_id: ObjectId, min: f64, max: f64) -> bool {
        if !self.state.contains_widget(widget_id) {
            return false;
        }
        self.state.set_range(widget_id, min, max)
    }

    fn widget_range(&self, widget_id: ObjectId) -> Option<(f64, f64)> {
        self.state.range(widget_id)
    }

    fn set_widget_selected_index(&self, widget_id: ObjectId, index: Option<usize>) -> bool {
        if !self.state.contains_widget(widget_id) {
            return false;
        }
        self.state.set_selected_index(widget_id, index)
    }

    fn widget_selected_index(&self, widget_id: ObjectId) -> Option<usize> {
        self.state.selected_index(widget_id)
    }

    fn set_widget_checked(&self, widget_id: ObjectId, checked: bool) -> bool {
        if !self.state.contains_widget(widget_id) {
            return false;
        }
        self.state.set_checked(widget_id, checked)
    }

    fn is_widget_checked(&self, widget_id: ObjectId) -> Option<bool> {
        self.state.checked(widget_id)
    }

    fn set_widget_step(&self, widget_id: ObjectId, step: f64) -> bool {
        if !self.state.contains_widget(widget_id) {
            return false;
        }
        self.state.set_step(widget_id, step)
    }

    fn widget_step(&self, widget_id: ObjectId) -> Option<f64> {
        self.state.step(widget_id)
    }

    fn set_widget_indeterminate(&self, widget_id: ObjectId, indeterminate: bool) -> bool {
        if !self.state.contains_widget(widget_id) {
            return false;
        }
        self.state.set_indeterminate(widget_id, indeterminate)
    }

    fn is_widget_indeterminate(&self, widget_id: ObjectId) -> Option<bool> {
        self.state.indeterminate(widget_id)
    }

    fn set_widget_read_only(&self, widget_id: ObjectId, read_only: bool) -> bool {
        if !self.state.contains_widget(widget_id) {
            return false;
        }
        self.state.set_read_only(widget_id, read_only)
    }

    fn is_widget_read_only(&self, widget_id: ObjectId) -> Option<bool> {
        self.state.read_only(widget_id)
    }

    fn set_widget_max_length(&self, widget_id: ObjectId, max_length: u32) -> bool {
        if !self.state.contains_widget(widget_id) {
            return false;
        }
        self.state.set_max_length(widget_id, max_length)
    }

    fn widget_max_length(&self, widget_id: ObjectId) -> Option<u32> {
        self.state.max_length(widget_id)
    }

    fn set_window_state(&self, widget_id: ObjectId, flag: WindowStateFlag, on: bool) -> bool {
        // Only a window has window state; the state record is `None` for every
        // other widget, so a control honestly reports refusal here.
        self.state.set_window_state(widget_id, flag, on)
    }

    fn is_window_in_state(&self, widget_id: ObjectId, flag: WindowStateFlag) -> Option<bool> {
        self.state.window_state(widget_id, flag)
    }

    fn set_window_min_size(&self, widget_id: ObjectId, width: u32, height: u32) -> bool {
        self.state.set_window_min_size(widget_id, width, height)
    }

    fn window_min_size(&self, widget_id: ObjectId) -> Option<(u32, u32)> {
        self.state.window_min_size(widget_id)
    }

    fn set_window_icon(&self, widget_id: ObjectId, path: &str) -> bool {
        self.state.set_window_icon(widget_id, path)
    }

    fn window_icon(&self, widget_id: ObjectId) -> Option<String> {
        self.state.window_icon(widget_id)
    }

    fn set_widget_selection(&self, widget_id: ObjectId, start: u32, end: u32) -> bool {
        self.state.set_selection(widget_id, start, end)
    }

    fn widget_selection(&self, widget_id: ObjectId) -> Option<(u32, u32)> {
        self.state.selection(widget_id)
    }

    fn set_widget_placeholder(&self, widget_id: ObjectId, text: &str) -> bool {
        self.state.set_placeholder(widget_id, text)
    }

    fn widget_placeholder(&self, widget_id: ObjectId) -> Option<String> {
        self.state.placeholder(widget_id)
    }

    fn set_widget_echo_mode(&self, widget_id: ObjectId, mode: EchoMode) -> bool {
        self.state.set_echo_mode(widget_id, mode)
    }

    fn widget_echo_mode(&self, widget_id: ObjectId) -> Option<EchoMode> {
        self.state.echo_mode(widget_id)
    }

    fn set_slider_orientation(&self, widget_id: ObjectId, orientation: Orientation) -> bool {
        if !self.state.contains_widget(widget_id) {
            return false;
        }
        self.state.set_orientation(widget_id, orientation)
    }

    fn slider_orientation(&self, widget_id: ObjectId) -> Option<Orientation> {
        self.state.orientation(widget_id)
    }

    /// Enables tri-state mode on a *checkable* control.
    ///
    /// Whether a widget can be tri-state is a property of the **widget**, not of
    /// the host, so the answer comes from the widget layer's own property table
    /// instead of a second list of kinds kept here. Before this delegation the
    /// stub answered from a local kind table, which is exactly the duplicated
    /// semantics BLUE15 removes: two places had to agree on which controls are
    /// checkable, and they eventually would not.
    ///
    /// Under `mini` there is no widget registry at all (widgets are stripped), so
    /// no control exists that could carry tri-state and the request is refused
    /// without reaching for a module that is not compiled in.
    fn set_widget_tristate(&self, widget_id: ObjectId, enabled: bool) -> bool {
        if !self.state.contains_widget(widget_id) {
            return false;
        }
        if !widget_is_checkable(widget_id) {
            return false;
        }
        self.state.set_tristate(widget_id, enabled)
    }

    fn is_widget_tristate(&self, widget_id: ObjectId) -> Option<bool> {
        // Reads must agree with the write gate above: a label never has tri-state
        // mode, so asking for it answers `None` rather than a stored `false` that
        // would imply the question was meaningful.
        if !widget_is_checkable(widget_id) {
            return None;
        }
        self.state.tristate(widget_id)
    }

    fn set_widget_group(&self, widget_id: ObjectId, group: &str) -> bool {
        if !self.state.contains_widget(widget_id) {
            return false;
        }
        self.state.set_group(widget_id, group)
    }

    fn widget_group(&self, widget_id: ObjectId) -> Option<String> {
        self.state.group(widget_id)
    }

    fn set_widget_scroll_position(&self, widget_id: ObjectId, x: i32, y: i32) -> bool {
        if !self.state.contains_widget(widget_id) {
            return false;
        }
        self.state.set_scroll(widget_id, x, y)
    }

    fn widget_scroll_position(&self, widget_id: ObjectId) -> Option<(i32, i32)> {
        self.state.scroll(widget_id)
    }

    fn set_widget_ime_enabled(&self, widget_id: ObjectId, enabled: bool) -> bool {
        self.state.set_ime_enabled(widget_id, enabled)
    }

    fn is_widget_ime_enabled(&self, widget_id: ObjectId) -> bool {
        self.state.ime_enabled(widget_id)
    }

    fn set_widget_accessibility_name(&self, widget_id: ObjectId, name: &str) -> bool {
        self.state.set_accessibility_name(widget_id, name)
    }

    fn get_widget_accessibility_name(&self, widget_id: ObjectId) -> String {
        self.state.accessibility_name(widget_id)
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

    fn begin_drag(&self, source_widget_id: ObjectId, mime: &str, payload: &[u8]) -> bool {
        self.state.begin_drag(source_widget_id, mime, payload)
    }

    fn poll_drop_event(&self) -> Option<DropEvent> {
        self.state.pop_drop_event()
    }

    fn inject_drop_event(&self, event: DropEvent) -> bool {
        self.state.inject_drop_event(event)
    }
}
