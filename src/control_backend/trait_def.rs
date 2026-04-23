use crate::control_backend::types::ControlBackendKind;
use crate::core::ObjectId;
use crate::platform::{WidgetTriggerEvent, WidgetTriggerKind};
/// Unified control backend contract.
pub trait ControlBackend: Send + Sync {
    /// Backend display name.
    fn backend_name(&self) -> &'static str;
    /// Backend family kind.
    fn kind(&self) -> ControlBackendKind;
    /// Create top-level window.
    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> ObjectId;
    /// Create button control.
    fn create_button(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create checkbox control.
    fn create_checkbox(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create line edit control.
    fn create_line_edit(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create label control.
    fn create_label(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create radio button control.
    fn create_radio_button(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create slider control.
    fn create_slider(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId;
    /// Create progress bar control.
    fn create_progress_bar(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create combo box control.
    fn create_combo_box(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create list box control.
    fn create_list_box(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create panel control.
    fn create_panel(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId;
    /// Create menu bar host control.
    fn create_menu_bar(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create menu host control.
    fn create_menu(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Attach menu bar to top-level window.
    fn attach_menu_bar_to_window(&self, window: ObjectId, menu_bar: ObjectId) -> bool;
    /// Add menu item to menu host control.
    fn menu_add_item(&self, parent_menu: ObjectId, text: &str, shortcut: Option<&str>) -> ObjectId;
    /// Create tool bar host control.
    fn create_tool_bar(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create status bar host control.
    fn create_status_bar(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create dialog control.
    fn create_dialog(
        &self,
        parent: ObjectId,
        title: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create message box control.
    #[allow(clippy::too_many_arguments)]
    fn create_message_box(
        &self,
        parent: ObjectId,
        title: &str,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create file dialog control.
    fn create_file_dialog(
        &self,
        parent: ObjectId,
        title: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create color dialog control.
    fn create_color_dialog(
        &self,
        parent: ObjectId,
        title: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create font dialog control.
    fn create_font_dialog(
        &self,
        parent: ObjectId,
        title: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create popup window control.
    fn create_popup_window(
        &self,
        parent: ObjectId,
        title: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create text edit control.
    fn create_text_edit(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create rich edit control.
    fn create_rich_edit(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create spin box control.
    fn create_spin_box(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create list view control.
    fn create_list_view(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create tree view control.
    fn create_tree_view(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create scroll bar control.
    fn create_scroll_bar(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create scroll area control.
    fn create_scroll_area(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create dock panel control.
    fn create_dock_panel(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create group box control.
    fn create_group_box(
        &self,
        parent: ObjectId,
        title: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create tab widget control.
    fn create_tab_widget(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create splitter control.
    fn create_splitter(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create stack widget control.
    fn create_stack_widget(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create MDI area control.
    fn create_mdi_area(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create canvas control.
    fn create_canvas(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId;
    /// Create table control.
    fn create_table(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId;
    /// Create grid control.
    fn create_grid(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId;
    /// Create chart control.
    fn create_chart(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId;
    /// Poll next menu trigger id.
    fn poll_menu_triggered(&self) -> Option<ObjectId>;
    /// Inject a menu trigger id.
    fn inject_menu_trigger(&self, menu_item_id: ObjectId) -> bool;
    /// Poll next widget id trigger if available.
    fn poll_widget_triggered(&self) -> Option<ObjectId> {
        self.poll_widget_trigger_event()
            .map(|event| event.widget_id)
    }
    /// Poll next typed widget trigger event.
    fn poll_widget_trigger_event(&self) -> Option<WidgetTriggerEvent>;
    /// Inject a typed widget trigger event.
    fn inject_widget_trigger_event(&self, widget_id: ObjectId, kind: WidgetTriggerKind) -> bool;
    /// Set widget text.
    fn set_widget_text(&self, widget_id: ObjectId, text: &str);
    /// Get widget text.
    fn get_widget_text(&self, widget_id: ObjectId) -> String;
    /// Show widget.
    fn show_widget(&self, widget_id: ObjectId) {
        self.set_widget_visible(widget_id, true);
    }
    /// Hide widget.
    fn hide_widget(&self, widget_id: ObjectId) {
        self.set_widget_visible(widget_id, false);
    }
    /// Set widget enabled state.
    fn set_widget_enabled(&self, widget_id: ObjectId, enabled: bool);
    /// Read widget enabled state.
    fn is_widget_enabled(&self, widget_id: ObjectId) -> bool;
    /// Set widget visibility.
    fn set_widget_visible(&self, widget_id: ObjectId, visible: bool);
    /// Read widget visibility state.
    fn is_widget_visible(&self, widget_id: ObjectId) -> bool;
    /// Set widget geometry.
    fn set_widget_geometry(&self, widget_id: ObjectId, x: i32, y: i32, width: u32, height: u32);
    /// Enable or disable IME input handling for a widget.
    fn set_widget_ime_enabled(&self, widget_id: ObjectId, enabled: bool) -> bool;
    /// Query IME enabled state for a widget.
    fn is_widget_ime_enabled(&self, widget_id: ObjectId) -> bool;
    /// Set accessibility name/label for a widget.
    fn set_widget_accessibility_name(&self, widget_id: ObjectId, name: &str) -> bool;
    /// Read accessibility name/label for a widget.
    fn get_widget_accessibility_name(&self, widget_id: ObjectId) -> String;
}
