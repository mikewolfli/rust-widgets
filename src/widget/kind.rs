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
    ///
    /// kind-role: base — `Panel` is a `pub type` for `GroupBox` under a second
    /// name. In a full build `create_panel` mounts the `panel` factory name, which
    /// reports `WidgetKind::GroupBox`; `Panel` is kept as a distinct variant only
    /// as the always-available kind for a stripped profile, where the `panel` name
    /// is the one `create_panel` mounts.
    ///
    /// It publishes no capability row of its own, so `capability_by_kind(Panel)`
    /// misses and `factory_name_for_kind(Panel)` is answered by the `panel` alias
    /// row in `capability::alias_factory_name` — not by a `Panel`-kinded
    /// capability, because none exists. That alias row is load-bearing:
    /// `theme::apply_active_theme` skips a widget whose factory name is empty, so
    /// without it a `Panel`-kinded widget silently received no theme role.
    Panel,
    /// Container that draws a border or 3D frame around its child.
    ///
    /// kind-role: base — a `Frame` is the drawing shell specialised controls
    /// embed; it publishes no capability of its own.
    Frame,
    /// A single dockable panel that holds one child widget and can sit at a dock edge.
    ///
    /// Alias target: `DockWidget` (`pub type DockPanel = DockWidget`), so this variant names the
    /// same single-panel type under the `panel` spelling. It is **not** a multi-region container
    /// that arranges several panels — an earlier doc comment here described one, which no type in
    /// this crate implements.
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
    /// never constructed directly from a factory name. It therefore has no
    /// capability row either; `factory_name_for_kind(MenuItem)` resolves through
    /// the `menu_item` alias row in `capability::alias_factory_name`, which the
    /// theme layer needs in order to classify the row at all.
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
    /// Heat map — a matrix of values rendered as a grid of coloured cells.
    ///
    /// A separate kind rather than a `Chart` variant because both of its axes are
    /// **categorical**: there is no ordered value axis to interpolate along, the cell at
    /// `(row, column)` *is* the datum, and the colour of that cell is a function of the
    /// value through a colour scale. `ChartWidget`'s model is one value per index, and a
    /// heat map's is one value per coordinate pair — not the same shape (rule #80).
    #[cfg(widgets_unstripped)]
    Heatmap,
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
    /// Progress indicator for a background operation.
    ///
    /// Alias target: `ProgressBar` (`pub type ActivityIndicator = ProgressBar`), and that alias is
    /// **not** an animated spinner — it is a plain progress bar whose value the caller advances.
    /// The earlier "Animated indicator" wording promised an animation the type does not have; a
    /// caller wanting a spinner wants [`Self::Spinner`].
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
    /// CupertinoSwitch — iOS-style switch.
    ///
    /// It **does** have its own capability row and constructor
    /// (`cupertino_switch_capability` / `create_cupertino_switch`), and `switch_capability`
    /// deliberately does not alias it. An earlier comment here claimed the kind was reported by
    /// `switch` with iOS styling and that "there is no separate constructor" — both false.
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
    ///
    /// Distinct from [`Self::SegmentedControl`]: this one's segments carry an `id` and an icon and
    /// it supports multi-select, whereas `SegmentedControl`'s carry an id and a label and it is
    /// single-select with a sliding indicator. `SegmentedControl` used to report **this** kind,
    /// which made `capability_by_kind` ambiguous between two controls and gave the accessibility
    /// role of a segmented control the answer for a toggle button.
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
    /// TreeTable — hierarchical rows combined with tabular columns in one view.
    ///
    /// Distinct from `TreeView` (a pure tree) and `Table`/`GridTable` (flat grids):
    /// the rows form a tree with expand/collapse, and each row carries column cells.
    #[cfg(widgets_unstripped)]
    TreeTable,
    /// Breadcrumb — a horizontal navigation trail of segments.
    ///
    /// Distinct from `Panel`, the plain container this control previously reported
    /// as its kind: the two share no behaviour, and conflating them made the
    /// accessibility role and the factory lookup answer "panel" for a navigation
    /// trail. Each trail segment is clickable and emits its id on activation.
    #[cfg(widgets_unstripped)]
    Breadcrumb,
    /// SignaturePad — a touch-friendly freehand drawing surface for capturing
    /// signatures. Records a list of pen strokes with optional smoothing, supports
    /// undo, clear and export to a polyline description.
    #[cfg(widgets_unstripped)]
    SignaturePad,
    /// DropZone — a named drop target that accepts a [`crate::event::dnd::DragPayload`]
    /// filtered by MIME type, emits an accept/commit signal, and renders a hover
    /// highlight while a compatible drag is over it.
    #[cfg(widgets_unstripped)]
    DropZone,
    /// SegmentedControl — a single-select segmented bar whose indicator slides between segments.
    ///
    /// Distinct from [`Self::SegmentedButton`] (see there) and from
    /// [`Self::CupertinoSegmentedControl`], which is the iOS-styled rendition of this control.
    ///
    /// # Why the variant is declared last
    ///
    /// `WidgetKind`'s declaration order reaches further than it looks: it is the iteration order of
    /// several `match`-free scans and, through `kind_canonical_name`, the order names are produced
    /// in. Inserting a variant in the middle therefore reshuffled unrelated controls' rendering —
    /// measured: adding this one next to `SegmentedButton` moved `calendar`'s selected-day highlight
    /// by one column in its snapshot. Appending keeps every previously-declared variant at its old
    /// position, so adding a control cannot rearrange the ones already there.
    #[cfg(widgets_unstripped)]
    SegmentedControl,
}

