use super::types::{PropertySchema, PropertyValueKind, WidgetCapability};
use crate::widget::WidgetKind;

pub(crate) const BUTTON_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "text",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "pressed",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "default",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "enabled",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "tooltip",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
];

pub(crate) const LABEL_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "text",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "alignment",
        value_kind: PropertyValueKind::Enum,
        readable: true,
        writable: true,
    },
];

pub(crate) const CHECK_BOX_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "text",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "state",
        value_kind: PropertyValueKind::Enum,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "checked",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "tristate_enabled",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
];

pub(crate) const RADIO_BUTTON_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "text",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "checked",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "group_id",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
];

pub(crate) const SLIDER_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "minimum",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "maximum",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "value",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "single_step",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "page_step",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "orientation",
        value_kind: PropertyValueKind::Enum,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "tick_position",
        value_kind: PropertyValueKind::Enum,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "tick_interval",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "tracking",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "slider_position",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
];

pub(crate) const PROGRESS_BAR_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "minimum",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "maximum",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "value",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "text_visible",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "orientation",
        value_kind: PropertyValueKind::Enum,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "inverted_appearance",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "progress",
        value_kind: PropertyValueKind::Float,
        readable: true,
        writable: false,
    },
];

pub(crate) const SCROLL_BAR_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "minimum",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "maximum",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "value",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "single_step",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "page_step",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "orientation",
        value_kind: PropertyValueKind::Enum,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "slider_size",
        value_kind: PropertyValueKind::Float,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "slider_position",
        value_kind: PropertyValueKind::Float,
        readable: true,
        writable: false,
    },
];

pub(crate) const LIST_BOX_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "item_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "selection_mode",
        value_kind: PropertyValueKind::Enum,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "current_row",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "item_height",
        value_kind: PropertyValueKind::Float,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "selected_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
];

pub(crate) const SPIN_BOX_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "minimum",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "maximum",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "value",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "single_step",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "prefix",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "suffix",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "special_value_text",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "wrapping",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
];

pub(crate) const COMBO_BOX_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "item_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "current_index",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "current_text",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "editable",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "max_visible_items",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
];

pub(crate) const DIAL_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "minimum",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "maximum",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "value",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "single_step",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "page_step",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "notches_visible",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "notch_target",
        value_kind: PropertyValueKind::Float,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "wrapping",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
];

pub(crate) const WINDOW_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "title",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "title_bar_height",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "close_button_size",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "button_spacing",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
];

pub(crate) const GROUP_BOX_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "title",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "alignment",
        value_kind: PropertyValueKind::Enum,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "checkable",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "checked",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
];

pub(crate) const SPLITTER_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "orientation",
        value_kind: PropertyValueKind::Enum,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "pane_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
];

pub(crate) const LCD_NUMBER_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "value",
        value_kind: PropertyValueKind::Float,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "min_value",
        value_kind: PropertyValueKind::Float,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "max_value",
        value_kind: PropertyValueKind::Float,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "num_digits",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "small_decimal_point",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "mode",
        value_kind: PropertyValueKind::Enum,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "segment_style",
        value_kind: PropertyValueKind::Enum,
        readable: true,
        writable: true,
    },
];

pub(crate) const COMMAND_LINK_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "text",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "description",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "enabled",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
];

pub(crate) const FONT_COMBO_BOX_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "current_font_family",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "item_count",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "current_index",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "editable",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "max_visible_items",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
];

pub(crate) const ACTION_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "text",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "icon_text",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "shortcut",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "checkable",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "checked",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "separator",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "command_id",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
];

pub(crate) const TOOL_BOX_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "item_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "current_index",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "orientation",
        value_kind: PropertyValueKind::Enum,
        readable: true,
        writable: true,
    },
];

pub(crate) const TAB_BAR_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "tab_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "current_index",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "closable",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "movable",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "tab_min_width",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "tab_max_width",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
];

pub(crate) const CALENDAR_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "selected_date",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "minimum_date",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "maximum_date",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "first_day_of_week",
        value_kind: PropertyValueKind::Enum,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "grid_visible",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "navigation_bar_visible",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "horizontal_header_visible",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "vertical_header_visible",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "date_format",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
];

pub(crate) const DATE_EDIT_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "date",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "minimum_date",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "maximum_date",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "display_format",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "calendar_popup",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
];

pub(crate) const TIME_EDIT_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "time",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "minimum_time",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "maximum_time",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "display_format",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
];

