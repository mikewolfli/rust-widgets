//! Generic read/write property dispatch for all widget kinds.
//!
//! [`read_widget_property_value`] and [`write_widget_property_value`] are the
//! two large match-on-`widget.kind()` functions that form the core of the
//! capability-based reflection layer. They downcast the `&dyn Widget` trait
//! object to the concrete widget type (via [`widget_as`] / [`widget_as_mut`])
//! and call the native getter or setter.
//!
//! These functions are called by `WidgetFactory::read_property` and
//! `WidgetFactory::write_property` after the property schema has been
//! validated — so the match arms here can assume the property exists and is
//! accessible.

#[cfg(not(any(feature = "mini", feature = "embedded")))]
use chrono::Weekday;

use crate::core::{Alignment, Orientation};
#[cfg(not(feature = "mini"))]
#[cfg(not(any(feature = "mini", feature = "embedded")))]
#[cfg(not(any(feature = "mini", feature = "embedded")))]
use crate::widget::advanced_widgets::calendar::Calendar;
#[cfg(not(any(feature = "mini", feature = "embedded")))]
use crate::widget::advanced_widgets::date_edit::Date;
#[cfg(not(any(feature = "mini", feature = "embedded")))]
use crate::widget::advanced_widgets::date_edit::DateEdit;
#[cfg(not(any(feature = "mini", feature = "embedded")))]
use crate::widget::advanced_widgets::date_time_edit::DateTimeEdit;
#[cfg(not(any(feature = "mini", feature = "embedded")))]
use crate::widget::advanced_widgets::dial::Dial;
#[cfg(not(any(feature = "mini", feature = "embedded")))]
use crate::widget::advanced_widgets::pie_menu::PieMenu;
#[cfg(not(any(feature = "mini", feature = "embedded")))]
use crate::widget::advanced_widgets::ribbon_bar::RibbonBar;
#[cfg(not(any(feature = "mini", feature = "embedded")))]
use crate::widget::advanced_widgets::tab_bar::TabBar;
#[cfg(not(any(feature = "mini", feature = "embedded")))]
use crate::widget::advanced_widgets::time_edit::Time;
#[cfg(not(any(feature = "mini", feature = "embedded")))]
use crate::widget::advanced_widgets::time_edit::TimeEdit;
use crate::widget::base_widgets::button::Button;
use crate::widget::base_widgets::checkbox::CheckBox;
use crate::widget::base_widgets::checkbox::CheckState;
use crate::widget::base_widgets::label::Label;
use crate::widget::base_widgets::radiobutton::RadioButton;
#[cfg(not(feature = "mini"))]
use crate::widget::base_widgets::toggle_button::{ToggleButton, ToggleButtonState};
use crate::widget::capability::coercion::*;
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
#[cfg(not(feature = "mini"))]
use crate::widget::chart_widgets::bar_chart::BarChart;
#[cfg(not(feature = "mini"))]
use crate::widget::chart_widgets::line_chart::LineChart;
#[cfg(not(feature = "mini"))]
use crate::widget::chart_widgets::pie_chart::PieChart;
#[cfg(not(feature = "mini"))]
use crate::widget::chart_widgets::sparkline::Sparkline;
#[cfg(not(feature = "mini"))]
use crate::widget::container_widgets::collapsible_pane::CollapsiblePane;
#[cfg(not(feature = "mini"))]
use crate::widget::container_widgets::dockwidget::DockWidget;
use crate::widget::container_widgets::groupbox::GroupBox;
#[cfg(not(feature = "mini"))]
use crate::widget::container_widgets::mdiarea::MdiArea;
#[cfg(not(feature = "mini"))]
use crate::widget::container_widgets::pager_page_view::PagerPageView;
use crate::widget::container_widgets::scrollarea::{ScrollArea, ScrollBarPolicy};
#[cfg(not(feature = "mini"))]
use crate::widget::container_widgets::splitter::Splitter;
#[cfg(not(feature = "mini"))]
use crate::widget::container_widgets::stackedwidget::StackedWidget;
#[cfg(not(feature = "mini"))]
use crate::widget::container_widgets::tabwidget::TabWidget;
use crate::widget::container_widgets::tile_view::TileView;
#[cfg(not(feature = "mini"))]
use crate::widget::container_widgets::toolbox::ToolBox;
#[cfg(not(feature = "mini"))]
use crate::widget::cupertino::core::CupertinoSlider;
#[cfg(not(feature = "mini"))]
use crate::widget::cupertino::core::MaterialNavigationRail;
#[cfg(not(feature = "mini"))]
use crate::widget::dialog::file_dialog::FileDialog;
#[cfg(not(feature = "mini"))]
use crate::widget::dialog::font_dialog::FontDialog;
#[cfg(not(feature = "mini"))]
use crate::widget::dialog::input_dialog::InputDialog;
#[cfg(not(feature = "mini"))]
use crate::widget::dialog::message_box::MessageBox;
#[cfg(not(feature = "mini"))]
use crate::widget::dialog::popup_window::PopupWindow;
#[cfg(not(feature = "mini"))]
use crate::widget::dialog::progress_dialog::ProgressDialog;
use crate::widget::display_widgets::arc::Arc;
use crate::widget::display_widgets::image_view::ImageView;
#[cfg(not(feature = "mini"))]
use crate::widget::display_widgets::lcd_number::{LCDNumber, LCDNumberMode, SegmentStyle};
use crate::widget::display_widgets::line::{Line, LineOrientation};
use crate::widget::display_widgets::meter::Meter;
use crate::widget::display_widgets::mini_chart::{ChartType, MiniChart};
use crate::widget::display_widgets::progressbar::ProgressBar;
use crate::widget::display_widgets::roller::Roller;
use crate::widget::display_widgets::scrollbar::ScrollBar;
use crate::widget::display_widgets::slider::{Slider, TickPosition};
use crate::widget::display_widgets::spinner::Spinner;
use crate::widget::display_widgets::switch::Switch;
use crate::widget::input_widgets::combobox::ComboBox;
#[cfg(not(feature = "mini"))]
use crate::widget::input_widgets::command_link::CommandLink;
use crate::widget::input_widgets::dropdown::Dropdown;
#[cfg(not(feature = "mini"))]
use crate::widget::input_widgets::font_combo_box::FontComboBox;
use crate::widget::input_widgets::keyboard::{Keyboard, KeyboardLayout};
use crate::widget::input_widgets::lineedit::LineEdit;
use crate::widget::input_widgets::listbox::ListBox;
use crate::widget::input_widgets::listbox::SelectionMode as ListBoxSelectionMode;
#[cfg(not(feature = "mini"))]
use crate::widget::input_widgets::search_bar::SearchBar;
#[cfg(not(feature = "mini"))]
use crate::widget::input_widgets::shortcut_editor::ShortcutEditor;
use crate::widget::input_widgets::spinbox::SpinBox;
use crate::widget::input_widgets::textarea::TextArea;
#[cfg(not(any(feature = "mini", feature = "embedded")))]
use crate::widget::media_widgets::animated_image::AnimatedImage;
#[cfg(not(any(feature = "mini", feature = "embedded")))]
use crate::widget::media_widgets::audio_visualizer::AudioVisualizer;
#[cfg(not(any(feature = "mini", feature = "embedded")))]
use crate::widget::media_widgets::camera_preview::CameraPreview;
#[cfg(not(any(feature = "mini", feature = "embedded")))]
use crate::widget::media_widgets::hero_animation::HeroAnimation;
#[cfg(not(any(feature = "mini", feature = "embedded")))]
use crate::widget::media_widgets::lottie_widget::LottieWidget;
#[cfg(not(any(feature = "mini", feature = "embedded")))]
use crate::widget::media_widgets::rive_widget::RiveWidget;
#[cfg(not(any(feature = "mini", feature = "embedded")))]
use crate::widget::media_widgets::video_player::VideoPlayer;
#[cfg(not(feature = "mini"))]
use crate::widget::menu_toolbar::action::Action;
#[cfg(not(feature = "mini"))]
use crate::widget::menu_toolbar::menu::Menu;
#[cfg(not(feature = "mini"))]
use crate::widget::menu_toolbar::menu_bar::MenuBar;
#[cfg(not(feature = "mini"))]
use crate::widget::menu_toolbar::status_bar::StatusBar;
#[cfg(not(feature = "mini"))]
use crate::widget::menu_toolbar::tool_bar::ToolBar;
#[cfg(not(feature = "mini"))]
use crate::widget::menu_toolbar::tool_bar::ToolBarOrientation;
#[cfg(not(feature = "mini"))]
use crate::widget::menu_toolbar::tool_button::ToolButton;
#[cfg(not(feature = "mini"))]
use crate::widget::misc_widgets::barcode_scanner::BarcodeScanner;
#[cfg(not(feature = "mini"))]
use crate::widget::misc_widgets::bezier_curve_editor::BezierCurveEditor;
#[cfg(not(feature = "mini"))]
use crate::widget::nav_widgets::tab_view::TabView;
#[cfg(not(feature = "mini"))]
use crate::widget::overlay_widgets::swipe_to_dismiss::SwipeToDismiss;
#[cfg(not(feature = "mini"))]
use crate::widget::special_widgets::breadcrumb::Breadcrumb;
#[cfg(not(feature = "mini"))]
use crate::widget::special_widgets::chip::Chip;
#[cfg(not(feature = "mini"))]
use crate::widget::special_widgets::code_editor::CodeEditor;
#[cfg(not(feature = "mini"))]
use crate::widget::special_widgets::color_picker::ColorPicker;
#[cfg(not(feature = "mini"))]
use crate::widget::special_widgets::freeform_shape::{FreeformShapeWidget, ShapePath};
#[cfg(not(feature = "mini"))]
use crate::widget::special_widgets::gantt_widget::GanttWidget;
#[cfg(not(feature = "mini"))]
use crate::widget::special_widgets::grid::GridWidget;
#[cfg(not(feature = "mini"))]
use crate::widget::special_widgets::map_view::MapView;
#[cfg(not(feature = "mini"))]
use crate::widget::special_widgets::media_player::MediaPlayer;
#[cfg(not(feature = "mini"))]
use crate::widget::special_widgets::terminal_view::TerminalView;
#[cfg(not(feature = "mini"))]
use crate::widget::view_widgets::data_grid::{ColumnFilter, DataGrid, SortSpec};
#[cfg(not(feature = "mini"))]
use crate::widget::view_widgets::image_gallery::ImageGallery;
#[cfg(not(feature = "mini"))]
use crate::widget::view_widgets::list_view::{ListView, SelectionMode, ViewMode};
#[cfg(not(feature = "mini"))]
use crate::widget::view_widgets::property_grid::PropertyGrid;
#[cfg(not(feature = "mini"))]
use crate::widget::view_widgets::table_widget::TableWidget;
#[cfg(not(feature = "mini"))]
use crate::widget::view_widgets::tree_table::TreeTable;
#[cfg(not(feature = "mini"))]
use crate::widget::view_widgets::tree_view::TreeView;
#[cfg(not(feature = "mini"))]
use crate::widget::view_widgets::virtual_list::VirtualList;
#[cfg(not(feature = "mini"))]
use crate::widget::view_widgets::virtual_table::VirtualTable;
#[cfg(not(feature = "mini"))]
use crate::widget::web_widgets::web_view::WebView;
use crate::widget::window::Window;
use crate::widget::{Widget, WidgetKind};

