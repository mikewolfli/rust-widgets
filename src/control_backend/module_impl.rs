//! Control backend abstraction for native and custom-painted control paths.

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

#[cfg(feature = "controls-custom")]
use std::sync::OnceLock;

use crate::core::ObjectId;
use crate::platform::{get_platform, WidgetTriggerEvent, WidgetTriggerKind};
use crate::widget::WidgetKind;

/// Control backend family used by runtime routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlBackendKind {
    /// Native platform control implementation.
    Native,
    /// Custom-painted control implementation.
    Custom,
}

/// Compile-time control route preference for a widget kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlRoutePreference {
    /// Prefer native backend when available.
    NativePreferred,
    /// Require custom-painted backend route.
    CustomRequired,
}

/// Returns the policy preference for one widget kind.
pub fn route_preference_for_widget_kind(kind: WidgetKind) -> ControlRoutePreference {
    match kind {
        WidgetKind::Window
        | WidgetKind::Dialog
        | WidgetKind::MessageBox
        | WidgetKind::FileDialog
        | WidgetKind::ColorDialog
        | WidgetKind::FontDialog
        | WidgetKind::PopupWindow
        | WidgetKind::Button
        | WidgetKind::CheckBox
        | WidgetKind::RadioButton
        | WidgetKind::Label
        | WidgetKind::LineEdit
        | WidgetKind::ComboBox
        | WidgetKind::SpinBox
        | WidgetKind::ListBox
        | WidgetKind::ProgressBar
        | WidgetKind::Slider
        | WidgetKind::ScrollBar
        | WidgetKind::ScrollArea
        | WidgetKind::Panel
        | WidgetKind::GroupBox
        | WidgetKind::TabWidget
        | WidgetKind::Splitter
        | WidgetKind::MenuBar
        | WidgetKind::Menu
        | WidgetKind::ContextMenu
        | WidgetKind::ToolBar
        | WidgetKind::StatusBar
        | WidgetKind::ToggleButton
        | WidgetKind::DoubleSpinBox
        | WidgetKind::Dial
        | WidgetKind::DatePicker
        | WidgetKind::TimePicker
        | WidgetKind::DateTimePicker
        | WidgetKind::DirectoryDialog
        | WidgetKind::ActivityIndicator
        | WidgetKind::Calendar
        | WidgetKind::LCDNumber
        | WidgetKind::FontComboBox => ControlRoutePreference::NativePreferred,
        WidgetKind::TextEdit
        | WidgetKind::RichEdit
        | WidgetKind::ListView
        | WidgetKind::TreeView
        | WidgetKind::DockPanel
        | WidgetKind::MdiArea
        | WidgetKind::Canvas
        | WidgetKind::Table
        | WidgetKind::Grid
        | WidgetKind::Chart
        | WidgetKind::CheckListBox
        | WidgetKind::Wizard
        | WidgetKind::DataView
        | WidgetKind::PropertyGrid
        | WidgetKind::Toolbox
        | WidgetKind::CollapsiblePane
        | WidgetKind::DockWidget
        | WidgetKind::WebView
        | WidgetKind::ColumnView
        | WidgetKind::UndoView
        | WidgetKind::CommandLink
        | WidgetKind::WebEngineView
        | WidgetKind::WebEnginePage
        | WidgetKind::WebEngineSettings
        | WidgetKind::WebEngineDownloadItem
        | WidgetKind::WebEngineCookieStore
        | WidgetKind::WebEngineWebChannel
        | WidgetKind::WebEngineFindTextResult
        | WidgetKind::WebEngineNotification
        | WidgetKind::WebEngineScriptDialog
        | WidgetKind::WebEngineContextMenuRequest => ControlRoutePreference::CustomRequired,
        WidgetKind::StackedWidget => ControlRoutePreference::CustomRequired,
    }
}

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
        _parent: ObjectId,
        title: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        get_platform().create_window(title, x, y, width, height)
    }

    fn create_message_box(
        &self,
        _parent: ObjectId,
        title: &str,
        _text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        get_platform().create_window(title, x, y, width, height)
    }

    fn create_file_dialog(
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

    fn create_color_dialog(
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

    fn create_font_dialog(
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
        get_platform().create_line_edit(parent, "", x, y, width, height)
    }

    fn create_list_view(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        get_platform().create_list_box(parent, x, y, width, height)
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

impl Default for CustomControlState {
    fn default() -> Self {
        Self {
            next_widget_id: 1,
            texts: HashMap::new(),
            enabled: HashMap::new(),
            visible: HashMap::new(),
            ime_enabled: HashMap::new(),
            accessibility_names: HashMap::new(),
            menu_trigger_queue: VecDeque::new(),
            widget_trigger_queue: VecDeque::new(),
            widget_properties: HashMap::new(),
        }
    }
}

struct CustomControlState {
    next_widget_id: ObjectId,
    texts: HashMap<ObjectId, String>,
    enabled: HashMap<ObjectId, bool>,
    visible: HashMap<ObjectId, bool>,
    ime_enabled: HashMap<ObjectId, bool>,
    accessibility_names: HashMap<ObjectId, String>,
    menu_trigger_queue: VecDeque<ObjectId>,
    widget_trigger_queue: VecDeque<WidgetTriggerEvent>,
    // Store widget properties for custom painting
    widget_properties: HashMap<ObjectId, CustomWidgetProperties>,
}

#[allow(dead_code)]
struct CustomWidgetProperties {
    parent: Option<ObjectId>,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    widget_kind: WidgetKind,
}

/// Custom-painted control backend scaffold.
pub struct CustomPaintControlBackend {
    state: Mutex<CustomControlState>,
}

impl CustomPaintControlBackend {
    /// Create custom-painted control backend.
    pub fn new() -> Self {
        Self {
            state: Mutex::new(CustomControlState {
                next_widget_id: 1,
                ..CustomControlState::default()
            }),
        }
    }

    fn alloc_widget_id(&self) -> ObjectId {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let widget_id = state.next_widget_id;
        state.next_widget_id = state.next_widget_id.saturating_add(1);
        widget_id
    }
}

impl Default for CustomPaintControlBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl ControlBackend for CustomPaintControlBackend {
    fn backend_name(&self) -> &'static str {
        "custom-paint-control-backend"
    }

    fn kind(&self) -> ControlBackendKind {
        ControlBackendKind::Custom
    }

    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.texts.insert(widget_id, title.to_string());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, title.to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: None,
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::Window,
            },
        );
        widget_id
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
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.texts.insert(widget_id, text.to_string());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, text.to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::Button,
            },
        );
        widget_id
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
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.texts.insert(widget_id, text.to_string());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, text.to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::CheckBox,
            },
        );
        widget_id
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
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.texts.insert(widget_id, text.to_string());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, true); // LineEdit enables IME by default
        state
            .accessibility_names
            .insert(widget_id, text.to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::LineEdit,
            },
        );
        widget_id
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
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.texts.insert(widget_id, text.to_string());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, text.to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::Label,
            },
        );
        widget_id
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
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.texts.insert(widget_id, text.to_string());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, text.to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::RadioButton,
            },
        );
        widget_id
    }

    fn create_slider(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, "Slider".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::Slider,
            },
        );
        widget_id
    }

    fn create_progress_bar(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, "ProgressBar".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::ProgressBar,
            },
        );
        widget_id
    }

    fn create_combo_box(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, "ComboBox".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::ComboBox,
            },
        );
        widget_id
    }

    fn create_list_box(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, "ListBox".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::ListBox,
            },
        );
        widget_id
    }

    fn create_panel(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, "Panel".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::Panel,
            },
        );
        widget_id
    }

    fn create_menu_bar(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, "MenuBar".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::MenuBar,
            },
        );
        widget_id
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
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.texts.insert(widget_id, text.to_string());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, text.to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::Menu,
            },
        );
        widget_id
    }

    fn attach_menu_bar_to_window(&self, _window: ObjectId, _menu_bar: ObjectId) -> bool {
        true
    }

    fn menu_add_item(
        &self,
        parent_menu: ObjectId,
        text: &str,
        _shortcut: Option<&str>,
    ) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.texts.insert(widget_id, text.to_string());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, text.to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent_menu),
                x: 0,
                y: 0,
                width: 0,
                height: 0,
                widget_kind: WidgetKind::Menu,
            },
        );
        widget_id
    }

    fn create_tool_bar(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, "ToolBar".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::ToolBar,
            },
        );
        widget_id
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
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.texts.insert(widget_id, text.to_string());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, text.to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::StatusBar,
            },
        );
        widget_id
    }

    fn poll_menu_triggered(&self) -> Option<ObjectId> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .menu_trigger_queue
            .pop_front()
    }

    fn inject_menu_trigger(&self, menu_item_id: ObjectId) -> bool {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.menu_trigger_queue.push_back(menu_item_id);
        true
    }

    fn poll_widget_trigger_event(&self) -> Option<WidgetTriggerEvent> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .widget_trigger_queue
            .pop_front()
    }

    fn inject_widget_trigger_event(&self, widget_id: ObjectId, kind: WidgetTriggerKind) -> bool {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state
            .widget_trigger_queue
            .push_back(WidgetTriggerEvent { widget_id, kind });
        true
    }

    fn set_widget_text(&self, widget_id: ObjectId, text: &str) {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .texts
            .insert(widget_id, text.to_string());
    }

    fn get_widget_text(&self, widget_id: ObjectId) -> String {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .texts
            .get(&widget_id)
            .cloned()
            .unwrap_or_default()
    }

    fn set_widget_enabled(&self, widget_id: ObjectId, enabled: bool) {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .enabled
            .insert(widget_id, enabled);
    }

    fn is_widget_enabled(&self, widget_id: ObjectId) -> bool {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .enabled
            .get(&widget_id)
            .copied()
            .unwrap_or(false)
    }

    fn set_widget_visible(&self, widget_id: ObjectId, visible: bool) {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .visible
            .insert(widget_id, visible);
    }

    fn is_widget_visible(&self, widget_id: ObjectId) -> bool {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .visible
            .get(&widget_id)
            .copied()
            .unwrap_or(false)
    }

    fn set_widget_geometry(&self, widget_id: ObjectId, x: i32, y: i32, width: u32, height: u32) {
        if let Some(props) = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .widget_properties
            .get_mut(&widget_id)
        {
            props.x = x;
            props.y = y;
            props.width = width;
            props.height = height;
        }
    }

    fn set_widget_ime_enabled(&self, widget_id: ObjectId, enabled: bool) -> bool {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .ime_enabled
            .insert(widget_id, enabled);
        true
    }

    fn is_widget_ime_enabled(&self, widget_id: ObjectId) -> bool {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .ime_enabled
            .get(&widget_id)
            .copied()
            .unwrap_or(false)
    }

    fn set_widget_accessibility_name(&self, widget_id: ObjectId, name: &str) -> bool {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .accessibility_names
            .insert(widget_id, name.to_string());
        true
    }

    fn get_widget_accessibility_name(&self, widget_id: ObjectId) -> String {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .accessibility_names
            .get(&widget_id)
            .cloned()
            .unwrap_or_default()
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
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.texts.insert(widget_id, title.to_string());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, title.to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::Dialog,
            },
        );
        widget_id
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
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.texts.insert(widget_id, text.to_string());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, title.to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::MessageBox,
            },
        );
        widget_id
    }

    fn create_file_dialog(
        &self,
        parent: ObjectId,
        title: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.texts.insert(widget_id, title.to_string());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, title.to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::FileDialog,
            },
        );
        widget_id
    }

    fn create_color_dialog(
        &self,
        parent: ObjectId,
        title: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.texts.insert(widget_id, title.to_string());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, title.to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::ColorDialog,
            },
        );
        widget_id
    }

    fn create_font_dialog(
        &self,
        parent: ObjectId,
        title: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.texts.insert(widget_id, title.to_string());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, title.to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::FontDialog,
            },
        );
        widget_id
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
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.texts.insert(widget_id, title.to_string());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, title.to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::PopupWindow,
            },
        );
        widget_id
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
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.texts.insert(widget_id, text.to_string());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, true); // TextEdit enables IME by default
        state
            .accessibility_names
            .insert(widget_id, text.to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::TextEdit,
            },
        );
        widget_id
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
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.texts.insert(widget_id, text.to_string());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, true); // RichEdit enables IME by default
        state
            .accessibility_names
            .insert(widget_id, text.to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::RichEdit,
            },
        );
        widget_id
    }

    fn create_spin_box(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.texts.insert(widget_id, "0".to_string());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, true); // SpinBox enables IME by default
        state
            .accessibility_names
            .insert(widget_id, "SpinBox".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::SpinBox,
            },
        );
        widget_id
    }

    fn create_list_view(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, "ListView".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::ListView,
            },
        );
        widget_id
    }

    fn create_tree_view(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, "TreeView".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::TreeView,
            },
        );
        widget_id
    }

    fn create_scroll_bar(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, "ScrollBar".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::ScrollBar,
            },
        );
        widget_id
    }

    fn create_scroll_area(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, "ScrollArea".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::ScrollArea,
            },
        );
        widget_id
    }

    fn create_dock_panel(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, "DockPanel".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::DockPanel,
            },
        );
        widget_id
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
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.texts.insert(widget_id, title.to_string());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, title.to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::GroupBox,
            },
        );
        widget_id
    }

    fn create_tab_widget(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, "TabWidget".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::TabWidget,
            },
        );
        widget_id
    }

    fn create_splitter(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, "Splitter".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::Splitter,
            },
        );
        widget_id
    }

    fn create_stack_widget(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, "StackWidget".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::StackedWidget,
            },
        );
        widget_id
    }

    fn create_mdi_area(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, "MdiArea".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::MdiArea,
            },
        );
        widget_id
    }

    fn create_canvas(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, "Canvas".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::Canvas,
            },
        );
        widget_id
    }

    fn create_table(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, "Table".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::Table,
            },
        );
        widget_id
    }

    fn create_grid(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, "Grid".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::Grid,
            },
        );
        widget_id
    }

    fn create_chart(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        let widget_id = self.alloc_widget_id();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.enabled.insert(widget_id, true);
        state.visible.insert(widget_id, true);
        state.ime_enabled.insert(widget_id, false);
        state
            .accessibility_names
            .insert(widget_id, "Chart".to_string());
        state.widget_properties.insert(
            widget_id,
            CustomWidgetProperties {
                parent: Some(parent),
                x,
                y,
                width,
                height,
                widget_kind: WidgetKind::Chart,
            },
        );
        widget_id
    }
}

