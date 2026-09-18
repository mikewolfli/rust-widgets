// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Widget models, their capabilities, and the runtime that drives them.
//!
//! # Layering
//!
//! The module is organised in three layers on top of the core types:
//!
//! 1. **Widget models** — the control types re-exported at the bottom of this
//!    file ([`Button`], [`Slider`], `MenuBar`, ...). Each is a plain Rust
//!    struct holding its own state and a [`BaseWidget`]. They implement
//!    [`Widget`] by delegating the shared fields to the base, and describe
//!    themselves for accessibility through [`Widget::accessible_role`].
//! 2. **Capability** ([`capability`]) — a uniform, name-addressed view over
//!    those models: a property schema per kind, a get/set dispatch
//!    ([`capability::WidgetProperties`], surfaced as
//!    [`Widget::properties_dyn`]), and the factory that constructs controls by
//!    kind. This is what lets generic tooling (property editors, serialisers,
//!    FFI bindings) manipulate a widget without knowing its concrete type.
//! 3. **Runtime** (`runtime`) — the owner of the live widget tree. It holds
//!    widgets behind `dyn Widget`, delivers events, and resolves
//!    [`ObjectId`](crate::core::ObjectId)s back to widgets. Only compiled when
//!    widgets are not stripped.
//!
//! The layering is one-way: models do not know about the runtime, and the
//! capability layer reaches models only through [`Widget`].
//!
//! # Profile gating
//!
//! Types are gated by profile so that the mini and embedded builds stay small.
//! [`Widget`], [`BaseWidget`], [`WidgetKind`], and the property contract exist
//! in every profile; concrete controls and the factory are gated behind
//! `widgets_unstripped` / `full_widgets`, and the most advanced controls behind
//! `full_widgets` alone. Code that must build everywhere should depend only on
//! [`Widget`] plus the capability layer, and refer to concrete kinds by name at
//! the factory instead of by type.
//!
//! # Reachability
//!
//! **State:** Production callers: `src/lib.rs:1`; 73 files reference it (the entire control library).

// Base widget types
/// The shared state block every widget embeds: id, kind, geometry, visibility,
/// enabled flag, style, parent/child links, and the standard signal set.
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
pub mod kind;

