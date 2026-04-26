use super::super::{DropEvent, Platform, WidgetTriggerEvent, WidgetTriggerKind};
use super::types::*;
use crate::core::{MutexExt, PlatformFamily};

use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;

impl Platform for HarmonyPlatform {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn backend_name(&self) -> &'static str {
        "harmony-desktop"
    }
    fn family(&self) -> PlatformFamily {
        PlatformFamily::Desktop
    }
    fn init(&self) {
        self.runtime.initialized.store(true, Ordering::SeqCst);
    }
    fn run(&self) {
        if !self.runtime.initialized.load(Ordering::SeqCst) {
            self.init();
        }
        self.runtime.running.store(true, Ordering::SeqCst);
        while self.runtime.running.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(16));
        }
    }
    fn quit(&self) {
        self.runtime.running.store(false, Ordering::SeqCst);
    }
    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        self.insert_widget(HarmonyHandleKind::Window, title, x, y, width, height)
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
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.insert_widget(HarmonyHandleKind::Button, text, x, y, width, height)
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
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.insert_widget(HarmonyHandleKind::CheckBox, text, x, y, width, height)
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
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.insert_widget(HarmonyHandleKind::LineEdit, text, x, y, width, height)
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
        self.insert_widget(HarmonyHandleKind::Label, text, x, y, width, height)
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
        self.insert_widget(HarmonyHandleKind::RadioButton, text, x, y, width, height)
    }
    fn create_slider(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.insert_widget(HarmonyHandleKind::Slider, "Slider", x, y, width, height)
    }
    fn create_progress_bar(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.insert_widget(
            HarmonyHandleKind::ProgressBar,
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
        let id = self.insert_widget(HarmonyHandleKind::ComboBox, "ComboBox", x, y, width, height);
        self.list_data
            .lock_guard()
            .insert(
                id,
                ListData {
                    items: Vec::new(),
                    current_index: None,
                },
            );
        id
    }
    fn create_list_box(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.insert_widget(HarmonyHandleKind::ListBox, "ListBox", x, y, width, height);
        self.list_data
            .lock_guard()
            .insert(
                id,
                ListData {
                    items: Vec::new(),
                    current_index: None,
                },
            );
        id
    }
    fn list_box_add_item(&self, list_box: u64, text: &str) -> bool {
        if !matches!(self.kind_of(list_box), Some(HarmonyHandleKind::ListBox)) {
            return false;
        }
        let mut data = self
            .list_data
            .lock_guard();
        let entry = data.entry(list_box).or_default();
        entry.items.push(text.to_string());
        true
    }
    fn list_box_remove_item(&self, list_box: u64, index: usize) -> bool {
        if !matches!(self.kind_of(list_box), Some(HarmonyHandleKind::ListBox)) {
            return false;
        }
        let mut data = self
            .list_data
            .lock_guard();
        let entry = match data.get_mut(&list_box) {
            Some(e) => e,
            None => return false,
        };
        if index >= entry.items.len() {
            return false;
        }
        entry.items.remove(index);
        if let Some(cur) = entry.current_index {
            if cur == index {
                entry.current_index = None;
            } else if cur > index {
                entry.current_index = Some(cur - 1);
            }
        }
        true
    }
    fn list_box_clear_items(&self, list_box: u64) -> bool {
        if !matches!(self.kind_of(list_box), Some(HarmonyHandleKind::ListBox)) {
            return false;
        }
        let mut data = self
            .list_data
            .lock_guard();
        if let Some(entry) = data.get_mut(&list_box) {
            entry.items.clear();
            entry.current_index = None;
        }
        true
    }
    fn list_box_set_current_index(&self, list_box: u64, index: usize) -> bool {
        if !matches!(self.kind_of(list_box), Some(HarmonyHandleKind::ListBox)) {
            return false;
        }
        let mut data = self
            .list_data
            .lock_guard();
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
        if !matches!(self.kind_of(list_box), Some(HarmonyHandleKind::ListBox)) {
            return None;
        }
        let data = self
            .list_data
            .lock_guard();
        data.get(&list_box).and_then(|entry| entry.current_index)
    }
    fn list_box_item_count(&self, list_box: u64) -> usize {
        if !matches!(self.kind_of(list_box), Some(HarmonyHandleKind::ListBox)) {
            return 0;
        }
        let data = self
            .list_data
            .lock_guard();
        data.get(&list_box).map_or(0, |entry| entry.items.len())
    }
    fn list_box_item_text(&self, list_box: u64, index: usize) -> Option<String> {
        if !matches!(self.kind_of(list_box), Some(HarmonyHandleKind::ListBox)) {
            return None;
        }
        let data = self
            .list_data
            .lock_guard();
        data.get(&list_box)
            .and_then(|entry| entry.items.get(index))
            .cloned()
    }
    fn combo_box_add_item(&self, combo_box: u64, text: &str) -> bool {
        if !matches!(self.kind_of(combo_box), Some(HarmonyHandleKind::ComboBox)) {
            return false;
        }
        let mut data = self
            .list_data
            .lock_guard();
        let entry = data.entry(combo_box).or_default();
        entry.items.push(text.to_string());
        true
    }
    fn combo_box_clear_items(&self, combo_box: u64) -> bool {
        if !matches!(self.kind_of(combo_box), Some(HarmonyHandleKind::ComboBox)) {
            return false;
        }
        let mut data = self
            .list_data
            .lock_guard();
        if let Some(entry) = data.get_mut(&combo_box) {
            entry.items.clear();
            entry.current_index = None;
        }
        true
    }
    fn combo_box_set_current_index(&self, combo_box: u64, index: usize) -> bool {
        if !matches!(self.kind_of(combo_box), Some(HarmonyHandleKind::ComboBox)) {
            return false;
        }
        let mut data = self
            .list_data
            .lock_guard();
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
        if !matches!(self.kind_of(combo_box), Some(HarmonyHandleKind::ComboBox)) {
            return None;
        }
        let data = self
            .list_data
            .lock_guard();
        data.get(&combo_box).and_then(|entry| entry.current_index)
    }
    fn combo_box_item_count(&self, combo_box: u64) -> usize {
        if !matches!(self.kind_of(combo_box), Some(HarmonyHandleKind::ComboBox)) {
            return 0;
        }
        let data = self
            .list_data
            .lock_guard();
        data.get(&combo_box).map_or(0, |entry| entry.items.len())
    }
    fn combo_box_item_text(&self, combo_box: u64, index: usize) -> Option<String> {
        if !matches!(self.kind_of(combo_box), Some(HarmonyHandleKind::ComboBox)) {
            return None;
        }
        let data = self
            .list_data
            .lock_guard();
        data.get(&combo_box)
            .and_then(|entry| entry.items.get(index))
            .cloned()
    }
    fn create_panel(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.insert_widget(HarmonyHandleKind::Panel, "Panel", x, y, width, height)
    }
    fn create_menu_bar(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if !matches!(self.kind_of(parent), Some(HarmonyHandleKind::Window)) {
            return 0;
        }
        self.insert_widget(HarmonyHandleKind::MenuBar, "MenuBar", x, y, width, height)
    }
    fn create_menu(&self, parent: u64, text: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if !matches!(
            self.kind_of(parent),
            Some(HarmonyHandleKind::MenuBar | HarmonyHandleKind::Menu)
        ) {
            return 0;
        }
        let id = self.insert_widget(HarmonyHandleKind::Menu, text, x, y, width, height);
        self.menus
            .lock_guard()
            .menu_children
            .entry(parent)
            .or_default()
            .push(id);
        id
    }
    fn create_tool_bar(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if !matches!(self.kind_of(parent), Some(HarmonyHandleKind::Window)) {
            return 0;
        }
        self.insert_widget(HarmonyHandleKind::ToolBar, "ToolBar", x, y, width, height)
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
        if !matches!(self.kind_of(parent), Some(HarmonyHandleKind::Window)) {
            return 0;
        }
        self.insert_widget(HarmonyHandleKind::StatusBar, text, x, y, width, height)
    }
    fn attach_menu_bar_to_window(&self, window: u64, menu_bar: u64) -> bool {
        if matches!(self.kind_of(window), Some(HarmonyHandleKind::Window))
            && matches!(self.kind_of(menu_bar), Some(HarmonyHandleKind::MenuBar))
        {
            self.menus
                .lock_guard()
                .attached_menu_bar
                .insert(window, menu_bar);
            return true;
        }
        false
    }
    fn menu_add_item(&self, parent_menu: u64, text: &str, shortcut: Option<&str>) -> u64 {
        if !matches!(self.kind_of(parent_menu), Some(HarmonyHandleKind::Menu)) {
            return 0;
        }
        let item_id = self.insert_widget(HarmonyHandleKind::MenuItem, text, 0, 0, 0, 0);
        let _ = shortcut;
        let mut menus = self.menus.lock_guard();
        menus
            .menu_children
            .entry(parent_menu)
            .or_default()
            .push(item_id);
        item_id
    }
    fn poll_menu_triggered(&self) -> Option<u64> {
        self.menus
            .lock_guard()
            .pending_menu_events
            .pop_front()
    }
    fn inject_menu_trigger(&self, menu_item_id: u64) -> bool {
        // Only menu items may generate menu trigger events.
        if !matches!(
            self.kind_of(menu_item_id),
            Some(HarmonyHandleKind::MenuItem)
        ) {
            return false;
        }
        self.menus
            .lock_guard()
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
            .lock_guard()
            .pending_widget_events
            .pop_front()
    }
    fn inject_widget_trigger_event(&self, widget_id: u64, kind: WidgetTriggerKind) -> bool {
        // Any known widget may enqueue a typed trigger event.
        if self.kind_of(widget_id).is_none() {
            return false;
        }
        self.menus
            .lock_guard()
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
        // Keep line-edit behavior consistent with value-changed semantics.
        if matches!(self.kind_of(widget_id), Some(HarmonyHandleKind::LineEdit)) {
            self.menus
                .lock_guard()
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
        _parent: u64,
        _title: &str,
        _text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        self.insert_widget(HarmonyHandleKind::MessageBox, _text, x, y, width, height)
    }
    fn create_file_dialog(&self, _parent: u64, _x: i32, _y: i32, width: u32, height: u32) -> u64 {
        self.insert_widget(
            HarmonyHandleKind::FileDialog,
            "FileDialog",
            _x,
            _y,
            width,
            height,
        )
    }
    fn create_color_dialog(&self, _parent: u64, _x: i32, _y: i32, width: u32, height: u32) -> u64 {
        self.insert_widget(
            HarmonyHandleKind::ColorDialog,
            "ColorDialog",
            _x,
            _y,
            width,
            height,
        )
    }
    fn create_font_dialog(&self, _parent: u64, _x: i32, _y: i32, width: u32, height: u32) -> u64 {
        self.insert_widget(
            HarmonyHandleKind::FontDialog,
            "FontDialog",
            _x,
            _y,
            width,
            height,
        )
    }
    fn create_spin_box(&self, _parent: u64, _x: i32, _y: i32, width: u32, height: u32) -> u64 {
        self.insert_widget(HarmonyHandleKind::SpinBox, "SpinBox", _x, _y, width, height)
    }
    fn create_list_view(&self, _parent: u64, _x: i32, _y: i32, width: u32, height: u32) -> u64 {
        self.insert_widget(
            HarmonyHandleKind::ListView,
            "ListView",
            _x,
            _y,
            width,
            height,
        )
    }
    fn create_scroll_area(&self, _parent: u64, _x: i32, _y: i32, width: u32, height: u32) -> u64 {
        self.insert_widget(
            HarmonyHandleKind::ScrollArea,
            "ScrollArea",
            _x,
            _y,
            width,
            height,
        )
    }
}
