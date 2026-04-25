use crate::control_backend::types::ControlRoutePreference;
use crate::widget::WidgetKind;
/// Returns the policy preference for one widget kind.
pub fn route_preference_for_widget_kind(kind: WidgetKind) -> ControlRoutePreference {
    match kind {
        WidgetKind::TabBar
        | WidgetKind::Window
        | WidgetKind::Dialog
        | WidgetKind::MessageBox
        | WidgetKind::FileDialog
        | WidgetKind::ColorDialog
        | WidgetKind::FontDialog
        | WidgetKind::InputDialog
        | WidgetKind::ProgressDialog
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
        | WidgetKind::MenuItem
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
        | WidgetKind::FontComboBox
        | WidgetKind::PieMenu
        | WidgetKind::RibbonBar => ControlRoutePreference::NativePreferred,
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
        | WidgetKind::FreeformShape
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
        WidgetKind::StackedWidget
        | WidgetKind::Action
        | WidgetKind::ToolButton
        | WidgetKind::ToolBox => ControlRoutePreference::CustomRequired,
    }
}