#[cfg(not(alloc_frugal))]
pub mod runtime;
/// Byte-index helpers shared by the text-editing controls.
pub mod text_utils;
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
    base_property_get, base_property_set, widget_property_get, widget_property_names,
    widget_property_set, CapabilityAccessError, CapabilityValue, PropertySchema, PropertyValueKind,
    WidgetCapability, WidgetProperties, BASE_PROPERTY_NAMES,
};
// The id-level accessors only exist where the capability layer does; the alloc-frugal
// profile compiles them out, so the re-export carries the same gate.
#[cfg(widgets_unstripped)]
pub use capability::{read_widget_property_by_id, write_widget_property_by_id};
pub use draw::Draw;
// The canonical types live in `crate::image`; re-exported here so a caller that
// already has `crate::widget` in scope does not need the second path.
#[cfg(feature = "image")]
pub use crate::image::Image;
#[cfg(feature = "image")]
pub use crate::image::ImageFormat;
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
    otp_input::OtpInput,
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
// `Cascader` publishes a property contract and is built by the factory, so a
// stripped profile compiles it out — same gate as its module declaration.
#[cfg(full_widgets)]
pub use input_widgets::cascader::{Cascader, CascaderOption};
// `Mention` publishes a property contract and is built by the factory, so it needs the
// full property registry and not just an unstripped widget set. Its module is declared
// under `full_widgets`, and this export must carry the *same* gate: listing it in the
// `widgets_unstripped` group above left the re-export dangling in any profile that has
// `widgets_unstripped` without `full_widgets` (a `--features wasm` build, for one), which
// is a compile error rather than a missing widget.
#[cfg(full_widgets)]
pub use input_widgets::mention::{CompletedMention, Mention, MentionCandidate};
// `NumberPicker` needs the full property registry (it publishes a contract and is
// constructed by the factory), so a stripped profile compiles it out. The export
// carries the same gate as the module, not the looser `full_widgets` one.
#[cfg(widgets_unstripped)]
pub use input_widgets::number_picker::NumberPicker;
// Re-export container widgets
#[cfg(widgets_unstripped)]
pub use container_widgets::collapsible_pane::CollapsiblePane;
#[cfg(widgets_unstripped)]
pub use container_widgets::dockwidget::DockWidget;
pub use container_widgets::groupbox::GroupBox;
#[cfg(widgets_unstripped)]
pub use container_widgets::mdiarea::MdiArea;
pub use container_widgets::scrollarea::{ScrollArea, StickyRegion};
#[cfg(widgets_unstripped)]
pub use container_widgets::splitter::Splitter;
#[cfg(widgets_unstripped)]
pub use container_widgets::stackedwidget::StackedWidget;
#[cfg(widgets_unstripped)]
pub use container_widgets::tabwidget::TabWidget;
#[cfg(widgets_unstripped)]
pub use container_widgets::toolbox::ToolBox;
/// Alias for [`ToolBox`], matching the `WidgetKind::Toolbox` spelling.
#[cfg(widgets_unstripped)]
pub type Toolbox = ToolBox;
/// Alias for [`GroupBox`], for callers that name the container a "panel".
///
/// **The same Rust type as `GroupBox`, exposed under the `panel` factory name.**
/// `WidgetKind::Panel` is declared by several capabilities — this control, plus
/// `breadcrumb`, which is a `Panel`-kinded navigation trail — so a caller that wants
/// the plain container must ask for `panel` by name; see `constructed_as` in
/// `src/widget/capability/registration.rs`.
pub type Panel = GroupBox;
pub use base_widgets::frame::Frame;
/// Alias for [`DockWidget`], for callers that think of a dockable region as a
/// panel. Used only when widgets are not stripped.
#[cfg(widgets_unstripped)]
pub type DockPanel = DockWidget;
// Re-export container widgets from new additions
#[cfg(widgets_unstripped)]
pub use container_widgets::carousel::{
    Carousel, CarouselIndicatorPosition, CarouselIndicatorStyle, CarouselPage, WidgetAndDraw,
};
#[cfg(widgets_unstripped)]
pub use container_widgets::masonry_layout::{MasonryItem, MasonryLayout};
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
// `EmojiPicker` publishes a property contract and is built by the factory, so a
// stripped profile compiles it out — same gate as its module declaration.
#[cfg(full_widgets)]
pub use display_widgets::emoji_picker::{EmojiGlyph, EmojiPicker};
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
pub use nav_widgets::pagination::Pagination;
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
pub use overlay_widgets::banner::Banner;
#[cfg(full_widgets)]
pub use overlay_widgets::fab::FAB;
#[cfg(full_widgets)]
pub use overlay_widgets::refresh_control::RefreshControl;
#[cfg(full_widgets)]
pub use overlay_widgets::splash_screen::SplashScreen;
/// Alias for [`RefreshControl`], naming it after the gesture it implements.
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
pub use misc_widgets::drop_zone::DropZone;
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
    dialog_widget::Dialog,
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
/// Alias for [`FileDialog`], for callers that only ever select directories.
/// Selecting files is not prevented by this alias — it is unchecked.
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
/// Alias for [`Menu`], naming the role rather than the control.
/// Context menus are ordinary menus shown at the pointer; the difference is
/// in how the caller shows them, not in the type.
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
    filter_expr::{FilterCondition, FilterExpr, FilterOperator},
    grid_table::GridTableWidget,
    image_gallery::{GalleryImage, ImageGallery},
    list_view::{ListModel, ListView, VecListModel},
    properties_panel::{PropertiesPanel, PropertyEntry, PropertyValue},
    property_grid::{PropertyGrid, PropertyItem},
    query_builder::{FilterConjunction, FilterField, QueryBuilder, QueryBuilderRow},
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
    FreeformShapeWidget, GanttTask, GanttWidget, GridWidget, KanbanBoard, KanbanCard, KanbanColumn,
    MapMarker, MapView, MarkdownEditor, MarkerSeverity, MediaPlayer, NotificationCenter,
    NotificationItem, NotificationLevel, RadarChart, SegmentItem, SegmentedControl, SignaturePad,
    SignatureStroke, Snackbar, SplitAction, SplitButton, TerminalView, TimelineItem,
    TimelineWidget, Toast, ToastItem, ToastLevel, ToastStack,
};
/// Alias for [`ProgressBar`], naming an indicator use case.
/// This is a plain progress bar: it does not animate on its own.
#[cfg(full_widgets)]
pub type ActivityIndicator = ProgressBar;
/// Alias for [`ListBox`], naming the checklist use case.
/// Per-item checkboxes are **not** implied — this is a selectable list.
#[cfg(full_widgets)]
pub type CheckListBox = ListBox;
/// Alias for [`ToolBox`] declared next to the re-export above, so it remains
/// reachable on a device build whose widget set is stripped.
/// It is the same control; the alias only carries the `WidgetKind::Toolbox` spelling.
#[cfg(all(full_widgets, not(widgets_unstripped)))]
pub type Toolbox = ToolBox;
/// Alias for [`SpinBox`] intended for floating-point input.
/// It is the same integer spin box: no decimal support is added by the alias.
#[cfg(full_widgets)]
pub type DoubleSpinBox = SpinBox;
/// Alias for [`WizardDialog`]; the `WidgetKind::Wizard` spelling.
#[cfg(full_widgets)]
pub type Wizard = WizardDialog;
// ── P3-6: WidgetKind variant type aliases ──
//
// Each alias renames a concrete control after the `WidgetKind` variant it
// corresponds to. They are pure renames: none adds behaviour, and none is a
// distinct type from its target, so they can be used interchangeably.

