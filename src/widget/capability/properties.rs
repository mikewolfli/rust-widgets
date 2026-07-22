use super::types::{PropertySchema, PropertyValueKind, WidgetCapability};
use crate::widget::WidgetKind;

// ── Property schema definitions (split into per-category files) ──
include!("properties_base.in.rs");
include!("properties_input.in.rs");
include!("properties_view.in.rs");
include!("properties_container.in.rs");
include!("properties_dialog.in.rs");
include!("properties_menu.in.rs");
include!("properties_advanced.in.rs");
include!("properties_media.in.rs");
include!("properties_other.in.rs");

// Master macro that expands all category macros at once.
macro_rules! impl_all_properties {
    () => {
        impl_properties_base!();
        impl_properties_input!();
        impl_properties_view!();
        impl_properties_container!();
        impl_properties_dialog!();
        impl_properties_menu!();
        impl_properties_advanced!();
        impl_properties_media!();
        impl_properties_other!();
    };
}
impl_all_properties!();

// ── Widget capability functions ─────────────────────────────────
// ── Dialog widgets ──────────────────────────────────────────
// ── Container widgets ───────────────────────────────────────
// ── Input / text widgets ────────────────────────────────────
// ── Web widgets ─────────────────────────────────────────────
// ── Advanced widgets ────────────────────────────────────────
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

#[cfg(not(feature = "mini"))]
#[cfg(not(feature = "mini"))]
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

#[cfg(not(feature = "mini"))]
#[cfg(not(feature = "mini"))]
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

#[cfg(not(feature = "mini"))]
#[cfg(not(feature = "mini"))]
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

#[cfg(not(feature = "mini"))]
#[cfg(not(feature = "mini"))]
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

#[cfg(not(feature = "mini"))]
#[cfg(not(feature = "mini"))]
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

#[cfg(not(feature = "mini"))]
#[cfg(not(feature = "mini"))]
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

#[cfg(not(feature = "mini"))]
#[cfg(not(feature = "mini"))]
pub(crate) fn tool_box_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Toolbox,
        canonical_name: "tool_box",
        aliases: &["toolbox"],
        properties: TOOL_BOX_PROPERTIES,
        events: &["current_changed"],
        commands: &["add_item", "remove_item", "set_current_index", "set_orientation"],
    }
}

#[cfg(not(feature = "mini"))]
#[cfg(not(feature = "mini"))]
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

#[cfg(not(feature = "mini"))]
#[cfg(not(feature = "mini"))]
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

#[cfg(not(feature = "mini"))]
#[cfg(not(feature = "mini"))]
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

#[cfg(not(feature = "mini"))]
#[cfg(not(feature = "mini"))]
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

#[cfg(not(feature = "mini"))]
#[cfg(not(feature = "mini"))]
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

#[cfg(not(feature = "mini"))]
#[cfg(not(feature = "mini"))]
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

#[cfg(not(feature = "mini"))]
#[cfg(not(feature = "mini"))]
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

#[cfg(not(feature = "mini"))]
#[cfg(not(feature = "mini"))]
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

#[cfg(not(feature = "mini"))]
#[cfg(not(feature = "mini"))]
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

#[cfg(not(feature = "mini"))]
#[cfg(not(feature = "mini"))]
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

#[cfg(not(feature = "mini"))]
#[cfg(not(feature = "mini"))]
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

#[cfg(not(feature = "mini"))]
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

#[cfg(not(feature = "mini"))]
#[cfg(not(feature = "mini"))]
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

#[cfg(not(feature = "mini"))]
#[cfg(not(feature = "mini"))]
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

#[cfg(not(feature = "mini"))]
#[cfg(not(feature = "mini"))]
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

#[cfg(not(feature = "mini"))]
#[cfg(not(feature = "mini"))]
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

#[cfg(not(feature = "mini"))]
#[cfg(not(feature = "mini"))]
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

#[cfg(not(feature = "mini"))]
#[cfg(not(feature = "mini"))]
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

#[cfg(not(feature = "mini"))]
#[cfg(not(feature = "mini"))]
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

