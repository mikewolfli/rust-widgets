// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use super::types::{PropertySchema, PropertyValueKind, WidgetCapability};
use crate::widget::WidgetKind;

// ── Published event schemas (generated from the signals) ─────────
// Included ahead of the capability constructors so `events_of!` is in scope for them. The
// generated table is the single place a name and its payload type are written down, which is
// what keeps the two from drifting (BLUE19 rule #95).
include!("event_payloads.rs");

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
        events: events_of!("button"),
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
        // Deliberately empty. BLUE20 §1.7 recorded this as a gap because
        // `Platform::create_checkbox` had no literal match here — but `WidgetFactory`
        // stores every name under `normalize_key`, which strips `_`/`-`/spaces, so
        // `"checkbox"` already resolves to `check_box` and adding the alias would be a
        // no-op that breaks `capability_alias_hygiene_test::no_alias_is_inert_under_normalisation`.
        // The gap was in the audit script's criterion, not in this table (rule #110).
        aliases: &[],
        properties: CHECK_BOX_PROPERTIES,
        events: events_of!("check_box"),
        commands: &["set_checked", "toggle", "set_state"],
    }
}

pub(crate) fn radio_button_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::RadioButton,
        canonical_name: "radio_button",
        aliases: &[],
        properties: RADIO_BUTTON_PROPERTIES,
        events: events_of!("radio_button"),
        commands: &["set_checked", "set_group_id"],
    }
}

pub(crate) fn slider_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Slider,
        canonical_name: "slider",
        aliases: &[],
        properties: SLIDER_PROPERTIES,
        events: events_of!("slider"),
        commands: &["set_range", "set_value", "set_slider_position"],
    }
}

pub(crate) fn progress_bar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ProgressBar,
        canonical_name: "progress_bar",
        aliases: &[],
        properties: PROGRESS_BAR_PROPERTIES,
        events: events_of!("progress_bar"),
        commands: &["set_range", "set_value", "set_orientation"],
    }
}

pub(crate) fn scroll_bar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ScrollBar,
        canonical_name: "scroll_bar",
        aliases: &[],
        properties: SCROLL_BAR_PROPERTIES,
        events: events_of!("scroll_bar"),
        commands: &["set_range", "set_value", "set_steps", "set_orientation"],
    }
}

pub(crate) fn list_box_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ListBox,
        canonical_name: "list_box",
        aliases: &[],
        properties: LIST_BOX_PROPERTIES,
        events: events_of!("list_box"),
        commands: &["add_item", "remove_item", "clear", "clear_selection", "set_selection_mode"],
    }
}

pub(crate) fn spin_box_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::SpinBox,
        canonical_name: "spin_box",
        aliases: &[],
        properties: SPIN_BOX_PROPERTIES,
        events: events_of!("spin_box"),
        commands: &["set_range", "set_value", "step_up", "step_down"],
    }
}

pub(crate) fn combo_box_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ComboBox,
        canonical_name: "combo_box",
        aliases: &[],
        properties: COMBO_BOX_PROPERTIES,
        events: events_of!("combo_box"),
        commands: &["set_items", "set_current_index", "clear"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn dial_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Dial,
        canonical_name: "dial",
        aliases: &["knob"],
        properties: DIAL_PROPERTIES,
        events: events_of!("dial"),
        commands: &["set_range", "set_value", "set_wrapping"],
    }
}

pub(crate) fn window_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Window,
        canonical_name: "window",
        aliases: &["main_window"],
        properties: WINDOW_PROPERTIES,
        events: events_of!("window"),
        commands: &["set_title", "close"],
    }
}

pub(crate) fn group_box_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::GroupBox,
        canonical_name: "group_box",
        aliases: &[],
        properties: GROUP_BOX_PROPERTIES,
        events: events_of!("group_box"),
        commands: &["set_title", "set_checkable", "set_checked", "toggle"],
    }
}

