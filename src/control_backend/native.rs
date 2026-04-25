use crate::control_backend::trait_def::ControlBackend;
use crate::control_backend::types::ControlBackendKind;
use crate::core::ObjectId;
use crate::platform::{get_platform, WidgetTriggerEvent, WidgetTriggerKind};
/// Native control backend that forwards to platform backend.
pub struct NativeControlBackend;
impl NativeControlBackend {
    /// Create native control backend.
    pub const fn new() -> Self {
        Self
    }
}
impl Default for NativeControlBackend {
    fn default() -> Self {
        Self::new()
    }
}
impl ControlBackend for NativeControlBackend {
    fn backend_name(&self) -> &'static str {
        "native-control-backend"
    }
    fn kind(&self) -> ControlBackendKind {
        ControlBackendKind::Native
    }
    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        get_platform().create_window(title, x, y, width, height)
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
        get_platform().create_button(parent, text, x, y, width, height)
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
        get_platform().create_checkbox(parent, text, x, y, width, height)
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
        get_platform().create_line_edit(parent, text, x, y, width, height)
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
        get_platform().create_label(parent, text, x, y, width, height)
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
        get_platform().create_radio_button(parent, text, x, y, width, height)
    }
    fn create_slider(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        get_platform().create_slider(parent, x, y, width, height)
    }
    fn create_progress_bar(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        get_platform().create_progress_bar(parent, x, y, width, height)
    }
    fn create_combo_box(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        get_platform().create_combo_box(parent, x, y, width, height)
    }
    fn create_list_box(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        get_platform().create_list_box(parent, x, y, width, height)
    }
    fn create_panel(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        get_platform().create_panel(parent, x, y, width, height)
    }
    fn create_menu_bar(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        get_platform().create_menu_bar(parent, x, y, width, height)
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
        get_platform().create_menu(parent, text, x, y, width, height)
    }
    fn attach_menu_bar_to_window(&self, window: ObjectId, menu_bar: ObjectId) -> bool {
        get_platform().attach_menu_bar_to_window(window, menu_bar)
    }
    fn menu_add_item(&self, parent_menu: ObjectId, text: &str, shortcut: Option<&str>) -> ObjectId {
        get_platform().menu_add_item(parent_menu, text, shortcut)
    }
    fn create_tool_bar(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        get_platform().create_tool_bar(parent, x, y, width, height)
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
        get_platform().create_status_bar(parent, text, x, y, width, height)
    }
    fn poll_menu_triggered(&self) -> Option<ObjectId> {
        get_platform().poll_menu_triggered()
    }
    fn inject_menu_trigger(&self, menu_item_id: ObjectId) -> bool {
        get_platform().inject_menu_trigger(menu_item_id)
    }
    fn poll_widget_trigger_event(&self) -> Option<WidgetTriggerEvent> {
        get_platform().poll_widget_trigger_event()
    }
    fn inject_widget_trigger_event(&self, widget_id: ObjectId, kind: WidgetTriggerKind) -> bool {
        get_platform().inject_widget_trigger_event(widget_id, kind)
    }
    fn set_widget_text(&self, widget_id: ObjectId, text: &str) {
        get_platform().set_widget_text(widget_id, text);
    }
    fn get_widget_text(&self, widget_id: ObjectId) -> String {
        get_platform().get_widget_text(widget_id)
    }
    fn set_widget_enabled(&self, widget_id: ObjectId, enabled: bool) {
        get_platform().set_widget_enabled(widget_id, enabled);
    }
    fn is_widget_enabled(&self, widget_id: ObjectId) -> bool {
        get_platform().is_widget_enabled(widget_id)
    }
    fn set_widget_visible(&self, widget_id: ObjectId, visible: bool) {
        get_platform().set_widget_visible(widget_id, visible);
    }
    fn show_widget(&self, widget_id: ObjectId) {
        get_platform().show_widget(widget_id);
    }
    fn hide_widget(&self, widget_id: ObjectId) {
        get_platform().hide_widget(widget_id);
    }
    fn is_widget_visible(&self, widget_id: ObjectId) -> bool {
        get_platform().is_widget_visible(widget_id)
    }
    fn set_widget_geometry(&self, widget_id: ObjectId, x: i32, y: i32, width: u32, height: u32) {
        get_platform().set_widget_geometry(widget_id, x, y, width, height);
    }
    fn set_widget_ime_enabled(&self, widget_id: ObjectId, enabled: bool) -> bool {
        get_platform().set_widget_ime_enabled(widget_id, enabled)
    }
    fn is_widget_ime_enabled(&self, widget_id: ObjectId) -> bool {
        get_platform().is_widget_ime_enabled(widget_id)
    }
    fn set_widget_accessibility_name(&self, widget_id: ObjectId, name: &str) -> bool {
        get_platform().set_widget_accessibility_name(widget_id, name)
    }
    fn get_widget_accessibility_name(&self, widget_id: ObjectId) -> String {
        get_platform().get_widget_accessibility_name(widget_id)
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
        get_platform().create_message_box(parent, title, "", x, y, width, height)
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
        get_platform().create_message_box(parent, title, text, x, y, width, height)
    }
    fn create_file_dialog(
        &self,
        parent: ObjectId,
        _title: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        get_platform().create_file_dialog(parent, x, y, width, height)
    }
    fn create_color_dialog(
        &self,
        parent: ObjectId,
        _title: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        get_platform().create_color_dialog(parent, x, y, width, height)
    }
    fn create_font_dialog(
        &self,
        parent: ObjectId,
        _title: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        get_platform().create_font_dialog(parent, x, y, width, height)
    }
    fn create_popup_window(
        &self,
        _parent: ObjectId,
        title: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        get_platform().create_window(title, x, y, width, height)
    }
    fn create_text_edit(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        get_platform().create_line_edit(parent, text, x, y, width, height)
    }
    fn create_rich_edit(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        get_platform().create_line_edit(parent, text, x, y, width, height)
    }
    fn create_spin_box(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        get_platform().create_spin_box(parent, x, y, width, height)
    }
    fn create_list_view(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        get_platform().create_list_view(parent, x, y, width, height)
    }
    fn create_tree_view(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        get_platform().create_list_box(parent, x, y, width, height)
    }
    fn create_scroll_bar(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        get_platform().create_slider(parent, x, y, width, height)
    }
    fn create_scroll_area(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        get_platform().create_panel(parent, x, y, width, height)
    }
    fn create_dock_panel(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        get_platform().create_panel(parent, x, y, width, height)
    }
    fn create_group_box(
        &self,
        parent: ObjectId,
        _title: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        get_platform().create_panel(parent, x, y, width, height)
    }
    fn create_tab_widget(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        get_platform().create_panel(parent, x, y, width, height)
    }
    fn create_splitter(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        get_platform().create_panel(parent, x, y, width, height)
    }
    fn create_stack_widget(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        get_platform().create_panel(parent, x, y, width, height)
    }
    fn create_mdi_area(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        get_platform().create_panel(parent, x, y, width, height)
    }
    fn create_canvas(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        get_platform().create_panel(parent, x, y, width, height)
    }
    fn create_table(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        get_platform().create_panel(parent, x, y, width, height)
    }
    fn create_grid(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        get_platform().create_panel(parent, x, y, width, height)
    }
    fn create_chart(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        get_platform().create_panel(parent, x, y, width, height)
    }
}