/// Alias for [`VirtualList`]; the `WidgetKind::DataView` spelling.
#[cfg(full_widgets)]
pub type DataView = VirtualList;
/// Alias for [`TreeView`]; the `WidgetKind::ColumnView` spelling. Despite the
/// name, this is a tree, not a column layout.
#[cfg(full_widgets)]
pub type ColumnView = TreeView;
/// Alias for [`ListView`]; the `WidgetKind::UndoView` spelling. Undo has to be
/// wired up by the caller — this is not an undo-aware view.
#[cfg(full_widgets)]
pub type UndoView = ListView;
/// Alias for [`DateEdit`]; the `WidgetKind::DatePicker` spelling.
#[cfg(full_widgets)]
pub type DatePicker = DateEdit;
/// Alias for [`TimeEdit`]; the `WidgetKind::TimePicker` spelling.
#[cfg(full_widgets)]
pub type TimePicker = TimeEdit;
/// Alias for [`DateTimeEdit`]; the `WidgetKind::DateTimePicker` spelling.
#[cfg(full_widgets)]
pub type DateTimePicker = DateTimeEdit;
/// Alias for [`GridWidget`]; the `WidgetKind::Grid` spelling.
#[cfg(full_widgets)]
pub type Grid = GridWidget;
/// Alias for [`ChartWidget`]; the `WidgetKind::Chart` spelling.
#[cfg(full_widgets)]
pub type Chart = ChartWidget;
/// Alias for [`GridTableWidget`]; the `WidgetKind::GridTable` spelling.
#[cfg(full_widgets)]
pub type GridTable = GridTableWidget;
/// Alias for [`TableWidget`]; the `WidgetKind::Table` spelling.
#[cfg(full_widgets)]
pub type Table = TableWidget;
/// Alias for [`FreeformShapeWidget`]; the `WidgetKind::FreeformShape` spelling.
#[cfg(full_widgets)]
pub type FreeformShape = FreeformShapeWidget;