include!("access_read_base.in.rs");
include!("access_read_view.in.rs");
include!("access_read_container.in.rs");
include!("access_read_dialog.in.rs");
include!("access_read_menu.in.rs");
#[cfg(not(any(feature = "mini", feature = "embedded")))]
include!("access_read_input.in.rs");
#[cfg(not(any(feature = "mini", feature = "embedded")))]
include!("access_read_advanced.in.rs");
#[cfg(not(any(feature = "mini", feature = "embedded")))]
include!("access_read_media.in.rs");
include!("access_read_other.in.rs");

#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub fn read_widget_property_value(
    widget: &dyn Widget,
    property_name: &str,
) -> Result<CapabilityValue, CapabilityAccessError> {
    // Try each category; propagate the first non-Unsupported result (even if Err).
    let result = read_base_props(widget, property_name);
    if !matches!(result, Err(CapabilityAccessError::UnsupportedOnWidget)) {
        return result;
    }
    let result = read_input_props(widget, property_name);
    if !matches!(result, Err(CapabilityAccessError::UnsupportedOnWidget)) {
        return result;
    }
    let result = read_view_props(widget, property_name);
    if !matches!(result, Err(CapabilityAccessError::UnsupportedOnWidget)) {
        return result;
    }
    let result = read_container_props(widget, property_name);
    if !matches!(result, Err(CapabilityAccessError::UnsupportedOnWidget)) {
        return result;
    }
    let result = read_dialog_props(widget, property_name);
    if !matches!(result, Err(CapabilityAccessError::UnsupportedOnWidget)) {
        return result;
    }
    let result = read_menu_props(widget, property_name);
    if !matches!(result, Err(CapabilityAccessError::UnsupportedOnWidget)) {
        return result;
    }
    let result = read_advanced_props(widget, property_name);
    if !matches!(result, Err(CapabilityAccessError::UnsupportedOnWidget)) {
        return result;
    }
    let result = read_media_props(widget, property_name);
    if !matches!(result, Err(CapabilityAccessError::UnsupportedOnWidget)) {
        return result;
    }
    let result = read_other_props(widget, property_name);
    if !matches!(result, Err(CapabilityAccessError::UnsupportedOnWidget)) {
        return result;
    }
    result
}

include!("access_write_base.in.rs");
#[cfg(not(any(feature = "mini", feature = "embedded")))]
include!("access_write_input.in.rs");
include!("access_write_view.in.rs");
include!("access_write_container.in.rs");
include!("access_write_dialog.in.rs");
include!("access_write_menu.in.rs");
#[cfg(not(any(feature = "mini", feature = "embedded")))]
include!("access_write_advanced.in.rs");
#[cfg(not(any(feature = "mini", feature = "embedded")))]
include!("access_write_media.in.rs");
include!("access_write_other.in.rs");