pub(crate) const LINE_EDIT_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "text",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "placeholder_text",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "max_length",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "read_only",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "cursor_position",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
];

pub(crate) const LIST_VIEW_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "has_model",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "row_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "focused_row",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "selection_mode",
        value_kind: PropertyValueKind::Enum,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "view_mode",
        value_kind: PropertyValueKind::Enum,
        readable: true,
        writable: true,
    },
];

pub(crate) const TREE_VIEW_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "has_model",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "node_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "focused_node",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "selected_node",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
];

pub(crate) const TABLE_WIDGET_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "has_model",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "has_delegate",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "row_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "column_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "selection_mode",
        value_kind: PropertyValueKind::Enum,
        readable: true,
        writable: true,
    },
];

pub(crate) const DATA_GRID_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "has_data_source",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "row_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "column_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "scroll_row",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "scroll_column",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "row_height",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "column_width",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "frozen_columns",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "sort_spec_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "filter_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "sort_specs",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "filters",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
];

pub(crate) const TREE_TABLE_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "has_model",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "row_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "column_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "selected_row",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "row_height",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "column_width",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
];

pub(crate) const VIRTUAL_TABLE_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "has_data_source",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "row_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "column_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "scroll_row",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "scroll_column",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "row_height",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "column_width",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "overscan_rows",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "overscan_columns",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "visible_window",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: false,
    },
];

pub(crate) const VIRTUAL_LIST_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "has_data_source",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "row_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "scroll_row",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "row_height",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "overscan",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "selected_row",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
];

pub(crate) const MENU_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "title",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "item_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "hovered_index",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
];

pub(crate) const MENU_BAR_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "entry_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "active_index",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "hovered_index",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
];

pub(crate) const TOOL_BAR_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "orientation",
        value_kind: PropertyValueKind::Enum,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "icon_size",
        value_kind: PropertyValueKind::Float,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "movable",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "floatable",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "item_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
];

pub(crate) const RIBBON_BAR_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "tab_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "current_tab",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "expanded",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "minimized",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
];

pub(crate) const COLOR_PICKER_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "hex_rgba",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "show_alpha",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "preset_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
];

pub(crate) const CODE_EDITOR_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "text",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "line_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "cursor_line",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "cursor_column",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "marker_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
];

pub(crate) const GANTT_WIDGET_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "task_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "selected_id",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "viewport_start",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "viewport_end",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
];

pub(crate) const TERMINAL_VIEW_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "output_line_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "input_line",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
];

pub(crate) const SNACKBAR_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "message",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "visible",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "action_label",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: false,
    },
];

pub(crate) const MAP_VIEW_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "center_x",
        value_kind: PropertyValueKind::Float,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "center_y",
        value_kind: PropertyValueKind::Float,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "zoom",
        value_kind: PropertyValueKind::Float,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "marker_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "selected_marker_id",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: false,
    },
];

pub(crate) const MEDIA_PLAYER_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "source",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "playing",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "duration_ms",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "position_ms",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "volume",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "muted",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "fullscreen",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
];

pub(crate) const BREADCRUMB_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "segment_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "selected_index",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
];

pub(crate) const SPLIT_BUTTON_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "text",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "action_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "menu_open",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "row_height",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
];

pub(crate) const SEGMENTED_CONTROL_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "item_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "selected_index",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "selected_id",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: false,
    },
];

pub(crate) const CHIP_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "item_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "multi_select",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "focused_index",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "selected_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
];

pub(crate) const GRID_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "rows",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "columns",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "spacing",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "line_color",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "cell_width",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "cell_height",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
];

pub(crate) const FREEFORM_SHAPE_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "path_kind",
        value_kind: PropertyValueKind::Enum,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "fill_rgba",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "stroke_rgba",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "stroke_width",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
];

// ── Dialog widgets ──────────────────────────────────────────

pub(crate) const MESSAGE_BOX_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "title",
        value_kind: PropertyValueKind::String,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "text",
        value_kind: PropertyValueKind::String,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "modal",
        value_kind: PropertyValueKind::Bool,
        readable: false,
        writable: false,
    },
];

pub(crate) const FILE_DIALOG_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "title",
        value_kind: PropertyValueKind::String,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "modal",
        value_kind: PropertyValueKind::Bool,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "directory",
        value_kind: PropertyValueKind::String,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "selected_file",
        value_kind: PropertyValueKind::String,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "mode",
        value_kind: PropertyValueKind::Enum,
        readable: false,
        writable: false,
    },
];

