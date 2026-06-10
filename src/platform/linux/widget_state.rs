use super::types::{LinuxHandleKind, LinuxPlatform};
#[cfg(all(target_os = "linux", feature = "gtk-native"))]
use crate::core::MutexExt;
use crate::platform::{DropEvent, WidgetTriggerEvent, WidgetTriggerKind};
#[cfg(all(target_os = "linux", feature = "gtk-native"))]
use gtk::prelude::*;

impl LinuxPlatform {
    pub(crate) fn show_widget_impl(&self, widget_id: u64) {
        self.state.set_visible(widget_id, true);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let native = self.native.lock_guard();
            if let Some(window) = native.windows.get(&widget_id) {
                window.show_all();
                return;
            }
            if let Some(widget) = native.widgets.get(&widget_id) {
                widget.show();
            }
        }
    }
    pub(crate) fn hide_widget_impl(&self, widget_id: u64) {
        self.state.set_visible(widget_id, false);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let native = self.native.lock_guard();
            if let Some(window) = native.windows.get(&widget_id) {
                window.hide();
                return;
            }
            if let Some(widget) = native.widgets.get(&widget_id) {
                widget.hide();
            }
        }
    }
    pub(crate) fn set_widget_geometry_impl(
        &self,
        widget_id: u64,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) {
        self.state.set_geometry(widget_id, x, y, width, height);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let parent_id = {
                let menus = self.menus.lock().expect("linux menu lock poisoned");
                menus.widget_parent.get(&widget_id).copied()
            };
            let native = self.native.lock_guard();
            if let Some(window) = native.windows.get(&widget_id) {
                window.move_(x, y);
                window.resize(width as i32, height as i32);
                return;
            }
            if let Some(widget) = native.widgets.get(&widget_id) {
                widget.set_size_request(width as i32, height as i32);
                if let Some(parent_id) = parent_id {
                    if let Some(container) = native.content_fixed.get(&parent_id) {
                        container.move_(widget, x, y);
                    }
                }
            }
        }
    }
    pub(crate) fn set_widget_text_impl(&self, widget_id: u64, text: &str) {
        if !self.state.set_text(widget_id, text) {
            return;
        }
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let native = self.native.lock_guard();
            if let Some(window) = native.windows.get(&widget_id) {
                window.set_title(text);
            } else if let Some(widget) = native.widgets.get(&widget_id) {
                if let Ok(button) = widget.clone().downcast::<gtk::Button>() {
                    button.set_label(text);
                } else if let Ok(check) = widget.clone().downcast::<gtk::CheckButton>() {
                    check.set_label(text);
                } else if let Ok(entry) = widget.clone().downcast::<gtk::Entry>() {
                    entry.set_text(text);
                } else if let Ok(label) = widget.clone().downcast::<gtk::Label>() {
                    label.set_text(text);
                } else if let Ok(menu_item) = widget.clone().downcast::<gtk::MenuItem>() {
                    menu_item.set_label(text);
                }
            }
        }
        if matches!(self.kind_of(widget_id), Some(LinuxHandleKind::LineEdit)) {
            self.menus
                .lock()
                .expect("linux menu lock poisoned")
                .pending_widget_events
                .push_back(WidgetTriggerEvent { widget_id, kind: WidgetTriggerKind::ValueChanged });
        }
    }
    pub(crate) fn get_widget_text_impl(&self, widget_id: u64) -> String {
        self.state.text(widget_id)
    }
    pub(crate) fn set_widget_enabled_impl(&self, widget_id: u64, enabled: bool) {
        self.state.set_enabled(widget_id, enabled);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let native = self.native.lock_guard();
            if let Some(widget) = native.widgets.get(&widget_id) {
                widget.set_sensitive(enabled);
            }
        }
    }
    pub(crate) fn is_widget_enabled_impl(&self, widget_id: u64) -> bool {
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let native = self.native.lock_guard();
            if let Some(widget) = native.widgets.get(&widget_id) {
                return widget.is_sensitive();
            }
        }
        self.state.enabled(widget_id)
    }
    pub(crate) fn set_widget_visible_impl(&self, widget_id: u64, visible: bool) {
        self.state.set_visible(widget_id, visible);
        if visible {
            self.show_widget_impl(widget_id);
        } else {
            self.hide_widget_impl(widget_id);
        }
    }
    pub(crate) fn is_widget_visible_impl(&self, widget_id: u64) -> bool {
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let native = self.native.lock_guard();
            if let Some(window) = native.windows.get(&widget_id) {
                return window.is_visible();
            }
            if let Some(widget) = native.widgets.get(&widget_id) {
                return widget.is_visible();
            }
        }
        self.state.visible(widget_id)
    }
    pub(crate) fn set_widget_ime_enabled_impl(&self, widget_id: u64, enabled: bool) -> bool {
        self.state.set_ime_enabled(widget_id, enabled)
    }
    pub(crate) fn is_widget_ime_enabled_impl(&self, widget_id: u64) -> bool {
        self.state.ime_enabled(widget_id)
    }
    pub(crate) fn set_widget_accessibility_name_impl(&self, widget_id: u64, name: &str) -> bool {
        self.state.set_accessibility_name(widget_id, name)
    }
    pub(crate) fn get_widget_accessibility_name_impl(&self, widget_id: u64) -> String {
        self.state.accessibility_name(widget_id)
    }
    pub(crate) fn set_clipboard_text_impl(&self, text: &str) -> bool {
        self.state.set_clipboard_text(text)
    }
    pub(crate) fn get_clipboard_text_impl(&self) -> String {
        self.state.clipboard_text()
    }
    pub(crate) fn begin_drag_impl(
        &self,
        source_widget_id: u64,
        mime: &str,
        payload: &[u8],
    ) -> bool {
        self.state.begin_drag(source_widget_id, mime, payload)
    }
    pub(crate) fn poll_drop_event_impl(&self) -> Option<DropEvent> {
        self.state.pop_drop_event()
    }
    pub(crate) fn inject_drop_event_impl(&self, event: DropEvent) -> bool {
        self.state.inject_drop_event(event)
    }
    pub(crate) fn create_message_box_impl(
        &self,
        _parent: u64,
        _title: &str,
        _text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        self.insert_widget(LinuxHandleKind::MessageBox, _text, x, y, width, height)
    }
    pub(crate) fn create_file_dialog_impl(
        &self,
        _parent: u64,
        _x: i32,
        _y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        self.insert_widget(LinuxHandleKind::FileDialog, "FileDialog", _x, _y, width, height)
    }
    pub(crate) fn create_color_dialog_impl(
        &self,
        _parent: u64,
        _x: i32,
        _y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        self.insert_widget(LinuxHandleKind::ColorDialog, "ColorDialog", _x, _y, width, height)
    }
    pub(crate) fn create_font_dialog_impl(
        &self,
        _parent: u64,
        _x: i32,
        _y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        self.insert_widget(LinuxHandleKind::FontDialog, "FontDialog", _x, _y, width, height)
    }
}
