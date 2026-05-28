//! Widget models and controls.
// Base widget types
pub mod base;
pub mod capability;
pub mod draw;
pub mod image;
pub mod kind;
pub mod widget_trait;
// Widget subfolders
pub mod advanced_widgets;
pub mod base_widgets;
pub mod container_widgets;
pub mod dialog;
pub mod display_widgets;
pub mod input_widgets;
pub mod menu_toolbar;
pub mod registry;
pub mod special_widgets;
pub mod view_widgets;
pub mod web_widgets;
// Individual widget files (not in subfolders)
pub mod svg;
pub mod window;
pub use window::Window;
// Legacy module aliases for backward compatibility paths.

// Re-export base types
pub use base::BaseWidget;
pub use capability::{
    CapabilityAccessError, CapabilityValue, PropertySchema, PropertyValueKind, WidgetCapability,
    WidgetFactory,
};
pub use draw::Draw;
pub use image::Image;
pub use image::ImageFormat;
pub use kind::WidgetKind;
pub use registry::SimpleRegistry;
pub use widget_trait::Widget;
// Re-export widget types from subfolders
pub use base_widgets::{
    button::{Button, ButtonState},
    checkbox::{CheckBox, CheckState},
    label::Label,
    radiobutton::RadioButton,
    toggle_button::{ToggleButton, ToggleButtonState},
};
pub use input_widgets::{
    combobox::ComboBox,
    command_link::CommandLink,
    font_combo_box::FontComboBox,
    lineedit::{EchoMode, LineEdit},
    listbox::{ListBox, SelectionMode},
    rich_edit::RichEdit,
    spinbox::SpinBox,
    textedit::TextEdit,
};
// Re-export container widgets
pub use container_widgets::{
    collapsible_pane::CollapsiblePane, dockwidget::DockWidget, groupbox::GroupBox,
    mdiarea::MdiArea, scrollarea::ScrollArea, splitter::Splitter, stackedwidget::StackedWidget,
    tabwidget::TabWidget, toolbox::ToolBox,
};
pub type Panel = GroupBox;
pub type DockPanel = DockWidget;
// Re-export display widgets
pub use display_widgets::lcd_number::LCDNumber as LcdNumber;
pub use display_widgets::{
    lcd_number::LCDNumber, progressbar::ProgressBar, scrollbar::ScrollBar, slider::Slider,
};
// Re-export web widgets
pub use web_widgets::{web_engine::WebEngine, web_view::WebView};
pub use web_widgets::{
    WebEngineContextMenuRequest, WebEngineCookieStore, WebEngineDownloadItem,
    WebEngineFindTextResult, WebEngineNotification, WebEnginePage, WebEngineScriptDialog,
    WebEngineSettings, WebEngineWebChannel,
};
// Re-export advanced widgets
pub use advanced_widgets::{
    calendar::Calendar, date_edit::DateEdit, date_time_edit::DateTimeEdit, dial::Dial,
    key_sequence_edit::KeySequenceEdit, pie_menu::PieMenu, pie_menu::PieMenuItem,
    ribbon_bar::RibbonBar, ribbon_bar::RibbonGroup, ribbon_bar::RibbonItem, tab_bar::TabBar,
    tab_bar::TabBarTab, time_edit::TimeEdit,
};
// Re-export dialog widgets
pub use dialog::{
    color_dialog::ColorDialog, file_dialog::FileDialog, font_dialog::FontDialog,
    input_dialog::InputDialog, message_box::MessageBox, popup_window::PopupWindow,
    progress_dialog::ProgressDialog,
};
pub type Dialog = PopupWindow;
pub type DirectoryDialog = FileDialog;
// Re-export menu and toolbar widgets
pub use menu_toolbar::{
    action::Action, menu::Menu, menu_bar::MenuBar, status_bar::StatusBar, tool_bar::ToolBar,
    tool_button::ToolButton,
};
pub type ContextMenu = Menu;
// Re-export view widgets
pub use view_widgets::table_widget::TableModel;
pub use view_widgets::tree_view::TreeModel;
pub use view_widgets::{
    list_view::{ListModel, ListView, VecListModel},
    table_view::TableView,
    table_widget::TableWidget,
    tree_view::TreeView,
};
// Re-export special widgets
pub use special_widgets::{Canvas, ChartWidget, FreeformShapeWidget, GridWidget};
pub type ActivityIndicator = ProgressBar;
pub type CheckListBox = ListBox;
pub type Toolbox = ToolBox;
pub type DoubleSpinBox = SpinBox;
pub type Wizard = Panel;
// ── P3-6: WidgetKind variant type aliases ──
pub type DataView = TableWidget;
pub type PropertyGrid = TreeView;
pub type ColumnView = TreeView;
pub type UndoView = ListView;
pub type DatePicker = DateEdit;
pub type TimePicker = TimeEdit;
pub type DateTimePicker = DateTimeEdit;
