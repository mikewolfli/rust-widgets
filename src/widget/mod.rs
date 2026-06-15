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
#[cfg(not(feature = "mini"))]
pub mod chart_widgets;
pub mod container_widgets;
#[cfg(not(feature = "mini"))]
pub mod cupertino;
#[cfg(not(feature = "mini"))]
pub mod dialog;
pub mod display_widgets;
pub mod input_widgets;
#[cfg(not(feature = "mini"))]
pub mod media_widgets;
#[cfg(not(feature = "mini"))]
pub mod menu_toolbar;
#[cfg(not(feature = "mini"))]
pub mod misc_widgets;
#[cfg(not(feature = "mini"))]
pub mod nav_widgets;
#[cfg(not(feature = "mini"))]
pub mod overlay_widgets;
pub mod registry;
#[cfg(not(feature = "mini"))]
pub mod special_widgets;
#[cfg(not(feature = "mini"))]
pub mod view_widgets;
#[cfg(not(feature = "mini"))]
pub mod web_widgets;
// Individual widget files (not in subfolders)
#[cfg(not(feature = "mini"))]
pub mod svg;
pub mod window;
pub use window::Window;

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
#[cfg(not(feature = "mini"))]
pub use input_widgets::{
    auto_complete_edit::AutoCompleteEdit,
    command_link::CommandLink,
    editable_combo_box::EditableComboBox,
    font_combo_box::FontComboBox,
    ime_preedit::ImePreedit,
    inplace_editor::InplaceEditor,
    masked_edit::MaskedEdit,
    multi_select_combo_box::{MultiSelectComboBox, MultiSelectItem},
    range_slider::{RangeSlider, RangeSliderOrientation},
    rich_edit::RichEdit,
    search_bar::SearchBar,
    search_box::SearchBox,
    shortcut_editor::{ShortcutEditor, ShortcutEntry},
    tag_input::TagInput,
    textedit::TextEdit,
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
pub use base_widgets::frame::Frame;
#[cfg(not(feature = "mini"))]
pub type DockPanel = DockWidget;
// Re-export container widgets from new additions
#[cfg(not(feature = "mini"))]
pub use container_widgets::carousel::Carousel;
#[cfg(not(feature = "mini"))]
pub use container_widgets::masonry_layout::{MasonryItem, MasonryLayout};
#[cfg(not(feature = "mini"))]
pub use container_widgets::pager_page_view::PagerPageView;
#[cfg(not(feature = "mini"))]
pub use container_widgets::safe_area::{SafeArea, SafeAreaInsets};
#[cfg(not(feature = "mini"))]
pub use container_widgets::stepper::Stepper;
// Re-export display widgets
pub use display_widgets::arc::Arc;
pub use display_widgets::image_view::ImageView;
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
// Re-export display widgets from new additions
#[cfg(not(feature = "mini"))]
pub use display_widgets::badge::Badge;
#[cfg(not(feature = "mini"))]
pub use display_widgets::color_history::ColorHistory;
#[cfg(not(feature = "mini"))]
pub use display_widgets::color_well::ColorWell;
#[cfg(not(feature = "mini"))]
pub use display_widgets::divider::Divider;
#[cfg(not(feature = "mini"))]
pub use display_widgets::empty_state::EmptyState;
#[cfg(not(feature = "mini"))]
pub use display_widgets::floating_label::FloatingLabel;
#[cfg(not(feature = "mini"))]
pub use display_widgets::font_preview::FontPreview;
#[cfg(not(feature = "mini"))]
pub use display_widgets::icon::{Icon, IconName};
#[cfg(not(feature = "mini"))]
pub use display_widgets::progress_circle::ProgressCircle;
#[cfg(not(feature = "mini"))]
pub use display_widgets::rating::Rating;
#[cfg(not(feature = "mini"))]
pub use display_widgets::skeleton_loader::SkeletonLoader;
pub use display_widgets::switch::Switch;
// Re-export nav widgets
#[cfg(not(feature = "mini"))]
pub use nav_widgets::adaptive_scaffold::AdaptiveScaffold;
#[cfg(not(feature = "mini"))]
pub use nav_widgets::app_bar::AppBar;
#[cfg(not(feature = "mini"))]
pub use nav_widgets::bottom_navigation_bar::BottomNavigationBar;
#[cfg(not(feature = "mini"))]
pub use nav_widgets::bottom_navigation_bar::NavItem;
#[cfg(not(feature = "mini"))]
pub use nav_widgets::navigation_drawer::NavigationDrawer;
#[cfg(not(feature = "mini"))]
pub use nav_widgets::navigation_stack::NavigationEvent;
#[cfg(not(feature = "mini"))]
pub use nav_widgets::navigation_stack::NavigationStack;
pub use nav_widgets::tab_view::TabPage;
pub use nav_widgets::tab_view::TabView;
// Re-export chart widgets
#[cfg(not(feature = "mini"))]
pub use chart_widgets::bar_chart::{BarChart, BarEntry};
#[cfg(not(feature = "mini"))]
pub use chart_widgets::line_chart::LineChart;
#[cfg(not(feature = "mini"))]
pub use chart_widgets::pie_chart::{PieChart, PieSlice};
#[cfg(not(feature = "mini"))]
pub use chart_widgets::sparkline::Sparkline;
// Re-export media widgets
pub use media_widgets::animated_image::{AnimatedFrame, AnimatedImage, AnimatedImageFormat};
#[cfg(not(feature = "mini"))]
pub use media_widgets::audio_visualizer::AudioVisualizer;
#[cfg(not(feature = "mini"))]
pub use media_widgets::camera_preview::CameraPreview;
#[cfg(not(feature = "mini"))]
pub use media_widgets::hero_animation::HeroAnimation;
#[cfg(not(feature = "mini"))]
pub use media_widgets::lottie_widget::LottieWidget;
#[cfg(not(feature = "mini"))]
pub use media_widgets::rive_widget::{RiveInput, RiveInputValue, RiveWidget};
#[cfg(not(feature = "mini"))]
pub use media_widgets::video_player::VideoPlayer;
// Re-export overlay widgets
#[cfg(not(feature = "mini"))]
pub use overlay_widgets::fab::FAB;
#[cfg(not(feature = "mini"))]
pub use overlay_widgets::refresh_control::RefreshControl;
#[cfg(not(feature = "mini"))]
pub type PullToRefresh = RefreshControl;
#[cfg(not(feature = "mini"))]
pub use overlay_widgets::swipe_to_dismiss::SwipeToDismiss;
// Re-export cupertino widgets
#[cfg(not(feature = "mini"))]
pub use cupertino::{
    core::CupertinoAlertDialog, core::CupertinoSlider, core::CupertinoSwitch,
    core::MaterialNavigationRail, core::MaterialSnackbar, core::RailItem, CupertinoDatePicker,
    CupertinoNavigationBar, CupertinoSegmentedControl,
};
// Re-export misc widgets
#[cfg(not(feature = "mini"))]
pub use misc_widgets::avatar::Avatar;
#[cfg(not(feature = "mini"))]
pub use misc_widgets::barcode_scanner::{BarcodeFormat, BarcodeResult, BarcodeScanner};
#[cfg(not(feature = "mini"))]
pub use misc_widgets::bezier_curve_editor::BezierCurveEditor;
#[cfg(not(feature = "mini"))]
pub use misc_widgets::date_range_picker::DateRangePicker;
#[cfg(not(feature = "mini"))]
pub use misc_widgets::mobile_date_picker::MobileDatePicker;
#[cfg(not(feature = "mini"))]
pub use misc_widgets::qr_code::QRCode;
#[cfg(not(feature = "mini"))]
pub use misc_widgets::segmented_button::{Segment, SegmentedButton};
// Re-export web widgets
#[cfg(not(feature = "mini"))]
pub use web_widgets::web_engine::WebEngine;
/// Type alias for backward compatibility — `WebView` is now `WebEngineView`.
#[cfg(not(feature = "mini"))]
pub type WebView = WebEngineView;
#[cfg(not(feature = "mini"))]
pub use web_widgets::{
    WebEngineContextMenuRequest, WebEngineCookieStore, WebEngineDownloadItem,
    WebEngineFindTextResult, WebEngineNotification, WebEnginePage, WebEngineScriptDialog,
    WebEngineSettings, WebEngineView, WebEngineWebChannel,
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
    bottom_sheet::BottomSheet,
    color_dialog::ColorDialog,
    file_dialog::FileDialog,
    find_replace_dialog::FindReplaceDialog,
    font_dialog::FontDialog,
    input_dialog::InputDialog,
    message_box::MessageBox,
    modal_bottom_sheet::ModalBottomSheet,
    popover::Popover,
    popup_window::PopupWindow,
    progress_dialog::ProgressDialog,
    tooltip::Tooltip,
    wizard::{WizardDialog, WizardStep},
};
#[cfg(not(feature = "mini"))]
pub type Dialog = PopupWindow;
#[cfg(not(feature = "mini"))]
pub type DirectoryDialog = FileDialog;
// Re-export menu and toolbar widgets
#[cfg(not(feature = "mini"))]
pub use menu_toolbar::{
    action::Action,
    dropdown_menu::{DropdownItem, DropdownMenu},
    menu::Menu,
    menu_bar::MenuBar,
    menu_button::{MenuButton, MenuItem},
    status_bar::StatusBar,
    tool_bar::ToolBar,
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
    image_gallery::{GalleryImage, ImageGallery},
    list_view::{ListModel, ListView, VecListModel},
    properties_panel::{PropertiesPanel, PropertyEntry, PropertyValue},
    property_grid::{PropertyGrid, PropertyItem},
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
