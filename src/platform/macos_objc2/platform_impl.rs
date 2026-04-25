use super::types::{MacOSObjc2Platform, MacObjc2HandleKind};
use crate::core::ObjectId;
use crate::core::PlatformFamily;
use crate::platform::{DropEvent, Platform, WidgetTriggerEvent, WidgetTriggerKind};
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;

impl Platform for MacOSObjc2Platform {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn backend_name(&self) -> &'static str {
        "macos-objc2-preview"
    }
    fn family(&self) -> PlatformFamily {
        PlatformFamily::Desktop
    }
    fn init(&self) {
        // Marker keeps objc2 dependency wired even before native event-loop bridging lands.
        let _ = self.objc2_runtime_marker();
        self.runtime.initialized.store(true, Ordering::SeqCst);
    }
    fn run(&self) {
        if !self.runtime.initialized.load(Ordering::SeqCst) {
            self.init();
        }
        // Preview backend uses a deterministic polling loop to preserve trait-level parity.
        self.runtime.running.store(true, Ordering::SeqCst);
        while self.runtime.running.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(16));
        }
    }
    fn quit(&self) {
        self.runtime.running.store(false, Ordering::SeqCst);
    }
    /// Create a new window with the given title and geometry.
    /// This is the entry point for window lifecycle parity tests.
    /// Returns a unique window id.
    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        // Insert window widget into backend state
        self.insert_widget(MacObjc2HandleKind::Window, title, x, y, width, height)
    }
    fn create_button(
        &self,
        parent: u64,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        // Mirror native constraint: child controls require a valid existing parent.
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.insert_widget(MacObjc2HandleKind::Button, text, x, y, width, height)
    }
    fn create_checkbox(
        &self,
        parent: u64,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        // Keep creation contract identical to default backend for migration parity.
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.insert_widget(MacObjc2HandleKind::CheckBox, text, x, y, width, height)
    }
    fn create_line_edit(
        &self,
        parent: u64,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        // Keep creation contract identical to default backend for migration parity.
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.insert_widget(MacObjc2HandleKind::LineEdit, text, x, y, width, height)
    }
    fn create_label(
        &self,
        parent: u64,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.insert_widget(MacObjc2HandleKind::Label, text, x, y, width, height)
    }
    fn create_radio_button(
        &self,
        parent: u64,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.insert_widget(MacObjc2HandleKind::RadioButton, text, x, y, width, height)
    }
    fn create_slider(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.insert_widget(MacObjc2HandleKind::Slider, "Slider", x, y, width, height)
    }
    fn create_progress_bar(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.insert_widget(
            MacObjc2HandleKind::ProgressBar,
            "ProgressBar",
            x,
            y,
            width,
            height,
        )
    }
    fn create_combo_box(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.insert_widget(
            MacObjc2HandleKind::ComboBox,
            "ComboBox",
            x,
            y,
            width,
            height,
        )
    }
    fn create_list_box(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.insert_widget(MacObjc2HandleKind::ListBox, "ListBox", x, y, width, height)
    }
    fn list_box_add_item(&self, list_box: u64, text: &str) -> bool {
        // Validate that widget exists and is a ListBox.
        if !matches!(self.kind_of(list_box), Some(MacObjc2HandleKind::ListBox)) {
            return false;
        }
        let mut data = self
            .list_data
            .lock()
            .expect("mac objc2 list data lock poisoned");
        let entry = data.entry(list_box).or_default();
        entry.items.push(text.to_string());
        true
    }
    fn list_box_remove_item(&self, list_box: u64, index: usize) -> bool {
        if !matches!(self.kind_of(list_box), Some(MacObjc2HandleKind::ListBox)) {
            return false;
        }
        let mut data = self
            .list_data
            .lock()
            .expect("mac objc2 list data lock poisoned");
        let entry = match data.get_mut(&list_box) {
            Some(e) => e,
            None => return false,
        };
        if index >= entry.items.len() {
            return false;
        }
        entry.items.remove(index);
        // Adjust current_index if the removed item was at or before it.
        if let Some(cur) = entry.current_index {
            if cur == index {
                // Item at the selected index was removed — clear selection.
                entry.current_index = None;
            } else if cur > index {
                // Selection shifted down by one.
                entry.current_index = Some(cur - 1);
            }
        }
        true
    }
    fn list_box_clear_items(&self, list_box: u64) -> bool {
        if !matches!(self.kind_of(list_box), Some(MacObjc2HandleKind::ListBox)) {
            return false;
        }
        let mut data = self
            .list_data
            .lock()
            .expect("mac objc2 list data lock poisoned");
        if let Some(entry) = data.get_mut(&list_box) {
            entry.items.clear();
            entry.current_index = None;
        }
        true
    }
    fn list_box_set_current_index(&self, list_box: u64, index: usize) -> bool {
        if !matches!(self.kind_of(list_box), Some(MacObjc2HandleKind::ListBox)) {
            return false;
        }
        let mut data = self
            .list_data
            .lock()
            .expect("mac objc2 list data lock poisoned");
        let entry = match data.get_mut(&list_box) {
            Some(e) => e,
            None => return false,
        };
        if index >= entry.items.len() {
            return false;
        }
        entry.current_index = Some(index);
        true
    }
    fn list_box_current_index(&self, list_box: u64) -> Option<usize> {
        if !matches!(self.kind_of(list_box), Some(MacObjc2HandleKind::ListBox)) {
            return None;
        }
        let data = self
            .list_data
            .lock()
            .expect("mac objc2 list data lock poisoned");
        data.get(&list_box).and_then(|entry| entry.current_index)
    }
    fn list_box_item_count(&self, list_box: u64) -> usize {
        if !matches!(self.kind_of(list_box), Some(MacObjc2HandleKind::ListBox)) {
            return 0;
        }
        let data = self
            .list_data
            .lock()
            .expect("mac objc2 list data lock poisoned");
        data.get(&list_box).map_or(0, |entry| entry.items.len())
    }
    fn list_box_item_text(&self, list_box: u64, index: usize) -> Option<String> {
        if !matches!(self.kind_of(list_box), Some(MacObjc2HandleKind::ListBox)) {
            return None;
        }
        let data = self
            .list_data
            .lock()
            .expect("mac objc2 list data lock poisoned");
        data.get(&list_box)
            .and_then(|entry| entry.items.get(index))
            .cloned()
    }
    fn combo_box_add_item(&self, combo_box: u64, text: &str) -> bool {
        if !matches!(self.kind_of(combo_box), Some(MacObjc2HandleKind::ComboBox)) {
            return false;
        }
        let mut data = self
            .list_data
            .lock()
            .expect("mac objc2 list data lock poisoned");
        let entry = data.entry(combo_box).or_default();
        entry.items.push(text.to_string());
        true
    }
    fn combo_box_clear_items(&self, combo_box: u64) -> bool {
        if !matches!(self.kind_of(combo_box), Some(MacObjc2HandleKind::ComboBox)) {
            return false;
        }
        let mut data = self
            .list_data
            .lock()
            .expect("mac objc2 list data lock poisoned");
        if let Some(entry) = data.get_mut(&combo_box) {
            entry.items.clear();
            entry.current_index = None;
        }
        true
    }
    fn combo_box_set_current_index(&self, combo_box: u64, index: usize) -> bool {
        if !matches!(self.kind_of(combo_box), Some(MacObjc2HandleKind::ComboBox)) {
            return false;
        }
        let mut data = self
            .list_data
            .lock()
            .expect("mac objc2 list data lock poisoned");
        let entry = match data.get_mut(&combo_box) {
            Some(e) => e,
            None => return false,
        };
        if index >= entry.items.len() {
            return false;
        }
        entry.current_index = Some(index);
        true
    }
    fn combo_box_current_index(&self, combo_box: u64) -> Option<usize> {
        if !matches!(self.kind_of(combo_box), Some(MacObjc2HandleKind::ComboBox)) {
            return None;
        }
        let data = self
            .list_data
            .lock()
            .expect("mac objc2 list data lock poisoned");
        data.get(&combo_box).and_then(|entry| entry.current_index)
    }
    fn combo_box_item_count(&self, combo_box: u64) -> usize {
        if !matches!(self.kind_of(combo_box), Some(MacObjc2HandleKind::ComboBox)) {
            return 0;
        }
        let data = self
            .list_data
            .lock()
            .expect("mac objc2 list data lock poisoned");
        data.get(&combo_box).map_or(0, |entry| entry.items.len())
    }
    fn combo_box_item_text(&self, combo_box: u64, index: usize) -> Option<String> {
        if !matches!(self.kind_of(combo_box), Some(MacObjc2HandleKind::ComboBox)) {
            return None;
        }
        let data = self
            .list_data
            .lock()
            .expect("mac objc2 list data lock poisoned");
        data.get(&combo_box)
            .and_then(|entry| entry.items.get(index))
            .cloned()
    }
    fn create_panel(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.insert_widget(MacObjc2HandleKind::Panel, "Panel", x, y, width, height)
    }
    fn create_menu_bar(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if !matches!(self.kind_of(parent), Some(MacObjc2HandleKind::Window)) {
            return 0;
        }
        self.insert_widget(MacObjc2HandleKind::MenuBar, "MenuBar", x, y, width, height)
    }
    fn create_menu(&self, parent: u64, text: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if !matches!(
            self.kind_of(parent),
            Some(MacObjc2HandleKind::MenuBar | MacObjc2HandleKind::Menu)
        ) {
            return 0;
        }
        let id = self.insert_widget(MacObjc2HandleKind::Menu, text, x, y, width, height);
        self.menus
            .lock()
            .expect("mac objc2 menu lock poisoned")
            .menu_children
            .entry(parent)
            .or_default()
            .push(id);
        id
    }
    fn create_tool_bar(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if !matches!(self.kind_of(parent), Some(MacObjc2HandleKind::Window)) {
            return 0;
        }
        self.insert_widget(MacObjc2HandleKind::ToolBar, "ToolBar", x, y, width, height)
    }
    fn create_status_bar(
        &self,
        parent: u64,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        if !matches!(self.kind_of(parent), Some(MacObjc2HandleKind::Window)) {
            return 0;
        }
        self.insert_widget(MacObjc2HandleKind::StatusBar, text, x, y, width, height)
    }
    fn attach_menu_bar_to_window(&self, window: u64, menu_bar: u64) -> bool {
        // Reject invalid kind combinations to avoid stale menu-bar ownership mappings.
        if matches!(self.kind_of(window), Some(MacObjc2HandleKind::Window))
            && matches!(self.kind_of(menu_bar), Some(MacObjc2HandleKind::MenuBar))
        {
            self.menus
                .lock()
                .expect("mac objc2 menu lock poisoned")
                .attached_menu_bar
                .insert(window, menu_bar);
            return true;
        }
        false
    }
    fn menu_add_item(&self, parent_menu: u64, text: &str, shortcut: Option<&str>) -> u64 {
        if !matches!(self.kind_of(parent_menu), Some(MacObjc2HandleKind::Menu)) {
            return 0;
        }
        let item_id = self.insert_widget(MacObjc2HandleKind::MenuItem, text, 0, 0, 0, 0);
        // Shortcut parsing is intentionally deferred until native objc2 menu bridging is finalized.
        let _ = shortcut;
        let mut menus = self.menus.lock().expect("mac objc2 menu lock poisoned");
        menus
            .menu_children
            .entry(parent_menu)
            .or_default()
            .push(item_id);
        item_id
    }
    fn poll_menu_triggered(&self) -> Option<u64> {
        self.menus
            .lock()
            .expect("mac objc2 menu lock poisoned")
            .pending_menu_events
            .pop_front()
    }
    fn inject_menu_trigger(&self, menu_item_id: u64) -> bool {
        if !matches!(
            self.kind_of(menu_item_id),
            Some(MacObjc2HandleKind::MenuItem)
        ) {
            return false;
        }
        // Queue-only bridge preserves deterministic trigger order across test and native paths.
        self.menus
            .lock()
            .expect("mac objc2 menu lock poisoned")
            .pending_menu_events
            .push_back(menu_item_id);
        true
    }
    fn poll_widget_triggered(&self) -> Option<u64> {
        self.poll_widget_trigger_event()
            .map(|event| event.widget_id)
    }
    fn poll_widget_trigger_event(&self) -> Option<WidgetTriggerEvent> {
        self.menus
            .lock()
            .expect("mac objc2 menu lock poisoned")
            .pending_widget_events
            .pop_front()
    }
    fn inject_widget_trigger_event(&self, widget_id: u64, kind: WidgetTriggerKind) -> bool {
        if self.kind_of(widget_id).is_none() {
            return false;
        }
        // Normalize native control notifications into typed cross-platform trigger events.
        self.menus
            .lock()
            .expect("mac objc2 menu lock poisoned")
            .pending_widget_events
            .push_back(WidgetTriggerEvent { widget_id, kind });
        true
    }
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
        if !self.state.set_text(widget_id, text) {
            return;
        }
        if matches!(self.kind_of(widget_id), Some(MacObjc2HandleKind::LineEdit)) {
            // Text edits emit value-changed semantics to match other desktop backends.
            self.menus
                .lock()
                .expect("mac objc2 menu lock poisoned")
                .pending_widget_events
                .push_back(WidgetTriggerEvent {
                    widget_id,
                    kind: WidgetTriggerKind::ValueChanged,
                });
        }
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
    fn set_widget_ime_enabled(&self, widget_id: u64, enabled: bool) -> bool {
        self.state.set_ime_enabled(widget_id, enabled)
    }
    fn is_widget_ime_enabled(&self, widget_id: u64) -> bool {
        self.state.ime_enabled(widget_id)
    }
    fn set_widget_accessibility_name(&self, widget_id: u64, name: &str) -> bool {
        self.state.set_accessibility_name(widget_id, name)
    }
    fn get_widget_accessibility_name(&self, widget_id: u64) -> String {
        self.state.accessibility_name(widget_id)
    }
    fn set_clipboard_text(&self, text: &str) -> bool {
        self.state.set_clipboard_text(text)
    }
    fn get_clipboard_text(&self) -> String {
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
        self.insert_widget(
            MacObjc2HandleKind::MessageBox,
            "MessageBox",
            x,
            y,
            width,
            height,
        )
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
        self.insert_widget(
            MacObjc2HandleKind::FileDialog,
            "FileDialog",
            x,
            y,
            width,
            height,
        )
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
        self.insert_widget(
            MacObjc2HandleKind::ColorDialog,
            "ColorDialog",
            x,
            y,
            width,
            height,
        )
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
        self.insert_widget(
            MacObjc2HandleKind::FontDialog,
            "FontDialog",
            x,
            y,
            width,
            height,
        )
    }
    fn create_spin_box(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.insert_widget(MacObjc2HandleKind::Panel, "SpinBox", x, y, width, height)
    }
    fn create_list_view(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.insert_widget(MacObjc2HandleKind::Panel, "ListView", x, y, width, height)
    }
    fn create_scroll_area(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.insert_widget(MacObjc2HandleKind::Panel, "ScrollArea", x, y, width, height)
    }
}