/// The capability for the `panel` entry — the plain container spelling.
///
/// # Why the schema and the kind both point at `GroupBox`
///
/// `Panel` is a `pub type` for `GroupBox` (`src/widget/mod.rs`), so `create_panel(..)`
/// builds a `GroupBox` and the control reports `WidgetKind::GroupBox`. Naming the kind
/// here as `Panel` was the earlier attempt, and it produced a second, undecidable
/// question: a mounted `GroupBox` then claimed *two* kinds depending on which entry it
/// came from, so the property contract had to publish a schema that matched neither
/// entry — which the schema/contract gate correctly rejected.
///
/// What was actually wrong was smaller: `WidgetKind::Panel` had **no capability named
/// after it**, so `create_panel(..)` resolved to whichever kind-`Panel` entry was
/// registered first (`breadcrumb`). Registering the `panel` name against the kind the
/// control really reports removes the ordering from that lookup and keeps the mounted
/// widget's kind, its schema and its contract agreeing with each other.
#[cfg(full_widgets)]
pub(crate) fn panel_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::GroupBox,
        canonical_name: "panel",
        aliases: &[],
        properties: GROUP_BOX_PROPERTIES,
        events: events_of!("panel"),
        commands: &["set_title", "set_checkable", "set_checked", "toggle"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn frame_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Frame,
        canonical_name: "frame",
        aliases: &["frame_widget"],
        properties: FRAME_PROPERTIES,
        events: &[],
        commands: &["set_frame_shape", "set_frame_shadow", "set_line_width"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn splitter_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Splitter,
        canonical_name: "splitter",
        aliases: &["pane_splitter"],
        properties: SPLITTER_PROPERTIES,
        events: events_of!("splitter"),
        commands: &["set_orientation"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn lcd_number_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::LCDNumber,
        canonical_name: "lcd_number",
        aliases: &[],
        properties: LCD_NUMBER_PROPERTIES,
        events: events_of!("lcd_number"),
        commands: &["set_value", "set_mode", "set_segment_style"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn command_link_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::CommandLink,
        canonical_name: "command_link",
        aliases: &[],
        properties: COMMAND_LINK_PROPERTIES,
        events: events_of!("command_link"),
        commands: &["set_text", "set_description", "set_enabled", "click"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn font_combo_box_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::FontComboBox,
        canonical_name: "font_combo_box",
        aliases: &[],
        properties: FONT_COMBO_BOX_PROPERTIES,
        events: events_of!("font_combo_box"),
        commands: &[
            "set_current_index",
            "set_editable",
            "set_max_visible_items",
            "show_popup",
            "hide_popup",
        ],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn action_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Action,
        canonical_name: "action",
        aliases: &["command_action"],
        properties: ACTION_PROPERTIES,
        events: events_of!("action"),
        commands: &["set_text", "set_checkable", "set_checked", "trigger"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn tool_box_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Toolbox,
        canonical_name: "tool_box",
        aliases: &[],
        properties: TOOL_BOX_PROPERTIES,
        events: events_of!("tool_box"),
        commands: &["add_item", "remove_item", "set_current_index", "set_orientation"],
    }
}

// NOTE: there is deliberately no second `toolbox` entry here.
//
// One existed, registered against the same kind, so `tool_box` and `toolbox` were two
// capabilities for one control. That made `capability("toolbox")` answer by
// registration order and gave `WidgetKind::Toolbox` two entries to disambiguate between
// with a tie-break row — all to buy a spelling `normalize_key` cannot distinguish from
// `tool_box` (`"toolbox"` and `"tool_box"` are the same key). The single `tool_box`
// entry accepts both spellings, so the second entry was duplication (rule #54) with no
// behaviour behind it.

#[cfg(not(alloc_frugal))]
pub(crate) fn tab_bar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::TabBar,
        canonical_name: "tab_bar",
        aliases: &[],
        properties: TAB_BAR_PROPERTIES,
        events: events_of!("tab_bar"),
        commands: &["add_tab", "remove_tab", "set_current_index", "set_closable", "set_movable"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn calendar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Calendar,
        canonical_name: "calendar",
        aliases: &["date_calendar"],
        properties: CALENDAR_PROPERTIES,
        events: events_of!("calendar"),
        commands: &["set_selected_date", "set_date_range", "set_first_day_of_week", "show_today"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn date_edit_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::DatePicker,
        canonical_name: "date_edit",
        // `date_picker` is the variant's own spelling (`WidgetKind::DatePicker`,
        // and `pub type DatePicker = DateEdit` in `src/widget/mod.rs`), so a caller
        // naming the control after the kind it is declared under must reach it. Its
        // two siblings already answer to theirs (`time_picker`, `date_time_picker`),
        // and the absence here made `factory.create("date_picker", ..)` return `None`
        // while `factory.create("time_picker", ..)` succeeded.
        aliases: &["date_picker"],
        properties: DATE_EDIT_PROPERTIES,
        events: events_of!("date_edit"),
        commands: &["set_date", "set_minimum_date", "set_maximum_date", "set_display_format"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn time_edit_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::TimePicker,
        canonical_name: "time_edit",
        aliases: &["time_picker"],
        properties: TIME_EDIT_PROPERTIES,
        events: events_of!("time_edit"),
        commands: &["set_time", "set_minimum_time", "set_maximum_time", "set_display_format"],
    }
}

pub(crate) fn line_edit_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::LineEdit,
        canonical_name: "line_edit",
        aliases: &["text_input", "input"],
        properties: LINE_EDIT_PROPERTIES,
        events: events_of!("line_edit"),
        commands: &["clear", "select_all"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn list_view_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ListView,
        canonical_name: "list_view",
        aliases: &[],
        properties: LIST_VIEW_PROPERTIES,
        events: events_of!("list_view"),
        commands: &["clear_selection", "clear_focused_row"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn tree_view_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::TreeView,
        canonical_name: "tree_view",
        aliases: &[],
        properties: TREE_VIEW_PROPERTIES,
        events: events_of!("tree_view"),
        commands: &["clear_selection", "clear_focused_node"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn table_widget_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Table,
        canonical_name: "table_widget",
        aliases: &[],
        properties: TABLE_WIDGET_PROPERTIES,
        events: events_of!("table_widget"),
        commands: &["clear_selection", "clear_focused_row"],
    }
}

/// The capability for `WidgetKind::Table`, the kind the public `create_table(..)`
/// mounts — and the one every `table` spelling also addresses.
///
/// # Why `table` was an alias of `table_widget` before this row existed
///
/// Because there was no other way to make it resolve. With no entry declaring
/// `WidgetKind::Table` as its canonical name, `capability_by_kind(Table)` fell back
/// to `indices[0]` — **whichever of `table_widget`, `data_grid`, `tree_table`,
/// `virtual_table` happened to be registered first**. That order is invisible to
/// every caller-facing check: the id that comes back is valid, the control mounts,
/// and only the *kind* of control is wrong. A row whose `canonical_name` is `table`
/// removes the ordering from the answer entirely: `capability_by_kind` matches on
/// `canonical_name == "table"`, so the resolution is the same for every registration
/// order (rule #2: fix the class, not the instance).
///
/// The property schema is the one `table_widget` published, so no caller sees a
/// change in behaviour — only a stable answer.
#[cfg(not(alloc_frugal))]
pub(crate) fn table_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Table,
        canonical_name: "table",
        aliases: &[],
        properties: TABLE_WIDGET_PROPERTIES,
        events: events_of!("table"),
        commands: &["clear_selection", "clear_focused_row"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn data_grid_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Table,
        canonical_name: "data_grid",
        aliases: &[],
        properties: DATA_GRID_PROPERTIES,
        events: events_of!("data_grid"),
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

#[cfg(not(alloc_frugal))]
pub(crate) fn tree_table_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::TreeTable,
        canonical_name: "tree_table",
        aliases: &[],
        properties: TREE_TABLE_PROPERTIES,
        events: events_of!("tree_table"),
        commands: &["set_model", "clear_model", "expand_row", "collapse_row", "select_row"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn virtual_table_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Table,
        canonical_name: "virtual_table",
        aliases: &[],
        properties: VIRTUAL_TABLE_PROPERTIES,
        events: events_of!("virtual_table"),
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

#[cfg(not(alloc_frugal))]
pub(crate) fn virtual_list_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::DataView,
        canonical_name: "virtual_list",
        aliases: &[],
        properties: VIRTUAL_LIST_PROPERTIES,
        events: events_of!("virtual_list"),
        commands: &["clear_data_source", "fetch_visible_rows"],
    }
}

/// The capability for `WidgetKind::DataView`, the kind the public
/// `create_data_view(..)` mounts.
///
/// # Why this exists next to `virtual_list_capability`
///
/// `DataView` and `VirtualList` are the same Rust type (`widget::mod.rs` declares
/// `pub type DataView = VirtualList;`), so both kinds are served by one constructor
/// and the factory keeps one entry whose canonical name is `virtual_list`.
///
/// The kind→capability index still has to answer for `DataView` — `create_data_view`
/// mounts exactly that kind — and the answer it used to give was **whichever entry
/// registered first**, because no entry declared the kind and the fallback in
/// `capability_by_kind` takes `indices[0]`. That is the same order-dependent
/// resolution that made `ToolButton` build a `split_button` and `Table` build a
/// `tree_table`; it is silent because the id is valid.
///
/// Rule #54 argues against a second *type*, and this is not one: it registers the
/// kind under its own name while keeping the same type, the same property schema and
/// therefore the same behaviour. The alternative — adding `dataview` as an alias of
/// `virtual_list` — was rejected because `capability_by_kind` matches on
/// `canonical_name`, so an alias would leave the resolution order-dependent and the
/// defect reachable again by a different registration order.
#[cfg(not(alloc_frugal))]
pub(crate) fn data_view_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::DataView,
        canonical_name: "data_view",
        aliases: &[],
        properties: VIRTUAL_LIST_PROPERTIES,
        events: events_of!("data_view"),
        commands: &["clear_data_source", "fetch_visible_rows"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn menu_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Menu,
        canonical_name: "menu",
        aliases: &["context_menu"],
        properties: MENU_PROPERTIES,
        events: events_of!("menu"),
        commands: &["clear", "add_action", "add_separator"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn menu_bar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::MenuBar,
        canonical_name: "menu_bar",
        aliases: &[],
        properties: MENU_BAR_PROPERTIES,
        events: events_of!("menu_bar"),
        commands: &["clear", "add_menu", "remove_menu"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn tool_bar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ToolBar,
        canonical_name: "tool_bar",
        aliases: &[],
        properties: TOOL_BAR_PROPERTIES,
        events: events_of!("tool_bar"),
        commands: &["clear", "add_action", "add_separator"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn ribbon_bar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::RibbonBar,
        canonical_name: "ribbon_bar",
        aliases: &["ribbon"],
        properties: RIBBON_BAR_PROPERTIES,
        events: events_of!("ribbon_bar"),
        commands: &["add_tab", "add_group", "add_item", "add_large_item", "clear"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn color_picker_capability() -> WidgetCapability {
    WidgetCapability {
        // `ColorPicker`, not `ColorDialog`: since BLUE16 phase E-6 the inline picker
        // declares its own kind instead of borrowing the dialog's. Before that, the
        // factory lookup, the accessibility role and the CSS selector all answered
        // "dialog" for a control with no window.
        kind: WidgetKind::ColorPicker,
        canonical_name: "color_picker",
        aliases: &[],
        properties: COLOR_PICKER_PROPERTIES,
        events: events_of!("color_picker"),
        commands: &["set_hex", "apply_preset"],
    }
}

/// The modal colour dialog — a window that *hosts* a picker.
///
/// # Why this is separate from `color_picker`
///
/// `ColorDialog` is a `WidgetKind` of its own (it has a window, a title, and
/// accept/reject) and `ColorPicker` is the inline control inside it. Before phase
/// E-6 one capability served both, which is why `color_dialog` was an *alias* of
/// `color_picker` — and why the dialog's kind had no canonical name of its own.
/// Splitting the picker onto its own kind therefore requires the dialog to get a
/// registration too, or `kind_factory_name(ColorDialog)` resolves to nothing and
/// `create_color_dialog` returns 0.
///
/// `color_dialog` is no longer an alias of the picker, so the two names now address
/// two different controls rather than one control under two names.
#[cfg(not(alloc_frugal))]
pub(crate) fn color_dialog_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ColorDialog,
        canonical_name: "color_dialog",
        aliases: &["colour_dialog"],
        properties: COLOR_DIALOG_PROPERTIES,
        events: events_of!("color_dialog"),
        commands: &["set_hex", "apply_preset"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn code_editor_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::RichEdit,
        canonical_name: "code_editor",
        aliases: &[],
        properties: CODE_EDITOR_PROPERTIES,
        events: events_of!("code_editor"),
        commands: &["set_text", "append_line", "set_markers", "set_cursor"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn gantt_widget_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Chart,
        canonical_name: "gantt_widget",
        aliases: &["gantt"],
        properties: GANTT_WIDGET_PROPERTIES,
        events: events_of!("gantt_widget"),
        commands: &["set_tasks", "zoom", "set_viewport", "select_task"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn terminal_view_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::TextEdit,
        canonical_name: "terminal_view",
        aliases: &["terminal"],
        properties: TERMINAL_VIEW_PROPERTIES,
        events: events_of!("terminal_view"),
        commands: &["append_output", "submit"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn snackbar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::StatusBar,
        canonical_name: "snackbar",
        aliases: &["toast_bar"],
        properties: SNACKBAR_PROPERTIES,
        events: events_of!("snackbar"),
        commands: &["show", "show_with_action", "dismiss"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn map_view_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Canvas,
        canonical_name: "map_view",
        aliases: &[],
        properties: MAP_VIEW_PROPERTIES,
        events: events_of!("map_view"),
        commands: &["set_markers", "set_center_x", "set_center_y", "set_zoom"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn media_player_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::WebEngineView,
        canonical_name: "media_player",
        aliases: &[],
        properties: MEDIA_PLAYER_PROPERTIES,
        events: events_of!("media_player"),
        commands: &["set_source", "clear_source", "play", "pause", "seek_to", "set_volume"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn breadcrumb_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Breadcrumb,
        canonical_name: "breadcrumb",
        aliases: &["nav_breadcrumb"],
        properties: BREADCRUMB_PROPERTIES,
        events: events_of!("breadcrumb"),
        commands: &["set_segments", "push_segment", "clear_segments"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn signature_pad_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::SignaturePad,
        canonical_name: "signature_pad",
        aliases: &["signature"],
        properties: SIGNATURE_PAD_PROPERTIES,
        events: events_of!("signature_pad"),
        commands: &["undo"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn drop_zone_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::DropZone,
        canonical_name: "drop_zone",
        aliases: &["drop_target"],
        properties: DROP_ZONE_PROPERTIES,
        events: events_of!("drop_zone"),
        commands: &[],
    }
}

// The six capabilities below describe controls that were implemented but never
// registered, so nothing could construct them by name (no JSON node, no CSS
// selector target, no factory lookup). Registration is the whole fix; the
// `commands` lists name only methods that actually exist on each type, because a
// command name a consumer cannot invoke is the same defect shape as an
// unreachable control.
#[cfg(not(alloc_frugal))]
pub(crate) fn timeline_widget_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Chart,
        canonical_name: "timeline_widget",
        aliases: &["timeline", "timeline_view"],
        properties: TIMELINE_WIDGET_PROPERTIES,
        events: events_of!("timeline_widget"),
        commands: &["set_items", "set_viewport", "zoom", "select_index"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn command_palette_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ListView,
        canonical_name: "command_palette",
        aliases: &["command_box"],
        properties: COMMAND_PALETTE_PROPERTIES,
        events: events_of!("command_palette"),
        commands: &[
            "set_entries",
            "set_query",
            "clear_query",
            "move_highlight",
            "activate_highlighted",
        ],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn notification_center_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ListView,
        canonical_name: "notification_center",
        aliases: &["notifications"],
        properties: NOTIFICATION_CENTER_PROPERTIES,
        events: events_of!("notification_center"),
        commands: &[
            "push",
            "clear",
            "set_read",
            "mark_all_read",
            "select_index",
            "activate_selected",
        ],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn diff_viewer_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Table,
        canonical_name: "diff_viewer",
        aliases: &["diff"],
        properties: DIFF_VIEWER_PROPERTIES,
        events: events_of!("diff_viewer"),
        commands: &["set_texts", "select_index"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn markdown_editor_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::RichEdit,
        canonical_name: "markdown_editor",
        aliases: &["md_editor"],
        properties: MARKDOWN_EDITOR_PROPERTIES,
        events: events_of!("markdown_editor"),
        commands: &["set_text", "append_line", "toggle_preview_mode", "set_preview_mode", "undo"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn toast_stack_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::PopupWindow,
        canonical_name: "toast_stack",
        aliases: &["toasts"],
        properties: TOAST_STACK_PROPERTIES,
        events: events_of!("toast_stack"),
        commands: &["push", "clear", "select_index", "activate_selected", "dismiss_selected"],
    }
}

// Found by the registration-fidelity gate rather than by hand: `GridTableWidget`
// is a complete virtualised table that was exported but never registered, so it
// joins the six above as a seventh接线-only fix. It reports `WidgetKind::GridTable`,
// which no other capability claims.
#[cfg(not(alloc_frugal))]
pub(crate) fn otp_input_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::OtpInput,
        canonical_name: "otp_input",
        aliases: &["otp"],
        properties: OTP_INPUT_PROPERTIES,
        events: events_of!("otp_input"),
        commands: &["insert_char", "backspace", "paste", "clear"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn banner_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Banner,
        canonical_name: "banner",
        aliases: &["notice"],
        properties: BANNER_PROPERTIES,
        events: events_of!("banner"),
        commands: &["dismiss", "show", "set_actions", "activate_action"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn toast_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Toast,
        canonical_name: "toast",
        aliases: &["notification", "toast_message"],
        properties: TOAST_PROPERTIES,
        events: events_of!("toast"),
        commands: &["dismiss", "set_message", "set_level"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn splash_screen_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::SplashScreen,
        canonical_name: "splash_screen",
        aliases: &["splash"],
        properties: SPLASH_SCREEN_PROPERTIES,
        // Dismissal is by the program, so the events are lifecycle rather than
        // interaction: `finished` when initialisation completes, `skipped` when the
        // user takes the optional skip affordance.
        events: events_of!("splash_screen"),
        commands: &["finish", "set_progress", "set_title"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn pagination_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Pagination,
        canonical_name: "pagination",
        aliases: &["pager", "page_numbers"],
        properties: PAGINATION_PROPERTIES,
        events: events_of!("pagination"),
        commands: &["next_page", "previous_page", "set_page", "set_total"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn number_picker_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::NumberPicker,
        canonical_name: "number_picker",
        aliases: &["picker"],
        properties: NUMBER_PICKER_PROPERTIES,
        events: events_of!("number_picker"),
        commands: &["scroll_rows", "set_value", "set_range", "set_step"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn grid_table_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::GridTable,
        canonical_name: "grid_table",
        aliases: &[],
        properties: GRID_TABLE_WIDGET_PROPERTIES,
        events: events_of!("grid_table"),
        commands: &[
            "set_data_source",
            "clear_data_source",
            "set_scroll_row",
            "set_scroll_column",
            "toggle_sort_column",
            "clear_selection",
        ],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn split_button_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ToolButton,
        canonical_name: "split_button",
        aliases: &[],
        properties: SPLIT_BUTTON_PROPERTIES,
        events: events_of!("split_button"),
        commands: &["add_action", "open_menu", "close_menu", "trigger_primary"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn segmented_control_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ToggleButton,
        canonical_name: "segmented_control",
        aliases: &[],
        properties: SEGMENTED_CONTROL_PROPERTIES,
        events: events_of!("segmented_control"),
        commands: &["set_items", "move_selection"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn chip_capability() -> WidgetCapability {
    WidgetCapability {
        // `WidgetKind::Chip` names this control. It previously declared
        // `CheckListBox` — which is a *type alias for `ListBox`* — so the lookup for
        // `Chip` found nothing and the lookup for `CheckListBox` was ambiguous
        // between this entry and the real list box. See the alias table in
        // `factory_name_for_kind` for the other half of that fix.
        kind: WidgetKind::Chip,
        canonical_name: "chip",
        aliases: &["chips"],
        properties: CHIP_PROPERTIES,
        events: events_of!("chip"),
        commands: &["set_items", "toggle_index", "move_focus"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn grid_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Grid,
        canonical_name: "grid",
        aliases: &["grid_widget", "gridwidget"],
        properties: GRID_PROPERTIES,
        events: events_of!("grid"),
        // `select_cell` and `clear_selection` were advertised but no such method
        // exists on `Grid`, so they named actions nobody could obtain. Removed
        // rather than left as a promise with no implementation (principle #18);
        // the cell-click path is covered by the `cell_clicked` event.
        commands: &["set_rows", "set_columns", "set_spacing", "set_line_color"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn freeform_shape_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::FreeformShape,
        canonical_name: "freeform_shape",
        commands: &["set_fill_rgba", "set_stroke_rgba", "set_stroke_width"],
        aliases: &[],
        properties: FREEFORM_SHAPE_PROPERTIES,
        events: events_of!("freeform_shape"),
    }
}

// ── Dialog widget capabilities ────────────────────────────────
#[cfg(not(alloc_frugal))]
pub(crate) fn message_box_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::MessageBox,
        canonical_name: "message_box",
        aliases: &["msgbox"],
        properties: MESSAGE_BOX_PROPERTIES,
        events: events_of!("message_box"),
        // `set_icon` is the severity of the prompt (`warning`, `critical`, …). It is
        // a real, drawn property of the control and was reachable from the JSON
        // loader but not from this command list, so a generic consumer could not
        // discover it.
        commands: &["set_text", "set_title", "set_icon"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn file_dialog_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::FileDialog,
        canonical_name: "file_dialog",
        aliases: &[],
        properties: FILE_DIALOG_PROPERTIES,
        events: events_of!("file_dialog"),
        commands: &["set_mode", "set_directory", "open"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn font_dialog_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::FontDialog,
        canonical_name: "font_dialog",
        aliases: &[],
        properties: FONT_DIALOG_PROPERTIES,
        events: events_of!("font_dialog"),
        commands: &["set_current_font", "accept", "reject"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn input_dialog_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::InputDialog,
        canonical_name: "input_dialog",
        aliases: &[],
        properties: INPUT_DIALOG_PROPERTIES,
        events: events_of!("input_dialog"),
        commands: &["set_mode", "set_text_value", "set_int_value", "set_double_value"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn progress_dialog_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ProgressDialog,
        canonical_name: "progress_dialog",
        aliases: &[],
        properties: PROGRESS_DIALOG_PROPERTIES,
        events: events_of!("progress_dialog"),
        commands: &["set_value", "set_range", "set_title", "set_label_text"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn popup_window_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::PopupWindow,
        canonical_name: "popup_window",
        aliases: &["popup"],
        properties: POPUP_WINDOW_PROPERTIES,
        events: events_of!("popup_window"),
        commands: &["set_content_widget"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn dialog_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Dialog,
        canonical_name: "dialog",
        aliases: &[],
        properties: DIALOG_PROPERTIES,
        events: events_of!("dialog"),
        commands: &["accept", "reject"],
    }
}

// ── Container widget capabilities ─────────────────────────────
/// Scroll area capability.
///
/// Not registered in mini mode, but kept compiled so the shared
/// `SCROLL_AREA_PROPERTIES` table (defined in the ungated
/// `properties_container.in.rs`) stays reachable.
#[cfg_attr(alloc_frugal, allow(dead_code))]
pub(crate) fn scroll_area_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ScrollArea,
        canonical_name: "scroll_area",
        aliases: &[],
        properties: SCROLL_AREA_PROPERTIES,
        events: events_of!("scroll_area"),
        commands: &[
            "set_widget_resizable",
            "set_horizontal_scroll_bar_policy",
            "set_vertical_scroll_bar_policy",
        ],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn tab_widget_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::TabWidget,
        canonical_name: "tab_widget",
        aliases: &[],
        properties: TAB_WIDGET_PROPERTIES,
        events: events_of!("tab_widget"),
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

#[cfg(not(alloc_frugal))]
pub(crate) fn stacked_widget_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::StackedWidget,
        canonical_name: "stacked_widget",
        aliases: &["stacked"],
        properties: STACKED_WIDGET_PROPERTIES,
        events: events_of!("stacked_widget"),
        commands: &["add_widget", "remove_widget", "set_current_index"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn collapsible_pane_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::CollapsiblePane,
        canonical_name: "collapsible_pane",
        aliases: &["collapsible"],
        properties: COLLAPSIBLE_PANE_PROPERTIES,
        events: events_of!("collapsible_pane"),
        commands: &["set_title", "set_collapsed", "toggle"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn dock_widget_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::DockWidget,
        canonical_name: "dock_widget",
        aliases: &["dock"],
        properties: DOCK_WIDGET_PROPERTIES,
        events: events_of!("dock_widget"),
        commands: &["set_title", "set_floating", "set_features", "set_allowed_areas"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn mdi_area_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::MdiArea,
        canonical_name: "mdi_area",
        aliases: &["mdi"],
        properties: MDI_AREA_PROPERTIES,
        events: events_of!("mdi_area"),
        commands: &["add_subwindow", "remove_subwindow", "set_view_mode", "activate_subwindow"],
    }
}

// ── Text/input widget capabilities ───────────────────────────
#[cfg(not(alloc_frugal))]
pub(crate) fn text_edit_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::TextEdit,
        canonical_name: "text_edit",
        aliases: &[],
        properties: TEXT_EDIT_PROPERTIES,
        events: events_of!("text_edit"),
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
/// The web view's capability.
///
/// # Why the canonical name is `web_engine_view`
///
/// `web_view` is one of the two things `WidgetKind::WebEngineView` is called, and it
/// was the canonical name until it became clear what that cost. Two capabilities
/// declare this kind — this one and `media_player` — and
/// [`WidgetFactory::capability_by_kind`] resolves a kind by matching the capability's
/// `canonical_name` against **the kind's own name**. With the canonical name spelled
/// `web_view`, that match failed for `WebEngineView`, the lookup fell through to
/// `indices[0]`, and `create_web_view(..)` — and the C ABI function behind it — built
/// a **`MediaPlayer`** while returning a valid id.
///
/// Naming the entry after the kind is the same fix as `table` / `panel` / `data_view`:
/// the kind's own name is the only key that cannot be reordered out from under the
/// lookup. `web_view` stays reachable as an alias, so `factory.create("web_view", ..)`
/// and `WebView` (the `pub type` for this same control) are unaffected.
#[cfg(not(alloc_frugal))]
pub(crate) fn web_view_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::WebEngineView,
        canonical_name: "web_engine_view",
        aliases: &["webview", "web_view"],
        properties: WEB_VIEW_PROPERTIES,
        events: events_of!("web_engine_view"),
        commands: &["set_url", "load_url", "reload", "go_back", "go_forward", "stop"],
    }
}

// ── Advanced widget capabilities ─────────────────────────────
#[cfg(not(alloc_frugal))]
pub(crate) fn pie_menu_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::PieMenu,
        canonical_name: "pie_menu",
        aliases: &["radial_menu"],
        properties: PIE_MENU_PROPERTIES,
        events: events_of!("pie_menu"),
        commands: &["add_item", "remove_item", "set_radius", "set_current_index"],
    }
}

// ── Always-available widget property arrays ────────────────────────
// These are NOT gated behind `

// ── Always-available widget capability functions ──────────────────
#[cfg(not(alloc_frugal))]
pub(crate) fn toggle_button_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ToggleButton,
        canonical_name: "toggle_button",
        aliases: &["toggle"],
        properties: TOGGLE_BUTTON_PROPERTIES,
        events: events_of!("toggle_button"),
        commands: &["set_checked", "toggle", "set_text"],
    }
}

pub(crate) fn arc_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Arc,
        canonical_name: "arc",
        aliases: &["arc_widget"],
        properties: ARC_PROPERTIES,
        events: events_of!("arc"),
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
        events: events_of!("roller"),
        commands: &["set_options", "set_selected_index", "set_visible_count"],
    }
}

pub(crate) fn dropdown_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Dropdown,
        canonical_name: "dropdown",
        aliases: &["dropdown_widget", "combo"],
        properties: DROPDOWN_PROPERTIES,
        events: events_of!("dropdown"),
        commands: &["set_items", "set_selected_index", "set_expanded", "toggle"],
    }
}

pub(crate) fn text_area_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::TextArea,
        canonical_name: "text_area",
        aliases: &["multiline_edit"],
        properties: TEXT_AREA_PROPERTIES,
        events: events_of!("text_area"),
        commands: &["set_text", "set_placeholder", "set_read_only", "insert", "delete_char"],
    }
}

pub(crate) fn keyboard_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Keyboard,
        canonical_name: "keyboard",
        aliases: &["keyboard_widget", "virtual_keyboard"],
        properties: KEYBOARD_PROPERTIES,
        events: events_of!("keyboard"),
        commands: &["set_layout", "set_lowercase", "toggle_shift"],
    }
}

pub(crate) fn switch_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Switch,
        canonical_name: "switch",
        // `cupertino_switch` is deliberately *not* an alias here. It names the
        // `WidgetKind::CupertinoSwitch` variant, which has its own capability
        // below; listing it as an alias of `switch` made the alias table answer
        // for the variant while the variant itself stayed unconstructible, so
        // `create_cupertino_switch(..)` produced id `0`.
        aliases: &["switch_widget", "toggle_switch"],
        properties: SWITCH_PROPERTIES,
        events: events_of!("switch"),
        commands: &["set_checked", "toggle"],
    }
}

/// `CupertinoSwitch` is the iOS-styled sibling of `Switch`.
///
/// # Why it has its own capability rather than being an alias
///
/// It reports `WidgetKind::CupertinoSwitch`, so the factory must register that
/// kind for `capability_by_kind(CupertinoSwitch)` to resolve — an alias row on
/// `switch` answers the *name* lookup but never the *kind* lookup, which is the
/// one `mount_widget_of_kind` uses. The property schema is shared with `Switch`
/// rather than duplicated, so the two cannot drift.
#[cfg(not(alloc_frugal))]
pub(crate) fn cupertino_switch_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::CupertinoSwitch,
        canonical_name: "cupertino_switch",
        aliases: &["ios_switch"],
        properties: SWITCH_PROPERTIES,
        events: events_of!("cupertino_switch"),
        commands: &["set_checked", "toggle"],
    }
}

pub(crate) fn line_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Line,
        canonical_name: "line",
        aliases: &["line_widget"],
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
        events: events_of!("meter"),
        commands: &["set_value", "set_range"],
    }
}

/// `RadarChart` — series plotted over shared dimension axes.
///
/// Not registered as a `chart_type` token: the data model differs (see the
/// module docs), so it is its own control rather than a style of `ChartWidget`.
#[cfg(not(alloc_frugal))]
pub(crate) fn radar_chart_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::RadarChart,
        canonical_name: "radar_chart",
        aliases: &["radar", "spider_chart"],
        properties: RADAR_CHART_PROPERTIES,
        events: events_of!("radar_chart"),
        commands: &["set_axes", "set_series", "add_series"],
    }
}

/// `Heatmap` — a matrix of values rendered as a grid of coloured cells.
///
/// Not registered as a `chart_type` token: the data model differs (two **categorical**
/// axes and one value per coordinate pair), so it is its own control rather than a style
/// of `ChartWidget`. See the module docs of `special_widgets::heatmap`.
#[cfg(not(alloc_frugal))]
pub(crate) fn heatmap_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Heatmap,
        canonical_name: "heatmap",
        // `heat_map` is deliberately *not* listed: `normalize_key` strips `_`, so both
        // spellings already reach this row, and an alias that normalises to its own
        // canonical name is a no-op (`capability_alias_hygiene_test`).
        aliases: &["heatmap_chart"],
        properties: HEATMAP_PROPERTIES,
        events: events_of!("heatmap"),
        commands: &["set_data", "set_cell", "clear"],
    }
}

/// `Mention` — completes `@`-mentions from a candidate list.
///
/// Registered separately from `auto_complete_edit`: the completion model differs
/// (token before the caret, many mentions per field), which is the plan's §三 A2
/// judgment.
#[cfg(not(alloc_frugal))]
pub(crate) fn mention_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Mention,
        canonical_name: "mention",
        aliases: &["mention_edit", "at_mention"],
        properties: MENTION_PROPERTIES,
        events: events_of!("mention"),
        commands: &["set_candidates", "set_trigger", "complete", "open_popup"],
    }
}

/// `EmojiPicker` — a shell for choosing a caller-supplied glyph.
///
/// Registered as a control with no glyph data: the table arrives through
/// `set_glyphs`, which is the whole point of the shell/data split.
#[cfg(not(alloc_frugal))]
pub(crate) fn emoji_picker_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::EmojiPicker,
        canonical_name: "emoji_picker",
        aliases: &["emoji"],
        properties: EMOJI_PICKER_PROPERTIES,
        events: events_of!("emoji_picker"),
        commands: &["set_glyphs", "set_categories", "set_search", "choose"],
    }
}

/// `QueryBuilder` — edits a recursive `FilterExpr` as condition rows.
///
/// Registered separately from `data_grid`: the model is shared (both produce a
/// `FilterExpr`) but the rendering is not, which is exactly the case the plan's
/// §四 B5 describes.
#[cfg(not(alloc_frugal))]
pub(crate) fn query_builder_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::QueryBuilder,
        canonical_name: "query_builder",
        aliases: &["filter_builder"],
        properties: QUERY_BUILDER_PROPERTIES,
        events: events_of!("query_builder"),
        commands: &["set_fields", "add_row", "remove_row", "toggle_conjunction"],
    }
}

/// `Cascader` — walks a path down a tree of options.
///
/// Not an alias of `dropdown`: the selection is a path of varying depth, not an
/// index (see the module docs).
#[cfg(not(alloc_frugal))]
pub(crate) fn cascader_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Cascader,
        canonical_name: "cascader",
        aliases: &["cascade", "multi_level_select"],
        properties: CASCADER_PROPERTIES,
        events: events_of!("cascader"),
        commands: &["set_options", "set_selected_path", "expand", "collapse"],
    }
}

/// `KanbanBoard` — columns of draggable cards.
///
/// The first control built on `event::dnd`: its own `DropTarget` impl is the
/// acceptance path a dragged card resolves through.
#[cfg(not(alloc_frugal))]
pub(crate) fn kanban_board_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::KanbanBoard,
        canonical_name: "kanban_board",
        aliases: &["kanban", "board"],
        properties: KANBAN_BOARD_PROPERTIES,
        events: events_of!("kanban_board"),
        commands: &["add_column", "add_card", "move_card", "cancel_drag"],
    }
}

