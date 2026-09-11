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
use crate::core::{ObjectId, PlatformFamily};
#[cfg(feature = "android-jni")]
use crate::platform::android_jni::AndroidViewClass;
use crate::platform::{DropEvent, Platform, WidgetTriggerEvent, WidgetTriggerKind};
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;

impl AndroidPlatform {
    /// Map a logical handle kind to the Android view class used for its native
    /// counterpart, or `None` for kinds with no single-view equivalent.
    #[cfg(feature = "android-jni")]
    fn view_class_for(kind: AndroidHandleKind) -> Option<AndroidViewClass> {
        use crate::platform::android_jni::AndroidLogicalKind as L;
        let logical = match kind {
            AndroidHandleKind::Window => L::Window,
            AndroidHandleKind::Button => L::Button,
            AndroidHandleKind::CheckBox => L::CheckBox,
            AndroidHandleKind::LineEdit => L::LineEdit,
            AndroidHandleKind::Label => L::Label,
            AndroidHandleKind::RadioButton => L::RadioButton,
            AndroidHandleKind::Slider => L::Slider,
            AndroidHandleKind::ProgressBar => L::ProgressBar,
            AndroidHandleKind::ComboBox => L::ComboBox,
            AndroidHandleKind::ListBox => L::ListBox,
            AndroidHandleKind::Panel => L::Panel,
            AndroidHandleKind::MenuBar => L::MenuBar,
            AndroidHandleKind::Menu => L::Menu,
            AndroidHandleKind::MenuItem => L::MenuItem,
            AndroidHandleKind::ToolBar => L::ToolBar,
            AndroidHandleKind::StatusBar => L::StatusBar,
            AndroidHandleKind::MessageBox => L::MessageBox,
            AndroidHandleKind::FileDialog => L::FileDialog,
            AndroidHandleKind::ColorDialog => L::ColorDialog,
            AndroidHandleKind::FontDialog => L::FontDialog,
            AndroidHandleKind::SpinBox => L::SpinBox,
            AndroidHandleKind::ListView => L::ListView,
            AndroidHandleKind::ScrollArea => L::ScrollArea,
            // Container / input kinds added for the Round-2/3 native work.
            // Android has no single-View equivalent for these, so they map to
            // the logical kinds that `view_class_for` resolves to `None`; the
            // hybrid route keeps them on the self-drawn backend.
            AndroidHandleKind::GroupBox => L::Panel,
            AndroidHandleKind::Frame => L::Panel,
            AndroidHandleKind::TabWidget => L::Panel,
            AndroidHandleKind::Splitter => L::Panel,
            AndroidHandleKind::ToggleButton => L::CheckBox,
            AndroidHandleKind::Calendar
            | AndroidHandleKind::ScrollBar
            | AndroidHandleKind::DoubleSpinBox
            | AndroidHandleKind::FontComboBox
            | AndroidHandleKind::ContextMenu
            | AndroidHandleKind::PopupWindow
            | AndroidHandleKind::InputDialog
            | AndroidHandleKind::ProgressDialog
            | AndroidHandleKind::DirectoryDialog
            | AndroidHandleKind::DatePicker
            | AndroidHandleKind::TimePicker
            | AndroidHandleKind::DateTimePicker
            | AndroidHandleKind::ActivityIndicator => return None,
            // A generic dialog has no single View equivalent; it maps onto the
            // logical Dialog kind, which `view_class_for` resolves to None.
            AndroidHandleKind::Dialog => return None,
        };
        crate::platform::android_jni::view_class_for(logical)
    }

