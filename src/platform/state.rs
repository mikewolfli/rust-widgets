//! Shared backend state model used by platform adapters.
use super::{DropEvent, WidgetTriggerEvent, WidgetTriggerKind};
use crate::core::ObjectId;
/// Generic widget state record owned by backend state model.
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::hash::Hash;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
#[derive(Clone, Serialize, Deserialize)]
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
}
/// Thread-safe state model split from native handle adapters.
#[derive(Serialize, Deserialize)]
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
            },
        );
        id
    }
    /// Return `true` when widget exists.
    pub fn contains_widget(&self, widget_id: ObjectId) -> bool {
        self.widgets.lock().expect("backend state widget lock poisoned").contains_key(&widget_id)
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
            target_widget_id: source_widget_id,
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
        self.pop_menu_event()
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
        #[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
        #[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        enum TestKind {
            Widget,
        }

        let state = BackendState::<TestKind>::new();
        assert!(!state.is_kind(999, TestKind::Widget));
    }
}