pub(crate) fn mini_chart_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::MiniChart,
        canonical_name: "mini_chart",
        aliases: &["mini_chart_widget"],
        properties: MINI_CHART_PROPERTIES,
        events: &[],
        commands: &["set_chart_type", "set_data", "set_range"],
    }
}

pub(crate) fn image_view_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ImageView,
        canonical_name: "image_view",
        aliases: &["image_viewer"],
        properties: IMAGE_VIEW_PROPERTIES,
        events: &[],
        commands: &["set_image", "set_scaled"],
    }
}

pub(crate) fn mini_canvas_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::MiniCanvas,
        canonical_name: "mini_canvas",
        aliases: &[],
        properties: MINI_CANVAS_PROPERTIES,
        events: events_of!("mini_canvas"),
        commands: &["fill_rect", "draw_rect", "draw_line", "fill_circle", "clear"],
    }
}

pub(crate) fn date_time_edit_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::DateTimePicker,
        canonical_name: "date_time_edit",
        aliases: &["datetimepicker", "date_time_picker"],
        properties: DATE_TIME_EDIT_PROPERTIES,
        events: events_of!("date_time_edit"),
        commands: &["set_datetime", "set_display_format", "set_calendar_popup"],
    }
}

// ── Group A property arrays (non-mini) ─────────────────────────
#[cfg(not(alloc_frugal))]
pub(crate) fn canvas_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Canvas,
        canonical_name: "canvas",
        aliases: &["canvas_widget", "drawing_surface"],
        properties: CANVAS_PROPERTIES,
        events: events_of!("canvas"),
        commands: &["set_zoom", "set_center", "clear"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn chart_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Chart,
        canonical_name: "chart",
        aliases: &["chart_widget", "chart_surface"],
        properties: CHART_PROPERTIES,
        events: events_of!("chart"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn search_box_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::SearchBox,
        canonical_name: "search_box",
        aliases: &["search"],
        properties: SEARCH_BOX_PROPERTIES,
        events: events_of!("search_box"),
        commands: &["set_text", "set_placeholder", "clear"],
    }
}