#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub fn write_widget_property_value(
    widget: &mut dyn Widget,
    property_name: &str,
    value: CapabilityValue,
) -> Result<(), CapabilityAccessError> {
    // Try each category; propagate the first non-Unsupported result (even if Err).
    let result = write_base_props(widget, property_name, value.clone());
    if !matches!(result, Err(CapabilityAccessError::UnsupportedOnWidget)) {
        return result;
    }
    let result = write_input_props(widget, property_name, value.clone());
    if !matches!(result, Err(CapabilityAccessError::UnsupportedOnWidget)) {
        return result;
    }
    let result = write_view_props(widget, property_name, value.clone());
    if !matches!(result, Err(CapabilityAccessError::UnsupportedOnWidget)) {
        return result;
    }
    let result = write_container_props(widget, property_name, value.clone());
    if !matches!(result, Err(CapabilityAccessError::UnsupportedOnWidget)) {
        return result;
    }
    let result = write_dialog_props(widget, property_name, value.clone());
    if !matches!(result, Err(CapabilityAccessError::UnsupportedOnWidget)) {
        return result;
    }
    let result = write_menu_props(widget, property_name, value.clone());
    if !matches!(result, Err(CapabilityAccessError::UnsupportedOnWidget)) {
        return result;
    }
    let result = write_advanced_props(widget, property_name, value.clone());
    if !matches!(result, Err(CapabilityAccessError::UnsupportedOnWidget)) {
        return result;
    }
    let result = write_media_props(widget, property_name, value.clone());
    if !matches!(result, Err(CapabilityAccessError::UnsupportedOnWidget)) {
        return result;
    }
    let result = write_other_props(widget, property_name, value);
    if !matches!(result, Err(CapabilityAccessError::UnsupportedOnWidget)) {
        return result;
    }
    result
}

#[cfg(any(feature = "mini", feature = "embedded"))]
pub fn read_widget_property_value(
    _widget: &dyn Widget,
    _property_name: &str,
) -> Result<CapabilityValue, CapabilityAccessError> {
    Err(CapabilityAccessError::UnsupportedOnWidget)
}

#[cfg(any(feature = "mini", feature = "embedded"))]
pub fn write_widget_property_value(
    _widget: &mut dyn Widget,
    _property_name: &str,
    _value: CapabilityValue,
) -> Result<(), CapabilityAccessError> {
    Err(CapabilityAccessError::UnsupportedOnWidget)
}

// ---------------------------------------------------------------------------
// Helper to-str / to-string conversions
// ---------------------------------------------------------------------------

#[cfg(not(feature = "mini"))]
pub fn sort_specs_to_string(sort_specs: &[SortSpec]) -> String {
    sort_specs
        .iter()
        .map(|spec| format!("{}:{}", spec.column, if spec.descending { "desc" } else { "asc" }))
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(not(feature = "mini"))]
pub fn column_filters_to_string(filters: &[ColumnFilter]) -> String {
    filters
        .iter()
        .map(|filter| format!("{}={}", filter.column, filter.query))
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(not(feature = "mini"))]
pub fn selection_mode_to_str(mode: SelectionMode) -> &'static str {
    match mode {
        SelectionMode::Single => "single",
        SelectionMode::Multi => "multi",
        SelectionMode::Extended => "extended",
    }
}

pub fn list_box_selection_mode_to_str(mode: ListBoxSelectionMode) -> &'static str {
    match mode {
        ListBoxSelectionMode::NoSelection => "none",
        ListBoxSelectionMode::SingleSelection => "single",
        ListBoxSelectionMode::MultiSelection => "multi",
        ListBoxSelectionMode::ExtendedSelection => "extended",
    }
}

#[cfg(not(feature = "mini"))]
pub fn view_mode_to_str(mode: ViewMode) -> &'static str {
    match mode {
        ViewMode::List => "list",
        ViewMode::Icon => "icon",
        ViewMode::Details => "details",
        ViewMode::Thumbnails => "thumbnails",
    }
}

#[cfg(not(feature = "mini"))]
pub fn tool_bar_orientation_to_str(orientation: ToolBarOrientation) -> &'static str {
    match orientation {
        ToolBarOrientation::Horizontal => "horizontal",
        ToolBarOrientation::Vertical => "vertical",
    }
}

pub fn scroll_bar_policy_to_str(policy: ScrollBarPolicy) -> &'static str {
    match policy {
        ScrollBarPolicy::AlwaysOn => "always_on",
        ScrollBarPolicy::AlwaysOff => "always_off",
        ScrollBarPolicy::AsNeeded => "as_needed",
    }
}

pub fn alignment_to_str(alignment: Alignment) -> &'static str {
    match alignment {
        Alignment::Left => "left",
        Alignment::Center => "center",
        Alignment::Right => "right",
        Alignment::Top => "top",
        Alignment::Bottom => "bottom",
    }
}

pub fn check_state_to_str(state: CheckState) -> &'static str {
    match state {
        CheckState::Unchecked => "unchecked",
        CheckState::PartiallyChecked => "partially_checked",
        CheckState::Checked => "checked",
    }
}

pub fn orientation_to_str(orientation: Orientation) -> &'static str {
    match orientation {
        Orientation::Horizontal => "horizontal",
        Orientation::Vertical => "vertical",
    }
}

#[cfg(not(feature = "mini"))]
pub fn tick_position_to_str(tick_position: TickPosition) -> &'static str {
    match tick_position {
        TickPosition::NoTicks => "none",
        TickPosition::TicksAbove => "above",
        TickPosition::TicksBelow => "below",
        TickPosition::TicksBothSides => "both",
    }
}

#[cfg(not(feature = "mini"))]
pub fn lcd_mode_to_str(mode: LCDNumberMode) -> &'static str {
    match mode {
        LCDNumberMode::Hex => "hex",
        LCDNumberMode::Dec => "dec",
        LCDNumberMode::Oct => "oct",
        LCDNumberMode::Bin => "bin",
    }
}

#[cfg(not(feature = "mini"))]
pub fn segment_style_to_str(style: SegmentStyle) -> &'static str {
    match style {
        SegmentStyle::Outline => "outline",
        SegmentStyle::Filled => "filled",
        SegmentStyle::Flat => "flat",
    }
}

#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub fn weekday_to_str(weekday: Weekday) -> &'static str {
    match weekday {
        Weekday::Mon => "mon",
        Weekday::Tue => "tue",
        Weekday::Wed => "wed",
        Weekday::Thu => "thu",
        Weekday::Fri => "fri",
        Weekday::Sat => "sat",
        Weekday::Sun => "sun",
    }
}

#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub fn date_to_string(date: Date) -> String {
    date.to_string()
}

#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub fn time_to_string(time: Time) -> String {
    time.to_string()
}

// ---------------------------------------------------------------------------
// Default property value lookup
// ---------------------------------------------------------------------------

