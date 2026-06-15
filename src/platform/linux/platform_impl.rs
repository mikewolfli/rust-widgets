use super::types::{LinuxHandleKind, LinuxPlatform};
use crate::compat::OnceLock;
use crate::core::PlatformFamily;
#[cfg(target_os = "linux")]
use crate::platform::accessibility::linux::LinuxAccessibilityBridge;
#[cfg(target_os = "linux")]
use crate::platform::accessibility::AccessibilityBridge;
use crate::platform::{DropEvent, Platform, WidgetTriggerEvent, WidgetTriggerKind};
use std::sync::atomic::Ordering;
#[cfg(not(all(target_os = "linux", feature = "gtk-native")))]
use std::thread;
#[cfg(not(all(target_os = "linux", feature = "gtk-native")))]
use std::time::Duration;

impl Platform for LinuxPlatform {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn backend_name(&self) -> &'static str {
        "gtk"
    }
    fn family(&self) -> PlatformFamily {
        PlatformFamily::Desktop
    }
    fn init(&self) {
        self.runtime.initialized.store(true, Ordering::SeqCst);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            if let Err(e) = gtk::init() {
                log::error!("[linux] gtk::init() failed: {:?}", e);
            }
        }
        #[cfg(not(all(target_os = "linux", feature = "gtk-native")))]
        {}
    }
    fn run(&self) {
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            gtk::main();
        }
        #[cfg(not(all(target_os = "linux", feature = "gtk-native")))]
        {
            if !self.runtime.initialized.load(Ordering::SeqCst) {
                self.init();
            }
            self.runtime.running.store(true, Ordering::SeqCst);
            while self.runtime.running.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(16));
            }
        }
    }
    fn quit(&self) {
        self.runtime.running.store(false, Ordering::SeqCst);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            gtk::main_quit();
        }
    }
    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        let id = self.insert_widget(LinuxHandleKind::Window, title, x, y, width, height);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let window = gtk::Window::new(gtk::WindowType::Toplevel);
            window.set_title(title);
            window.set_default_size(width as i32, height as i32);
            window.move_(x, y);
            let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
            let fixed = gtk::Fixed::new();
            root.pack_start(&fixed, true, true, 0);
            window.add(&root);
            let mut native = self.native.lock_guard();
            native.windows.insert(id, window.clone());
            native.root_boxes.insert(id, root);
            native.content_fixed.insert(id, fixed.clone());
            native.widgets.insert(id, window.clone().upcast::<gtk::Widget>());
        }
        id
    }

    // ── Widget creation methods ──
    fn create_button(
        &self,
        parent: u64,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        self.create_button_impl(parent, text, x, y, width, height)
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
        self.create_checkbox_impl(parent, text, x, y, width, height)
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
        self.create_line_edit_impl(parent, text, x, y, width, height)
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
        self.create_label_impl(parent, text, x, y, width, height)
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
        self.create_radio_button_impl(parent, text, x, y, width, height)
    }
    fn create_slider(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        self.create_slider_impl(parent, x, y, width, height)
    }
    fn create_progress_bar(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        self.create_progress_bar_impl(parent, x, y, width, height)
    }
    fn create_combo_box(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        self.create_combo_box_impl(parent, x, y, width, height)
    }
    fn create_list_box(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        self.create_list_box_impl(parent, x, y, width, height)
    }
    fn list_box_add_item(&self, list_box: u64, text: &str) -> bool {
        self.list_box_add_item_impl(list_box, text)
    }
    fn list_box_remove_item(&self, list_box: u64, index: usize) -> bool {
        self.list_box_remove_item_impl(list_box, index)
    }
    fn list_box_clear_items(&self, list_box: u64) -> bool {
        self.list_box_clear_items_impl(list_box)
    }
    fn list_box_set_current_index(&self, list_box: u64, index: usize) -> bool {
        self.list_box_set_current_index_impl(list_box, index)
    }
    fn list_box_current_index(&self, list_box: u64) -> Option<usize> {
        self.list_box_current_index_impl(list_box)
    }
    fn list_box_item_count(&self, list_box: u64) -> usize {
        self.list_box_item_count_impl(list_box)
    }
    fn list_box_item_text(&self, list_box: u64, index: usize) -> Option<String> {
        self.list_box_item_text_impl(list_box, index)
    }
    fn combo_box_add_item(&self, combo_box: u64, text: &str) -> bool {
        self.combo_box_add_item_impl(combo_box, text)
    }
    fn combo_box_clear_items(&self, combo_box: u64) -> bool {
        self.combo_box_clear_items_impl(combo_box)
    }
    fn combo_box_set_current_index(&self, combo_box: u64, index: usize) -> bool {
        self.combo_box_set_current_index_impl(combo_box, index)
    }
    fn combo_box_current_index(&self, combo_box: u64) -> Option<usize> {
        self.combo_box_current_index_impl(combo_box)
    }
    fn combo_box_item_count(&self, combo_box: u64) -> usize {
        self.combo_box_item_count_impl(combo_box)
    }
    fn combo_box_item_text(&self, combo_box: u64, index: usize) -> Option<String> {
        self.combo_box_item_text_impl(combo_box, index)
    }
    fn create_panel(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        self.create_panel_impl(parent, x, y, width, height)
    }
    fn create_spin_box(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        self.create_spin_box_impl(parent, x, y, width, height)
    }
    fn create_list_view(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        self.create_list_view_impl(parent, x, y, width, height)
    }
    fn create_scroll_area(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        self.create_scroll_area_impl(parent, x, y, width, height)
    }

    // ── Menu methods ──
    fn create_menu_bar(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        self.create_menu_bar_impl(parent, x, y, width, height)
    }
    fn create_menu(&self, parent: u64, text: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        self.create_menu_impl(parent, text, x, y, width, height)
    }
    fn create_tool_bar(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        self.create_tool_bar_impl(parent, x, y, width, height)
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
        self.create_status_bar_impl(parent, text, x, y, width, height)
    }
    fn attach_menu_bar_to_window(&self, window: u64, menu_bar: u64) -> bool {
        self.attach_menu_bar_to_window_impl(window, menu_bar)
    }
    fn menu_add_item(&self, parent_menu: u64, text: &str, shortcut: Option<&str>) -> u64 {
        self.menu_add_item_impl(parent_menu, text, shortcut)
    }
    fn poll_menu_triggered(&self) -> Option<u64> {
        self.poll_menu_triggered_impl()
    }
    fn inject_menu_trigger(&self, menu_item_id: u64) -> bool {
        self.inject_menu_trigger_impl(menu_item_id)
    }
    fn poll_widget_triggered(&self) -> Option<u64> {
        self.poll_widget_triggered_impl()
    }
    fn poll_widget_trigger_event(&self) -> Option<WidgetTriggerEvent> {
        self.poll_widget_trigger_event_impl()
    }
    fn inject_widget_trigger_event(&self, widget_id: u64, kind: WidgetTriggerKind) -> bool {
        self.inject_widget_trigger_event_impl(widget_id, kind)
    }

    // ── Widget state methods ──
    fn show_widget(&self, widget_id: u64) {
        self.show_widget_impl(widget_id)
    }
    fn hide_widget(&self, widget_id: u64) {
        self.hide_widget_impl(widget_id)
    }
    fn set_widget_geometry(&self, widget_id: u64, x: i32, y: i32, width: u32, height: u32) {
        self.set_widget_geometry_impl(widget_id, x, y, width, height)
    }
    fn set_widget_text(&self, widget_id: u64, text: &str) {
        self.set_widget_text_impl(widget_id, text)
    }
    fn get_widget_text(&self, widget_id: u64) -> String {
        self.get_widget_text_impl(widget_id)
    }
    fn set_widget_enabled(&self, widget_id: u64, enabled: bool) {
        self.set_widget_enabled_impl(widget_id, enabled)
    }
    fn is_widget_enabled(&self, widget_id: u64) -> bool {
        self.is_widget_enabled_impl(widget_id)
    }
    fn set_widget_visible(&self, widget_id: u64, visible: bool) {
        self.set_widget_visible_impl(widget_id, visible)
    }
    fn is_widget_visible(&self, widget_id: u64) -> bool {
        self.is_widget_visible_impl(widget_id)
    }
    fn set_widget_ime_enabled(&self, widget_id: u64, enabled: bool) -> bool {
        self.set_widget_ime_enabled_impl(widget_id, enabled)
    }
    fn is_widget_ime_enabled(&self, widget_id: u64) -> bool {
        self.is_widget_ime_enabled_impl(widget_id)
    }
    fn set_widget_accessibility_name(&self, widget_id: u64, name: &str) -> bool {
        self.set_widget_accessibility_name_impl(widget_id, name)
    }
    fn get_widget_accessibility_name(&self, widget_id: u64) -> String {
        self.get_widget_accessibility_name_impl(widget_id)
    }
    fn set_clipboard_text(&self, text: &str) -> bool {
        self.set_clipboard_text_impl(text)
    }
    fn get_clipboard_text(&self) -> String {
        self.get_clipboard_text_impl()
    }
    fn begin_drag(&self, source_widget_id: u64, mime: &str, payload: &[u8]) -> bool {
        self.begin_drag_impl(source_widget_id, mime, payload)
    }
    fn poll_drop_event(&self) -> Option<DropEvent> {
        self.poll_drop_event_impl()
    }
    fn inject_drop_event(&self, event: DropEvent) -> bool {
        self.inject_drop_event_impl(event)
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
        self.create_message_box_impl(_parent, _title, _text, x, y, width, height)
    }
    fn create_file_dialog(&self, _parent: u64, _x: i32, _y: i32, width: u32, height: u32) -> u64 {
        self.create_file_dialog_impl(_parent, _x, _y, width, height)
    }
    fn create_color_dialog(&self, _parent: u64, _x: i32, _y: i32, width: u32, height: u32) -> u64 {
        self.create_color_dialog_impl(_parent, _x, _y, width, height)
    }
    fn create_font_dialog(&self, _parent: u64, _x: i32, _y: i32, width: u32, height: u32) -> u64 {
        self.create_font_dialog_impl(_parent, _x, _y, width, height)
    }

    #[cfg(target_os = "linux")]
    fn accessibility_bridge(&self) -> Option<&dyn AccessibilityBridge> {
        static BRIDGE: OnceLock<LinuxAccessibilityBridge> = OnceLock::new();
        Some(BRIDGE.get_or_init(LinuxAccessibilityBridge::new))
    }
}