pub(crate) const FONT_DIALOG_PROPERTIES: &[PropertySchema] = &[PropertySchema {
    name: "modal",
    value_kind: PropertyValueKind::Bool,
    readable: false,
    writable: false,
}];

pub(crate) const INPUT_DIALOG_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "title",
        value_kind: PropertyValueKind::String,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "label_text",
        value_kind: PropertyValueKind::String,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "mode",
        value_kind: PropertyValueKind::Enum,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "text_value",
        value_kind: PropertyValueKind::String,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "int_value",
        value_kind: PropertyValueKind::Int,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "double_value",
        value_kind: PropertyValueKind::Float,
        readable: false,
        writable: false,
    },
];

pub(crate) const PROGRESS_DIALOG_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "title",
        value_kind: PropertyValueKind::String,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "label_text",
        value_kind: PropertyValueKind::String,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "value",
        value_kind: PropertyValueKind::Int,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "minimum",
        value_kind: PropertyValueKind::Int,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "maximum",
        value_kind: PropertyValueKind::Int,
        readable: false,
        writable: false,
    },
];

pub(crate) const POPUP_WINDOW_PROPERTIES: &[PropertySchema] = &[PropertySchema {
    name: "has_content",
    value_kind: PropertyValueKind::Bool,
    readable: false,
    writable: false,
}];

// ── Container widgets ───────────────────────────────────────

pub(crate) const SCROLL_AREA_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "widget_resizable",
        value_kind: PropertyValueKind::Bool,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "horizontal_scroll_bar_policy",
        value_kind: PropertyValueKind::Enum,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "vertical_scroll_bar_policy",
        value_kind: PropertyValueKind::Enum,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "scroll_position_x",
        value_kind: PropertyValueKind::Int,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "scroll_position_y",
        value_kind: PropertyValueKind::Int,
        readable: false,
        writable: false,
    },
];

pub(crate) const TAB_WIDGET_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "tab_count",
        value_kind: PropertyValueKind::UInt,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "current_index",
        value_kind: PropertyValueKind::UInt,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "closable",
        value_kind: PropertyValueKind::Bool,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "movable",
        value_kind: PropertyValueKind::Bool,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "tab_position",
        value_kind: PropertyValueKind::Enum,
        readable: false,
        writable: false,
    },
];

pub(crate) const STACKED_WIDGET_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "widget_count",
        value_kind: PropertyValueKind::UInt,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "current_index",
        value_kind: PropertyValueKind::UInt,
        readable: false,
        writable: false,
    },
];

pub(crate) const COLLAPSIBLE_PANE_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "title",
        value_kind: PropertyValueKind::String,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "collapsed",
        value_kind: PropertyValueKind::Bool,
        readable: false,
        writable: false,
    },
];

pub(crate) const DOCK_WIDGET_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "title",
        value_kind: PropertyValueKind::String,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "floating",
        value_kind: PropertyValueKind::Bool,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "docked",
        value_kind: PropertyValueKind::Bool,
        readable: false,
        writable: false,
    },
];

pub(crate) const MDI_AREA_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "subwindow_count",
        value_kind: PropertyValueKind::UInt,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "active_subwindow",
        value_kind: PropertyValueKind::UInt,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "view_mode",
        value_kind: PropertyValueKind::Enum,
        readable: false,
        writable: false,
    },
];

// ── Input / text widgets ────────────────────────────────────

pub(crate) const TEXT_EDIT_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "text",
        value_kind: PropertyValueKind::String,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "placeholder_text",
        value_kind: PropertyValueKind::String,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "max_length",
        value_kind: PropertyValueKind::UInt,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "read_only",
        value_kind: PropertyValueKind::Bool,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "line_wrap",
        value_kind: PropertyValueKind::Bool,
        readable: false,
        writable: false,
    },
];

// ── Web widgets ─────────────────────────────────────────────

pub(crate) const WEB_VIEW_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "url",
        value_kind: PropertyValueKind::String,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "loading",
        value_kind: PropertyValueKind::Bool,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "title",
        value_kind: PropertyValueKind::String,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "can_go_back",
        value_kind: PropertyValueKind::Bool,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "can_go_forward",
        value_kind: PropertyValueKind::Bool,
        readable: false,
        writable: false,
    },
];

// ── Advanced widgets ────────────────────────────────────────

