//! iOS platform trait implementation.
//!
//! Implements the Platform contract for iOS mobile devices.
//! This is a state-driven backend that can be progressively enhanced
//! with native UIKit/SwiftUI bindings.
//!
//! ## UIKit Integration Path (BLUE11 R2.4)
//!
//! All widget creation methods (`create_window`, `create_button`, etc.)
//! currently delegate to the state backend (`IosMobilePlatform::insert_widget`)
//! which returns a monotonically increasing handle ID.
//!
//! To wire real UIKit views:
//!
//! 1. Check [`IosMobilePlatform::ui_kit_available()`] — returns `false` currently.
//! 2. When FFI is wired, each creation method should additionally spawn a real
//!    `UIView` / `UIButton` / `UILabel` etc. via `objc2` and store the pointer
//!    alongside the state handle.
//! 3. State operations (`set_widget_text`, `set_widget_geometry`, etc.) should
//!    first perform the Rust-side mutation, then forward the call to UIKit.
//! 4. All real FFI code should be feature-gated (`#[cfg(feature = "ios-uikit-ffi")]`)
//!    so the state-only backend remains the default for testing and CI.

use super::types::{IosHandleKind, IosMobilePlatform};
use crate::core::PlatformFamily;
use crate::platform::{DropEvent, Platform, WidgetTriggerEvent, WidgetTriggerKind};
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;