#[cfg(not(feature = "mini"))]
#[cfg(not(feature = "mini"))]
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

#[cfg(not(feature = "mini"))]
#[cfg(not(feature = "mini"))]
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

#[cfg(not(feature = "mini"))]
#[cfg(not(feature = "mini"))]
pub(crate) fn media_player_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::WebEngineView,
        canonical_name: "media_player",
        aliases: &["mediaplayer"],
        properties: MEDIA_PLAYER_PROPERTIES,
        events: &["playback_changed", "position_changed", "volume_changed", "source_changed"],
        commands: &["set_source", "clear_source", "play", "pause", "seek_to", "set_volume"],
    }
}

#[cfg(not(feature = "mini"))]
#[cfg(not(feature = "mini"))]
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

#[cfg(not(feature = "mini"))]
#[cfg(not(feature = "mini"))]
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

#[cfg(not(feature = "mini"))]
#[cfg(not(feature = "mini"))]
#[cfg(not(feature = "mini"))]
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

#[cfg(not(feature = "mini"))]
#[cfg(not(feature = "mini"))]
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

#[cfg(not(feature = "mini"))]
#[cfg(not(feature = "mini"))]
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

#[cfg(not(feature = "mini"))]
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
#[cfg(not(feature = "mini"))]
#[cfg(not(feature = "mini"))]
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

#[cfg(not(feature = "mini"))]
#[cfg(not(feature = "mini"))]
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

#[cfg(not(feature = "mini"))]
#[cfg(not(feature = "mini"))]
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

#[cfg(not(feature = "mini"))]
#[cfg(not(feature = "mini"))]
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

#[cfg(not(feature = "mini"))]
#[cfg(not(feature = "mini"))]
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

#[cfg(not(feature = "mini"))]
#[cfg(not(feature = "mini"))]
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

#[cfg(not(feature = "mini"))]
#[cfg(not(feature = "mini"))]
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

#[cfg(not(feature = "mini"))]
#[cfg(not(feature = "mini"))]
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

#[cfg(not(feature = "mini"))]
#[cfg(not(feature = "mini"))]
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

#[cfg(not(feature = "mini"))]
#[cfg(not(feature = "mini"))]
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

#[cfg(not(feature = "mini"))]
#[cfg(not(feature = "mini"))]
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
#[cfg(not(feature = "mini"))]
#[cfg(not(feature = "mini"))]
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
#[cfg(not(feature = "mini"))]
#[cfg(not(feature = "mini"))]
pub(crate) fn web_view_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::WebEngineView,
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
#[cfg(not(feature = "mini"))]
#[cfg(not(feature = "mini"))]
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

// ── Always-available widget property arrays ────────────────────────
// These are NOT gated behind `

// ── Always-available widget capability functions ──────────────────
#[cfg(not(feature = "mini"))]
pub(crate) fn toggle_button_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ToggleButton,
        canonical_name: "toggle_button",
        aliases: &["togglebutton", "toggle"],
        properties: TOGGLE_BUTTON_PROPERTIES,
        events: &["toggled", "checked_changed", "pressed", "released"],
        commands: &["set_checked", "toggle", "set_text"],
    }
}

pub(crate) fn arc_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Arc,
        canonical_name: "arc",
        aliases: &["arc_widget", "circular_progress"],
        properties: ARC_PROPERTIES,
        events: &["changed"],
        commands: &["set_value", "set_range"],
    }
}

pub(crate) fn spinner_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Spinner,
        canonical_name: "spinner",
        aliases: &["spinner_widget", "loading_spinner"],
        properties: SPINNER_PROPERTIES,
        events: &[],
        commands: &["set_active", "set_speed", "set_thickness", "set_size_ratio"],
    }
}

pub(crate) fn roller_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Roller,
        canonical_name: "roller",
        aliases: &["roller_widget", "scroll_wheel"],
        properties: ROLLER_PROPERTIES,
        events: &["changed"],
        commands: &["set_options", "set_selected_index", "set_visible_count"],
    }
}

