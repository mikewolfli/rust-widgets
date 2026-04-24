//! Widget kind enum — discrete categories supported by the widget model layer.

/// Discrete widget categories supported by the widget model layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetKind {
    /// Top-level window.
    Window,
    Dialog,
    MessageBox,
    FileDialog,
    ColorDialog,
    FontDialog,
    PopupWindow,
    Button,
    CheckBox,
    RadioButton,
    Label,
    LineEdit,
    TextEdit,
    RichEdit,
    ComboBox,
    SpinBox,
    ListBox,
    ListView,
    TreeView,
    ProgressBar,
    Slider,
    ScrollBar,
    ScrollArea,
    Panel,
    DockPanel,
    GroupBox,
    TabWidget,
    Splitter,
    MdiArea,
    MenuBar,
    Menu,
    ContextMenu,
    ToolBar,
    StatusBar,
    Canvas,
    Table,
    Grid,
    /// Chart surface widget.
    Chart,
    ToggleButton,
    CheckListBox,
    DoubleSpinBox,
    Dial,
    Wizard,
    DatePicker,
    TimePicker,
    DateTimePicker,
    DirectoryDialog,
    DataView,
    PropertyGrid,
    Toolbox,
    StackedWidget,
    CollapsiblePane,
    DockWidget,
    WebView,
    ActivityIndicator,
    Calendar,
    ColumnView,
    UndoView,
    CommandLink,
    LCDNumber,
    FontComboBox,
    /// Web engine view widget for displaying web content.
    WebEngineView,
    /// Web engine page widget for managing web content.
    WebEnginePage,
    /// Web engine settings widget for configuring web engine behavior.
    WebEngineSettings,
    /// Web engine download item widget for managing downloads.
    WebEngineDownloadItem,
    /// Web engine cookie store widget for managing cookies.
    WebEngineCookieStore,
    /// Web engine web channel widget for JavaScript communication.
    WebEngineWebChannel,
    /// Web engine find text result widget for text search results.
    WebEngineFindTextResult,
    /// Web engine notification widget for web notifications.
    WebEngineNotification,
    /// Web engine script dialog widget for JavaScript dialogs.
    WebEngineScriptDialog,
    /// Web engine context menu request widget for context menu handling.
    WebEngineContextMenuRequest,
    /// Action widget for menu and toolbar actions.
    Action,
    /// Tool button widget.
    ToolButton,
    /// Tool box widget (alias with capital B).
    ToolBox,
}
