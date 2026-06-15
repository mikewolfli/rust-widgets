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
use crate::platform::{DropEvent, Platform, WidgetTriggerEvent};
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