pub(crate) const PIE_MENU_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "item_count",
        value_kind: PropertyValueKind::UInt,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "radius",
        value_kind: PropertyValueKind::Float,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "inner_radius",
        value_kind: PropertyValueKind::Float,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "current_index",
        value_kind: PropertyValueKind::UInt,
        readable: false,
        writable: false,
    },
];

pub(crate) const DATE_TIME_EDIT_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "datetime",
        value_kind: PropertyValueKind::String,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "display_format",
        value_kind: PropertyValueKind::String,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "calendar_popup",
        value_kind: PropertyValueKind::Bool,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "minimum",
        value_kind: PropertyValueKind::String,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "maximum",
        value_kind: PropertyValueKind::String,
        readable: false,
        writable: false,
    },
];

pub(crate) fn button_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Button,
        canonical_name: "button",
        aliases: &["pushbutton", "btn"],
        properties: BUTTON_PROPERTIES,
        events: &["clicked", "pressed", "released", "state_changed"],
        commands: &["click"],
    }
}

pub(crate) fn label_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Label,
        canonical_name: "label",
        aliases: &["text_label"],
        properties: LABEL_PROPERTIES,
        events: &[],
        commands: &["set_text", "set_alignment"],
    }
}

pub(crate) fn check_box_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::CheckBox,
        canonical_name: "check_box",
        aliases: &["checkbox"],
        properties: CHECK_BOX_PROPERTIES,
        events: &["toggled", "state_changed"],
        commands: &["set_checked", "toggle", "set_state"],
    }
}

pub(crate) fn radio_button_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::RadioButton,
        canonical_name: "radio_button",
        aliases: &["radiobutton"],
        properties: RADIO_BUTTON_PROPERTIES,
        events: &["selected", "checked_changed"],
        commands: &["set_checked", "set_group_id"],
    }
}

pub(crate) fn slider_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Slider,
        canonical_name: "slider",
        aliases: &["range_slider"],
        properties: SLIDER_PROPERTIES,
        events: &["value_changed", "slider_moved"],
        commands: &["set_range", "set_value", "set_slider_position"],
    }
}

pub(crate) fn progress_bar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ProgressBar,
        canonical_name: "progress_bar",
        aliases: &["progressbar"],
        properties: PROGRESS_BAR_PROPERTIES,
        events: &["value_changed", "range_changed"],
        commands: &["set_range", "set_value", "set_orientation"],
    }
}

pub(crate) fn scroll_bar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ScrollBar,
        canonical_name: "scroll_bar",
        aliases: &["scrollbar"],
        properties: SCROLL_BAR_PROPERTIES,
        events: &["value_changed", "range_changed", "slider_moved"],
        commands: &["set_range", "set_value", "set_steps", "set_orientation"],
    }
}

pub(crate) fn list_box_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ListBox,
        canonical_name: "list_box",
        aliases: &["listbox"],
        properties: LIST_BOX_PROPERTIES,
        events: &["item_selected", "item_activated", "selection_changed"],
        commands: &["add_item", "remove_item", "clear", "clear_selection", "set_selection_mode"],
    }
}

pub(crate) fn spin_box_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::SpinBox,
        canonical_name: "spin_box",
        aliases: &["spinbox"],
        properties: SPIN_BOX_PROPERTIES,
        events: &["value_changed", "editing_finished"],
        commands: &["set_range", "set_value", "step_up", "step_down"],
    }
}

pub(crate) fn combo_box_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ComboBox,
        canonical_name: "combo_box",
        aliases: &["combobox"],
        properties: COMBO_BOX_PROPERTIES,
        events: &["current_index_changed", "current_text_changed", "activated"],
        commands: &["set_items", "set_current_index", "clear"],
    }
}

pub(crate) fn dial_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Dial,
        canonical_name: "dial",
        aliases: &["knob"],
        properties: DIAL_PROPERTIES,
        events: &["value_changed", "slider_moved", "slider_pressed", "slider_released"],
        commands: &["set_range", "set_value", "set_wrapping"],
    }
}

pub(crate) fn window_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Window,
        canonical_name: "window",
        aliases: &["main_window"],
        properties: WINDOW_PROPERTIES,
        events: &["closed"],
        commands: &["set_title", "close"],
    }
}

pub(crate) fn group_box_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::GroupBox,
        canonical_name: "group_box",
        aliases: &["groupbox"],
        properties: GROUP_BOX_PROPERTIES,
        events: &["toggled"],
        commands: &["set_title", "set_checkable", "set_checked", "toggle"],
    }
}

