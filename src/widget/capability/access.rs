// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Id-level property access and the value/name codecs the schema layer publishes.
//!
//! Backends hold widget **ids**, not `&dyn Widget`, so [`read_widget_property_by_id`]
//! and [`write_widget_property_by_id`] are the entry points they use. Resolving the
//! id through [`crate::widget::runtime`] means the answer always describes the live
//! control rather than a copy (BLUE15 §10.3).
//!
//! # History
//!
//! This module used to also host `read_widget_property_legacy` /
//! `write_widget_property_legacy`: two large match-on-`widget.kind()` functions that
//! downcast the trait object and call native getters and setters, as a fallback for
//! controls whose readers had not yet moved onto their own `WidgetProperties`
//! impl (BLUE15 Phase C-1). Every registered control now implements the contract and
//! a test asserts it, so those two functions — and the nine `access_read_*.in.rs` /
//! `access_write_*.in.rs` arms they dispatched over — duplicated the contract for
//! 39 controls and have been deleted rather than left as dead code.
//!
//! What remains are the shared codecs below: they convert the enum-backed property
//! values to and from the strings the schema publishes, and several controls call
//! them directly from their own `set` / `get` arms.

/// The reflection entry points, re-exported so the id-level accessors in this module
/// resolve them from one place.
pub use super::properties_trait::{read_widget_property_by_name, write_widget_property_by_name};

#[cfg(full_widgets)]
use chrono::Weekday;

use crate::core::ObjectId;

#[cfg(full_widgets)]
use crate::widget::advanced_widgets::date_edit::Date;
#[cfg(full_widgets)]
use crate::widget::advanced_widgets::time_edit::Time;
#[cfg(full_widgets)]
use crate::widget::capability::coercion::*;
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::container_widgets::scrollarea::ScrollBarPolicy;
use crate::widget::input_widgets::listbox::SelectionMode as ListBoxSelectionMode;
#[cfg(full_widgets)]
use crate::widget::menu_toolbar::tool_bar::ToolBarOrientation;
#[cfg(full_widgets)]
use crate::widget::view_widgets::data_grid::{ColumnFilter, SortSpec};
#[cfg(full_widgets)]
use crate::widget::view_widgets::list_view::{SelectionMode, ViewMode};
#[cfg(full_widgets)]
use crate::widget::WidgetKind;

/// Reads a property from the widget registered under `widget_id`.
///
/// The id-level counterpart to [`read_widget_property_by_name`]. Backends hold
/// widget **ids**, not `&dyn Widget`, so they need this shape; resolving the id
/// through [`crate::widget::runtime`] also means the answer always describes the
/// live control rather than a copy (BLUE15 §10.3).
///
/// Returns [`CapabilityAccessError::UnknownWidget`] when the id addresses nothing.
pub fn read_widget_property_by_id(
    widget_id: ObjectId,
    property_name: &str,
) -> Result<CapabilityValue, CapabilityAccessError> {
    crate::widget::runtime::with_widget(widget_id, |widget| {
        read_widget_property_by_name(widget, property_name)
    })
    .unwrap_or(Err(CapabilityAccessError::UnknownWidget))
}

/// Writes a property to the widget registered under `widget_id`.
///
/// See [`read_widget_property_by_id`]. Asks the platform to repaint on success,
/// because a property write that changes what the control looks like must not
/// leave a stale frame on screen.
pub fn write_widget_property_by_id(
    widget_id: ObjectId,
    property_name: &str,
    value: CapabilityValue,
) -> Result<(), CapabilityAccessError> {
    let written = crate::widget::runtime::with_widget_mut(widget_id, |widget| {
        write_widget_property_by_name(widget, property_name, value)
    })
    .unwrap_or(Err(CapabilityAccessError::UnknownWidget));
    if written.is_ok() {
        crate::widget::runtime::request_repaint(widget_id);
    }
    written
}

// ---------------------------------------------------------------------------
// Helper to-str / to-string conversions
// ---------------------------------------------------------------------------

