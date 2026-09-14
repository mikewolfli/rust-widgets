// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Widget models and controls.
// Base widget types
pub mod base;
/// Widget capability metadata, the runtime factory, and the property contract.
///
/// The module is compiled in **every** profile, not only device ones: the
/// property contract ([`capability::WidgetProperties`]) is what
/// `Widget::properties_dyn` returns, and `Widget` exists everywhere. Only the
/// profile-specific *parts* — the property schema tables, the legacy centralised
/// access layer, the factory and its registration — are gated, below.
pub mod capability;
pub mod draw;
/// The bridge that connects a self-painting widget to `dyn Widget`.
///
/// Declared with `#[macro_use]` so `impl_draw_bridge!` is in scope in every
/// widget module without each one adding an import — the macro is meant to be
/// called from the widget's own `impl Widget` block, wherever that block lives.
#[macro_use]
pub mod draw_bridge;
#[cfg(feature = "image")]
pub mod image;
pub mod kind;
#[cfg(not(alloc_frugal))]
pub mod runtime;
pub mod widget_trait;
// Widget subfolders
#[cfg(full_widgets)]
pub mod advanced_widgets;
pub mod base_widgets;
#[cfg(full_widgets)]
pub mod chart_widgets;
pub mod container_widgets;
#[cfg(full_widgets)]
pub mod cupertino;
#[cfg(full_widgets)]
pub mod dialog;
pub mod display_widgets;
pub mod input_widgets;
#[cfg(full_widgets)]
pub mod media_widgets;
#[cfg(full_widgets)]
pub mod menu_toolbar;
#[cfg(full_widgets)]
pub mod misc_widgets;
#[cfg(full_widgets)]
pub mod nav_widgets;
#[cfg(full_widgets)]
pub mod overlay_widgets;
pub mod registry;
#[cfg(full_widgets)]
pub mod special_widgets;
#[cfg(full_widgets)]
pub mod view_widgets;
#[cfg(full_widgets)]
pub mod web_widgets;
// Individual widget files (not in subfolders)
pub mod svg;
pub mod window;
pub use window::Window;