pub(crate) fn dropdown_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Dropdown,
        canonical_name: "dropdown",
        aliases: &["dropdown_widget", "combo"],
        properties: DROPDOWN_PROPERTIES,
        events: &["changed"],
        commands: &["set_items", "set_selected_index", "set_expanded", "toggle"],
    }
}

pub(crate) fn text_area_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::TextArea,
        canonical_name: "text_area",
        aliases: &["textarea", "multiline_edit"],
        properties: TEXT_AREA_PROPERTIES,
        events: &["changed"],
        commands: &["set_text", "set_placeholder", "set_read_only", "insert", "delete_char"],
    }
}

pub(crate) fn keyboard_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Keyboard,
        canonical_name: "keyboard",
        aliases: &["keyboard_widget", "virtual_keyboard"],
        properties: KEYBOARD_PROPERTIES,
        events: &["key_pressed", "enter_pressed", "backspace_pressed", "space_pressed"],
        commands: &["set_layout", "set_lowercase", "toggle_shift"],
    }
}

pub(crate) fn switch_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Switch,
        canonical_name: "switch",
        aliases: &["switch_widget", "toggle_switch"],
        properties: SWITCH_PROPERTIES,
        events: &["toggled"],
        commands: &["set_checked", "toggle"],
    }
}

pub(crate) fn line_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Line,
        canonical_name: "line",
        aliases: &["line_widget", "divider"],
        properties: LINE_PROPERTIES,
        events: &[],
        commands: &["set_orientation"],
    }
}

pub(crate) fn meter_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Meter,
        canonical_name: "meter",
        aliases: &["meter_widget", "gauge"],
        properties: METER_PROPERTIES,
        events: &["changed"],
        commands: &["set_value", "set_range"],
    }
}

pub(crate) fn mini_chart_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::MiniChart,
        canonical_name: "mini_chart",
        aliases: &["minichart", "mini_chart_widget", "sparkline"],
        properties: MINI_CHART_PROPERTIES,
        events: &[],
        commands: &["set_chart_type", "set_data", "set_range"],
    }
}

pub(crate) fn image_view_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ImageView,
        canonical_name: "image_view",
        aliases: &["imageview", "image_viewer"],
        properties: IMAGE_VIEW_PROPERTIES,
        events: &[],
        commands: &["set_image", "set_scaled"],
    }
}

pub(crate) fn mini_canvas_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::MiniCanvas,
        canonical_name: "mini_canvas",
        aliases: &["minicanvas", "canvas"],
        properties: MINI_CANVAS_PROPERTIES,
        events: &["clicked", "mouse_pressed", "mouse_released"],
        commands: &["fill_rect", "draw_rect", "draw_line", "fill_circle", "clear"],
    }
}

pub(crate) fn tile_view_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::TileView,
        canonical_name: "tile_view",
        aliases: &["tileview", "page_view"],
        properties: TILE_VIEW_PROPERTIES,
        events: &["page_changed"],
        commands: &["set_current_page", "set_page_count"],
    }
}

#[cfg(not(feature = "mini"))]
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