impl WidgetKind {
    /// The `Debug` spelling of this kind, as a `'static` string.
    ///
    /// The stylesheet and theme layers address controls by this spelling (`Button`, not
    /// `button`), and the JSON loader used to obtain it with `format!("{:?}", kind)` — a heap
    /// allocation per node to name a value that is a compile-time constant.
    ///
    /// The match is exhaustive on purpose: a new variant fails to compile here, so the string
    /// can never silently go missing. `css_name_matches_the_debug_spelling` additionally pins
    /// every reachable kind against `format!("{:?}")`, because the compiler checks that each
    /// variant is *named*, not that it is named *correctly*.
    pub const fn css_name(self) -> &'static str {
        match self {
            Self::Window => "Window",
            #[cfg(widgets_unstripped)]
            Self::Dialog => "Dialog",
            #[cfg(widgets_unstripped)]
            Self::MessageBox => "MessageBox",
            #[cfg(widgets_unstripped)]
            Self::FileDialog => "FileDialog",
            #[cfg(widgets_unstripped)]
            Self::ColorDialog => "ColorDialog",
            #[cfg(widgets_unstripped)]
            Self::FontDialog => "FontDialog",
            #[cfg(widgets_unstripped)]
            Self::InputDialog => "InputDialog",
            #[cfg(widgets_unstripped)]
            Self::ProgressDialog => "ProgressDialog",
            #[cfg(widgets_unstripped)]
            Self::PopupWindow => "PopupWindow",
            Self::Button => "Button",
            Self::CheckBox => "CheckBox",
            Self::RadioButton => "RadioButton",
            Self::Label => "Label",
            Self::LineEdit => "LineEdit",
            #[cfg(widgets_unstripped)]
            Self::TextEdit => "TextEdit",
            #[cfg(widgets_unstripped)]
            Self::RichEdit => "RichEdit",
            Self::ComboBox => "ComboBox",
            Self::SpinBox => "SpinBox",
            Self::ListBox => "ListBox",
            #[cfg(widgets_unstripped)]
            Self::ListView => "ListView",
            #[cfg(widgets_unstripped)]
            Self::TreeView => "TreeView",
            Self::ProgressBar => "ProgressBar",
            Self::Slider => "Slider",
            Self::ScrollBar => "ScrollBar",
            Self::ScrollArea => "ScrollArea",
            Self::Panel => "Panel",
            Self::Frame => "Frame",
            #[cfg(widgets_unstripped)]
            Self::DockPanel => "DockPanel",
            Self::GroupBox => "GroupBox",
            #[cfg(widgets_unstripped)]
            Self::TabWidget => "TabWidget",
            #[cfg(widgets_unstripped)]
            Self::Splitter => "Splitter",
            #[cfg(widgets_unstripped)]
            Self::MdiArea => "MdiArea",
            #[cfg(widgets_unstripped)]
            Self::MenuBar => "MenuBar",
            #[cfg(widgets_unstripped)]
            Self::Menu => "Menu",
            #[cfg(widgets_unstripped)]
            Self::MenuItem => "MenuItem",
            #[cfg(widgets_unstripped)]
            Self::ContextMenu => "ContextMenu",
            #[cfg(widgets_unstripped)]
            Self::ToolBar => "ToolBar",
            #[cfg(widgets_unstripped)]
            Self::StatusBar => "StatusBar",
            #[cfg(widgets_unstripped)]
            Self::Canvas => "Canvas",
            #[cfg(widgets_unstripped)]
            Self::Table => "Table",
            #[cfg(widgets_unstripped)]
            Self::Grid => "Grid",
            #[cfg(widgets_unstripped)]
            Self::Chart => "Chart",
            #[cfg(widgets_unstripped)]
            Self::RadarChart => "RadarChart",
            #[cfg(widgets_unstripped)]
            Self::Heatmap => "Heatmap",
            #[cfg(widgets_unstripped)]
            Self::KanbanBoard => "KanbanBoard",
            #[cfg(widgets_unstripped)]
            Self::ToggleButton => "ToggleButton",
            #[cfg(widgets_unstripped)]
            Self::CheckListBox => "CheckListBox",
            #[cfg(widgets_unstripped)]
            Self::DoubleSpinBox => "DoubleSpinBox",
            #[cfg(widgets_unstripped)]
            Self::Dial => "Dial",
            #[cfg(widgets_unstripped)]
            Self::Wizard => "Wizard",
            #[cfg(widgets_unstripped)]
            Self::DatePicker => "DatePicker",
            #[cfg(widgets_unstripped)]
            Self::TimePicker => "TimePicker",
            #[cfg(widgets_unstripped)]
            Self::DateTimePicker => "DateTimePicker",
            #[cfg(widgets_unstripped)]
            Self::DirectoryDialog => "DirectoryDialog",
            #[cfg(widgets_unstripped)]
            Self::DataView => "DataView",
            #[cfg(widgets_unstripped)]
            Self::EmojiPicker => "EmojiPicker",
            #[cfg(widgets_unstripped)]
            Self::Mention => "Mention",
            #[cfg(widgets_unstripped)]
            Self::QueryBuilder => "QueryBuilder",
            #[cfg(widgets_unstripped)]
            Self::PropertyGrid => "PropertyGrid",
            #[cfg(widgets_unstripped)]
            Self::Toolbox => "Toolbox",
            #[cfg(widgets_unstripped)]
            Self::StackedWidget => "StackedWidget",
            #[cfg(widgets_unstripped)]
            Self::CollapsiblePane => "CollapsiblePane",
            #[cfg(widgets_unstripped)]
            Self::DockWidget => "DockWidget",
            #[cfg(widgets_unstripped)]
            Self::ActivityIndicator => "ActivityIndicator",
            #[cfg(widgets_unstripped)]
            Self::Calendar => "Calendar",
            #[cfg(widgets_unstripped)]
            Self::ColumnView => "ColumnView",
            #[cfg(widgets_unstripped)]
            Self::UndoView => "UndoView",
            #[cfg(widgets_unstripped)]
            Self::CommandLink => "CommandLink",
            #[cfg(widgets_unstripped)]
            Self::LCDNumber => "LCDNumber",
            #[cfg(widgets_unstripped)]
            Self::FontComboBox => "FontComboBox",
            #[cfg(widgets_unstripped)]
            Self::WebEngineView => "WebEngineView",
            #[cfg(widgets_unstripped)]
            Self::Action => "Action",
            #[cfg(widgets_unstripped)]
            Self::ToolButton => "ToolButton",
            #[cfg(widgets_unstripped)]
            Self::Cascader => "Cascader",
            #[cfg(widgets_unstripped)]
            Self::FreeformShape => "FreeformShape",
            #[cfg(widgets_unstripped)]
            Self::TabBar => "TabBar",
            #[cfg(widgets_unstripped)]
            Self::PieMenu => "PieMenu",
            #[cfg(widgets_unstripped)]
            Self::RibbonBar => "RibbonBar",
            Self::Line => "Line",
            Self::Meter => "Meter",
            Self::MiniChart => "MiniChart",
            Self::ImageView => "ImageView",
            Self::MiniCanvas => "MiniCanvas",
            Self::Arc => "Arc",
            Self::Spinner => "Spinner",
            Self::Roller => "Roller",
            Self::Dropdown => "Dropdown",
            Self::TextArea => "TextArea",
            Self::Keyboard => "Keyboard",
            Self::Switch => "Switch",
            #[cfg(widgets_unstripped)]
            Self::SearchBox => "SearchBox",
            #[cfg(widgets_unstripped)]
            Self::Chip => "Chip",
            #[cfg(widgets_unstripped)]
            Self::Badge => "Badge",
            #[cfg(widgets_unstripped)]
            Self::SkeletonLoader => "SkeletonLoader",
            #[cfg(widgets_unstripped)]
            Self::FAB => "FAB",
            #[cfg(widgets_unstripped)]
            Self::BottomSheet => "BottomSheet",
            #[cfg(widgets_unstripped)]
            Self::BottomNavigationBar => "BottomNavigationBar",
            #[cfg(widgets_unstripped)]
            Self::NavigationDrawer => "NavigationDrawer",
            #[cfg(widgets_unstripped)]
            Self::AppBar => "AppBar",
            #[cfg(widgets_unstripped)]
            Self::MobileDatePicker => "MobileDatePicker",
            #[cfg(widgets_unstripped)]
            Self::Divider => "Divider",
            #[cfg(widgets_unstripped)]
            Self::Stepper => "Stepper",
            #[cfg(widgets_unstripped)]
            Self::Rating => "Rating",
            #[cfg(widgets_unstripped)]
            Self::Avatar => "Avatar",
            #[cfg(widgets_unstripped)]
            Self::EmptyState => "EmptyState",
            #[cfg(widgets_unstripped)]
            Self::Carousel => "Carousel",
            #[cfg(widgets_unstripped)]
            Self::ColorHistory => "ColorHistory",
            #[cfg(widgets_unstripped)]
            Self::ColorWell => "ColorWell",
            #[cfg(widgets_unstripped)]
            Self::TagInput => "TagInput",
            #[cfg(widgets_unstripped)]
            Self::ImePreedit => "ImePreedit",
            #[cfg(widgets_unstripped)]
            Self::InplaceEditor => "InplaceEditor",
            #[cfg(widgets_unstripped)]
            Self::QRCode => "QRCode",
            #[cfg(widgets_unstripped)]
            Self::MasonryLayout => "MasonryLayout",
            #[cfg(widgets_unstripped)]
            Self::CupertinoSwitch => "CupertinoSwitch",
            #[cfg(widgets_unstripped)]
            Self::MaterialSnackbar => "MaterialSnackbar",
            #[cfg(widgets_unstripped)]
            Self::AdaptiveScaffold => "AdaptiveScaffold",
            #[cfg(widgets_unstripped)]
            Self::WizardDialog => "WizardDialog",
            #[cfg(widgets_unstripped)]
            Self::SafeArea => "SafeArea",
            #[cfg(widgets_unstripped)]
            Self::CupertinoAlertDialog => "CupertinoAlertDialog",
            #[cfg(widgets_unstripped)]
            Self::CupertinoSlider => "CupertinoSlider",
            #[cfg(widgets_unstripped)]
            Self::MaterialNavigationRail => "MaterialNavigationRail",
            #[cfg(widgets_unstripped)]
            Self::Tooltip => "Tooltip",
            #[cfg(widgets_unstripped)]
            Self::SegmentedButton => "SegmentedButton",
            #[cfg(widgets_unstripped)]
            Self::NavigationStack => "NavigationStack",
            #[cfg(widgets_unstripped)]
            Self::ProgressCircle => "ProgressCircle",
            #[cfg(widgets_unstripped)]
            Self::Icon => "Icon",
            #[cfg(widgets_unstripped)]
            Self::DropdownMenu => "DropdownMenu",
            #[cfg(widgets_unstripped)]
            Self::MaskedEdit => "MaskedEdit",
            #[cfg(widgets_unstripped)]
            Self::MenuButton => "MenuButton",
            #[cfg(widgets_unstripped)]
            Self::Popover => "Popover",
            #[cfg(widgets_unstripped)]
            Self::AutoCompleteEdit => "AutoCompleteEdit",
            #[cfg(widgets_unstripped)]
            Self::MultiSelectComboBox => "MultiSelectComboBox",
            #[cfg(widgets_unstripped)]
            Self::RangeSlider => "RangeSlider",
            #[cfg(widgets_unstripped)]
            Self::FloatingLabel => "FloatingLabel",
            #[cfg(widgets_unstripped)]
            Self::FontPreview => "FontPreview",
            #[cfg(widgets_unstripped)]
            Self::CupertinoNavigationBar => "CupertinoNavigationBar",
            #[cfg(widgets_unstripped)]
            Self::CupertinoSegmentedControl => "CupertinoSegmentedControl",
            #[cfg(widgets_unstripped)]
            Self::SwipeToDismiss => "SwipeToDismiss",
            #[cfg(widgets_unstripped)]
            Self::TabView => "TabView",
            #[cfg(widgets_unstripped)]
            Self::SearchBar => "SearchBar",
            #[cfg(widgets_unstripped)]
            Self::ShortcutEditor => "ShortcutEditor",
            #[cfg(widgets_unstripped)]
            Self::RefreshControl => "RefreshControl",
            #[cfg(widgets_unstripped)]
            Self::ModalBottomSheet => "ModalBottomSheet",
            #[cfg(widgets_unstripped)]
            Self::LineChart => "LineChart",
            #[cfg(widgets_unstripped)]
            Self::Sparkline => "Sparkline",
            #[cfg(widgets_unstripped)]
            Self::BarChart => "BarChart",
            #[cfg(widgets_unstripped)]
            Self::CandlestickChart => "CandlestickChart",
            #[cfg(widgets_unstripped)]
            Self::VolumeChart => "VolumeChart",
            #[cfg(widgets_unstripped)]
            Self::DepthChart => "DepthChart",
            #[cfg(widgets_unstripped)]
            Self::OrderBook => "OrderBook",
            #[cfg(widgets_unstripped)]
            Self::QuoteBoard => "QuoteBoard",
            #[cfg(widgets_unstripped)]
            Self::IndicatorChart => "IndicatorChart",
            #[cfg(widgets_unstripped)]
            Self::FindReplaceDialog => "FindReplaceDialog",
            #[cfg(widgets_unstripped)]
            Self::PropertiesPanel => "PropertiesPanel",
            #[cfg(widgets_unstripped)]
            Self::PieChart => "PieChart",
            #[cfg(widgets_unstripped)]
            Self::CupertinoDatePicker => "CupertinoDatePicker",
            #[cfg(widgets_unstripped)]
            Self::EditableComboBox => "EditableComboBox",
            #[cfg(widgets_unstripped)]
            Self::DateRangePicker => "DateRangePicker",
            #[cfg(widgets_unstripped)]
            Self::AnimatedImage => "AnimatedImage",
            #[cfg(widgets_unstripped)]
            Self::HeroAnimation => "HeroAnimation",
            #[cfg(widgets_unstripped)]
            Self::BezierCurveEditor => "BezierCurveEditor",
            #[cfg(widgets_unstripped)]
            Self::LottieWidget => "LottieWidget",
            #[cfg(widgets_unstripped)]
            Self::RiveWidget => "RiveWidget",
            #[cfg(widgets_unstripped)]
            Self::VideoPlayer => "VideoPlayer",
            #[cfg(widgets_unstripped)]
            Self::ImageGallery => "ImageGallery",
            #[cfg(widgets_unstripped)]
            Self::AudioVisualizer => "AudioVisualizer",
            #[cfg(widgets_unstripped)]
            Self::CameraPreview => "CameraPreview",
            #[cfg(widgets_unstripped)]
            Self::BarcodeScanner => "BarcodeScanner",
            #[cfg(widgets_unstripped)]
            Self::GridTable => "GridTable",
            #[cfg(widgets_unstripped)]
            Self::NumberPicker => "NumberPicker",
            #[cfg(widgets_unstripped)]
            Self::OtpInput => "OtpInput",
            #[cfg(widgets_unstripped)]
            Self::Banner => "Banner",
            #[cfg(widgets_unstripped)]
            Self::Pagination => "Pagination",
            #[cfg(widgets_unstripped)]
            Self::ColorPicker => "ColorPicker",
            #[cfg(widgets_unstripped)]
            Self::Toast => "Toast",
            #[cfg(widgets_unstripped)]
            Self::SplashScreen => "SplashScreen",
            #[cfg(widgets_unstripped)]
            Self::TreeTable => "TreeTable",
            #[cfg(widgets_unstripped)]
            Self::Breadcrumb => "Breadcrumb",
            #[cfg(widgets_unstripped)]
            Self::SignaturePad => "SignaturePad",
            #[cfg(widgets_unstripped)]
            Self::DropZone => "DropZone",
            #[cfg(widgets_unstripped)]
            Self::SegmentedControl => "SegmentedControl",
        }
    }

    /// Whether a user can reach this control with the keyboard.
    ///
    /// # Why this lives on the kind instead of on every control
    ///
    /// [`Widget::is_focusable`](crate::widget::Widget::is_focusable) is what the registry asks
    /// when a control mounts, and [the pointer router](crate::widget::runtime::dispatch_pointer_event)
    /// asks the same question before moving focus on a press. Before this predicate existed the
    /// trait's default answered `false` for **every** control, so no click could focus a field and
    /// Tab had an empty order to walk. One predicate on the kind keeps the two readers agreeing and
    /// lets a control override the trait when its kind is not enough.
    ///
    /// The positive list is deliberately about *interaction*: text entries, buttons and toggles,
    /// value editors, item views and menu entries take keyboard input; containers, decorations and
    /// read-only indicators do not. A kind that is not listed answers `false`, so a future control
    /// that forgets to think about focus cannot silently swallow Tab.
    pub const fn wants_keyboard_focus(self) -> bool {
        match self {
            Self::Button
            | Self::CheckBox
            | Self::RadioButton
            | Self::LineEdit
            | Self::ComboBox
            | Self::SpinBox
            | Self::ListBox
            | Self::Slider
            | Self::Dropdown
            | Self::TextArea
            | Self::Switch => true,
            #[cfg(widgets_unstripped)]
            Self::TextEdit
            | Self::RichEdit
            | Self::ListView
            | Self::TreeView
            | Self::MenuItem
            | Self::Table
            | Self::KanbanBoard
            | Self::ToggleButton
            | Self::CheckListBox
            | Self::DoubleSpinBox
            | Self::Dial
            | Self::DatePicker
            | Self::TimePicker
            | Self::DateTimePicker
            | Self::DataView
            | Self::EmojiPicker
            | Self::Mention
            | Self::QueryBuilder
            | Self::PropertyGrid
            | Self::CollapsiblePane
            | Self::Calendar
            | Self::ColumnView
            | Self::UndoView
            | Self::CommandLink
            | Self::FontComboBox
            | Self::Action
            | Self::ToolButton
            | Self::Cascader
            | Self::TabBar
            | Self::MenuButton
            | Self::MaskedEdit
            | Self::AutoCompleteEdit
            | Self::MultiSelectComboBox
            | Self::RangeSlider
            | Self::FloatingLabel
            | Self::CupertinoSwitch
            | Self::SegmentedButton
            | Self::EditableComboBox
            | Self::SearchBar
            | Self::ShortcutEditor
            | Self::Stepper
            | Self::Rating
            | Self::ColorWell
            | Self::NumberPicker
            | Self::OtpInput
            | Self::GridTable
            | Self::TreeTable
            | Self::Breadcrumb
            | Self::SegmentedControl
            | Self::CupertinoSlider
            | Self::MaterialNavigationRail
            | Self::CupertinoSegmentedControl
            | Self::DateRangePicker
            | Self::CupertinoDatePicker => true,
            _ => false,
        }
    }

    /// Whether this control carries editable text and therefore joins the IME's focus.
    ///
    /// A toolkit input-method context must only consume keys while a control that accepts
    /// committed text owns focus; filtering keys for a button would swallow its activation. The
    /// list is the text-entry family, and a kind outside it answers `false` so a new entry control
    /// must opt in deliberately.
    pub const fn accepts_ime_input(self) -> bool {
        match self {
            Self::LineEdit => true,
            Self::TextArea => true,
            #[cfg(widgets_unstripped)]
            Self::TextEdit
            | Self::RichEdit
            | Self::SearchBar
            | Self::SearchBox
            | Self::TagInput
            | Self::MaskedEdit
            | Self::FloatingLabel
            | Self::AutoCompleteEdit
            | Self::Mention
            | Self::EditableComboBox
            | Self::OtpInput
            | Self::InplaceEditor => true,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    /// Every registered kind must name itself exactly as `Debug` does.
    ///
    /// [`WidgetKind::css_name`] duplicates the spelling the compiler derives, because deriving
    /// is what allocates. The exhaustive match guarantees a variant is *named*; only this test
    /// can show it is named *correctly*. The registry is the right list to check: it holds every
    /// kind a JSON document can produce, and the loader's stylesheet pass is the only consumer
    /// of the string.
    #[test]
    fn css_name_matches_the_debug_spelling_for_every_registered_kind() {
        let capabilities = crate::widget::WidgetFactory::shared().capabilities();
        assert!(
            capabilities.len() > 100,
            "the registry has {} entries, which would make this check vacuous",
            capabilities.len()
        );
        for capability in capabilities {
            assert_eq!(
                capability.kind.css_name(),
                format!("{:?}", capability.kind),
                "{:?} names itself differently from its Debug spelling",
                capability.kind
            );
        }
    }
}
