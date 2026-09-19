// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Id-level property access and the value/name codecs the schema layer publishes.
//!
//! Backends hold widget **ids**, not `&dyn Widget`, so [`read_widget_property_by_id`]
//! and [`write_widget_property_by_id`] are the entry points they use. Resolving the
//! id through `crate::widget::runtime` means the answer always describes the live
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
/// through `crate::widget::runtime` also means the answer always describes the
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
/// Encodes a data-grid sort specification as a single string for the property
/// layer.
///
/// Each entry becomes `"<column>:asc"` or `"<column>:desc"`, joined with `,`.
/// The column *name* is written verbatim, so a name containing `:` or `,`
/// produces an ambiguous encoding that a reader cannot reliably split.
/// Empty input yields an empty string.
pub fn sort_specs_to_string(sort_specs: &[SortSpec]) -> String {
    sort_specs
        .iter()
        .map(|spec| format!("{}:{}", spec.column, if spec.descending { "desc" } else { "asc" }))
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(full_widgets)]
/// Encodes data-grid column filters as a single `"<column>=<query>"` string,
/// joined with `,`.
///
/// As with [`sort_specs_to_string`], `=` and `,` inside a column name or query
/// make the result ambiguous. Empty input yields an empty string.
pub fn column_filters_to_string(filters: &[ColumnFilter]) -> String {
    filters
        .iter()
        .map(|filter| format!("{}={}", filter.column, filter.query))
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(full_widgets)]
/// Returns the published token for a list-view selection mode: one of
/// `"single"`, `"multi"`, `"extended"`, or `"none"`.
///
/// These strings are part of the property schema and are matched by consumers,
/// so they must not be reworded.
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
/// Returns the published token for a list-view display mode: one of `"list"`,
/// `"icon"`, `"details"`, or `"thumbnails"`.
///
/// `thumbnails` exists in the codec even though the icon view may not render a
/// distinct thumbnail layout; the token is still valid to round-trip.
pub fn view_mode_to_str(mode: ViewMode) -> &'static str {
    match mode {
        ViewMode::List => "list",
        ViewMode::Icon => "icon",
        ViewMode::Details => "details",
        ViewMode::Thumbnails => "thumbnails",
    }
}

#[cfg(full_widgets)]
/// Returns the published token for a toolbar orientation: `"horizontal"` or
/// `"vertical"`.
pub fn tool_bar_orientation_to_str(orientation: ToolBarOrientation) -> &'static str {
    match orientation {
        ToolBarOrientation::Horizontal => "horizontal",
        ToolBarOrientation::Vertical => "vertical",
    }
}

/// Returns the published token for a scroll bar policy: `"always_on"`,
/// `"always_off"`, or `"as_needed"`.
///
/// Note the underscore spelling, unlike the other codecs in this module which
/// use single words — these tokens are the schema's, not a convention.
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
/// Returns the published three-letter token for a weekday: `"mon"` through
/// `"sun"`.
///
/// Lowercase and fixed-length; unlike locale-aware day names, these are stable
/// identifiers suitable for storing and matching.
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
/// Formats a [`Date`] as its `Display` representation for the property layer.
/// The exact layout is the date type's, not fixed here.
pub fn date_to_string(date: Date) -> String {
    date.to_string()
}

#[cfg(full_widgets)]
/// Formats a [`Time`] as its `Display` representation for the property layer.
/// The exact layout is the time type's, not fixed here.
pub fn time_to_string(time: Time) -> String {
    time.to_string()
}

// ---------------------------------------------------------------------------
// Default property value lookup
// ---------------------------------------------------------------------------

