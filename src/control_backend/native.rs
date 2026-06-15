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
    fn combo_box_add_item(&self, widget_id: ObjectId, text: &str) -> bool {
        get_platform().combo_box_add_item(widget_id, text)
    }
    fn combo_box_clear_items(&self, widget_id: ObjectId) -> bool {
        get_platform().combo_box_clear_items(widget_id)
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
    fn list_box_add_item(&self, widget_id: ObjectId, text: &str) -> bool {
        get_platform().list_box_add_item(widget_id, text)
    }
    fn list_box_remove_item(&self, widget_id: ObjectId, index: usize) -> bool {
        get_platform().list_box_remove_item(widget_id, index)
    }
    fn list_box_clear_items(&self, widget_id: ObjectId) -> bool {
        get_platform().list_box_clear_items(widget_id)
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
    fn set_clipboard_text(&self, text: &str) -> bool {
        get_platform().set_clipboard_text(text)
    }
    fn get_clipboard_text(&self) -> String {
        get_platform().get_clipboard_text()
    }
    fn begin_drag(&self, source: ObjectId, mime_type: &str, payload: &[u8]) -> bool {
        get_platform().begin_drag(source, mime_type, payload)
    }
    fn poll_drop_event(&self) -> Option<crate::platform::DropEvent> {
        get_platform().poll_drop_event()
    }
    fn inject_drop_event(&self, event: crate::platform::DropEvent) -> bool {
        get_platform().inject_drop_event(event)
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
    fn create_toggle_button(
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
    fn create_check_list_box(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        get_platform().create_list_box(parent, x, y, width, height)
    }
    fn create_double_spin_box(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        get_platform().create_spin_box(parent, x, y, width, height)
    }
    fn create_dial(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        get_platform().create_slider(parent, x, y, width, height)
    }
    fn create_wizard(
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
    fn create_date_picker(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        get_platform().create_panel(parent, x, y, width, height)
    }
    fn create_time_picker(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        get_platform().create_panel(parent, x, y, width, height)
    }
    fn create_date_time_picker(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        get_platform().create_panel(parent, x, y, width, height)
    }
    fn create_directory_dialog(
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
    fn create_data_view(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        log::warn!("shallow implementation: DataView maps to virtualized data-view host");
        get_platform().create_panel(parent, x, y, width, height)
    }
    fn create_property_grid(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        log::warn!("shallow implementation: PropertyGrid is an alias for TreeView");
        get_platform().create_panel(parent, x, y, width, height)
    }
    fn create_toolbox(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        get_platform().create_panel(parent, x, y, width, height)
    }
    fn create_collapsible_pane(
        &self,
        parent: ObjectId,
        _title: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        log::warn!("shallow implementation: CollapsiblePane is an alias for Panel");
        get_platform().create_panel(parent, x, y, width, height)
    }
    fn create_dock_widget(
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
    fn create_web_view(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        get_platform().create_panel(parent, x, y, width, height)
    }
    fn create_activity_indicator(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        get_platform().create_progress_bar(parent, x, y, width, height)
    }
    fn create_calendar(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        get_platform().create_panel(parent, x, y, width, height)
    }
    fn create_column_view(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        log::warn!("shallow implementation: ColumnView is an alias for TreeView");
        get_platform().create_list_view(parent, x, y, width, height)
    }
    fn create_undo_view(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        log::warn!("shallow implementation: UndoView is an alias for ListView");
        get_platform().create_list_view(parent, x, y, width, height)
    }
    fn create_command_link(
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
    fn create_lcd_number(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        get_platform().create_label(parent, "0", x, y, width, height)
    }
    fn create_font_combo_box(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        get_platform().create_combo_box(parent, x, y, width, height)
    }
    fn create_web_engine_view(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        get_platform().create_panel(parent, x, y, width, height)
    }
    fn create_web_engine_page(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        get_platform().create_panel(parent, x, y, width, height)
    }
    fn create_web_engine_settings(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        get_platform().create_panel(parent, x, y, width, height)
    }
    fn create_web_engine_download_item(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        get_platform().create_panel(parent, x, y, width, height)
    }
    fn create_web_engine_cookie_store(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        get_platform().create_panel(parent, x, y, width, height)
    }
    fn create_web_engine_web_channel(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        get_platform().create_panel(parent, x, y, width, height)
    }
    fn create_web_engine_find_text_result(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        get_platform().create_panel(parent, x, y, width, height)
    }
    fn create_web_engine_notification(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        get_platform().create_panel(parent, x, y, width, height)
    }
    fn create_web_engine_script_dialog(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        get_platform().create_panel(parent, x, y, width, height)
    }
    fn create_web_engine_context_menu_request(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        get_platform().create_panel(parent, x, y, width, height)
    }
    fn create_action(
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
    fn create_tool_button(
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
    fn create_tool_box(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        get_platform().create_panel(parent, x, y, width, height)
    }
    fn create_context_menu(
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::control_backend::types::ControlBackendKind;

    #[test]
    fn native_control_backend_new_creates_instance() {
        let backend = NativeControlBackend::new();
        assert_eq!(backend.backend_name(), "native-control-backend");
        assert_eq!(backend.kind(), ControlBackendKind::Native);
    }

    #[test]
    fn native_control_backend_default() {
        let backend = NativeControlBackend;
        assert_eq!(backend.backend_name(), "native-control-backend");
        assert_eq!(backend.kind(), ControlBackendKind::Native);
    }

    #[test]
    fn backend_name_is_not_empty() {
        let backend = NativeControlBackend::new();
        let name = backend.backend_name();
        assert!(!name.is_empty(), "backend_name must not be empty");
    }

    #[test]
    fn kind_is_native() {
        let backend = NativeControlBackend::new();
        assert_eq!(backend.kind(), ControlBackendKind::Native);
    }

    #[test]
    fn native_backend_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<NativeControlBackend>();
    }
}
