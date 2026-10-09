// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Shared backend state model used by platform adapters.
use super::{DropEvent, EchoMode, WidgetTriggerEvent, WidgetTriggerKind, WindowStateFlag};
use crate::compat::lock;
use crate::compat::HashMap;
use crate::compat::Mutex;
use crate::compat::String;
use crate::compat::ToString;
// `Vec` is used only by `serialize_widget_snapshot`, which is itself behind
// `feature = "serde_json"`. Importing it through the `compat` bridge (rather than
// relying on the prelude) is what keeps this file compiling under `mini`, where
// `#![no_std]` removes the prelude entirely.
#[cfg(feature = "serde_json")]
use crate::compat::Vec;
use crate::core::ObjectId;
use crate::core::Orientation;
use crate::core::Rect;
use alloc::collections::VecDeque;
use core::hash::Hash;
use core::sync::atomic::{AtomicU64, Ordering};
/// Generic widget state record owned by backend state model.
#[cfg(all(feature = "serde", widgets_unstripped))]
use serde::{Deserialize, Serialize};
/// The platform-neutral state a backend keeps for one widget handle.
///
/// `K` is the backend's own handle-kind discriminator; the record itself is
/// written by the shared accessors in the stub backend and by any OS
/// adapter, so the accessors read back what they were told to set rather than
/// querying an OS control. Optional fields follow the convention described on
/// each: `None` means "this control has no such concept here", never "zero".
#[derive(Clone, Debug)]
#[cfg_attr(all(feature = "serde", widgets_unstripped), derive(Serialize, Deserialize))]
pub struct WidgetRecord<K> {
    /// Backend-specific widget kind discriminator.
    pub kind: K,
    /// Widget text/content payload.
    pub text: String,
    /// Visibility state.
    pub visible: bool,
    /// Enabled/disabled state.
    pub enabled: bool,
    /// IME enabled state.
    pub ime_enabled: bool,
    /// Accessibility label.
    pub accessibility_name: String,
    /// Geometry origin x.
    pub x: i32,
    /// Geometry origin y.
    pub y: i32,
    /// Geometry width.
    pub width: u32,
    /// Geometry height.
    pub height: u32,
    /// Primary numeric value for value-carrying controls (slider, progress bar,
    /// spin box, scroll bar, dial). `None` means "this widget has no numeric
    /// value", which is distinct from `Some(0.0)` — a backend must not invent a
    /// value for a control that has none (principle #37).
    pub value: Option<f64>,
    /// `(min, max)` range of [`WidgetRecord::value`], when the control has one.
    pub range: Option<(f64, f64)>,
    /// Current selection index for selection-model controls (combo box, list
    /// box, tab widget). `None` means "nothing selected" or "no selection
    /// model".
    pub selected_index: Option<usize>,
    /// Checked state for checkable controls (check box, radio button, toggle
    /// button). `None` means the control is not checkable on this backend.
    pub checked: Option<bool>,
    /// Increment step for value controls with a settable stride (slider, spin
    /// box, scroll bar). `None` means the control has no settable step here.
    pub step: Option<f64>,
    /// Indeterminate (busy) state for progress-style controls. `None` means the
    /// control has no indeterminate mode here.
    pub indeterminate: Option<bool>,
    /// Read-only state for text-entry controls. `None` means the control is not
    /// a text entry here.
    pub read_only: Option<bool>,
    /// Maximum accepted character count for text-entry controls. `None` means the
    /// control has no settable limit here.
    pub max_length: Option<u32>,
    /// Window state flags for window handles (maximised, minimised, full-screen,
    /// resizable, decorated). `None` means the widget is not a window on this
    /// backend, which is what keeps `is_window_in_state` honest for controls.
    pub window_state: Option<WindowStateRecord>,
    /// Text-entry selection range as `(start, end)` character offsets. `None`
    /// means nothing is selected (or the control has no selectable text).
    pub selection: Option<(u32, u32)>,
    /// Placeholder (cue) text for a text entry. `None` until one is set.
    pub placeholder: Option<String>,
    /// Echo mode for a text entry, when the backend can apply one.
    pub echo_mode: Option<EchoMode>,
    /// Tri-state mode for a checkable control. `None` means the control is not
    /// checkable, or this backend has no tri-state support.
    pub tristate: Option<bool>,
    /// Mutually-exclusive group name for a radio button.
    pub group: Option<String>,
    /// Scroll offset of a scrollable container, as `(x, y)` virtual pixels.
    pub scroll: Option<(i32, i32)>,
    /// Slider orientation chosen at creation time, stored as "horizontal".
    ///
    /// Kept as a `bool` rather than `core::Orientation` so this serde-derived
    /// record does not force a serde derive onto a core geometry type (the
    /// `BackendState` snapshot is serialized, and widening `core`'s feature
    /// surface for one field is not worth it).
    pub horizontal: Option<bool>,
}

/// The togglable states of a window, as stored by a backend.
///
/// Captured as a small record instead of five parallel `Option<bool>` fields so
/// the whole window state travels together and cannot drift out of sync. It also
/// carries the two non-boolean window attributes (minimum size, icon path), which
/// share the "is this a window?" gate that `WidgetRecord::window_state` provides.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(all(feature = "serde", widgets_unstripped), derive(Serialize, Deserialize))]
pub struct WindowStateRecord {
    /// Window is maximised rather than restored.
    pub maximized: bool,
    /// Window is minimised.
    pub minimized: bool,
    /// Window is full-screen.
    pub fullscreen: bool,
    /// Window is user-resizable.
    pub resizable: bool,
    /// Window is OS-decorated.
    pub decorated: bool,
    /// Minimum content size. `None` until the caller sets one, so a read can tell
    /// "no explicit constraint" from a real request.
    pub min_size: Option<(u32, u32)>,
    /// Icon path supplied to `set_window_icon`. `None` until set.
    pub icon: Option<String>,
}

impl WindowStateRecord {
    /// The state a freshly created OS window starts in: restored, windowed,
    /// resizable, decorated, with no explicit minimum size and no icon.
    ///
    /// This is the single source of truth for window defaults — every backend's
    /// `create_window` uses it, so a new field cannot be added in one place and
    /// silently left unset in another.
    pub fn new_window() -> Self {
        Self {
            maximized: false,
            minimized: false,
            fullscreen: false,
            resizable: true,
            decorated: true,
            min_size: None,
            icon: None,
        }
    }

