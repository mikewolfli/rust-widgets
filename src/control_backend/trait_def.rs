//! Unified control backend contract.
//!
//! This trait is designed for full implementation — it exposes a large surface
//! of widget-creation and lifecycle methods. Rather than adding ~97 default
//! no-op implementations here, helper traits and pattern implementations are
//! provided in [`custom`](crate::control_backend::custom) and
//! [`native`](crate::control_backend::native) modules for common use cases.
//!
//! Implementors should override only the methods they support.

use crate::control_backend::types::ControlBackendKind;
use crate::core::ObjectId;
use crate::platform::{WidgetTriggerEvent, WidgetTriggerKind};
pub trait ControlBackend: Send + Sync {
    /// Backend display name.
    fn backend_name(&self) -> &'static str;
    /// Backend family kind.
    fn kind(&self) -> ControlBackendKind;

    // ── Widget creation helpers (default implementations) ──

    /// Create a widget from a standard geometry pattern with default text "".
    ///
    /// This is a convenience helper that calls `create_widget(geom)` with
    /// default text. Override `create_widget` for custom widget creation.
    /// Default returns a no-op (0) handle.
    fn create_widget(
        &self,
        _kind: &str,
        _parent: ObjectId,
        _text: &str,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }

    // ── Concrete widget creation methods ──

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
    /// Create toggle button control.
    fn create_toggle_button(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create check list box control.
    fn create_check_list_box(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create double spin box control.
    fn create_double_spin_box(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create dial control.
    fn create_dial(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId;
    /// Create wizard control.
    fn create_wizard(
        &self,
        parent: ObjectId,
        title: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create date picker control.
    fn create_date_picker(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create time picker control.
    fn create_time_picker(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create date time picker control.
    fn create_date_time_picker(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create directory dialog control.
    fn create_directory_dialog(
        &self,
        parent: ObjectId,
        title: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create data view control.
    fn create_data_view(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create property grid control.
    fn create_property_grid(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create toolbox control.
    fn create_toolbox(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32)
        -> ObjectId;
    /// Create collapsible pane control.
    fn create_collapsible_pane(
        &self,
        parent: ObjectId,
        title: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create dock widget control.
    fn create_dock_widget(
        &self,
        parent: ObjectId,
        title: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create web view control.
    fn create_web_view(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create activity indicator control.
    fn create_activity_indicator(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create calendar control.
    fn create_calendar(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create column view control.
    fn create_column_view(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create undo view control.
    fn create_undo_view(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create command link control.
    fn create_command_link(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create LCD number control.
    fn create_lcd_number(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create font combo box control.
    fn create_font_combo_box(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create web engine view control.
    fn create_web_engine_view(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create web engine page control.
    fn create_web_engine_page(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create web engine settings control.
    fn create_web_engine_settings(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create web engine download item control.
    fn create_web_engine_download_item(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create web engine cookie store control.
    fn create_web_engine_cookie_store(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create web engine web channel control.
    fn create_web_engine_web_channel(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create web engine find text result control.
    fn create_web_engine_find_text_result(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create web engine notification control.
    fn create_web_engine_notification(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create web engine script dialog control.
    fn create_web_engine_script_dialog(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create web engine context menu request control.
    fn create_web_engine_context_menu_request(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create action control.
    fn create_action(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create tool button control.
    fn create_tool_button(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create tool box control.
    fn create_tool_box(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create context menu control.
    fn create_context_menu(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal test backend that implements ControlBackend for testing defaults.
    struct TestBackend;

    impl ControlBackend for TestBackend {
        fn backend_name(&self) -> &'static str {
            "test-backend"
        }

        fn kind(&self) -> ControlBackendKind {
            ControlBackendKind::Native
        }

        fn create_window(
            &self,
            _title: &str,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            100
        }

        fn create_button(
            &self,
            _parent: ObjectId,
            _text: &str,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            101
        }

        fn create_checkbox(
            &self,
            _parent: ObjectId,
            _text: &str,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            102
        }

        fn create_line_edit(
            &self,
            _parent: ObjectId,
            _text: &str,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            103
        }

        fn create_label(
            &self,
            _parent: ObjectId,
            _text: &str,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            104
        }

        fn create_radio_button(
            &self,
            _parent: ObjectId,
            _text: &str,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            105
        }

        fn create_slider(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            106
        }

        fn create_progress_bar(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            107
        }

        fn create_combo_box(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            108
        }

        fn create_list_box(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            109
        }

        fn create_panel(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            110
        }

        fn create_menu_bar(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            111
        }

        fn create_menu(
            &self,
            _parent: ObjectId,
            _text: &str,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            112
        }

        fn attach_menu_bar_to_window(&self, _window: ObjectId, _menu_bar: ObjectId) -> bool {
            false
        }

        fn menu_add_item(
            &self,
            _parent_menu: ObjectId,
            _text: &str,
            _shortcut: Option<&str>,
        ) -> ObjectId {
            201
        }

        fn create_tool_bar(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            113
        }

        fn create_status_bar(
            &self,
            _parent: ObjectId,
            _text: &str,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            114
        }

        fn create_dialog(
            &self,
            _parent: ObjectId,
            _title: &str,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            115
        }

        #[allow(clippy::too_many_arguments)]
        fn create_message_box(
            &self,
            _parent: ObjectId,
            _title: &str,
            _text: &str,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            116
        }

        fn create_file_dialog(
            &self,
            _parent: ObjectId,
            _title: &str,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            117
        }

        fn create_color_dialog(
            &self,
            _parent: ObjectId,
            _title: &str,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            118
        }

        fn create_font_dialog(
            &self,
            _parent: ObjectId,
            _title: &str,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            119
        }

        fn create_popup_window(
            &self,
            _parent: ObjectId,
            _title: &str,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            120
        }

        fn create_text_edit(
            &self,
            _parent: ObjectId,
            _text: &str,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            121
        }

        fn create_rich_edit(
            &self,
            _parent: ObjectId,
            _text: &str,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            122
        }

        fn create_spin_box(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            123
        }

        fn create_list_view(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            124
        }

        fn create_tree_view(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            125
        }

        fn create_scroll_bar(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            126
        }

        fn create_scroll_area(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            127
        }

        fn create_dock_panel(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            128
        }

        fn create_group_box(
            &self,
            _parent: ObjectId,
            _title: &str,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            129
        }

        fn create_tab_widget(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            130
        }

        fn create_splitter(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            131
        }

        fn create_stack_widget(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            132
        }

        fn create_mdi_area(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            133
        }

        fn create_canvas(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            134
        }

        fn create_table(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            135
        }

        fn create_grid(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            136
        }

        fn create_chart(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            137
        }

        fn create_toggle_button(
            &self,
            _parent: ObjectId,
            _text: &str,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            138
        }

        fn create_check_list_box(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            139
        }

        fn create_double_spin_box(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            140
        }

        fn create_dial(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            141
        }

        fn create_wizard(
            &self,
            _parent: ObjectId,
            _title: &str,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            142
        }

        fn create_date_picker(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            143
        }

        fn create_time_picker(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            144
        }

        fn create_date_time_picker(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            145
        }

        fn create_directory_dialog(
            &self,
            _parent: ObjectId,
            _title: &str,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            146
        }

        fn create_data_view(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            147
        }

        fn create_property_grid(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            148
        }

        fn create_toolbox(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            149
        }

        fn create_collapsible_pane(
            &self,
            _parent: ObjectId,
            _title: &str,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            150
        }

        fn create_dock_widget(
            &self,
            _parent: ObjectId,
            _title: &str,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            151
        }

        fn create_web_view(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            152
        }

        fn create_activity_indicator(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            153
        }

        fn create_calendar(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            154
        }

        fn create_column_view(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            155
        }

        fn create_undo_view(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            156
        }

        fn create_command_link(
            &self,
            _parent: ObjectId,
            _text: &str,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            157
        }

        fn create_lcd_number(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            158
        }

        fn create_font_combo_box(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            159
        }

        fn create_web_engine_view(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            160
        }

        fn create_web_engine_page(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            161
        }

        fn create_web_engine_settings(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            162
        }

        fn create_web_engine_download_item(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            163
        }

        fn create_web_engine_cookie_store(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            164
        }

        fn create_web_engine_web_channel(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            165
        }

        fn create_web_engine_find_text_result(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            166
        }

        fn create_web_engine_notification(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            167
        }

        fn create_web_engine_script_dialog(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            168
        }

        fn create_web_engine_context_menu_request(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            169
        }

        fn create_action(
            &self,
            _parent: ObjectId,
            _text: &str,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            170
        }

        fn create_tool_button(
            &self,
            _parent: ObjectId,
            _text: &str,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            171
        }

        fn create_tool_box(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            172
        }

        fn create_context_menu(
            &self,
            _parent: ObjectId,
            _text: &str,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            173
        }

        fn poll_menu_triggered(&self) -> Option<ObjectId> {
            None
        }

        fn inject_menu_trigger(&self, _menu_item_id: ObjectId) -> bool {
            false
        }

        fn poll_widget_trigger_event(&self) -> Option<WidgetTriggerEvent> {
            None
        }

        fn inject_widget_trigger_event(
            &self,
            _widget_id: ObjectId,
            _kind: WidgetTriggerKind,
        ) -> bool {
            false
        }

        fn set_widget_text(&self, _widget_id: ObjectId, _text: &str) {}

        fn get_widget_text(&self, _widget_id: ObjectId) -> String {
            String::new()
        }

        fn set_widget_enabled(&self, _widget_id: ObjectId, _enabled: bool) {}

        fn is_widget_enabled(&self, _widget_id: ObjectId) -> bool {
            false
        }

        fn set_widget_visible(&self, _widget_id: ObjectId, _visible: bool) {}

        fn is_widget_visible(&self, _widget_id: ObjectId) -> bool {
            false
        }

        fn set_widget_geometry(
            &self,
            _widget_id: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) {
        }

        fn set_widget_ime_enabled(&self, _widget_id: ObjectId, _enabled: bool) -> bool {
            false
        }

        fn is_widget_ime_enabled(&self, _widget_id: ObjectId) -> bool {
            false
        }

        fn set_widget_accessibility_name(&self, _widget_id: ObjectId, _name: &str) -> bool {
            false
        }

        fn get_widget_accessibility_name(&self, _widget_id: ObjectId) -> String {
            String::new()
        }
    }

    #[test]
    fn test_backend_can_be_constructed() {
        let backend = TestBackend;
        assert_eq!(backend.backend_name(), "test-backend");
        assert_eq!(backend.kind(), ControlBackendKind::Native);
    }

    #[test]
    fn create_widget_default_returns_zero() {
        let backend = TestBackend;
        let id = ControlBackend::create_widget(&backend, "test", 0, "hello", 10, 20, 100, 200);
        assert_eq!(id, 0, "default create_widget must return 0");
    }

    #[test]
    fn create_widget_default_with_various_kinds() {
        let backend = TestBackend;
        assert_eq!(
            ControlBackend::create_widget(&backend, "button", 0, "", 0, 0, 50, 30),
            0,
        );
        assert_eq!(
            ControlBackend::create_widget(&backend, "label", 1, "Hello", 5, 5, 80, 20),
            0,
        );
        assert_eq!(
            ControlBackend::create_widget(&backend, "window", 0, "Main", 0, 0, 800, 600),
            0,
        );
    }

    #[test]
    fn test_backend_creates_window() {
        let backend = TestBackend;
        let id = backend.create_window("Test", 0, 0, 800, 600);
        assert_eq!(id, 100);
    }

    #[test]
    fn test_backend_creates_button() {
        let backend = TestBackend;
        let id = backend.create_button(1, "Click", 10, 20, 100, 30);
        assert_eq!(id, 101);
    }

    #[test]
    fn test_backend_creates_label() {
        let backend = TestBackend;
        let id = backend.create_label(0, "Hello", 0, 0, 200, 50);
        assert_eq!(id, 104);
    }

    #[test]
    fn test_backend_show_hide_widget_defaults() {
        let backend = TestBackend;
        // show_widget and hide_widget have default implementations
        backend.show_widget(42);
        backend.hide_widget(42);
        // default set_widget_visible is no-op, so nothing to assert beyond no panic
    }

    #[test]
    fn test_backend_poll_widget_triggered_default() {
        let backend = TestBackend;
        // poll_widget_triggered has a default impl calling poll_widget_trigger_event
        let triggered = backend.poll_widget_triggered();
        assert!(triggered.is_none());
    }

    #[test]
    fn backend_name_variations() {
        let backend = TestBackend;
        let name = backend.backend_name();
        assert!(!name.is_empty(), "backend_name must not be empty");
        assert_eq!(name, "test-backend");
    }

    #[test]
    fn kind_is_native() {
        let backend = TestBackend;
        assert_eq!(backend.kind(), ControlBackendKind::Native);
    }
}