#[cfg(full_widgets)]
pub fn sort_specs_to_string(sort_specs: &[SortSpec]) -> String {
    sort_specs
        .iter()
        .map(|spec| format!("{}:{}", spec.column, if spec.descending { "desc" } else { "asc" }))
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(full_widgets)]
pub fn column_filters_to_string(filters: &[ColumnFilter]) -> String {
    filters
        .iter()
        .map(|filter| format!("{}={}", filter.column, filter.query))
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(full_widgets)]
pub fn selection_mode_to_str(mode: SelectionMode) -> &'static str {
    match mode {
        SelectionMode::Single => "single",
        SelectionMode::Multi => "multi",
        SelectionMode::Extended => "extended",
        SelectionMode::None => "none",
    }
}

/// The published token for a list-box selection mode.
///
/// `ListBox`'s `SelectionMode` is a type alias of the list view's, so this is a
/// spelling of the same mapping, not a second one — it delegates rather than
/// repeating the match (principle #54). It is available wherever `ListBox` is,
/// which is why it is not gated with the list-view-only converters above.
pub fn list_box_selection_mode_to_str(mode: ListBoxSelectionMode) -> &'static str {
    match mode {
        ListBoxSelectionMode::Single => "single",
        ListBoxSelectionMode::Multi => "multi",
        ListBoxSelectionMode::Extended => "extended",
        ListBoxSelectionMode::None => "none",
    }
}

#[cfg(full_widgets)]
pub fn view_mode_to_str(mode: ViewMode) -> &'static str {
    match mode {
        ViewMode::List => "list",
        ViewMode::Icon => "icon",
        ViewMode::Details => "details",
        ViewMode::Thumbnails => "thumbnails",
    }
}

#[cfg(full_widgets)]
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

pub use super::coercion::{
    alignment_to_str, check_state_to_str, lcd_mode_to_str, orientation_to_str,
    segment_style_to_str, tick_position_to_str,
};

#[cfg(full_widgets)]
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

#[cfg(full_widgets)]
pub fn date_to_string(date: Date) -> String {
    date.to_string()
}

#[cfg(full_widgets)]
pub fn time_to_string(time: Time) -> String {
    time.to_string()
}

// ---------------------------------------------------------------------------
// Default property value lookup
// ---------------------------------------------------------------------------

#[cfg(full_widgets)]
pub fn default_widget_property_default_value(
    kind: WidgetKind,
    property_name: &str,
) -> Option<CapabilityValue> {
    // The properties every control inherits from `BaseWidget` get their defaults
    // here, once, rather than being repeated in every kind's arm below. Every
    // schema declares them (they are part of each control's published contract),
    // so a per-kind arm could only ever be a copy of these values.
    match property_name {
        "enabled" => return Some(CapabilityValue::Bool(true)),
        "tooltip" => return Some(CapabilityValue::String(String::new())),
        "geometry" => {
            return Some(CapabilityValue::String("0,0,0,0".to_string()));
        }
        // `visible` is shared by every control and declared by every schema, but
        // four kinds give the name their own meaning: `Tooltip`, `Popover` and
        // `ModalBottomSheet` read it as "the popup is shown", `StatusBar` as "the
        // status text is shown". Those must fall through to their own arm below,
        // so they are excluded here; every other kind means the base widget's
        // visibility, which defaults to shown.
        "visible"
            if !matches!(
                kind,
                WidgetKind::Tooltip
                    | WidgetKind::Popover
                    | WidgetKind::ModalBottomSheet
                    | WidgetKind::StatusBar
            ) =>
        {
            return Some(CapabilityValue::Bool(true));
        }
        _ => {}
    }

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
            "current_index" => CapabilityValue::UInt(0),
            "item_count" => CapabilityValue::UInt(0),
            "current_page_title" => CapabilityValue::String(String::new()),
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
        // A `Chip` declares these; `CheckListBox` is a *type alias for `ListBox`* and
        // is served by the `ListBox` arm below. The two were conflated before, which
        // left `WidgetKind::Chip` with no capability and made the list-box lookup
        // ambiguous.
        WidgetKind::Chip => match property_name {
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
            "title" => CapabilityValue::String(String::new()),
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
            "shape" => CapabilityValue::String("rect".to_string()),
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
            "title" => CapabilityValue::String(String::new()),
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
            "shown" => CapabilityValue::Bool(false),
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
            "shown" => CapabilityValue::Bool(false),
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

#[cfg(stripped_widgets)]
pub fn default_widget_property_default_value(
    _kind: WidgetKind,
    _property_name: &str,
) -> Option<CapabilityValue> {
    None
}