fn native_control_backend() -> &'static NativeControlBackend {
    static BACKEND: NativeControlBackend = NativeControlBackend::new();
    &BACKEND
}

#[cfg(feature = "controls-custom")]
fn custom_control_backend() -> &'static CustomPaintControlBackend {
    static BACKEND: OnceLock<CustomPaintControlBackend> = OnceLock::new();
    BACKEND.get_or_init(CustomPaintControlBackend::new)
}

/// Return active control backend selected by compile-time features.
#[cfg(all(feature = "controls-native", feature = "controls-custom"))]
pub fn get_control_backend() -> &'static dyn ControlBackend {
    native_control_backend()
}

/// Return active control backend selected by compile-time features.
#[cfg(all(not(feature = "controls-native"), feature = "controls-custom"))]
pub fn get_control_backend() -> &'static dyn ControlBackend {
    custom_control_backend()
}

/// Return active control backend selected by compile-time features.
#[cfg(all(feature = "controls-native", not(feature = "controls-custom")))]
pub fn get_control_backend() -> &'static dyn ControlBackend {
    native_control_backend()
}

/// Return active control backend selected by compile-time features.
#[cfg(all(not(feature = "controls-native"), not(feature = "controls-custom")))]
pub fn get_control_backend() -> &'static dyn ControlBackend {
    native_control_backend()
}

