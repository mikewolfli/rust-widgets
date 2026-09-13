// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Widget kind enum — discrete categories supported by the widget model layer.

#[cfg(all(feature = "serde", not(any(feature = "mini", feature = "embedded"))))]
use serde::{Deserialize, Serialize};

/// Discrete widget categories supported by the widget model layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(
    all(feature = "serde", not(any(feature = "mini", feature = "embedded"))),
    derive(Serialize, Deserialize)
)]
pub enum WidgetKind {
    /// Top-level window.
    Window,
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    Dialog,
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    MessageBox,
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    FileDialog,
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    ColorDialog,
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    FontDialog,
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    InputDialog,
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    ProgressDialog,
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    PopupWindow,
    Button,
    CheckBox,
    RadioButton,
    Label,
    LineEdit,
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    TextEdit,
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    RichEdit,
    ComboBox,
    SpinBox,
    ListBox,
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    ListView,
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    TreeView,
    ProgressBar,
    Slider,
    ScrollBar,
    ScrollArea,
    Panel,
    Frame,
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    DockPanel,
    GroupBox,
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    TabWidget,
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    Splitter,
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    MdiArea,
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    MenuBar,
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    Menu,
    /// Individual item inside a menu.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    MenuItem,
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    ContextMenu,
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    ToolBar,
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    StatusBar,
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    Canvas,
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    Table,
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    Grid,
    /// Chart surface widget.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    Chart,
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    ToggleButton,
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    CheckListBox,
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    DoubleSpinBox,
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    Dial,
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    Wizard,
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    DatePicker,
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    TimePicker,
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    DateTimePicker,
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    DirectoryDialog,
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    DataView,
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    PropertyGrid,
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    Toolbox,
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    StackedWidget,
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    CollapsiblePane,
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    DockWidget,
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    ActivityIndicator,
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    Calendar,
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    ColumnView,
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    UndoView,
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    CommandLink,
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    LCDNumber,
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    FontComboBox,
    /// Web engine view widget for displaying web content.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    WebEngineView,
    /// Web engine page widget for managing web content.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    WebEnginePage,
    /// Web engine settings widget for configuring web engine behavior.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    WebEngineSettings,
    /// Web engine download item widget for managing downloads.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    WebEngineDownloadItem,
    /// Web engine cookie store widget for managing cookies.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    WebEngineCookieStore,
    /// Web engine web channel widget for JavaScript communication.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    WebEngineWebChannel,
    /// Web engine find text result widget for text search results.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    WebEngineFindTextResult,
    /// Web engine notification widget for web notifications.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    WebEngineNotification,
    /// Web engine script dialog widget for JavaScript dialogs.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    WebEngineScriptDialog,
    /// Web engine context menu request widget for context menu handling.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    WebEngineContextMenuRequest,
    /// Action widget for menu and toolbar actions.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    Action,
    /// Tool button widget.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    ToolButton,
    /// Freeform shape widget — a path-based non-rectangular clickable shape.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    FreeformShape,
    /// Standalone tab bar widget (decoupled from TabWidget).
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    TabBar,
    /// Pie menu / radial menu widget.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    PieMenu,
    /// RibbonBar (Office-style ribbon) widget.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
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
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    SearchBox,
    /// Chip/Tag widget for labels and tokens.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    Chip,
    /// Badge widget for notification counts and status indicators.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    Badge,
    /// Skeleton loader placeholder widget.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    SkeletonLoader,
    /// Floating action button.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    FAB,
    /// Bottom sheet modal panel.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    BottomSheet,
    /// Bottom navigation bar (mobile tab bar).
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    BottomNavigationBar,
    /// Navigation drawer sidebar.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    NavigationDrawer,
    /// Top app bar.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    AppBar,
    /// Mobile-style date picker.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    MobileDatePicker,
    /// Divider/Separator line widget.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    Divider,
    /// Stepper widget for numeric increment/decrement with +/- buttons.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    Stepper,
    /// Star rating control.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    Rating,
    /// Avatar widget — circular/square user image placeholder with initials fallback.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    Avatar,
    /// EmptyState widget — placeholder shown when a view has no content.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    EmptyState,
    /// Carousel/SwipeView widget — horizontal swipeable page carousel with dot indicators.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    Carousel,
    /// ColorHistory widget — a color history picker with a swatch grid.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    ColorHistory,
    /// ColorWell widget — compact color swatch that shows the current color and emits a signal when clicked.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    ColorWell,
    /// TagInput widget — text input that creates tags/chips on Enter or comma, with removable tags.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    TagInput,
    /// IME preedit text overlay widget for composition text input.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    ImePreedit,
    /// InplaceEditor — an in-place text editing control for table/cell editing.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    InplaceEditor,
    /// QRCode widget — displays a deterministic QR code pattern from a data string.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    QRCode,
    /// MasonryLayout widget — a Pinterest-style waterfall grid layout.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    MasonryLayout,
    /// CupertinoSwitch — iOS-style switch (alias for Switch with iOS coloring).
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    CupertinoSwitch,
    /// MaterialSnackbar — Material Design snackbar notification.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    MaterialSnackbar,
    /// AdaptiveScaffold — cross-platform adaptive scaffold with AppBar + content + bottom nav.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    AdaptiveScaffold,
    /// WizardDialog — step-by-step wizard control with back/next/finish navigation.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    WizardDialog,
    /// SafeArea — mobile safe area widget that insets content to avoid notches, status bars, and home indicators.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    SafeArea,
    /// CupertinoAlertDialog — iOS-style alert dialog with title, message, and buttons.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    CupertinoAlertDialog,
    /// CupertinoSlider — iOS-style slider with rounded track and circular knob.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    CupertinoSlider,
    /// MaterialNavigationRail — Material Design side navigation rail for tablets.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    MaterialNavigationRail,
    /// Tooltip — a popup label that appears on hover for context info.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    Tooltip,
    /// SegmentedButton — a horizontal group of selectable segments (Material 3 style).
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    SegmentedButton,
    /// NavigationStack — a push/pop page navigation container.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    NavigationStack,
    /// ProgressCircle — a circular progress indicator.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    ProgressCircle,
    /// Icon — a widget for rendering simple geometric icon representations.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    Icon,
    /// DropdownMenu — a cascading/linked dropdown selector with item list.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    DropdownMenu,
    /// MaskedEdit — a formatted text input with mask-based input constraints.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    MaskedEdit,
    /// MenuButton — a button that opens a dropdown menu when clicked.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    MenuButton,
    /// Popover — a floating bubble card with an anchor arrow.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    Popover,
    /// AutoCompleteEdit — a text input with auto-completion dropdown.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    AutoCompleteEdit,
    /// MultiSelectComboBox — a combo box that allows multiple selections.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    MultiSelectComboBox,
    /// RangeSlider — a dual-handle range slider for min-max selection.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    RangeSlider,
    /// FloatingLabel — a text input with a floating label (Material Design style).
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    FloatingLabel,
    /// FontPreview — a font preview panel for font selection dialogs.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    FontPreview,
    /// CupertinoNavigationBar — iOS-style large title navigation bar.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    CupertinoNavigationBar,
    /// CupertinoSegmentedControl — iOS-style pill-shaped segmented control.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    CupertinoSegmentedControl,
    /// SwipeToDismiss — swipe-to-dismiss/delete gesture container.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    SwipeToDismiss,
    /// PagerPageView — horizontal page view with dot indicators.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    PagerPageView,
    /// TabView — iOS-style segmented tab page view.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    TabView,
    /// SearchBar — iOS-style search bar with cancel button.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    SearchBar,
    /// ShortcutEditor — a keyboard shortcut editor widget.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    ShortcutEditor,
    /// RefreshControl — pull-to-refresh control for scrollable views.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    RefreshControl,
    /// ModalBottomSheet — Material-style modal bottom sheet with drag-to-dismiss.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    ModalBottomSheet,
    /// LineChart — a 2D line chart for visualizing data series.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    LineChart,
    /// Sparkline — a compact inline sparkline chart without axes.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    Sparkline,
    /// BarChart — a vertical bar chart for categorical data visualization.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    BarChart,
    /// FindReplaceDialog — a find/replace dialog with text input, toggles, and action buttons.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    FindReplaceDialog,
    /// PropertiesPanel — a categorized property editor panel with grid layout.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    PropertiesPanel,
    /// PieChart — a circular statistical chart with colored sectors.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    PieChart,
    /// CupertinoDatePicker — iOS UIPickerView-style scrolling wheel date picker.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    CupertinoDatePicker,
    /// EditableComboBox — a combo box that allows typing custom values.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    EditableComboBox,
    /// DateRangePicker — a calendar-based date range selection widget.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    DateRangePicker,
    /// AnimatedImage — plays animated images (GIF/APNG/WebP frame sequences).
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    AnimatedImage,
    /// HeroAnimation — shared element transition with interpolated position/size/opacity.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    HeroAnimation,
    /// BezierCurveEditor — interactive cubic bezier curve editor for custom easing curves.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    BezierCurveEditor,
    /// LottieWidget — Lottie JSON animation player.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    LottieWidget,
    /// RiveWidget — Rive animation runtime widget.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    RiveWidget,
    /// VideoPlayer — video player widget with playback controls.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    VideoPlayer,
    /// ImageGallery — image gallery/browser with thumbnails.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    ImageGallery,
    /// AudioVisualizer — real-time audio waveform/spectrum visualization.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    AudioVisualizer,
    /// CameraPreview — camera viewfinder preview area with controls.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    CameraPreview,
    /// BarcodeScanner — barcode/QR code scanner viewfinder with detection.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    BarcodeScanner,
    /// GridTable — feature-rich virtualized table with grid lines, headers, sorting, and selection.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    GridTable,
}