// Re-export base types
pub use base::BaseWidget;
// The value/error types and the property contract exist in every profile; the
// factory and its metadata do not, because they enumerate concrete controls.
#[cfg(widgets_unstripped)]
pub use capability::WidgetFactory;
pub use capability::{
    base_property_get, base_property_set, CapabilityAccessError, CapabilityValue, PropertySchema,
    PropertyValueKind, WidgetCapability, WidgetProperties, BASE_PROPERTY_NAMES,
};
pub use draw::Draw;
#[cfg(feature = "image")]
pub use image::Image;
#[cfg(feature = "image")]
pub use image::ImageFormat;
pub use kind::WidgetKind;
pub use registry::SimpleRegistry;
pub use widget_trait::Widget;
// Re-export widget types from subfolders
#[cfg(widgets_unstripped)]
pub use base_widgets::toggle_button::{ToggleButton, ToggleButtonState};
pub use base_widgets::{
    button::{Button, ButtonState},
    checkbox::{CheckBox, CheckState},
    label::Label,
    radiobutton::RadioButton,
};
#[cfg(widgets_unstripped)]
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
#[cfg(widgets_unstripped)]
pub use container_widgets::collapsible_pane::CollapsiblePane;
#[cfg(widgets_unstripped)]
pub use container_widgets::dockwidget::DockWidget;
pub use container_widgets::groupbox::GroupBox;
#[cfg(widgets_unstripped)]
pub use container_widgets::mdiarea::MdiArea;
pub use container_widgets::scrollarea::ScrollArea;
#[cfg(widgets_unstripped)]
pub use container_widgets::splitter::Splitter;
#[cfg(widgets_unstripped)]
pub use container_widgets::stackedwidget::StackedWidget;
#[cfg(widgets_unstripped)]
pub use container_widgets::tabwidget::TabWidget;
pub use container_widgets::tile_view::TileView;
#[cfg(widgets_unstripped)]
pub use container_widgets::toolbox::ToolBox;
pub type Panel = GroupBox;
pub use base_widgets::frame::Frame;
#[cfg(widgets_unstripped)]
pub type DockPanel = DockWidget;
// Re-export container widgets from new additions
#[cfg(widgets_unstripped)]
pub use container_widgets::carousel::Carousel;
#[cfg(widgets_unstripped)]
pub use container_widgets::masonry_layout::{MasonryItem, MasonryLayout};
#[cfg(widgets_unstripped)]
pub use container_widgets::pager_page_view::PagerPageView;
#[cfg(widgets_unstripped)]
pub use container_widgets::safe_area::{SafeArea, SafeAreaInsets};
#[cfg(widgets_unstripped)]
pub use container_widgets::stepper::Stepper;
// Re-export display widgets
pub use display_widgets::arc::Arc;
#[cfg(feature = "image")]
pub use display_widgets::image_view::ImageView;
#[cfg(widgets_unstripped)]
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
#[cfg(widgets_unstripped)]
pub use display_widgets::badge::Badge;
#[cfg(widgets_unstripped)]
pub use display_widgets::color_history::ColorHistory;
#[cfg(widgets_unstripped)]
pub use display_widgets::color_well::ColorWell;
#[cfg(widgets_unstripped)]
pub use display_widgets::divider::Divider;
#[cfg(widgets_unstripped)]
pub use display_widgets::empty_state::EmptyState;
#[cfg(widgets_unstripped)]
pub use display_widgets::floating_label::FloatingLabel;
#[cfg(widgets_unstripped)]
pub use display_widgets::font_preview::FontPreview;
#[cfg(widgets_unstripped)]
pub use display_widgets::icon::{Icon, IconName};
#[cfg(widgets_unstripped)]
pub use display_widgets::progress_circle::ProgressCircle;
#[cfg(widgets_unstripped)]
pub use display_widgets::rating::Rating;
#[cfg(widgets_unstripped)]
pub use display_widgets::skeleton_loader::SkeletonLoader;
pub use display_widgets::switch::Switch;
// Re-export nav widgets
#[cfg(full_widgets)]
pub use nav_widgets::adaptive_scaffold::AdaptiveScaffold;
#[cfg(full_widgets)]
pub use nav_widgets::app_bar::AppBar;
#[cfg(full_widgets)]
pub use nav_widgets::bottom_navigation_bar::BottomNavigationBar;
#[cfg(full_widgets)]
pub use nav_widgets::bottom_navigation_bar::NavItem;
#[cfg(full_widgets)]
pub use nav_widgets::navigation_drawer::NavigationDrawer;
#[cfg(full_widgets)]
pub use nav_widgets::navigation_stack::NavigationEvent;
#[cfg(full_widgets)]
pub use nav_widgets::navigation_stack::NavigationStack;
#[cfg(full_widgets)]
pub use nav_widgets::tab_view::TabPage;
#[cfg(full_widgets)]
pub use nav_widgets::tab_view::TabView;
// Re-export chart widgets
#[cfg(full_widgets)]
pub use chart_widgets::bar_chart::{BarChart, BarEntry};
#[cfg(full_widgets)]
pub use chart_widgets::line_chart::LineChart;
#[cfg(full_widgets)]
pub use chart_widgets::pie_chart::{PieChart, PieSlice};
#[cfg(full_widgets)]
pub use chart_widgets::sparkline::Sparkline;
// Re-export media widgets
#[cfg(full_widgets)]
pub use media_widgets::animated_image::{AnimatedFrame, AnimatedImage, AnimatedImageFormat};
#[cfg(full_widgets)]
pub use media_widgets::audio_visualizer::AudioVisualizer;
#[cfg(full_widgets)]
pub use media_widgets::camera_preview::CameraPreview;
#[cfg(full_widgets)]
pub use media_widgets::hero_animation::HeroAnimation;
#[cfg(full_widgets)]
pub use media_widgets::lottie_widget::LottieWidget;
#[cfg(full_widgets)]
pub use media_widgets::rive_widget::{RiveInput, RiveInputValue, RiveWidget};
#[cfg(full_widgets)]
pub use media_widgets::video_player::VideoPlayer;
// Re-export overlay widgets
#[cfg(full_widgets)]
pub use overlay_widgets::fab::FAB;
#[cfg(full_widgets)]
pub use overlay_widgets::refresh_control::RefreshControl;
#[cfg(full_widgets)]
pub type PullToRefresh = RefreshControl;
#[cfg(full_widgets)]
pub use overlay_widgets::swipe_to_dismiss::SwipeToDismiss;
// Re-export cupertino widgets
#[cfg(full_widgets)]
pub use cupertino::{
    core::CupertinoAlertDialog, core::CupertinoSlider, core::CupertinoSwitch,
    core::MaterialNavigationRail, core::MaterialSnackbar, core::RailItem, CupertinoDatePicker,
    CupertinoNavigationBar, CupertinoSegmentedControl,
};
// Re-export misc widgets
#[cfg(full_widgets)]
pub use misc_widgets::avatar::Avatar;
#[cfg(full_widgets)]
pub use misc_widgets::barcode_scanner::{BarcodeFormat, BarcodeResult, BarcodeScanner};
#[cfg(full_widgets)]
pub use misc_widgets::bezier_curve_editor::BezierCurveEditor;
#[cfg(full_widgets)]
pub use misc_widgets::date_range_picker::DateRangePicker;
#[cfg(full_widgets)]
pub use misc_widgets::mobile_date_picker::MobileDatePicker;
#[cfg(full_widgets)]
pub use misc_widgets::qr_code::QRCode;
#[cfg(full_widgets)]
pub use misc_widgets::segmented_button::{Segment, SegmentedButton};
// Re-export web widgets
#[cfg(full_widgets)]
pub use web_widgets::web_engine::WebEngine;
/// Type alias for backward compatibility — `WebView` is now `WebEngineView`.
#[cfg(full_widgets)]
pub type WebView = WebEngineView;
#[cfg(full_widgets)]
pub use web_widgets::{
    WebEngineContextMenuRequest, WebEngineCookieStore, WebEngineDownloadItem,
    WebEngineFindTextResult, WebEngineNotification, WebEnginePage, WebEngineScriptDialog,
    WebEngineSettings, WebEngineView, WebEngineWebChannel,
};
// Re-export advanced widgets
#[cfg(full_widgets)]
pub use advanced_widgets::{
    calendar::Calendar, date_edit::DateEdit, date_time_edit::DateTimeEdit, dial::Dial,
    key_sequence_edit::KeySequenceEdit, pie_menu::PieMenu, pie_menu::PieMenuItem,
    ribbon_bar::RibbonBar, ribbon_bar::RibbonGroup, ribbon_bar::RibbonItem, tab_bar::TabBar,
    tab_bar::TabBarTab, time_edit::TimeEdit,
};
// Re-export dialog widgets
#[cfg(full_widgets)]
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
#[cfg(full_widgets)]
pub type Dialog = PopupWindow;
#[cfg(full_widgets)]
pub type DirectoryDialog = FileDialog;
// Re-export menu and toolbar widgets
#[cfg(full_widgets)]
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
#[cfg(full_widgets)]
pub type ContextMenu = Menu;
// Re-export view widgets
#[cfg(full_widgets)]
pub use view_widgets::table_widget::TableModel;
#[cfg(full_widgets)]
pub use view_widgets::tree_view::TreeModel;
#[cfg(full_widgets)]
pub use view_widgets::{
    data_grid::{ColumnFilter, DataGrid, SortSpec},
    grid_table::GridTableWidget,
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
#[cfg(full_widgets)]
pub use special_widgets::{
    Breadcrumb, BreadcrumbSegment, Canvas, ChartWidget, Chip, ChipItem, CodeEditor, ColorPicker,
    CommandEntry, CommandPalette, DiagnosticMarker, DiffKind, DiffLine, DiffViewer,
    FreeformShapeWidget, GanttTask, GanttWidget, GridWidget, MapMarker, MapView, MarkdownEditor,
    MarkerSeverity, MediaPlayer, NotificationCenter, NotificationItem, NotificationLevel,
    SegmentItem, SegmentedControl, Snackbar, SplitAction, SplitButton, TerminalView, TimelineItem,
    TimelineWidget, ToastItem, ToastLevel, ToastStack,
};
#[cfg(full_widgets)]
pub type ActivityIndicator = ProgressBar;
#[cfg(full_widgets)]
pub type CheckListBox = ListBox;
#[cfg(full_widgets)]
pub type Toolbox = ToolBox;
#[cfg(full_widgets)]
pub type DoubleSpinBox = SpinBox;
#[cfg(full_widgets)]
pub type Wizard = WizardDialog;
// ── P3-6: WidgetKind variant type aliases ──
#[cfg(full_widgets)]
pub type DataView = VirtualList;
#[cfg(full_widgets)]
pub type ColumnView = TreeView;
#[cfg(full_widgets)]
pub type UndoView = ListView;
#[cfg(full_widgets)]
pub type DatePicker = DateEdit;
#[cfg(full_widgets)]
pub type TimePicker = TimeEdit;
#[cfg(full_widgets)]
pub type DateTimePicker = DateTimeEdit;
#[cfg(full_widgets)]
pub type Grid = GridWidget;
#[cfg(full_widgets)]
pub type Chart = ChartWidget;
#[cfg(full_widgets)]
pub type GridTable = GridTableWidget;
#[cfg(full_widgets)]
pub type Table = TableWidget;
#[cfg(full_widgets)]
pub type FreeformShape = FreeformShapeWidget;