impl Platform for IosMobilePlatform {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn backend_name(&self) -> &'static str {
        "ios-state-backend"
    }

    fn family(&self) -> PlatformFamily {
        PlatformFamily::Mobile
    }

    fn init(&self) {
        let _ = self.ios_runtime_marker();
        self.runtime.initialized.store(true, Ordering::SeqCst);
    }

    fn run(&self) {
        if !self.runtime.initialized.load(Ordering::SeqCst) {
            self.init();
        }
        // iOS state backend uses polling loop for deterministic behavior.
        self.runtime.running.store(true, Ordering::SeqCst);
        while self.runtime.running.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(16));
        }
    }

    fn quit(&self) {
        self.runtime.running.store(false, Ordering::SeqCst);
    }

    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        let id = self.insert_widget(IosHandleKind::Window, title, x, y, width, height);

        #[cfg(feature = "ios-uikit-ffi")]
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            let window = super::native::create_ui_window(mtm, title, x, y, width, height);
            let ptr = objc2::rc::Retained::into_raw(window) as *mut std::ffi::c_void;
            super::native::store_native_view(id, ptr);
            // Windows are top-level; no subview addition needed.
        }

        id
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
        let id = self.insert_widget(IosHandleKind::Button, text, x, y, width, height);

        #[cfg(feature = "ios-uikit-ffi")]
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            let button = super::native::create_ui_button(mtm, text, x, y, width, height);
            let ptr = objc2::rc::Retained::into_raw(button) as *mut std::ffi::c_void;
            super::native::store_native_view(id, ptr);
            super::native::set_parent(id, parent);
            super::native::add_as_subview(id, parent);
            super::native::wire_button_action(id);
        }

        id
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
        let id = self.insert_widget(IosHandleKind::CheckBox, text, x, y, width, height);

        #[cfg(feature = "ios-uikit-ffi")]
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            let checkbox = super::native::create_ui_checkbox(mtm, text, x, y, width, height);
            let ptr = objc2::rc::Retained::into_raw(checkbox) as *mut std::ffi::c_void;
            super::native::store_native_view(id, ptr);
            super::native::set_parent(id, parent);
            super::native::add_as_subview(id, parent);
        }

        id
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
        let id = self.insert_widget(IosHandleKind::LineEdit, text, x, y, width, height);

        #[cfg(feature = "ios-uikit-ffi")]
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            let line_edit = super::native::create_ui_line_edit(mtm, text, x, y, width, height);
            let ptr = objc2::rc::Retained::into_raw(line_edit) as *mut std::ffi::c_void;
            super::native::store_native_view(id, ptr);
            super::native::set_parent(id, parent);
            super::native::add_as_subview(id, parent);
        }

        id
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
        let id = self.insert_widget(IosHandleKind::Label, text, x, y, width, height);

        #[cfg(feature = "ios-uikit-ffi")]
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            let label = super::native::create_ui_label(mtm, text, x, y, width, height);
            let ptr = objc2::rc::Retained::into_raw(label) as *mut std::ffi::c_void;
            super::native::store_native_view(id, ptr);
            super::native::set_parent(id, parent);
            super::native::add_as_subview(id, parent);
        }

        id
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
        let id = self.insert_widget(IosHandleKind::RadioButton, text, x, y, width, height);

        #[cfg(feature = "ios-uikit-ffi")]
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            let radio = super::native::create_ui_radio_button(mtm, text, x, y, width, height);
            let ptr = objc2::rc::Retained::into_raw(radio) as *mut std::ffi::c_void;
            super::native::store_native_view(id, ptr);
            super::native::set_parent(id, parent);
            super::native::add_as_subview(id, parent);
        }

        id
    }

    fn create_slider(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.insert_widget(IosHandleKind::Slider, "Slider", x, y, width, height);

        #[cfg(feature = "ios-uikit-ffi")]
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            let slider = super::native::create_ui_slider(mtm, x, y, width, height);
            let ptr = objc2::rc::Retained::into_raw(slider) as *mut std::ffi::c_void;
            super::native::store_native_view(id, ptr);
            super::native::set_parent(id, parent);
            super::native::add_as_subview(id, parent);
        }

        id
    }

    fn create_progress_bar(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.insert_widget(IosHandleKind::ProgressBar, "ProgressBar", x, y, width, height);

        #[cfg(feature = "ios-uikit-ffi")]
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            let progress = super::native::create_ui_progress_bar(mtm, x, y, width, height);
            let ptr = objc2::rc::Retained::into_raw(progress) as *mut std::ffi::c_void;
            super::native::store_native_view(id, ptr);
            super::native::set_parent(id, parent);
            super::native::add_as_subview(id, parent);
        }

        id
    }

    fn create_combo_box(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.insert_widget(IosHandleKind::ComboBox, "ComboBox", x, y, width, height);

        #[cfg(feature = "ios-uikit-ffi")]
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            let combo = super::native::create_ui_combo_box(mtm, x, y, width, height);
            let ptr = objc2::rc::Retained::into_raw(combo) as *mut std::ffi::c_void;
            super::native::store_native_view(id, ptr);
            super::native::set_parent(id, parent);
            super::native::add_as_subview(id, parent);
        }

        id
    }

    fn create_list_box(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.insert_widget(IosHandleKind::ListBox, "ListBox", x, y, width, height);

        #[cfg(feature = "ios-uikit-ffi")]
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            let list_box = super::native::create_ui_list_box(mtm, x, y, width, height);
            let ptr = objc2::rc::Retained::into_raw(list_box) as *mut std::ffi::c_void;
            super::native::store_native_view(id, ptr);
            super::native::set_parent(id, parent);
            super::native::add_as_subview(id, parent);
        }

        id
    }

    fn list_box_add_item(&self, list_box: u64, text: &str) -> bool {
        if !matches!(self.kind_of(list_box), Some(IosHandleKind::ListBox)) {
            return false;
        }
        let mut data = self.list_data.lock().expect("ios list data lock poisoned");
        let entry = data.entry(list_box).or_default();
        entry.items.push(text.to_string());
        true
    }

    fn list_box_remove_item(&self, list_box: u64, index: usize) -> bool {
        if !matches!(self.kind_of(list_box), Some(IosHandleKind::ListBox)) {
            return false;
        }
        let mut data = self.list_data.lock().expect("ios list data lock poisoned");
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
        if !matches!(self.kind_of(list_box), Some(IosHandleKind::ListBox)) {
            return false;
        }
        let mut data = self.list_data.lock().expect("ios list data lock poisoned");
        if let Some(entry) = data.get_mut(&list_box) {
            entry.items.clear();
            entry.current_index = None;
            true
        } else {
            false
        }
    }

    fn get_widget_text(&self, widget_id: u64) -> String {
        self.state.widget_text(widget_id)
    }

    fn set_widget_text(&self, widget_id: u64, text: &str) -> bool {
        self.state.set_widget_text(widget_id, text)
    }

    fn get_widget_geometry(&self, widget_id: u64) -> Option<(i32, i32, u32, u32)> {
        self.state.widget_geometry(widget_id)
    }

    fn set_widget_geometry(&self, widget_id: u64, x: i32, y: i32, width: u32, height: u32) -> bool {
        self.state.set_widget_geometry(widget_id, x, y, width, height)
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

    // ─── Panel ───

    fn create_panel(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.insert_widget(IosHandleKind::Panel, "Panel", x, y, width, height);

        #[cfg(feature = "ios-uikit-ffi")]
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            let panel = super::native::create_ui_panel(mtm, x, y, width, height);
            let ptr = objc2::rc::Retained::into_raw(panel) as *mut std::ffi::c_void;
            super::native::store_native_view(id, ptr);
            super::native::set_parent(id, parent);
            super::native::add_as_subview(id, parent);
        }

        id
    }

    // ─── Scroll Area ───

    fn create_scroll_area(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.insert_widget(IosHandleKind::Panel, "ScrollArea", x, y, width, height);

        #[cfg(feature = "ios-uikit-ffi")]
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            let scroll = super::native::create_ui_scroll(mtm, x, y, width, height);
            let ptr = objc2::rc::Retained::into_raw(scroll) as *mut std::ffi::c_void;
            super::native::store_native_view(id, ptr);
            super::native::set_parent(id, parent);
            super::native::add_as_subview(id, parent);
        }

        id
    }

    // ─── Menu Bar / Menu / Menu Item ───

    fn create_menu_bar(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.insert_widget(IosHandleKind::MenuBar, "MenuBar", x, y, width, height);
        // On iOS, menu bars are state-only (no native NSMenu equivalent).
        id
    }

    fn create_menu(&self, parent: u64, text: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.insert_widget(IosHandleKind::Menu, text, x, y, width, height);

        // Register child relationship in menu state.
        let mut menus = self.menus.lock().expect("ios menus lock poisoned");
        menus.menu_children.entry(parent).or_default().push(id);

        id
    }

    fn attach_menu_bar_to_window(&self, window: u64, menu_bar: u64) -> bool {
        if !matches!(self.kind_of(window), Some(IosHandleKind::Window)) {
            return false;
        }
        if !matches!(self.kind_of(menu_bar), Some(IosHandleKind::MenuBar)) {
            return false;
        }
        let mut menus = self.menus.lock().expect("ios menus lock poisoned");
        menus.attached_menu_bar.insert(window, menu_bar);
        true
    }

    fn menu_add_item(&self, parent_menu: u64, text: &str, _shortcut: Option<&str>) -> u64 {
        if self.kind_of(parent_menu).is_none() {
            return 0;
        }
        let id = self.insert_widget(IosHandleKind::MenuItem, text, 0, 0, 0, 0);

        let mut menus = self.menus.lock().expect("ios menus lock poisoned");
        menus.menu_children.entry(parent_menu).or_default().push(id);

        id
    }

    fn poll_menu_triggered(&self) -> Option<u64> {
        self.menus.lock().expect("ios menus lock poisoned").pending_menu_events.pop_front()
    }

    fn inject_menu_trigger(&self, menu_item_id: u64) -> bool {
        if self.kind_of(menu_item_id).is_none() {
            return false;
        }
        self.menus
            .lock()
            .expect("ios menus lock poisoned")
            .pending_menu_events
            .push_back(menu_item_id);
        true
    }

    // ─── Widget Trigger Events ───

    fn poll_widget_triggered(&self) -> Option<u64> {
        self.state.pop_widget_event().map(|event| event.widget_id)
    }

    fn poll_widget_trigger_event(&self) -> Option<WidgetTriggerEvent> {
        self.state.pop_widget_event()
    }

    fn inject_widget_trigger_event(&self, widget_id: u64, kind: WidgetTriggerKind) -> bool {
        if self.kind_of(widget_id).is_none() {
            return false;
        }
        self.state.push_widget_event(WidgetTriggerEvent { widget_id, kind });
        true
    }

    // ─── Tool Bar / Status Bar ───

    fn create_tool_bar(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.insert_widget(IosHandleKind::ToolBar, "ToolBar", x, y, width, height)
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
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.insert_widget(IosHandleKind::StatusBar, text, x, y, width, height)
    }

    // ─── Message Box ───

    fn create_message_box(
        &self,
        parent: u64,
        title: &str,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.insert_widget(IosHandleKind::MessageBox, text, x, y, width, height);

        #[cfg(feature = "ios-uikit-ffi")]
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            let alert = super::native::create_ui_alert(mtm, title, text);
            let ptr = objc2::rc::Retained::into_raw(alert) as *mut std::ffi::c_void;
            super::native::store_native_view(id, ptr);
            // Present the alert on the parent window's root view controller.
            if let Some(parent_ptr) = super::native::get_native_view(parent) {
                unsafe {
                    let parent_obj: *mut objc2::runtime::Object = parent_ptr as *mut _;
                    let root_vc: *mut objc2::runtime::Object =
                        msg_send![parent_obj, rootViewController];
                    if !root_vc.is_null() {
                        let _: () = msg_send![root_vc, presentViewController: &*alert animated: 1u8 completion: 0u64 as *mut objc2::runtime::Object];
                    }
                }
            }
        }

        id
    }

    // ─── Dialogs (state-only on iOS) ───

    fn create_file_dialog(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.insert_widget(IosHandleKind::FileDialog, "FileDialog", x, y, width, height)
    }

    fn create_color_dialog(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.insert_widget(IosHandleKind::ColorDialog, "ColorDialog", x, y, width, height)
    }

    fn create_font_dialog(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.insert_widget(IosHandleKind::FontDialog, "FontDialog", x, y, width, height)
    }

    // ─── Spin Box ───

    fn create_spin_box(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.insert_widget(IosHandleKind::LineEdit, "SpinBox", x, y, width, height)
    }

    // ─── List View ───

    fn create_list_view(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.insert_widget(IosHandleKind::ListBox, "ListView", x, y, width, height);

        #[cfg(feature = "ios-uikit-ffi")]
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            let list_view = super::native::create_ui_list_box(mtm, x, y, width, height);
            let ptr = objc2::rc::Retained::into_raw(list_view) as *mut std::ffi::c_void;
            super::native::store_native_view(id, ptr);
            super::native::set_parent(id, parent);
            super::native::add_as_subview(id, parent);
        }

        id
    }

    // ─── Show / Hide ───

    fn show_widget(&self, widget_id: u64) {
        self.state.set_visible(widget_id, true);
    }

    fn hide_widget(&self, widget_id: u64) {
        self.state.set_visible(widget_id, false);
    }

    // ─── Enabled / Visible ───

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

    // ─── Combo Box (state-backed) ───

    fn combo_box_add_item(&self, combo_box: u64, text: &str) -> bool {
        if !matches!(self.kind_of(combo_box), Some(IosHandleKind::ComboBox)) {
            return false;
        }
        let mut data = self.combo_data.lock().expect("ios combo data lock poisoned");
        let entry = data.entry(combo_box).or_default();
        entry.items.push(text.to_string());
        true
    }

    fn combo_box_clear_items(&self, combo_box: u64) -> bool {
        if !matches!(self.kind_of(combo_box), Some(IosHandleKind::ComboBox)) {
            return false;
        }
        let mut data = self.combo_data.lock().expect("ios combo data lock poisoned");
        if let Some(entry) = data.get_mut(&combo_box) {
            entry.items.clear();
            entry.current_index = None;
            true
        } else {
            false
        }
    }

    fn combo_box_set_current_index(&self, combo_box: u64, index: usize) -> bool {
        if !matches!(self.kind_of(combo_box), Some(IosHandleKind::ComboBox)) {
            return false;
        }
        let mut data = self.combo_data.lock().expect("ios combo data lock poisoned");
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
        let data = self.combo_data.lock().expect("ios combo data lock poisoned");
        data.get(&combo_box).and_then(|entry| entry.current_index)
    }

    fn combo_box_item_count(&self, combo_box: u64) -> usize {
        let data = self.combo_data.lock().expect("ios combo data lock poisoned");
        data.get(&combo_box).map(|entry| entry.items.len()).unwrap_or(0)
    }

    fn combo_box_item_text(&self, combo_box: u64, index: usize) -> Option<String> {
        let data = self.combo_data.lock().expect("ios combo data lock poisoned");
        data.get(&combo_box).and_then(|entry| entry.items.get(index).cloned())
    }

    // ─── Remaining List Box methods ───

    fn list_box_set_current_index(&self, list_box: u64, index: usize) -> bool {
        if !matches!(self.kind_of(list_box), Some(IosHandleKind::ListBox)) {
            return false;
        }
        let mut data = self.list_data.lock().expect("ios list data lock poisoned");
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
        let data = self.list_data.lock().expect("ios list data lock poisoned");
        data.get(&list_box).and_then(|entry| entry.current_index)
    }

    fn list_box_item_count(&self, list_box: u64) -> usize {
        let data = self.list_data.lock().expect("ios list data lock poisoned");
        data.get(&list_box).map(|entry| entry.items.len()).unwrap_or(0)
    }

    fn list_box_item_text(&self, list_box: u64, index: usize) -> Option<String> {
        let data = self.list_data.lock().expect("ios list data lock poisoned");
        data.get(&list_box).and_then(|entry| entry.items.get(index).cloned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ios_platform_window_creation() {
        let platform = IosMobilePlatform::new();
        platform.init();

        let window_id = platform.create_window("Test Window", 0, 0, 320, 568);
        assert_ne!(window_id, 0);

        assert_eq!(platform.backend_name(), "ios-state-backend");
        assert_eq!(platform.family(), PlatformFamily::Mobile);
    }

    #[test]
    fn ios_platform_button_requires_valid_parent() {
        let platform = IosMobilePlatform::new();
        platform.init();

        // Attempt to create button without valid parent should fail
        let button_id = platform.create_button(999, "Button", 0, 0, 80, 44);
        assert_eq!(button_id, 0);

        // Create window as parent
        let window_id = platform.create_window("Window", 0, 0, 320, 568);
        assert_ne!(window_id, 0);

        // Now button creation should succeed
        let button_id = platform.create_button(window_id, "Button", 0, 0, 80, 44);
        assert_ne!(button_id, 0);
    }

    #[test]
    fn ios_platform_list_box_items() {
        let platform = IosMobilePlatform::new();
        platform.init();

        let window_id = platform.create_window("Window", 0, 0, 320, 568);
        let list_box_id = platform.create_list_box(window_id, 0, 0, 320, 200);

        assert!(platform.list_box_add_item(list_box_id, "Item 1"));
        assert!(platform.list_box_add_item(list_box_id, "Item 2"));

        assert!(platform.list_box_remove_item(list_box_id, 0));
        assert!(platform.list_box_clear_items(list_box_id));
    }

    #[test]
    fn ios_platform_state_serialization() {
        let platform = IosMobilePlatform::new();
        platform.init();

        let _window_id = platform.create_window("Window", 0, 0, 320, 568);
        let result = platform.serialize_state();
        assert!(result.is_ok());
    }
}
