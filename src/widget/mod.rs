//! Widget models and controls.
// Base widget types
pub mod base;
pub mod capability;
pub mod draw;
pub mod image;
pub mod kind;
pub mod widget_trait;
// Widget subfolders
#[cfg(not(feature = "mini"))]
pub mod advanced_widgets;
pub mod base_widgets;
pub mod container_widgets;
#[cfg(not(feature = "mini"))]
pub mod dialog;
pub mod display_widgets;
pub mod input_widgets;
#[cfg(not(feature = "mini"))]
pub mod menu_toolbar;
pub mod registry;
#[cfg(not(feature = "mini"))]
pub mod special_widgets;
#[cfg(not(feature = "mini"))]
pub mod view_widgets;
#[cfg(not(feature = "mini"))]
pub mod web_widgets;
// New widget types (modern UI controls, mobile-first)
pub mod new_widgets;
// Individual widget files (not in subfolders)
#[cfg(not(feature = "mini"))]
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
#[cfg(not(feature = "mini"))]
pub use base_widgets::toggle_button::{ToggleButton, ToggleButtonState};
pub use base_widgets::{
    button::{Button, ButtonState},
    checkbox::{CheckBox, CheckState},
    label::Label,
    radiobutton::RadioButton,
};
pub use input_widgets::{
    combobox::ComboBox,
    dropdown::Dropdown,
    keyboard::Keyboard,
    lineedit::{EchoMode, LineEdit},
    listbox::{ListBox, SelectionMode},
    spinbox::SpinBox,
    textarea::TextArea,
};
#[cfg(not(feature = "mini"))]
pub use input_widgets::{
    command_link::CommandLink, font_combo_box::FontComboBox, rich_edit::RichEdit,
    textedit::TextEdit,
};
// Re-export container widgets
#[cfg(not(feature = "mini"))]
pub use container_widgets::collapsible_pane::CollapsiblePane;
#[cfg(not(feature = "mini"))]
pub use container_widgets::dockwidget::DockWidget;
pub use container_widgets::groupbox::GroupBox;
#[cfg(not(feature = "mini"))]
pub use container_widgets::mdiarea::MdiArea;
pub use container_widgets::scrollarea::ScrollArea;
#[cfg(not(feature = "mini"))]
pub use container_widgets::splitter::Splitter;
#[cfg(not(feature = "mini"))]
pub use container_widgets::stackedwidget::StackedWidget;
#[cfg(not(feature = "mini"))]
pub use container_widgets::tabwidget::TabWidget;
pub use container_widgets::tile_view::TileView;
#[cfg(not(feature = "mini"))]
pub use container_widgets::toolbox::ToolBox;
pub type Panel = GroupBox;
#[cfg(not(feature = "mini"))]
pub type DockPanel = DockWidget;
// Re-export display widgets
pub use display_widgets::arc::Arc;
pub use display_widgets::image_view::ImageView;
#[cfg(not(feature = "mini"))]
pub use display_widgets::lcd_number::LCDNumber as LcdNumber;
#[cfg(not(feature = "mini"))]
pub use display_widgets::lcd_number::LCDNumber;
pub use display_widgets::line::{Line, LineOrientation};
pub use display_widgets::meter::Meter;
pub use display_widgets::mini_canvas::MiniCanvas;
pub use display_widgets::mini_chart::MiniChart;
pub use display_widgets::progressbar::ProgressBar;
pub use display_widgets::roller::Roller;
pub use display_widgets::scrollbar::ScrollBar;
pub use display_widgets::slider::Slider;
pub use display_widgets::spinner::Spinner;
// Re-export new widgets (core: Switch is always available)
pub use new_widgets::switch::Switch;
#[cfg(not(feature = "mini"))]
pub use new_widgets::{
    adaptive_scaffold::AdaptiveScaffold,
    animated_image::{AnimatedFrame, AnimatedImage, AnimatedImageFormat},
    app_bar::AppBar,
    audio_visualizer::AudioVisualizer,
    auto_complete_edit::AutoCompleteEdit,
    avatar::Avatar,
    badge::Badge,
    bar_chart::{BarChart, BarEntry},
    barcode_scanner::{BarcodeFormat, BarcodeResult, BarcodeScanner},
    bezier_curve_editor::BezierCurveEditor,
    bottom_navigation_bar::BottomNavigationBar,
    bottom_navigation_bar::NavItem,
    bottom_sheet::BottomSheet,
    camera_preview::CameraPreview,
    carousel::Carousel,
    color_history::ColorHistory,
    color_well::ColorWell,
    cupertino::CupertinoAlertDialog,
    cupertino::CupertinoSlider,
    cupertino::CupertinoSwitch,
    cupertino::MaterialNavigationRail,
    cupertino::MaterialSnackbar,
    cupertino::RailItem,
    cupertino_date_picker::CupertinoDatePicker,
    cupertino_nav_bar::CupertinoNavigationBar,
    cupertino_segmented_control::CupertinoSegmentedControl,
    date_range_picker::DateRangePicker,
    divider::Divider,
    dropdown_menu::{DropdownItem, DropdownMenu},
    editable_combo_box::EditableComboBox,
    empty_state::EmptyState,
    fab::FAB,
    find_replace_dialog::FindReplaceDialog,
    floating_label::FloatingLabel,
    font_preview::FontPreview,
    hero_animation::HeroAnimation,
    icon::Icon,
    icon::IconName,
    image_gallery::{GalleryImage, ImageGallery},
    inplace_editor::InplaceEditor,
    line_chart::LineChart,
    lottie_widget::LottieWidget,
    masked_edit::MaskedEdit,
    masonry_layout::MasonryItem,
    masonry_layout::MasonryLayout,
    menu_button::{MenuButton, MenuItem},
    mobile_date_picker::MobileDatePicker,
    modal_bottom_sheet::ModalBottomSheet,
    multi_select_combo_box::MultiSelectComboBox,
    multi_select_combo_box::MultiSelectItem,
    navigation_drawer::NavigationDrawer,
    navigation_stack::NavigationEvent,
    navigation_stack::NavigationStack,
    pager_page_view::PagerPageView,
    pie_chart::PieChart,
    pie_chart::PieSlice,
    popover::Popover,
    progress_circle::ProgressCircle,
    properties_panel::{PropertiesPanel, PropertyEntry, PropertyValue},
    property_grid::{PropertyGrid, PropertyItem},
    pull_to_refresh::PullToRefresh,
    qr_code::QRCode,
    range_slider::RangeSlider,
    range_slider::RangeSliderOrientation,
    rating::Rating,
    refresh_control::RefreshControl,
    rive_widget::{RiveInput, RiveInputValue, RiveWidget},
    safe_area::{SafeArea, SafeAreaInsets},
    search_bar::SearchBar,
    search_box::SearchBox,
    segmented_button::Segment,
    segmented_button::SegmentedButton,
    shortcut_editor::ShortcutEditor,
    shortcut_editor::ShortcutEntry,
    skeleton_loader::SkeletonLoader,
    sparkline::Sparkline,
    stepper::Stepper,
    swipe_to_dismiss::SwipeToDismiss,
    tab_view::TabPage,
    tab_view::TabView,
    tag_input::TagInput,
    tooltip::Tooltip,
    video_player::VideoPlayer,
    wizard::{WizardDialog, WizardStep},
};
// Re-export web widgets
#[cfg(not(feature = "mini"))]
pub use web_widgets::{web_engine::WebEngine, web_view::WebView};
#[cfg(not(feature = "mini"))]
pub use web_widgets::{
    WebEngineContextMenuRequest, WebEngineCookieStore, WebEngineDownloadItem,
    WebEngineFindTextResult, WebEngineNotification, WebEnginePage, WebEngineScriptDialog,
    WebEngineSettings, WebEngineWebChannel,
};
// Re-export advanced widgets
#[cfg(not(feature = "mini"))]
pub use advanced_widgets::{
    calendar::Calendar, date_edit::DateEdit, date_time_edit::DateTimeEdit, dial::Dial,
    key_sequence_edit::KeySequenceEdit, pie_menu::PieMenu, pie_menu::PieMenuItem,
    ribbon_bar::RibbonBar, ribbon_bar::RibbonGroup, ribbon_bar::RibbonItem, tab_bar::TabBar,
    tab_bar::TabBarTab, time_edit::TimeEdit,
};
// Re-export dialog widgets
#[cfg(not(feature = "mini"))]
pub use dialog::{
    color_dialog::ColorDialog, file_dialog::FileDialog, font_dialog::FontDialog,
    input_dialog::InputDialog, message_box::MessageBox, popup_window::PopupWindow,
    progress_dialog::ProgressDialog,
};
#[cfg(not(feature = "mini"))]
pub type Dialog = PopupWindow;
#[cfg(not(feature = "mini"))]
pub type DirectoryDialog = FileDialog;
// Re-export menu and toolbar widgets
#[cfg(not(feature = "mini"))]
pub use menu_toolbar::{
    action::Action, menu::Menu, menu_bar::MenuBar, status_bar::StatusBar, tool_bar::ToolBar,
    tool_button::ToolButton,
};
#[cfg(not(feature = "mini"))]
pub type ContextMenu = Menu;
// Re-export view widgets
#[cfg(not(feature = "mini"))]
pub use view_widgets::table_widget::TableModel;
#[cfg(not(feature = "mini"))]
pub use view_widgets::tree_view::TreeModel;
#[cfg(not(feature = "mini"))]
pub use view_widgets::{
    data_grid::{ColumnFilter, DataGrid, SortSpec},
    list_view::{ListModel, ListView, VecListModel},
    table_widget::TableWidget,
    tree_table::{TreeTable, TreeTableModel},
    tree_view::TreeView,
    virtual_list::VirtualList,
    virtual_table::VirtualTable,
};
// Re-export special widgets
#[cfg(not(feature = "mini"))]
pub use special_widgets::{
    Breadcrumb, BreadcrumbSegment, Canvas, ChartWidget, Chip, ChipItem, CodeEditor, ColorPicker,
    CommandEntry, CommandPalette, DiagnosticMarker, DiffKind, DiffLine, DiffViewer,
    FreeformShapeWidget, GanttTask, GanttWidget, GridWidget, MapMarker, MapView, MarkdownEditor,
    MarkerSeverity, MediaPlayer, NotificationCenter, NotificationItem, NotificationLevel,
    SegmentItem, SegmentedControl, Snackbar, SplitAction, SplitButton, TerminalView, TimelineItem,
    TimelineWidget, ToastItem, ToastLevel, ToastStack,
};
#[cfg(not(feature = "mini"))]
pub type ActivityIndicator = ProgressBar;
#[cfg(not(feature = "mini"))]
pub type CheckListBox = ListBox;
#[cfg(not(feature = "mini"))]
pub type Toolbox = ToolBox;
#[cfg(not(feature = "mini"))]
pub type DoubleSpinBox = SpinBox;
#[cfg(not(feature = "mini"))]
pub type Wizard = Panel;
// ── P3-6: WidgetKind variant type aliases ──
#[cfg(not(feature = "mini"))]
pub type DataView = VirtualList;
#[cfg(not(feature = "mini"))]
pub type ColumnView = TreeView;
#[cfg(not(feature = "mini"))]
pub type UndoView = ListView;
#[cfg(not(feature = "mini"))]
pub type DatePicker = DateEdit;
#[cfg(not(feature = "mini"))]
pub type TimePicker = TimeEdit;
#[cfg(not(feature = "mini"))]
pub type DateTimePicker = DateTimeEdit;
