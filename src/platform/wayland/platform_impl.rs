//! Wayland backend platform implementation.
//!
//! This module implements the `Platform` trait for the Wayland backend.
//! All widget operations are backed by `BackendState<WaylandHandleKind>`.
//! Native Wayland protocol integration (via wayland-client / wayland-protocols)
//! will be wired in subsequent rounds.
//!
//! Architecture follows the same pattern as LinuxPlatform / HarmonyPlatform:
//! - State-only operations for all widget creation and lifecycle
//! - Thread-safe interior mutability
//! - Deterministic ID allocation via `insert_widget()`

use crate::core::ObjectId;
use crate::platform::types::{
    DropEvent, Platform, PlatformCapabilities, PlatformFamily, WidgetTriggerEvent,
    WidgetTriggerKind,
};
use crate::platform::wayland::types::{
    ListData, WaylandHandleKind, WaylandMenuState, WaylandPlatform, WaylandRuntimeState,
};

// ---------------------------------------------------------------------------
// Platform trait implementation
// ---------------------------------------------------------------------------

impl Platform for WaylandPlatform {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn backend_name(&self) -> &'static str {
        "wayland"
    }

    fn family(&self) -> PlatformFamily {
        PlatformFamily::Desktop
    }

    fn capabilities(&self) -> PlatformCapabilities {
        PlatformCapabilities {
            dpi_scaling: true,
            ime: true,
            accessibility: true,
            native_menu: true,
            typed_widget_trigger: true,
        }
    }

    fn dpi_scale_factor(&self) -> f32 {
        // TODO: Query wl_output scale factor via wayland-client when native integration is wired.
        1.0
    }

    // -----------------------------------------------------------------------
    // Initialization / lifecycle
    // -----------------------------------------------------------------------

    fn init(&self) {
        self.runtime
            .initialized
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    fn run(&self) {
        self.runtime
            .running
            .store(true, std::sync::atomic::Ordering::SeqCst);
        // TODO: Enter Wayland event loop dispatch (wl_display_dispatch) when native integration is wired.
        // For now, the state-only backend simply marks itself as running and returns.
    }

    fn quit(&self) {
        self.runtime
            .running
            .store(false, std::sync::atomic::Ordering::SeqCst);
    }

    // -----------------------------------------------------------------------
    // Widget creation
    // -----------------------------------------------------------------------

    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        self.insert_widget(WaylandHandleKind::Window, title, x, y, width, height)
    }

    fn create_button(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.insert_widget(WaylandHandleKind::Button, text, x, y, width, height)
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
        self.insert_widget(WaylandHandleKind::CheckBox, text, x, y, width, height)
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
        self.insert_widget(WaylandHandleKind::LineEdit, text, x, y, width, height)
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
        self.insert_widget(WaylandHandleKind::Label, text, x, y, width, height)
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
        self.insert_widget(WaylandHandleKind::RadioButton, text, x, y, width, height)
    }

    fn create_slider(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        self.insert_widget(WaylandHandleKind::Slider, "Slider", x, y, width, height)
    }

    fn create_progress_bar(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.insert_widget(
            WaylandHandleKind::ProgressBar,
            "ProgressBar",
            x,
            y,
            width,
            height,
        )
    }

    fn create_combo_box(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let id = self.insert_widget(WaylandHandleKind::ComboBox, "ComboBox", x, y, width, height);
        // Initialize empty list data for this combo box.
        if let Ok(mut data) = self.list_data.lock() {
            data.entry(id).or_insert_with(ListData::default);
        }
        id
    }

    fn create_list_box(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let id = self.insert_widget(WaylandHandleKind::ListBox, "ListBox", x, y, width, height);
        // Initialize empty list data for this list box.
        if let Ok(mut data) = self.list_data.lock() {
            data.entry(id).or_insert_with(ListData::default);
        }
        id
    }

    fn create_panel(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        self.insert_widget(WaylandHandleKind::Panel, "Panel", x, y, width, height)
    }

    fn create_menu_bar(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.insert_widget(WaylandHandleKind::MenuBar, "MenuBar", x, y, width, height)
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
        self.insert_widget(WaylandHandleKind::Menu, text, x, y, width, height)
    }

    fn create_tool_bar(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.insert_widget(WaylandHandleKind::ToolBar, "ToolBar", x, y, width, height)
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
        self.insert_widget(WaylandHandleKind::StatusBar, text, x, y, width, height)
    }

    // -----------------------------------------------------------------------
    // Dialogs and extended controls
    // -----------------------------------------------------------------------

    fn create_message_box(
        &self,
        _parent: ObjectId,
        _title: &str,
        _text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.insert_widget(WaylandHandleKind::MessageBox, _text, x, y, width, height)
    }

    fn create_file_dialog(
        &self,
        _parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.insert_widget(
            WaylandHandleKind::FileDialog,
            "FileDialog",
            x,
            y,
            width,
            height,
        )
    }

    fn create_color_dialog(
        &self,
        _parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.insert_widget(
            WaylandHandleKind::ColorDialog,
            "ColorDialog",
            x,
            y,
            width,
            height,
        )
    }

    fn create_font_dialog(
        &self,
        _parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.insert_widget(
            WaylandHandleKind::FontDialog,
            "FontDialog",
            x,
            y,
            width,
            height,
        )
    }

    fn create_spin_box(
        &self,
        _parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.insert_widget(WaylandHandleKind::SpinBox, "SpinBox", x, y, width, height)
    }

    fn create_list_view(
        &self,
        _parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.insert_widget(WaylandHandleKind::ListView, "ListView", x, y, width, height)
    }

    fn create_scroll_area(
        &self,
        _parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.insert_widget(
            WaylandHandleKind::ScrollArea,
            "ScrollArea",
            x,
            y,
            width,
            height,
        )
    }

    // -----------------------------------------------------------------------
    // Menu system
    // -----------------------------------------------------------------------

    fn attach_menu_bar_to_window(&self, window: ObjectId, menu_bar: ObjectId) -> bool {
        if let Ok(mut menus) = self.menus.lock() {
            menus.attached_menu_bar.insert(window, menu_bar);
            true
        } else {
            false
        }
    }

    fn menu_add_item(&self, parent_menu: ObjectId, text: &str, shortcut: Option<&str>) -> ObjectId {
        let display = if let Some(shortcut) = shortcut {
            format!("{}\t{}", text, shortcut)
        } else {
            text.to_string()
        };
        let id = self.insert_widget(WaylandHandleKind::MenuItem, &display, 0, 0, 0, 0);
        if let Ok(mut menus) = self.menus.lock() {
            menus.menu_children.entry(parent_menu).or_default().push(id);
        }
        id
    }

    fn poll_menu_triggered(&self) -> Option<ObjectId> {
        if let Ok(mut menus) = self.menus.lock() {
            menus.pending_menu_events.pop_front()
        } else {
            None
        }
    }

    fn inject_menu_trigger(&self, menu_item_id: ObjectId) -> bool {
        if !self.state.contains_widget(menu_item_id) {
            return false;
        }
        if let Ok(mut menus) = self.menus.lock() {
            menus.pending_menu_events.push_back(menu_item_id);
            true
        } else {
            false
        }
    }

    // -----------------------------------------------------------------------
    // Widget trigger events
    // -----------------------------------------------------------------------

    fn poll_widget_triggered(&self) -> Option<ObjectId> {
        if let Ok(mut menus) = self.menus.lock() {
            menus.pending_widget_events.pop_front().map(|e| e.widget_id)
        } else {
            None
        }
    }

    fn poll_widget_trigger_event(&self) -> Option<WidgetTriggerEvent> {
        if let Ok(mut menus) = self.menus.lock() {
            menus.pending_widget_events.pop_front()
        } else {
            None
        }
    }

    fn inject_widget_trigger_event(&self, widget_id: ObjectId, kind: WidgetTriggerKind) -> bool {
        if !self.state.contains_widget(widget_id) {
            return false;
        }
        if let Ok(mut menus) = self.menus.lock() {
            menus
                .pending_widget_events
                .push_back(WidgetTriggerEvent { widget_id, kind });
            true
        } else {
            false
        }
    }

    // -----------------------------------------------------------------------
    // Widget lifecycle operations
    // -----------------------------------------------------------------------

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

    // -----------------------------------------------------------------------
    // IME / Accessibility
    // -----------------------------------------------------------------------

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

    // -----------------------------------------------------------------------
    // Clipboard
    // -----------------------------------------------------------------------

    fn set_clipboard_text(&self, text: &str) -> bool {
        self.state.set_clipboard_text(text)
    }

    fn get_clipboard_text(&self) -> String {
        self.state.clipboard_text()
    }

    // -----------------------------------------------------------------------
    // Drag and drop
    // -----------------------------------------------------------------------

    fn begin_drag(&self, source_widget_id: ObjectId, mime: &str, payload: &[u8]) -> bool {
        self.state.begin_drag(source_widget_id, mime, payload)
    }

    fn poll_drop_event(&self) -> Option<DropEvent> {
        self.state.pop_drop_event()
    }

    fn inject_drop_event(&self, event: DropEvent) -> bool {
        self.state.inject_drop_event(event)
    }

    // -----------------------------------------------------------------------
    // ComboBox data methods
    // -----------------------------------------------------------------------

    fn combo_box_add_item(&self, combo_box: ObjectId, text: &str) -> bool {
        if let Ok(mut data) = self.list_data.lock() {
            if let Some(list) = data.get_mut(&combo_box) {
                list.items.push(text.to_string());
                return true;
            }
        }
        false
    }

    fn combo_box_clear_items(&self, combo_box: ObjectId) -> bool {
        if let Ok(mut data) = self.list_data.lock() {
            if let Some(list) = data.get_mut(&combo_box) {
                list.items.clear();
                list.current_index = None;
                return true;
            }
        }
        false
    }

    fn combo_box_set_current_index(&self, combo_box: ObjectId, index: usize) -> bool {
        if let Ok(mut data) = self.list_data.lock() {
            if let Some(list) = data.get_mut(&combo_box) {
                if index < list.items.len() {
                    let previous = list.current_index;
                    list.current_index = Some(index);
                    // Fire selection changed trigger event.
                    if previous != Some(index) {
                        if let Ok(mut menus) = self.menus.lock() {
                            menus.pending_widget_events.push_back(WidgetTriggerEvent {
                                widget_id: combo_box,
                                kind: WidgetTriggerKind::SelectionChanged,
                            });
                        }
                    }
                    return true;
                }
            }
        }
        false
    }

    fn combo_box_current_index(&self, combo_box: ObjectId) -> Option<usize> {
        if let Ok(data) = self.list_data.lock() {
            data.get(&combo_box).and_then(|list| list.current_index)
        } else {
            None
        }
    }

    fn combo_box_item_count(&self, combo_box: ObjectId) -> usize {
        if let Ok(data) = self.list_data.lock() {
            data.get(&combo_box)
                .map(|list| list.items.len())
                .unwrap_or(0)
        } else {
            0
        }
    }

    fn combo_box_item_text(&self, combo_box: ObjectId, index: usize) -> Option<String> {
        if let Ok(data) = self.list_data.lock() {
            data.get(&combo_box)
                .and_then(|list| list.items.get(index).cloned())
        } else {
            None
        }
    }

    // -----------------------------------------------------------------------
    // ListBox data methods
    // -----------------------------------------------------------------------

    fn list_box_add_item(&self, list_box: ObjectId, text: &str) -> bool {
        if let Ok(mut data) = self.list_data.lock() {
            if let Some(list) = data.get_mut(&list_box) {
                list.items.push(text.to_string());
                return true;
            }
        }
        false
    }

    fn list_box_remove_item(&self, list_box: ObjectId, index: usize) -> bool {
        if let Ok(mut data) = self.list_data.lock() {
            if let Some(list) = data.get_mut(&list_box) {
                if index < list.items.len() {
                    list.items.remove(index);
                    // Adjust current index if needed.
                    if let Some(cur) = list.current_index {
                        if cur == index {
                            if list.items.is_empty() {
                                list.current_index = None;
                            } else if cur >= list.items.len() {
                                list.current_index = Some(list.items.len() - 1);
                            }
                        } else if cur > index {
                            list.current_index = Some(cur - 1);
                        }
                    }
                    return true;
                }
            }
        }
        false
    }

    fn list_box_clear_items(&self, list_box: ObjectId) -> bool {
        if let Ok(mut data) = self.list_data.lock() {
            if let Some(list) = data.get_mut(&list_box) {
                list.items.clear();
                list.current_index = None;
                return true;
            }
        }
        false
    }

    fn list_box_set_current_index(&self, list_box: ObjectId, index: usize) -> bool {
        if let Ok(mut data) = self.list_data.lock() {
            if let Some(list) = data.get_mut(&list_box) {
                if index < list.items.len() {
                    let previous = list.current_index;
                    list.current_index = Some(index);
                    // Fire selection changed trigger event.
                    if previous != Some(index) {
                        if let Ok(mut menus) = self.menus.lock() {
                            menus.pending_widget_events.push_back(WidgetTriggerEvent {
                                widget_id: list_box,
                                kind: WidgetTriggerKind::SelectionChanged,
                            });
                        }
                    }
                    return true;
                }
            }
        }
        false
    }

    fn list_box_current_index(&self, list_box: ObjectId) -> Option<usize> {
        if let Ok(data) = self.list_data.lock() {
            data.get(&list_box).and_then(|list| list.current_index)
        } else {
            None
        }
    }

    fn list_box_item_count(&self, list_box: ObjectId) -> usize {
        if let Ok(data) = self.list_data.lock() {
            data.get(&list_box)
                .map(|list| list.items.len())
                .unwrap_or(0)
        } else {
            0
        }
    }

    fn list_box_item_text(&self, list_box: ObjectId, index: usize) -> Option<String> {
        if let Ok(data) = self.list_data.lock() {
            data.get(&list_box)
                .and_then(|list| list.items.get(index).cloned())
        } else {
            None
        }
    }
}
