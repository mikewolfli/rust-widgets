//! Stub platform implementation for testing and demonstrations.
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use crate::core::{ObjectId, PlatformFamily};
use crate::platform::types::*;
pub struct StubPlatform {
    backend: &'static str,
    family: PlatformFamily,
    next_id: AtomicU64,
    widgets: Mutex<HashMap<ObjectId, WidgetState>>,
    menu_nodes: Mutex<HashMap<ObjectId, MenuNodeState>>,
    menu_events: Mutex<VecDeque<ObjectId>>,
    /// Queue used by injection APIs and test/demo bridge paths.
    widget_events: Mutex<VecDeque<WidgetTriggerEvent>>,
    /// Process-local clipboard text fallback.
    clipboard_text: Mutex<String>,
    /// Drag-drop event queue for backend adapters/tests.
    drop_events: Mutex<VecDeque<DropEvent>>,
    /// In-memory combo-box item storage by logical combo widget id.
    combo_box_items: Mutex<HashMap<ObjectId, Vec<String>>>,
    /// In-memory combo-box selected index by logical combo widget id.
    combo_box_selection: Mutex<HashMap<ObjectId, Option<usize>>>,
    /// In-memory list-box item storage by logical list widget id.
    list_box_items: Mutex<HashMap<ObjectId, Vec<String>>>,
    /// In-memory list-box selected index by logical list widget id.
    list_box_selection: Mutex<HashMap<ObjectId, Option<usize>>>,
}
impl StubPlatform {
    /// Creates a new in-memory stub backend for tests and demos.
    pub fn new(backend: &'static str, family: PlatformFamily) -> Self {
        Self {
            backend,
            family,
            next_id: AtomicU64::new(1),
            widgets: Mutex::new(HashMap::new()),
            menu_nodes: Mutex::new(HashMap::new()),
            menu_events: Mutex::new(VecDeque::new()),
            widget_events: Mutex::new(VecDeque::new()),
            clipboard_text: Mutex::new(String::new()),
            drop_events: Mutex::new(VecDeque::new()),
            combo_box_items: Mutex::new(HashMap::new()),
            combo_box_selection: Mutex::new(HashMap::new()),
            list_box_items: Mutex::new(HashMap::new()),
            list_box_selection: Mutex::new(HashMap::new()),
        }
    }
    fn new_id(&self) -> ObjectId {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }
    fn is_embedded_profile(&self) -> bool {
        matches!(self.family, PlatformFamily::Embedded)
    }
    fn create_widget_state(&self, text: &str, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        let id = self.new_id();
        self.widgets.lock().expect("platform lock poisoned").insert(
            id,
            WidgetState {
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
    fn embedded_unsupported_id(&self, _name: &str) -> ObjectId {
        // Return a dummy id for unsupported features in embedded profile
        0
    }
    fn embedded_unsupported_bool(&self, _name: &str) -> bool {
        // Return false for unsupported features in embedded profile
        false
    }
}
impl Platform for StubPlatform {
    fn backend_name(&self) -> &'static str {
        self.backend
    }
    fn family(&self) -> PlatformFamily {
        self.family
    }
    fn init(&self) {
        // no-op platform stub
    }
    fn run(&self) {
        // no-op platform stub
    }
    fn quit(&self) {
        // no-op platform stub
    }
    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        self.create_widget_state(title, x, y, width, height)
    }
    fn create_button(
        &self,
        _parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.create_widget_state(text, x, y, width, height)
    }
    fn create_menu_bar(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.is_embedded_profile() {
            return self.embedded_unsupported_id("create_menu_bar");
        }
        let id = self.create_button(parent, "MenuBar", x, y, width, height);
        self.menu_nodes
            .lock()
            .expect("platform lock poisoned")
            .insert(
                id,
                MenuNodeState {
                    text: "MenuBar".to_string(),
                },
            );
        id
    }
    fn create_checkbox(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let _ = parent;
        self.create_widget_state(text, x, y, width, height)
    }
    fn create_line_edit(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let _ = parent;
        self.create_widget_state(text, x, y, width, height)
    }
    fn create_label(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let _ = parent;
        self.create_widget_state(text, x, y, width, height)
    }
    fn create_radio_button(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let _ = parent;
        self.create_widget_state(text, x, y, width, height)
    }
    fn create_slider(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        let _ = parent;
        self.create_widget_state("Slider", x, y, width, height)
    }
    fn create_progress_bar(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let _ = parent;
        self.create_widget_state("ProgressBar", x, y, width, height)
    }
    fn create_combo_box(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let id = self.create_button(parent, "ComboBox", x, y, width, height);
        self.combo_box_items
            .lock()
            .expect("platform lock poisoned")
            .insert(id, Vec::new());
        self.combo_box_selection
            .lock()
            .expect("platform lock poisoned")
            .insert(id, None);
        id
    }
    fn combo_box_add_item(&self, combo_box: ObjectId, _text: &str) -> bool {
        let mut items = self.combo_box_items.lock().expect("platform lock poisoned");
        let list = match items.get_mut(&combo_box) {
            Some(list) => list,
            None => return false,
        };
        list.push(_text.to_string());
        true
    }
    fn combo_box_clear_items(&self, combo_box: ObjectId) -> bool {
        let mut items = self.combo_box_items.lock().expect("platform lock poisoned");
        if let Some(list) = items.get_mut(&combo_box) {
            list.clear();
            self.combo_box_selection
                .lock()
                .expect("platform lock poisoned")
                .insert(combo_box, None);
            return true;
        }
        false
    }
    fn combo_box_set_current_index(&self, combo_box: ObjectId, index: usize) -> bool {
        let items = self.combo_box_items.lock().expect("platform lock poisoned");
        let len = match items.get(&combo_box) {
            Some(list) => list.len(),
            None => return false,
        };
        if index >= len {
            return false;
        }
        drop(items);
        self.combo_box_selection
            .lock()
            .expect("platform lock poisoned")
            .insert(combo_box, Some(index));
        true
    }
    fn combo_box_current_index(&self, combo_box: ObjectId) -> Option<usize> {
        self.combo_box_selection
            .lock()
            .expect("platform lock poisoned")
            .get(&combo_box)
            .and_then(|index| *index)
    }
    fn combo_box_item_count(&self, combo_box: ObjectId) -> usize {
        self.combo_box_items
            .lock()
            .expect("platform lock poisoned")
            .get(&combo_box)
            .map(|items| items.len())
            .unwrap_or(0)
    }
    fn combo_box_item_text(&self, combo_box: ObjectId, index: usize) -> Option<String> {
        self.combo_box_items
            .lock()
            .expect("platform lock poisoned")
            .get(&combo_box)
            .and_then(|items| items.get(index).cloned())
    }
    fn create_list_box(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let id = self.create_button(parent, "ListBox", x, y, width, height);
        self.list_box_items
            .lock()
            .expect("platform lock poisoned")
            .insert(id, Vec::new());
        self.list_box_selection
            .lock()
            .expect("platform lock poisoned")
            .insert(id, None);
        id
    }
    fn list_box_add_item(&self, list_box: ObjectId, text: &str) -> bool {
        let mut items = self.list_box_items.lock().expect("platform lock poisoned");
        let list = match items.get_mut(&list_box) {
            Some(list) => list,
            None => return false,
        };
        list.push(text.to_string());
        true
    }
    fn list_box_remove_item(&self, list_box: ObjectId, index: usize) -> bool {
        let mut items = self.list_box_items.lock().expect("platform lock poisoned");
        let list = match items.get_mut(&list_box) {
            Some(list) => list,
            None => return false,
        };
        if index >= list.len() {
            return false;
        }
        list.remove(index);
        let mut selection = self
            .list_box_selection
            .lock()
            .expect("platform lock poisoned");
        if let Some(current) = selection.get(&list_box).and_then(|value| *value) {
            if current == index {
                selection.insert(list_box, None);
            } else if current > index {
                selection.insert(list_box, Some(current - 1));
            }
        }
        true
    }
    fn list_box_clear_items(&self, list_box: ObjectId) -> bool {
        let mut items = self.list_box_items.lock().expect("platform lock poisoned");
        if let Some(list) = items.get_mut(&list_box) {
            list.clear();
            self.list_box_selection
                .lock()
                .expect("platform lock poisoned")
                .insert(list_box, None);
            return true;
        }
        false
    }
    fn list_box_set_current_index(&self, list_box: ObjectId, index: usize) -> bool {
        let items = self.list_box_items.lock().expect("platform lock poisoned");
        let len = match items.get(&list_box) {
            Some(list) => list.len(),
            None => return false,
        };
        if index >= len {
            return false;
        }
        drop(items);
        self.list_box_selection
            .lock()
            .expect("platform lock poisoned")
            .insert(list_box, Some(index));
        true
    }
    fn list_box_current_index(&self, list_box: ObjectId) -> Option<usize> {
        self.list_box_selection
            .lock()
            .expect("platform lock poisoned")
            .get(&list_box)
            .and_then(|index| *index)
    }
    fn list_box_item_count(&self, list_box: ObjectId) -> usize {
        self.list_box_items
            .lock()
            .expect("platform lock poisoned")
            .get(&list_box)
            .map(|items| items.len())
            .unwrap_or(0)
    }
    fn list_box_item_text(&self, list_box: ObjectId, index: usize) -> Option<String> {
        self.list_box_items
            .lock()
            .expect("platform lock poisoned")
            .get(&list_box)
            .and_then(|items| items.get(index).cloned())
    }
    fn create_panel(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        let _ = parent;
        self.create_widget_state("Panel", x, y, width, height)
    }
    fn create_menu(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.is_embedded_profile() {
            return self.embedded_unsupported_id("create_menu");
        }
        let id = self.create_button(parent, text, x, y, width, height);
        self.menu_nodes
            .lock()
            .expect("platform lock poisoned")
            .insert(
                id,
                MenuNodeState {
                    text: text.to_string(),
                },
            );
        id
    }
    fn create_tool_bar(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.is_embedded_profile() {
            return self.embedded_unsupported_id("create_tool_bar");
        }
        self.create_button(parent, "ToolBar", x, y, width, height)
    }
    fn create_status_bar(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.is_embedded_profile() {
            return self.embedded_unsupported_id("create_status_bar");
        }
        self.create_button(parent, text, x, y, width, height)
    }
    fn create_message_box(
        &self,
        parent: ObjectId,
        title: &str,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let _ = (parent, title, text);
        self.create_widget_state("MessageBox", x, y, width, height)
    }
    fn create_file_dialog(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let _ = parent;
        self.create_widget_state("FileDialog", x, y, width, height)
    }
    fn create_color_dialog(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let _ = parent;
        self.create_widget_state("ColorDialog", x, y, width, height)
    }
    fn create_font_dialog(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let _ = parent;
        self.create_widget_state("FontDialog", x, y, width, height)
    }
    fn create_spin_box(
        &self,
        _parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.create_widget_state("ComboBox", x, y, width, height)
    }
    fn create_list_view(
        &self,
        _parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.create_widget_state("ListBox", x, y, width, height)
    }
    fn create_scroll_area(
        &self,
        _parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.create_widget_state("Panel", x, y, width, height)
    }
    fn attach_menu_bar_to_window(&self, window: ObjectId, menu_bar: ObjectId) -> bool {
        if self.is_embedded_profile() {
            return self.embedded_unsupported_bool("attach_menu_bar_to_window");
        }
        let widgets = self.widgets.lock().expect("platform lock poisoned");
        widgets.contains_key(&window) && widgets.contains_key(&menu_bar)
    }
    fn menu_add_item(&self, parent_menu: ObjectId, text: &str, shortcut: Option<&str>) -> ObjectId {
        if self.is_embedded_profile() {
            return self.embedded_unsupported_id("menu_add_item");
        }
        let id = self.create_menu(parent_menu, text, 0, 0, 0, 0);
        self.menu_nodes
            .lock()
            .expect("platform lock poisoned")
            .insert(
                id,
                MenuNodeState {
                    text: text.to_string(),
                },
            );
        let _ = shortcut;
        id
    }
    fn poll_menu_triggered(&self) -> Option<ObjectId> {
        let mut events = self.menu_events.lock().expect("platform lock poisoned");
        events.pop_front()
    }
    fn inject_menu_trigger(&self, menu_item_id: ObjectId) -> bool {
        if self.is_embedded_profile() {
            return self.embedded_unsupported_bool("inject_menu_trigger");
        }
        // Accept only known menu ids to avoid emitting orphan events.
        if !self
            .menu_nodes
            .lock()
            .expect("platform lock poisoned")
            .contains_key(&menu_item_id)
        {
            return false;
        }
        self.menu_events
            .lock()
            .expect("platform lock poisoned")
            .push_back(menu_item_id);
        true
    }
    fn poll_widget_triggered(&self) -> Option<ObjectId> {
        self.poll_widget_trigger_event()
            .map(|event| event.widget_id)
    }
    fn poll_widget_trigger_event(&self) -> Option<WidgetTriggerEvent> {
        self.widget_events
            .lock()
            .expect("platform lock poisoned")
            .pop_front()
    }
    fn inject_widget_trigger_event(&self, widget_id: ObjectId, kind: WidgetTriggerKind) -> bool {
        // Accept only known widget ids to keep queue semantics deterministic.
        if !self
            .widgets
            .lock()
            .expect("platform lock poisoned")
            .contains_key(&widget_id)
        {
            return false;
        }
        self.widget_events
            .lock()
            .expect("platform lock poisoned")
            .push_back(WidgetTriggerEvent { widget_id, kind });
        true
    }
    fn show_widget(&self, widget_id: ObjectId) {
        if let Some(widget) = self
            .widgets
            .lock()
            .expect("platform lock poisoned")
            .get_mut(&widget_id)
        {
            widget.visible = true;
        }
    }
    fn hide_widget(&self, widget_id: ObjectId) {
        if let Some(widget) = self
            .widgets
            .lock()
            .expect("platform lock poisoned")
            .get_mut(&widget_id)
        {
            widget.visible = false;
        }
    }
    fn set_widget_geometry(&self, widget_id: ObjectId, x: i32, y: i32, width: u32, height: u32) {
        if let Some(widget) = self
            .widgets
            .lock()
            .expect("platform lock poisoned")
            .get_mut(&widget_id)
        {
            widget.x = x;
            widget.y = y;
            widget.width = width;
            widget.height = height;
        }
    }
    fn set_widget_text(&self, widget_id: ObjectId, text: &str) {
        if let Some(widget) = self
            .widgets
            .lock()
            .expect("platform lock poisoned")
            .get_mut(&widget_id)
        {
            widget.text = text.to_string();
        }
        if let Some(node) = self
            .menu_nodes
            .lock()
            .expect("platform lock poisoned")
            .get_mut(&widget_id)
        {
            node.text = text.to_string();
        }
    }
    fn get_widget_text(&self, widget_id: ObjectId) -> String {
        self.widgets
            .lock()
            .expect("platform lock poisoned")
            .get(&widget_id)
            .map(|widget| widget.text.clone())
            .unwrap_or_default()
    }
    fn set_widget_enabled(&self, widget_id: ObjectId, enabled: bool) {
        if let Some(widget) = self
            .widgets
            .lock()
            .expect("platform lock poisoned")
            .get_mut(&widget_id)
        {
            widget.enabled = enabled;
        }
    }
    fn is_widget_enabled(&self, widget_id: ObjectId) -> bool {
        self.widgets
            .lock()
            .expect("platform lock poisoned")
            .get(&widget_id)
            .map(|widget| widget.enabled)
            .unwrap_or(false)
    }
    fn set_widget_visible(&self, widget_id: ObjectId, visible: bool) {
        if let Some(widget) = self
            .widgets
            .lock()
            .expect("platform lock poisoned")
            .get_mut(&widget_id)
        {
            widget.visible = visible;
        }
    }
    fn is_widget_visible(&self, widget_id: ObjectId) -> bool {
        self.widgets
            .lock()
            .expect("platform lock poisoned")
            .get(&widget_id)
            .map(|widget| widget.visible)
            .unwrap_or(false)
    }
    fn set_widget_ime_enabled(&self, widget_id: ObjectId, enabled: bool) -> bool {
        if let Some(widget) = self
            .widgets
            .lock()
            .expect("platform lock poisoned")
            .get_mut(&widget_id)
        {
            widget.ime_enabled = enabled;
            return true;
        }
        false
    }
    fn is_widget_ime_enabled(&self, widget_id: ObjectId) -> bool {
        self.widgets
            .lock()
            .expect("platform lock poisoned")
            .get(&widget_id)
            .map(|widget| widget.ime_enabled)
            .unwrap_or(false)
    }
    fn set_widget_accessibility_name(&self, widget_id: ObjectId, name: &str) -> bool {
        if let Some(widget) = self
            .widgets
            .lock()
            .expect("platform lock poisoned")
            .get_mut(&widget_id)
        {
            widget.accessibility_name = name.to_string();
            return true;
        }
        false
    }
    fn get_widget_accessibility_name(&self, widget_id: ObjectId) -> String {
        self.widgets
            .lock()
            .expect("platform lock poisoned")
            .get(&widget_id)
            .map(|widget| widget.accessibility_name.clone())
            .unwrap_or_default()
    }
    fn set_clipboard_text(&self, text: &str) -> bool {
        *self
            .clipboard_text
            .lock()
            .expect("platform clipboard lock poisoned") = text.to_string();
        true
    }
    fn get_clipboard_text(&self) -> String {
        self.clipboard_text
            .lock()
            .expect("platform clipboard lock poisoned")
            .clone()
    }
    fn begin_drag(&self, source_widget_id: ObjectId, mime: &str, payload: &[u8]) -> bool {
        if !self
            .widgets
            .lock()
            .expect("platform lock poisoned")
            .contains_key(&source_widget_id)
        {
            return false;
        }
        self.drop_events
            .lock()
            .expect("platform drop lock poisoned")
            .push_back(DropEvent {
                source_widget_id,
                target_widget_id: source_widget_id,
                mime: mime.to_string(),
                payload: payload.to_vec(),
            });
        true
    }
    fn poll_drop_event(&self) -> Option<DropEvent> {
        self.drop_events
            .lock()
            .expect("platform drop lock poisoned")
            .pop_front()
    }
    fn inject_drop_event(&self, event: DropEvent) -> bool {
        if !self
            .widgets
            .lock()
            .expect("platform lock poisoned")
            .contains_key(&event.target_widget_id)
        {
            return false;
        }
        self.drop_events
            .lock()
            .expect("platform drop lock poisoned")
            .push_back(event);
        true
    }
}
