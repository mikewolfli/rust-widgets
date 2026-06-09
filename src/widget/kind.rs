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
    InputDialog,
    ProgressDialog,
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
    /// Individual item inside a menu.
    MenuItem,
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
    /// Freeform shape widget — a path-based non-rectangular clickable shape.
    FreeformShape,
    /// Standalone tab bar widget (decoupled from TabWidget).
    TabBar,
    /// Pie menu / radial menu widget.
    PieMenu,
    /// RibbonBar (Office-style ribbon) widget.
    RibbonBar,
    /// Switch/Toggle widget for on/off binary state.
    Switch,
    /// Search box with search icon and clear button.
    SearchBox,
    /// Chip/Tag widget for labels and tokens.
    Chip,
    /// Badge widget for notification counts and status indicators.
    Badge,
    /// Skeleton loader placeholder widget.
    SkeletonLoader,
    /// Floating action button.
    FAB,
    /// Pull-to-refresh control for scrollable views.
    PullToRefresh,
    /// Bottom sheet modal panel.
    BottomSheet,
    /// Bottom navigation bar (mobile tab bar).
    BottomNavigationBar,
    /// Navigation drawer sidebar.
    NavigationDrawer,
    /// Top app bar.
    AppBar,
    /// Mobile-style date picker.
    MobileDatePicker,
    /// Divider/Separator line widget.
    Divider,
    /// Stepper widget for numeric increment/decrement with +/- buttons.
    Stepper,
    /// Star rating control.
    Rating,
    /// Avatar widget — circular/square user image placeholder with initials fallback.
    Avatar,
    /// EmptyState widget — placeholder shown when a view has no content.
    EmptyState,
    /// Carousel/SwipeView widget — horizontal swipeable page carousel with dot indicators.
    Carousel,
    /// ColorWell widget — compact color swatch that shows the current color and emits a signal when clicked.
    ColorWell,
    /// TagInput widget — text input that creates tags/chips on Enter or comma, with removable tags.
    TagInput,
    /// IME preedit text overlay widget for composition text input.
    ImePreedit,
    /// QRCode widget — displays a deterministic QR code pattern from a data string.
    QRCode,
    /// MasonryLayout widget — a Pinterest-style waterfall grid layout.
    MasonryLayout,
    /// CupertinoSwitch — iOS-style switch (alias for Switch with iOS coloring).
    CupertinoSwitch,
    /// MaterialSnackbar — Material Design snackbar notification.
    MaterialSnackbar,
    /// AdaptiveScaffold — cross-platform adaptive scaffold with AppBar + content + bottom nav.
    AdaptiveScaffold,
    /// WizardDialog — step-by-step wizard control with back/next/finish navigation.
    WizardDialog,
    /// SafeArea — mobile safe area widget that insets content to avoid notches, status bars, and home indicators.
    SafeArea,
    /// CupertinoAlertDialog — iOS-style alert dialog with title, message, and buttons.
    CupertinoAlertDialog,
    /// CupertinoSlider — iOS-style slider with rounded track and circular knob.
    CupertinoSlider,
    /// MaterialNavigationRail — Material Design side navigation rail for tablets.
    MaterialNavigationRail,
}
