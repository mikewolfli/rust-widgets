//! Widget kind enum — discrete categories supported by the widget model layer.

/// Discrete widget categories supported by the widget model layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WidgetKind {
    /// Top-level window.
    Window,
    #[cfg(not(feature = "mini"))]
    Dialog,
    #[cfg(not(feature = "mini"))]
    MessageBox,
    #[cfg(not(feature = "mini"))]
    FileDialog,
    #[cfg(not(feature = "mini"))]
    ColorDialog,
    #[cfg(not(feature = "mini"))]
    FontDialog,
    #[cfg(not(feature = "mini"))]
    InputDialog,
    #[cfg(not(feature = "mini"))]
    ProgressDialog,
    #[cfg(not(feature = "mini"))]
    PopupWindow,
    Button,
    CheckBox,
    RadioButton,
    Label,
    LineEdit,
    #[cfg(not(feature = "mini"))]
    TextEdit,
    #[cfg(not(feature = "mini"))]
    RichEdit,
    ComboBox,
    SpinBox,
    ListBox,
    #[cfg(not(feature = "mini"))]
    ListView,
    #[cfg(not(feature = "mini"))]
    TreeView,
    ProgressBar,
    Slider,
    ScrollBar,
    ScrollArea,
    Panel,
    Frame,
    #[cfg(not(feature = "mini"))]
    DockPanel,
    GroupBox,
    #[cfg(not(feature = "mini"))]
    TabWidget,
    #[cfg(not(feature = "mini"))]
    Splitter,
    #[cfg(not(feature = "mini"))]
    MdiArea,
    #[cfg(not(feature = "mini"))]
    MenuBar,
    #[cfg(not(feature = "mini"))]
    Menu,
    /// Individual item inside a menu.
    #[cfg(not(feature = "mini"))]
    MenuItem,
    #[cfg(not(feature = "mini"))]
    ContextMenu,
    #[cfg(not(feature = "mini"))]
    ToolBar,
    #[cfg(not(feature = "mini"))]
    StatusBar,
    #[cfg(not(feature = "mini"))]
    Canvas,
    #[cfg(not(feature = "mini"))]
    Table,
    #[cfg(not(feature = "mini"))]
    Grid,
    /// Chart surface widget.
    #[cfg(not(feature = "mini"))]
    Chart,
    #[cfg(not(feature = "mini"))]
    ToggleButton,
    #[cfg(not(feature = "mini"))]
    CheckListBox,
    #[cfg(not(feature = "mini"))]
    DoubleSpinBox,
    #[cfg(not(feature = "mini"))]
    Dial,
    #[cfg(not(feature = "mini"))]
    Wizard,
    #[cfg(not(feature = "mini"))]
    DatePicker,
    #[cfg(not(feature = "mini"))]
    TimePicker,
    #[cfg(not(feature = "mini"))]
    DateTimePicker,
    #[cfg(not(feature = "mini"))]
    DirectoryDialog,
    #[cfg(not(feature = "mini"))]
    DataView,
    #[cfg(not(feature = "mini"))]
    PropertyGrid,
    #[cfg(not(feature = "mini"))]
    Toolbox,
    #[cfg(not(feature = "mini"))]
    StackedWidget,
    #[cfg(not(feature = "mini"))]
    CollapsiblePane,
    #[cfg(not(feature = "mini"))]
    DockWidget,
    #[cfg(not(feature = "mini"))]
    ActivityIndicator,
    #[cfg(not(feature = "mini"))]
    Calendar,
    #[cfg(not(feature = "mini"))]
    ColumnView,
    #[cfg(not(feature = "mini"))]
    UndoView,
    #[cfg(not(feature = "mini"))]
    CommandLink,
    #[cfg(not(feature = "mini"))]
    LCDNumber,
    #[cfg(not(feature = "mini"))]
    FontComboBox,
    /// Web engine view widget for displaying web content.
    #[cfg(not(feature = "mini"))]
    WebEngineView,
    /// Web engine page widget for managing web content.
    #[cfg(not(feature = "mini"))]
    WebEnginePage,
    /// Web engine settings widget for configuring web engine behavior.
    #[cfg(not(feature = "mini"))]
    WebEngineSettings,
    /// Web engine download item widget for managing downloads.
    #[cfg(not(feature = "mini"))]
    WebEngineDownloadItem,
    /// Web engine cookie store widget for managing cookies.
    #[cfg(not(feature = "mini"))]
    WebEngineCookieStore,
    /// Web engine web channel widget for JavaScript communication.
    #[cfg(not(feature = "mini"))]
    WebEngineWebChannel,
    /// Web engine find text result widget for text search results.
    #[cfg(not(feature = "mini"))]
    WebEngineFindTextResult,
    /// Web engine notification widget for web notifications.
    #[cfg(not(feature = "mini"))]
    WebEngineNotification,
    /// Web engine script dialog widget for JavaScript dialogs.
    #[cfg(not(feature = "mini"))]
    WebEngineScriptDialog,
    /// Web engine context menu request widget for context menu handling.
    #[cfg(not(feature = "mini"))]
    WebEngineContextMenuRequest,
    /// Action widget for menu and toolbar actions.
    #[cfg(not(feature = "mini"))]
    Action,
    /// Tool button widget.
    #[cfg(not(feature = "mini"))]
    ToolButton,
    /// Freeform shape widget — a path-based non-rectangular clickable shape.
    #[cfg(not(feature = "mini"))]
    FreeformShape,
    /// Standalone tab bar widget (decoupled from TabWidget).
    #[cfg(not(feature = "mini"))]
    TabBar,
    /// Pie menu / radial menu widget.
    #[cfg(not(feature = "mini"))]
    PieMenu,
    /// RibbonBar (Office-style ribbon) widget.
    #[cfg(not(feature = "mini"))]
    RibbonBar,
    /// TileView widget — swipeable tiled page view (BLUE13 R2.8).
    TileView,
    /// Line widget — horizontal or vertical divider line (BLUE13 R2.13).
    Line,
    /// Meter widget — gauge with arc and needle (BLUE13 R2.14).
    Meter,
    /// MiniChart widget — simplified line/bar chart (BLUE13 R2.10).
    MiniChart,
    /// ImageView widget — displays an Image as a widget (BLUE13 R2.12).
    ImageView,
    /// MiniCanvas widget — simplified drawing surface (BLUE13 R2.11).
    MiniCanvas,
    /// Arc widget — circular progress/indicator (BLUE13 R2.1).
    Arc,
    /// Spinner widget — rotating loading indicator (BLUE13 R2.2).
    Spinner,
    /// Roller widget — scroll-wheel style selector (BLUE13 R2.3).
    Roller,
    /// Dropdown widget — standalone dropdown list selector (BLUE13 R2.4).
    Dropdown,
    /// TextArea widget — multi-line text input (BLUE13 R2.5).
    TextArea,
    /// Keyboard widget — on-screen virtual keyboard (BLUE13 R2.6).
    Keyboard,
    /// Switch/Toggle widget for on/off binary state.
    Switch,
    /// Search box with search icon and clear button.
    #[cfg(not(feature = "mini"))]
    SearchBox,
    /// Chip/Tag widget for labels and tokens.
    #[cfg(not(feature = "mini"))]
    Chip,
    /// Badge widget for notification counts and status indicators.
    #[cfg(not(feature = "mini"))]
    Badge,
    /// Skeleton loader placeholder widget.
    #[cfg(not(feature = "mini"))]
    SkeletonLoader,
    /// Floating action button.
    #[cfg(not(feature = "mini"))]
    FAB,
    /// Bottom sheet modal panel.
    #[cfg(not(feature = "mini"))]
    BottomSheet,
    /// Bottom navigation bar (mobile tab bar).
    #[cfg(not(feature = "mini"))]
    BottomNavigationBar,
    /// Navigation drawer sidebar.
    #[cfg(not(feature = "mini"))]
    NavigationDrawer,
    /// Top app bar.
    #[cfg(not(feature = "mini"))]
    AppBar,
    /// Mobile-style date picker.
    #[cfg(not(feature = "mini"))]
    MobileDatePicker,
    /// Divider/Separator line widget.
    #[cfg(not(feature = "mini"))]
    Divider,
    /// Stepper widget for numeric increment/decrement with +/- buttons.
    #[cfg(not(feature = "mini"))]
    Stepper,
    /// Star rating control.
    #[cfg(not(feature = "mini"))]
    Rating,
    /// Avatar widget — circular/square user image placeholder with initials fallback.
    #[cfg(not(feature = "mini"))]
    Avatar,
    /// EmptyState widget — placeholder shown when a view has no content.
    #[cfg(not(feature = "mini"))]
    EmptyState,
    /// Carousel/SwipeView widget — horizontal swipeable page carousel with dot indicators.
    #[cfg(not(feature = "mini"))]
    Carousel,
    /// ColorHistory widget — a color history picker with a swatch grid.
    #[cfg(not(feature = "mini"))]
    ColorHistory,
    /// ColorWell widget — compact color swatch that shows the current color and emits a signal when clicked.
    #[cfg(not(feature = "mini"))]
    ColorWell,
    /// TagInput widget — text input that creates tags/chips on Enter or comma, with removable tags.
    #[cfg(not(feature = "mini"))]
    TagInput,
    /// IME preedit text overlay widget for composition text input.
    #[cfg(not(feature = "mini"))]
    ImePreedit,
    /// InplaceEditor — an in-place text editing control for table/cell editing.
    #[cfg(not(feature = "mini"))]
    InplaceEditor,
    /// QRCode widget — displays a deterministic QR code pattern from a data string.
    #[cfg(not(feature = "mini"))]
    QRCode,
    /// MasonryLayout widget — a Pinterest-style waterfall grid layout.
    #[cfg(not(feature = "mini"))]
    MasonryLayout,
    /// CupertinoSwitch — iOS-style switch (alias for Switch with iOS coloring).
    #[cfg(not(feature = "mini"))]
    CupertinoSwitch,
    /// MaterialSnackbar — Material Design snackbar notification.
    #[cfg(not(feature = "mini"))]
    MaterialSnackbar,
    /// AdaptiveScaffold — cross-platform adaptive scaffold with AppBar + content + bottom nav.
    #[cfg(not(feature = "mini"))]
    AdaptiveScaffold,
    /// WizardDialog — step-by-step wizard control with back/next/finish navigation.
    #[cfg(not(feature = "mini"))]
    WizardDialog,
    /// SafeArea — mobile safe area widget that insets content to avoid notches, status bars, and home indicators.
    #[cfg(not(feature = "mini"))]
    SafeArea,
    /// CupertinoAlertDialog — iOS-style alert dialog with title, message, and buttons.
    #[cfg(not(feature = "mini"))]
    CupertinoAlertDialog,
    /// CupertinoSlider — iOS-style slider with rounded track and circular knob.
    #[cfg(not(feature = "mini"))]
    CupertinoSlider,
    /// MaterialNavigationRail — Material Design side navigation rail for tablets.
    #[cfg(not(feature = "mini"))]
    MaterialNavigationRail,
    /// Tooltip — a popup label that appears on hover for context info.
    #[cfg(not(feature = "mini"))]
    Tooltip,
    /// SegmentedButton — a horizontal group of selectable segments (Material 3 style).
    #[cfg(not(feature = "mini"))]
    SegmentedButton,
    /// NavigationStack — a push/pop page navigation container.
    #[cfg(not(feature = "mini"))]
    NavigationStack,
    /// ProgressCircle — a circular progress indicator.
    #[cfg(not(feature = "mini"))]
    ProgressCircle,
    /// Icon — a widget for rendering simple geometric icon representations.
    #[cfg(not(feature = "mini"))]
    Icon,
    /// DropdownMenu — a cascading/linked dropdown selector with item list.
    #[cfg(not(feature = "mini"))]
    DropdownMenu,
    /// MaskedEdit — a formatted text input with mask-based input constraints.
    #[cfg(not(feature = "mini"))]
    MaskedEdit,
    /// MenuButton — a button that opens a dropdown menu when clicked.
    #[cfg(not(feature = "mini"))]
    MenuButton,
    /// Popover — a floating bubble card with an anchor arrow.
    #[cfg(not(feature = "mini"))]
    Popover,
    /// AutoCompleteEdit — a text input with auto-completion dropdown.
    #[cfg(not(feature = "mini"))]
    AutoCompleteEdit,
    /// MultiSelectComboBox — a combo box that allows multiple selections.
    #[cfg(not(feature = "mini"))]
    MultiSelectComboBox,
    /// RangeSlider — a dual-handle range slider for min-max selection.
    #[cfg(not(feature = "mini"))]
    RangeSlider,
    /// FloatingLabel — a text input with a floating label (Material Design style).
    #[cfg(not(feature = "mini"))]
    FloatingLabel,
    /// FontPreview — a font preview panel for font selection dialogs.
    #[cfg(not(feature = "mini"))]
    FontPreview,
    /// CupertinoNavigationBar — iOS-style large title navigation bar.
    #[cfg(not(feature = "mini"))]
    CupertinoNavigationBar,
    /// CupertinoSegmentedControl — iOS-style pill-shaped segmented control.
    #[cfg(not(feature = "mini"))]
    CupertinoSegmentedControl,
    /// SwipeToDismiss — swipe-to-dismiss/delete gesture container.
    #[cfg(not(feature = "mini"))]
    SwipeToDismiss,
    /// PagerPageView — horizontal page view with dot indicators.
    #[cfg(not(feature = "mini"))]
    PagerPageView,
    /// TabView — iOS-style segmented tab page view.
    #[cfg(not(feature = "mini"))]
    TabView,
    /// SearchBar — iOS-style search bar with cancel button.
    #[cfg(not(feature = "mini"))]
    SearchBar,
    /// ShortcutEditor — a keyboard shortcut editor widget.
    #[cfg(not(feature = "mini"))]
    ShortcutEditor,
    /// RefreshControl — pull-to-refresh control for scrollable views.
    #[cfg(not(feature = "mini"))]
    RefreshControl,
    /// ModalBottomSheet — Material-style modal bottom sheet with drag-to-dismiss.
    #[cfg(not(feature = "mini"))]
    ModalBottomSheet,
    /// LineChart — a 2D line chart for visualizing data series.
    #[cfg(not(feature = "mini"))]
    LineChart,
    /// Sparkline — a compact inline sparkline chart without axes.
    #[cfg(not(feature = "mini"))]
    Sparkline,
    /// BarChart — a vertical bar chart for categorical data visualization.
    #[cfg(not(feature = "mini"))]
    BarChart,
    /// FindReplaceDialog — a find/replace dialog with text input, toggles, and action buttons.
    #[cfg(not(feature = "mini"))]
    FindReplaceDialog,
    /// PropertiesPanel — a categorized property editor panel with grid layout.
    #[cfg(not(feature = "mini"))]
    PropertiesPanel,
    /// PieChart — a circular statistical chart with colored sectors.
    #[cfg(not(feature = "mini"))]
    PieChart,
    /// CupertinoDatePicker — iOS UIPickerView-style scrolling wheel date picker.
    #[cfg(not(feature = "mini"))]
    CupertinoDatePicker,
    /// EditableComboBox — a combo box that allows typing custom values.
    #[cfg(not(feature = "mini"))]
    EditableComboBox,
    /// DateRangePicker — a calendar-based date range selection widget.
    #[cfg(not(feature = "mini"))]
    DateRangePicker,
    /// AnimatedImage — plays animated images (GIF/APNG/WebP frame sequences).
    #[cfg(not(feature = "mini"))]
    AnimatedImage,
    /// HeroAnimation — shared element transition with interpolated position/size/opacity.
    #[cfg(not(feature = "mini"))]
    HeroAnimation,
    /// BezierCurveEditor — interactive cubic bezier curve editor for custom easing curves.
    #[cfg(not(feature = "mini"))]
    BezierCurveEditor,
    /// LottieWidget — Lottie JSON animation player.
    #[cfg(not(feature = "mini"))]
    LottieWidget,
    /// RiveWidget — Rive animation runtime widget.
    #[cfg(not(feature = "mini"))]
    RiveWidget,
    /// VideoPlayer — video player widget with playback controls.
    #[cfg(not(feature = "mini"))]
    VideoPlayer,
    /// ImageGallery — image gallery/browser with thumbnails.
    #[cfg(not(feature = "mini"))]
    ImageGallery,
    /// AudioVisualizer — real-time audio waveform/spectrum visualization.
    #[cfg(not(feature = "mini"))]
    AudioVisualizer,
    /// CameraPreview — camera viewfinder preview area with controls.
    #[cfg(not(feature = "mini"))]
    CameraPreview,
    /// BarcodeScanner — barcode/QR code scanner viewfinder with detection.
    #[cfg(not(feature = "mini"))]
    BarcodeScanner,
}