pub(crate) fn splitter_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Splitter,
        canonical_name: "splitter",
        aliases: &["pane_splitter"],
        properties: SPLITTER_PROPERTIES,
        events: &["pane_layout_changed", "orientation_changed"],
        commands: &["set_orientation", "set_ratio", "set_ratios"],
    }
}

pub(crate) fn lcd_number_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::LCDNumber,
        canonical_name: "lcd_number",
        aliases: &["lcdnumber"],
        properties: LCD_NUMBER_PROPERTIES,
        events: &["value_changed", "overflow"],
        commands: &["set_value", "set_mode", "set_segment_style"],
    }
}

pub(crate) fn command_link_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::CommandLink,
        canonical_name: "command_link",
        aliases: &["commandlink"],
        properties: COMMAND_LINK_PROPERTIES,
        events: &["clicked", "hovered"],
        commands: &["set_text", "set_description", "set_enabled", "click"],
    }
}

pub(crate) fn font_combo_box_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::FontComboBox,
        canonical_name: "font_combo_box",
        aliases: &["fontcombobox"],
        properties: FONT_COMBO_BOX_PROPERTIES,
        events: &[
            "current_font_changed",
            "current_index_changed",
            "activated",
            "text_edited",
            "popup_shown",
            "popup_hidden",
        ],
        commands: &[
            "set_current_index",
            "set_editable",
            "set_max_visible_items",
            "show_popup",
            "hide_popup",
        ],
    }
}

pub(crate) fn action_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Action,
        canonical_name: "action",
        aliases: &["command_action"],
        properties: ACTION_PROPERTIES,
        events: &["triggered", "toggled", "hovered", "changed"],
        commands: &["set_text", "set_checkable", "set_checked", "trigger"],
    }
}

pub(crate) fn tool_box_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ToolBox,
        canonical_name: "tool_box",
        aliases: &["toolbox"],
        properties: TOOL_BOX_PROPERTIES,
        events: &["current_changed"],
        commands: &["add_item", "remove_item", "set_current_index", "set_orientation"],
    }
}

pub(crate) fn tab_bar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::TabBar,
        canonical_name: "tab_bar",
        aliases: &["tabbar"],
        properties: TAB_BAR_PROPERTIES,
        events: &["current_changed", "tab_close_requested", "tab_moved"],
        commands: &["add_tab", "remove_tab", "set_current_index", "set_closable", "set_movable"],
    }
}

pub(crate) fn calendar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Calendar,
        canonical_name: "calendar",
        aliases: &["date_calendar"],
        properties: CALENDAR_PROPERTIES,
        events: &["selection_changed"],
        commands: &["set_selected_date", "set_date_range", "set_first_day_of_week", "show_today"],
    }
}

pub(crate) fn date_edit_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::DatePicker,
        canonical_name: "date_edit",
        aliases: &["dateedit", "date_picker"],
        properties: DATE_EDIT_PROPERTIES,
        events: &["date_changed"],
        commands: &["set_date", "set_date_range", "set_display_format"],
    }
}

pub(crate) fn time_edit_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::TimePicker,
        canonical_name: "time_edit",
        aliases: &["timeedit", "time_picker"],
        properties: TIME_EDIT_PROPERTIES,
        events: &["time_changed"],
        commands: &["set_time", "set_time_range", "set_display_format"],
    }
}

pub(crate) fn line_edit_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::LineEdit,
        canonical_name: "line_edit",
        aliases: &["lineedit", "text_input", "input"],
        properties: LINE_EDIT_PROPERTIES,
        events: &["text_changed", "editing_finished", "return_pressed"],
        commands: &["clear", "select_all"],
    }
}

pub(crate) fn list_view_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ListView,
        canonical_name: "list_view",
        aliases: &["listview"],
        properties: LIST_VIEW_PROPERTIES,
        events: &["selection_changed", "focused_row_changed"],
        commands: &["clear_selection", "clear_focused_row"],
    }
}

pub(crate) fn tree_view_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::TreeView,
        canonical_name: "tree_view",
        aliases: &["treeview"],
        properties: TREE_VIEW_PROPERTIES,
        events: &["selection_changed", "focused_node_changed"],
        commands: &["clear_selection", "clear_focused_node"],
    }
}

pub(crate) fn table_widget_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Table,
        canonical_name: "table_widget",
        aliases: &["tablewidget", "table"],
        properties: TABLE_WIDGET_PROPERTIES,
        events: &["selection_changed", "focused_row_changed"],
        commands: &["clear_selection", "clear_focused_row"],
    }
}