    /// Read one flag.
    pub fn get(&self, flag: WindowStateFlag) -> bool {
        match flag {
            WindowStateFlag::Maximized => self.maximized,
            WindowStateFlag::Minimized => self.minimized,
            WindowStateFlag::Fullscreen => self.fullscreen,
            WindowStateFlag::Resizable => self.resizable,
            WindowStateFlag::Decorated => self.decorated,
        }
    }

    /// Write one flag.
    pub fn set(&mut self, flag: WindowStateFlag, on: bool) {
        match flag {
            WindowStateFlag::Maximized => self.maximized = on,
            WindowStateFlag::Minimized => self.minimized = on,
            WindowStateFlag::Fullscreen => self.fullscreen = on,
            WindowStateFlag::Resizable => self.resizable = on,
            WindowStateFlag::Decorated => self.decorated = on,
        }
    }
}

impl Default for WindowStateRecord {
    fn default() -> Self {
        Self::new_window()
    }
}
/// Converts a [`Rect`] to the four primitives the surface map stores.
///
/// See `BackendState::surfaces` for why the rect is not stored as a `Rect`.
fn rect_to_tuple(rect: Rect) -> (i32, i32, u32, u32) {
    (rect.x, rect.y, rect.width, rect.height)
}

/// Thread-safe state model split from native handle adapters.
#[cfg_attr(all(feature = "serde", widgets_unstripped), derive(Serialize, Deserialize))]
pub struct BackendState<K> {
    next_id: AtomicU64,
    widgets: Mutex<HashMap<ObjectId, WidgetRecord<K>>>,
    menu_events: Mutex<VecDeque<ObjectId>>,
    widget_events: Mutex<VecDeque<WidgetTriggerEvent>>,
    clipboard_text: Mutex<String>,
    drop_events: Mutex<VecDeque<DropEvent>>,
    /// Widgets this host is displaying, with the rect each surface covers.
    ///
    /// A state-only backend has no native object per control (every `WidgetKind` is
    /// painted by the library), so "is it displayed?" is exactly this map. The host
    /// still owns the pixels and pulls them via `widget::runtime::render_frame`.
    ///
    /// Stored as primitives rather than [`Rect`] because this struct is serialized by
    /// the iOS/Android backends, and `Rect` deliberately carries no serde derive
    /// (widening `core`'s feature surface for one field is not worth it).
    surfaces: Mutex<HashMap<ObjectId, (i32, i32, u32, u32)>>,
    /// Widgets whose pixels changed since the host last asked.
    ///
    /// A queue rather than a flag set: the host drains it in order, and coalescing on
    /// insert keeps a burst of invalidations from producing a burst of repaints.
    pending_repaints: Mutex<VecDeque<ObjectId>>,
}
impl<K> Default for BackendState<K>
where
    K: Copy + Eq + Hash,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<K> BackendState<K>