/// Returns control backend resolved by compile-time policy for one widget kind.
#[cfg(all(feature = "controls-native", feature = "controls-custom"))]
pub fn get_control_backend_for_widget(kind: WidgetKind) -> &'static dyn ControlBackend {
    match route_preference_for_widget_kind(kind) {
        ControlRoutePreference::NativePreferred => native_control_backend(),
        ControlRoutePreference::CustomRequired => custom_control_backend(),
    }
}

/// Returns control backend resolved by compile-time policy for one widget kind.
#[cfg(all(not(feature = "controls-native"), feature = "controls-custom"))]
pub fn get_control_backend_for_widget(_kind: WidgetKind) -> &'static dyn ControlBackend {
    custom_control_backend()
}

/// Returns control backend resolved by compile-time policy for one widget kind.
#[cfg(all(feature = "controls-native", not(feature = "controls-custom")))]
pub fn get_control_backend_for_widget(_kind: WidgetKind) -> &'static dyn ControlBackend {
    native_control_backend()
}

/// Returns control backend resolved by compile-time policy for one widget kind.
#[cfg(all(not(feature = "controls-native"), not(feature = "controls-custom")))]
pub fn get_control_backend_for_widget(_kind: WidgetKind) -> &'static dyn ControlBackend {
    native_control_backend()
}

/// Return compile-time control policy label used by diagnostics and docs.
#[cfg(all(feature = "controls-native", feature = "controls-custom"))]
pub fn active_control_policy() -> &'static str {
    "hybrid-native-first"
}

/// Return compile-time control policy label used by diagnostics and docs.
#[cfg(all(not(feature = "controls-native"), feature = "controls-custom"))]
pub fn active_control_policy() -> &'static str {
    "custom-full"
}

/// Return compile-time control policy label used by diagnostics and docs.
#[cfg(all(feature = "controls-native", not(feature = "controls-custom")))]
pub fn active_control_policy() -> &'static str {
    "native-strict"
}

/// Return compile-time control policy label used by diagnostics and docs.
#[cfg(all(not(feature = "controls-native"), not(feature = "controls-custom")))]
pub fn active_control_policy() -> &'static str {
    "native-strict"
}