#[cfg(not(alloc_frugal))]
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

#[cfg(not(alloc_frugal))]
pub(crate) fn skeleton_loader_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::SkeletonLoader,
        canonical_name: "skeleton_loader",
        aliases: &["skeleton", "shimmer"],
        properties: SKELETON_LOADER_PROPERTIES,
        events: &[],
        commands: &["set_active"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn fab_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::FAB,
        canonical_name: "fab",
        aliases: &["floating_action_button", "action_button"],
        properties: FAB_PROPERTIES,
        events: events_of!("fab"),
        commands: &["set_icon"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn bottom_sheet_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::BottomSheet,
        canonical_name: "bottom_sheet",
        aliases: &["sheet"],
        properties: BOTTOM_SHEET_PROPERTIES,
        events: events_of!("bottom_sheet"),
        commands: &["set_expanded", "set_peek_height"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn bottom_navigation_bar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::BottomNavigationBar,
        canonical_name: "bottom_navigation_bar",
        aliases: &["bottom_nav", "bottomnav"],
        properties: BOTTOM_NAVIGATION_BAR_PROPERTIES,
        events: events_of!("bottom_navigation_bar"),
        commands: &["set_selected_index"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn navigation_drawer_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::NavigationDrawer,
        canonical_name: "navigation_drawer",
        aliases: &["nav_drawer", "drawer"],
        properties: NAVIGATION_DRAWER_PROPERTIES,
        events: events_of!("navigation_drawer"),
        commands: &["set_open", "set_width"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn app_bar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::AppBar,
        canonical_name: "app_bar",
        aliases: &["top_app_bar"],
        properties: APP_BAR_PROPERTIES,
        // The widget emits both of these (`app_bar.rs`); advertising an empty
        // list made a by-name subscriber unable to discover them.
        events: events_of!("app_bar"),
        commands: &["set_title"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn mobile_date_picker_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::MobileDatePicker,
        canonical_name: "mobile_date_picker",
        aliases: &[],
        properties: MOBILE_DATE_PICKER_PROPERTIES,
        events: events_of!("mobile_date_picker"),
        commands: &["set_selected_date"],
    }
}

#[cfg(not(alloc_frugal))]
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

#[cfg(not(alloc_frugal))]
pub(crate) fn stepper_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Stepper,
        canonical_name: "stepper",
        aliases: &["stepper_widget", "step_control"],
        properties: STEPPER_PROPERTIES,
        events: events_of!("stepper"),
        commands: &["set_value", "set_minimum", "set_maximum", "set_step"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn rating_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Rating,
        canonical_name: "rating",
        aliases: &["rating_widget", "star_rating"],
        properties: RATING_PROPERTIES,
        events: events_of!("rating"),
        commands: &["set_value", "set_max"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn avatar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Avatar,
        canonical_name: "avatar",
        aliases: &["avatar_widget", "user_avatar"],
        properties: AVATAR_PROPERTIES,
        events: &[],
        commands: &["set_initials", "set_image_source"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn empty_state_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::EmptyState,
        canonical_name: "empty_state",
        aliases: &["empty_placeholder"],
        properties: EMPTY_STATE_PROPERTIES,
        events: events_of!("empty_state"),
        commands: &["set_message", "set_description"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn color_history_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ColorHistory,
        canonical_name: "color_history",
        aliases: &[],
        properties: COLOR_HISTORY_PROPERTIES,
        events: events_of!("color_history"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn color_well_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ColorWell,
        canonical_name: "color_well",
        aliases: &[],
        properties: COLOR_WELL_PROPERTIES,
        events: events_of!("color_well"),
        commands: &["set_color"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn tag_input_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::TagInput,
        canonical_name: "tag_input",
        aliases: &["tag_editor", "chip_input"],
        properties: TAG_INPUT_PROPERTIES,
        events: events_of!("tag_input"),
        commands: &["add_tag", "remove_tag", "set_placeholder"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn ime_preedit_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ImePreedit,
        canonical_name: "ime_preedit",
        aliases: &["preedit"],
        properties: IME_PREEDIT_PROPERTIES,
        events: &[],
        commands: &["set_text", "set_cursor_position"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn inplace_editor_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::InplaceEditor,
        canonical_name: "inplace_editor",
        aliases: &["inline_editor"],
        properties: INPLACE_EDITOR_PROPERTIES,
        events: events_of!("inplace_editor"),
        commands: &["set_text", "start_editing", "finish_editing"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn qr_code_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::QRCode,
        canonical_name: "qr_code",
        aliases: &["qr"],
        properties: QR_CODE_PROPERTIES,
        events: &[],
        commands: &["set_data", "set_size"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn masonry_layout_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::MasonryLayout,
        canonical_name: "masonry_layout",
        aliases: &["waterfall_layout"],
        properties: MASONRY_LAYOUT_PROPERTIES,
        events: &[],
        commands: &["set_column_count"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn material_snackbar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::MaterialSnackbar,
        canonical_name: "material_snackbar",
        aliases: &[],
        properties: MATERIAL_SNACKBAR_PROPERTIES,
        events: events_of!("material_snackbar"),
        commands: &["show", "dismiss", "set_message", "set_action_text"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn adaptive_scaffold_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::AdaptiveScaffold,
        canonical_name: "adaptive_scaffold",
        aliases: &["scaffold"],
        properties: ADAPTIVE_SCAFFOLD_PROPERTIES,
        events: events_of!("adaptive_scaffold"),
        commands: &["set_title"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn wizard_dialog_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::WizardDialog,
        canonical_name: "wizard_dialog",
        aliases: &["wizard"],
        properties: WIZARD_DIALOG_PROPERTIES,
        events: events_of!("wizard_dialog"),
        // `set_current_step` was published against a read-only property, so its
        // `OutOfRange` answer pointed the caller at a write the schema refuses.
        // Navigation is fully covered by `next` / `back` / `finish`, which are the
        // commands this control actually implements.
        commands: &["next", "back", "finish"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn safe_area_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::SafeArea,
        canonical_name: "safe_area",
        aliases: &["safe_area_insets"],
        properties: SAFE_AREA_PROPERTIES,
        events: &[],
        commands: &["set_top_inset", "set_bottom_inset", "set_left_inset", "set_right_inset"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn cupertino_alert_dialog_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::CupertinoAlertDialog,
        canonical_name: "cupertino_alert_dialog",
        aliases: &["ios_alert"],
        properties: CUPERTINO_ALERT_DIALOG_PROPERTIES,
        events: events_of!("cupertino_alert_dialog"),
        commands: &["set_title", "set_message"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn cupertino_slider_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::CupertinoSlider,
        canonical_name: "cupertino_slider",
        aliases: &["ios_slider"],
        properties: CUPERTINO_SLIDER_PROPERTIES,
        events: events_of!("cupertino_slider"),
        commands: &["set_value", "set_min", "set_max"],
    }
}

#[cfg(not(alloc_frugal))]
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

#[cfg(not(alloc_frugal))]
pub(crate) fn segmented_button_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::SegmentedButton,
        canonical_name: "segmented_button",
        aliases: &["segmented_btn"],
        properties: SEGMENTED_BUTTON_PROPERTIES,
        events: events_of!("segmented_button"),
        commands: &["set_selected_index"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn navigation_stack_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::NavigationStack,
        canonical_name: "navigation_stack",
        aliases: &["nav_stack"],
        properties: NAVIGATION_STACK_PROPERTIES,
        // `navigation_changed` carries a `NavigationEvent`; `page_changed` was
        // advertised but never emitted by this widget.
        events: events_of!("navigation_stack"),
        commands: &["push", "pop", "set_current_page"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn progress_circle_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ProgressCircle,
        canonical_name: "progress_circle",
        aliases: &["circular_progress"],
        properties: PROGRESS_CIRCLE_PROPERTIES,
        events: &[],
        commands: &["set_value", "set_thickness", "set_indeterminate"],
    }
}

#[cfg(not(alloc_frugal))]
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

#[cfg(not(alloc_frugal))]
pub(crate) fn dropdown_menu_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::DropdownMenu,
        canonical_name: "dropdown_menu",
        aliases: &[],
        properties: DROPDOWN_MENU_PROPERTIES,
        // The widget's signal is `item_selected`; `selected_changed` was never
        // an emitted name, so a subscriber wired by it never fired.
        events: events_of!("dropdown_menu"),
        commands: &["set_selected_index", "set_expanded"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn masked_edit_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::MaskedEdit,
        canonical_name: "masked_edit",
        aliases: &["masked_input"],
        properties: MASKED_EDIT_PROPERTIES,
        events: events_of!("masked_edit"),
        commands: &["set_text", "set_mask"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn menu_button_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::MenuButton,
        canonical_name: "menu_button",
        aliases: &["dropdown_button"],
        properties: MENU_BUTTON_PROPERTIES,
        // `item_triggered` is the real signal; `selected_changed` is not emitted.
        events: events_of!("menu_button"),
        commands: &["set_text", "set_expanded"],
    }
}

#[cfg(not(alloc_frugal))]
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

#[cfg(not(alloc_frugal))]
pub(crate) fn auto_complete_edit_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::AutoCompleteEdit,
        canonical_name: "auto_complete_edit",
        aliases: &["autocomplete"],
        properties: AUTO_COMPLETE_EDIT_PROPERTIES,
        // The names must match the signals the control emits: `text_changed` (emitted by
        // `set_text` and the edit path) and `suggestion_selected`. The previous spellings
        // (`changed`, `selected`) were accepted by `connect_event`, which validates against this
        // list and then subscribes on the hub — so a consumer got a live-looking subscription
        // that no code path could ever invoke.
        events: events_of!("auto_complete_edit"),
        commands: &["set_text"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn multi_select_combo_box_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::MultiSelectComboBox,
        canonical_name: "multi_select_combo_box",
        aliases: &["multi_combo"],
        properties: MULTI_SELECT_COMBO_BOX_PROPERTIES,
        events: events_of!("multi_select_combo_box"),
        commands: &["set_expanded"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn range_slider_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::RangeSlider,
        canonical_name: "range_slider",
        aliases: &["dual_slider"],
        properties: RANGE_SLIDER_PROPERTIES,
        events: events_of!("range_slider"),
        commands: &["set_lower", "set_upper", "set_range"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn floating_label_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::FloatingLabel,
        canonical_name: "floating_label",
        aliases: &["floating_input"],
        properties: FLOATING_LABEL_PROPERTIES,
        events: events_of!("floating_label"),
        commands: &["set_text", "set_placeholder", "set_focused"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn font_preview_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::FontPreview,
        canonical_name: "font_preview",
        aliases: &["font_viewer"],
        properties: FONT_PREVIEW_PROPERTIES,
        events: &[],
        commands: &["set_font_family", "set_font_size", "set_preview_text"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn cupertino_navigation_bar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::CupertinoNavigationBar,
        canonical_name: "cupertino_navigation_bar",
        aliases: &["cupertinonavbar", "ios_nav_bar"],
        properties: CUPERTINO_NAVIGATION_BAR_PROPERTIES,
        events: events_of!("cupertino_navigation_bar"),
        commands: &["set_title", "set_large_title"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn cupertino_segmented_control_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::CupertinoSegmentedControl,
        canonical_name: "cupertino_segmented_control",
        aliases: &["cupertinosegmented", "ios_segmented"],
        properties: CUPERTINO_SEGMENTED_CONTROL_PROPERTIES,
        events: events_of!("cupertino_segmented_control"),
        commands: &["set_selected_index"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn refresh_control_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::RefreshControl,
        canonical_name: "refresh_control",
        aliases: &["pull_to_refresh"],
        properties: REFRESH_CONTROL_PROPERTIES,
        events: events_of!("refresh_control"),
        commands: &["set_refreshing"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn modal_bottom_sheet_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ModalBottomSheet,
        canonical_name: "modal_bottom_sheet",
        aliases: &["modal_sheet"],
        properties: MODAL_BOTTOM_SHEET_PROPERTIES,
        events: events_of!("modal_bottom_sheet"),
        commands: &["set_visible", "show", "dismiss"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn find_replace_dialog_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::FindReplaceDialog,
        canonical_name: "find_replace_dialog",
        aliases: &["find_replace"],
        properties: FIND_REPLACE_DIALOG_PROPERTIES,
        events: events_of!("find_replace_dialog"),
        commands: &["set_find_text", "set_replace_text", "set_match_case", "set_wrap_around"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn properties_panel_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::PropertiesPanel,
        canonical_name: "properties_panel",
        aliases: &["property_panel"],
        properties: PROPERTIES_PANEL_PROPERTIES,
        events: events_of!("properties_panel"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn cupertino_date_picker_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::CupertinoDatePicker,
        canonical_name: "cupertino_date_picker",
        aliases: &["ios_date_picker"],
        properties: CUPERTINO_DATE_PICKER_PROPERTIES,
        events: events_of!("cupertino_date_picker"),
        commands: &["set_selected_date"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn editable_combo_box_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::EditableComboBox,
        canonical_name: "editable_combo_box",
        aliases: &["editable_combo"],
        properties: EDITABLE_COMBO_BOX_PROPERTIES,
        events: events_of!("editable_combo_box"),
        commands: &["set_text"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn date_range_picker_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::DateRangePicker,
        canonical_name: "date_range_picker",
        aliases: &["range_picker"],
        properties: DATE_RANGE_PICKER_PROPERTIES,
        events: events_of!("date_range_picker"),
        commands: &["set_start_date", "set_end_date"],
    }
}

// ── New widget properties (non-mini) ────────────────────────────
// ── New widget capability functions (non-mini) ──────────────────
#[cfg(not(alloc_frugal))]
pub(crate) fn rich_edit_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::RichEdit,
        canonical_name: "rich_edit",
        aliases: &["rich_text_editor"],
        properties: RICH_EDIT_PROPERTIES,
        events: events_of!("rich_edit"),
        commands: &["set_text"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn carousel_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Carousel,
        canonical_name: "carousel",
        aliases: &["carousel_widget", "swipe_view"],
        properties: CAROUSEL_PROPERTIES,
        events: events_of!("carousel"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn material_navigation_rail_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::MaterialNavigationRail,
        canonical_name: "material_navigation_rail",
        aliases: &["nav_rail", "navigation_rail"],
        properties: MATERIAL_NAVIGATION_RAIL_PROPERTIES,
        events: events_of!("material_navigation_rail"),
        commands: &["set_selected_index"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn tab_view_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::TabView,
        canonical_name: "tab_view",
        aliases: &["page_tab_view"],
        properties: TAB_VIEW_PROPERTIES,
        events: events_of!("tab_view"),
        commands: &["set_selected_index"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn search_bar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::SearchBar,
        canonical_name: "search_bar",
        aliases: &["search_field"],
        properties: SEARCH_BAR_PROPERTIES,
        events: events_of!("search_bar"),
        commands: &["set_text", "set_placeholder"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn shortcut_editor_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ShortcutEditor,
        canonical_name: "shortcut_editor",
        aliases: &["keyboard_shortcut_editor"],
        properties: SHORTCUT_EDITOR_PROPERTIES,
        events: events_of!("shortcut_editor"),
        commands: &["set_filter_text"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn swipe_to_dismiss_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::SwipeToDismiss,
        canonical_name: "swipe_to_dismiss",
        aliases: &["swipe_dismiss"],
        properties: SWIPE_TO_DISMISS_PROPERTIES,
        events: events_of!("swipe_to_dismiss"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn line_chart_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::LineChart,
        canonical_name: "line_chart",
        aliases: &["line_graph"],
        properties: LINE_CHART_PROPERTIES,
        events: &[],
        commands: &["set_stroke_width"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn sparkline_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Sparkline,
        canonical_name: "sparkline",
        aliases: &["sparkline_chart"],
        properties: SPARKLINE_PROPERTIES,
        events: &[],
        commands: &["set_stroke_width"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn bar_chart_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::BarChart,
        canonical_name: "bar_chart",
        aliases: &["bar_graph"],
        properties: BAR_CHART_PROPERTIES,
        events: &[],
        commands: &["set_bar_spacing"],
    }
}

#[cfg(not(alloc_frugal))]
#[cfg(not(alloc_frugal))]
pub(crate) fn candlestick_chart_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::CandlestickChart,
        canonical_name: "candlestick_chart",
        aliases: &["candlestick", "kline", "k_line", "k_line_chart", "ohlc_chart"],
        properties: CANDLESTICK_CHART_PROPERTIES,
        events: events_of!("candlestick_chart"),
        commands: &["set_series", "add_overlay"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn volume_chart_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::VolumeChart,
        canonical_name: "volume_chart",
        aliases: &["volume", "volume_histogram", "volume_bars"],
        properties: VOLUME_CHART_PROPERTIES,
        events: events_of!("volume_chart"),
        // `add_overlay` was advertised but only `CandlestickChart` implements
        // it; the other finance views have no overlay API, so naming it here
        // promised an action that could not be obtained (principle #18).
        commands: &["set_series"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn depth_chart_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::DepthChart,
        canonical_name: "depth_chart",
        aliases: &["market_depth", "depth_graph", "liquidity_chart"],
        properties: DEPTH_CHART_PROPERTIES,
        events: events_of!("depth_chart"),
        // `add_overlay` was advertised but only `CandlestickChart` implements
        // it; the other finance views have no overlay API, so naming it here
        // promised an action that could not be obtained (principle #18).
        commands: &["set_series"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn order_book_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::OrderBook,
        canonical_name: "order_book",
        aliases: &["book_ladder", "market_depth_ladder"],
        properties: ORDER_BOOK_PROPERTIES,
        events: events_of!("order_book"),
        // `add_overlay` was advertised but only `CandlestickChart` implements
        // it; the other finance views have no overlay API, so naming it here
        // promised an action that could not be obtained (principle #18).
        commands: &["set_series"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn quote_board_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::QuoteBoard,
        canonical_name: "quote_board",
        aliases: &["quotes", "watchlist", "quote_table", "market_watch"],
        properties: QUOTE_BOARD_PROPERTIES,
        events: events_of!("quote_board"),
        // `add_overlay` was advertised but only `CandlestickChart` implements
        // it; the other finance views have no overlay API, so naming it here
        // promised an action that could not be obtained (principle #18).
        commands: &["set_series"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn indicator_chart_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::IndicatorChart,
        canonical_name: "indicator_chart",
        aliases: &["indicator", "technical_indicator", "oscillator", "macd_chart"],
        properties: INDICATOR_CHART_PROPERTIES,
        events: events_of!("indicator_chart"),
        // `add_overlay` was advertised but only `CandlestickChart` implements
        // it; the other finance views have no overlay API, so naming it here
        // promised an action that could not be obtained (principle #18).
        commands: &["set_series"],
    }
}

pub(crate) fn pie_chart_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::PieChart,
        canonical_name: "pie_chart",
        aliases: &["pie_graph"],
        properties: PIE_CHART_PROPERTIES,
        events: &[],
        commands: &["set_donut"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn animated_image_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::AnimatedImage,
        canonical_name: "animated_image",
        aliases: &["anim_image"],
        properties: ANIMATED_IMAGE_PROPERTIES,
        // `animation_finished` and `frame_changed` are the signals the control emits; the
        // published name was `finished`, which nothing ever fired. See
        // `auto_complete_edit` for why a name that does not match its signal is a live-looking
        // subscription rather than a visible error.
        events: events_of!("animated_image"),
        commands: &["set_playing"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn hero_animation_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::HeroAnimation,
        canonical_name: "hero_animation",
        aliases: &["hero"],
        properties: HERO_ANIMATION_PROPERTIES,
        events: events_of!("hero_animation"),
        commands: &["set_animation_progress"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn bezier_curve_editor_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::BezierCurveEditor,
        canonical_name: "bezier_curve_editor",
        aliases: &["curve_editor"],
        properties: BEZIER_CURVE_EDITOR_PROPERTIES,
        events: events_of!("bezier_curve_editor"),
        commands: &["set_snap_to_grid"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn lottie_widget_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::LottieWidget,
        canonical_name: "lottie_widget",
        aliases: &["lottie"],
        properties: LOTTIE_WIDGET_PROPERTIES,
        // `animation_finished` is the emitted signal; the published name was `finished`.
        events: events_of!("lottie_widget"),
        commands: &["set_playing"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn rive_widget_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::RiveWidget,
        canonical_name: "rive_widget",
        aliases: &["rive"],
        properties: RIVE_WIDGET_PROPERTIES,
        // `animation_finished` is the emitted signal; the published name was `finished`.
        events: events_of!("rive_widget"),
        commands: &["set_is_playing"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn video_player_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::VideoPlayer,
        canonical_name: "video_player",
        aliases: &["video"],
        properties: VIDEO_PLAYER_PROPERTIES,
        // The player publishes the four signals it actually emits; `finished` matched none of
        // them.
        events: events_of!("video_player"),
        commands: &["set_is_playing", "set_volume"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn image_gallery_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ImageGallery,
        canonical_name: "image_gallery",
        aliases: &["gallery"],
        properties: IMAGE_GALLERY_PROPERTIES,
        events: events_of!("image_gallery"),
        commands: &["set_current_index"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn audio_visualizer_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::AudioVisualizer,
        canonical_name: "audio_visualizer",
        aliases: &["audio_viz"],
        properties: AUDIO_VISUALIZER_PROPERTIES,
        events: &[],
        commands: &["set_bar_count"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn camera_preview_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::CameraPreview,
        canonical_name: "camera_preview",
        aliases: &["camera"],
        properties: CAMERA_PREVIEW_PROPERTIES,
        events: &[],
        commands: &["set_is_active"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn barcode_scanner_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::BarcodeScanner,
        canonical_name: "barcode_scanner",
        aliases: &["scanner"],
        properties: BARCODE_SCANNER_PROPERTIES,
        events: events_of!("barcode_scanner"),
        commands: &["set_is_scanning"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn tool_button_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ToolButton,
        canonical_name: "tool_button",
        aliases: &["toolbar_button"],
        properties: TOOL_BUTTON_PROPERTIES,
        events: events_of!("tool_button"),
        commands: &["set_text", "set_checked"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn status_bar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::StatusBar,
        canonical_name: "status_bar",
        aliases: &["status"],
        properties: STATUS_BAR_PROPERTIES,
        events: events_of!("status_bar"),
        commands: &["set_message"],
    }
}

/// Property grid properties: property_count (read-only), selected_index (r/w).
#[cfg(not(alloc_frugal))]
pub(crate) fn property_grid_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::PropertyGrid,
        canonical_name: "property_grid",
        aliases: &["inspector"],
        properties: PROPERTY_GRID_PROPERTIES,
        events: events_of!("property_grid"),
        commands: &["add_property", "set_value", "clear", "set_selected_index"],
    }
}
