// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! iOS platform trait implementation.
//!
//! Implements the Platform contract for iOS mobile devices.
//! This is a state-driven backend that can be progressively enhanced
//! with native UIKit/SwiftUI bindings.
//!
//! ## Menu / status-bar semantics (iOS)
//!
//! iOS has no desktop `MenuBar`/`StatusBar` chrome. These handles are modelled
//! as **in-process data**: kind-constrained parents, textual payload, and
//! injectable trigger events. They are intentionally *not* advertised as native
//! menu capability (`capabilities().native_menu == false`) and never attached to
//! a UIKit view. Apps that need UIKit contextual menus should build them from the
//! same data instead of treating these handles as native menus.
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
//! 4. All real FFI code should be feature-gated (`#[cfg(all(target_os = "ios", feature = "ios-uikit-ffi"))]`)
//!    so the state-only backend remains the default for testing and CI.

use super::types::{IosHandleKind, IosMobilePlatform};
use crate::core::{ObjectId, PlatformFamily};
use crate::platform::{
    DropEvent, Platform, PlatformCapabilities, WidgetTriggerEvent, WidgetTriggerKind,
};
#[cfg(all(target_os = "ios", feature = "ios-uikit-ffi"))]
use objc2::msg_send;
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

    /// iOS exposes installed RAM through `sysctl hw.memsize`; the iOS runtime is
    /// an XNU kernel and provides the same sysctl. Returns `None` when the sandbox
    /// denies the call so callers keep their conservative default.
    fn total_memory_mb(&self) -> Option<u64> {
        let output =
            std::process::Command::new("sysctl").args(["-n", "hw.memsize"]).output().ok()?;
        if !output.status.success() {
            return None;
        }
        let bytes = String::from_utf8_lossy(&output.stdout).trim().parse::<u64>().ok()?;
        Some(bytes / (1024 * 1024))
    }

    /// iPhones and iPads run on battery by definition.
    fn is_on_battery(&self) -> bool {
        true
    }

    /// Process memory accounting is not reachable from the app sandbox through a
    /// stable interface here, so this backend honestly reports `None`.
    fn process_memory_utilization(&self) -> Option<f32> {
        None
    }

    /// CPU accounting likewise has no portable sandbox-visible source here.
    fn process_cpu_utilization(&self) -> Option<f32> {
        None
    }

    /// iOS printing goes through `UIPrintInteractionController`, not a spooler
    /// command, so this state backend cannot submit a job file.
    fn spawn_print_job(&self, _job_file: &std::path::Path) -> Result<(), String> {
        Err("iOS printing requires UIPrintInteractionController (not bound)".to_string())
    }

    /// iOS uses the same AppKit-style accelerator symbols as macOS (`⌘⇧Z`).
    fn shortcut_style(&self) -> crate::shortcut::PlatformShortcutStyle {
        crate::shortcut::PlatformShortcutStyle::Mac
    }

    #[cfg(feature = "mobile-api")]
    fn mobile_extension(&self) -> Option<&dyn crate::platform::types::MobilePlatformExtension> {
        Some(self)
    }

    fn capabilities(&self) -> PlatformCapabilities {
        PlatformCapabilities {
            dpi_scaling: true,
            ime: true,
            accessibility: true,
            native_menu: false,
            typed_widget_trigger: true,
        }
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

    fn destroy_widget(&self, widget_id: u64) -> bool {
        // The kind is only consulted for the native registries, which exist only
        // on the UIKit FFI path; the state record carries the authority for the
        // return value on every configuration.
        #[cfg(all(target_os = "ios", feature = "ios-uikit-ffi"))]
        let kind = self.kind_of(widget_id);
        let existed = self.state.destroy_widget(widget_id);

        // Release the native UIKit registries. `remove_native_view` drops the raw
        // view pointer and the child-to-parent map entry; buttons additionally own
        // a retained `ButtonTarget` that UIKit does not keep alive for us.
        #[cfg(all(target_os = "ios", feature = "ios-uikit-ffi"))]
        {
            if matches!(kind, Some(IosHandleKind::Button | IosHandleKind::RadioButton)) {
                super::native::remove_button_target(widget_id);
            }
            super::native::remove_native_view(widget_id);
        }

        // Drop the per-widget list storage for both list-backed widgets. Each lock
        // is released at the end of its statement so no two guards are held at once.
        self.list_data.lock().expect("ios list data lock poisoned").remove(&widget_id);
        self.combo_data.lock().expect("ios combo data lock poisoned").remove(&widget_id);

        // Drop the menu bookkeeping that names this widget: attached menu-bar
        // ownership, membership in a parent menu's child list, and any queued
        // trigger that would otherwise fire for a widget that no longer exists.
        let mut menus = self.menus.lock().expect("ios menus lock poisoned");
        menus.attached_menu_bar.retain(|_window, menu_bar| *menu_bar != widget_id);
        let destroyed_children = menus.menu_children.remove(&widget_id);
        menus.menu_children.retain(|_parent, children| {
            children.retain(|child| *child != widget_id);
            !children.is_empty()
        });
        menus.pending_menu_events.retain(|queued| *queued != widget_id);
        drop(menus);

        // A destroyed `Menu` owns child menu items that were only reachable
        // through it; cascade the teardown so those items are not orphaned.
        for child in destroyed_children.into_iter().flatten() {
            self.destroy_widget(child);
        }

        existed
    }

    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        let id = self.insert_widget(IosHandleKind::Window, title, x, y, width, height);

        #[cfg(all(target_os = "ios", feature = "ios-uikit-ffi"))]
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

        #[cfg(all(target_os = "ios", feature = "ios-uikit-ffi"))]
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

        #[cfg(all(target_os = "ios", feature = "ios-uikit-ffi"))]
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

        #[cfg(all(target_os = "ios", feature = "ios-uikit-ffi"))]
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

        #[cfg(all(target_os = "ios", feature = "ios-uikit-ffi"))]
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

        #[cfg(all(target_os = "ios", feature = "ios-uikit-ffi"))]
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

        #[cfg(all(target_os = "ios", feature = "ios-uikit-ffi"))]
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

        #[cfg(all(target_os = "ios", feature = "ios-uikit-ffi"))]
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

        #[cfg(all(target_os = "ios", feature = "ios-uikit-ffi"))]
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

        #[cfg(all(target_os = "ios", feature = "ios-uikit-ffi"))]
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
        self.state.text(widget_id)
    }

    fn set_widget_text(&self, widget_id: u64, text: &str) {
        let _ = self.state.set_text(widget_id, text);
        #[cfg(all(target_os = "ios", feature = "ios-uikit-ffi"))]
        super::native::set_native_text(widget_id, text);
    }

    fn set_widget_geometry(&self, widget_id: u64, x: i32, y: i32, width: u32, height: u32) {
        self.state.set_geometry(widget_id, x, y, width, height);
        #[cfg(all(target_os = "ios", feature = "ios-uikit-ffi"))]
        super::native::set_native_frame(widget_id, x, y, width, height);
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

        #[cfg(all(target_os = "ios", feature = "ios-uikit-ffi"))]
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
        let id = self.insert_widget(IosHandleKind::ScrollArea, "ScrollArea", x, y, width, height);

        #[cfg(all(target_os = "ios", feature = "ios-uikit-ffi"))]
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            let scroll = super::native::create_ui_scroll(mtm, x, y, width, height);
            let ptr = objc2::rc::Retained::into_raw(scroll) as *mut std::ffi::c_void;
            super::native::store_native_view(id, ptr);
            super::native::set_parent(id, parent);
            super::native::add_as_subview(id, parent);
        }

        id
    }
    fn create_group_box(
        &self,
        parent: ObjectId,
        title: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(IosHandleKind::GroupBox, title, x, y, width, height)
    }
    fn create_frame(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(IosHandleKind::Frame, "Frame", x, y, width, height)
    }
    fn create_tab_widget(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(IosHandleKind::TabWidget, "TabWidget", x, y, width, height)
    }
    fn create_splitter(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(IosHandleKind::Splitter, "Splitter", x, y, width, height)
    }
    fn create_toggle_button(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(IosHandleKind::ToggleButton, text, x, y, width, height)
    }
    fn create_calendar(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(IosHandleKind::Calendar, "Calendar", x, y, width, height)
    }
    fn create_scroll_bar(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(IosHandleKind::ScrollBar, "ScrollBar", x, y, width, height)
    }
    fn create_double_spin_box(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(IosHandleKind::DoubleSpinBox, "DoubleSpinBox", x, y, width, height)
    }
    fn create_font_combo_box(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(IosHandleKind::FontComboBox, "FontComboBox", x, y, width, height)
    }
    fn create_context_menu(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(IosHandleKind::ContextMenu, "ContextMenu", x, y, width, height)
    }
    fn create_popup_window(
        &self,
        parent: ObjectId,
        title: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(IosHandleKind::PopupWindow, title, x, y, width, height)
    }
    fn create_dialog(
        &self,
        parent: ObjectId,
        title: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(IosHandleKind::Dialog, title, x, y, width, height)
    }
    fn create_input_dialog(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(IosHandleKind::InputDialog, "Input", x, y, width, height)
    }
    fn create_progress_dialog(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(IosHandleKind::ProgressDialog, "Progress", x, y, width, height)
    }
    fn create_directory_dialog(
        &self,
        parent: ObjectId,
        title: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(IosHandleKind::DirectoryDialog, title, x, y, width, height)
    }
    fn create_date_picker(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(IosHandleKind::DatePicker, "DatePicker", x, y, width, height)
    }
    fn create_time_picker(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(IosHandleKind::TimePicker, "TimePicker", x, y, width, height)
    }
    fn create_date_time_picker(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(
            IosHandleKind::DateTimePicker,
            "DateTimePicker",
            x,
            y,
            width,
            height,
        )
    }
    fn create_activity_indicator(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(
            IosHandleKind::ActivityIndicator,
            "ActivityIndicator",
            x,
            y,
            width,
            height,
        )
    }

    // ─── Menu Bar / Menu / Menu Item ───
    //
    // iOS has no desktop menu chrome. These handles are an in-process data
    // model: MenuBar is owned by a Window, Menu belongs to a MenuBar/Menu, and
    // MenuItem belongs to a Menu. Triggers are delivered through the injectable
    // `pending_menu_events` queue; `native_menu` capability stays false.

    fn create_menu_bar(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if !matches!(self.kind_of(parent), Some(IosHandleKind::Window)) {
            return 0;
        }
        self.insert_widget(IosHandleKind::MenuBar, "MenuBar", x, y, width, height)
    }

    fn create_menu(&self, parent: u64, text: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if !matches!(self.kind_of(parent), Some(IosHandleKind::MenuBar | IosHandleKind::Menu)) {
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
        if !matches!(self.kind_of(parent_menu), Some(IosHandleKind::Menu)) {
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
        if !matches!(self.kind_of(menu_item_id), Some(IosHandleKind::MenuItem)) {
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
        self.poll_widget_trigger_event().map(|event| event.widget_id)
    }

    fn poll_widget_trigger_event(&self) -> Option<WidgetTriggerEvent> {
        #[cfg(all(target_os = "ios", feature = "ios-uikit-ffi"))]
        {
            for widget_id in super::native::drain_button_events() {
                self.state.push_widget_event(WidgetTriggerEvent {
                    widget_id,
                    kind: WidgetTriggerKind::Clicked,
                });
            }
        }
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
        if !matches!(self.kind_of(parent), Some(IosHandleKind::Window)) {
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
        if !matches!(self.kind_of(parent), Some(IosHandleKind::Window)) {
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
        // `title` is consumed by the UIKit alert on the FFI path only; the pure
        // state path records `text` and ignores it.
        #[cfg(not(all(target_os = "ios", feature = "ios-uikit-ffi")))]
        let _ = title;
        let id = self.insert_widget(IosHandleKind::MessageBox, text, x, y, width, height);

        #[cfg(all(target_os = "ios", feature = "ios-uikit-ffi"))]
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            let alert = super::native::create_ui_alert(mtm, title, text);
            // Present the alert on the parent window's root view controller.
            if let Some(parent_ptr) = super::native::get_native_view(parent) {
                unsafe {
                    let parent_obj: *mut objc2::runtime::AnyObject = parent_ptr as *mut _;
                    let root_vc: *mut objc2::runtime::AnyObject =
                        msg_send![parent_obj, rootViewController];
                    if !root_vc.is_null() {
                        let _: () = msg_send![root_vc, presentViewController: &*alert, animated: 1u8, completion: std::ptr::null_mut::<objc2::runtime::AnyObject>()];
                    }
                }
            }
            let ptr = &*alert as *const _ as *mut std::ffi::c_void;
            super::native::store_native_view(id, ptr);
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
        self.insert_widget(IosHandleKind::SpinBox, "SpinBox", x, y, width, height)
    }

    // ─── List View ───

    fn create_list_view(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.insert_widget(IosHandleKind::ListView, "ListView", x, y, width, height);

        #[cfg(all(target_os = "ios", feature = "ios-uikit-ffi"))]
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
        #[cfg(all(target_os = "ios", feature = "ios-uikit-ffi"))]
        super::native::set_native_hidden(widget_id, false);
    }

    fn hide_widget(&self, widget_id: u64) {
        self.state.set_visible(widget_id, false);
        #[cfg(all(target_os = "ios", feature = "ios-uikit-ffi"))]
        super::native::set_native_hidden(widget_id, true);
    }

    // ─── Enabled / Visible ───

    fn set_widget_enabled(&self, widget_id: u64, enabled: bool) {
        self.state.set_enabled(widget_id, enabled);
        #[cfg(all(target_os = "ios", feature = "ios-uikit-ffi"))]
        super::native::set_native_enabled(widget_id, enabled);
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

    #[cfg(all(
        feature = "serde_json",
        feature = "serde",
        not(any(feature = "mini", feature = "embedded"))
    ))]
    #[test]
    fn ios_platform_state_serialization() {
        let platform = IosMobilePlatform::new();
        platform.init();

        let _window_id = platform.create_window("Window", 0, 0, 320, 568);
        let result = platform.serialize_state();
        assert!(result.is_ok());
    }

    #[test]
    fn ios_platform_reports_explicit_mobile_capabilities() {
        let platform = IosMobilePlatform::new();
        let caps = platform.capabilities();

        assert_eq!(platform.family(), PlatformFamily::Mobile);
        assert!(caps.dpi_scaling);
        assert!(caps.ime);
        assert!(caps.accessibility);
        assert!(!caps.native_menu);
        assert!(caps.typed_widget_trigger);
    }

    #[test]
    fn ios_platform_preserves_semantic_kinds_for_extended_controls() {
        let platform = IosMobilePlatform::new();
        platform.init();
        let window = platform.create_window("Window", 0, 0, 320, 568);

        let spin = platform.create_spin_box(window, 0, 0, 80, 44);
        let list_view = platform.create_list_view(window, 0, 50, 320, 120);
        let scroll = platform.create_scroll_area(window, 0, 180, 320, 120);

        assert_eq!(platform.kind_of(spin), Some(IosHandleKind::SpinBox));
        assert_eq!(platform.kind_of(list_view), Some(IosHandleKind::ListView));
        assert_eq!(platform.kind_of(scroll), Some(IosHandleKind::ScrollArea));
    }
}
