//! Android platform trait implementation.
//!
//! Implements the `Platform` contract for Android mobile devices.
//! This is a state-driven backend that can be progressively enhanced
//! with native Android views via JNI bindings.
//!
//! ## JNI Integration Path (android-jni feature)
//!
//! All widget creation methods (`create_window`, `create_button`, etc.)
//! currently delegate to the state backend (`AndroidPlatform::insert_widget`)
//! which returns a monotonically increasing handle ID.
//!
//! To wire real Android Views:
//!
//! 1. Check [`AndroidPlatform::jni_available()`] — returns `true` when
//!    the `android-jni` feature is enabled and `JAVA_VM` is initialized.
//! 2. When JNI is wired, each creation method should additionally call the
//!    corresponding JNI native method to create a real Android View and
//!    register it in the view registry.
//! 3. State operations (`set_widget_text`, `set_widget_geometry`, etc.) should
//!    first perform the Rust-side mutation, then forward the call to JNI.
//! 4. All real JNI code should be feature-gated (`#[cfg(feature = "android-jni")]`)
//!    so the state-only backend remains the default for testing and CI.

use super::types::{AndroidHandleKind, AndroidPlatform};
use crate::core::PlatformFamily;
use crate::platform::{DropEvent, Platform, WidgetTriggerEvent, WidgetTriggerKind};
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;

