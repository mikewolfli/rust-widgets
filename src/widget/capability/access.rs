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

use chrono::Weekday;

use crate::core::{Alignment, Orientation};
#[cfg(not(feature = "mini"))]
use crate::widget::advanced_widgets::calendar::Calendar;
#[cfg(not(feature = "mini"))]
use crate::widget::advanced_widgets::date_edit::Date;
#[cfg(not(feature = "mini"))]
use crate::widget::advanced_widgets::date_edit::DateEdit;
#[cfg(not(feature = "mini"))]
use crate::widget::advanced_widgets::date_time_edit::DateTimeEdit;
#[cfg(not(feature = "mini"))]
use crate::widget::advanced_widgets::dial::Dial;
#[cfg(not(feature = "mini"))]
use crate::widget::advanced_widgets::pie_menu::PieMenu;
#[cfg(not(feature = "mini"))]
use crate::widget::advanced_widgets::ribbon_bar::RibbonBar;
#[cfg(not(feature = "mini"))]
use crate::widget::advanced_widgets::tab_bar::TabBar;
#[cfg(not(feature = "mini"))]
use crate::widget::advanced_widgets::time_edit::Time;
#[cfg(not(feature = "mini"))]
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
#[cfg(not(feature = "mini"))]
use crate::widget::media_widgets::animated_image::AnimatedImage;
#[cfg(not(feature = "mini"))]
use crate::widget::media_widgets::audio_visualizer::AudioVisualizer;
#[cfg(not(feature = "mini"))]
use crate::widget::media_widgets::camera_preview::CameraPreview;
#[cfg(not(feature = "mini"))]
use crate::widget::media_widgets::hero_animation::HeroAnimation;
#[cfg(not(feature = "mini"))]
use crate::widget::media_widgets::lottie_widget::LottieWidget;
#[cfg(not(feature = "mini"))]
use crate::widget::media_widgets::rive_widget::RiveWidget;
#[cfg(not(feature = "mini"))]
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