#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub fn default_widget_property_value(
    kind: WidgetKind,
    property_name: &str,
) -> Option<CapabilityValue> {
    let value = match kind {
        WidgetKind::Button => match property_name {
            "text" => CapabilityValue::String(String::new()),
            "pressed" => CapabilityValue::Bool(false),
            "default" => CapabilityValue::Bool(false),
            "enabled" => CapabilityValue::Bool(true),
            "tooltip" => CapabilityValue::String(String::new()),
            _ => return None,
        },
        WidgetKind::Label => match property_name {
            "text" => CapabilityValue::String(String::new()),
            "alignment" => CapabilityValue::String("left".to_string()),
            _ => return None,
        },
        WidgetKind::CheckBox => match property_name {
            "text" => CapabilityValue::String(String::new()),
            "state" => CapabilityValue::String("unchecked".to_string()),
            "checked" => CapabilityValue::Bool(false),
            "tristate_enabled" => CapabilityValue::Bool(false),
            _ => return None,
        },
        WidgetKind::RadioButton => match property_name {
            "text" => CapabilityValue::String(String::new()),
            "checked" => CapabilityValue::Bool(false),
            "group_id" => CapabilityValue::Null,
            _ => return None,
        },
        WidgetKind::Slider => match property_name {
            "minimum" => CapabilityValue::Int(0),
            "maximum" => CapabilityValue::Int(100),
            "value" => CapabilityValue::Int(0),
            "single_step" => CapabilityValue::Int(1),
            "page_step" => CapabilityValue::Int(10),
            "orientation" => CapabilityValue::String("horizontal".to_string()),
            "tick_position" => CapabilityValue::String("none".to_string()),
            "tick_interval" => CapabilityValue::Int(0),
            "tracking" => CapabilityValue::Bool(true),
            "slider_position" => CapabilityValue::Int(0),
            _ => return None,
        },
        WidgetKind::ProgressBar => match property_name {
            "minimum" => CapabilityValue::Int(0),
            "maximum" => CapabilityValue::Int(100),
            "value" => CapabilityValue::Int(0),
            "text_visible" => CapabilityValue::Bool(true),
            "orientation" => CapabilityValue::String("horizontal".to_string()),
            "inverted_appearance" => CapabilityValue::Bool(false),
            "progress" => CapabilityValue::Float(0.0),
            _ => return None,
        },
        WidgetKind::ScrollBar => match property_name {
            "minimum" => CapabilityValue::Int(0),
            "maximum" => CapabilityValue::Int(100),
            "value" => CapabilityValue::Int(0),
            "single_step" => CapabilityValue::Int(1),
            "page_step" => CapabilityValue::Int(10),
            "orientation" => CapabilityValue::String("horizontal".to_string()),
            "slider_size" => CapabilityValue::Float(0.1),
            "slider_position" => CapabilityValue::Float(0.0),
            _ => return None,
        },
        WidgetKind::ListBox => match property_name {
            "item_count" => CapabilityValue::UInt(0),
            "selection_mode" => CapabilityValue::String("single".to_string()),
            "current_row" => CapabilityValue::Null,
            "item_height" => CapabilityValue::Float(20.0),
            "selected_count" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::SpinBox => match property_name {
            "minimum" => CapabilityValue::Int(0),
            "maximum" => CapabilityValue::Int(99),
            "value" => CapabilityValue::Int(0),
            "single_step" => CapabilityValue::Int(1),
            "prefix" => CapabilityValue::String(String::new()),
            "suffix" => CapabilityValue::String(String::new()),
            "special_value_text" => CapabilityValue::Null,
            "wrapping" => CapabilityValue::Bool(false),
            _ => return None,
        },
        WidgetKind::ComboBox => match property_name {
            "item_count" => CapabilityValue::UInt(0),
            "current_index" => CapabilityValue::Null,
            "current_text" => CapabilityValue::String(String::new()),
            "editable" => CapabilityValue::Bool(false),
            "max_visible_items" => CapabilityValue::UInt(10),
            _ => return None,
        },
        WidgetKind::Dial => match property_name {
            "minimum" => CapabilityValue::Int(0),
            "maximum" => CapabilityValue::Int(99),
            "value" => CapabilityValue::Int(0),
            "single_step" => CapabilityValue::Int(1),
            "page_step" => CapabilityValue::Int(10),
            "notches_visible" => CapabilityValue::Bool(false),
            "notch_target" => CapabilityValue::Float(3.7),
            "wrapping" => CapabilityValue::Bool(false),
            _ => return None,
        },
        WidgetKind::Window => match property_name {
            "title" => CapabilityValue::String("Window".to_string()),
            "title_bar_height" => CapabilityValue::UInt(32),
            "close_button_size" => CapabilityValue::UInt(14),
            "button_spacing" => CapabilityValue::UInt(40),
            _ => return None,
        },
        WidgetKind::GroupBox => match property_name {
            "title" => CapabilityValue::String(String::new()),
            "alignment" => CapabilityValue::String("left".to_string()),
            "checkable" => CapabilityValue::Bool(false),
            "checked" => CapabilityValue::Bool(true),
            _ => return None,
        },
        WidgetKind::Splitter => match property_name {
            "orientation" => CapabilityValue::String("horizontal".to_string()),
            "pane_count" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::LCDNumber => match property_name {
            "value" => CapabilityValue::Float(0.0),
            "min_value" => CapabilityValue::Float(-999999.0),
            "max_value" => CapabilityValue::Float(999999.0),
            "num_digits" => CapabilityValue::Int(6),
            "small_decimal_point" => CapabilityValue::Bool(false),
            "mode" => CapabilityValue::String("dec".to_string()),
            "segment_style" => CapabilityValue::String("filled".to_string()),
            _ => return None,
        },
        WidgetKind::CommandLink => match property_name {
            "text" => CapabilityValue::String("Command".to_string()),
            "description" => CapabilityValue::String(String::new()),
            "enabled" => CapabilityValue::Bool(true),
            _ => return None,
        },
        WidgetKind::FontComboBox => match property_name {
            "current_font_family" => CapabilityValue::String("Arial".to_string()),
            "item_count" => CapabilityValue::Int(0),
            "current_index" => CapabilityValue::Int(-1),
            "editable" => CapabilityValue::Bool(false),
            "max_visible_items" => CapabilityValue::Int(10),
            _ => return None,
        },
        WidgetKind::Action => match property_name {
            "text" => CapabilityValue::String(String::new()),
            "icon_text" => CapabilityValue::String(String::new()),
            "shortcut" => CapabilityValue::String(String::new()),
            "checkable" => CapabilityValue::Bool(false),
            "checked" => CapabilityValue::Bool(false),
            "separator" => CapabilityValue::Bool(false),
            "command_id" => CapabilityValue::Null,
            _ => return None,
        },
        WidgetKind::Toolbox => match property_name {
            "item_count" => CapabilityValue::UInt(0),
            "current_index" => CapabilityValue::UInt(0),
            "orientation" => CapabilityValue::String("vertical".to_string()),
            _ => return None,
        },
        WidgetKind::TabBar => match property_name {
            "tab_count" => CapabilityValue::UInt(0),
            "current_index" => CapabilityValue::UInt(0),
            "closable" => CapabilityValue::Bool(false),
            "movable" => CapabilityValue::Bool(false),
            "tab_min_width" => CapabilityValue::UInt(40),
            "tab_max_width" => CapabilityValue::UInt(200),
            _ => return None,
        },
        WidgetKind::Calendar => match property_name {
            "selected_date" => {
                CapabilityValue::String(naive_date_to_string(chrono::Local::now().date_naive()))
            }
            "minimum_date" => CapabilityValue::String("1900-01-01".to_string()),
            "maximum_date" => CapabilityValue::String("3000-12-31".to_string()),
            "first_day_of_week" => CapabilityValue::String("mon".to_string()),
            "grid_visible" => CapabilityValue::Bool(true),
            "navigation_bar_visible" => CapabilityValue::Bool(true),
            "horizontal_header_visible" => CapabilityValue::Bool(true),
            "vertical_header_visible" => CapabilityValue::Bool(false),
            "date_format" => CapabilityValue::String("%Y-%m-%d".to_string()),
            _ => return None,
        },
        WidgetKind::DatePicker => match property_name {
            "date" => CapabilityValue::String("2024-01-01".to_string()),
            "minimum_date" => CapabilityValue::String("1752-09-14".to_string()),
            "maximum_date" => CapabilityValue::String("9999-12-31".to_string()),
            "display_format" => CapabilityValue::String("yyyy-MM-dd".to_string()),
            "calendar_popup" => CapabilityValue::Bool(false),
            _ => return None,
        },
        WidgetKind::TimePicker => match property_name {
            "time" => CapabilityValue::String("00:00:00".to_string()),
            "minimum_time" => CapabilityValue::String("00:00:00".to_string()),
            "maximum_time" => CapabilityValue::String("23:59:59".to_string()),
            "display_format" => CapabilityValue::String("HH:mm:ss".to_string()),
            _ => return None,
        },
        WidgetKind::LineEdit => match property_name {
            "text" => CapabilityValue::String(String::new()),
            "placeholder_text" => CapabilityValue::String(String::new()),
            "max_length" => CapabilityValue::Null,
            "read_only" => CapabilityValue::Bool(false),
            "cursor_position" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::ListView => match property_name {
            "has_model" => CapabilityValue::Bool(false),
            "row_count" => CapabilityValue::UInt(0),
            "focused_row" => CapabilityValue::Null,
            "selection_mode" => CapabilityValue::String("single".to_string()),
            "view_mode" => CapabilityValue::String("list".to_string()),
            _ => return None,
        },
        WidgetKind::TreeView => match property_name {
            "has_model" => CapabilityValue::Bool(false),
            "node_count" => CapabilityValue::UInt(0),
            "focused_node" => CapabilityValue::Null,
            "selected_node" => CapabilityValue::Null,
            "row_count" => CapabilityValue::UInt(0),
            "column_count" => CapabilityValue::UInt(0),
            "selected_row" => CapabilityValue::Null,
            "row_height" => CapabilityValue::UInt(20),
            "column_width" => CapabilityValue::UInt(140),
            "projection_state" => CapabilityValue::String("rows=0,selected=None".to_string()),
            _ => return None,
        },
        WidgetKind::Table => match property_name {
            "has_model" => CapabilityValue::Bool(false),
            "has_delegate" => CapabilityValue::Bool(false),
            "row_count" => CapabilityValue::UInt(0),
            "column_count" => CapabilityValue::UInt(0),
            "selection_mode" => CapabilityValue::String("single".to_string()),
            "has_data_source" => CapabilityValue::Bool(false),
            "scroll_row" => CapabilityValue::UInt(0),
            "scroll_column" => CapabilityValue::UInt(0),
            "row_height" => CapabilityValue::UInt(20),
            "column_width" => CapabilityValue::UInt(120),
            "overscan_rows" => CapabilityValue::UInt(2),
            "overscan_columns" => CapabilityValue::UInt(1),
            "frozen_columns" => CapabilityValue::UInt(0),
            "sort_spec_count" => CapabilityValue::UInt(0),
            "filter_count" => CapabilityValue::UInt(0),
            "sort_specs" => CapabilityValue::String(String::new()),
            "filters" => CapabilityValue::String(String::new()),
            "visible_window" => CapabilityValue::String("0:0:0:0".to_string()),
            _ => return None,
        },
        WidgetKind::DataView => match property_name {
            "has_data_source" => CapabilityValue::Bool(false),
            "row_count" => CapabilityValue::UInt(0),
            "scroll_row" => CapabilityValue::UInt(0),
            "row_height" => CapabilityValue::UInt(20),
            "overscan" => CapabilityValue::UInt(2),
            "selected_row" => CapabilityValue::Null,
            _ => return None,
        },
        WidgetKind::Menu => match property_name {
            "title" => CapabilityValue::String(String::new()),
            "item_count" => CapabilityValue::UInt(0),
            "hovered_index" => CapabilityValue::Null,
            _ => return None,
        },
        WidgetKind::MenuBar => match property_name {
            "entry_count" => CapabilityValue::UInt(0),
            "active_index" => CapabilityValue::Null,
            "hovered_index" => CapabilityValue::Null,
            _ => return None,
        },
        WidgetKind::ToolBar => match property_name {
            "orientation" => CapabilityValue::String("horizontal".to_string()),
            "icon_size" => CapabilityValue::Float(24.0),
            "movable" => CapabilityValue::Bool(true),
            "floatable" => CapabilityValue::Bool(true),
            "item_count" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::RibbonBar => match property_name {
            "tab_count" => CapabilityValue::UInt(0),
            "current_tab" => CapabilityValue::UInt(0),
            "expanded" => CapabilityValue::Bool(true),
            "minimized" => CapabilityValue::Bool(false),
            _ => return None,
        },
        WidgetKind::ColorDialog => match property_name {
            "hex_rgba" => CapabilityValue::String("#FF0000FF".to_string()),
            "show_alpha" => CapabilityValue::Bool(true),
            "preset_count" => CapabilityValue::UInt(6),
            _ => return None,
        },
        WidgetKind::RichEdit => match property_name {
            "text" => CapabilityValue::String(String::new()),
            "line_count" => CapabilityValue::UInt(0),
            "cursor_line" => CapabilityValue::UInt(0),
            "cursor_column" => CapabilityValue::UInt(0),
            "marker_count" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::Chart => match property_name {
            "task_count" => CapabilityValue::UInt(0),
            "selected_id" => CapabilityValue::Null,
            "selected_marker_id" => CapabilityValue::Null,
            "viewport_start" => CapabilityValue::Int(0),
            "viewport_end" => CapabilityValue::Int(100),
            _ => return None,
        },
        WidgetKind::TextEdit => match property_name {
            "text" => CapabilityValue::String(String::new()),
            "placeholder_text" => CapabilityValue::String(String::new()),
            "max_length" => CapabilityValue::Null,
            "read_only" => CapabilityValue::Bool(false),
            "line_wrap" => CapabilityValue::Bool(true),
            "output_line_count" => CapabilityValue::UInt(0),
            "input_line" => CapabilityValue::String(String::new()),
            _ => return None,
        },

        WidgetKind::Canvas => match property_name {
            "center_x" => CapabilityValue::Float(0.0),
            "center_y" => CapabilityValue::Float(0.0),
            "zoom" => CapabilityValue::Float(1.0),
            "marker_count" => CapabilityValue::UInt(0),
            "selected_marker_id" => CapabilityValue::Null,
            _ => return None,
        },
        WidgetKind::Carousel => match property_name {
            "page_count" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::WebEngineView => match property_name {
            "url" => CapabilityValue::String("about:blank".to_string()),
            "loading" => CapabilityValue::Bool(false),
            "title" => CapabilityValue::String(String::new()),
            "can_go_back" => CapabilityValue::Bool(false),
            "can_go_forward" => CapabilityValue::Bool(false),
            "source" => CapabilityValue::Null,
            "playing" => CapabilityValue::Bool(false),
            "duration_ms" => CapabilityValue::UInt(0),
            "position_ms" => CapabilityValue::UInt(0),
            "volume" => CapabilityValue::UInt(80),
            "muted" => CapabilityValue::Bool(false),
            "fullscreen" => CapabilityValue::Bool(false),
            _ => return None,
        },

        WidgetKind::Panel | WidgetKind::Frame => match property_name {
            "segment_count" => CapabilityValue::UInt(0),
            "selected_index" => CapabilityValue::Null,
            _ => return None,
        },

        WidgetKind::ToggleButton => match property_name {
            "text" => CapabilityValue::String(String::new()),
            "checked" => CapabilityValue::Bool(false),
            "state" => CapabilityValue::String("normal".to_string()),
            "item_count" => CapabilityValue::UInt(0),
            "selected_index" => CapabilityValue::Null,
            "selected_id" => CapabilityValue::Null,
            _ => return None,
        },
        WidgetKind::CheckListBox => match property_name {
            "item_count" => CapabilityValue::UInt(0),
            "multi_select" => CapabilityValue::Bool(false),
            "focused_index" => CapabilityValue::Null,
            "selected_count" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::Grid => match property_name {
            "rows" => CapabilityValue::UInt(1),
            "columns" => CapabilityValue::UInt(1),
            "spacing" => CapabilityValue::UInt(0),
            "line_color" => CapabilityValue::String("#DCDCDCFF".to_string()),
            "cell_width" => CapabilityValue::UInt(0),
            "cell_height" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::FreeformShape => match property_name {
            "path_kind" => CapabilityValue::String("rounded_rect".to_string()),
            "fill_rgba" => CapabilityValue::String("#C8DCFFFF".to_string()),
            "stroke_rgba" => CapabilityValue::String("#5078C8FF".to_string()),
            "stroke_width" => CapabilityValue::UInt(2),
            _ => return None,
        },
        // ── Always-available widget defaults (not mini-gated) ─────
        WidgetKind::Arc => match property_name {
            "value" => CapabilityValue::UInt(0),
            "minimum" => CapabilityValue::UInt(0),
            "maximum" => CapabilityValue::UInt(100),
            "thickness" => CapabilityValue::UInt(10),
            "sweep_angle" => CapabilityValue::UInt(360),
            "indeterminate" => CapabilityValue::Bool(false),
            _ => return None,
        },
        WidgetKind::Spinner => match property_name {
            "active" => CapabilityValue::Bool(true),
            "thickness" => CapabilityValue::UInt(4),
            "speed" => CapabilityValue::Float(1.0),
            "size_ratio" => CapabilityValue::Float(0.8),
            _ => return None,
        },
        WidgetKind::Roller => match property_name {
            "selected_index" => CapabilityValue::UInt(0),
            "visible_count" => CapabilityValue::UInt(5),
            "item_count" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::Dropdown => match property_name {
            "text" => CapabilityValue::String(String::new()),
            "selected_index" => CapabilityValue::UInt(0),
            "item_count" => CapabilityValue::UInt(0),
            "expanded" => CapabilityValue::Bool(false),
            _ => return None,
        },
        WidgetKind::TextArea => match property_name {
            "text" => CapabilityValue::String(String::new()),
            "placeholder" => CapabilityValue::String(String::new()),
            "read_only" => CapabilityValue::Bool(false),
            _ => return None,
        },
        WidgetKind::Keyboard => match property_name {
            "layout" => CapabilityValue::String("qwerty".to_string()),
            "lowercase" => CapabilityValue::Bool(true),
            _ => return None,
        },
        WidgetKind::Switch => match property_name {
            "checked" => CapabilityValue::Bool(false),
            _ => return None,
        },
        WidgetKind::Line => match property_name {
            "orientation" => CapabilityValue::String("horizontal".to_string()),
            _ => return None,
        },
        WidgetKind::Meter => match property_name {
            "value" => CapabilityValue::UInt(0),
            "minimum" => CapabilityValue::UInt(0),
            "maximum" => CapabilityValue::UInt(100),
            _ => return None,
        },
        WidgetKind::MiniChart => match property_name {
            "chart_type" => CapabilityValue::String("line".to_string()),
            _ => return None,
        },
        WidgetKind::ImageView => match property_name {
            "scaled" => CapabilityValue::Bool(false),
            _ => return None,
        },
        WidgetKind::TileView => match property_name {
            "current_page" => CapabilityValue::UInt(0),
            "page_count" => CapabilityValue::UInt(1),
            _ => return None,
        },
        // ── Dialog widgets ──────────────────────────────────
        WidgetKind::MessageBox => match property_name {
            "title" => CapabilityValue::String(String::new()),
            "text" => CapabilityValue::String(String::new()),
            "modal" => CapabilityValue::Bool(true),
            _ => return None,
        },
        WidgetKind::FileDialog => match property_name {
            "title" => CapabilityValue::String("Open File".to_string()),
            "modal" => CapabilityValue::Bool(true),
            "directory" => CapabilityValue::String(String::new()),
            "selected_file" => CapabilityValue::Null,
            "mode" => CapabilityValue::String("open_file".to_string()),
            _ => return None,
        },
        WidgetKind::FontDialog => match property_name {
            "modal" => CapabilityValue::Bool(true),
            _ => return None,
        },
        WidgetKind::InputDialog => match property_name {
            "title" => CapabilityValue::String(String::new()),
            "label_text" => CapabilityValue::String(String::new()),
            "mode" => CapabilityValue::String("text".to_string()),
            "text_value" => CapabilityValue::String(String::new()),
            "int_value" => CapabilityValue::Int(0),
            "double_value" => CapabilityValue::Float(0.0),
            _ => return None,
        },
        WidgetKind::ProgressDialog => match property_name {
            "title" => CapabilityValue::String(String::new()),
            "label_text" => CapabilityValue::String(String::new()),
            "value" => CapabilityValue::Int(0),
            "minimum" => CapabilityValue::Int(0),
            "maximum" => CapabilityValue::Int(100),
            _ => return None,
        },
        WidgetKind::PopupWindow => match property_name {
            "has_content" => CapabilityValue::Bool(false),
            _ => return None,
        },
        // ── Container widgets ───────────────────────────────
        WidgetKind::ScrollArea => match property_name {
            "widget_resizable" => CapabilityValue::Bool(true),
            "horizontal_scroll_bar_policy" => CapabilityValue::String("as_needed".to_string()),
            "vertical_scroll_bar_policy" => CapabilityValue::String("as_needed".to_string()),
            "scroll_position_x" => CapabilityValue::Int(0),
            "scroll_position_y" => CapabilityValue::Int(0),
            _ => return None,
        },
        WidgetKind::TabWidget => match property_name {
            "tab_count" => CapabilityValue::UInt(0),
            "current_index" => CapabilityValue::UInt(0),
            "closable" => CapabilityValue::Bool(false),
            "movable" => CapabilityValue::Bool(false),
            "tab_position" => CapabilityValue::String("north".to_string()),
            _ => return None,
        },
        WidgetKind::StackedWidget => match property_name {
            "widget_count" => CapabilityValue::UInt(0),
            "current_index" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::CollapsiblePane => match property_name {
            "title" => CapabilityValue::String(String::new()),
            "collapsed" => CapabilityValue::Bool(false),
            _ => return None,
        },
        WidgetKind::DockWidget => match property_name {
            "title" => CapabilityValue::String(String::new()),
            "floating" => CapabilityValue::Bool(false),
            "docked" => CapabilityValue::Bool(true),
            _ => return None,
        },
        WidgetKind::MdiArea => match property_name {
            "subwindow_count" => CapabilityValue::UInt(0),
            "active_subwindow" => CapabilityValue::Null,
            "view_mode" => CapabilityValue::String("sub_window_view".to_string()),
            _ => return None,
        },

        // ── Advanced widgets ────────────────────────────────
        WidgetKind::PieMenu => match property_name {
            "item_count" => CapabilityValue::UInt(0),
            "radius" => CapabilityValue::Float(100.0),
            "inner_radius" => CapabilityValue::Float(35.0),
            "current_index" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::DateTimePicker => match property_name {
            "datetime" => CapabilityValue::Null,
            "display_format" => CapabilityValue::String("yyyy-MM-dd HH:mm:ss".to_string()),
            "calendar_popup" => CapabilityValue::Bool(false),
            "minimum" => CapabilityValue::Null,
            "maximum" => CapabilityValue::Null,
            _ => return None,
        },
        // ── Group A widgets (non-mini) ─────────────────────
        WidgetKind::SearchBox => match property_name {
            "text" => CapabilityValue::String(String::new()),
            "placeholder" => CapabilityValue::String("Search...".to_string()),
            _ => return None,
        },
        WidgetKind::Badge => match property_name {
            "text" => CapabilityValue::String(String::new()),
            "count" => CapabilityValue::Int(0),
            _ => return None,
        },
        WidgetKind::SkeletonLoader => match property_name {
            "active" => CapabilityValue::Bool(true),
            _ => return None,
        },
        WidgetKind::FAB => match property_name {
            "icon" => CapabilityValue::String("+".to_string()),
            _ => return None,
        },
        WidgetKind::BottomSheet => match property_name {
            "expanded" => CapabilityValue::Bool(false),
            "peek_height" => CapabilityValue::Float(100.0),
            _ => return None,
        },
        WidgetKind::BottomNavigationBar => match property_name {
            "item_count" => CapabilityValue::UInt(0),
            "selected_index" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::NavigationDrawer => match property_name {
            "open" => CapabilityValue::Bool(false),
            "width" => CapabilityValue::Float(250.0),
            _ => return None,
        },
        WidgetKind::AppBar => match property_name {
            "title" => CapabilityValue::String(String::new()),
            _ => return None,
        },
        WidgetKind::MobileDatePicker => match property_name {
            "selected_date" => CapabilityValue::String(String::new()),
            _ => return None,
        },
        WidgetKind::Divider => match property_name {
            "orientation" => CapabilityValue::String("horizontal".to_string()),
            "thickness" => CapabilityValue::Float(1.0),
            _ => return None,
        },
        WidgetKind::Stepper => match property_name {
            "value" => CapabilityValue::Int(0),
            "minimum" => CapabilityValue::Int(0),
            "maximum" => CapabilityValue::Int(100),
            "step" => CapabilityValue::Int(1),
            _ => return None,
        },
        WidgetKind::Rating => match property_name {
            "value" => CapabilityValue::Float(0.0),
            "max" => CapabilityValue::UInt(5),
            _ => return None,
        },
        WidgetKind::Avatar => match property_name {
            "initials" => CapabilityValue::String(String::new()),
            "image_source" => CapabilityValue::String(String::new()),
            _ => return None,
        },
        WidgetKind::EmptyState => match property_name {
            "message" => CapabilityValue::String("No data".to_string()),
            "description" => CapabilityValue::String(String::new()),
            _ => return None,
        },
        WidgetKind::ColorHistory => match property_name {
            "color_count" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::ColorWell => match property_name {
            "color" => CapabilityValue::String("#FF0000FF".to_string()),
            _ => return None,
        },
        WidgetKind::TagInput => match property_name {
            "tags" => CapabilityValue::String(String::new()),
            "placeholder" => CapabilityValue::String("Add tag...".to_string()),
            _ => return None,
        },
        WidgetKind::ImePreedit => match property_name {
            "text" => CapabilityValue::String(String::new()),
            "cursor_position" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::InplaceEditor => match property_name {
            "text" => CapabilityValue::String(String::new()),
            "editing" => CapabilityValue::Bool(false),
            _ => return None,
        },
        WidgetKind::QRCode => match property_name {
            "data" => CapabilityValue::String(String::new()),
            "size" => CapabilityValue::UInt(256),
            _ => return None,
        },
        WidgetKind::MasonryLayout => match property_name {
            "column_count" => CapabilityValue::UInt(2),
            "item_count" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::MaterialSnackbar => match property_name {
            "message" => CapabilityValue::String(String::new()),
            "action_text" => CapabilityValue::String(String::new()),
            _ => return None,
        },
        WidgetKind::AdaptiveScaffold => match property_name {
            "title" => CapabilityValue::String(String::new()),
            _ => return None,
        },
        WidgetKind::WizardDialog => match property_name {
            "current_step" => CapabilityValue::UInt(0),
            "step_count" => CapabilityValue::UInt(0),
            "can_go_back" => CapabilityValue::Bool(false),
            "can_go_forward" => CapabilityValue::Bool(true),
            _ => return None,
        },
        WidgetKind::SafeArea => match property_name {
            "top_inset" => CapabilityValue::Float(0.0),
            "bottom_inset" => CapabilityValue::Float(0.0),
            "left_inset" => CapabilityValue::Float(0.0),
            "right_inset" => CapabilityValue::Float(0.0),
            _ => return None,
        },
        WidgetKind::CupertinoAlertDialog => match property_name {
            "title" => CapabilityValue::String(String::new()),
            "message" => CapabilityValue::String(String::new()),
            _ => return None,
        },
        WidgetKind::CupertinoSlider => match property_name {
            "value" => CapabilityValue::Float(0.0),
            "min" => CapabilityValue::Float(0.0),
            "max" => CapabilityValue::Float(1.0),
            _ => return None,
        },
        WidgetKind::Tooltip => match property_name {
            "text" => CapabilityValue::String(String::new()),
            "visible" => CapabilityValue::Bool(false),
            _ => return None,
        },
        WidgetKind::SegmentedButton => match property_name {
            "selected_index" => CapabilityValue::UInt(0),
            "segment_count" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::NavigationStack => match property_name {
            "page_count" => CapabilityValue::UInt(0),
            "current_page" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::ProgressCircle => match property_name {
            "value" => CapabilityValue::Float(0.0),
            "thickness" => CapabilityValue::Float(4.0),
            "indeterminate" => CapabilityValue::Bool(false),
            _ => return None,
        },
        WidgetKind::Icon => match property_name {
            "icon_name" => CapabilityValue::String(String::new()),
            "size" => CapabilityValue::Float(24.0),
            _ => return None,
        },
        WidgetKind::DropdownMenu => match property_name {
            "item_count" => CapabilityValue::UInt(0),
            "selected_index" => CapabilityValue::UInt(0),
            "expanded" => CapabilityValue::Bool(false),
            _ => return None,
        },
        WidgetKind::MaskedEdit => match property_name {
            "text" => CapabilityValue::String(String::new()),
            "mask" => CapabilityValue::String(String::new()),
            _ => return None,
        },
        WidgetKind::MenuButton => match property_name {
            "text" => CapabilityValue::String("Menu".to_string()),
            "item_count" => CapabilityValue::UInt(0),
            "expanded" => CapabilityValue::Bool(false),
            _ => return None,
        },
        WidgetKind::Popover => match property_name {
            "visible" => CapabilityValue::Bool(false),
            "text" => CapabilityValue::String(String::new()),
            _ => return None,
        },
        WidgetKind::AutoCompleteEdit => match property_name {
            "text" => CapabilityValue::String(String::new()),
            "suggestion_count" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::MultiSelectComboBox => match property_name {
            "selected_count" => CapabilityValue::UInt(0),
            "expanded" => CapabilityValue::Bool(false),
            _ => return None,
        },
        WidgetKind::RangeSlider => match property_name {
            "min_value" => CapabilityValue::Float(0.0),
            "max_value" => CapabilityValue::Float(100.0),
            "lower" => CapabilityValue::Float(25.0),
            "upper" => CapabilityValue::Float(75.0),
            _ => return None,
        },
        WidgetKind::FloatingLabel => match property_name {
            "text" => CapabilityValue::String(String::new()),
            "placeholder" => CapabilityValue::String(String::new()),
            "focused" => CapabilityValue::Bool(false),
            _ => return None,
        },
        WidgetKind::FontPreview => match property_name {
            "font_family" => CapabilityValue::String("Arial".to_string()),
            "font_size" => CapabilityValue::Float(16.0),
            "preview_text" => CapabilityValue::String("The quick brown fox...".to_string()),
            _ => return None,
        },
        WidgetKind::CupertinoNavigationBar => match property_name {
            "title" => CapabilityValue::String(String::new()),
            "large_title" => CapabilityValue::Bool(true),
            _ => return None,
        },
        WidgetKind::CupertinoSegmentedControl => match property_name {
            "selected_index" => CapabilityValue::UInt(0),
            "segment_count" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::RefreshControl => match property_name {
            "refreshing" => CapabilityValue::Bool(false),
            _ => return None,
        },
        WidgetKind::ModalBottomSheet => match property_name {
            "visible" => CapabilityValue::Bool(false),
            _ => return None,
        },
        WidgetKind::FindReplaceDialog => match property_name {
            "find_text" => CapabilityValue::String(String::new()),
            "replace_text" => CapabilityValue::String(String::new()),
            "match_case" => CapabilityValue::Bool(false),
            "wrap_around" => CapabilityValue::Bool(true),
            _ => return None,
        },
        WidgetKind::PropertiesPanel => match property_name {
            "property_count" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::CupertinoDatePicker => match property_name {
            "selected_date" => CapabilityValue::String("2025-01-01".to_string()),
            _ => return None,
        },
        WidgetKind::EditableComboBox => match property_name {
            "text" => CapabilityValue::String(String::new()),
            "item_count" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::DateRangePicker => match property_name {
            "start_date" => CapabilityValue::String(String::new()),
            "end_date" => CapabilityValue::String(String::new()),
            _ => return None,
        },

        // ── New widgets (menu/toolbar) ───────────────────────────
        WidgetKind::ToolButton => match property_name {
            "text" => CapabilityValue::String(String::new()),
            "checked" => CapabilityValue::Bool(false),
            "menu_open" => CapabilityValue::Bool(false),
            "action_count" => CapabilityValue::UInt(0),
            "row_height" => CapabilityValue::UInt(24),
            _ => return None,
        },
        WidgetKind::StatusBar => match property_name {
            "message" => CapabilityValue::String(String::new()),
            "visible" => CapabilityValue::Bool(false),
            "action_label" => CapabilityValue::Null,
            _ => return None,
        },

        // ── New widgets (input) ────────────────────────────────────
        WidgetKind::SearchBar => match property_name {
            "text" => CapabilityValue::String(String::new()),
            "placeholder" => CapabilityValue::String("Search...".to_string()),
            _ => return None,
        },
        WidgetKind::ShortcutEditor => match property_name {
            "filter_text" => CapabilityValue::String(String::new()),
            _ => return None,
        },

        // ── New widgets (navigation) ───────────────────────────────
        WidgetKind::TabView => match property_name {
            "selected_index" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::MaterialNavigationRail => match property_name {
            "selected_index" => CapabilityValue::UInt(0),
            _ => return None,
        },

        // ── New widgets (container) ─────────────────────────────────
        WidgetKind::PagerPageView => match property_name {
            "current_page" => CapabilityValue::UInt(0),
            _ => return None,
        },

        // ── New widgets (overlay) ───────────────────────────────────
        WidgetKind::SwipeToDismiss => match property_name {
            "is_dismissed" => CapabilityValue::Bool(false),
            _ => return None,
        },

        // ── New widgets (chart) ─────────────────────────────────────
        WidgetKind::LineChart => match property_name {
            "stroke_width" => CapabilityValue::Float(2.0),
            _ => return None,
        },
        WidgetKind::Sparkline => match property_name {
            "stroke_width" => CapabilityValue::Float(1.5),
            _ => return None,
        },
        WidgetKind::BarChart => match property_name {
            "bar_spacing" => CapabilityValue::Float(0.2),
            _ => return None,
        },
        WidgetKind::PieChart => match property_name {
            "donut" => CapabilityValue::Bool(false),
            _ => return None,
        },

        // ── New widgets (media/animation) ───────────────────────────
        WidgetKind::AnimatedImage => match property_name {
            "playing" => CapabilityValue::Bool(false),
            _ => return None,
        },
        WidgetKind::HeroAnimation => match property_name {
            "animation_progress" => CapabilityValue::Float(0.0),
            _ => return None,
        },
        WidgetKind::LottieWidget => match property_name {
            "playing" => CapabilityValue::Bool(false),
            _ => return None,
        },
        WidgetKind::RiveWidget => match property_name {
            "is_playing" => CapabilityValue::Bool(false),
            _ => return None,
        },
        WidgetKind::VideoPlayer => match property_name {
            "is_playing" => CapabilityValue::Bool(false),
            "volume" => CapabilityValue::Float(0.8),
            _ => return None,
        },

        // ── New widgets (view) ──────────────────────────────────────
        WidgetKind::ImageGallery => match property_name {
            "current_index" => CapabilityValue::UInt(0),
            _ => return None,
        },

        // ── New widgets (view / property) ───────────────────────────
        WidgetKind::PropertyGrid => match property_name {
            "property_count" => CapabilityValue::UInt(0),
            "selected_index" => CapabilityValue::Null,
            _ => return None,
        },

        // ── New widgets (misc) ──────────────────────────────────────
        WidgetKind::AudioVisualizer => match property_name {
            "bar_count" => CapabilityValue::UInt(64),
            _ => return None,
        },
        WidgetKind::CameraPreview => match property_name {
            "is_active" => CapabilityValue::Bool(false),
            _ => return None,
        },
        WidgetKind::BarcodeScanner => match property_name {
            "is_scanning" => CapabilityValue::Bool(false),
            _ => return None,
        },
        WidgetKind::BezierCurveEditor => match property_name {
            "snap_to_grid" => CapabilityValue::Bool(false),
            _ => return None,
        },

        _ => return None,
    };

    Some(value)
}

#[cfg(any(feature = "mini", feature = "embedded"))]
pub fn default_widget_property_value(
    _kind: WidgetKind,
    _property_name: &str,
) -> Option<CapabilityValue> {
    None
}