// ── Group A property arrays (non-mini) ─────────────────────────
#[cfg(not(feature = "mini"))]
pub(crate) fn canvas_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Canvas,
        canonical_name: "canvas",
        aliases: &["canvas_widget", "drawing_surface"],
        properties: CANVAS_PROPERTIES,
        events: &["clicked", "mouse_pressed", "mouse_released"],
        commands: &["set_zoom", "set_center", "clear"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn chart_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Chart,
        canonical_name: "chart",
        aliases: &["chart_widget", "chart_surface"],
        properties: CHART_PROPERTIES,
        events: &["changed"],
        commands: &[],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn search_box_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::SearchBox,
        canonical_name: "search_box",
        aliases: &["searchbox", "search"],
        properties: SEARCH_BOX_PROPERTIES,
        events: &["changed", "search_submitted"],
        commands: &["set_text", "set_placeholder", "clear"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn badge_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Badge,
        canonical_name: "badge",
        aliases: &["badge_widget", "notification_badge"],
        properties: BADGE_PROPERTIES,
        events: &[],
        commands: &["set_text", "set_count"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn skeleton_loader_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::SkeletonLoader,
        canonical_name: "skeleton_loader",
        aliases: &["skeletonloader", "skeleton", "shimmer"],
        properties: SKELETON_LOADER_PROPERTIES,
        events: &[],
        commands: &["set_active"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn fab_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::FAB,
        canonical_name: "fab",
        aliases: &["floating_action_button", "action_button"],
        properties: FAB_PROPERTIES,
        events: &["clicked"],
        commands: &["set_icon"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn bottom_sheet_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::BottomSheet,
        canonical_name: "bottom_sheet",
        aliases: &["bottomsheet", "sheet"],
        properties: BOTTOM_SHEET_PROPERTIES,
        events: &["expanded_changed"],
        commands: &["set_expanded", "set_peek_height"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn bottom_navigation_bar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::BottomNavigationBar,
        canonical_name: "bottom_navigation_bar",
        aliases: &["bottomnavigationbar", "bottom_nav", "bottomnav"],
        properties: BOTTOM_NAVIGATION_BAR_PROPERTIES,
        events: &["selected_changed"],
        commands: &["set_selected_index", "set_items"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn navigation_drawer_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::NavigationDrawer,
        canonical_name: "navigation_drawer",
        aliases: &["navigationdrawer", "nav_drawer", "drawer"],
        properties: NAVIGATION_DRAWER_PROPERTIES,
        events: &["open_changed"],
        commands: &["set_open", "set_width"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn app_bar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::AppBar,
        canonical_name: "app_bar",
        aliases: &["appbar", "top_app_bar"],
        properties: APP_BAR_PROPERTIES,
        events: &[],
        commands: &["set_title"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn mobile_date_picker_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::MobileDatePicker,
        canonical_name: "mobile_date_picker",
        aliases: &["mobiledatepicker", "date_picker"],
        properties: MOBILE_DATE_PICKER_PROPERTIES,
        events: &["date_changed"],
        commands: &["set_selected_date"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn divider_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Divider,
        canonical_name: "divider",
        aliases: &["divider_widget", "separator"],
        properties: DIVIDER_PROPERTIES,
        events: &[],
        commands: &["set_orientation", "set_thickness"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn stepper_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Stepper,
        canonical_name: "stepper",
        aliases: &["stepper_widget", "step_control"],
        properties: STEPPER_PROPERTIES,
        events: &["changed"],
        commands: &["set_value", "set_range", "set_step"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn rating_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Rating,
        canonical_name: "rating",
        aliases: &["rating_widget", "star_rating"],
        properties: RATING_PROPERTIES,
        events: &["changed"],
        commands: &["set_value", "set_max"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn avatar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Avatar,
        canonical_name: "avatar",
        aliases: &["avatar_widget", "user_avatar"],
        properties: AVATAR_PROPERTIES,
        events: &[],
        commands: &["set_initials", "set_image"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn empty_state_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::EmptyState,
        canonical_name: "empty_state",
        aliases: &["emptystate", "empty_placeholder"],
        properties: EMPTY_STATE_PROPERTIES,
        events: &[],
        commands: &["set_message", "set_description"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn color_history_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ColorHistory,
        canonical_name: "color_history",
        aliases: &["colorhistory", "color_swatch"],
        properties: COLOR_HISTORY_PROPERTIES,
        events: &["color_selected"],
        commands: &[],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn color_well_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ColorWell,
        canonical_name: "color_well",
        aliases: &["colorwell", "color_swatch"],
        properties: COLOR_WELL_PROPERTIES,
        events: &["clicked"],
        commands: &["set_color"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn tag_input_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::TagInput,
        canonical_name: "tag_input",
        aliases: &["taginput", "tag_editor", "chip_input"],
        properties: TAG_INPUT_PROPERTIES,
        events: &["tags_changed"],
        commands: &["add_tag", "remove_tag", "set_placeholder"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn ime_preedit_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ImePreedit,
        canonical_name: "ime_preedit",
        aliases: &["imepreedit", "preedit"],
        properties: IME_PREEDIT_PROPERTIES,
        events: &[],
        commands: &["set_text", "set_cursor_position"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn inplace_editor_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::InplaceEditor,
        canonical_name: "inplace_editor",
        aliases: &["inplaceeditor", "inline_editor"],
        properties: INPLACE_EDITOR_PROPERTIES,
        events: &["edit_completed"],
        commands: &["set_text", "start_editing", "finish_editing"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn qr_code_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::QRCode,
        canonical_name: "qr_code",
        aliases: &["qrcode", "qr"],
        properties: QR_CODE_PROPERTIES,
        events: &[],
        commands: &["set_data", "set_size"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn masonry_layout_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::MasonryLayout,
        canonical_name: "masonry_layout",
        aliases: &["masonrylayout", "waterfall_layout"],
        properties: MASONRY_LAYOUT_PROPERTIES,
        events: &[],
        commands: &["set_column_count"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn material_snackbar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::MaterialSnackbar,
        canonical_name: "material_snackbar",
        aliases: &["materialsnackbar"],
        properties: MATERIAL_SNACKBAR_PROPERTIES,
        events: &["action_pressed", "dismissed"],
        commands: &["show", "dismiss", "set_message", "set_action_text"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn adaptive_scaffold_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::AdaptiveScaffold,
        canonical_name: "adaptive_scaffold",
        aliases: &["adaptivescaffold", "scaffold"],
        properties: ADAPTIVE_SCAFFOLD_PROPERTIES,
        events: &[],
        commands: &["set_title"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn wizard_dialog_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::WizardDialog,
        canonical_name: "wizard_dialog",
        aliases: &["wizarddialog", "wizard"],
        properties: WIZARD_DIALOG_PROPERTIES,
        events: &["finished", "cancelled"],
        commands: &["set_current_step", "next", "back", "finish"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn safe_area_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::SafeArea,
        canonical_name: "safe_area",
        aliases: &["safearea", "safe_area_insets"],
        properties: SAFE_AREA_PROPERTIES,
        events: &[],
        commands: &["set_insets"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn cupertino_alert_dialog_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::CupertinoAlertDialog,
        canonical_name: "cupertino_alert_dialog",
        aliases: &["cupertinoalertdialog", "ios_alert"],
        properties: CUPERTINO_ALERT_DIALOG_PROPERTIES,
        events: &["confirmed", "cancelled"],
        commands: &["set_title", "set_message"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn cupertino_slider_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::CupertinoSlider,
        canonical_name: "cupertino_slider",
        aliases: &["cupertinoslider", "ios_slider"],
        properties: CUPERTINO_SLIDER_PROPERTIES,
        events: &["value_changed"],
        commands: &["set_value", "set_min", "set_max"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn tooltip_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Tooltip,
        canonical_name: "tooltip",
        aliases: &["tooltip_widget", "hover_tip"],
        properties: TOOLTIP_PROPERTIES,
        events: &[],
        commands: &["set_text", "show", "hide"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn segmented_button_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::SegmentedButton,
        canonical_name: "segmented_button",
        aliases: &["segmentedbutton", "segmented_btn"],
        properties: SEGMENTED_BUTTON_PROPERTIES,
        events: &["selected_changed"],
        commands: &["set_selected_index"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn navigation_stack_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::NavigationStack,
        canonical_name: "navigation_stack",
        aliases: &["navigationstack", "nav_stack"],
        properties: NAVIGATION_STACK_PROPERTIES,
        events: &["page_changed"],
        commands: &["push", "pop", "set_current_page"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn progress_circle_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ProgressCircle,
        canonical_name: "progress_circle",
        aliases: &["progresscircle", "circular_progress"],
        properties: PROGRESS_CIRCLE_PROPERTIES,
        events: &[],
        commands: &["set_value", "set_thickness", "set_indeterminate"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn icon_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Icon,
        canonical_name: "icon",
        aliases: &["icon_widget", "icon_display"],
        properties: ICON_PROPERTIES,
        events: &[],
        commands: &["set_icon_name", "set_size"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn dropdown_menu_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::DropdownMenu,
        canonical_name: "dropdown_menu",
        aliases: &["dropdownmenu"],
        properties: DROPDOWN_MENU_PROPERTIES,
        events: &["selected_changed"],
        commands: &["set_selected_index", "set_expanded"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn masked_edit_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::MaskedEdit,
        canonical_name: "masked_edit",
        aliases: &["maskededit", "masked_input"],
        properties: MASKED_EDIT_PROPERTIES,
        events: &["changed"],
        commands: &["set_text", "set_mask"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn menu_button_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::MenuButton,
        canonical_name: "menu_button",
        aliases: &["menubutton", "dropdown_button"],
        properties: MENU_BUTTON_PROPERTIES,
        events: &["selected_changed"],
        commands: &["set_text", "set_expanded"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn popover_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Popover,
        canonical_name: "popover",
        aliases: &["popover_widget", "popup_card"],
        properties: POPOVER_PROPERTIES,
        events: &[],
        commands: &["set_visible", "set_text"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn auto_complete_edit_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::AutoCompleteEdit,
        canonical_name: "auto_complete_edit",
        aliases: &["autocompleteedit", "autocomplete"],
        properties: AUTO_COMPLETE_EDIT_PROPERTIES,
        events: &["changed", "selected"],
        commands: &["set_text"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn multi_select_combo_box_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::MultiSelectComboBox,
        canonical_name: "multi_select_combo_box",
        aliases: &["multiselectcombobox", "multi_combo"],
        properties: MULTI_SELECT_COMBO_BOX_PROPERTIES,
        events: &["selection_changed"],
        commands: &["set_expanded"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn range_slider_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::RangeSlider,
        canonical_name: "range_slider",
        aliases: &["rangeslider", "dual_slider"],
        properties: RANGE_SLIDER_PROPERTIES,
        events: &["changed"],
        commands: &["set_lower", "set_upper", "set_range"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn floating_label_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::FloatingLabel,
        canonical_name: "floating_label",
        aliases: &["floatinglabel", "floating_input"],
        properties: FLOATING_LABEL_PROPERTIES,
        events: &["changed", "focused"],
        commands: &["set_text", "set_placeholder", "set_focused"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn font_preview_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::FontPreview,
        canonical_name: "font_preview",
        aliases: &["fontpreview", "font_viewer"],
        properties: FONT_PREVIEW_PROPERTIES,
        events: &[],
        commands: &["set_font_family", "set_font_size", "set_preview_text"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn cupertino_navigation_bar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::CupertinoNavigationBar,
        canonical_name: "cupertino_navigation_bar",
        aliases: &["cupertinonavbar", "ios_nav_bar"],
        properties: CUPERTINO_NAVIGATION_BAR_PROPERTIES,
        events: &["back_pressed"],
        commands: &["set_title", "set_large_title"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn cupertino_segmented_control_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::CupertinoSegmentedControl,
        canonical_name: "cupertino_segmented_control",
        aliases: &["cupertinosegmented", "ios_segmented"],
        properties: CUPERTINO_SEGMENTED_CONTROL_PROPERTIES,
        events: &["value_changed"],
        commands: &["set_selected_index"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn refresh_control_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::RefreshControl,
        canonical_name: "refresh_control",
        aliases: &["refreshcontrol", "pull_to_refresh"],
        properties: REFRESH_CONTROL_PROPERTIES,
        events: &["refreshed"],
        commands: &["set_refreshing"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn modal_bottom_sheet_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ModalBottomSheet,
        canonical_name: "modal_bottom_sheet",
        aliases: &["modalbottomsheet", "modal_sheet"],
        properties: MODAL_BOTTOM_SHEET_PROPERTIES,
        events: &["dismissed"],
        commands: &["set_visible", "show", "dismiss"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn find_replace_dialog_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::FindReplaceDialog,
        canonical_name: "find_replace_dialog",
        aliases: &["findreplacedialog", "find_replace"],
        properties: FIND_REPLACE_DIALOG_PROPERTIES,
        events: &["find", "replace", "replace_all"],
        commands: &["set_find_text", "set_replace_text", "set_match_case", "set_wrap_around"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn properties_panel_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::PropertiesPanel,
        canonical_name: "properties_panel",
        aliases: &["propertiespanel", "property_panel"],
        properties: PROPERTIES_PANEL_PROPERTIES,
        events: &["property_changed"],
        commands: &[],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn cupertino_date_picker_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::CupertinoDatePicker,
        canonical_name: "cupertino_date_picker",
        aliases: &["cupertinodatepicker", "ios_date_picker"],
        properties: CUPERTINO_DATE_PICKER_PROPERTIES,
        events: &["date_changed"],
        commands: &["set_selected_date"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn editable_combo_box_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::EditableComboBox,
        canonical_name: "editable_combo_box",
        aliases: &["editablecombobox", "editable_combo"],
        properties: EDITABLE_COMBO_BOX_PROPERTIES,
        events: &["changed"],
        commands: &["set_text"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn date_range_picker_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::DateRangePicker,
        canonical_name: "date_range_picker",
        aliases: &["daterangepicker", "range_picker"],
        properties: DATE_RANGE_PICKER_PROPERTIES,
        events: &["range_changed"],
        commands: &["set_start_date", "set_end_date"],
    }
}

// ── New widget properties (non-mini) ────────────────────────────
// ── New widget capability functions (non-mini) ──────────────────
#[cfg(not(feature = "mini"))]
pub(crate) fn rich_edit_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::RichEdit,
        canonical_name: "rich_edit",
        aliases: &["richedit", "rich_text_editor"],
        properties: RICH_EDIT_PROPERTIES,
        events: &["text_changed"],
        commands: &["set_text"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn carousel_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Carousel,
        canonical_name: "carousel",
        aliases: &["carousel_widget", "swipe_view"],
        properties: CAROUSEL_PROPERTIES,
        events: &["page_changed"],
        commands: &[],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn material_navigation_rail_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::MaterialNavigationRail,
        canonical_name: "material_navigation_rail",
        aliases: &["materialnavigationrail", "nav_rail", "navigation_rail"],
        properties: MATERIAL_NAVIGATION_RAIL_PROPERTIES,
        events: &["selected_changed"],
        commands: &["set_selected_index"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn tab_view_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::TabView,
        canonical_name: "tab_view",
        aliases: &["tabview", "page_tab_view"],
        properties: TAB_VIEW_PROPERTIES,
        events: &["tab_changed"],
        commands: &["set_selected_index"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn search_bar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::SearchBar,
        canonical_name: "search_bar",
        aliases: &["searchbar", "search_field"],
        properties: SEARCH_BAR_PROPERTIES,
        events: &["text_changed", "search_submitted"],
        commands: &["set_text", "set_placeholder"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn shortcut_editor_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ShortcutEditor,
        canonical_name: "shortcut_editor",
        aliases: &["shortcuteditor", "keyboard_shortcut_editor"],
        properties: SHORTCUT_EDITOR_PROPERTIES,
        events: &["shortcut_changed"],
        commands: &["set_filter_text"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn swipe_to_dismiss_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::SwipeToDismiss,
        canonical_name: "swipe_to_dismiss",
        aliases: &["swipetodismiss", "swipe_dismiss"],
        properties: SWIPE_TO_DISMISS_PROPERTIES,
        events: &["dismissed"],
        commands: &[],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn pager_page_view_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::PagerPageView,
        canonical_name: "pager_page_view",
        aliases: &["pagerpageview", "pager_view", "page_view"],
        properties: PAGER_PAGE_VIEW_PROPERTIES,
        events: &["page_changed"],
        commands: &["set_current_page"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn line_chart_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::LineChart,
        canonical_name: "line_chart",
        aliases: &["linechart", "line_graph"],
        properties: LINE_CHART_PROPERTIES,
        events: &["changed"],
        commands: &["set_stroke_width"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn sparkline_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Sparkline,
        canonical_name: "sparkline",
        aliases: &["sparkline_chart", "spark_line"],
        properties: SPARKLINE_PROPERTIES,
        events: &[],
        commands: &["set_stroke_width"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn bar_chart_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::BarChart,
        canonical_name: "bar_chart",
        aliases: &["barchart", "bar_graph"],
        properties: BAR_CHART_PROPERTIES,
        events: &["changed"],
        commands: &["set_bar_spacing"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn pie_chart_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::PieChart,
        canonical_name: "pie_chart",
        aliases: &["piechart", "pie_graph"],
        properties: PIE_CHART_PROPERTIES,
        events: &["changed"],
        commands: &["set_donut"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn animated_image_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::AnimatedImage,
        canonical_name: "animated_image",
        aliases: &["animatedimage", "anim_image"],
        properties: ANIMATED_IMAGE_PROPERTIES,
        events: &["finished"],
        commands: &["set_playing"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn hero_animation_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::HeroAnimation,
        canonical_name: "hero_animation",
        aliases: &["heroanimation", "hero"],
        properties: HERO_ANIMATION_PROPERTIES,
        events: &["animation_completed"],
        commands: &["set_animation_progress"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn bezier_curve_editor_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::BezierCurveEditor,
        canonical_name: "bezier_curve_editor",
        aliases: &["beziercurveeditor", "curve_editor"],
        properties: BEZIER_CURVE_EDITOR_PROPERTIES,
        events: &["changed"],
        commands: &["set_snap_to_grid"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn lottie_widget_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::LottieWidget,
        canonical_name: "lottie_widget",
        aliases: &["lottiewidget", "lottie"],
        properties: LOTTIE_WIDGET_PROPERTIES,
        events: &["finished"],
        commands: &["set_playing"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn rive_widget_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::RiveWidget,
        canonical_name: "rive_widget",
        aliases: &["rivewidget", "rive"],
        properties: RIVE_WIDGET_PROPERTIES,
        events: &["finished"],
        commands: &["set_playing"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn video_player_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::VideoPlayer,
        canonical_name: "video_player",
        aliases: &["videoplayer", "video"],
        properties: VIDEO_PLAYER_PROPERTIES,
        events: &["finished"],
        commands: &["set_playing", "set_volume"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn image_gallery_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ImageGallery,
        canonical_name: "image_gallery",
        aliases: &["imagegallery", "gallery"],
        properties: IMAGE_GALLERY_PROPERTIES,
        events: &["image_changed"],
        commands: &["set_current_index"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn audio_visualizer_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::AudioVisualizer,
        canonical_name: "audio_visualizer",
        aliases: &["audiovisualizer", "audio_viz"],
        properties: AUDIO_VISUALIZER_PROPERTIES,
        events: &[],
        commands: &["set_bar_count"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn camera_preview_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::CameraPreview,
        canonical_name: "camera_preview",
        aliases: &["camerapreview", "camera"],
        properties: CAMERA_PREVIEW_PROPERTIES,
        events: &[],
        commands: &["set_active"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn barcode_scanner_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::BarcodeScanner,
        canonical_name: "barcode_scanner",
        aliases: &["barcodescanner", "scanner"],
        properties: BARCODE_SCANNER_PROPERTIES,
        events: &["barcode_detected"],
        commands: &["set_scanning"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn tool_button_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ToolButton,
        canonical_name: "tool_button",
        aliases: &["toolbutton", "toolbar_button"],
        properties: TOOL_BUTTON_PROPERTIES,
        events: &["clicked", "toggled"],
        commands: &["set_text", "set_checked"],
    }
}

#[cfg(not(feature = "mini"))]
pub(crate) fn status_bar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::StatusBar,
        canonical_name: "status_bar",
        aliases: &["statusbar", "status"],
        properties: STATUS_BAR_PROPERTIES,
        events: &["message_changed"],
        commands: &["set_message"],
    }
}

/// Property grid properties: property_count (read-only), selected_index (r/w).
#[cfg(not(feature = "mini"))]
pub(crate) fn property_grid_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::PropertyGrid,
        canonical_name: "property_grid",
        aliases: &["propertygrid", "inspector"],
        properties: PROPERTY_GRID_PROPERTIES,
        events: &["selected"],
        commands: &["add_property", "set_value", "clear", "set_selected_index"],
    }
}