    /// Register a native Android view for a freshly created logical widget.
    ///
    /// No-op (returning `None`) when the bridge is uninitialized or the kind has
    /// no single-view equivalent, so the state backend stays authoritative.
    #[cfg(feature = "android-jni")]
    fn attach_view(
        &self,
        logical_id: u64,
        kind: AndroidHandleKind,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> Option<crate::core::ObjectId> {
        let class = Self::view_class_for(kind)?;
        self.attach_native_view(logical_id, class, text, x, y, width, height)
    }

    /// Create a logical widget and, when JNI is available, its native view.
    ///
    /// This exists in both feature configurations: without `android-jni` it is
    /// identical to [`Self::insert_widget`], which keeps every `create_*` method
    /// a single expression and avoids duplicated `#[cfg]` blocks.
    #[cfg(feature = "android-jni")]
    fn create_with_native(
        &self,
        kind: AndroidHandleKind,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        let id = self.insert_widget(kind, text, x, y, width, height);
        self.attach_view(id, kind, text, x, y, width, height);
        id
    }

    #[cfg(not(feature = "android-jni"))]
    fn create_with_native(
        &self,
        kind: AndroidHandleKind,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        self.insert_widget(kind, text, x, y, width, height)
    }

    /// Create a combo box or list box: a logical widget plus its native view,
    /// and an entry in the shared list-data table.
    fn create_with_list_data(
        &self,
        kind: AndroidHandleKind,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        let id = self.create_with_native(kind, text, x, y, width, height);
        self.list_data.lock().expect("android list data lock poisoned").entry(id).or_default();
        id
    }

    /// Whether a logical kind is backed by an `AlertDialog` rather than a `View`.
    ///
    /// Dialogs implement `show()`/`dismiss()`/`setMessage()` instead of the
    /// `View` visibility/text API, so the setters must dispatch on this.
    #[cfg(feature = "android-jni")]
    fn is_dialog_kind(&self, logical_id: u64) -> bool {
        matches!(self.kind_of(logical_id), Some(AndroidHandleKind::MessageBox))
    }
}

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

    #[cfg(feature = "mobile-api")]
    fn mobile_extension(&self) -> Option<&dyn crate::platform::contract::MobilePlatformExtension> {
        Some(self)
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
        self.create_with_native(AndroidHandleKind::Window, title, x, y, width, height)
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
        self.create_with_native(AndroidHandleKind::Button, text, x, y, width, height)
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
        self.create_with_native(AndroidHandleKind::CheckBox, text, x, y, width, height)
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
        self.create_with_native(AndroidHandleKind::LineEdit, text, x, y, width, height)
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
        self.create_with_native(AndroidHandleKind::Label, text, x, y, width, height)
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
        self.create_with_native(AndroidHandleKind::RadioButton, text, x, y, width, height)
    }

    fn create_slider(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.create_with_native(AndroidHandleKind::Slider, "Slider", x, y, width, height)
    }

    fn create_progress_bar(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.create_with_native(AndroidHandleKind::ProgressBar, "ProgressBar", x, y, width, height)
    }

    fn create_combo_box(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.create_with_list_data(AndroidHandleKind::ComboBox, "ComboBox", x, y, width, height)
    }