where
    K: Copy + Eq + Hash,
{
    #[cfg(feature = "serde_json")]
    /// Serialize widget text snapshots without exposing synchronization primitives or id counters.
    pub fn serialize_widget_snapshot(&self) -> Result<String, serde_json::Error> {
        let texts: Vec<String> =
            lock(&self.widgets).values().map(|record| record.text.clone()).collect();
        serde_json::to_string(&texts)
    }

    /// Create empty backend state.
    pub fn new() -> Self {
        Self {
            next_id: AtomicU64::new(1),
            widgets: Mutex::new(HashMap::new()),
            menu_events: Mutex::new(VecDeque::new()),
            widget_events: Mutex::new(VecDeque::new()),
            clipboard_text: Mutex::new(String::new()),
            drop_events: Mutex::new(VecDeque::new()),
            surfaces: Mutex::new(HashMap::new()),
            pending_repaints: Mutex::new(VecDeque::new()),
        }
    }

    // -----------------------------------------------------------------------
    // Widget surfaces
    // -----------------------------------------------------------------------

    /// Records that `id` is displayed on a host surface covering `rect`.
    ///
    /// The host owns the pixels; this records only *which* widgets the host agreed
    /// to display and where. `mount_surface` returns this, and
    /// [`BackendState::invalidate_surface_record`] queues a repaint the host drains
    /// through [`BackendState::take_pending_repaint`].
    ///
    /// Refuses an unknown `id`: a surface for a widget that does not exist could only
    /// produce a frame nobody reads, so reporting `false` is the honest answer.
    pub fn mount_surface_record(&self, id: ObjectId, rect: Rect) -> bool {
        // `widgets` first (matching `destroy_widget`), then `surfaces`, so the check
        // cannot be separated from the insert by a concurrent destroy (D08-P-03).
        let widgets = lock(&self.widgets);
        if !widgets.contains_key(&id) {
            return false;
        }
        let mut surfaces = lock(&self.surfaces);
        surfaces.insert(id, rect_to_tuple(rect));
        drop(surfaces);
        drop(widgets);
        true
    }

    /// Forgets the surface for `id`. Returns whether one was recorded.
    pub fn unmount_surface_record(&self, id: ObjectId) -> bool {
        let removed = lock(&self.surfaces).remove(&id).is_some();
        if removed {
            // A repaint request for a widget that is gone would make the host ask for
            // a frame that can never be produced.
            lock(&self.pending_repaints).retain(|&pending| pending != id);
        }
        removed
    }

    /// Updates the rect of a mounted surface. Returns whether `id` was mounted.
    pub fn resize_surface_record(&self, id: ObjectId, rect: Rect) -> bool {
        let mut surfaces = lock(&self.surfaces);
        match surfaces.get_mut(&id) {
            Some(existing) => {
                *existing = rect_to_tuple(rect);
                true
            }
            None => false,
        }
    }

    /// Returns the rect of a mounted surface, or `None` when `id` is not mounted.
    pub fn surface_rect(&self, id: ObjectId) -> Option<Rect> {
        lock(&self.surfaces).get(&id).map(|&(x, y, width, height)| Rect::new(x, y, width, height))
    }

    /// Queues a repaint for `id` and reports whether it was mounted.
    ///
    /// The queue is drained by the host, which is the only party that can put the
    /// pixels on screen. Coalesced: a widget already awaiting a repaint is not queued
    /// again, so a burst of invalidations in one frame produces one repaint.
    pub fn invalidate_surface_record(&self, id: ObjectId) -> bool {
        // Hold `widgets` across the surface check and the repaint enqueue so a
        // concurrent destroy cannot clear `pending_repaints` between them and leave a
        // repaint queued for a widget that is gone (D08-P-03).
        let widgets = lock(&self.widgets);
        if !widgets.contains_key(&id) {
            return false;
        }
        if !lock(&self.surfaces).contains_key(&id) {
            return false;
        }
        let mut pending = lock(&self.pending_repaints);
        if !pending.contains(&id) {
            pending.push_back(id);
        }
        drop(pending);
        drop(widgets);
        true
    }

    /// Removes and returns the next widget awaiting a repaint.
    pub fn take_pending_repaint(&self) -> Option<ObjectId> {
        lock(&self.pending_repaints).pop_front()
    }

    /// Records a repaint request for any widget this backend knows, including a
    /// container.
    ///
    /// # Why this is separate from [`Self::invalidate_surface_record`]
    ///
    /// That method answers about a **mounted surface** and returns `false` for an
    /// ordinary control, which is right when the caller is asking "will my
    /// mounted surface repaint?". It is the wrong test for
    /// [`Platform::invalidate_surface`](crate::platform::Platform::invalidate_surface) —
    /// the call the library makes when it wants a widget redrawn — because a
    /// widget the backend knows but never mounted is exactly the window case: the
    /// library repaints windows to reveal their children (see
    /// `widget::runtime::request_repaint_subtree`).
    ///
    /// So this records the request for every id in the registry and reports
    /// `false` only for an id that addresses nothing, which is the one case where
    /// no repaint can be truthful.
    ///
    /// # Why an id in the registry is the right test, and `entry` is not
    ///
    /// `crate::app` registers a window's *widget* id and associates it with the
    /// **platform** id `create_window` returned, while both live in this registry.
    /// A request naming the widget id — which is what the library holds — only
    /// becomes a platform id through that association, so a host that wants to
    /// present the frame should resolve it with `crate::app::window_handle_for`
    /// (or read [`Self::surface_rect`] for a widget mounted directly).
    pub fn record_repaint_request(&self, id: ObjectId) -> bool {
        // Atomic with the existence check (D08-P-03), same lock order as destroy.
        let widgets = lock(&self.widgets);
        if !widgets.contains_key(&id) {
            return false;
        }
        let mut pending = lock(&self.pending_repaints);
        if !pending.contains(&id) {
            pending.push_back(id);
        }
        drop(pending);
        drop(widgets);
        true
    }

    /// Returns how many widgets are awaiting a repaint.
    pub fn pending_repaint_count(&self) -> usize {
        lock(&self.pending_repaints).len()
    }

    /// Returns how many surfaces this host is displaying.
    pub fn mounted_surface_count(&self) -> usize {
        lock(&self.surfaces).len()
    }
    /// Insert one widget record and return allocated logical id.
    pub fn create_widget(
        &self,
        kind: K,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.insert_widget(id, kind, text, x, y, width, height);
        id
    }

    /// Insert one widget record under a **caller-chosen** id.
    ///
    /// Used by self-drawn mounts: the id originates in
    /// `widget::runtime`, which owns the widget, so the backend state
    /// has to adopt it rather than allocate its own. Also advances the internal
    /// allocator past `id` so a later `create_widget` cannot collide with it.
    pub fn register_widget_with_id(
        &self,
        id: ObjectId,
        kind: K,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) {
        self.insert_widget(id, kind, text, x, y, width, height);
        // Keep the allocator ahead of any externally supplied id. `id + 1` is not
        // unconditionally representable: `ObjectId` is `u64` and a caller may hand in
        // `u64::MAX`, whose successor does not exist. In that case there is nothing to
        // advance *to*, so the allocator is left alone — which is correct, because every
        // id it could hand out is already `<= u64::MAX` and cannot collide with the one
        // just registered without exhausting the space.
        let Some(advanced) = id.checked_add(1) else {
            return;
        };
        let mut next = self.next_id.load(Ordering::Relaxed);
        while next <= id {
            match self.next_id.compare_exchange(
                next,
                advanced,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(current) => next = current,
            }
        }
    }

    /// Shared insert used by both creation paths.
    fn insert_widget(
        &self,
        id: ObjectId,
        kind: K,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) {
        lock(&self.widgets).insert(
            id,
            WidgetRecord {
                kind,
                text: text.to_string(),
                visible: true,
                enabled: true,
                ime_enabled: true,
                accessibility_name: text.to_string(),
                x,
                y,
                width,
                height,
                value: None,
                range: None,
                selected_index: None,
                checked: None,
                step: None,
                indeterminate: None,
                read_only: None,
                max_length: None,
                window_state: None,
                selection: None,
                placeholder: None,
                echo_mode: None,
                tristate: None,
                group: None,
                scroll: None,
                horizontal: None,
            },
        );
    }
    /// Return `true` when widget exists.
    pub fn contains_widget(&self, widget_id: ObjectId) -> bool {
        lock(&self.widgets).contains_key(&widget_id)
    }

    /// Remove a widget record **and every other piece of state keyed by its id**, returning `true`
    /// when the widget record existed.
    ///
    /// This is the state-side half of widget teardown. Without it a backend's
    /// registry could only ever grow: a long-running app that rebuilds its UI
    /// (create/discard cycles) would leak one record — plus whatever native
    /// object the backend stored — per discarded widget, forever.
    ///
    /// # Why the other containers are cleared too
    ///
    /// The id-keyed state does not live only in `widgets`. A widget may also have an injected trigger
    /// waiting in `widget_events`, a mounted surface in `surfaces`, a queued repaint in
    /// `pending_repaints`, a queued menu trigger, and drop events naming it. Removing only the
    /// `widgets` entry left all of those behind: the next `poll` could return an event for an id that
    /// no longer exists, a reused id could inherit a stale surface rect or a queued repaint, and a
    /// surface never got a matching `unmount`. Clearing every id-keyed container here is what makes
    /// destroy a single complete operation rather than a partial one a caller must remember to finish
    /// (rule #13).
    pub fn destroy_widget(&self, widget_id: ObjectId) -> bool {
        let removed = lock(&self.widgets).remove(&widget_id).is_some();
        // Id-keyed queues and records: drop everything that names this widget, whether or not the
        // widget record itself was still present, so a partially-cleaned prior destroy still heals.
        lock(&self.widget_events).retain(|event| event.widget_id != widget_id);
        lock(&self.menu_events).retain(|queued| *queued != widget_id);
        lock(&self.surfaces).remove(&widget_id);
        lock(&self.pending_repaints).retain(|queued| *queued != widget_id);
        lock(&self.drop_events).retain(|event| {
            event.source_widget_id != widget_id && event.target_widget_id != widget_id
        });
        removed
    }

    /// Number of live widget records. Used by tests and diagnostics to prove
    /// that teardown actually releases state.
    pub fn widget_count(&self) -> usize {
        lock(&self.widgets).len()
    }
    /// Return kind for an existing widget.
    pub fn kind_of(&self, widget_id: ObjectId) -> Option<K> {
        lock(&self.widgets).get(&widget_id).map(|widget| widget.kind)
    }
    /// Return `true` when widget exists and kind matches.
    pub fn is_kind(&self, widget_id: ObjectId, kind: K) -> bool {
        self.kind_of(widget_id).map(|k| k == kind).unwrap_or(false)
    }
    /// Set visibility for a widget.
    pub fn set_visible(&self, widget_id: ObjectId, visible: bool) {
        if let Some(widget) = lock(&self.widgets).get_mut(&widget_id) {
            widget.visible = visible;
        }
    }
    /// Return visibility for a widget.
    pub fn visible(&self, widget_id: ObjectId) -> bool {
        lock(&self.widgets).get(&widget_id).map(|widget| widget.visible).unwrap_or(false)
    }
    /// Set enabled state for a widget.
    pub fn set_enabled(&self, widget_id: ObjectId, enabled: bool) {
        if let Some(widget) = lock(&self.widgets).get_mut(&widget_id) {
            widget.enabled = enabled;
        }
    }
    /// Return enabled state for a widget.
    pub fn enabled(&self, widget_id: ObjectId) -> bool {
        lock(&self.widgets).get(&widget_id).map(|widget| widget.enabled).unwrap_or(false)
    }
    /// Set geometry for a widget.
    pub fn set_geometry(&self, widget_id: ObjectId, x: i32, y: i32, width: u32, height: u32) {
        if let Some(widget) = lock(&self.widgets).get_mut(&widget_id) {
            widget.x = x;
            widget.y = y;
            widget.width = width;
            widget.height = height;
        }
    }
    /// Set text for a widget.
    pub fn set_text(&self, widget_id: ObjectId, text: &str) -> bool {
        if let Some(widget) = lock(&self.widgets).get_mut(&widget_id) {
            widget.text = text.to_string();
            return true;
        }
        false
    }
    /// Return text for a widget.
    pub fn text(&self, widget_id: ObjectId) -> String {
        lock(&self.widgets).get(&widget_id).map(|widget| widget.text.clone()).unwrap_or_default()
    }
    /// Set IME enabled state for a widget.
    pub fn set_ime_enabled(&self, widget_id: ObjectId, enabled: bool) -> bool {
        if let Some(widget) = lock(&self.widgets).get_mut(&widget_id) {
            widget.ime_enabled = enabled;
            return true;
        }
        false
    }
    /// Return IME enabled state for a widget.
    pub fn ime_enabled(&self, widget_id: ObjectId) -> bool {
        lock(&self.widgets).get(&widget_id).map(|widget| widget.ime_enabled).unwrap_or(false)
    }
    /// Set accessibility label for a widget.
    pub fn set_accessibility_name(&self, widget_id: ObjectId, name: &str) -> bool {
        if let Some(widget) = lock(&self.widgets).get_mut(&widget_id) {
            widget.accessibility_name = name.to_string();
            return true;
        }
        false
    }
    /// Return accessibility label for a widget.
    pub fn accessibility_name(&self, widget_id: ObjectId) -> String {
        lock(&self.widgets)
            .get(&widget_id)
            .map(|widget| widget.accessibility_name.clone())
            .unwrap_or_default()
    }

    // ─── Uniform property storage ────────────────────────────────────────────
    //
    // These accessors back `Platform::{set_widget_value, set_widget_range,
    // set_widget_selected_index, set_widget_checked, ...}`. A backend whose
    // native control is the source of truth (macOS/Windows/GTK) writes through
    // to the control and *also* mirrors here, so off-main calls and teardown
    // still observe a consistent value. A state-only backend reads straight from
    // this record.
    //
    // A record only ever holds a property for a control that actually has one —
    // `None` means "this backend's control has no such property", which is why
    // the setters below are never called by a backend for an unsupported
    // control.

    /// Store a widget's numeric value, returning `false` for an unknown id.
    pub fn set_value(&self, widget_id: ObjectId, value: f64) -> bool {
        if let Some(widget) = lock(&self.widgets).get_mut(&widget_id) {
            widget.value = Some(value);
            return true;
        }
        false
    }
    /// Return a widget's numeric value, or `None` when it has none.
    pub fn value(&self, widget_id: ObjectId) -> Option<f64> {
        lock(&self.widgets).get(&widget_id).and_then(|widget| widget.value)
    }
    /// Store a widget's `(min, max)` range, returning `false` for an unknown id.
    pub fn set_range(&self, widget_id: ObjectId, min: f64, max: f64) -> bool {
        if let Some(widget) = lock(&self.widgets).get_mut(&widget_id) {
            widget.range = Some((min, max));
            // Keep the stored value inside the new range, mirroring what every
            // native control does when its range shrinks under the current value.
            if let Some(value) = widget.value {
                widget.value = Some(value.clamp(min.min(max), max.max(min)));
            }
            return true;
        }
        false
    }
    /// Return a widget's `(min, max)` range, or `None` when it has none.
    pub fn range(&self, widget_id: ObjectId) -> Option<(f64, f64)> {
        lock(&self.widgets).get(&widget_id).and_then(|widget| widget.range)
    }
    /// Store a widget's selection index, returning `false` for an unknown id.
    pub fn set_selected_index(&self, widget_id: ObjectId, index: Option<usize>) -> bool {
        if let Some(widget) = lock(&self.widgets).get_mut(&widget_id) {
            widget.selected_index = index;
            return true;
        }
        false
    }
    /// Return a widget's selection index, or `None` when nothing is selected.
    pub fn selected_index(&self, widget_id: ObjectId) -> Option<usize> {
        lock(&self.widgets).get(&widget_id).and_then(|widget| widget.selected_index)
    }
    /// Store a widget's checked state, returning `false` for an unknown id.
    pub fn set_checked(&self, widget_id: ObjectId, checked: bool) -> bool {
        if let Some(widget) = lock(&self.widgets).get_mut(&widget_id) {
            widget.checked = Some(checked);
            return true;
        }
        false
    }
    /// Return a widget's checked state, or `None` when it is not checkable.
    pub fn checked(&self, widget_id: ObjectId) -> Option<bool> {
        lock(&self.widgets).get(&widget_id).and_then(|widget| widget.checked)
    }

    /// Store a widget's increment step, returning `false` for an unknown id.
    pub fn set_step(&self, widget_id: ObjectId, step: f64) -> bool {
        if let Some(widget) = lock(&self.widgets).get_mut(&widget_id) {
            widget.step = Some(step);
            return true;
        }
        false
    }
    /// Return a widget's increment step, or `None` when it has none.
    pub fn step(&self, widget_id: ObjectId) -> Option<f64> {
        lock(&self.widgets).get(&widget_id).and_then(|widget| widget.step)
    }
    /// Store a widget's indeterminate state, returning `false` for an unknown id.
    pub fn set_indeterminate(&self, widget_id: ObjectId, indeterminate: bool) -> bool {
        if let Some(widget) = lock(&self.widgets).get_mut(&widget_id) {
            widget.indeterminate = Some(indeterminate);
            return true;
        }
        false
    }
    /// Return a widget's indeterminate state, or `None` when it has none.
    pub fn indeterminate(&self, widget_id: ObjectId) -> Option<bool> {
        lock(&self.widgets).get(&widget_id).and_then(|widget| widget.indeterminate)
    }
    /// Store a widget's read-only state, returning `false` for an unknown id.
    pub fn set_read_only(&self, widget_id: ObjectId, read_only: bool) -> bool {
        if let Some(widget) = lock(&self.widgets).get_mut(&widget_id) {
            widget.read_only = Some(read_only);
            return true;
        }
        false
    }
    /// Return a widget's read-only state, or `None` when it has none.
    pub fn read_only(&self, widget_id: ObjectId) -> Option<bool> {
        lock(&self.widgets).get(&widget_id).and_then(|widget| widget.read_only)
    }
    /// Store a widget's maximum text length, returning `false` for an unknown id.
    pub fn set_max_length(&self, widget_id: ObjectId, max_length: u32) -> bool {
        if let Some(widget) = lock(&self.widgets).get_mut(&widget_id) {
            widget.max_length = Some(max_length);
            return true;
        }
        false
    }
    /// Return a widget's maximum text length, or `None` when it has none.
    pub fn max_length(&self, widget_id: ObjectId) -> Option<u32> {
        lock(&self.widgets).get(&widget_id).and_then(|widget| widget.max_length)
    }

    /// Mark a widget as a window and seed its initial window state.
    ///
    /// Called by a backend's `create_window`. Until this runs the widget has
    /// `window_state == None`, so `is_window_in_state` correctly reports "not a
    /// window" for every non-window control.
    pub fn init_window_state(&self, widget_id: ObjectId, initial: WindowStateRecord) -> bool {
        if let Some(widget) = lock(&self.widgets).get_mut(&widget_id) {
            widget.window_state = Some(initial);
            return true;
        }
        false
    }

    /// Store one window state flag, returning `false` when the id is not a window.
    pub fn set_window_state(&self, widget_id: ObjectId, flag: WindowStateFlag, on: bool) -> bool {
        if let Some(widget) = lock(&self.widgets).get_mut(&widget_id) {
            if let Some(state) = widget.window_state.as_mut() {
                state.set(flag, on);
                return true;
            }
        }
        false
    }

    /// Read one window state flag, or `None` when the id is not a window.
    pub fn window_state(&self, widget_id: ObjectId, flag: WindowStateFlag) -> Option<bool> {
        lock(&self.widgets)
            .get(&widget_id)
            .and_then(|widget| widget.window_state.as_ref())
            .map(|state| state.get(flag))
    }

    /// Store a window's minimum content size, returning `false` for a non-window.
    pub fn set_window_min_size(&self, widget_id: ObjectId, width: u32, height: u32) -> bool {
        if let Some(widget) = lock(&self.widgets).get_mut(&widget_id) {
            if let Some(state) = widget.window_state.as_mut() {
                state.min_size = Some((width, height));
                return true;
            }
        }
        false
    }

    /// Read a window's minimum content size, or `None` when it has none.
    pub fn window_min_size(&self, widget_id: ObjectId) -> Option<(u32, u32)> {
        lock(&self.widgets)
            .get(&widget_id)
            .and_then(|widget| widget.window_state.as_ref())
            .and_then(|state| state.min_size)
    }

    /// Store a window's icon path, returning `false` for a non-window.
    pub fn set_window_icon(&self, widget_id: ObjectId, path: &str) -> bool {
        if let Some(widget) = lock(&self.widgets).get_mut(&widget_id) {
            if let Some(state) = widget.window_state.as_mut() {
                state.icon = Some(path.to_string());
                return true;
            }
        }
        false
    }

    /// Read a window's icon path, or `None` when it has none.
    pub fn window_icon(&self, widget_id: ObjectId) -> Option<String> {
        lock(&self.widgets)
            .get(&widget_id)
            .and_then(|widget| widget.window_state.as_ref())
            .and_then(|state| state.icon.clone())
    }

    /// Store a text entry's selection range, returning `false` for unknown ids.
    pub fn set_selection(&self, widget_id: ObjectId, start: u32, end: u32) -> bool {
        if let Some(widget) = lock(&self.widgets).get_mut(&widget_id) {
            widget.selection = Some((start, end));
            return true;
        }
        false
    }
    /// Read a text entry's selection range, or `None` when nothing is selected.
    pub fn selection(&self, widget_id: ObjectId) -> Option<(u32, u32)> {
        lock(&self.widgets).get(&widget_id).and_then(|widget| widget.selection)
    }
    /// Store a text entry's placeholder text, returning `false` for unknown ids.
    pub fn set_placeholder(&self, widget_id: ObjectId, text: &str) -> bool {
        if let Some(widget) = lock(&self.widgets).get_mut(&widget_id) {
            widget.placeholder = Some(text.to_string());
            return true;
        }
        false
    }
    /// Read a text entry's placeholder text.
    pub fn placeholder(&self, widget_id: ObjectId) -> Option<String> {
        lock(&self.widgets).get(&widget_id).and_then(|widget| widget.placeholder.clone())
    }
    /// Store a text entry's echo mode, returning `false` for unknown ids.
    pub fn set_echo_mode(&self, widget_id: ObjectId, mode: EchoMode) -> bool {
        if let Some(widget) = lock(&self.widgets).get_mut(&widget_id) {
            widget.echo_mode = Some(mode);
            return true;
        }
        false
    }
    /// Read a text entry's echo mode.
    pub fn echo_mode(&self, widget_id: ObjectId) -> Option<EchoMode> {
        lock(&self.widgets).get(&widget_id).and_then(|widget| widget.echo_mode)
    }
    /// Store a checkable control's tri-state mode, `false` for unknown ids.
    pub fn set_tristate(&self, widget_id: ObjectId, enabled: bool) -> bool {
        if let Some(widget) = lock(&self.widgets).get_mut(&widget_id) {
            widget.tristate = Some(enabled);
            return true;
        }
        false
    }
    /// Read a checkable control's tri-state mode.
    pub fn tristate(&self, widget_id: ObjectId) -> Option<bool> {
        lock(&self.widgets).get(&widget_id).and_then(|widget| widget.tristate)
    }
    /// Store a radio button's group name, `false` for unknown ids.
    pub fn set_group(&self, widget_id: ObjectId, group: &str) -> bool {
        if let Some(widget) = lock(&self.widgets).get_mut(&widget_id) {
            widget.group = Some(group.to_string());
            return true;
        }
        false
    }
    /// Read a radio button's group name.
    pub fn group(&self, widget_id: ObjectId) -> Option<String> {
        lock(&self.widgets).get(&widget_id).and_then(|widget| widget.group.clone())
    }
    /// Store a scroll container's offset, `false` for unknown ids.
    pub fn set_scroll(&self, widget_id: ObjectId, x: i32, y: i32) -> bool {
        if let Some(widget) = lock(&self.widgets).get_mut(&widget_id) {
            widget.scroll = Some((x, y));
            return true;
        }
        false
    }
    /// Read a scroll container's offset.
    pub fn scroll(&self, widget_id: ObjectId) -> Option<(i32, i32)> {
        lock(&self.widgets).get(&widget_id).and_then(|widget| widget.scroll)
    }
    /// Store a slider's creation-time orientation, `false` for unknown ids.
    pub fn set_orientation(&self, widget_id: ObjectId, orientation: Orientation) -> bool {
        if let Some(widget) = lock(&self.widgets).get_mut(&widget_id) {
            widget.horizontal = Some(orientation == Orientation::Horizontal);
            return true;
        }
        false
    }
    /// Read a slider's creation-time orientation.
    pub fn orientation(&self, widget_id: ObjectId) -> Option<Orientation> {
        lock(&self.widgets).get(&widget_id).and_then(|widget| widget.horizontal).map(|horizontal| {
            if horizontal {
                Orientation::Horizontal
            } else {
                Orientation::Vertical
            }
        })
    }

    // ─── Backend event methods ─────────────────────────────────────────────────
    // These methods provide event system integration for menu and widget trigger
    // dispatch. They are called by the macos, mobile, and stub platform backends.

    /// Push menu trigger event.
    /// Reserved for menu system integration (not yet wired to platform backends).
    pub fn push_menu_event(&self, item_id: ObjectId) {
        lock(&self.menu_events).push_back(item_id);
    }
    /// Pop menu trigger event.
    /// Reserved for menu system integration (paired with push_menu_event).
    ///
    /// As with [`BackendState::pop_widget_event`], the atomic inject path is what keeps the
    /// queue honest; directly pushed entries are returned as-is.
    pub fn pop_menu_event(&self) -> Option<ObjectId> {
        lock(&self.menu_events).pop_front()
    }
    /// Push typed widget trigger event.
    /// Reserved for event system integration (not yet wired to platform backends).
    pub fn push_widget_event(&self, event: WidgetTriggerEvent) {
        lock(&self.widget_events).push_back(event);
    }
    /// Pop typed widget trigger event.
    ///
    /// The atomic [`BackendState::inject_widget_trigger_event`] already guarantees that a
    /// queued event's widget existed at enqueue time and could not be destroyed mid-enqueue,
    /// so the queue itself is the authority here. Events pushed directly by
    /// [`BackendState::push_widget_event`] (a public path used to seed state, e.g. in tests)
    /// are returned as-is rather than filtered against the widget registry.
    pub fn pop_widget_event(&self) -> Option<WidgetTriggerEvent> {
        lock(&self.widget_events).pop_front()
    }

    /// Remove and return the **oldest** queued event for `widget_id`, if there is one.
    ///
    /// # Why a targeted pop has to exist alongside the FIFO drain
    ///
    /// The queue is process-wide (see [`BackendState`]'s own documentation on the widget maps), so
    /// a drain that takes the front of it gets whatever any producer put there -- and a consumer that
    /// wants *its own* widget's events has no way to ask for them. In a single-threaded host that is
    /// invisible, because the only producer is the same host. Under a test runner with a shared thread
    /// pool it is not: several tests inject for their own widgets and each drains the front of one
    /// common queue, so a test can dispatch dozens of other tests' events and never reach its own.
    ///
    /// This returns the oldest event belonging to `widget_id` and **leaves the rest queued**, so a
    /// caller that knows which widget it is driving can consume exactly its own work without disturbing
    /// (or discarding) anyone else's. Ordering among a single widget's own events is preserved.
    pub fn pop_widget_event_for(&self, widget_id: ObjectId) -> Option<WidgetTriggerEvent> {
        let mut queue = lock(&self.widget_events);
        let at = queue.iter().position(|event| event.widget_id == widget_id)?;
        queue.remove(at)
    }
    /// Set clipboard text.
    pub fn set_clipboard_text(&self, text: &str) -> bool {
        *lock(&self.clipboard_text) = text.to_string();
        true
    }
    /// Get clipboard text.
    pub fn clipboard_text(&self) -> String {
        lock(&self.clipboard_text).clone()
    }
    /// Begin drag event for existing source widget.
    pub fn begin_drag(&self, source_widget_id: ObjectId, mime: &str, payload: &[u8]) -> bool {
        // Atomic with the existence check (D08-P-03), same lock order as destroy.
        let widgets = lock(&self.widgets);
        if !widgets.contains_key(&source_widget_id) {
            return false;
        }
        lock(&self.drop_events).push_back(DropEvent {
            source_widget_id,
            target_widget_id: 0, // Not yet known — target is determined at drop time
            mime: mime.to_string(),
            payload: payload.to_vec(),
        });
        drop(widgets);
        true
    }
    /// Pop one drop event.
    pub fn pop_drop_event(&self) -> Option<DropEvent> {
        lock(&self.drop_events).pop_front()
    }
    /// Inject drop event when target widget exists.
    ///
    /// Atomic with the existence check (D08-P-03).
    pub fn inject_drop_event(&self, event: DropEvent) -> bool {
        let widgets = lock(&self.widgets);
        if !widgets.contains_key(&event.target_widget_id) {
            return false;
        }
        lock(&self.drop_events).push_back(event);
        drop(widgets);
        true
    }

    // ─── Test/programmatic event injection ─────────────────────────────────────
    // These helpers are called by the macos, mobile, and stub platform backends
    // for event system bridge functions.

    /// Inject a typed widget trigger event, atomically with the existence check.
    ///
    /// # Why this is one operation rather than `contains_widget` + `push_widget_event`
    ///
    /// The two-lock form had a window between the existence check and the enqueue: a
    /// concurrent [`BackendState::destroy_widget`] could remove the widget (and clear its
    /// queues) after the check passed, and the push then re-added an event for an id that
    /// no longer exists — which `poll` would hand back (D08-P-03). Holding the `widgets`
    /// lock across the check-and-enqueue closes that window. The lock order
    /// (`widgets` → `widget_events`) matches [`BackendState::destroy_widget`], so the two
    /// cannot deadlock.
    pub fn inject_widget_trigger_event(
        &self,
        widget_id: ObjectId,
        kind: WidgetTriggerKind,
    ) -> bool {
        let widgets = lock(&self.widgets);
        if !widgets.contains_key(&widget_id) {
            return false;
        }
        // The widget cannot be destroyed while `widgets` is held, so this enqueue
        // cannot outlive the id it names.
        lock(&self.widget_events).push_back(WidgetTriggerEvent { widget_id, kind });
        drop(widgets);
        true
    }

    /// Inject a menu trigger event, atomically with the existence check.
    ///
    /// Same reasoning as [`BackendState::inject_widget_trigger_event`]: the check and the
    /// enqueue must not be separable by a concurrent destroy (D08-P-03).
    pub fn inject_menu_trigger(&self, menu_item_id: ObjectId) -> bool {
        let widgets = lock(&self.widgets);
        if !widgets.contains_key(&menu_item_id) {
            return false;
        }
        lock(&self.menu_events).push_back(menu_item_id);
        drop(widgets);
        true
    }
    /// Pop widget trigger event.
    /// Reserved for event processing in platform backends.
    pub fn pop_widget_trigger(&self) -> Option<ObjectId> {
        // Pop the widget_id from the widget_events queue (typed variant),
        // extracting just the widget_id. This is the widget-side counterpart
        // of push_widget_event / inject_widget_trigger_event.
        self.pop_widget_event().map(|e| e.widget_id)
    }
    /// Pop typed widget trigger event.
    /// Reserved for event processing in platform backends (typed variant).
    pub fn pop_widget_trigger_event(&self) -> Option<WidgetTriggerEvent> {
        self.pop_widget_event()
    }

    /// The size `window_id` was created with, if it exists.
    ///
    /// Used as the fallback behind a backend's `Platform::window_client_size`: a window
    /// that has never been resized still has the dimensions it was created at, and
    /// reporting those is honest where inventing a number would not be (principle #37).
    /// The *reported* size lives with the control backend, which owns the window.
    pub fn window_size(&self, window_id: ObjectId) -> Option<(u32, u32)> {
        lock(&self.widgets).get(&window_id).map(|record| (record.width, record.height))
    }

    /// The full geometry `widget_id` was last given, if it exists.
    ///
    /// A backend that resizes a window by rewriting its record (rather than by keeping a
    /// separate client-size map) needs the origin to write back, because
    /// [`BackendState::set_geometry`] takes all four numbers: dropping the origin would
    /// move the window to `(0, 0)` as a side effect of resizing it.
    pub fn widget_geometry(&self, widget_id: ObjectId) -> Option<(i32, i32, u32, u32)> {
        lock(&self.widgets)
            .get(&widget_id)
            .map(|record| (record.x, record.y, record.width, record.height))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_kind_returns_true_for_matching_kind() {
        #[derive(Clone, Copy, PartialEq, Eq, Hash)]
        #[cfg_attr(all(feature = "serde", widgets_unstripped), derive(Serialize, Deserialize))]
        enum TestKind {
            Button,
            Label,
        }

        let state = BackendState::<TestKind>::new();
        let id1 = state.create_widget(TestKind::Button, "Click", 0, 0, 100, 30);
        let id2 = state.create_widget(TestKind::Label, "Name:", 0, 0, 50, 20);

        assert!(state.is_kind(id1, TestKind::Button));
        assert!(!state.is_kind(id1, TestKind::Label));
        assert!(state.is_kind(id2, TestKind::Label));
        assert!(!state.is_kind(id2, TestKind::Button));
    }

    #[test]
    fn is_kind_returns_false_for_nonexistent_widget() {
        #[derive(Clone, Copy, PartialEq, Eq, Hash)]
        #[cfg_attr(all(feature = "serde", widgets_unstripped), derive(Serialize, Deserialize))]
        enum TestKind {
            Widget,
        }

        let state = BackendState::<TestKind>::new();
        assert!(!state.is_kind(999, TestKind::Widget));
    }

    /// N-P-05: registering `u64::MAX` as an external id must not overflow the allocator.
    ///
    /// `ObjectId` is `u64`, and the self-drawn mount path adopts an id chosen by the caller, so
    /// `u64::MAX` is reachable. The old `compare_exchange(next, id + 1, …)` computed `u64::MAX + 1`,
    /// which aborts in a debug build and wraps `next_id` to `0` in a release one — after which every
    /// subsequent `create_widget` could hand out an id it had already seen. The fix leaves the
    /// allocator untouched when there is no successor to advance to.
    #[test]
    fn registering_the_max_id_does_not_overflow_the_allocator() {
        #[derive(Clone, Copy, PartialEq, Eq, Hash)]
        #[cfg_attr(all(feature = "serde", widgets_unstripped), derive(Serialize, Deserialize))]
        enum TestKind {
            Widget,
        }

        let state = BackendState::<TestKind>::new();
        // Debug builds panic on `u64::MAX + 1`, so reaching the next line is the assertion.
        state.register_widget_with_id(u64::MAX, TestKind::Widget, "x", 0, 0, 1, 1);
        assert!(state.is_kind(u64::MAX, TestKind::Widget));
        // A normal id still advances the allocator past itself.
        state.register_widget_with_id(5, TestKind::Widget, "y", 0, 0, 1, 1);
        let next = state.create_widget(TestKind::Widget, "z", 0, 0, 1, 1);
        assert!(next > 5, "the allocator must move past an externally supplied id, got {next}");
    }

    /// The targeted pop takes **one widget's** oldest event and leaves every other widget's queued.
    ///
    /// # The defect this pins
    ///
    /// The queue is process-wide and `pop_widget_event` takes its front unconditionally, so a consumer
    /// that wants its *own* widget's events has no way to ask for them -- it gets whatever any producer
    /// put there. In a single-threaded host that is invisible; under a test runner with a shared thread
    /// pool it is not, which is why `drive_frame_drains_input_before_advancing` was flaky: several tests
    /// inject for their own widgets and each frame drains the front of one common queue, so a test could
    /// dispatch dozens of other tests' events and never reach its own.
    ///
    /// This is the property that makes the targeted drain usable, and it is asserted here -- on the queue
    /// itself, with no frames and no threads -- because it cannot be asserted reliably through the frame
    /// loop: a concurrent drain of the same queue can always take an event first.
    #[test]
    fn the_targeted_pop_takes_only_its_own_widgets_oldest_event() {
        use crate::platform::types::WidgetTriggerKind;

        #[derive(Clone, Copy, PartialEq, Eq, Hash)]
        #[cfg_attr(all(feature = "serde", widgets_unstripped), derive(Serialize, Deserialize))]
        // This enum exists to *name* the backend's trigger kind for the generic instantiation
        // below; the variant is never constructed because the test only ever uses the type. That is
        // a marker type's whole purpose, so the dead-code lint is answered rather than the variant
        // removed (removing it would empty the enum).
        #[allow(dead_code)]
        enum TestKind {
            Widget,
        }
        let state = BackendState::<TestKind>::new();
        state.push_widget_event(WidgetTriggerEvent {
            widget_id: 1,
            kind: WidgetTriggerKind::ValueChanged,
        });
        state.push_widget_event(WidgetTriggerEvent {
            widget_id: 2,
            kind: WidgetTriggerKind::Clicked,
        });
        state.push_widget_event(WidgetTriggerEvent {
            widget_id: 1,
            kind: WidgetTriggerKind::Clicked,
        });

        // Widget 1's *oldest* event, not the front of the queue (which is also widget 1 here, so the
        // next assertion is the one that proves it skipped widget 2 rather than draining in order).
        let first = state.pop_widget_event_for(1).expect("widget 1 has a queued event");
        assert_eq!(first.kind, WidgetTriggerKind::ValueChanged, "the oldest comes first");
        // Widget 1's second event is now the one it gets; widget 2's was **not** consumed on the way.
        let second = state.pop_widget_event_for(1).expect("widget 1 has a second event");
        assert_eq!(second.kind, WidgetTriggerKind::Clicked);
        assert_eq!(state.pop_widget_event_for(1), None, "and then nothing of widget 1's is left");

        // Widget 2's event survived both pops: the targeted drain neither consumed nor discarded it.
        let other = state.pop_widget_event().expect("widget 2's event is still queued");
        assert_eq!(other.widget_id, 2, "and it is the one that was queued for widget 2");
        assert_eq!(other.kind, WidgetTriggerKind::Clicked);
        assert!(state.pop_widget_event().is_none(), "the queue is now empty");
    }

    /// D08-P-03: a destroy that races an inject must not leave a queued event for a
    /// widget that no longer exists.
    ///
    /// A `Barrier` fixes the interleaving: one thread blocks inside the inject after
    /// its existence check used to pass, and the other destroys the widget in that
    /// window. With the check and the enqueue now under one `widgets` lock, the destroy
    /// cannot slip between them, so the pop can never surface the dead id.
    #[test]
    fn destroy_racing_an_inject_leaves_no_event_for_a_dead_widget() {
        use crate::platform::types::WidgetTriggerKind;
        use std::sync::{Arc, Barrier};

        #[derive(Clone, Copy, PartialEq, Eq, Hash)]
        #[allow(dead_code)]
        enum TestKind {
            Widget,
        }

        // Run many rounds: the barrier makes the race deterministic when the fields are
        // not atomic, and the loop raises the chance of catching a regression.
        for _ in 0..200 {
            let state = Arc::new(BackendState::<TestKind>::new());
            let id = state.create_widget(TestKind::Widget, "probe", 0, 0, 10, 10);
            let barrier = Arc::new(Barrier::new(2));

            let inject_state = Arc::clone(&state);
            let inject_barrier = Arc::clone(&barrier);
            let injector = std::thread::spawn(move || {
                inject_barrier.wait();
                inject_state.inject_widget_trigger_event(id, WidgetTriggerKind::Clicked)
            });

            let destroy_state = Arc::clone(&state);
            let destroy_barrier = Arc::clone(&barrier);
            let destroyer = std::thread::spawn(move || {
                destroy_barrier.wait();
                destroy_state.destroy_widget(id)
            });

            let injected = injector.join().expect("inject thread must not panic");
            let _existed = destroyer.join().expect("destroy thread must not panic");

            // Whatever the interleaving, a pop may only return an event for a still-live
            // widget. After destroy the widget is gone, so the queue must be either empty
            // (the inject lost the race) or contain the event only if the inject ran first
            // AND the destroy then cleared it — i.e. always empty once destroy has completed.
            assert!(
                state.pop_widget_event().is_none(),
                "a destroyed widget must not leave a queued event (injected={injected})"
            );
        }
    }
}
