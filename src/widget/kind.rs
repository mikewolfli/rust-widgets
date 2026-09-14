// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Widget kind enum — discrete categories supported by the widget model layer.

#[cfg(all(feature = "serde", widgets_unstripped))]
use serde::{Deserialize, Serialize};

/// Discrete widget categories supported by the widget model layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(all(feature = "serde", widgets_unstripped), derive(Serialize, Deserialize))]
pub enum WidgetKind {
    /// Top-level window.
    Window,
    #[cfg(widgets_unstripped)]
    Dialog,
    #[cfg(widgets_unstripped)]
    MessageBox,
    #[cfg(widgets_unstripped)]
    FileDialog,
    #[cfg(widgets_unstripped)]
    ColorDialog,
    #[cfg(widgets_unstripped)]
    FontDialog,
    #[cfg(widgets_unstripped)]
    InputDialog,
    #[cfg(widgets_unstripped)]
    ProgressDialog,
    #[cfg(widgets_unstripped)]
    PopupWindow,
    Button,
    CheckBox,
    RadioButton,
    Label,
    LineEdit,
    #[cfg(widgets_unstripped)]
    TextEdit,
    #[cfg(widgets_unstripped)]
    RichEdit,
    ComboBox,
    SpinBox,
    ListBox,
    #[cfg(widgets_unstripped)]
    ListView,
    #[cfg(widgets_unstripped)]
    TreeView,
    ProgressBar,
    Slider,
    ScrollBar,
    ScrollArea,
    Panel,
    Frame,
    #[cfg(widgets_unstripped)]
    DockPanel,
    GroupBox,
    #[cfg(widgets_unstripped)]
    TabWidget,
    #[cfg(widgets_unstripped)]
    Splitter,
    #[cfg(widgets_unstripped)]
    MdiArea,
    #[cfg(widgets_unstripped)]
    MenuBar,
    #[cfg(widgets_unstripped)]
    Menu,
    /// Individual item inside a menu.
    #[cfg(widgets_unstripped)]
    MenuItem,
    #[cfg(widgets_unstripped)]
    ContextMenu,
    #[cfg(widgets_unstripped)]
    ToolBar,
    #[cfg(widgets_unstripped)]
    StatusBar,
    #[cfg(widgets_unstripped)]
    Canvas,
    #[cfg(widgets_unstripped)]
    Table,
    #[cfg(widgets_unstripped)]
    Grid,
    /// Chart surface widget.
    #[cfg(widgets_unstripped)]
    Chart,
    #[cfg(widgets_unstripped)]
    ToggleButton,
    #[cfg(widgets_unstripped)]
    CheckListBox,
    #[cfg(widgets_unstripped)]
    DoubleSpinBox,
    #[cfg(widgets_unstripped)]
    Dial,
    #[cfg(widgets_unstripped)]
    Wizard,
    #[cfg(widgets_unstripped)]
    DatePicker,
    #[cfg(widgets_unstripped)]
    TimePicker,
    #[cfg(widgets_unstripped)]
    DateTimePicker,
    #[cfg(widgets_unstripped)]
    DirectoryDialog,
    #[cfg(widgets_unstripped)]
    DataView,
    #[cfg(widgets_unstripped)]
    PropertyGrid,
    #[cfg(widgets_unstripped)]
    Toolbox,
    #[cfg(widgets_unstripped)]
    StackedWidget,
    #[cfg(widgets_unstripped)]
    CollapsiblePane,
    #[cfg(widgets_unstripped)]
    DockWidget,
    #[cfg(widgets_unstripped)]
    ActivityIndicator,
    #[cfg(widgets_unstripped)]
    Calendar,
    #[cfg(widgets_unstripped)]
    ColumnView,
    #[cfg(widgets_unstripped)]
    UndoView,
    #[cfg(widgets_unstripped)]
    CommandLink,
    #[cfg(widgets_unstripped)]
    LCDNumber,
    #[cfg(widgets_unstripped)]
    FontComboBox,
    /// Web engine view widget for displaying web content.
    #[cfg(widgets_unstripped)]
    WebEngineView,
    /// Web engine page widget for managing web content.
    #[cfg(widgets_unstripped)]
    WebEnginePage,
    /// Web engine settings widget for configuring web engine behavior.
    #[cfg(widgets_unstripped)]
    WebEngineSettings,
    /// Web engine download item widget for managing downloads.
    #[cfg(widgets_unstripped)]
    WebEngineDownloadItem,
    /// Web engine cookie store widget for managing cookies.
    #[cfg(widgets_unstripped)]
    WebEngineCookieStore,
    /// Web engine web channel widget for JavaScript communication.
    #[cfg(widgets_unstripped)]
    WebEngineWebChannel,
    /// Web engine find text result widget for text search results.
    #[cfg(widgets_unstripped)]
    WebEngineFindTextResult,
    /// Web engine notification widget for web notifications.
    #[cfg(widgets_unstripped)]
    WebEngineNotification,
    /// Web engine script dialog widget for JavaScript dialogs.
    #[cfg(widgets_unstripped)]
    WebEngineScriptDialog,
    /// Web engine context menu request widget for context menu handling.
    #[cfg(widgets_unstripped)]
    WebEngineContextMenuRequest,
    /// Action widget for menu and toolbar actions.
    #[cfg(widgets_unstripped)]
    Action,
    /// Tool button widget.
    #[cfg(widgets_unstripped)]
    ToolButton,
    /// Freeform shape widget — a path-based non-rectangular clickable shape.
    #[cfg(widgets_unstripped)]
    FreeformShape,
    /// Standalone tab bar widget (decoupled from TabWidget).
    #[cfg(widgets_unstripped)]
    TabBar,
    /// Pie menu / radial menu widget.
    #[cfg(widgets_unstripped)]
    PieMenu,
    /// RibbonBar (Office-style ribbon) widget.
    #[cfg(widgets_unstripped)]
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
    #[cfg(widgets_unstripped)]
    SearchBox,
    /// Chip/Tag widget for labels and tokens.
    #[cfg(widgets_unstripped)]
    Chip,
    /// Badge widget for notification counts and status indicators.
    #[cfg(widgets_unstripped)]
    Badge,
    /// Skeleton loader placeholder widget.
    #[cfg(widgets_unstripped)]
    SkeletonLoader,
    /// Floating action button.
    #[cfg(widgets_unstripped)]
    FAB,
    /// Bottom sheet modal panel.
    #[cfg(widgets_unstripped)]
    BottomSheet,
    /// Bottom navigation bar (mobile tab bar).
    #[cfg(widgets_unstripped)]
    BottomNavigationBar,
    /// Navigation drawer sidebar.
    #[cfg(widgets_unstripped)]
    NavigationDrawer,
    /// Top app bar.
    #[cfg(widgets_unstripped)]
    AppBar,
    /// Mobile-style date picker.
    #[cfg(widgets_unstripped)]
    MobileDatePicker,
    /// Divider/Separator line widget.
    #[cfg(widgets_unstripped)]
    Divider,
    /// Stepper widget for numeric increment/decrement with +/- buttons.
    #[cfg(widgets_unstripped)]
    Stepper,
    /// Star rating control.
    #[cfg(widgets_unstripped)]
    Rating,
    /// Avatar widget — circular/square user image placeholder with initials fallback.
    #[cfg(widgets_unstripped)]
    Avatar,
    /// EmptyState widget — placeholder shown when a view has no content.
    #[cfg(widgets_unstripped)]
    EmptyState,
    /// Carousel/SwipeView widget — horizontal swipeable page carousel with dot indicators.
    #[cfg(widgets_unstripped)]
    Carousel,
    /// ColorHistory widget — a color history picker with a swatch grid.
    #[cfg(widgets_unstripped)]
    ColorHistory,
    /// ColorWell widget — compact color swatch that shows the current color and emits a signal when clicked.
    #[cfg(widgets_unstripped)]
    ColorWell,
    /// TagInput widget — text input that creates tags/chips on Enter or comma, with removable tags.
    #[cfg(widgets_unstripped)]
    TagInput,
    /// IME preedit text overlay widget for composition text input.
    #[cfg(widgets_unstripped)]
    ImePreedit,
    /// InplaceEditor — an in-place text editing control for table/cell editing.
    #[cfg(widgets_unstripped)]
    InplaceEditor,
    /// QRCode widget — displays a deterministic QR code pattern from a data string.
    #[cfg(widgets_unstripped)]
    QRCode,
    /// MasonryLayout widget — a Pinterest-style waterfall grid layout.
    #[cfg(widgets_unstripped)]
    MasonryLayout,
    /// CupertinoSwitch — iOS-style switch (alias for Switch with iOS coloring).
    #[cfg(widgets_unstripped)]
    CupertinoSwitch,
    /// MaterialSnackbar — Material Design snackbar notification.
    #[cfg(widgets_unstripped)]
    MaterialSnackbar,
    /// AdaptiveScaffold — cross-platform adaptive scaffold with AppBar + content + bottom nav.
    #[cfg(widgets_unstripped)]
    AdaptiveScaffold,
    /// WizardDialog — step-by-step wizard control with back/next/finish navigation.
    #[cfg(widgets_unstripped)]
    WizardDialog,
    /// SafeArea — mobile safe area widget that insets content to avoid notches, status bars, and home indicators.
    #[cfg(widgets_unstripped)]
    SafeArea,
    /// CupertinoAlertDialog — iOS-style alert dialog with title, message, and buttons.
    #[cfg(widgets_unstripped)]
    CupertinoAlertDialog,
    /// CupertinoSlider — iOS-style slider with rounded track and circular knob.
    #[cfg(widgets_unstripped)]
    CupertinoSlider,
    /// MaterialNavigationRail — Material Design side navigation rail for tablets.
    #[cfg(widgets_unstripped)]
    MaterialNavigationRail,
    /// Tooltip — a popup label that appears on hover for context info.
    #[cfg(widgets_unstripped)]
    Tooltip,
    /// SegmentedButton — a horizontal group of selectable segments (Material 3 style).
    #[cfg(widgets_unstripped)]
    SegmentedButton,
    /// NavigationStack — a push/pop page navigation container.
    #[cfg(widgets_unstripped)]
    NavigationStack,
    /// ProgressCircle — a circular progress indicator.
    #[cfg(widgets_unstripped)]
    ProgressCircle,
    /// Icon — a widget for rendering simple geometric icon representations.
    #[cfg(widgets_unstripped)]
    Icon,
    /// DropdownMenu — a cascading/linked dropdown selector with item list.
    #[cfg(widgets_unstripped)]
    DropdownMenu,
    /// MaskedEdit — a formatted text input with mask-based input constraints.
    #[cfg(widgets_unstripped)]
    MaskedEdit,
    /// MenuButton — a button that opens a dropdown menu when clicked.
    #[cfg(widgets_unstripped)]
    MenuButton,
    /// Popover — a floating bubble card with an anchor arrow.
    #[cfg(widgets_unstripped)]
    Popover,
    /// AutoCompleteEdit — a text input with auto-completion dropdown.
    #[cfg(widgets_unstripped)]
    AutoCompleteEdit,
    /// MultiSelectComboBox — a combo box that allows multiple selections.
    #[cfg(widgets_unstripped)]
    MultiSelectComboBox,
    /// RangeSlider — a dual-handle range slider for min-max selection.
    #[cfg(widgets_unstripped)]
    RangeSlider,
    /// FloatingLabel — a text input with a floating label (Material Design style).
    #[cfg(widgets_unstripped)]
    FloatingLabel,
    /// FontPreview — a font preview panel for font selection dialogs.
    #[cfg(widgets_unstripped)]
    FontPreview,
    /// CupertinoNavigationBar — iOS-style large title navigation bar.
    #[cfg(widgets_unstripped)]
    CupertinoNavigationBar,
    /// CupertinoSegmentedControl — iOS-style pill-shaped segmented control.
    #[cfg(widgets_unstripped)]
    CupertinoSegmentedControl,
    /// SwipeToDismiss — swipe-to-dismiss/delete gesture container.
    #[cfg(widgets_unstripped)]
    SwipeToDismiss,
    /// PagerPageView — horizontal page view with dot indicators.
    #[cfg(widgets_unstripped)]
    PagerPageView,
    /// TabView — iOS-style segmented tab page view.
    #[cfg(widgets_unstripped)]
    TabView,
    /// SearchBar — iOS-style search bar with cancel button.
    #[cfg(widgets_unstripped)]
    SearchBar,
    /// ShortcutEditor — a keyboard shortcut editor widget.
    #[cfg(widgets_unstripped)]
    ShortcutEditor,
    /// RefreshControl — pull-to-refresh control for scrollable views.
    #[cfg(widgets_unstripped)]
    RefreshControl,
    /// ModalBottomSheet — Material-style modal bottom sheet with drag-to-dismiss.
    #[cfg(widgets_unstripped)]
    ModalBottomSheet,
    /// LineChart — a 2D line chart for visualizing data series.
    #[cfg(widgets_unstripped)]
    LineChart,
    /// Sparkline — a compact inline sparkline chart without axes.
    #[cfg(widgets_unstripped)]
    Sparkline,
    /// BarChart — a vertical bar chart for categorical data visualization.
    #[cfg(widgets_unstripped)]
    BarChart,
    /// FindReplaceDialog — a find/replace dialog with text input, toggles, and action buttons.
    #[cfg(widgets_unstripped)]
    FindReplaceDialog,
    /// PropertiesPanel — a categorized property editor panel with grid layout.
    #[cfg(widgets_unstripped)]
    PropertiesPanel,
    /// PieChart — a circular statistical chart with colored sectors.
    #[cfg(widgets_unstripped)]
    PieChart,
    /// CupertinoDatePicker — iOS UIPickerView-style scrolling wheel date picker.
    #[cfg(widgets_unstripped)]
    CupertinoDatePicker,
    /// EditableComboBox — a combo box that allows typing custom values.
    #[cfg(widgets_unstripped)]
    EditableComboBox,
    /// DateRangePicker — a calendar-based date range selection widget.
    #[cfg(widgets_unstripped)]
    DateRangePicker,
    /// AnimatedImage — plays animated images (GIF/APNG/WebP frame sequences).
    #[cfg(widgets_unstripped)]
    AnimatedImage,
    /// HeroAnimation — shared element transition with interpolated position/size/opacity.
    #[cfg(widgets_unstripped)]
    HeroAnimation,
    /// BezierCurveEditor — interactive cubic bezier curve editor for custom easing curves.
    #[cfg(widgets_unstripped)]
    BezierCurveEditor,
    /// LottieWidget — Lottie JSON animation player.
    #[cfg(widgets_unstripped)]
    LottieWidget,
    /// RiveWidget — Rive animation runtime widget.
    #[cfg(widgets_unstripped)]
    RiveWidget,
    /// VideoPlayer — video player widget with playback controls.
    #[cfg(widgets_unstripped)]
    VideoPlayer,
    /// ImageGallery — image gallery/browser with thumbnails.
    #[cfg(widgets_unstripped)]
    ImageGallery,
    /// AudioVisualizer — real-time audio waveform/spectrum visualization.
    #[cfg(widgets_unstripped)]
    AudioVisualizer,
    /// CameraPreview — camera viewfinder preview area with controls.
    #[cfg(widgets_unstripped)]
    CameraPreview,
    /// BarcodeScanner — barcode/QR code scanner viewfinder with detection.
    #[cfg(widgets_unstripped)]
    BarcodeScanner,
    /// GridTable — feature-rich virtualized table with grid lines, headers, sorting, and selection.
    #[cfg(widgets_unstripped)]
    GridTable,
}