impl Platform for AndroidPlatform {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn backend_name(&self) -> &'static str {
        "android-state-backend"
    }

    fn family(&self) -> PlatformFamily {
        PlatformFamily::Mobile
    }

    fn init(&self) {
        let _ = self.android_runtime_marker();
        self.runtime.initialized.store(true, Ordering::SeqCst);
    }

    fn run(&self) {
        if !self.runtime.initialized.load(Ordering::SeqCst) {
            self.init();
        }
        // Android state backend uses polling loop for deterministic behavior.
        self.runtime.running.store(true, Ordering::SeqCst);
        while self.runtime.running.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(16));
        }
    }

    fn quit(&self) {
        self.runtime.running.store(false, Ordering::SeqCst);
    }

    // ─── Widget creation ─────────────────────────────────────────────────

    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        let id = self.insert_widget(AndroidHandleKind::Window, title, x, y, width, height);

        #[cfg(feature = "android-jni")]
        if self.jni_available() {
            // Future: call JNI to create Android Window/Dialog
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
        let id = self.insert_widget(AndroidHandleKind::Button, text, x, y, width, height);

        #[cfg(feature = "android-jni")]
        if self.jni_available() {
            crate::platform::android_jni::with_jni_env(|env| {
                // Future: call nativeCreateButton through JNI
                let _ = env;
            });
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
        let id = self.insert_widget(AndroidHandleKind::CheckBox, text, x, y, width, height);

        #[cfg(feature = "android-jni")]
        if self.jni_available() {
            // Future: call JNI nativeCreateCheckBox
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
        let id = self.insert_widget(AndroidHandleKind::LineEdit, text, x, y, width, height);

        #[cfg(feature = "android-jni")]
        if self.jni_available() {
            // Future: call JNI nativeCreateEditText
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
        let id = self.insert_widget(AndroidHandleKind::Label, text, x, y, width, height);

        #[cfg(feature = "android-jni")]
        if self.jni_available() {
            // Future: call JNI nativeCreateTextView
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
        let id = self.insert_widget(AndroidHandleKind::RadioButton, text, x, y, width, height);

        #[cfg(feature = "android-jni")]
        if self.jni_available() {
            // Future: call JNI nativeCreateRadioButton
        }

        id
    }

    fn create_slider(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.insert_widget(AndroidHandleKind::Slider, "Slider", x, y, width, height);

        #[cfg(feature = "android-jni")]
        if self.jni_available() {
            // Future: call JNI nativeCreateSeekBar
        }

        id
    }

    fn create_progress_bar(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id =
            self.insert_widget(AndroidHandleKind::ProgressBar, "ProgressBar", x, y, width, height);

        #[cfg(feature = "android-jni")]
        if self.jni_available() {
            // Future: call JNI nativeCreateProgressBar
        }

        id
    }

    fn create_combo_box(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.insert_widget(AndroidHandleKind::ComboBox, "ComboBox", x, y, width, height);

        #[cfg(feature = "android-jni")]
        if self.jni_available() {
            // Future: call JNI to create Spinner
        }

        id
    }

    fn combo_box_add_item(&self, combo_box: u64, text: &str) -> bool {
        if !matches!(self.kind_of(combo_box), Some(AndroidHandleKind::ComboBox)) {
            return false;
        }
        let mut data = self.list_data.lock().expect("android list data lock poisoned");
        let entry = data.entry(combo_box).or_default();
        entry.items.push(text.to_string());

        #[cfg(feature = "android-jni")]
        if self.jni_available() {
            // Future: add item to native Spinner adapter
        }

        true
    }

    fn combo_box_clear_items(&self, combo_box: u64) -> bool {
        if !matches!(self.kind_of(combo_box), Some(AndroidHandleKind::ComboBox)) {
            return false;
        }
        let mut data = self.list_data.lock().expect("android list data lock poisoned");
        if let Some(entry) = data.get_mut(&combo_box) {
            entry.items.clear();
            entry.current_index = None;
            true
        } else {
            false
        }
    }

    fn combo_box_set_current_index(&self, combo_box: u64, index: usize) -> bool {
        if !matches!(self.kind_of(combo_box), Some(AndroidHandleKind::ComboBox)) {
            return false;
        }
        let mut data = self.list_data.lock().expect("android list data lock poisoned");
        if let Some(entry) = data.get_mut(&combo_box) {
            if index < entry.items.len() {
                entry.current_index = Some(index);
                return true;
            }
        }
        false
    }

    fn combo_box_current_index(&self, combo_box: u64) -> Option<usize> {
        if !matches!(self.kind_of(combo_box), Some(AndroidHandleKind::ComboBox)) {
            return None;
        }
        let data = self.list_data.lock().expect("android list data lock poisoned");
        data.get(&combo_box).and_then(|entry| entry.current_index)
    }

    fn combo_box_item_count(&self, combo_box: u64) -> usize {
        if !matches!(self.kind_of(combo_box), Some(AndroidHandleKind::ComboBox)) {
            return 0;
        }
        let data = self.list_data.lock().expect("android list data lock poisoned");
        data.get(&combo_box).map(|entry| entry.items.len()).unwrap_or(0)
    }

    fn combo_box_item_text(&self, combo_box: u64, index: usize) -> Option<String> {
        if !matches!(self.kind_of(combo_box), Some(AndroidHandleKind::ComboBox)) {
            return None;
        }
        let data = self.list_data.lock().expect("android list data lock poisoned");
        data.get(&combo_box).and_then(|entry| entry.items.get(index).cloned())
    }

    fn create_list_box(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.insert_widget(AndroidHandleKind::ListBox, "ListBox", x, y, width, height);

        #[cfg(feature = "android-jni")]
        if self.jni_available() {
            // Future: call JNI to create ListView
        }

        id
    }

    fn list_box_add_item(&self, list_box: u64, text: &str) -> bool {
        if !matches!(self.kind_of(list_box), Some(AndroidHandleKind::ListBox)) {
            return false;
        }
        let mut data = self.list_data.lock().expect("android list data lock poisoned");
        let entry = data.entry(list_box).or_default();
        entry.items.push(text.to_string());
        true
    }

    fn list_box_remove_item(&self, list_box: u64, index: usize) -> bool {
        if !matches!(self.kind_of(list_box), Some(AndroidHandleKind::ListBox)) {
            return false;
        }
        let mut data = self.list_data.lock().expect("android list data lock poisoned");
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
        if !matches!(self.kind_of(list_box), Some(AndroidHandleKind::ListBox)) {
            return false;
        }
        let mut data = self.list_data.lock().expect("android list data lock poisoned");
        if let Some(entry) = data.get_mut(&list_box) {
            entry.items.clear();
            entry.current_index = None;
            true
        } else {
            false
        }
    }

    fn list_box_set_current_index(&self, list_box: u64, index: usize) -> bool {
        if !matches!(self.kind_of(list_box), Some(AndroidHandleKind::ListBox)) {
            return false;
        }
        let mut data = self.list_data.lock().expect("android list data lock poisoned");
        if let Some(entry) = data.get_mut(&list_box) {
            if index < entry.items.len() {
                entry.current_index = Some(index);
                return true;
            }
        }
        false
    }

    fn list_box_current_index(&self, list_box: u64) -> Option<usize> {
        if !matches!(self.kind_of(list_box), Some(AndroidHandleKind::ListBox)) {
            return None;
        }
        let data = self.list_data.lock().expect("android list data lock poisoned");
        data.get(&list_box).and_then(|entry| entry.current_index)
    }

    fn list_box_item_count(&self, list_box: u64) -> usize {
        if !matches!(self.kind_of(list_box), Some(AndroidHandleKind::ListBox)) {
            return 0;
        }
        let data = self.list_data.lock().expect("android list data lock poisoned");
        data.get(&list_box).map(|entry| entry.items.len()).unwrap_or(0)
    }

    fn list_box_item_text(&self, list_box: u64, index: usize) -> Option<String> {
        if !matches!(self.kind_of(list_box), Some(AndroidHandleKind::ListBox)) {
            return None;
        }
        let data = self.list_data.lock().expect("android list data lock poisoned");
        data.get(&list_box).and_then(|entry| entry.items.get(index).cloned())
    }

    fn create_panel(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.insert_widget(AndroidHandleKind::Panel, "Panel", x, y, width, height);

        #[cfg(feature = "android-jni")]
        if self.jni_available() {
            // Future: call JNI to create ViewGroup
        }

        id
    }

    fn create_menu_bar(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.insert_widget(AndroidHandleKind::MenuBar, "MenuBar", x, y, width, height);

        #[cfg(feature = "android-jni")]
        if self.jni_available() {
            // Future: call JNI to create menu bar
        }

        id
    }

    fn create_menu(&self, parent: u64, text: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.insert_widget(AndroidHandleKind::Menu, text, x, y, width, height);

        let mut menus = self.menus.lock().expect("android menus lock poisoned");
        menus.menu_children.entry(parent).or_default().push(id);

        #[cfg(feature = "android-jni")]
        if self.jni_available() {
            // Future: call JNI to create menu
        }

        id
    }

    fn attach_menu_bar_to_window(&self, window: u64, menu_bar: u64) -> bool {
        if !matches!(self.kind_of(window), Some(AndroidHandleKind::Window)) {
            return false;
        }
        if !matches!(self.kind_of(menu_bar), Some(AndroidHandleKind::MenuBar)) {
            return false;
        }
        let mut menus = self.menus.lock().expect("android menus lock poisoned");
        menus.attached_menu_bar.insert(window, menu_bar);
        true
    }

    fn menu_add_item(&self, parent_menu: u64, text: &str, shortcut: Option<&str>) -> u64 {
        if self.kind_of(parent_menu).is_none() {
            return 0;
        }
        let id = self.insert_widget(AndroidHandleKind::MenuItem, text, 0, 0, 0, 0);

        let display_text = match shortcut {
            Some(shortcut) => format!("{} ({})", text, shortcut),
            None => text.to_string(),
        };
        self.state.set_text(id, &display_text);

        let mut menus = self.menus.lock().expect("android menus lock poisoned");
        menus.menu_children.entry(parent_menu).or_default().push(id);

        #[cfg(feature = "android-jni")]
        if self.jni_available() {
            // Future: call JNI to add menu item
        }

        id
    }

    fn poll_menu_triggered(&self) -> Option<u64> {
        let mut menus = self.menus.lock().expect("android menus lock poisoned");
        menus.pending_menu_events.pop_front()
    }

    fn inject_menu_trigger(&self, menu_item_id: u64) -> bool {
        if !self.state.contains_widget(menu_item_id) {
            return false;
        }
        let mut menus = self.menus.lock().expect("android menus lock poisoned");
        menus.pending_menu_events.push_back(menu_item_id);
        true
    }

    fn poll_widget_triggered(&self) -> Option<u64> {
        self.state.pop_widget_trigger()
    }

    fn poll_widget_trigger_event(&self) -> Option<WidgetTriggerEvent> {
        self.state.pop_widget_trigger_event()
    }

    fn inject_widget_trigger_event(&self, widget_id: u64, kind: WidgetTriggerKind) -> bool {
        self.state.inject_widget_trigger_event(widget_id, kind)
    }

    fn create_tool_bar(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.insert_widget(AndroidHandleKind::ToolBar, "ToolBar", x, y, width, height);

        #[cfg(feature = "android-jni")]
        if self.jni_available() {
            // Future: call JNI to create Toolbar
        }

        id
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
        let id = self.insert_widget(AndroidHandleKind::StatusBar, text, x, y, width, height);

        #[cfg(feature = "android-jni")]
        if self.jni_available() {
            // Future: call JNI to create status bar view
        }

        id
    }

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
        let combined = format!("{}: {}", title, text);
        let id = self.insert_widget(AndroidHandleKind::MessageBox, &combined, x, y, width, height);

        #[cfg(feature = "android-jni")]
        if self.jni_available() {
            // Future: call JNI to create AlertDialog
        }

        id
    }

    fn create_file_dialog(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id =
            self.insert_widget(AndroidHandleKind::FileDialog, "FileDialog", x, y, width, height);

        #[cfg(feature = "android-jni")]
        if self.jni_available() {
            // Future: call JNI to create file picker
        }

        id
    }

    fn create_color_dialog(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id =
            self.insert_widget(AndroidHandleKind::ColorDialog, "ColorDialog", x, y, width, height);

        #[cfg(feature = "android-jni")]
        if self.jni_available() {
            // Future: call JNI to create color picker
        }

        id
    }

    fn create_font_dialog(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id =
            self.insert_widget(AndroidHandleKind::FontDialog, "FontDialog", x, y, width, height);

        #[cfg(feature = "android-jni")]
        if self.jni_available() {
            // Future: call JNI to create font picker
        }

        id
    }

    fn create_spin_box(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.insert_widget(AndroidHandleKind::SpinBox, "SpinBox", x, y, width, height);

        #[cfg(feature = "android-jni")]
        if self.jni_available() {
            // Future: call JNI to create NumberPicker
        }

        id
    }

    fn create_list_view(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.insert_widget(AndroidHandleKind::ListView, "ListView", x, y, width, height);

        #[cfg(feature = "android-jni")]
        if self.jni_available() {
            // Future: call JNI to create ListView
        }

        id
    }

    fn create_scroll_area(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id =
            self.insert_widget(AndroidHandleKind::ScrollArea, "ScrollArea", x, y, width, height);

        #[cfg(feature = "android-jni")]
        if self.jni_available() {
            // Future: call JNI to create ScrollView
        }

        id
    }

    // ─── Widget manipulation ─────────────────────────────────────────────

    fn show_widget(&self, widget_id: u64) {
        self.state.set_visible(widget_id, true);

        #[cfg(feature = "android-jni")]
        if self.jni_available() {
            crate::platform::android_jni::with_jni_env(|env| {
                // Future: call JNI nativeSetViewVisibility with VISIBLE
                let _ = env;
            });
        }
    }

    fn hide_widget(&self, widget_id: u64) {
        self.state.set_visible(widget_id, false);

        #[cfg(feature = "android-jni")]
        if self.jni_available() {
            crate::platform::android_jni::with_jni_env(|env| {
                // Future: call JNI nativeSetViewVisibility with GONE
                let _ = env;
            });
        }
    }

    fn set_widget_geometry(&self, widget_id: u64, x: i32, y: i32, width: u32, height: u32) {
        self.state.set_geometry(widget_id, x, y, width, height);

        #[cfg(feature = "android-jni")]
        if self.jni_available() {
            crate::platform::android_jni::with_jni_env(|env| {
                // Future: call JNI nativeSetViewBounds
                let _ = env;
            });
        }
    }

    fn set_widget_text(&self, widget_id: u64, text: &str) {
        self.state.set_text(widget_id, text);

        #[cfg(feature = "android-jni")]
        if self.jni_available() {
            crate::platform::android_jni::with_jni_env(|env| {
                // Future: call JNI nativeSetViewText
                let _ = env;
            });
        }
    }

    fn get_widget_text(&self, widget_id: u64) -> String {
        self.state.text(widget_id)
    }

    fn set_widget_enabled(&self, widget_id: u64, enabled: bool) {
        self.state.set_enabled(widget_id, enabled);

        #[cfg(feature = "android-jni")]
        if self.jni_available() {
            crate::platform::android_jni::with_jni_env(|env| {
                // Future: call JNI nativeSetViewEnabled
                let _ = env;
            });
        }
    }

    fn is_widget_enabled(&self, widget_id: u64) -> bool {
        self.state.enabled(widget_id)
    }

    fn set_widget_visible(&self, widget_id: u64, visible: bool) {
        self.state.set_visible(widget_id, visible);

        #[cfg(feature = "android-jni")]
        if self.jni_available() {
            crate::platform::android_jni::with_jni_env(|env| {
                // Future: call JNI nativeSetViewVisibility
                let _ = env;
            });
        }
    }

    fn is_widget_visible(&self, widget_id: u64) -> bool {
        self.state.visible(widget_id)
    }

    // ─── Clipboard ───────────────────────────────────────────────────────

    fn set_clipboard_text(&self, text: &str) -> bool {
        self.state.set_clipboard_text(text)
    }

    fn get_clipboard_text(&self) -> String {
        self.state.clipboard_text()
    }

    // ─── Drag and drop ───────────────────────────────────────────────────

    fn begin_drag(&self, source_widget_id: u64, mime: &str, payload: &[u8]) -> bool {
        self.state.begin_drag(source_widget_id, mime, payload)
    }

    fn poll_drop_event(&self) -> Option<DropEvent> {
        self.state.pop_drop_event()
    }

    fn inject_drop_event(&self, event: DropEvent) -> bool {
        self.state.inject_drop_event(event)
    }

    // ─── IME ─────────────────────────────────────────────────────────────

    fn set_widget_ime_enabled(&self, widget_id: u64, enabled: bool) -> bool {
        self.state.set_ime_enabled(widget_id, enabled)
    }

    fn is_widget_ime_enabled(&self, widget_id: u64) -> bool {
        self.state.ime_enabled(widget_id)
    }

    // ─── Accessibility ───────────────────────────────────────────────────

    fn set_widget_accessibility_name(&self, widget_id: u64, name: &str) -> bool {
        self.state.set_accessibility_name(widget_id, name)
    }

    fn get_widget_accessibility_name(&self, widget_id: u64) -> String {
        self.state.accessibility_name(widget_id)
    }
}

// ─── MobilePlatformExtension ─────────────────────────────────────────────

impl crate::platform::contract::MobilePlatformExtension for AndroidPlatform {
    fn mobile_backend(&self) -> crate::platform::MobileBackend {
        crate::platform::MobileBackend::Android
    }

    fn attach_to_native_view(&self, _native_handle: usize) -> bool {
        #[cfg(feature = "android-jni")]
        if self.jni_available() {
            // Future: store native view handle for rendering surface
            return true;
        }
        false
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn android_platform_window_creation() {
        let platform = AndroidPlatform::new();
        platform.init();

        let window_id = platform.create_window("Test Window", 0, 0, 320, 568);
        assert_ne!(window_id, 0);

        assert_eq!(platform.backend_name(), "android-state-backend");
        assert_eq!(platform.family(), PlatformFamily::Mobile);
    }

    #[test]
    fn android_platform_button_requires_valid_parent() {
        let platform = AndroidPlatform::new();
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
    fn android_platform_combo_box_items() {
        let platform = AndroidPlatform::new();
        platform.init();

        let window_id = platform.create_window("Window", 0, 0, 320, 568);
        assert_ne!(window_id, 0);

        let combo_id = platform.create_combo_box(window_id, 0, 0, 200, 40);
        assert_ne!(combo_id, 0);

        assert!(platform.combo_box_add_item(combo_id, "Item 1"));
        assert!(platform.combo_box_add_item(combo_id, "Item 2"));
        assert!(platform.combo_box_add_item(combo_id, "Item 3"));

        assert_eq!(platform.combo_box_item_count(combo_id), 3);
        assert_eq!(platform.combo_box_item_text(combo_id, 0), Some("Item 1".to_string()));
        assert!(platform.combo_box_set_current_index(combo_id, 1));
        assert_eq!(platform.combo_box_current_index(combo_id), Some(1));

        assert!(platform.combo_box_clear_items(combo_id));
        assert_eq!(platform.combo_box_item_count(combo_id), 0);
    }
}