pub(crate) fn data_grid_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Table,
        canonical_name: "data_grid",
        aliases: &["datagrid"],
        properties: DATA_GRID_PROPERTIES,
        events: &["visible_window_changed"],
        commands: &[
            "set_data_source",
            "clear_data_source",
            "set_scroll_row",
            "set_scroll_column",
            "set_row_height",
            "set_column_width",
            "set_frozen_columns",
        ],
    }
}

pub(crate) fn tree_table_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::TreeView,
        canonical_name: "tree_table",
        aliases: &["treetable"],
        properties: TREE_TABLE_PROPERTIES,
        events: &["projection_changed", "selection_changed"],
        commands: &["set_model", "clear_model", "expand_row", "collapse_row", "select_row"],
    }
}

pub(crate) fn virtual_table_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Table,
        canonical_name: "virtual_table",
        aliases: &["virtualtable"],
        properties: VIRTUAL_TABLE_PROPERTIES,
        events: &["visible_window_changed"],
        commands: &[
            "set_data_source",
            "clear_data_source",
            "set_scroll_row",
            "set_scroll_column",
            "set_row_height",
            "set_column_width",
            "set_overscan_rows",
            "set_overscan_columns",
            "fetch_visible_window",
        ],
    }
}

pub(crate) fn virtual_list_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::DataView,
        canonical_name: "virtual_list",
        aliases: &["virtuallist", "data_view", "dataview"],
        properties: VIRTUAL_LIST_PROPERTIES,
        events: &["selection_changed", "visible_window_changed"],
        commands: &["clear_data_source", "fetch_visible_rows"],
    }
}

pub(crate) fn menu_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Menu,
        canonical_name: "menu",
        aliases: &["context_menu"],
        properties: MENU_PROPERTIES,
        events: &["triggered", "triggered_index", "about_to_show", "about_to_hide"],
        commands: &["clear", "add_action", "add_separator"],
    }
}

pub(crate) fn menu_bar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::MenuBar,
        canonical_name: "menu_bar",
        aliases: &["menubar"],
        properties: MENU_BAR_PROPERTIES,
        events: &["triggered", "hovered_entry"],
        commands: &["clear", "add_menu", "remove_menu"],
    }
}

pub(crate) fn tool_bar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ToolBar,
        canonical_name: "tool_bar",
        aliases: &["toolbar"],
        properties: TOOL_BAR_PROPERTIES,
        events: &[
            "action_triggered",
            "orientation_changed",
            "top_level_changed",
            "visibility_changed",
        ],
        commands: &["clear", "add_action", "add_separator"],
    }
}

pub(crate) fn ribbon_bar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::RibbonBar,
        canonical_name: "ribbon_bar",
        aliases: &["ribbonbar", "ribbon"],
        properties: RIBBON_BAR_PROPERTIES,
        events: &["current_tab_changed", "item_triggered"],
        commands: &["add_tab", "add_group", "add_item", "add_large_item", "clear"],
    }
}

pub(crate) fn color_picker_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ColorDialog,
        canonical_name: "color_picker",
        aliases: &["colorpicker"],
        properties: COLOR_PICKER_PROPERTIES,
        events: &["color_changed", "hex_changed"],
        commands: &["set_hex", "apply_preset"],
    }
}

pub(crate) fn code_editor_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::RichEdit,
        canonical_name: "code_editor",
        aliases: &["codeeditor"],
        properties: CODE_EDITOR_PROPERTIES,
        events: &["text_changed", "cursor_moved", "selection_changed"],
        commands: &["set_text", "append_line", "set_markers", "set_cursor"],
    }
}

pub(crate) fn gantt_widget_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Chart,
        canonical_name: "gantt_widget",
        aliases: &["gantt", "ganttwidget"],
        properties: GANTT_WIDGET_PROPERTIES,
        events: &["task_selected", "viewport_changed"],
        commands: &["set_tasks", "zoom", "set_viewport", "select_task"],
    }
}

pub(crate) fn terminal_view_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::TextEdit,
        canonical_name: "terminal_view",
        aliases: &["terminal", "terminalview"],
        properties: TERMINAL_VIEW_PROPERTIES,
        events: &["command_submitted"],
        commands: &["append_output", "submit"],
    }
}

