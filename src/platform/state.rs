// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Shared backend state model used by platform adapters.
use super::{DropEvent, WidgetTriggerEvent, WidgetTriggerKind};
use crate::compat::HashMap;
use crate::compat::Mutex;
use crate::core::ObjectId;
use alloc::collections::VecDeque;
use core::hash::Hash;
use core::sync::atomic::{AtomicU64, Ordering};
/// Generic widget state record owned by backend state model.
#[cfg(all(feature = "serde", not(any(feature = "mini", feature = "embedded"))))]
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug)]
#[cfg_attr(
    all(feature = "serde", not(any(feature = "mini", feature = "embedded"))),
    derive(Serialize, Deserialize)
)]
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
}
/// Thread-safe state model split from native handle adapters.
#[cfg_attr(
    all(feature = "serde", not(any(feature = "mini", feature = "embedded"))),
    derive(Serialize, Deserialize)
)]
pub struct BackendState<K> {
    next_id: AtomicU64,
    widgets: Mutex<HashMap<ObjectId, WidgetRecord<K>>>,
    menu_events: Mutex<VecDeque<ObjectId>>,
    widget_events: Mutex<VecDeque<WidgetTriggerEvent>>,
    clipboard_text: Mutex<String>,
    drop_events: Mutex<VecDeque<DropEvent>>,
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
        let texts: Vec<String> = self
            .widgets
            .lock()
            .expect("backend state widget lock poisoned")
            .values()
            .map(|record| record.text.clone())
            .collect();
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
        }
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
    /// [`crate::widget::runtime`], which owns the widget, so the backend state
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
        // Keep the allocator ahead of any externally supplied id.
        let mut next = self.next_id.load(Ordering::Relaxed);
        while next <= id {
            match self.next_id.compare_exchange(next, id + 1, Ordering::Relaxed, Ordering::Relaxed)
            {
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
        self.widgets.lock().expect("backend state widget lock poisoned").insert(
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
            },
        );
    }
    /// Return `true` when widget exists.
    pub fn contains_widget(&self, widget_id: ObjectId) -> bool {
        self.widgets.lock().expect("backend state widget lock poisoned").contains_key(&widget_id)
    }

    /// Remove a widget record, returning `true` when it existed.
    ///
    /// This is the state-side half of widget teardown. Without it a backend's
    /// registry could only ever grow: a long-running app that rebuilds its UI
    /// (create/discard cycles) would leak one record — plus whatever native
    /// object the backend stored — per discarded widget, forever.
    pub fn destroy_widget(&self, widget_id: ObjectId) -> bool {
        self.widgets
            .lock()
            .expect("backend state widget lock poisoned")
            .remove(&widget_id)
            .is_some()
    }

    /// Number of live widget records. Used by tests and diagnostics to prove
    /// that teardown actually releases state.
    pub fn widget_count(&self) -> usize {
        self.widgets.lock().expect("backend state widget lock poisoned").len()
    }
    /// Return kind for an existing widget.
    pub fn kind_of(&self, widget_id: ObjectId) -> Option<K> {
        self.widgets
            .lock()
            .expect("backend state widget lock poisoned")
            .get(&widget_id)
            .map(|widget| widget.kind)
    }
    /// Return `true` when widget exists and kind matches.
    pub fn is_kind(&self, widget_id: ObjectId, kind: K) -> bool {
        self.kind_of(widget_id).map(|k| k == kind).unwrap_or(false)
    }
    /// Set visibility for a widget.
    pub fn set_visible(&self, widget_id: ObjectId, visible: bool) {
        if let Some(widget) =
            self.widgets.lock().expect("backend state widget lock poisoned").get_mut(&widget_id)
        {
            widget.visible = visible;
        }
    }
    /// Return visibility for a widget.
    pub fn visible(&self, widget_id: ObjectId) -> bool {
        self.widgets
            .lock()
            .expect("backend state widget lock poisoned")
            .get(&widget_id)
            .map(|widget| widget.visible)
            .unwrap_or(false)
    }
    /// Set enabled state for a widget.
    pub fn set_enabled(&self, widget_id: ObjectId, enabled: bool) {
        if let Some(widget) =
            self.widgets.lock().expect("backend state widget lock poisoned").get_mut(&widget_id)
        {
            widget.enabled = enabled;
        }
    }
    /// Return enabled state for a widget.
    pub fn enabled(&self, widget_id: ObjectId) -> bool {
        self.widgets
            .lock()
            .expect("backend state widget lock poisoned")
            .get(&widget_id)
            .map(|widget| widget.enabled)
            .unwrap_or(false)
    }
    /// Set geometry for a widget.
    pub fn set_geometry(&self, widget_id: ObjectId, x: i32, y: i32, width: u32, height: u32) {
        if let Some(widget) =
            self.widgets.lock().expect("backend state widget lock poisoned").get_mut(&widget_id)
        {
            widget.x = x;
            widget.y = y;
            widget.width = width;
            widget.height = height;
        }
    }
    /// Set text for a widget.
    pub fn set_text(&self, widget_id: ObjectId, text: &str) -> bool {
        if let Some(widget) =
            self.widgets.lock().expect("backend state widget lock poisoned").get_mut(&widget_id)
        {
            widget.text = text.to_string();
            return true;
        }
        false
    }
    /// Return text for a widget.
    pub fn text(&self, widget_id: ObjectId) -> String {
        self.widgets
            .lock()
            .expect("backend state widget lock poisoned")
            .get(&widget_id)
            .map(|widget| widget.text.clone())
            .unwrap_or_default()
    }
    /// Set IME enabled state for a widget.
    pub fn set_ime_enabled(&self, widget_id: ObjectId, enabled: bool) -> bool {
        if let Some(widget) =
            self.widgets.lock().expect("backend state widget lock poisoned").get_mut(&widget_id)
        {
            widget.ime_enabled = enabled;
            return true;
        }
        false
    }
    /// Return IME enabled state for a widget.
    pub fn ime_enabled(&self, widget_id: ObjectId) -> bool {
        self.widgets
            .lock()
            .expect("backend state widget lock poisoned")
            .get(&widget_id)
            .map(|widget| widget.ime_enabled)
            .unwrap_or(false)
    }
    /// Set accessibility label for a widget.
    pub fn set_accessibility_name(&self, widget_id: ObjectId, name: &str) -> bool {
        if let Some(widget) =
            self.widgets.lock().expect("backend state widget lock poisoned").get_mut(&widget_id)
        {
            widget.accessibility_name = name.to_string();
            return true;
        }
        false
    }
    /// Return accessibility label for a widget.
    pub fn accessibility_name(&self, widget_id: ObjectId) -> String {
        self.widgets
            .lock()
            .expect("backend state widget lock poisoned")
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
        if let Some(widget) =
            self.widgets.lock().expect("backend state widget lock poisoned").get_mut(&widget_id)
        {
            widget.value = Some(value);
            return true;
        }
        false
    }
    /// Return a widget's numeric value, or `None` when it has none.
    pub fn value(&self, widget_id: ObjectId) -> Option<f64> {
        self.widgets
            .lock()
            .expect("backend state widget lock poisoned")
            .get(&widget_id)
            .and_then(|widget| widget.value)
    }
    /// Store a widget's `(min, max)` range, returning `false` for an unknown id.
    pub fn set_range(&self, widget_id: ObjectId, min: f64, max: f64) -> bool {
        if let Some(widget) =
            self.widgets.lock().expect("backend state widget lock poisoned").get_mut(&widget_id)
        {
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
        self.widgets
            .lock()
            .expect("backend state widget lock poisoned")
            .get(&widget_id)
            .and_then(|widget| widget.range)
    }
    /// Store a widget's selection index, returning `false` for an unknown id.
    pub fn set_selected_index(&self, widget_id: ObjectId, index: Option<usize>) -> bool {
        if let Some(widget) =
            self.widgets.lock().expect("backend state widget lock poisoned").get_mut(&widget_id)
        {
            widget.selected_index = index;
            return true;
        }
        false
    }
    /// Return a widget's selection index, or `None` when nothing is selected.
    pub fn selected_index(&self, widget_id: ObjectId) -> Option<usize> {
        self.widgets
            .lock()
            .expect("backend state widget lock poisoned")
            .get(&widget_id)
            .and_then(|widget| widget.selected_index)
    }
    /// Store a widget's checked state, returning `false` for an unknown id.
    pub fn set_checked(&self, widget_id: ObjectId, checked: bool) -> bool {
        if let Some(widget) =
            self.widgets.lock().expect("backend state widget lock poisoned").get_mut(&widget_id)
        {
            widget.checked = Some(checked);
            return true;
        }
        false
    }
    /// Return a widget's checked state, or `None` when it is not checkable.
    pub fn checked(&self, widget_id: ObjectId) -> Option<bool> {
        self.widgets
            .lock()
            .expect("backend state widget lock poisoned")
            .get(&widget_id)
            .and_then(|widget| widget.checked)
    }

    /// Store a widget's increment step, returning `false` for an unknown id.
    pub fn set_step(&self, widget_id: ObjectId, step: f64) -> bool {
        if let Some(widget) =
            self.widgets.lock().expect("backend state widget lock poisoned").get_mut(&widget_id)
        {
            widget.step = Some(step);
            return true;
        }
        false
    }
    /// Return a widget's increment step, or `None` when it has none.
    pub fn step(&self, widget_id: ObjectId) -> Option<f64> {
        self.widgets
            .lock()
            .expect("backend state widget lock poisoned")
            .get(&widget_id)
            .and_then(|widget| widget.step)
    }
    /// Store a widget's indeterminate state, returning `false` for an unknown id.
    pub fn set_indeterminate(&self, widget_id: ObjectId, indeterminate: bool) -> bool {
        if let Some(widget) =
            self.widgets.lock().expect("backend state widget lock poisoned").get_mut(&widget_id)
        {
            widget.indeterminate = Some(indeterminate);
            return true;
        }
        false
    }
    /// Return a widget's indeterminate state, or `None` when it has none.
    pub fn indeterminate(&self, widget_id: ObjectId) -> Option<bool> {
        self.widgets
            .lock()
            .expect("backend state widget lock poisoned")
            .get(&widget_id)
            .and_then(|widget| widget.indeterminate)
    }
    /// Store a widget's read-only state, returning `false` for an unknown id.
    pub fn set_read_only(&self, widget_id: ObjectId, read_only: bool) -> bool {
        if let Some(widget) =
            self.widgets.lock().expect("backend state widget lock poisoned").get_mut(&widget_id)
        {
            widget.read_only = Some(read_only);
            return true;
        }
        false
    }
    /// Return a widget's read-only state, or `None` when it has none.
    pub fn read_only(&self, widget_id: ObjectId) -> Option<bool> {
        self.widgets
            .lock()
            .expect("backend state widget lock poisoned")
            .get(&widget_id)
            .and_then(|widget| widget.read_only)
    }
    /// Store a widget's maximum text length, returning `false` for an unknown id.
    pub fn set_max_length(&self, widget_id: ObjectId, max_length: u32) -> bool {
        if let Some(widget) =
            self.widgets.lock().expect("backend state widget lock poisoned").get_mut(&widget_id)
        {
            widget.max_length = Some(max_length);
            return true;
        }
        false
    }
    /// Return a widget's maximum text length, or `None` when it has none.
    pub fn max_length(&self, widget_id: ObjectId) -> Option<u32> {
        self.widgets
            .lock()
            .expect("backend state widget lock poisoned")
            .get(&widget_id)
            .and_then(|widget| widget.max_length)
    }

    // ─── Backend event methods ─────────────────────────────────────────────────
    // These methods provide event system integration for menu and widget trigger
    // dispatch. They are called by the macos, mobile, and stub platform backends.

    /// Push menu trigger event.
    /// Reserved for menu system integration (not yet wired to platform backends).
    pub fn push_menu_event(&self, item_id: ObjectId) {
        self.menu_events.lock().expect("backend state menu lock poisoned").push_back(item_id);
    }
    /// Pop menu trigger event.
    /// Reserved for menu system integration (paired with push_menu_event).
    pub fn pop_menu_event(&self) -> Option<ObjectId> {
        self.menu_events.lock().expect("backend state menu lock poisoned").pop_front()
    }
    /// Push typed widget trigger event.
    /// Reserved for event system integration (not yet wired to platform backends).
    pub fn push_widget_event(&self, event: WidgetTriggerEvent) {
        self.widget_events
            .lock()
            .expect("backend state widget-event lock poisoned")
            .push_back(event);
    }
    /// Pop typed widget trigger event.
    pub fn pop_widget_event(&self) -> Option<WidgetTriggerEvent> {
        self.widget_events.lock().expect("backend state widget-event lock poisoned").pop_front()
    }
    /// Set clipboard text.
    pub fn set_clipboard_text(&self, text: &str) -> bool {
        *self.clipboard_text.lock().expect("backend state clipboard lock poisoned") =
            text.to_string();
        true
    }
    /// Get clipboard text.
    pub fn clipboard_text(&self) -> String {
        self.clipboard_text.lock().expect("backend state clipboard lock poisoned").clone()
    }
    /// Begin drag event for existing source widget.
    pub fn begin_drag(&self, source_widget_id: ObjectId, mime: &str, payload: &[u8]) -> bool {
        if !self.contains_widget(source_widget_id) {
            return false;
        }
        self.drop_events.lock().expect("backend state drop lock poisoned").push_back(DropEvent {
            source_widget_id,
            target_widget_id: 0, // Not yet known — target is determined at drop time
            mime: mime.to_string(),
            payload: payload.to_vec(),
        });
        true
    }
    /// Pop one drop event.
    pub fn pop_drop_event(&self) -> Option<DropEvent> {
        self.drop_events.lock().expect("backend state drop lock poisoned").pop_front()
    }
    /// Inject drop event when target widget exists.
    pub fn inject_drop_event(&self, event: DropEvent) -> bool {
        if !self.contains_widget(event.target_widget_id) {
            return false;
        }
        self.drop_events.lock().expect("backend state drop lock poisoned").push_back(event);
        true
    }

    // ─── Test/programmatic event injection ─────────────────────────────────────
    // These helpers are called by the macos, mobile, and stub platform backends
    // for event system bridge functions.

    /// Inject menu trigger event.
    /// Reserved for testing and programmatic event injection.
    pub fn inject_menu_trigger(&self, menu_item_id: ObjectId) -> bool {
        if !self.contains_widget(menu_item_id) {
            return false;
        }
        self.push_menu_event(menu_item_id);
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
    /// Inject widget trigger event.
    /// Reserved for testing and programmatic event injection (typed variant).
    pub fn inject_widget_trigger_event(
        &self,
        widget_id: ObjectId,
        kind: WidgetTriggerKind,
    ) -> bool {
        if !self.contains_widget(widget_id) {
            return false;
        }
        self.push_widget_event(WidgetTriggerEvent { widget_id, kind });
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_kind_returns_true_for_matching_kind() {
        #[derive(Clone, Copy, PartialEq, Eq, Hash)]
        #[cfg_attr(
            all(feature = "serde", not(any(feature = "mini", feature = "embedded"))),
            derive(Serialize, Deserialize)
        )]
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
        #[cfg_attr(
            all(feature = "serde", not(any(feature = "mini", feature = "embedded"))),
            derive(Serialize, Deserialize)
        )]
        enum TestKind {
            Widget,
        }

        let state = BackendState::<TestKind>::new();
        assert!(!state.is_kind(999, TestKind::Widget));
    }
}