#[cfg(not(feature = "mini"))]
pub fn read_widget_property_value(
    widget: &dyn Widget,
    property_name: &str,
) -> Result<CapabilityValue, CapabilityAccessError> {
    match widget.kind() {
        WidgetKind::Button => match property_name {
            "text" => {
                if let Some(button) = widget_as::<Button>(widget) {
                    Ok(CapabilityValue::String(button.text().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "pressed" => {
                if let Some(button) = widget_as::<Button>(widget) {
                    Ok(CapabilityValue::Bool(button.is_pressed()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "default" => {
                if let Some(button) = widget_as::<Button>(widget) {
                    Ok(CapabilityValue::Bool(button.is_default()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "enabled" => {
                if let Some(button) = widget_as::<Button>(widget) {
                    Ok(CapabilityValue::Bool(button.is_enabled()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "tooltip" => {
                if let Some(button) = widget_as::<Button>(widget) {
                    Ok(CapabilityValue::String(button.tooltip().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::Label => match property_name {
            "text" => {
                if let Some(label) = widget_as::<Label>(widget) {
                    Ok(CapabilityValue::String(label.text().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "alignment" => {
                if let Some(label) = widget_as::<Label>(widget) {
                    Ok(CapabilityValue::String(alignment_to_str(label.alignment()).to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::CheckBox => match property_name {
            "text" => {
                if let Some(cb) = widget_as::<CheckBox>(widget) {
                    Ok(CapabilityValue::String(cb.text().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "state" => {
                if let Some(cb) = widget_as::<CheckBox>(widget) {
                    Ok(CapabilityValue::String(check_state_to_str(cb.state()).to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "checked" => {
                if let Some(cb) = widget_as::<CheckBox>(widget) {
                    Ok(CapabilityValue::Bool(cb.is_checked()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "tristate_enabled" => {
                if let Some(cb) = widget_as::<CheckBox>(widget) {
                    Ok(CapabilityValue::Bool(cb.is_tristate_enabled()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::RadioButton => match property_name {
            "text" => {
                if let Some(rb) = widget_as::<RadioButton>(widget) {
                    Ok(CapabilityValue::String(rb.text().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "checked" => {
                if let Some(rb) = widget_as::<RadioButton>(widget) {
                    Ok(CapabilityValue::Bool(rb.is_checked()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "group_id" => {
                if let Some(rb) = widget_as::<RadioButton>(widget) {
                    match rb.group_id() {
                        Some(id) => Ok(CapabilityValue::String(id.to_string())),
                        None => Ok(CapabilityValue::Null),
                    }
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::Slider => match property_name {
            "minimum" => {
                if let Some(slider) = widget_as::<Slider>(widget) {
                    Ok(CapabilityValue::Int(slider.minimum() as i64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "maximum" => {
                if let Some(slider) = widget_as::<Slider>(widget) {
                    Ok(CapabilityValue::Int(slider.maximum() as i64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "value" => {
                if let Some(slider) = widget_as::<Slider>(widget) {
                    Ok(CapabilityValue::Int(slider.value() as i64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "single_step" => {
                if let Some(slider) = widget_as::<Slider>(widget) {
                    Ok(CapabilityValue::Int(slider.single_step() as i64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "page_step" => {
                if let Some(slider) = widget_as::<Slider>(widget) {
                    Ok(CapabilityValue::Int(slider.page_step() as i64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "orientation" => {
                if let Some(slider) = widget_as::<Slider>(widget) {
                    Ok(CapabilityValue::String(
                        orientation_to_str(slider.orientation()).to_string(),
                    ))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "tick_position" => {
                if let Some(slider) = widget_as::<Slider>(widget) {
                    Ok(CapabilityValue::String(
                        tick_position_to_str(slider.tick_position()).to_string(),
                    ))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "tick_interval" => {
                if let Some(slider) = widget_as::<Slider>(widget) {
                    Ok(CapabilityValue::Int(slider.tick_interval() as i64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "tracking" => {
                if let Some(slider) = widget_as::<Slider>(widget) {
                    Ok(CapabilityValue::Bool(slider.tracking()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "slider_position" => {
                if let Some(slider) = widget_as::<Slider>(widget) {
                    Ok(CapabilityValue::Int(slider.slider_position() as i64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::ProgressBar => match property_name {
            "minimum" => {
                if let Some(pb) = widget_as::<ProgressBar>(widget) {
                    Ok(CapabilityValue::Int(pb.minimum() as i64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "maximum" => {
                if let Some(pb) = widget_as::<ProgressBar>(widget) {
                    Ok(CapabilityValue::Int(pb.maximum() as i64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "value" => {
                if let Some(pb) = widget_as::<ProgressBar>(widget) {
                    Ok(CapabilityValue::Int(pb.value() as i64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "text_visible" => {
                if let Some(pb) = widget_as::<ProgressBar>(widget) {
                    Ok(CapabilityValue::Bool(pb.is_text_visible()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "orientation" => {
                if let Some(pb) = widget_as::<ProgressBar>(widget) {
                    Ok(CapabilityValue::String(orientation_to_str(pb.orientation()).to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "inverted_appearance" => {
                if let Some(pb) = widget_as::<ProgressBar>(widget) {
                    Ok(CapabilityValue::Bool(pb.is_inverted_appearance()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "progress" => {
                if let Some(pb) = widget_as::<ProgressBar>(widget) {
                    Ok(CapabilityValue::Float(pb.progress() as f64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::ScrollBar => match property_name {
            "minimum" => {
                if let Some(sb) = widget_as::<ScrollBar>(widget) {
                    Ok(CapabilityValue::Int(sb.minimum() as i64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "maximum" => {
                if let Some(sb) = widget_as::<ScrollBar>(widget) {
                    Ok(CapabilityValue::Int(sb.maximum() as i64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "value" => {
                if let Some(sb) = widget_as::<ScrollBar>(widget) {
                    Ok(CapabilityValue::Int(sb.value() as i64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "single_step" => {
                if let Some(sb) = widget_as::<ScrollBar>(widget) {
                    Ok(CapabilityValue::Int(sb.single_step() as i64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "page_step" => {
                if let Some(sb) = widget_as::<ScrollBar>(widget) {
                    Ok(CapabilityValue::Int(sb.page_step() as i64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "orientation" => {
                if let Some(sb) = widget_as::<ScrollBar>(widget) {
                    Ok(CapabilityValue::String(orientation_to_str(sb.orientation()).to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "slider_size" => {
                if let Some(sb) = widget_as::<ScrollBar>(widget) {
                    Ok(CapabilityValue::Float(sb.slider_size() as f64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "slider_position" => {
                if let Some(sb) = widget_as::<ScrollBar>(widget) {
                    Ok(CapabilityValue::Float(sb.slider_position() as f64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::ListBox => match property_name {
            "item_count" => {
                if let Some(lb) = widget_as::<ListBox>(widget) {
                    Ok(CapabilityValue::UInt(lb.count() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "selection_mode" => {
                if let Some(lb) = widget_as::<ListBox>(widget) {
                    Ok(CapabilityValue::String(
                        list_box_selection_mode_to_str(lb.selection_mode()).to_string(),
                    ))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "current_row" => {
                if let Some(lb) = widget_as::<ListBox>(widget) {
                    match lb.current_row() {
                        Some(row) => Ok(CapabilityValue::UInt(row as u64)),
                        None => Ok(CapabilityValue::Null),
                    }
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "item_height" => {
                if let Some(lb) = widget_as::<ListBox>(widget) {
                    Ok(CapabilityValue::Float(lb.item_height() as f64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "selected_count" => {
                if let Some(lb) = widget_as::<ListBox>(widget) {
                    Ok(CapabilityValue::UInt(lb.selected_indices().len() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::SpinBox => match property_name {
            "minimum" => {
                if let Some(sb) = widget_as::<SpinBox>(widget) {
                    Ok(CapabilityValue::Int(sb.minimum() as i64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "maximum" => {
                if let Some(sb) = widget_as::<SpinBox>(widget) {
                    Ok(CapabilityValue::Int(sb.maximum() as i64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "value" => {
                if let Some(sb) = widget_as::<SpinBox>(widget) {
                    Ok(CapabilityValue::Int(sb.value() as i64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "single_step" => {
                if let Some(sb) = widget_as::<SpinBox>(widget) {
                    Ok(CapabilityValue::Int(sb.single_step() as i64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "prefix" => {
                if let Some(sb) = widget_as::<SpinBox>(widget) {
                    Ok(CapabilityValue::String(sb.prefix().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "suffix" => {
                if let Some(sb) = widget_as::<SpinBox>(widget) {
                    Ok(CapabilityValue::String(sb.suffix().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "special_value_text" => {
                if let Some(sb) = widget_as::<SpinBox>(widget) {
                    match sb.special_value_text() {
                        Some(text) => Ok(CapabilityValue::String(text.to_string())),
                        None => Ok(CapabilityValue::Null),
                    }
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "wrapping" => {
                if let Some(sb) = widget_as::<SpinBox>(widget) {
                    Ok(CapabilityValue::Bool(sb.wrapping()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::ComboBox => match property_name {
            "item_count" => {
                if let Some(cb) = widget_as::<ComboBox>(widget) {
                    Ok(CapabilityValue::UInt(cb.count() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "current_index" => {
                if let Some(cb) = widget_as::<ComboBox>(widget) {
                    match cb.current_index() {
                        Some(idx) => Ok(CapabilityValue::UInt(idx as u64)),
                        None => Ok(CapabilityValue::Null),
                    }
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "current_text" => {
                if let Some(cb) = widget_as::<ComboBox>(widget) {
                    Ok(CapabilityValue::String(cb.current_text().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "editable" => {
                if let Some(cb) = widget_as::<ComboBox>(widget) {
                    Ok(CapabilityValue::Bool(cb.is_editable()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "max_visible_items" => {
                if let Some(cb) = widget_as::<ComboBox>(widget) {
                    Ok(CapabilityValue::UInt(cb.max_visible_items() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::Dial => match property_name {
            "minimum" => {
                if let Some(dial) = widget_as::<Dial>(widget) {
                    Ok(CapabilityValue::Int(dial.minimum() as i64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "maximum" => {
                if let Some(dial) = widget_as::<Dial>(widget) {
                    Ok(CapabilityValue::Int(dial.maximum() as i64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "value" => {
                if let Some(dial) = widget_as::<Dial>(widget) {
                    Ok(CapabilityValue::Int(dial.value() as i64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "single_step" => {
                if let Some(dial) = widget_as::<Dial>(widget) {
                    Ok(CapabilityValue::Int(dial.single_step() as i64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "page_step" => {
                if let Some(dial) = widget_as::<Dial>(widget) {
                    Ok(CapabilityValue::Int(dial.page_step() as i64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "notches_visible" => {
                if let Some(dial) = widget_as::<Dial>(widget) {
                    Ok(CapabilityValue::Bool(dial.notches_visible()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "notch_target" => {
                if let Some(dial) = widget_as::<Dial>(widget) {
                    Ok(CapabilityValue::Float(dial.notch_target()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "wrapping" => {
                if let Some(dial) = widget_as::<Dial>(widget) {
                    Ok(CapabilityValue::Bool(dial.wrapping()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::Window => match property_name {
            "title" => {
                if let Some(win) = widget_as::<Window>(widget) {
                    Ok(CapabilityValue::String(win.title().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "title_bar_height" => {
                if let Some(win) = widget_as::<Window>(widget) {
                    Ok(CapabilityValue::UInt(win.title_bar_height() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "close_button_size" => {
                if let Some(win) = widget_as::<Window>(widget) {
                    Ok(CapabilityValue::UInt(win.close_button_size() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "button_spacing" => {
                if let Some(win) = widget_as::<Window>(widget) {
                    Ok(CapabilityValue::UInt(win.button_spacing() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::GroupBox => match property_name {
            "title" => {
                if let Some(gb) = widget_as::<GroupBox>(widget) {
                    Ok(CapabilityValue::String(gb.title().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "alignment" => {
                if let Some(gb) = widget_as::<GroupBox>(widget) {
                    Ok(CapabilityValue::String(alignment_to_str(gb.alignment()).to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "checkable" => {
                if let Some(gb) = widget_as::<GroupBox>(widget) {
                    Ok(CapabilityValue::Bool(gb.is_checkable()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "checked" => {
                if let Some(gb) = widget_as::<GroupBox>(widget) {
                    Ok(CapabilityValue::Bool(gb.is_checked()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::Splitter => match property_name {
            "orientation" => {
                if let Some(splitter) = widget_as::<Splitter>(widget) {
                    Ok(CapabilityValue::String(
                        orientation_to_str(splitter.orientation()).to_string(),
                    ))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "pane_count" => {
                if let Some(splitter) = widget_as::<Splitter>(widget) {
                    Ok(CapabilityValue::UInt(splitter.pane_count() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::LCDNumber => match property_name {
            "value" => {
                if let Some(lcd) = widget_as::<LCDNumber>(widget) {
                    Ok(CapabilityValue::Float(lcd.value()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "min_value" => {
                if let Some(lcd) = widget_as::<LCDNumber>(widget) {
                    Ok(CapabilityValue::Float(lcd.min_value()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "max_value" => {
                if let Some(lcd) = widget_as::<LCDNumber>(widget) {
                    Ok(CapabilityValue::Float(lcd.max_value()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "num_digits" => {
                if let Some(lcd) = widget_as::<LCDNumber>(widget) {
                    Ok(CapabilityValue::Int(lcd.num_digits() as i64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "small_decimal_point" => {
                if let Some(lcd) = widget_as::<LCDNumber>(widget) {
                    Ok(CapabilityValue::Bool(lcd.is_small_decimal_point()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "mode" => {
                if let Some(lcd) = widget_as::<LCDNumber>(widget) {
                    Ok(CapabilityValue::String(lcd_mode_to_str(lcd.mode()).to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "segment_style" => {
                if let Some(lcd) = widget_as::<LCDNumber>(widget) {
                    Ok(CapabilityValue::String(
                        segment_style_to_str(lcd.segment_style()).to_string(),
                    ))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::CommandLink => match property_name {
            "text" => {
                if let Some(cl) = widget_as::<CommandLink>(widget) {
                    Ok(CapabilityValue::String(cl.text().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "description" => {
                if let Some(cl) = widget_as::<CommandLink>(widget) {
                    Ok(CapabilityValue::String(cl.description().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "enabled" => {
                if let Some(cl) = widget_as::<CommandLink>(widget) {
                    Ok(CapabilityValue::Bool(cl.is_enabled()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::FontComboBox => match property_name {
            "current_font_family" => {
                if let Some(fcb) = widget_as::<FontComboBox>(widget) {
                    Ok(CapabilityValue::String(fcb.current_text()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "item_count" => {
                if let Some(fcb) = widget_as::<FontComboBox>(widget) {
                    Ok(CapabilityValue::Int(fcb.count() as i64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "current_index" => {
                if let Some(fcb) = widget_as::<FontComboBox>(widget) {
                    Ok(CapabilityValue::Int(fcb.current_index() as i64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "editable" => {
                if let Some(fcb) = widget_as::<FontComboBox>(widget) {
                    Ok(CapabilityValue::Bool(fcb.is_editable()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "max_visible_items" => {
                if let Some(fcb) = widget_as::<FontComboBox>(widget) {
                    Ok(CapabilityValue::Int(fcb.max_visible_items() as i64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::Action => match property_name {
            "text" => {
                if let Some(action) = widget_as::<Action>(widget) {
                    Ok(CapabilityValue::String(action.text().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "icon_text" => {
                if let Some(action) = widget_as::<Action>(widget) {
                    Ok(CapabilityValue::String(action.icon_text().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "shortcut" => {
                if let Some(action) = widget_as::<Action>(widget) {
                    Ok(CapabilityValue::String(action.shortcut().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "checkable" => {
                if let Some(action) = widget_as::<Action>(widget) {
                    Ok(CapabilityValue::Bool(action.is_checkable()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "checked" => {
                if let Some(action) = widget_as::<Action>(widget) {
                    Ok(CapabilityValue::Bool(action.is_checked()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "separator" => {
                if let Some(action) = widget_as::<Action>(widget) {
                    Ok(CapabilityValue::Bool(action.is_separator()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "command_id" => {
                if let Some(action) = widget_as::<Action>(widget) {
                    match action.command_id() {
                        Some(id) => Ok(CapabilityValue::String(id.to_string())),
                        None => Ok(CapabilityValue::Null),
                    }
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::Toolbox => match property_name {
            "item_count" => {
                if let Some(tb) = widget_as::<ToolBox>(widget) {
                    Ok(CapabilityValue::UInt(tb.count() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "current_index" => {
                if let Some(tb) = widget_as::<ToolBox>(widget) {
                    Ok(CapabilityValue::UInt(tb.current_index() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "orientation" => {
                if let Some(tb) = widget_as::<ToolBox>(widget) {
                    Ok(CapabilityValue::String(orientation_to_str(tb.orientation()).to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::TabBar => match property_name {
            "tab_count" => {
                if let Some(tb) = widget_as::<TabBar>(widget) {
                    Ok(CapabilityValue::UInt(tb.tab_count() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "current_index" => {
                if let Some(tb) = widget_as::<TabBar>(widget) {
                    match tb.current_index() {
                        Some(idx) => Ok(CapabilityValue::UInt(idx as u64)),
                        None => Ok(CapabilityValue::Null),
                    }
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "closable" => {
                if let Some(tb) = widget_as::<TabBar>(widget) {
                    Ok(CapabilityValue::Bool(tb.closable()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "movable" => {
                if let Some(tb) = widget_as::<TabBar>(widget) {
                    Ok(CapabilityValue::Bool(tb.movable()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "tab_min_width" => {
                if let Some(tb) = widget_as::<TabBar>(widget) {
                    Ok(CapabilityValue::UInt(tb.tab_min_width() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "tab_max_width" => {
                if let Some(tb) = widget_as::<TabBar>(widget) {
                    Ok(CapabilityValue::UInt(tb.tab_max_width() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::Calendar => match property_name {
            "selected_date" => {
                if let Some(cal) = widget_as::<Calendar>(widget) {
                    Ok(CapabilityValue::String(naive_date_to_string(cal.selected_date())))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "minimum_date" => {
                if let Some(cal) = widget_as::<Calendar>(widget) {
                    Ok(CapabilityValue::String(naive_date_to_string(cal.minimum_date())))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "maximum_date" => {
                if let Some(cal) = widget_as::<Calendar>(widget) {
                    Ok(CapabilityValue::String(naive_date_to_string(cal.maximum_date())))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "first_day_of_week" => {
                if let Some(cal) = widget_as::<Calendar>(widget) {
                    Ok(CapabilityValue::String(weekday_to_str(cal.first_day_of_week()).to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "grid_visible" => {
                if let Some(cal) = widget_as::<Calendar>(widget) {
                    Ok(CapabilityValue::Bool(cal.is_grid_visible()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "navigation_bar_visible" => {
                if let Some(cal) = widget_as::<Calendar>(widget) {
                    Ok(CapabilityValue::Bool(cal.is_navigation_bar_visible()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "horizontal_header_visible" => {
                if let Some(cal) = widget_as::<Calendar>(widget) {
                    Ok(CapabilityValue::Bool(cal.is_horizontal_header_visible()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "vertical_header_visible" => {
                if let Some(cal) = widget_as::<Calendar>(widget) {
                    Ok(CapabilityValue::Bool(cal.is_vertical_header_visible()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "date_format" => {
                if let Some(cal) = widget_as::<Calendar>(widget) {
                    Ok(CapabilityValue::String(cal.date_format().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::DatePicker => match property_name {
            "date" => {
                if let Some(de) = widget_as::<DateEdit>(widget) {
                    Ok(CapabilityValue::String(date_to_string(de.date())))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "minimum_date" => {
                if let Some(de) = widget_as::<DateEdit>(widget) {
                    Ok(CapabilityValue::String(date_to_string(de.minimum_date())))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "maximum_date" => {
                if let Some(de) = widget_as::<DateEdit>(widget) {
                    Ok(CapabilityValue::String(date_to_string(de.maximum_date())))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "display_format" => {
                if let Some(de) = widget_as::<DateEdit>(widget) {
                    Ok(CapabilityValue::String(de.display_format().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "calendar_popup" => {
                if let Some(de) = widget_as::<DateEdit>(widget) {
                    Ok(CapabilityValue::Bool(de.calendar_popup()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::TimePicker => match property_name {
            "time" => {
                if let Some(te) = widget_as::<TimeEdit>(widget) {
                    Ok(CapabilityValue::String(time_to_string(te.time())))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "minimum_time" => {
                if let Some(te) = widget_as::<TimeEdit>(widget) {
                    Ok(CapabilityValue::String(time_to_string(te.minimum_time())))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "maximum_time" => {
                if let Some(te) = widget_as::<TimeEdit>(widget) {
                    Ok(CapabilityValue::String(time_to_string(te.maximum_time())))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "display_format" => {
                if let Some(te) = widget_as::<TimeEdit>(widget) {
                    Ok(CapabilityValue::String(te.display_format().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::LineEdit => match property_name {
            "text" => {
                if let Some(le) = widget_as::<LineEdit>(widget) {
                    Ok(CapabilityValue::String(le.text().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "placeholder_text" => {
                if let Some(le) = widget_as::<LineEdit>(widget) {
                    Ok(CapabilityValue::String(le.placeholder_text().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "max_length" => {
                if let Some(le) = widget_as::<LineEdit>(widget) {
                    match le.max_length() {
                        Some(len) => Ok(CapabilityValue::UInt(len as u64)),
                        None => Ok(CapabilityValue::Null),
                    }
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "read_only" => {
                if let Some(le) = widget_as::<LineEdit>(widget) {
                    Ok(CapabilityValue::Bool(le.is_read_only()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "cursor_position" => {
                if let Some(le) = widget_as::<LineEdit>(widget) {
                    Ok(CapabilityValue::UInt(le.cursor_position() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::ListView => match property_name {
            "has_model" => {
                if let Some(lv) = widget_as::<ListView>(widget) {
                    Ok(CapabilityValue::Bool(lv.has_model()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "row_count" => {
                if let Some(lv) = widget_as::<ListView>(widget) {
                    Ok(CapabilityValue::UInt(lv.row_count() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "focused_row" => {
                if let Some(lv) = widget_as::<ListView>(widget) {
                    match lv.focused_row() {
                        Some(row) => Ok(CapabilityValue::UInt(row as u64)),
                        None => Ok(CapabilityValue::Null),
                    }
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "selection_mode" => {
                if let Some(lv) = widget_as::<ListView>(widget) {
                    Ok(CapabilityValue::String(
                        selection_mode_to_str(lv.selection_mode()).to_string(),
                    ))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "view_mode" => {
                if let Some(lv) = widget_as::<ListView>(widget) {
                    Ok(CapabilityValue::String(view_mode_to_str(lv.view_mode()).to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::TreeView => match property_name {
            "has_model" => {
                if let Some(tt) = widget_as::<TreeTable>(widget) {
                    Ok(CapabilityValue::Bool(tt.has_model()))
                } else if let Some(tv) = widget_as::<TreeView>(widget) {
                    Ok(CapabilityValue::Bool(tv.has_model()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "node_count" => {
                if let Some(tv) = widget_as::<TreeView>(widget) {
                    Ok(CapabilityValue::UInt(tv.node_count() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "focused_node" => {
                if let Some(tv) = widget_as::<TreeView>(widget) {
                    match tv.focused_node() {
                        Some(node) => Ok(CapabilityValue::UInt(node as u64)),
                        None => Ok(CapabilityValue::Null),
                    }
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "selected_node" => {
                if let Some(tv) = widget_as::<TreeView>(widget) {
                    match tv.selected_node() {
                        Some(node) => Ok(CapabilityValue::UInt(node as u64)),
                        None => Ok(CapabilityValue::Null),
                    }
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "row_count" => {
                if let Some(tt) = widget_as::<TreeTable>(widget) {
                    Ok(CapabilityValue::UInt(tt.row_count() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "column_count" => {
                if let Some(tt) = widget_as::<TreeTable>(widget) {
                    Ok(CapabilityValue::UInt(tt.column_count() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "selected_row" => {
                if let Some(tt) = widget_as::<TreeTable>(widget) {
                    match tt.selected_row() {
                        Some(row) => Ok(CapabilityValue::UInt(row as u64)),
                        None => Ok(CapabilityValue::Null),
                    }
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "row_height" => {
                if let Some(tt) = widget_as::<TreeTable>(widget) {
                    Ok(CapabilityValue::UInt(tt.row_height() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "column_width" => {
                if let Some(tt) = widget_as::<TreeTable>(widget) {
                    Ok(CapabilityValue::UInt(tt.column_width() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "projection_state" => {
                if let Some(tt) = widget_as::<TreeTable>(widget) {
                    let selected = match tt.selected_row() {
                        Some(r) => format!("Some({})", r),
                        None => "None".to_string(),
                    };
                    Ok(CapabilityValue::String(format!(
                        "rows={},selected={}",
                        tt.row_count(),
                        selected
                    )))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::Table => match property_name {
            "has_model" => {
                if let Some(tw) = widget_as::<TableWidget>(widget) {
                    Ok(CapabilityValue::Bool(tw.has_model()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "has_delegate" => {
                if let Some(tw) = widget_as::<TableWidget>(widget) {
                    Ok(CapabilityValue::Bool(tw.has_delegate()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "row_count" => {
                if let Some(tw) = widget_as::<TableWidget>(widget) {
                    Ok(CapabilityValue::UInt(tw.row_count() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "column_count" => {
                if let Some(tw) = widget_as::<TableWidget>(widget) {
                    Ok(CapabilityValue::UInt(tw.column_count() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "selection_mode" => {
                if let Some(tw) = widget_as::<TableWidget>(widget) {
                    Ok(CapabilityValue::String(
                        selection_mode_to_str(tw.selection_mode()).to_string(),
                    ))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "has_data_source" => {
                if let Some(dg) = widget_as::<DataGrid>(widget) {
                    Ok(CapabilityValue::Bool(dg.has_data_source()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "scroll_row" => {
                if let Some(dg) = widget_as::<DataGrid>(widget) {
                    Ok(CapabilityValue::UInt(dg.scroll_row() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "scroll_column" => {
                if let Some(dg) = widget_as::<DataGrid>(widget) {
                    Ok(CapabilityValue::UInt(dg.scroll_column() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "row_height" => {
                if let Some(dg) = widget_as::<DataGrid>(widget) {
                    Ok(CapabilityValue::UInt(dg.row_height() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "column_width" => {
                if let Some(dg) = widget_as::<DataGrid>(widget) {
                    Ok(CapabilityValue::UInt(dg.column_width() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "overscan_rows" => {
                if let Some(vt) = widget_as::<VirtualTable>(widget) {
                    Ok(CapabilityValue::UInt(vt.overscan_rows() as u64))
                } else if let Some(_dg) = widget_as::<DataGrid>(widget) {
                    Ok(CapabilityValue::Null)
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "overscan_columns" => {
                if let Some(vt) = widget_as::<VirtualTable>(widget) {
                    Ok(CapabilityValue::UInt(vt.overscan_columns() as u64))
                } else if let Some(_dg) = widget_as::<DataGrid>(widget) {
                    Ok(CapabilityValue::Null)
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "frozen_columns" => {
                if let Some(dg) = widget_as::<DataGrid>(widget) {
                    Ok(CapabilityValue::UInt(dg.frozen_columns() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "sort_spec_count" => {
                if let Some(dg) = widget_as::<DataGrid>(widget) {
                    Ok(CapabilityValue::UInt(dg.sort_specs().len() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "filter_count" => {
                if let Some(dg) = widget_as::<DataGrid>(widget) {
                    Ok(CapabilityValue::UInt(dg.filters().len() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "sort_specs" => {
                if let Some(dg) = widget_as::<DataGrid>(widget) {
                    Ok(CapabilityValue::String(sort_specs_to_string(dg.sort_specs())))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "filters" => {
                if let Some(dg) = widget_as::<DataGrid>(widget) {
                    Ok(CapabilityValue::String(column_filters_to_string(dg.filters())))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "visible_window" => {
                if let Some(dg) = widget_as::<DataGrid>(widget) {
                    let (rs, rl, cs, cl) = dg.visible_window();
                    Ok(CapabilityValue::String(format!("{rs}:{rl}:{cs}:{cl}")))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::DataView => match property_name {
            "has_data_source" => {
                if let Some(vl) = widget_as::<VirtualList>(widget) {
                    Ok(CapabilityValue::Bool(vl.has_data_source()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "row_count" => {
                if let Some(vl) = widget_as::<VirtualList>(widget) {
                    Ok(CapabilityValue::UInt(vl.row_count() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "scroll_row" => {
                if let Some(vl) = widget_as::<VirtualList>(widget) {
                    Ok(CapabilityValue::UInt(vl.scroll_row() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "row_height" => {
                if let Some(vl) = widget_as::<VirtualList>(widget) {
                    Ok(CapabilityValue::UInt(vl.row_height() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "overscan" => {
                if let Some(vl) = widget_as::<VirtualList>(widget) {
                    Ok(CapabilityValue::UInt(vl.overscan() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "selected_row" => {
                if let Some(vl) = widget_as::<VirtualList>(widget) {
                    match vl.selected_row() {
                        Some(row) => Ok(CapabilityValue::UInt(row as u64)),
                        None => Ok(CapabilityValue::Null),
                    }
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::Menu => match property_name {
            "title" => {
                if let Some(menu) = widget_as::<Menu>(widget) {
                    Ok(CapabilityValue::String(menu.title().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "item_count" => {
                if let Some(menu) = widget_as::<Menu>(widget) {
                    Ok(CapabilityValue::UInt(menu.items().len() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "hovered_index" => {
                if let Some(menu) = widget_as::<Menu>(widget) {
                    match menu.hovered_index() {
                        Some(idx) => Ok(CapabilityValue::UInt(idx as u64)),
                        None => Ok(CapabilityValue::Null),
                    }
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::MenuBar => match property_name {
            "entry_count" => {
                if let Some(mb) = widget_as::<MenuBar>(widget) {
                    Ok(CapabilityValue::UInt(mb.entries().len() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "active_index" => {
                if let Some(mb) = widget_as::<MenuBar>(widget) {
                    match mb.active_index() {
                        Some(idx) => Ok(CapabilityValue::UInt(idx as u64)),
                        None => Ok(CapabilityValue::Null),
                    }
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::ToolBar => match property_name {
            "orientation" => {
                if let Some(tb) = widget_as::<ToolBar>(widget) {
                    Ok(CapabilityValue::String(
                        tool_bar_orientation_to_str(tb.orientation()).to_string(),
                    ))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "icon_size" => {
                if let Some(tb) = widget_as::<ToolBar>(widget) {
                    Ok(CapabilityValue::Float(tb.icon_size() as f64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "floatable" => {
                if let Some(tb) = widget_as::<ToolBar>(widget) {
                    Ok(CapabilityValue::Bool(tb.is_floatable()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "movable" => {
                if let Some(tb) = widget_as::<ToolBar>(widget) {
                    Ok(CapabilityValue::Bool(tb.is_movable()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::RibbonBar => match property_name {
            "tab_count" => {
                if let Some(rb) = widget_as::<RibbonBar>(widget) {
                    Ok(CapabilityValue::UInt(rb.tab_count() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "current_tab" => {
                if let Some(rb) = widget_as::<RibbonBar>(widget) {
                    Ok(CapabilityValue::UInt(rb.current_tab() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::ColorDialog => match property_name {
            "hex_rgba" => {
                if let Some(cp) = widget_as::<ColorPicker>(widget) {
                    Ok(CapabilityValue::String(cp.hex_rgba().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "show_alpha" => {
                if let Some(cp) = widget_as::<ColorPicker>(widget) {
                    Ok(CapabilityValue::Bool(cp.show_alpha()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::RichEdit => match property_name {
            "text" => {
                if let Some(ce) = widget_as::<CodeEditor>(widget) {
                    Ok(CapabilityValue::String(ce.text().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "line_count" => {
                if let Some(ce) = widget_as::<CodeEditor>(widget) {
                    Ok(CapabilityValue::UInt(ce.line_count() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "cursor_line" => {
                if let Some(ce) = widget_as::<CodeEditor>(widget) {
                    Ok(CapabilityValue::UInt(ce.cursor().0 as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "cursor_column" => {
                if let Some(ce) = widget_as::<CodeEditor>(widget) {
                    Ok(CapabilityValue::UInt(ce.cursor().1 as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::Chart => match property_name {
            "task_count" => {
                if let Some(gw) = widget_as::<GanttWidget>(widget) {
                    Ok(CapabilityValue::UInt(gw.tasks().len() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "selected_id" => {
                if let Some(gw) = widget_as::<GanttWidget>(widget) {
                    match gw.selected_id() {
                        Some(id) => Ok(CapabilityValue::String(id.to_string())),
                        None => Ok(CapabilityValue::Null),
                    }
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "viewport_start" => {
                if let Some(gw) = widget_as::<GanttWidget>(widget) {
                    Ok(CapabilityValue::Int(gw.viewport().0))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "viewport_end" => {
                if let Some(gw) = widget_as::<GanttWidget>(widget) {
                    Ok(CapabilityValue::Int(gw.viewport().1))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "zoom_level" => {
                if let Some(_gw) = widget_as::<GanttWidget>(widget) {
                    Ok(CapabilityValue::Null)
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::TextEdit => match property_name {
            "output_line_count" => {
                if let Some(tv) = widget_as::<TerminalView>(widget) {
                    Ok(CapabilityValue::UInt(tv.lines().len() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "input_line" => {
                if let Some(tv) = widget_as::<TerminalView>(widget) {
                    Ok(CapabilityValue::String(tv.input_line().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },

        WidgetKind::Canvas => match property_name {
            "center_x" => {
                if let Some(mv) = widget_as::<MapView>(widget) {
                    Ok(CapabilityValue::Float(mv.center().0 as f64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "center_y" => {
                if let Some(mv) = widget_as::<MapView>(widget) {
                    Ok(CapabilityValue::Float(mv.center().1 as f64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "zoom" => {
                if let Some(mv) = widget_as::<MapView>(widget) {
                    Ok(CapabilityValue::Float(mv.zoom() as f64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "marker_count" => {
                if let Some(mv) = widget_as::<MapView>(widget) {
                    Ok(CapabilityValue::UInt(mv.markers().len() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::WebEngineView => match property_name {
            "url" => {
                if let Some(wv) = widget_as::<WebView>(widget) {
                    Ok(CapabilityValue::String(wv.url().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "loading" => {
                if let Some(wv) = widget_as::<WebView>(widget) {
                    Ok(CapabilityValue::Bool(wv.is_loading()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "title" => {
                if let Some(wv) = widget_as::<WebView>(widget) {
                    Ok(CapabilityValue::String(wv.title().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::Panel | WidgetKind::Frame => match property_name {
            "segment_count" => {
                if let Some(bc) = widget_as::<Breadcrumb>(widget) {
                    Ok(CapabilityValue::UInt(bc.segments().len() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "selected_index" => {
                if let Some(bc) = widget_as::<Breadcrumb>(widget) {
                    match bc.selected_index() {
                        Some(idx) => Ok(CapabilityValue::UInt(idx as u64)),
                        None => Ok(CapabilityValue::Null),
                    }
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },

        WidgetKind::ToggleButton => match property_name {
            "text" => {
                if let Some(tb) = widget_as::<ToggleButton>(widget) {
                    Ok(CapabilityValue::String(tb.text().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "checked" => {
                if let Some(tb) = widget_as::<ToggleButton>(widget) {
                    Ok(CapabilityValue::Bool(tb.is_checked()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "state" => {
                if let Some(tb) = widget_as::<ToggleButton>(widget) {
                    let s = match tb.state() {
                        ToggleButtonState::Normal => "normal",
                        ToggleButtonState::Checked => "checked",
                        ToggleButtonState::Disabled => "disabled",
                    };
                    Ok(CapabilityValue::String(s.to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::CheckListBox => match property_name {
            "item_count" => {
                if let Some(chip) = widget_as::<Chip>(widget) {
                    Ok(CapabilityValue::UInt(chip.items().len() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "multi_select" => {
                if let Some(chip) = widget_as::<Chip>(widget) {
                    Ok(CapabilityValue::Bool(chip.multi_select()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::Grid => match property_name {
            "rows" => {
                if let Some(grid) = widget_as::<GridWidget>(widget) {
                    Ok(CapabilityValue::UInt(grid.rows() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "columns" => {
                if let Some(grid) = widget_as::<GridWidget>(widget) {
                    Ok(CapabilityValue::UInt(grid.columns() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "spacing" => {
                if let Some(grid) = widget_as::<GridWidget>(widget) {
                    Ok(CapabilityValue::UInt(grid.spacing() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::FreeformShape => match property_name {
            "path_kind" => {
                if let Some(fs) = widget_as::<FreeformShapeWidget>(widget) {
                    let s = match fs.path() {
                        ShapePath::Heart => "heart",
                        ShapePath::Star { .. } => "star",
                        ShapePath::Polygon(_) => "polygon",
                        ShapePath::RoundedRect { .. } => "rounded_rect",
                        ShapePath::Bubble { .. } => "bubble",
                        ShapePath::Custom(_) => "custom",
                    };
                    Ok(CapabilityValue::String(s.to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "fill_rgba" => {
                if let Some(fs) = widget_as::<FreeformShapeWidget>(widget) {
                    Ok(CapabilityValue::String(fs.fill_color().to_hex_rgba()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "stroke_rgba" => {
                if let Some(fs) = widget_as::<FreeformShapeWidget>(widget) {
                    match fs.stroke_color() {
                        Some(color) => Ok(CapabilityValue::String(color.to_hex_rgba())),
                        None => Ok(CapabilityValue::Null),
                    }
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "stroke_width" => {
                if let Some(fs) = widget_as::<FreeformShapeWidget>(widget) {
                    Ok(CapabilityValue::UInt(fs.stroke_width() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::MessageBox => match property_name {
            "title" => {
                if let Some(mb) = widget_as::<MessageBox>(widget) {
                    Ok(CapabilityValue::String(mb.title().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "text" => {
                if let Some(mb) = widget_as::<MessageBox>(widget) {
                    Ok(CapabilityValue::String(mb.text().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::FileDialog => match property_name {
            "title" => {
                if let Some(fd) = widget_as::<FileDialog>(widget) {
                    Ok(CapabilityValue::String(fd.title().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "modal" => {
                if let Some(fd) = widget_as::<FileDialog>(widget) {
                    Ok(CapabilityValue::Bool(fd.is_modal()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::FontDialog => match property_name {
            "modal" => {
                if let Some(fd) = widget_as::<FontDialog>(widget) {
                    Ok(CapabilityValue::Bool(fd.is_modal()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::InputDialog => match property_name {
            "title" => {
                if let Some(id) = widget_as::<InputDialog>(widget) {
                    Ok(CapabilityValue::String(id.title().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "label_text" => {
                if let Some(id) = widget_as::<InputDialog>(widget) {
                    Ok(CapabilityValue::String(id.label_text().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::ProgressDialog => match property_name {
            "title" => {
                if let Some(pd) = widget_as::<ProgressDialog>(widget) {
                    Ok(CapabilityValue::String(pd.title().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "label_text" => {
                if let Some(pd) = widget_as::<ProgressDialog>(widget) {
                    Ok(CapabilityValue::String(pd.label_text().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::PopupWindow => match property_name {
            "has_content" => {
                if let Some(pw) = widget_as::<PopupWindow>(widget) {
                    Ok(CapabilityValue::Bool(pw.content_widget().is_some()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::ScrollArea => match property_name {
            "widget_resizable" => {
                if let Some(sa) = widget_as::<ScrollArea>(widget) {
                    Ok(CapabilityValue::Bool(sa.widget_resizable()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "horizontal_scroll_bar_policy" => {
                if let Some(sa) = widget_as::<ScrollArea>(widget) {
                    let s = scroll_bar_policy_to_str(sa.horizontal_scroll_bar_policy());
                    Ok(CapabilityValue::String(s.to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "vertical_scroll_bar_policy" => {
                if let Some(sa) = widget_as::<ScrollArea>(widget) {
                    let s = scroll_bar_policy_to_str(sa.vertical_scroll_bar_policy());
                    Ok(CapabilityValue::String(s.to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::TabWidget => match property_name {
            "tab_count" => {
                if let Some(tw) = widget_as::<TabWidget>(widget) {
                    Ok(CapabilityValue::UInt(tw.count() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "current_index" => {
                if let Some(tw) = widget_as::<TabWidget>(widget) {
                    Ok(CapabilityValue::UInt(tw.current_index() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::StackedWidget => match property_name {
            "widget_count" => {
                if let Some(sw) = widget_as::<StackedWidget>(widget) {
                    Ok(CapabilityValue::UInt(sw.widget_count() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "current_index" => {
                if let Some(sw) = widget_as::<StackedWidget>(widget) {
                    Ok(CapabilityValue::UInt(sw.current_index() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::CollapsiblePane => match property_name {
            "title" => {
                if let Some(cp) = widget_as::<CollapsiblePane>(widget) {
                    Ok(CapabilityValue::String(cp.title().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "collapsed" => {
                if let Some(cp) = widget_as::<CollapsiblePane>(widget) {
                    Ok(CapabilityValue::Bool(cp.is_collapsed()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::DockWidget => match property_name {
            "title" => {
                if let Some(dw) = widget_as::<DockWidget>(widget) {
                    Ok(CapabilityValue::String(dw.title().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "floating" => {
                if let Some(dw) = widget_as::<DockWidget>(widget) {
                    Ok(CapabilityValue::Bool(dw.is_floating()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::MdiArea => match property_name {
            "subwindow_count" => {
                if let Some(ma) = widget_as::<MdiArea>(widget) {
                    Ok(CapabilityValue::UInt(ma.sub_window_count() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "active_subwindow" => {
                if let Some(ma) = widget_as::<MdiArea>(widget) {
                    match ma.active_sub_window() {
                        Some(id) => Ok(CapabilityValue::UInt(id)),
                        None => Ok(CapabilityValue::Null),
                    }
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "view_mode" => {
                if let Some(ma) = widget_as::<MdiArea>(widget) {
                    let s = match ma.view_mode() {
                        crate::widget::container_widgets::mdiarea::ViewMode::SubWindowView => {
                            "sub_window_view"
                        }
                        crate::widget::container_widgets::mdiarea::ViewMode::TabbedView => "tabbed",
                    };
                    Ok(CapabilityValue::String(s.to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::PieMenu => match property_name {
            "item_count" => {
                if let Some(pm) = widget_as::<PieMenu>(widget) {
                    Ok(CapabilityValue::UInt(pm.item_count() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "radius" => {
                if let Some(pm) = widget_as::<PieMenu>(widget) {
                    Ok(CapabilityValue::Float(pm.radius() as f64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "inner_radius" => {
                if let Some(pm) = widget_as::<PieMenu>(widget) {
                    Ok(CapabilityValue::Float(pm.inner_radius() as f64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "current_index" => {
                if let Some(pm) = widget_as::<PieMenu>(widget) {
                    Ok(CapabilityValue::UInt(pm.current_index() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::DateTimePicker => match property_name {
            "datetime" => {
                if let Some(dte) = widget_as::<DateTimeEdit>(widget) {
                    Ok(CapabilityValue::String(dte.datetime().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "display_format" => {
                if let Some(dte) = widget_as::<DateTimeEdit>(widget) {
                    Ok(CapabilityValue::String(dte.display_format().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "calendar_popup" => {
                if let Some(dte) = widget_as::<DateTimeEdit>(widget) {
                    Ok(CapabilityValue::Bool(dte.calendar_popup()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        // ── Always-available widget reads (not mini-gated) ─────────
        WidgetKind::Arc => match property_name {
            "value" => {
                if let Some(w) = widget_as::<Arc>(widget) {
                    Ok(CapabilityValue::UInt(w.value() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::Spinner => match property_name {
            "active" => {
                if let Some(w) = widget_as::<Spinner>(widget) {
                    Ok(CapabilityValue::Bool(w.is_active()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "thickness" => {
                if let Some(w) = widget_as::<Spinner>(widget) {
                    Ok(CapabilityValue::UInt(w.thickness() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "speed" => {
                if let Some(w) = widget_as::<Spinner>(widget) {
                    Ok(CapabilityValue::Float(w.speed() as f64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "size_ratio" => {
                if let Some(w) = widget_as::<Spinner>(widget) {
                    Ok(CapabilityValue::Float(w.size_ratio() as f64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::Roller => match property_name {
            "selected_index" => {
                if let Some(w) = widget_as::<Roller>(widget) {
                    Ok(CapabilityValue::UInt(w.selected_index() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "visible_count" => {
                if let Some(w) = widget_as::<Roller>(widget) {
                    Ok(CapabilityValue::UInt(w.visible_count() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "item_count" => {
                if let Some(w) = widget_as::<Roller>(widget) {
                    Ok(CapabilityValue::UInt(w.options().len() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::Dropdown => match property_name {
            "text" => {
                if let Some(w) = widget_as::<Dropdown>(widget) {
                    match w.selected_text() {
                        Some(t) => Ok(CapabilityValue::String(t.to_string())),
                        None => Ok(CapabilityValue::String(String::new())),
                    }
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "selected_index" => {
                if let Some(w) = widget_as::<Dropdown>(widget) {
                    Ok(CapabilityValue::UInt(w.selected_index() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "item_count" => {
                if let Some(w) = widget_as::<Dropdown>(widget) {
                    Ok(CapabilityValue::UInt(w.items().len() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "expanded" => {
                if let Some(w) = widget_as::<Dropdown>(widget) {
                    Ok(CapabilityValue::Bool(w.is_expanded()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::TextArea => match property_name {
            "text" => {
                if let Some(w) = widget_as::<TextArea>(widget) {
                    Ok(CapabilityValue::String(w.text().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "placeholder" => {
                if let Some(w) = widget_as::<TextArea>(widget) {
                    Ok(CapabilityValue::String(w.placeholder().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "read_only" => {
                if let Some(w) = widget_as::<TextArea>(widget) {
                    Ok(CapabilityValue::Bool(w.is_read_only()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::Keyboard => match property_name {
            "layout" => {
                if let Some(w) = widget_as::<Keyboard>(widget) {
                    let s = match w.layout() {
                        KeyboardLayout::Qwerty => "qwerty",
                        KeyboardLayout::Numeric => "numeric",
                    };
                    Ok(CapabilityValue::String(s.to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "lowercase" => {
                if let Some(w) = widget_as::<Keyboard>(widget) {
                    Ok(CapabilityValue::Bool(w.lowercase()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::Switch => match property_name {
            "checked" => {
                if let Some(w) = widget_as::<Switch>(widget) {
                    Ok(CapabilityValue::Bool(w.is_checked()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::Line => match property_name {
            "orientation" => {
                if let Some(w) = widget_as::<Line>(widget) {
                    let s = match w.orientation() {
                        LineOrientation::Horizontal => "horizontal",
                        LineOrientation::Vertical => "vertical",
                    };
                    Ok(CapabilityValue::String(s.to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::Meter => match property_name {
            "value" => {
                if let Some(w) = widget_as::<Meter>(widget) {
                    Ok(CapabilityValue::UInt(w.value() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::MiniChart => match property_name {
            "chart_type" => {
                if let Some(w) = widget_as::<MiniChart>(widget) {
                    let s = match w.chart_type() {
                        ChartType::Line => "line",
                        ChartType::Bar => "bar",
                    };
                    Ok(CapabilityValue::String(s.to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::ImageView => match property_name {
            "scaled" => {
                if let Some(w) = widget_as::<ImageView>(widget) {
                    Ok(CapabilityValue::Bool(w.is_scaled()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::TileView => match property_name {
            "current_page" => {
                if let Some(w) = widget_as::<TileView>(widget) {
                    Ok(CapabilityValue::UInt(w.current_page() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "page_count" => {
                if let Some(w) = widget_as::<TileView>(widget) {
                    Ok(CapabilityValue::UInt(w.page_count() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },

        // ── New widgets (menu/toolbar) ───────────────────────────
        WidgetKind::ToolButton => match property_name {
            "text" => {
                if let Some(w) = widget_as::<ToolButton>(widget) {
                    Ok(CapabilityValue::String(w.text().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "checked" => {
                if let Some(w) = widget_as::<ToolButton>(widget) {
                    Ok(CapabilityValue::Bool(w.is_checked()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::StatusBar => match property_name {
            "message" => {
                if let Some(w) = widget_as::<StatusBar>(widget) {
                    Ok(CapabilityValue::String(w.message().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },

        // ── New widgets (input) ────────────────────────────────────
        WidgetKind::SearchBar => match property_name {
            "text" => {
                if let Some(w) = widget_as::<SearchBar>(widget) {
                    Ok(CapabilityValue::String(w.text().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "placeholder" => {
                if let Some(w) = widget_as::<SearchBar>(widget) {
                    Ok(CapabilityValue::String(w.placeholder().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::ShortcutEditor => match property_name {
            "filter_text" => {
                if let Some(w) = widget_as::<ShortcutEditor>(widget) {
                    Ok(CapabilityValue::String(w.filter_text().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },

        // ── New widgets (navigation) ───────────────────────────────
        WidgetKind::TabView => match property_name {
            "selected_index" => {
                if let Some(w) = widget_as::<TabView>(widget) {
                    Ok(CapabilityValue::UInt(w.current_index() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::MaterialNavigationRail => match property_name {
            "selected_index" => {
                if let Some(w) = widget_as::<MaterialNavigationRail>(widget) {
                    Ok(CapabilityValue::UInt(w.selected() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },

        // ── New widgets (container) ─────────────────────────────────
        WidgetKind::PagerPageView => match property_name {
            "current_page" => {
                if let Some(w) = widget_as::<PagerPageView>(widget) {
                    Ok(CapabilityValue::UInt(w.current_page() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },

        // ── New widgets (overlay) ───────────────────────────────────
        WidgetKind::SwipeToDismiss => match property_name {
            "is_dismissed" => {
                if let Some(w) = widget_as::<SwipeToDismiss>(widget) {
                    Ok(CapabilityValue::Bool(w.is_dismissed()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },

        // ── New widgets (chart) ─────────────────────────────────────
        WidgetKind::LineChart => match property_name {
            "stroke_width" => {
                if let Some(w) = widget_as::<LineChart>(widget) {
                    Ok(CapabilityValue::Float(w.stroke_width() as f64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::Sparkline => match property_name {
            "stroke_width" => {
                if let Some(w) = widget_as::<Sparkline>(widget) {
                    Ok(CapabilityValue::Float(w.stroke_width() as f64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::BarChart => match property_name {
            "bar_spacing" => {
                if let Some(w) = widget_as::<BarChart>(widget) {
                    Ok(CapabilityValue::Float(w.bar_spacing() as f64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::PieChart => match property_name {
            "donut" => {
                if let Some(w) = widget_as::<PieChart>(widget) {
                    Ok(CapabilityValue::Bool(w.is_donut()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },

        // ── New widgets (media/animation) ───────────────────────────
        WidgetKind::AnimatedImage => match property_name {
            "playing" => {
                if let Some(w) = widget_as::<AnimatedImage>(widget) {
                    Ok(CapabilityValue::Bool(w.is_playing()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::HeroAnimation => match property_name {
            "animation_progress" => {
                if let Some(w) = widget_as::<HeroAnimation>(widget) {
                    Ok(CapabilityValue::Float(w.progress() as f64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::LottieWidget => match property_name {
            "playing" => {
                if let Some(w) = widget_as::<LottieWidget>(widget) {
                    Ok(CapabilityValue::Bool(w.is_playing()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::RiveWidget => match property_name {
            "is_playing" => {
                if let Some(w) = widget_as::<RiveWidget>(widget) {
                    Ok(CapabilityValue::Bool(w.is_playing()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::VideoPlayer => match property_name {
            "is_playing" => {
                if let Some(w) = widget_as::<VideoPlayer>(widget) {
                    Ok(CapabilityValue::Bool(w.is_playing()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "volume" => {
                if let Some(w) = widget_as::<VideoPlayer>(widget) {
                    Ok(CapabilityValue::Float(w.volume() as f64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },

        // ── New widgets (view) ──────────────────────────────────────
        WidgetKind::ImageGallery => match property_name {
            "current_index" => {
                if let Some(w) = widget_as::<ImageGallery>(widget) {
                    Ok(CapabilityValue::UInt(w.current_index() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },

        // ── New widgets (view / property) ───────────────────────────
        WidgetKind::PropertyGrid => match property_name {
            "property_count" => {
                if let Some(w) = widget_as::<PropertyGrid>(widget) {
                    Ok(CapabilityValue::UInt(w.property_count() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "selected_index" => {
                if let Some(w) = widget_as::<PropertyGrid>(widget) {
                    match w.selected_index() {
                        Some(idx) => Ok(CapabilityValue::UInt(idx as u64)),
                        None => Ok(CapabilityValue::Null),
                    }
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },

        // ── New widgets (misc) ──────────────────────────────────────
        WidgetKind::AudioVisualizer => match property_name {
            "bar_count" => {
                if let Some(w) = widget_as::<AudioVisualizer>(widget) {
                    Ok(CapabilityValue::UInt(w.bar_count() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::CameraPreview => match property_name {
            "is_active" => {
                if let Some(w) = widget_as::<CameraPreview>(widget) {
                    Ok(CapabilityValue::Bool(w.is_active()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::BarcodeScanner => match property_name {
            "is_scanning" => {
                if let Some(w) = widget_as::<BarcodeScanner>(widget) {
                    Ok(CapabilityValue::Bool(w.is_scanning()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::BezierCurveEditor => match property_name {
            "snap_to_grid" => {
                if let Some(w) = widget_as::<BezierCurveEditor>(widget) {
                    Ok(CapabilityValue::Bool(w.snap_to_grid()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::CupertinoSlider => match property_name {
            "value" => {
                if let Some(w) = widget_as::<CupertinoSlider>(widget) {
                    Ok(CapabilityValue::Float(w.value().into()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "min" => {
                if let Some(w) = widget_as::<CupertinoSlider>(widget) {
                    Ok(CapabilityValue::Float(w.min().into()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "max" => {
                if let Some(w) = widget_as::<CupertinoSlider>(widget) {
                    Ok(CapabilityValue::Float(w.max().into()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },

        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
    }
}

#[cfg(not(feature = "mini"))]
pub fn write_widget_property_value(
    widget: &mut dyn Widget,
    property_name: &str,
    value: CapabilityValue,
) -> Result<(), CapabilityAccessError> {
    match widget.kind() {
        WidgetKind::Button => {
            if let Some(button) = widget_as_mut::<Button>(widget) {
                match property_name {
                    "text" => {
                        button.set_text(expect_string(value)?);
                        Ok(())
                    }
                    "pressed" => {
                        button.set_pressed(expect_bool(value)?);
                        Ok(())
                    }
                    "default" => {
                        button.set_default(expect_bool(value)?);
                        Ok(())
                    }
                    "enabled" => {
                        button.set_enabled(expect_bool(value)?);
                        Ok(())
                    }
                    "tooltip" => {
                        button.set_tooltip(expect_string(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::Label => {
            if let Some(label) = widget_as_mut::<Label>(widget) {
                match property_name {
                    "text" => {
                        label.set_text(expect_string(value)?);
                        Ok(())
                    }
                    "alignment" => {
                        label.set_alignment(expect_alignment(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::CheckBox => {
            if let Some(check_box) = widget_as_mut::<CheckBox>(widget) {
                match property_name {
                    "text" => {
                        check_box.set_text(expect_string(value)?);
                        Ok(())
                    }
                    "state" => {
                        check_box.set_state(expect_check_state(value)?);
                        Ok(())
                    }
                    "checked" => {
                        check_box.set_checked(expect_bool(value)?);
                        Ok(())
                    }
                    "tristate_enabled" => {
                        check_box.set_tristate_enabled(expect_bool(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::RadioButton => {
            if let Some(radio_button) = widget_as_mut::<RadioButton>(widget) {
                match property_name {
                    "text" => {
                        radio_button.set_text(expect_string(value)?);
                        Ok(())
                    }
                    "checked" => {
                        radio_button.set_checked(expect_bool(value)?);
                        Ok(())
                    }
                    "group_id" => {
                        match value {
                            CapabilityValue::Null => radio_button.set_group_id(None),
                            other => radio_button.set_group_id(Some(expect_string(other)?)),
                        }
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::Slider => {
            if let Some(slider) = widget_as_mut::<Slider>(widget) {
                match property_name {
                    "minimum" => {
                        slider.set_minimum(expect_i64(value)? as i32);
                        Ok(())
                    }
                    "maximum" => {
                        slider.set_maximum(expect_i64(value)? as i32);
                        Ok(())
                    }
                    "value" => {
                        slider.set_value(expect_i64(value)? as i32);
                        Ok(())
                    }
                    "single_step" => {
                        slider.set_single_step(expect_i64(value)? as i32);
                        Ok(())
                    }
                    "page_step" => {
                        slider.set_page_step(expect_i64(value)? as i32);
                        Ok(())
                    }
                    "orientation" => {
                        slider.set_orientation(expect_orientation(value)?);
                        Ok(())
                    }
                    "tick_position" => {
                        slider.set_tick_position(expect_tick_position(value)?);
                        Ok(())
                    }
                    "tick_interval" => {
                        slider.set_tick_interval(expect_i64(value)? as i32);
                        Ok(())
                    }
                    "tracking" => {
                        slider.set_tracking(expect_bool(value)?);
                        Ok(())
                    }
                    "slider_position" => {
                        slider.set_slider_position(expect_i64(value)? as i32);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::ProgressBar => {
            if let Some(progress_bar) = widget_as_mut::<ProgressBar>(widget) {
                match property_name {
                    "minimum" => {
                        progress_bar.set_minimum(expect_i64(value)? as i32);
                        Ok(())
                    }
                    "maximum" => {
                        progress_bar.set_maximum(expect_i64(value)? as i32);
                        Ok(())
                    }
                    "value" => {
                        progress_bar.set_value(expect_i64(value)? as i32);
                        Ok(())
                    }
                    "text_visible" => {
                        progress_bar.set_text_visible(expect_bool(value)?);
                        Ok(())
                    }
                    "orientation" => {
                        progress_bar.set_orientation(expect_orientation(value)?);
                        Ok(())
                    }
                    "inverted_appearance" => {
                        progress_bar.set_inverted_appearance(expect_bool(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::ScrollBar => {
            if let Some(scroll_bar) = widget_as_mut::<ScrollBar>(widget) {
                match property_name {
                    "minimum" => {
                        scroll_bar.set_minimum(expect_i64(value)? as i32);
                        Ok(())
                    }
                    "maximum" => {
                        scroll_bar.set_maximum(expect_i64(value)? as i32);
                        Ok(())
                    }
                    "value" => {
                        scroll_bar.set_value(expect_i64(value)? as i32);
                        Ok(())
                    }
                    "single_step" => {
                        scroll_bar.set_single_step(expect_i64(value)? as i32);
                        Ok(())
                    }
                    "page_step" => {
                        scroll_bar.set_page_step(expect_i64(value)? as i32);
                        Ok(())
                    }
                    "orientation" => {
                        scroll_bar.set_orientation(expect_orientation(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::ListBox => {
            if let Some(list_box) = widget_as_mut::<ListBox>(widget) {
                match property_name {
                    "selection_mode" => {
                        list_box.set_selection_mode(expect_list_box_selection_mode(value)?);
                        Ok(())
                    }
                    "current_row" => {
                        match value {
                            CapabilityValue::Null => list_box.set_current_row(None),
                            other => list_box.set_current_row(Some(expect_usize(other)?)),
                        }
                        Ok(())
                    }
                    "item_height" => {
                        list_box.set_item_height(expect_f32(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::SpinBox => {
            if let Some(spin_box) = widget_as_mut::<SpinBox>(widget) {
                match property_name {
                    "minimum" => {
                        spin_box.set_minimum(expect_i64(value)? as i32);
                        Ok(())
                    }
                    "maximum" => {
                        spin_box.set_maximum(expect_i64(value)? as i32);
                        Ok(())
                    }
                    "value" => {
                        spin_box.set_value(expect_i64(value)? as i32);
                        Ok(())
                    }
                    "single_step" => {
                        spin_box.set_single_step(expect_i64(value)? as i32);
                        Ok(())
                    }
                    "prefix" => {
                        spin_box.set_prefix(expect_string(value)?);
                        Ok(())
                    }
                    "suffix" => {
                        spin_box.set_suffix(expect_string(value)?);
                        Ok(())
                    }
                    "special_value_text" => {
                        match value {
                            CapabilityValue::Null => spin_box.set_special_value_text(None),
                            other => {
                                spin_box.set_special_value_text(Some(expect_string(other)?));
                            }
                        }
                        Ok(())
                    }
                    "wrapping" => {
                        spin_box.set_wrapping(expect_bool(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::ComboBox => {
            if let Some(combo_box) = widget_as_mut::<ComboBox>(widget) {
                match property_name {
                    "current_index" => {
                        match value {
                            CapabilityValue::Null => combo_box.set_current_index(None),
                            other => combo_box.set_current_index(Some(expect_usize(other)?)),
                        }
                        Ok(())
                    }
                    "current_text" => {
                        combo_box.set_current_text(expect_string(value)?);
                        Ok(())
                    }
                    "editable" => {
                        combo_box.set_editable(expect_bool(value)?);
                        Ok(())
                    }
                    "max_visible_items" => {
                        combo_box.set_max_visible_items(expect_usize(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::Dial => {
            if let Some(dial) = widget_as_mut::<Dial>(widget) {
                match property_name {
                    "minimum" => {
                        dial.set_minimum(expect_i64(value)? as i32);
                        Ok(())
                    }
                    "maximum" => {
                        dial.set_maximum(expect_i64(value)? as i32);
                        Ok(())
                    }
                    "value" => {
                        dial.set_value(expect_i64(value)? as i32);
                        Ok(())
                    }
                    "single_step" => {
                        dial.set_single_step(expect_i64(value)? as i32);
                        Ok(())
                    }
                    "page_step" => {
                        dial.set_page_step(expect_i64(value)? as i32);
                        Ok(())
                    }
                    "notches_visible" => {
                        dial.set_notches_visible(expect_bool(value)?);
                        Ok(())
                    }
                    "notch_target" => {
                        dial.set_notch_target(expect_f64(value)?);
                        Ok(())
                    }
                    "wrapping" => {
                        dial.set_wrapping(expect_bool(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::Window => {
            if let Some(window) = widget_as_mut::<Window>(widget) {
                match property_name {
                    "title" => {
                        window.set_title(expect_string(value)?);
                        Ok(())
                    }
                    "title_bar_height" => {
                        window.set_title_bar_height(expect_usize(value)? as u32);
                        Ok(())
                    }
                    "close_button_size" => {
                        window.set_close_button_size(expect_usize(value)? as u32);
                        Ok(())
                    }
                    "button_spacing" => {
                        window.set_button_spacing(expect_usize(value)? as u32);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::GroupBox => {
            if let Some(group_box) = widget_as_mut::<GroupBox>(widget) {
                match property_name {
                    "title" => {
                        group_box.set_title(expect_string(value)?);
                        Ok(())
                    }
                    "alignment" => {
                        group_box.set_alignment(expect_alignment(value)?);
                        Ok(())
                    }
                    "checkable" => {
                        group_box.set_checkable(expect_bool(value)?);
                        Ok(())
                    }
                    "checked" => {
                        group_box.set_checked(expect_bool(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::Splitter => {
            if let Some(splitter) = widget_as_mut::<Splitter>(widget) {
                match property_name {
                    "orientation" => {
                        splitter.set_orientation(expect_orientation(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::LCDNumber => {
            if let Some(lcd) = widget_as_mut::<LCDNumber>(widget) {
                match property_name {
                    "value" => {
                        lcd.set_value(expect_f64(value)?);
                        Ok(())
                    }
                    "min_value" => {
                        lcd.set_min_value(expect_f64(value)?);
                        Ok(())
                    }
                    "max_value" => {
                        lcd.set_max_value(expect_f64(value)?);
                        Ok(())
                    }
                    "num_digits" => {
                        lcd.set_num_digits(expect_i64(value)? as i32);
                        Ok(())
                    }
                    "small_decimal_point" => {
                        lcd.set_small_decimal_point(expect_bool(value)?);
                        Ok(())
                    }
                    "mode" => {
                        lcd.set_mode(expect_lcd_mode(value)?);
                        Ok(())
                    }
                    "segment_style" => {
                        lcd.set_segment_style(expect_segment_style(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::CommandLink => {
            if let Some(command_link) = widget_as_mut::<CommandLink>(widget) {
                match property_name {
                    "text" => {
                        command_link.set_text(expect_string(value)?);
                        Ok(())
                    }
                    "description" => {
                        command_link.set_description(expect_string(value)?);
                        Ok(())
                    }
                    "enabled" => {
                        command_link.set_enabled(expect_bool(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::FontComboBox => {
            if let Some(font_combo) = widget_as_mut::<FontComboBox>(widget) {
                match property_name {
                    "current_index" => {
                        font_combo.set_current_index(expect_i64(value)? as i32);
                        Ok(())
                    }
                    "editable" => {
                        font_combo.set_editable(expect_bool(value)?);
                        Ok(())
                    }
                    "max_visible_items" => {
                        font_combo.set_max_visible_items(expect_i64(value)? as i32);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::Action => {
            if let Some(action) = widget_as_mut::<Action>(widget) {
                match property_name {
                    "text" => {
                        action.set_text(expect_string(value)?);
                        Ok(())
                    }
                    "icon_text" => {
                        action.set_icon_text(expect_string(value)?);
                        Ok(())
                    }
                    "shortcut" => {
                        action.set_shortcut(expect_string(value)?);
                        Ok(())
                    }
                    "checkable" => {
                        action.set_checkable(expect_bool(value)?);
                        Ok(())
                    }
                    "checked" => {
                        action.set_checked(expect_bool(value)?);
                        Ok(())
                    }
                    "command_id" => {
                        match value {
                            CapabilityValue::Null => action.clear_command_id(),
                            other => action.set_command_id(expect_string(other)?),
                        }
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::Toolbox => {
            if let Some(tool_box) = widget_as_mut::<ToolBox>(widget) {
                match property_name {
                    "current_index" => {
                        tool_box.set_current_index(expect_usize(value)?);
                        Ok(())
                    }
                    "orientation" => {
                        tool_box.set_orientation(expect_orientation(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::TabBar => {
            if let Some(tab_bar) = widget_as_mut::<TabBar>(widget) {
                match property_name {
                    "current_index" => {
                        tab_bar.set_current_index(expect_usize(value)?);
                        Ok(())
                    }
                    "closable" => {
                        tab_bar.set_closable(expect_bool(value)?);
                        Ok(())
                    }
                    "movable" => {
                        tab_bar.set_movable(expect_bool(value)?);
                        Ok(())
                    }
                    "tab_min_width" => {
                        tab_bar.set_tab_min_width(expect_usize(value)? as u32);
                        Ok(())
                    }
                    "tab_max_width" => {
                        tab_bar.set_tab_max_width(expect_usize(value)? as u32);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::Calendar => {
            if let Some(calendar) = widget_as_mut::<Calendar>(widget) {
                match property_name {
                    "selected_date" => {
                        calendar.set_selected_date(expect_naive_date(value)?);
                        Ok(())
                    }
                    "minimum_date" => {
                        calendar.set_minimum_date(expect_naive_date(value)?);
                        Ok(())
                    }
                    "maximum_date" => {
                        calendar.set_maximum_date(expect_naive_date(value)?);
                        Ok(())
                    }
                    "first_day_of_week" => {
                        calendar.set_first_day_of_week(expect_weekday(value)?);
                        Ok(())
                    }
                    "grid_visible" => {
                        calendar.set_grid_visible(expect_bool(value)?);
                        Ok(())
                    }
                    "navigation_bar_visible" => {
                        calendar.set_navigation_bar_visible(expect_bool(value)?);
                        Ok(())
                    }
                    "horizontal_header_visible" => {
                        calendar.set_horizontal_header_visible(expect_bool(value)?);
                        Ok(())
                    }
                    "vertical_header_visible" => {
                        calendar.set_vertical_header_visible(expect_bool(value)?);
                        Ok(())
                    }
                    "date_format" => {
                        calendar.set_date_format(expect_string(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::DatePicker => {
            if let Some(date_edit) = widget_as_mut::<DateEdit>(widget) {
                match property_name {
                    "date" => {
                        date_edit.set_date(expect_date(value)?);
                        Ok(())
                    }
                    "minimum_date" => {
                        date_edit.set_minimum_date(expect_date(value)?);
                        Ok(())
                    }
                    "maximum_date" => {
                        date_edit.set_maximum_date(expect_date(value)?);
                        Ok(())
                    }
                    "display_format" => {
                        date_edit.set_display_format(expect_string(value)?);
                        Ok(())
                    }
                    "calendar_popup" => {
                        date_edit.set_calendar_popup(expect_bool(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::TimePicker => {
            if let Some(time_edit) = widget_as_mut::<TimeEdit>(widget) {
                match property_name {
                    "time" => {
                        time_edit.set_time(expect_time(value)?);
                        Ok(())
                    }
                    "minimum_time" => {
                        time_edit.set_minimum_time(expect_time(value)?);
                        Ok(())
                    }
                    "maximum_time" => {
                        time_edit.set_maximum_time(expect_time(value)?);
                        Ok(())
                    }
                    "display_format" => {
                        time_edit.set_display_format(expect_string(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::LineEdit => {
            if let Some(line_edit) = widget_as_mut::<LineEdit>(widget) {
                match property_name {
                    "text" => {
                        line_edit.set_text(expect_string(value)?);
                        Ok(())
                    }
                    "placeholder_text" => {
                        line_edit.set_placeholder_text(expect_string(value)?);
                        Ok(())
                    }
                    "max_length" => {
                        match value {
                            CapabilityValue::Null => line_edit.set_max_length(None),
                            other => line_edit.set_max_length(Some(expect_usize(other)?)),
                        }
                        Ok(())
                    }
                    "read_only" => {
                        line_edit.set_read_only(expect_bool(value)?);
                        Ok(())
                    }
                    "cursor_position" => {
                        line_edit.set_cursor_position(expect_usize(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::ListView => {
            if let Some(list_view) = widget_as_mut::<ListView>(widget) {
                match property_name {
                    "focused_row" => match value {
                        CapabilityValue::Null => {
                            list_view.clear_focused_row();
                            Ok(())
                        }
                        other => {
                            let row = expect_usize(other)?;
                            if list_view.set_focused_row(row) {
                                Ok(())
                            } else {
                                Err(CapabilityAccessError::UnsupportedOnWidget)
                            }
                        }
                    },
                    "selection_mode" => {
                        list_view.set_selection_mode(expect_selection_mode(value)?);
                        Ok(())
                    }
                    "view_mode" => {
                        list_view.set_view_mode(expect_view_mode(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::TreeView => {
            if let Some(tree_table) = widget_as_mut::<TreeTable>(widget) {
                match property_name {
                    "selected_row" => match value {
                        CapabilityValue::Null => {
                            if let Some(selected) = tree_table.selected_row() {
                                let _ = tree_table.select_row(selected);
                            }
                            Ok(())
                        }
                        other => {
                            let row = expect_usize(other)?;
                            if tree_table.select_row(row) || tree_table.row_count() == 0 {
                                Ok(())
                            } else {
                                Err(CapabilityAccessError::UnsupportedOnWidget)
                            }
                        }
                    },
                    "row_height" => {
                        tree_table.set_row_height(expect_u32(value)?);
                        Ok(())
                    }
                    "column_width" => {
                        tree_table.set_column_width(expect_u32(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else if let Some(tree_view) = widget_as_mut::<TreeView>(widget) {
                match property_name {
                    "focused_node" => match value {
                        CapabilityValue::Null => {
                            tree_view.clear_focused_node();
                            Ok(())
                        }
                        other => {
                            let node = expect_usize(other)?;
                            if tree_view.set_focused_node(node) {
                                Ok(())
                            } else {
                                Err(CapabilityAccessError::UnsupportedOnWidget)
                            }
                        }
                    },
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::Menu => {
            if let Some(menu) = widget_as_mut::<Menu>(widget) {
                match property_name {
                    "title" => {
                        menu.set_title(expect_string(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::DataView => {
            if let Some(virtual_list) = widget_as_mut::<VirtualList>(widget) {
                match property_name {
                    "scroll_row" => {
                        virtual_list.set_scroll_row(expect_usize(value)?);
                        Ok(())
                    }
                    "row_height" => {
                        let row_height = expect_u32(value)?;
                        virtual_list.set_row_height(row_height);
                        Ok(())
                    }
                    "overscan" => {
                        virtual_list.set_overscan(expect_usize(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::Table => {
            if let Some(data_grid) = widget_as_mut::<DataGrid>(widget) {
                match property_name {
                    "scroll_row" => {
                        data_grid.set_scroll_row(expect_usize(value)?);
                        Ok(())
                    }
                    "scroll_column" => {
                        data_grid.set_scroll_column(expect_usize(value)?);
                        Ok(())
                    }
                    "row_height" => {
                        data_grid.set_row_height(expect_u32(value)?);
                        Ok(())
                    }
                    "column_width" => {
                        data_grid.set_column_width(expect_u32(value)?);
                        Ok(())
                    }
                    "frozen_columns" => {
                        data_grid.set_frozen_columns(expect_usize(value)?);
                        Ok(())
                    }
                    "sort_specs" => {
                        data_grid.set_sort_specs(expect_sort_specs(value)?);
                        Ok(())
                    }
                    "filters" => {
                        data_grid.set_filters(expect_column_filters(value)?);
                        Ok(())
                    }
                    "sort_spec_count" | "filter_count" | "visible_window" => {
                        Err(CapabilityAccessError::ReadOnlyProperty)
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else if let Some(virtual_table) = widget_as_mut::<VirtualTable>(widget) {
                match property_name {
                    "scroll_row" => {
                        virtual_table.set_scroll_row(expect_usize(value)?);
                        Ok(())
                    }
                    "scroll_column" => {
                        virtual_table.set_scroll_column(expect_usize(value)?);
                        Ok(())
                    }
                    "row_height" => {
                        virtual_table.set_row_height(expect_u32(value)?);
                        Ok(())
                    }
                    "column_width" => {
                        virtual_table.set_column_width(expect_u32(value)?);
                        Ok(())
                    }
                    "overscan_rows" => {
                        virtual_table.set_overscan_rows(expect_usize(value)?);
                        Ok(())
                    }
                    "overscan_columns" => {
                        virtual_table.set_overscan_columns(expect_usize(value)?);
                        Ok(())
                    }
                    "visible_window" => Err(CapabilityAccessError::ReadOnlyProperty),
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else if let Some(table_widget) = widget_as_mut::<TableWidget>(widget) {
                match property_name {
                    "selection_mode" => {
                        table_widget.set_selection_mode(expect_selection_mode(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::ToolBar => {
            if let Some(tool_bar) = widget_as_mut::<ToolBar>(widget) {
                match property_name {
                    "movable" => {
                        tool_bar.set_movable(expect_bool(value)?);
                        Ok(())
                    }
                    "floatable" => {
                        tool_bar.set_floatable(expect_bool(value)?);
                        Ok(())
                    }
                    "icon_size" => {
                        tool_bar.set_icon_size(expect_f32(value)?);
                        Ok(())
                    }
                    "orientation" => {
                        tool_bar.set_orientation(expect_toolbar_orientation(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::RibbonBar => {
            if let Some(ribbon_bar) = widget_as_mut::<RibbonBar>(widget) {
                match property_name {
                    "current_tab" => {
                        ribbon_bar.set_current_tab(expect_usize(value)?);
                        Ok(())
                    }
                    "expanded" => {
                        ribbon_bar.set_expanded(expect_bool(value)?);
                        Ok(())
                    }
                    "minimized" => {
                        ribbon_bar.set_minimized(expect_bool(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::ColorDialog => {
            if let Some(color_picker) = widget_as_mut::<ColorPicker>(widget) {
                match property_name {
                    "hex_rgba" => {
                        let hex = expect_string(value)?;
                        if color_picker.set_hex(&hex) {
                            Ok(())
                        } else {
                            Err(CapabilityAccessError::TypeMismatch)
                        }
                    }
                    "show_alpha" => {
                        color_picker.set_show_alpha(expect_bool(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::RichEdit => {
            if let Some(code_editor) = widget_as_mut::<CodeEditor>(widget) {
                match property_name {
                    "text" => {
                        code_editor.set_text(expect_string(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::Chart => {
            if let Some(gantt_widget) = widget_as_mut::<GanttWidget>(widget) {
                match property_name {
                    "viewport_start" => {
                        let start = expect_i64(value)?;
                        let (_, end) = gantt_widget.viewport();
                        gantt_widget.set_viewport(start, end);
                        Ok(())
                    }
                    "viewport_end" => {
                        let end = expect_i64(value)?;
                        let (start, _) = gantt_widget.viewport();
                        gantt_widget.set_viewport(start, end);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::TextEdit => {
            if let Some(terminal_view) = widget_as_mut::<TerminalView>(widget) {
                match property_name {
                    "input_line" => {
                        terminal_view.set_input_line(expect_string(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }

        WidgetKind::Canvas => {
            if let Some(map_view) = widget_as_mut::<MapView>(widget) {
                match property_name {
                    "center_x" => {
                        let x = expect_f32(value)?;
                        let (_, y) = map_view.center();
                        map_view.set_center(x, y);
                        Ok(())
                    }
                    "center_y" => {
                        let y = expect_f32(value)?;
                        let (x, _) = map_view.center();
                        map_view.set_center(x, y);
                        Ok(())
                    }
                    "zoom" => {
                        map_view.set_zoom(expect_f32(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::WebEngineView => {
            if let Some(media_player) = widget_as_mut::<MediaPlayer>(widget) {
                match property_name {
                    "source" => match value {
                        CapabilityValue::Null => {
                            media_player.clear_source();
                            Ok(())
                        }
                        other => {
                            let source = expect_string(other)?;
                            let duration = media_player.duration_ms();
                            media_player.set_source(source, duration);
                            Ok(())
                        }
                    },
                    "playing" => {
                        if expect_bool(value)? {
                            let _ = media_player.play();
                        } else {
                            media_player.pause();
                        }
                        Ok(())
                    }
                    "position_ms" => {
                        media_player.seek_to(expect_usize(value)? as u64);
                        Ok(())
                    }
                    "volume" => {
                        media_player.set_volume(expect_u32(value)? as u8);
                        Ok(())
                    }
                    "muted" => {
                        media_player.set_muted(expect_bool(value)?);
                        Ok(())
                    }
                    "fullscreen" => {
                        media_player.set_fullscreen(expect_bool(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }

        WidgetKind::CheckListBox => {
            if let Some(chip) = widget_as_mut::<Chip>(widget) {
                match property_name {
                    "multi_select" => {
                        chip.set_multi_select(expect_bool(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::Grid => {
            if let Some(grid) = widget_as_mut::<GridWidget>(widget) {
                match property_name {
                    "rows" => {
                        grid.set_rows(expect_u32(value)?);
                        Ok(())
                    }
                    "columns" => {
                        grid.set_columns(expect_u32(value)?);
                        Ok(())
                    }
                    "spacing" => {
                        grid.set_spacing(expect_u32(value)?);
                        Ok(())
                    }
                    "line_color" => {
                        match value {
                            CapabilityValue::Null => grid.set_line_color(None),
                            CapabilityValue::String(raw) => {
                                let Some(color) = crate::core::Color::parse_hex(&raw) else {
                                    return Err(CapabilityAccessError::TypeMismatch);
                                };
                                grid.set_line_color(Some(color));
                            }
                            _ => return Err(CapabilityAccessError::TypeMismatch),
                        }
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::FreeformShape => {
            if let Some(shape) = widget_as_mut::<FreeformShapeWidget>(widget) {
                match property_name {
                    "fill_rgba" => {
                        let raw = expect_string(value)?;
                        let Some(color) = crate::core::Color::parse_hex(&raw) else {
                            return Err(CapabilityAccessError::TypeMismatch);
                        };
                        shape.set_fill_color(color);
                        Ok(())
                    }
                    "stroke_rgba" => {
                        match value {
                            CapabilityValue::Null => shape.set_stroke_color(None),
                            CapabilityValue::String(raw) => {
                                let Some(color) = crate::core::Color::parse_hex(&raw) else {
                                    return Err(CapabilityAccessError::TypeMismatch);
                                };
                                shape.set_stroke_color(Some(color));
                            }
                            _ => return Err(CapabilityAccessError::TypeMismatch),
                        }
                        Ok(())
                    }
                    "stroke_width" => {
                        shape.set_stroke_width(expect_u32(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        // ── Always-available widget writes (not mini-gated) ────────
        WidgetKind::ToggleButton => {
            if let Some(w) = widget_as_mut::<ToggleButton>(widget) {
                match property_name {
                    "text" => {
                        w.set_text(expect_string(value)?);
                        Ok(())
                    }
                    "checked" => {
                        w.set_checked(expect_bool(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::Arc => {
            if let Some(w) = widget_as_mut::<Arc>(widget) {
                match property_name {
                    "value" => {
                        w.set_value(expect_u32(value)?);
                        Ok(())
                    }
                    "thickness" => {
                        w.set_thickness(expect_u32(value)?);
                        Ok(())
                    }
                    "sweep_angle" => {
                        w.set_sweep_angle(expect_u32(value)? as u16);
                        Ok(())
                    }
                    "indeterminate" => {
                        w.set_indeterminate(expect_bool(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::Spinner => {
            if let Some(w) = widget_as_mut::<Spinner>(widget) {
                match property_name {
                    "active" => {
                        w.set_active(expect_bool(value)?);
                        Ok(())
                    }
                    "thickness" => {
                        w.set_thickness(expect_u32(value)?);
                        Ok(())
                    }
                    "speed" => {
                        w.set_speed(expect_f32(value)?);
                        Ok(())
                    }
                    "size_ratio" => {
                        w.set_size_ratio(expect_f32(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::Roller => {
            if let Some(w) = widget_as_mut::<Roller>(widget) {
                match property_name {
                    "selected_index" => {
                        w.set_selected_index(expect_usize(value)?);
                        Ok(())
                    }
                    "visible_count" => {
                        w.set_visible_count(expect_u32(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::Dropdown => {
            if let Some(w) = widget_as_mut::<Dropdown>(widget) {
                match property_name {
                    "selected_index" => {
                        w.set_selected_index(expect_usize(value)?);
                        Ok(())
                    }
                    "expanded" => {
                        w.set_expanded(expect_bool(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::TextArea => {
            if let Some(w) = widget_as_mut::<TextArea>(widget) {
                match property_name {
                    "text" => {
                        w.set_text(expect_string(value)?);
                        Ok(())
                    }
                    "placeholder" => {
                        w.set_placeholder(expect_string(value)?);
                        Ok(())
                    }
                    "read_only" => {
                        w.set_read_only(expect_bool(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::Keyboard => {
            if let Some(w) = widget_as_mut::<Keyboard>(widget) {
                match property_name {
                    "layout" => {
                        let s = expect_string(value)?;
                        let layout = match s.as_str() {
                            "qwerty" => KeyboardLayout::Qwerty,
                            "numeric" => KeyboardLayout::Numeric,
                            _ => return Err(CapabilityAccessError::TypeMismatch),
                        };
                        w.set_layout(layout);
                        Ok(())
                    }
                    "lowercase" => {
                        w.set_lowercase(expect_bool(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::Switch => {
            if let Some(w) = widget_as_mut::<Switch>(widget) {
                match property_name {
                    "checked" => {
                        w.set_checked(expect_bool(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::Line => {
            if let Some(w) = widget_as_mut::<Line>(widget) {
                match property_name {
                    "orientation" => {
                        let s = expect_string(value)?;
                        let ori = match s.as_str() {
                            "horizontal" => LineOrientation::Horizontal,
                            "vertical" => LineOrientation::Vertical,
                            _ => return Err(CapabilityAccessError::TypeMismatch),
                        };
                        w.set_orientation(ori);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::Meter => {
            if let Some(w) = widget_as_mut::<Meter>(widget) {
                match property_name {
                    "value" => {
                        w.set_value(expect_u32(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::MiniChart => {
            if let Some(w) = widget_as_mut::<MiniChart>(widget) {
                match property_name {
                    "chart_type" => {
                        let s = expect_string(value)?;
                        let ct = match s.as_str() {
                            "line" => ChartType::Line,
                            "bar" => ChartType::Bar,
                            _ => return Err(CapabilityAccessError::TypeMismatch),
                        };
                        w.set_chart_type(ct);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::ImageView => {
            if let Some(w) = widget_as_mut::<ImageView>(widget) {
                match property_name {
                    "scaled" => {
                        w.set_scaled(expect_bool(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::TileView => {
            if let Some(w) = widget_as_mut::<TileView>(widget) {
                match property_name {
                    "current_page" => {
                        w.set_current_page(expect_u32(value)?);
                        Ok(())
                    }
                    "page_count" => {
                        w.set_page_count(expect_u32(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }

        // ── New widgets (menu/toolbar) ───────────────────────────
        WidgetKind::ToolButton => {
            if let Some(w) = widget_as_mut::<ToolButton>(widget) {
                match property_name {
                    "text" => {
                        w.set_text(expect_string(value)?);
                        Ok(())
                    }
                    "checked" => {
                        w.set_checked(expect_bool(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::StatusBar => {
            if let Some(w) = widget_as_mut::<StatusBar>(widget) {
                match property_name {
                    "message" => {
                        w.show_message(expect_string(value)?, 0);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }

        // ── New widgets (input) ────────────────────────────────────
        WidgetKind::SearchBar => {
            if let Some(w) = widget_as_mut::<SearchBar>(widget) {
                match property_name {
                    "text" => {
                        w.set_text(expect_string(value)?);
                        Ok(())
                    }
                    "placeholder" => {
                        w.set_placeholder(expect_string(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::ShortcutEditor => {
            if let Some(w) = widget_as_mut::<ShortcutEditor>(widget) {
                match property_name {
                    "filter_text" => {
                        let text = expect_string(value)?;
                        w.set_filter(&text);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }

        // ── New widgets (navigation) ───────────────────────────────
        WidgetKind::TabView => {
            if let Some(w) = widget_as_mut::<TabView>(widget) {
                match property_name {
                    "selected_index" => {
                        w.set_current_index(expect_usize(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::MaterialNavigationRail => {
            if let Some(w) = widget_as_mut::<MaterialNavigationRail>(widget) {
                match property_name {
                    "selected_index" => {
                        w.set_selected(expect_usize(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }

        // ── New widgets (container) ─────────────────────────────────
        WidgetKind::PagerPageView => {
            if let Some(w) = widget_as_mut::<PagerPageView>(widget) {
                match property_name {
                    "current_page" => {
                        w.set_current_page(expect_usize(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }

        // ── New widgets (overlay) ───────────────────────────────────
        WidgetKind::SwipeToDismiss => {
            if let Some(_w) = widget_as_mut::<SwipeToDismiss>(widget) {
                match property_name {
                    "is_dismissed" => Err(CapabilityAccessError::UnsupportedOnWidget),
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }

        // ── New widgets (chart) ─────────────────────────────────────
        WidgetKind::LineChart => {
            if let Some(w) = widget_as_mut::<LineChart>(widget) {
                match property_name {
                    "stroke_width" => {
                        w.set_stroke_width(expect_f32(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::Sparkline => {
            if let Some(w) = widget_as_mut::<Sparkline>(widget) {
                match property_name {
                    "stroke_width" => {
                        w.set_stroke_width(expect_f32(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::BarChart => {
            if let Some(w) = widget_as_mut::<BarChart>(widget) {
                match property_name {
                    "bar_spacing" => {
                        w.set_bar_spacing(expect_f32(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::PieChart => {
            if let Some(w) = widget_as_mut::<PieChart>(widget) {
                match property_name {
                    "donut" => {
                        w.set_donut_mode(expect_bool(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }

        // ── New widgets (media/animation) ───────────────────────────
        WidgetKind::AnimatedImage => {
            if let Some(w) = widget_as_mut::<AnimatedImage>(widget) {
                match property_name {
                    "playing" => {
                        if expect_bool(value)? {
                            w.play();
                        } else {
                            w.pause();
                        }
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::HeroAnimation => {
            if let Some(w) = widget_as_mut::<HeroAnimation>(widget) {
                match property_name {
                    "animation_progress" => {
                        w.set_progress(expect_f32(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::LottieWidget => {
            if let Some(w) = widget_as_mut::<LottieWidget>(widget) {
                match property_name {
                    "playing" => {
                        if expect_bool(value)? {
                            w.play();
                        } else {
                            w.pause();
                        }
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::RiveWidget => {
            if let Some(w) = widget_as_mut::<RiveWidget>(widget) {
                match property_name {
                    "is_playing" => {
                        if expect_bool(value)? {
                            w.play();
                        } else {
                            w.pause();
                        }
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::VideoPlayer => {
            if let Some(w) = widget_as_mut::<VideoPlayer>(widget) {
                match property_name {
                    "is_playing" => {
                        if expect_bool(value)? {
                            w.play();
                        } else {
                            w.pause();
                        }
                        Ok(())
                    }
                    "volume" => {
                        w.set_volume(expect_f32(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }

        // ── New widgets (view) ──────────────────────────────────────
        WidgetKind::ImageGallery => {
            if let Some(w) = widget_as_mut::<ImageGallery>(widget) {
                match property_name {
                    "current_index" => {
                        w.set_current_index(expect_usize(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }

        // ── New widgets (view / property) ───────────────────────────
        WidgetKind::PropertyGrid => {
            if let Some(w) = widget_as_mut::<PropertyGrid>(widget) {
                match property_name {
                    "selected_index" => {
                        match value {
                            CapabilityValue::Null => w.set_selected_index(None),
                            other => w.set_selected_index(Some(expect_usize(other)?)),
                        }
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }

        // ── New widgets (misc) ──────────────────────────────────────
        WidgetKind::AudioVisualizer => {
            if let Some(w) = widget_as_mut::<AudioVisualizer>(widget) {
                match property_name {
                    "bar_count" => {
                        w.set_bar_count(expect_usize(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::CameraPreview => {
            if let Some(w) = widget_as_mut::<CameraPreview>(widget) {
                match property_name {
                    "is_active" => {
                        if expect_bool(value)? {
                            w.start_preview();
                        } else {
                            w.stop_preview();
                        }
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::BarcodeScanner => {
            if let Some(w) = widget_as_mut::<BarcodeScanner>(widget) {
                match property_name {
                    "is_scanning" => {
                        if expect_bool(value)? {
                            w.start_scanning();
                        } else {
                            w.stop_scanning();
                        }
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::BezierCurveEditor => {
            if let Some(w) = widget_as_mut::<BezierCurveEditor>(widget) {
                match property_name {
                    "snap_to_grid" => {
                        w.set_snap_to_grid(expect_bool(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::CupertinoSlider => {
            if let Some(w) = widget_as_mut::<CupertinoSlider>(widget) {
                match property_name {
                    "value" => {
                        w.set_value(expect_f32(value)?);
                        Ok(())
                    }
                    "min" => {
                        w.set_min(expect_f32(value)?);
                        Ok(())
                    }
                    "max" => {
                        w.set_max(expect_f32(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }

        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
    }
}

#[cfg(feature = "mini")]
pub fn read_widget_property_value(
    _widget: &dyn Widget,
    _property_name: &str,
) -> Result<CapabilityValue, CapabilityAccessError> {
    Err(CapabilityAccessError::UnsupportedOnWidget)
}

#[cfg(feature = "mini")]
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

#[cfg(not(feature = "mini"))]
pub fn date_to_string(date: Date) -> String {
    date.to_string()
}

#[cfg(not(feature = "mini"))]
pub fn time_to_string(time: Time) -> String {
    time.to_string()
}

// ---------------------------------------------------------------------------
// Default property value lookup
// ---------------------------------------------------------------------------

#[cfg(not(feature = "mini"))]
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

#[cfg(feature = "mini")]
pub fn default_widget_property_value(
    _kind: WidgetKind,
    _property_name: &str,
) -> Option<CapabilityValue> {
    None
}