    fn combo_box_add_item(&self, combo_box: u64, text: &str) -> bool {
        if !matches!(self.kind_of(combo_box), Some(AndroidHandleKind::ComboBox)) {
            return false;
        }
        let mut data = self.list_data.lock().expect("android list data lock poisoned");
        let entry = data.entry(combo_box).or_default();
        entry.items.push(text.to_string());
        #[cfg(feature = "android-jni")]
        let is_first_item = entry.items.len() == 1;
        drop(data);

        // Mirror the appended item onto the native Spinner adapter when present
        // so the native view and logical state stay in sync.
        #[cfg(feature = "android-jni")]
        if let Some(jni_id) = self.native_view_of(combo_box) {
            if !crate::platform::android_jni::append_spinner_item(jni_id, text, is_first_item) {
                log::warn!(
                    "[android] combo_box_add_item({combo_box}): native adapter update failed"
                );
            }
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
        self.create_with_list_data(AndroidHandleKind::ListBox, "ListBox", x, y, width, height)
    }

    fn list_box_add_item(&self, list_box: u64, text: &str) -> bool {
        if !matches!(self.kind_of(list_box), Some(AndroidHandleKind::ListBox)) {
            return false;
        }
        let mut data = self.list_data.lock().expect("android list data lock poisoned");
        let entry = data.entry(list_box).or_default();
        entry.items.push(text.to_string());
        drop(data);

        #[cfg(feature = "android-jni")]
        if let Some(jni_id) = self.native_view_of(list_box) {
            if !crate::platform::android_jni::append_list_item(jni_id, &[text]) {
                log::warn!("[android] list_box_add_item({list_box}): native list update failed");
            }
        }

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
        self.create_with_native(AndroidHandleKind::Panel, "Panel", x, y, width, height)
    }

    fn create_menu_bar(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if !matches!(self.kind_of(parent), Some(AndroidHandleKind::Window)) {
            return 0;
        }
        // Android has no standalone native menu-bar View; the menu is modelled
        // as in-process data consumed by the host Activity's own menu.
        self.insert_widget(AndroidHandleKind::MenuBar, "MenuBar", x, y, width, height)
    }

    fn create_menu(&self, parent: u64, text: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        // A menu must hang off a menu bar or another menu, matching the
        // Harmony/Wayland contract.
        if !matches!(
            self.kind_of(parent),
            Some(AndroidHandleKind::MenuBar | AndroidHandleKind::Menu)
        ) {
            return 0;
        }
        let id = self.insert_widget(AndroidHandleKind::Menu, text, x, y, width, height);

        let mut menus = self.menus.lock().expect("android menus lock poisoned");
        menus.menu_children.entry(parent).or_default().push(id);
        drop(menus);

        // Android menus are modelled as in-process data and rendered by the host
        // Activity's own `onCreateOptionsMenu`; there is no per-item native View
        // to create here, so the logical handle is the complete representation.
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
        // A menu item must hang off a menu.
        if !matches!(self.kind_of(parent_menu), Some(AndroidHandleKind::Menu)) {
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

        // Native menu items are represented through the Activity's own menu
        // resource; no standalone Android View exists for a menu entry.
        id
    }

    fn poll_menu_triggered(&self) -> Option<u64> {
        let mut menus = self.menus.lock().expect("android menus lock poisoned");
        menus.pending_menu_events.pop_front()
    }

    fn inject_menu_trigger(&self, menu_item_id: u64) -> bool {
        // Only a menu item may produce a menu trigger.
        if !matches!(self.kind_of(menu_item_id), Some(AndroidHandleKind::MenuItem)) {
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
        // `androidx.appcompat.widget.Toolbar` belongs to the AndroidX support
        // library and is not guaranteed on every device, so the toolbar stays a
        // logical region the host Activity populates.
        self.insert_widget(AndroidHandleKind::ToolBar, "ToolBar", x, y, width, height)
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
        self.create_with_native(AndroidHandleKind::StatusBar, text, x, y, width, height)
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
        // Create a real `AlertDialog` when the bridge is ready; the dialog object
        // is registered under the same id so show/hide/text forward to it.
        #[cfg(feature = "android-jni")]
        if self.jni_available() {
            if let Some(jni_id) = crate::platform::android_jni::create_native_dialog(title, text) {
                self.set_native_view(id, jni_id);
            } else {
                log::warn!("[android] create_message_box({id}): native AlertDialog not created");
            }
        }
        id
    }

    fn create_file_dialog(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        // Android has no file-picker View: selection is an Activity operation.
        // The logical handle records the request, and when the bridge holds an
        // Activity we launch ACTION_OPEN_DOCUMENT through it (see
        // `android_jni::launch_file_dialog`). The picker result is delivered to
        // the host Activity's own callback. A non-Activity Context cannot do
        // this; the bridge logs that explicitly instead of failing silently.
        let id =
            self.insert_widget(AndroidHandleKind::FileDialog, "FileDialog", x, y, width, height);
        #[cfg(feature = "android-jni")]
        if self.jni_available() {
            // Empty MIME type selects `*/*`; callers that need a filter can
            // drive `launch_file_dialog` directly.
            if !crate::platform::android_jni::launch_file_dialog("") {
                log::info!(
                    "[android] create_file_dialog({id}): ACTION_OPEN_DOCUMENT not launched \
                     (see the preceding [android-jni] diagnostic)"
                );
            }
        }
        id
    }

    fn create_color_dialog(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        // Android has no platform color picker; the host app supplies one.
        let id =
            self.insert_widget(AndroidHandleKind::ColorDialog, "ColorDialog", x, y, width, height);
        #[cfg(feature = "android-jni")]
        if self.jni_available() {
            log::info!(
                "[android] create_color_dialog({id}): logical only — Android has no platform \
                 color picker, the host app must provide one"
            );
        }
        id
    }

    fn create_font_dialog(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        // Android has no platform font picker; the host app supplies one.
        let id =
            self.insert_widget(AndroidHandleKind::FontDialog, "FontDialog", x, y, width, height);
        #[cfg(feature = "android-jni")]
        if self.jni_available() {
            log::info!(
                "[android] create_font_dialog({id}): logical only — Android has no platform \
                 font picker, the host app must provide one"
            );
        }
        id
    }

    fn create_spin_box(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.create_with_native(AndroidHandleKind::SpinBox, "SpinBox", x, y, width, height)
    }

    fn create_list_view(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.create_with_list_data(AndroidHandleKind::ListView, "ListView", x, y, width, height)
    }

    fn create_scroll_area(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.create_with_native(AndroidHandleKind::ScrollArea, "ScrollArea", x, y, width, height)
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
        self.state.create_widget(AndroidHandleKind::GroupBox, title, x, y, width, height)
    }
    fn create_frame(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(AndroidHandleKind::Frame, "Frame", x, y, width, height)
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
        self.state.create_widget(AndroidHandleKind::TabWidget, "TabWidget", x, y, width, height)
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
        self.state.create_widget(AndroidHandleKind::Splitter, "Splitter", x, y, width, height)
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
        self.state.create_widget(AndroidHandleKind::ToggleButton, text, x, y, width, height)
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
        self.state.create_widget(AndroidHandleKind::Calendar, "Calendar", x, y, width, height)
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
        self.state.create_widget(AndroidHandleKind::ScrollBar, "ScrollBar", x, y, width, height)
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
        self.state.create_widget(
            AndroidHandleKind::DoubleSpinBox,
            "DoubleSpinBox",
            x,
            y,
            width,
            height,
        )
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
        self.state.create_widget(
            AndroidHandleKind::FontComboBox,
            "FontComboBox",
            x,
            y,
            width,
            height,
        )
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
        self.state.create_widget(AndroidHandleKind::ContextMenu, "ContextMenu", x, y, width, height)
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
        self.state.create_widget(AndroidHandleKind::PopupWindow, title, x, y, width, height)
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
        self.state.create_widget(AndroidHandleKind::Dialog, title, x, y, width, height)
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
        self.state.create_widget(AndroidHandleKind::InputDialog, "Input", x, y, width, height)
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
        self.state.create_widget(AndroidHandleKind::ProgressDialog, "Progress", x, y, width, height)
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
        self.state.create_widget(AndroidHandleKind::DirectoryDialog, title, x, y, width, height)
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
        self.state.create_widget(AndroidHandleKind::DatePicker, "DatePicker", x, y, width, height)
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
        self.state.create_widget(AndroidHandleKind::TimePicker, "TimePicker", x, y, width, height)
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
            AndroidHandleKind::DateTimePicker,
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
            AndroidHandleKind::ActivityIndicator,
            "ActivityIndicator",
            x,
            y,
            width,
            height,
        )
    }

    // ─── Widget manipulation ─────────────────────────────────────────────

    fn show_widget(&self, widget_id: u64) {
        self.state.set_visible(widget_id, true);

        #[cfg(feature = "android-jni")]
        if let Some(jni_id) = self.native_view_of(widget_id) {
            if self.is_dialog_kind(widget_id) {
                crate::platform::android_jni::set_native_dialog_visible(jni_id, true);
            } else {
                crate::platform::android_jni::set_native_view_visibility(jni_id, true);
            }
        }
    }

    fn hide_widget(&self, widget_id: u64) {
        self.state.set_visible(widget_id, false);

        #[cfg(feature = "android-jni")]
        if let Some(jni_id) = self.native_view_of(widget_id) {
            if self.is_dialog_kind(widget_id) {
                crate::platform::android_jni::set_native_dialog_visible(jni_id, false);
            } else {
                crate::platform::android_jni::set_native_view_visibility(jni_id, false);
            }
        }
    }

    fn set_widget_geometry(&self, widget_id: u64, x: i32, y: i32, width: u32, height: u32) {
        self.state.set_geometry(widget_id, x, y, width, height);

        #[cfg(feature = "android-jni")]
        if let Some(jni_id) = self.native_view_of(widget_id) {
            crate::platform::android_jni::set_native_view_bounds(jni_id, x, y, width, height);
        }
    }

    fn set_widget_text(&self, widget_id: u64, text: &str) {
        self.state.set_text(widget_id, text);

        #[cfg(feature = "android-jni")]
        if let Some(jni_id) = self.native_view_of(widget_id) {
            if matches!(self.kind_of(widget_id), Some(AndroidHandleKind::MessageBox)) {
                // A dialog's body is its message, not a `View` text field.
                crate::platform::android_jni::set_native_dialog_message(jni_id, text);
            } else {
                crate::platform::android_jni::set_native_view_text(jni_id, text);
            }
        }
    }

    fn get_widget_text(&self, widget_id: u64) -> String {
        self.state.text(widget_id)
    }

    fn set_widget_enabled(&self, widget_id: u64, enabled: bool) {
        self.state.set_enabled(widget_id, enabled);

        #[cfg(feature = "android-jni")]
        if let Some(jni_id) = self.native_view_of(widget_id) {
            crate::platform::android_jni::set_native_view_enabled(jni_id, enabled);
        }
    }

    fn is_widget_enabled(&self, widget_id: u64) -> bool {
        self.state.enabled(widget_id)
    }

    fn set_widget_visible(&self, widget_id: u64, visible: bool) {
        self.state.set_visible(widget_id, visible);

        #[cfg(feature = "android-jni")]
        if let Some(jni_id) = self.native_view_of(widget_id) {
            crate::platform::android_jni::set_native_view_visibility(jni_id, visible);
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

    fn attach_to_native_view(&self, native_handle: usize) -> bool {
        // The handle is the host Activity's Java `Context` reference. Store it as
        // a GlobalRef so `create_native_view` can construct Android Views on any
        // later thread; without it the state-only path stays active.
        #[cfg(feature = "android-jni")]
        {
            if native_handle == 0 || !crate::platform::android_jni::is_initialized() {
                return false;
            }
            // `JObject` does not own the reference; the GlobalRef created by
            // `set_activity_context` is what keeps it alive.
            let context_obj: jni::objects::JObject<'_> =
                unsafe { jni::objects::JObject::from_raw(native_handle as jni::sys::jobject) };
            crate::platform::android_jni::with_jni_env(|env| {
                crate::platform::android_jni::set_activity_context(env, &context_obj)
            })
            .unwrap_or(false)
        }
        #[cfg(not(feature = "android-jni"))]
        {
            let _ = native_handle;
            false
        }
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

    #[cfg(feature = "android-jni")]
    #[test]
    fn android_platform_degrades_to_state_without_jvm() {
        // Without a stored JavaVM the native view path must be skipped and the
        // logical state handle must remain authoritative (honest degradation).
        let platform = AndroidPlatform::new();
        platform.init();
        assert!(!platform.jni_available());

        let window_id = platform.create_window("Window", 0, 0, 320, 568);
        assert_ne!(window_id, 0);
        assert!(platform.native_view_of(window_id).is_none());

        let button = platform.create_button(window_id, "Button", 0, 0, 80, 44);
        assert_ne!(button, 0);
        assert!(platform.native_view_of(button).is_none());

        // State-backed property roundtrips must still work.
        platform.set_widget_text(button, "Updated");
        assert_eq!(platform.get_widget_text(button), "Updated");
        platform.set_widget_enabled(button, false);
        assert!(!platform.is_widget_enabled(button));
        platform.hide_widget(button);
        assert!(!platform.is_widget_visible(button));
    }

    #[cfg(feature = "android-jni")]
    #[test]
    fn android_view_class_mapping_is_complete_for_native_kinds() {
        // Every handle kind that maps to a concrete Android widget must resolve
        // to exactly one view class; dialog/menu kinds intentionally map to None.
        let native_kinds = [
            AndroidHandleKind::Button,
            AndroidHandleKind::Label,
            AndroidHandleKind::StatusBar,
            AndroidHandleKind::LineEdit,
            AndroidHandleKind::CheckBox,
            AndroidHandleKind::RadioButton,
            AndroidHandleKind::Slider,
            AndroidHandleKind::ProgressBar,
            AndroidHandleKind::ComboBox,
            AndroidHandleKind::ListBox,
            AndroidHandleKind::ListView,
            AndroidHandleKind::ScrollArea,
            AndroidHandleKind::SpinBox,
            AndroidHandleKind::Panel,
            AndroidHandleKind::Window,
        ];
        for kind in native_kinds {
            assert!(
                AndroidPlatform::view_class_for(kind).is_some(),
                "{kind:?} should map to a native view"
            );
        }

        let logical_only = [
            AndroidHandleKind::MenuBar,
            AndroidHandleKind::Menu,
            AndroidHandleKind::MenuItem,
            AndroidHandleKind::ToolBar,
            AndroidHandleKind::MessageBox,
            AndroidHandleKind::FileDialog,
            AndroidHandleKind::ColorDialog,
            AndroidHandleKind::FontDialog,
        ];
        for kind in logical_only {
            assert!(
                AndroidPlatform::view_class_for(kind).is_none(),
                "{kind:?} has no standalone native view and must map to None"
            );
        }
    }

    #[test]
    fn android_menu_requires_correct_parent_kind() {
        let platform = AndroidPlatform::new();
        platform.init();
        let window = platform.create_window("Window", 0, 0, 320, 568);
        let button = platform.create_button(window, "Button", 0, 0, 80, 44);

        // MenuBar requires a window.
        assert_eq!(platform.create_menu_bar(button, 0, 0, 320, 24), 0);
        let menu_bar = platform.create_menu_bar(window, 0, 0, 320, 24);
        assert_ne!(menu_bar, 0);

        // A menu requires a menu bar or another menu.
        assert_eq!(platform.create_menu(window, "Bad", 0, 0, 80, 24), 0);
        assert_eq!(platform.create_menu(button, "Bad", 0, 0, 80, 24), 0);
        let menu = platform.create_menu(menu_bar, "File", 0, 0, 80, 24);
        assert_ne!(menu, 0);
        let submenu = platform.create_menu(menu, "Recent", 0, 0, 80, 24);
        assert_ne!(submenu, 0, "a menu may nest under another menu");

        // A menu item requires a menu.
        assert_eq!(platform.menu_add_item(window, "Bad", None), 0);
        assert_eq!(platform.menu_add_item(menu_bar, "Bad", None), 0);
        let item = platform.menu_add_item(menu, "Open", Some("Ctrl+O"));
        assert_ne!(item, 0);

        // Only a menu item may be injected as a menu trigger.
        assert!(!platform.inject_menu_trigger(window));
        assert!(!platform.inject_menu_trigger(menu_bar));
        assert!(platform.inject_menu_trigger(item));
        assert_eq!(platform.poll_menu_triggered(), Some(item));

        // attach_menu_bar_to_window validates both ids and their kinds.
        assert!(!platform.attach_menu_bar_to_window(9999, menu_bar));
        assert!(!platform.attach_menu_bar_to_window(window, item));
        assert!(platform.attach_menu_bar_to_window(window, menu_bar));
    }

    #[test]
    fn android_menu_item_retains_shortcut() {
        let platform = AndroidPlatform::new();
        platform.init();
        let window = platform.create_window("Window", 0, 0, 320, 568);
        let menu_bar = platform.create_menu_bar(window, 0, 0, 320, 24);
        let menu = platform.create_menu(menu_bar, "File", 0, 0, 80, 24);

        let plain = platform.menu_add_item(menu, "Open", None);
        assert_eq!(platform.get_widget_text(plain), "Open");

        let with_shortcut = platform.menu_add_item(menu, "Save", Some("Ctrl+S"));
        assert_eq!(platform.get_widget_text(with_shortcut), "Save (Ctrl+S)");
    }
}