pub(crate) fn snackbar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::StatusBar,
        canonical_name: "snackbar",
        aliases: &["toast_bar"],
        properties: SNACKBAR_PROPERTIES,
        events: &["action_triggered", "dismissed"],
        commands: &["show", "show_with_action", "dismiss"],
    }
}

pub(crate) fn map_view_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Canvas,
        canonical_name: "map_view",
        aliases: &["mapview"],
        properties: MAP_VIEW_PROPERTIES,
        events: &["center_changed", "zoom_changed", "marker_selected"],
        commands: &["set_markers", "set_center", "set_zoom"],
    }
}

pub(crate) fn media_player_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::WebView,
        canonical_name: "media_player",
        aliases: &["mediaplayer"],
        properties: MEDIA_PLAYER_PROPERTIES,
        events: &["playback_changed", "position_changed", "volume_changed", "source_changed"],
        commands: &["set_source", "clear_source", "play", "pause", "seek_to", "set_volume"],
    }
}

pub(crate) fn breadcrumb_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Panel,
        canonical_name: "breadcrumb",
        aliases: &["nav_breadcrumb"],
        properties: BREADCRUMB_PROPERTIES,
        events: &["segment_activated"],
        commands: &["set_segments", "push_segment", "clear_segments"],
    }
}

pub(crate) fn split_button_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ToolButton,
        canonical_name: "split_button",
        aliases: &["splitbutton"],
        properties: SPLIT_BUTTON_PROPERTIES,
        events: &["triggered", "action_selected", "menu_toggled"],
        commands: &["add_action", "open_menu", "close_menu", "trigger_primary"],
    }
}

pub(crate) fn segmented_control_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ToggleButton,
        canonical_name: "segmented_control",
        aliases: &["segmentedcontrol"],
        properties: SEGMENTED_CONTROL_PROPERTIES,
        events: &["selection_changed"],
        commands: &["set_items", "move_selection"],
    }
}

pub(crate) fn chip_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::CheckListBox,
        canonical_name: "chip",
        aliases: &["chips"],
        properties: CHIP_PROPERTIES,
        events: &["chip_toggled"],
        commands: &["set_items", "toggle_index", "move_focus"],
    }
}

pub(crate) fn grid_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Grid,
        canonical_name: "grid",
        aliases: &["grid_widget", "gridwidget"],
        properties: GRID_PROPERTIES,
        events: &["cell_clicked", "cell_double_clicked", "selection_changed"],
        commands: &[
            "set_rows",
            "set_columns",
            "set_spacing",
            "select_cell",
            "clear_selection",
            "set_line_color",
        ],
    }
}

pub(crate) fn freeform_shape_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::FreeformShape,
        canonical_name: "freeform_shape",
        commands: &["set_fill_color", "set_stroke_color", "set_stroke_width"],
        aliases: &["freeformshape"],
        properties: FREEFORM_SHAPE_PROPERTIES,
        events: &["clicked", "hovered_changed", "pressed_changed"],
    }
}

// ── Dialog widget capabilities ────────────────────────────────

pub(crate) fn message_box_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::MessageBox,
        canonical_name: "message_box",
        aliases: &["messagebox", "msgbox"],
        properties: MESSAGE_BOX_PROPERTIES,
        events: &["button_clicked", "accepted", "rejected"],
        commands: &["set_text", "set_title", "set_icon"],
    }
}

pub(crate) fn file_dialog_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::FileDialog,
        canonical_name: "file_dialog",
        aliases: &["filedialog"],
        properties: FILE_DIALOG_PROPERTIES,
        events: &["file_selected", "files_selected", "accepted", "rejected"],
        commands: &["set_mode", "set_directory", "open"],
    }
}

pub(crate) fn font_dialog_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::FontDialog,
        canonical_name: "font_dialog",
        aliases: &["fontdialog"],
        properties: FONT_DIALOG_PROPERTIES,
        events: &["font_selected", "accepted", "rejected"],
        commands: &["set_current_font", "accept", "reject"],
    }
}

pub(crate) fn input_dialog_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::InputDialog,
        canonical_name: "input_dialog",
        aliases: &["inputdialog"],
        properties: INPUT_DIALOG_PROPERTIES,
        events: &[
            "text_value_changed",
            "int_value_changed",
            "double_value_changed",
            "accepted",
            "rejected",
        ],
        commands: &["set_mode", "set_text_value", "set_int_value", "set_double_value"],
    }
}