#[cfg(full_widgets)]
/// Looks up the value a property reports before anything has written it.
///
/// Returns `None` when the kind/property pair is unknown to the schema, so a
/// `None` here means "no default is declared", not "the default is null".
/// Values are produced fresh on each call; enum-backed properties are returned
/// in their published string form.
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
        WidgetKind::Frame => match property_name {
            // Mirrors `Frame::new`'s field initialisers, so a freshly constructed
            // frame and the schema's defaults agree.
            "frame_shape" => CapabilityValue::String("box".to_string()),
            "frame_shadow" => CapabilityValue::String("plain".to_string()),
            "line_width" => CapabilityValue::Float(1.0),
            "mid_line_width" => CapabilityValue::Float(0.0),
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
            // The names below belong to the capabilities that share this kind
            // (`command_palette`, `notification_center`). Defaults are keyed by
            // kind, so the arm must answer for every capability that reports it;
            // each control's own `get` answers its real value.
            "query" => CapabilityValue::String(String::new()),
            "entry_count" => CapabilityValue::UInt(0),
            "filtered_count" => CapabilityValue::UInt(0),
            "highlighted_index" => CapabilityValue::Null,
            "item_count" => CapabilityValue::UInt(0),
            "unread_count" => CapabilityValue::UInt(0),
            "selected_index" => CapabilityValue::Null,
            "row_height" => CapabilityValue::UInt(24),
            _ => return None,
        },
        WidgetKind::TreeView => match property_name {
            "has_model" => CapabilityValue::Bool(false),
            "node_count" => CapabilityValue::UInt(0),
            "focused_node" => CapabilityValue::Null,
            "selected_node" => CapabilityValue::Null,
            _ => return None,
        },
        WidgetKind::TreeTable => match property_name {
            "has_model" => CapabilityValue::Bool(false),
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
            // `diff_viewer` shares this kind; see the `ListView` note above.
            "left_text" => CapabilityValue::String(String::new()),
            "right_text" => CapabilityValue::String(String::new()),
            "line_count" => CapabilityValue::UInt(0),
            "change_count" => CapabilityValue::UInt(0),
            "selected_index" => CapabilityValue::Null,
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
        WidgetKind::GridTable => match property_name {
            "has_data_source" => CapabilityValue::Bool(false),
            "row_count" => CapabilityValue::UInt(0),
            "column_count" => CapabilityValue::UInt(0),
            "scroll_row" => CapabilityValue::UInt(0),
            "scroll_column" => CapabilityValue::UInt(0),
            "row_height" => CapabilityValue::UInt(24),
            "selection_mode" => CapabilityValue::String("cell".to_string()),
            "sort_spec_count" => CapabilityValue::UInt(0),
            "selected_cell" => CapabilityValue::Null,
            _ => return None,
        },
        WidgetKind::NumberPicker => match property_name {
            "value" => CapabilityValue::Int(0),
            "minimum" => CapabilityValue::Int(0),
            "maximum" => CapabilityValue::Int(100),
            "step" => CapabilityValue::Int(1),
            "wrap" => CapabilityValue::Bool(false),
            "suffix" => CapabilityValue::String(String::new()),
            "row_count" => CapabilityValue::UInt(101),
            "selected_row" => CapabilityValue::UInt(0),
            "row_height" => CapabilityValue::UInt(28),
            _ => return None,
        },
        WidgetKind::OtpInput => match property_name {
            "value" => CapabilityValue::String(String::new()),
            "length" => CapabilityValue::UInt(6),
            "masked" => CapabilityValue::Bool(false),
            "separator" => CapabilityValue::String(String::new()),
            "focused_index" => CapabilityValue::UInt(0),
            "is_complete" => CapabilityValue::Bool(false),
            _ => return None,
        },
        WidgetKind::Banner => match property_name {
            "text" => CapabilityValue::String(String::new()),
            "severity" => CapabilityValue::String("info".to_string()),
            "dismissible" => CapabilityValue::Bool(true),
            "dismissed" => CapabilityValue::Bool(false),
            "action_count" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::Toast => match property_name {
            "message" => CapabilityValue::String(String::new()),
            "level" => CapabilityValue::String("info".to_string()),
            // Matches `Toast::new`, which is the value a caller gets without asking:
            // a default that disagreed with the constructor would make a schema read
            // describe a control that cannot be built.
            "ttl_ms" => CapabilityValue::UInt(3000),
            "dismissible" => CapabilityValue::Bool(true),
            _ => return None,
        },
        WidgetKind::CandlestickChart => match property_name {
            "series" => CapabilityValue::String(alloc::string::String::new()),
            "overlay_count" => CapabilityValue::UInt(0),
            "show_price_levels" => CapabilityValue::Bool(false),
            _ => return None,
        },
        WidgetKind::VolumeChart => match property_name {
            "series" => CapabilityValue::String(alloc::string::String::new()),
            "color_mode" => CapabilityValue::String(alloc::string::String::from("direction")),
            "headroom" => CapabilityValue::Float(0.92),
            _ => return None,
        },
        WidgetKind::DepthChart => match property_name {
            "depth" => CapabilityValue::UInt(0),
            "bid_color" => CapabilityValue::Color(crate::core::Color::rgb(38, 166, 91)),
            "ask_color" => CapabilityValue::Color(crate::core::Color::rgb(220, 68, 70)),
            _ => return None,
        },
        WidgetKind::OrderBook => match property_name {
            "depth" => CapabilityValue::UInt(5),
            "decimals" => CapabilityValue::Null,
            "show_spread" => CapabilityValue::Bool(true),
            _ => return None,
        },
        WidgetKind::QuoteBoard => match property_name {
            "sort" => CapabilityValue::String(alloc::string::String::from("none")),
            "selected_index" => CapabilityValue::Null,
            "row_height" => CapabilityValue::UInt(22),
            _ => return None,
        },
        WidgetKind::IndicatorChart => match property_name {
            "series" => CapabilityValue::String(alloc::string::String::new()),
            "mode" => CapabilityValue::String(alloc::string::String::from("macd")),
            "period" => CapabilityValue::UInt(14),
            "show_reference_levels" => CapabilityValue::Bool(true),
            _ => return None,
        },
        WidgetKind::SplashScreen => match property_name {
            "title" => CapabilityValue::String(String::new()),
            "subtitle" => CapabilityValue::String(String::new()),
            // `Null` is the indeterminate state, matching `SplashScreen::new`.
            "progress" => CapabilityValue::Null,
            "skippable" => CapabilityValue::Bool(false),
            _ => return None,
        },
        WidgetKind::Pagination => match property_name {
            "total" => CapabilityValue::UInt(0),
            "page_size" => CapabilityValue::UInt(10),
            "page" => CapabilityValue::UInt(0),
            "page_count" => CapabilityValue::UInt(1),
            "last_page" => CapabilityValue::UInt(0),
            "sibling_count" => CapabilityValue::UInt(1),
            "show_nav_buttons" => CapabilityValue::Bool(true),
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
            "current_color" => CapabilityValue::String("#FFFFFFFF".to_string()),
            "modal" => CapabilityValue::Bool(true),
            "options_alpha" => CapabilityValue::Bool(false),
            _ => return None,
        },
        // `ColorPicker` declares its own kind since BLUE16 phase E-6. The defaults
        // match the dialog's picker because both describe the same picker geometry:
        // the difference between them is the host window, not the control.
        WidgetKind::ColorPicker => match property_name {
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
            // `rich_edit` and `code_editor` share this kind; see the `ListView` note.
            "read_only" => CapabilityValue::Bool(false),
            // `markdown_editor` shares this kind; see the `ListView` note above.
            "preview_mode" => CapabilityValue::Bool(false),
            "word_count" => CapabilityValue::UInt(0),
            "heading_count" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::Chart => match property_name {
            "task_count" => CapabilityValue::UInt(0),
            "selected_id" => CapabilityValue::Null,
            "selected_marker_id" => CapabilityValue::Null,
            "viewport_start" => CapabilityValue::Int(0),
            "viewport_end" => CapabilityValue::Int(100),
            // `timeline_widget` shares this kind; see the `ListView` note above.
            "item_count" => CapabilityValue::UInt(0),
            "selected_index" => CapabilityValue::Null,
            "row_height" => CapabilityValue::UInt(24),
            // `chart` shares this kind; see the `ListView` note above.
            "chart_type" => CapabilityValue::String("bar".to_string()),
            "point_count" => CapabilityValue::UInt(0),
            "label_count" => CapabilityValue::UInt(0),
            "series_count" => CapabilityValue::UInt(0),
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

        // `Canvas` and `MapView` share `WidgetKind::Canvas`; defaults are keyed by
        // kind, so this arm answers for both controls.
        WidgetKind::Canvas => match property_name {
            // `canvas`
            "command_count" => CapabilityValue::UInt(0),
            // `map_view`
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
            // Mirrors `Carousel::new`: wrap and autoplay are off, the indicator is a
            // dot row at the bottom.
            "loop" => CapabilityValue::Bool(false),
            "autoplay_interval" => CapabilityValue::Null,
            "indicator_style" => CapabilityValue::String("dots".to_string()),
            "indicator_position" => CapabilityValue::String("bottom".to_string()),
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

        // `Breadcrumb` previously reported `WidgetKind::Panel` (a shared kind), so
        // this arm served it. It now has its own `WidgetKind::Breadcrumb` kind, so
        // the `segment_count` / `selected_index` defaults are declared on the kind
        // that actually owns them.
        WidgetKind::Breadcrumb => match property_name {
            "segment_count" => CapabilityValue::UInt(0),
            "selected_index" => CapabilityValue::Null,
            _ => return None,
        },
        WidgetKind::SignaturePad => match property_name {
            "stroke_count" => CapabilityValue::UInt(0),
            "stroke_width" => CapabilityValue::UInt(2),
            "stroke_color" => CapabilityValue::Color(crate::core::Color::rgb(20, 20, 20)),
            "min_point_distance" => CapabilityValue::Float(1.5),
            _ => return None,
        },
        WidgetKind::DropZone => match property_name {
            "accepted_type" => CapabilityValue::String("text/plain".to_string()),
            "hovered" => CapabilityValue::Bool(false),
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
        // `CupertinoSwitch` publishes the same schema as `Switch` (it is the
        // iOS-styled sibling of the same control), so it shares this arm rather
        // than repeating the defaults. Registering the kind without adding it
        // here left `default_property_value("cupertino_switch", "checked")`
        // answering `None` while the schema declared the property readable,
        // which `schema_defaults_are_readable_and_writable_when_declared`
        // rejects.
        WidgetKind::Switch | WidgetKind::CupertinoSwitch => match property_name {
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
            // Mirrors `Meter::new`.
            "tick_count" => CapabilityValue::UInt(5),
            "show_tick_labels" => CapabilityValue::Bool(false),
            "unit" => CapabilityValue::String(String::new()),
            "value_text" => CapabilityValue::String("0".to_string()),
            "threshold_count" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::RadarChart => match property_name {
            "axis_count" => CapabilityValue::UInt(0),
            "series_count" => CapabilityValue::UInt(0),
            // Mirrors `RadarChart::new`.
            "show_grid" => CapabilityValue::Bool(true),
            "show_axis_labels" => CapabilityValue::Bool(true),
            "show_legend" => CapabilityValue::Bool(true),
            "hovered_axis" => CapabilityValue::Null,
            _ => return None,
        },
        WidgetKind::KanbanBoard => match property_name {
            "column_count" => CapabilityValue::UInt(0),
            "card_count" => CapabilityValue::UInt(0),
            "dragging_card_id" => CapabilityValue::Null,
            "hovered_column" => CapabilityValue::Null,
            // Mirrors the `COLUMN_WIDTH` layout constant in the control.
            "column_width" => CapabilityValue::UInt(220),
            _ => return None,
        },
        WidgetKind::Cascader => match property_name {
            "value" => CapabilityValue::String(String::new()),
            "depth" => CapabilityValue::UInt(0),
            // Mirrors `Cascader::new`.
            "expanded" => CapabilityValue::Bool(false),
            "separator" => CapabilityValue::String("/".to_string()),
            "filterable" => CapabilityValue::Bool(false),
            "filter" => CapabilityValue::String(String::new()),
            _ => return None,
        },
        WidgetKind::QueryBuilder => match property_name {
            "row_count" => CapabilityValue::UInt(0),
            "incomplete_row_count" => CapabilityValue::UInt(0),
            "field_count" => CapabilityValue::UInt(0),
            // Mirrors `QueryBuilder::new`.
            "conjunction" => CapabilityValue::String("and".to_string()),
            "active_row" => CapabilityValue::Null,
            _ => return None,
        },
        WidgetKind::EmojiPicker => match property_name {
            "glyph_count" => CapabilityValue::UInt(0),
            "visible_glyph_count" => CapabilityValue::UInt(0),
            "tab_count" => CapabilityValue::UInt(0),
            // Mirrors `EmojiPicker::new`.
            "active_tab" => CapabilityValue::UInt(0),
            "search" => CapabilityValue::String(String::new()),
            "recent_count" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::Mention => match property_name {
            "text" => CapabilityValue::String(String::new()),
            // Mirrors `Mention::new`.
            "trigger" => CapabilityValue::String("@".to_string()),
            "candidate_count" => CapabilityValue::UInt(0),
            "visible_candidate_count" => CapabilityValue::UInt(0),
            "mention_count" => CapabilityValue::UInt(0),
            "popup_open" => CapabilityValue::Bool(false),
            "caret" => CapabilityValue::UInt(0),
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
        // ── Dialog widgets ──────────────────────────────────
        WidgetKind::Dialog => match property_name {
            "title" => CapabilityValue::String(String::new()),
            "modal" => CapabilityValue::Bool(true),
            "has_content" => CapabilityValue::Bool(false),
            _ => return None,
        },
        WidgetKind::MessageBox => match property_name {
            "title" => CapabilityValue::String(String::new()),
            "text" => CapabilityValue::String(String::new()),
            // Mirrors `MessageBox::new`, which starts at `MessageBoxIcon::NoIcon`.
            "icon" => CapabilityValue::String("none".to_string()),
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
            // `toast_stack` shares this kind; see the `ListView` note above.
            "toast_count" => CapabilityValue::UInt(0),
            "selected_id" => CapabilityValue::Null,
            "row_height" => CapabilityValue::UInt(30),
            _ => return None,
        },
        // ── Container widgets ───────────────────────────────
        WidgetKind::ScrollArea => match property_name {
            "widget_resizable" => CapabilityValue::Bool(true),
            "horizontal_scroll_bar_policy" => CapabilityValue::String("as_needed".to_string()),
            "vertical_scroll_bar_policy" => CapabilityValue::String("as_needed".to_string()),
            "scroll_position_x" => CapabilityValue::Int(0),
            "scroll_position_y" => CapabilityValue::Int(0),
            "sticky_region_count" => CapabilityValue::UInt(0),
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
            // Mirrors `BadgeLevel::default()`, which is `Info`.
            "level" => CapabilityValue::String("info".to_string()),
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
            // An icon with no explicit colour follows the theme, so the schema
            // default is the empty string rather than a colour literal: "unset" is
            // the honest default, and `Icon::color` resolves what will be drawn.
            "color" => CapabilityValue::String(String::new()),
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
/// Stripped-profile stub: always `None`.
///
/// With the concrete controls compiled out there is no schema to consult, so
/// no property has a declared default. Callers must treat `None` as "unknown"
/// rather than "unset", exactly as in the full profile.
pub fn default_widget_property_default_value(
    _kind: WidgetKind,
    _property_name: &str,
) -> Option<CapabilityValue> {
    None
}
