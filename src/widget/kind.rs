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
    /// Secondary window, usually modal, that hosts its own child widgets.
    #[cfg(widgets_unstripped)]
    Dialog,
    /// Modal alert that shows a short message and a fixed set of response buttons.
    #[cfg(widgets_unstripped)]
    MessageBox,
    /// Modal dialog for browsing the filesystem and picking a file.
    #[cfg(widgets_unstripped)]
    FileDialog,
    /// Modal dialog with a color wheel, swatches and RGBA value entry for picking a color.
    ///
    /// kind-role: base — the control registered for this kind is `color_picker`;
    /// the kind name itself has no factory entry (see `color_dialog` alias).
    #[cfg(widgets_unstripped)]
    ColorDialog,
    /// Modal dialog for browsing installed typefaces and picking a font.
    #[cfg(widgets_unstripped)]
    FontDialog,
    /// Modal prompt that collects a single line of text from the user.
    #[cfg(widgets_unstripped)]
    InputDialog,
    /// Modal dialog with a progress bar and a cancel button for long-running work.
    #[cfg(widgets_unstripped)]
    ProgressDialog,
    /// Borderless window that floats above other windows.
    #[cfg(widgets_unstripped)]
    PopupWindow,
    /// Push button that fires its clicked signal when activated.
    Button,
    /// Label with an independent on/off box, used for multi-choice settings.
    CheckBox,
    /// Round choice control, mutually exclusive with the other radio buttons in its group.
    RadioButton,
    /// Non-interactive text display.
    Label,
    /// Single-line text input field.
    LineEdit,
    /// Framed multi-line plain-text editor.
    #[cfg(widgets_unstripped)]
    TextEdit,
    /// Multi-line editor with character and paragraph formatting attributes.
    #[cfg(widgets_unstripped)]
    RichEdit,
    /// Text field with a drop-down list of selectable items.
    ComboBox,
    /// Numeric entry with increment/decrement arrows and a range constraint.
    SpinBox,
    /// Scrollable list from which one or more items can be selected.
    ListBox,
    /// Item view backed by a model with a customizable item delegate.
    #[cfg(widgets_unstripped)]
    ListView,
    /// Item view that presents a model as expandable hierarchical rows.
    #[cfg(widgets_unstripped)]
    TreeView,
    /// Visual fill indicator for the progress of a bounded operation.
    ProgressBar,
    /// Handle dragged along a groove to pick a value from a continuous range.
    Slider,
    /// Thumb on a rail that scrolls a related viewport.
    ScrollBar,
    /// Scrollable container that clips and offsets a larger child widget.
    ScrollArea,
    /// Plain rectangular container used for grouping child widgets.
    Panel,
    /// Container that draws a border or 3D frame around its child.
    ///
    /// kind-role: base — a `Frame` is the drawing shell specialised controls
    /// embed; it publishes no capability of its own.
    Frame,
    /// Dockable container that arranges child panels into docked regions.
    #[cfg(widgets_unstripped)]
    DockPanel,
    /// Frame with a title label drawn over its top border, grouping related controls.
    GroupBox,
    /// Container of tabbed pages in which only the selected page is visible.
    #[cfg(widgets_unstripped)]
    TabWidget,
    /// Draggable divider that splits a container into resizable panes.
    #[cfg(widgets_unstripped)]
    Splitter,
    /// Workspace that hosts multiple independent child windows.
    #[cfg(widgets_unstripped)]
    MdiArea,
    /// Horizontal strip of top-level menu titles.
    #[cfg(widgets_unstripped)]
    MenuBar,
    /// Drop-down list of commands or submenus.
    #[cfg(widgets_unstripped)]
    Menu,
    /// Individual item inside a menu.
    ///
    /// kind-role: child — `MenuItem` rows are created by their owning `Menu`,
    /// never constructed directly from a factory name.
    #[cfg(widgets_unstripped)]
    MenuItem,
    /// Menu opened at the pointer position in response to a secondary click.
    #[cfg(widgets_unstripped)]
    ContextMenu,
    /// Strip of icon buttons and other controls for quick actions.
    #[cfg(widgets_unstripped)]
    ToolBar,
    /// Strip at the bottom of a window that shows transient status messages.
    #[cfg(widgets_unstripped)]
    StatusBar,
    /// Free-form surface that draws user-supplied shapes.
    #[cfg(widgets_unstripped)]
    Canvas,
    /// Cell-based view that arranges its model into rows and columns.
    #[cfg(widgets_unstripped)]
    Table,
    /// Rectangular layout container that arranges children into cells.
    #[cfg(widgets_unstripped)]
    Grid,
    /// Chart surface widget.
    #[cfg(widgets_unstripped)]
    Chart,
    /// Radar (spider) chart — series plotted over shared dimension axes.
    ///
    /// A separate kind rather than a `Chart` variant: `ChartWidget` plots a value
    /// against its position in an ordered series, while this plots a value against
    /// *which dimension* it belongs to, and draws each series as a closed polygon.
    /// The two data models are not the same shape (see the module docs of
    /// `special_widgets::radar_chart`), which is the condition rule #80 sets.
    #[cfg(widgets_unstripped)]
    RadarChart,
    /// Board of columns holding draggable cards.
    ///
    /// A separate kind rather than a `ListView` variant: the model is two-level and
    /// a drag changes both the column and the index (see the module docs of
    /// `special_widgets::kanban_board`). It is also the first consumer of
    /// `event::dnd`.
    #[cfg(widgets_unstripped)]
    KanbanBoard,
    /// Button that latches between checked and unchecked states when clicked.
    #[cfg(widgets_unstripped)]
    ToggleButton,
    /// List box whose rows carry independent check boxes.
    #[cfg(widgets_unstripped)]
    CheckListBox,
    /// Spin box that edits a floating-point value.
    #[cfg(widgets_unstripped)]
    DoubleSpinBox,
    /// Rotary knob that maps its angle to a bounded value.
    #[cfg(widgets_unstripped)]
    Dial,
    /// Multi-page dialog that walks the user through ordered steps.
    #[cfg(widgets_unstripped)]
    Wizard,
    /// Field that opens a calendar popup for selecting a date.
    #[cfg(widgets_unstripped)]
    DatePicker,
    /// Field that opens a popup for selecting a time of day.
    #[cfg(widgets_unstripped)]
    TimePicker,
    /// Field for selecting a combined date and time value.
    #[cfg(widgets_unstripped)]
    DateTimePicker,
    /// Modal dialog for choosing a directory from the filesystem.
    #[cfg(widgets_unstripped)]
    DirectoryDialog,
    /// View that renders model items using one of several switchable display modes.
    #[cfg(widgets_unstripped)]
    DataView,
    /// Shell for choosing a glyph, with the glyph table supplied by the caller.
    ///
    /// Ships no glyph data — this crate bundles no binary assets, and the Unicode
    /// emoji table would dwarf the library. The picker supplies the grid, search,
    /// categories, recents and keyboard navigation; `set_glyphs` supplies the
    /// symbols (see `display_widgets::emoji_picker`).
    #[cfg(widgets_unstripped)]
    EmojiPicker,
    /// Text field that completes `@`-mentions from a candidate list.
    ///
    /// A separate kind from `AutoCompleteEdit`: that control matches its suggestions
    /// against the whole field and replaces all of it, while this completes the
    /// *token before the caret* and supports several mentions in one field (see
    /// `input_widgets::mention`).
    #[cfg(widgets_unstripped)]
    Mention,
    /// Builder that edits a recursive filter as a list of condition rows.
    #[cfg(widgets_unstripped)]
    QueryBuilder,
    /// Two-column editor that lists named properties with an editable value cell.
    #[cfg(widgets_unstripped)]
    PropertyGrid,
    /// Side panel of expandable grouped commands, typically shown alongside a design surface.
    #[cfg(widgets_unstripped)]
    Toolbox,
    /// Container that shows one child page at a time, selected programmatically.
    #[cfg(widgets_unstripped)]
    StackedWidget,
    /// Collapsible section with a clickable header that expands or hides its content.
    #[cfg(widgets_unstripped)]
    CollapsiblePane,
    /// Individual panel that can be detached and re-docked inside a DockPanel.
    #[cfg(widgets_unstripped)]
    DockWidget,
    /// Animated indicator shown while a background operation is running.
    #[cfg(widgets_unstripped)]
    ActivityIndicator,
    /// Month grid for browsing and selecting calendar dates.
    #[cfg(widgets_unstripped)]
    Calendar,
    /// Hierarchical view that lists one column of children per selected branch.
    #[cfg(widgets_unstripped)]
    ColumnView,
    /// Read-only list of the edit commands available in the current undo stack.
    #[cfg(widgets_unstripped)]
    UndoView,
    /// Button styled as a key command link, optionally with an explanatory subtitle.
    #[cfg(widgets_unstripped)]
    CommandLink,
    /// Seven-segment display that renders a numeric string as digit segments.
    #[cfg(widgets_unstripped)]
    LCDNumber,
    /// Combo box populated with the installed typefaces and rendered in each font.
    #[cfg(widgets_unstripped)]
    FontComboBox,
    /// Web engine view widget for displaying web content.
    ///
    /// # Why this is the only `WebEngine*` variant
    ///
    /// The other `WebEngine*` names (`WebEnginePage`, `WebEngineSettings`, …) were
    /// once variants here, marked `kind-role: base`. They were not kinds at all:
    /// each named a newtype wrapper around this view whose `Widget::base()`
    /// forwards to the wrapped view, so `kind()` answered `WebEngineView` for
    /// every one of them. Nothing could ever produce those variants, which made
    /// them orphans (rule #22) — and because `factory_name_for_kind` resolves a
    /// kind through `capability_by_kind`, `create_web_engine_page(..)` silently
    /// produced id `0` rather than a control.
    ///
    /// The wrappers themselves are real and stay: they are the render pipeline's
    /// named symbols for one view, reachable as Rust types
    /// ([`crate::widget::WebEnginePage`] and siblings) and through the one
    /// registered name `web_view`.
    #[cfg(widgets_unstripped)]
    WebEngineView,
    /// Action widget for menu and toolbar actions.
    #[cfg(widgets_unstripped)]
    Action,
    /// Compact button with an icon and optional caption, used on toolbars.
    #[cfg(widgets_unstripped)]
    ToolButton,
    /// Multi-level chooser that walks a path down a tree of options.
    #[cfg(widgets_unstripped)]
    Cascader,
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
    /// Text field with a leading search icon and a trailing clear button.
    #[cfg(widgets_unstripped)]
    SearchBox,
    /// Compact rounded label that carries a short tag or token, optionally removable.
    #[cfg(widgets_unstripped)]
    Chip,
    /// Small overlay marker carrying a count or status dot, anchored to another widget.
    #[cfg(widgets_unstripped)]
    Badge,
    /// Grey placeholder block shown while the real content is still loading.
    #[cfg(widgets_unstripped)]
    SkeletonLoader,
    /// Round, elevated button that floats over content for the primary page action.
    #[cfg(widgets_unstripped)]
    FAB,
    /// Modal panel that slides up from the bottom edge and can be dragged back down.
    #[cfg(widgets_unstripped)]
    BottomSheet,
    /// Row of tabs fixed along the bottom edge, as used by mobile apps.
    #[cfg(widgets_unstripped)]
    BottomNavigationBar,
    /// Sidebar that slides in from the edge to hold primary navigation entries.
    #[cfg(widgets_unstripped)]
    NavigationDrawer,
    /// Bar pinned to the top of a window that holds the title and primary actions.
    #[cfg(widgets_unstripped)]
    AppBar,
    /// Date picker laid out for touch input rather than a desktop calendar grid.
    #[cfg(widgets_unstripped)]
    MobileDatePicker,
    /// Thin rule that separates adjacent sections of content.
    #[cfg(widgets_unstripped)]
    Divider,
    /// Numeric field flanked by plus and minus buttons that nudge the value by fixed steps.
    #[cfg(widgets_unstripped)]
    Stepper,
    /// Row of selectable stars or icons that records a discrete rating value.
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
    ///
    /// kind-role: base — the kind is reported by the `switch` control when it is
    /// configured with iOS styling; there is no separate constructor.
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
    /// CandlestickChart — a K-line chart of OHLC bars with indicator overlays.
    #[cfg(widgets_unstripped)]
    CandlestickChart,
    /// VolumeChart — a volume histogram, aligned to a CandlestickChart's bars.
    #[cfg(widgets_unstripped)]
    VolumeChart,
    /// DepthChart — a cumulative bid/ask depth curve.
    #[cfg(widgets_unstripped)]
    DepthChart,
    /// OrderBook — a live bid/ask ladder with sizes at each level.
    #[cfg(widgets_unstripped)]
    OrderBook,
    /// QuoteBoard — a table of instruments with price, change and volume.
    #[cfg(widgets_unstripped)]
    QuoteBoard,
    /// IndicatorChart — a technical-indicator pane (MACD, RSI, KDJ).
    #[cfg(widgets_unstripped)]
    IndicatorChart,
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
    /// NumberPicker — vertically scrolling digit wheel for numeric selection.
    ///
    /// Distinct from `SpinBox`, which is a typed field with step buttons: a picker
    /// keeps a scroll offset and shows its neighbours, so its interaction is a
    /// drag or a flick rather than a click per increment.
    #[cfg(widgets_unstripped)]
    NumberPicker,
    /// OtpInput — segmented single-character code entry that advances per keystroke.
    #[cfg(widgets_unstripped)]
    OtpInput,
    /// Banner — persistent full-width notice that stays until the user dismisses it.
    #[cfg(widgets_unstripped)]
    Banner,
    /// Pagination — numbered page navigation bar for paged content.
    ///
    /// Holds no content itself; `Carousel` is the control that contains the pages and
    /// changes which one is visible. This is the index for content another control
    /// owns.
    #[cfg(widgets_unstripped)]
    Pagination,
    /// ColorPicker — an inline HSVA colour picker.
    ///
    /// Distinct from `ColorDialog`, which is the modal window that *hosts* a
    /// picker. Conflating the two made the accessibility role, the factory lookup
    /// and the CSS selector all answer "dialog" for a control that is embedded in
    /// a form.
    #[cfg(widgets_unstripped)]
    ColorPicker,
    /// Toast — a single transient notification message.
    ///
    /// `ToastStack` is the container that queues and lays out toasts; this is one
    /// message, which is what a `toast("saved")` call actually creates.
    #[cfg(widgets_unstripped)]
    Toast,
    /// SplashScreen — the startup screen shown while an application initialises.
    ///
    /// Carries a logo, a title and an optional progress indicator, and is dismissed
    /// when the application is ready rather than by user action.
    #[cfg(widgets_unstripped)]
    SplashScreen,
}