pub(crate) fn progress_dialog_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ProgressDialog,
        canonical_name: "progress_dialog",
        aliases: &["progressdialog"],
        properties: PROGRESS_DIALOG_PROPERTIES,
        events: &["canceled"],
        commands: &["set_value", "set_range", "set_title", "set_label_text"],
    }
}

pub(crate) fn popup_window_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::PopupWindow,
        canonical_name: "popup_window",
        aliases: &["popupwindow", "popup"],
        properties: POPUP_WINDOW_PROPERTIES,
        events: &[],
        commands: &["set_content_widget"],
    }
}

// ── Container widget capabilities ─────────────────────────────

pub(crate) fn scroll_area_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ScrollArea,
        canonical_name: "scroll_area",
        aliases: &["scrollarea"],
        properties: SCROLL_AREA_PROPERTIES,
        events: &["scroll_position_changed"],
        commands: &["set_widget_resizable", "set_horizontal_policy", "set_vertical_policy"],
    }
}

pub(crate) fn tab_widget_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::TabWidget,
        canonical_name: "tab_widget",
        aliases: &["tabwidget"],
        properties: TAB_WIDGET_PROPERTIES,
        events: &["current_changed", "tab_close_requested"],
        commands: &[
            "add_tab",
            "remove_tab",
            "set_current_index",
            "set_closable",
            "set_movable",
            "set_tab_position",
        ],
    }
}

pub(crate) fn stacked_widget_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::StackedWidget,
        canonical_name: "stacked_widget",
        aliases: &["stackedwidget", "stacked"],
        properties: STACKED_WIDGET_PROPERTIES,
        events: &["current_changed"],
        commands: &["add_widget", "remove_widget", "set_current_index"],
    }
}

pub(crate) fn collapsible_pane_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::CollapsiblePane,
        canonical_name: "collapsible_pane",
        aliases: &["collapsiblepane", "collapsible"],
        properties: COLLAPSIBLE_PANE_PROPERTIES,
        events: &["toggled"],
        commands: &["set_title", "set_collapsed", "toggle"],
    }
}

pub(crate) fn dock_widget_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::DockWidget,
        canonical_name: "dock_widget",
        aliases: &["dockwidget", "dock"],
        properties: DOCK_WIDGET_PROPERTIES,
        events: &["dock_location_changed", "features_changed", "top_level_changed"],
        commands: &["set_title", "set_floating", "set_features", "set_allowed_areas"],
    }
}

pub(crate) fn mdi_area_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::MdiArea,
        canonical_name: "mdi_area",
        aliases: &["mdiarea", "mdi"],
        properties: MDI_AREA_PROPERTIES,
        events: &["subwindow_activated"],
        commands: &["add_subwindow", "remove_subwindow", "set_view_mode", "activate_subwindow"],
    }
}

// ── Text/input widget capabilities ───────────────────────────

pub(crate) fn text_edit_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::TextEdit,
        canonical_name: "text_edit",
        aliases: &["textedit"],
        properties: TEXT_EDIT_PROPERTIES,
        events: &["text_changed", "cursor_position_changed"],
        commands: &[
            "set_text",
            "set_placeholder_text",
            "set_max_length",
            "set_read_only",
            "set_line_wrap",
        ],
    }
}

// ── Web widget capabilities ──────────────────────────────────

pub(crate) fn web_view_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::WebView,
        canonical_name: "web_view",
        aliases: &["webview"],
        properties: WEB_VIEW_PROPERTIES,
        events: &[
            "loading_started",
            "loading_finished",
            "title_changed",
            "url_changed",
            "error_occurred",
            "navigation_state_changed",
        ],
        commands: &["set_url", "load_url", "reload", "go_back", "go_forward", "stop"],
    }
}

// ── Advanced widget capabilities ─────────────────────────────

pub(crate) fn pie_menu_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::PieMenu,
        canonical_name: "pie_menu",
        aliases: &["piemenu", "radial_menu"],
        properties: PIE_MENU_PROPERTIES,
        events: &["triggered", "triggered_text", "about_to_show", "about_to_hide"],
        commands: &["add_item", "remove_item", "set_radius", "set_current_index"],
    }
}

pub(crate) fn date_time_edit_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::DateTimePicker,
        canonical_name: "date_time_edit",
        aliases: &["datetimeedit", "datetimepicker", "date_time_picker"],
        properties: DATE_TIME_EDIT_PROPERTIES,
        events: &["datetime_changed"],
        commands: &["set_datetime", "set_display_format", "set_calendar_popup"],
    }
}
