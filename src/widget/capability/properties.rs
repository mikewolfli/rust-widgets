// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use super::types::{Constraint, PropertySchema, PropertyValueKind, RangeSpec, WidgetCapability};
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
        events: events_of!("label"),
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
        // Audit 20261026: `set_range` stays unpublished — the range is the writable
        // `minimum`/`maximum` pair, so a whole-range write is reachable through the property route
        // (principle #18).
        commands: &["set_value", "set_slider_position"],
    }
}

pub(crate) fn progress_bar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ProgressBar,
        canonical_name: "progress_bar",
        aliases: &[],
        properties: PROGRESS_BAR_PROPERTIES,
        events: events_of!("progress_bar"),
        // Audit 20261026: `set_range` stays unpublished — the range is the writable
        // `minimum`/`maximum` pair, so a whole-range write is reachable through the property route
        // (principle #18).
        commands: &["set_value", "set_orientation"],
    }
}

pub(crate) fn scroll_bar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ScrollBar,
        canonical_name: "scroll_bar",
        aliases: &[],
        properties: SCROLL_BAR_PROPERTIES,
        events: events_of!("scroll_bar"),
        // Audit 20261026: `set_range` and `set_steps` stay unpublished — the range is the writable
        // `minimum`/`maximum` pair and the steps are the writable `single_step`/`page_step` pair,
        // so both writes are reachable through the property route (principle #18).
        commands: &["set_value", "set_orientation"],
    }
}

pub(crate) fn list_box_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ListBox,
        canonical_name: "list_box",
        aliases: &[],
        properties: LIST_BOX_PROPERTIES,
        events: events_of!("list_box"),
        // Audit 20261026: `add_item`, `remove_item` stay unpublished — each takes a payload the bare
        // command channel cannot carry. `set_items` is the published form of a whole-list write:
        // its value travels through the `items` StringList property (B26.03).
        commands: &["clear", "clear_selection", "set_selection_mode", "set_items"],
    }
}

pub(crate) fn spin_box_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::SpinBox,
        canonical_name: "spin_box",
        aliases: &[],
        properties: SPIN_BOX_PROPERTIES,
        events: events_of!("spin_box"),
        // Audit 20261026: `set_range` stays unpublished — the range is the writable
        // `minimum`/`maximum` pair, so a whole-range write is reachable through the property route
        // (principle #18).
        commands: &["set_value", "step_up", "step_down"],
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
        // Audit 20261026: `set_range` stays unpublished — the range is the writable
        // `minimum`/`maximum` pair, so a whole-range write is reachable through the property route
        // (principle #18).
        commands: &["set_value", "set_wrapping"],
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
        // Audit 20261026: `add_item`, `remove_item` stay unpublished — each item carries the
        // `ObjectId` of the page widget it shows, and no property value can carry a widget
        // reference; `item_count`/`current_index` report the list's shape instead (principle
        // #18).
        commands: &["set_current_index", "set_orientation"],
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
        // Audit 20261026: `add_tab`, `remove_tab` stay unpublished — each takes a payload the bare
        // command channel cannot carry. `set_tabs` is the published form of a whole-strip write:
        // its rows travel through the `tabs` records property (B26.03).
        commands: &["set_current_index", "set_closable", "set_movable", "set_tabs"],
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
        // Audit 20261026: `set_date_range` stays unpublished — the range is the writable
        // `minimum_date`/`maximum_date` pair, so a whole-range write is reachable through the
        // property route (principle #18).
        commands: &["set_selected_date", "set_first_day_of_week", "show_today"],
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
        // `time_picker` is no longer an alias here: it is its own canonical row over the same
        // control (see `time_picker_capability`), and a name may not be both an alias and a
        // canonical spelling — the alias index would answer it twice.
        aliases: &["time_input"],
        properties: TIME_EDIT_PROPERTIES,
        events: events_of!("time_edit"),
        commands: &[
            "set_time",
            "set_minimum_time",
            "set_maximum_time",
            "set_display_format",
            "set_clock_popup",
        ],
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

/// A `LineEdit` that echoes dots — the password-entry form of the same control.
///
/// Same kind and the same property table as `line_edit`: the constructor installs the echo
/// mode, which is the one fact a caller would otherwise have to remember to set after
/// construction (and could forget between two fields of a form).
pub(crate) fn password_edit_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::LineEdit,
        canonical_name: "password_edit",
        aliases: &["password_input", "password_line_edit"],
        properties: LINE_EDIT_PROPERTIES,
        events: events_of!("line_edit"),
        commands: &["clear", "select_all"],
    }
}

/// A `LineEdit` that accepts phone-number characters and reports `valid`.
///
/// The grammar is the field's own (see `InputValidator`), so typing, pasting and a
/// programmatic `text` write all pass through it and `valid` always describes the value the
/// field actually holds.
pub(crate) fn phone_input_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::LineEdit,
        canonical_name: "phone_input",
        aliases: &["phone_edit", "telephone_input"],
        properties: LINE_EDIT_PROPERTIES,
        events: events_of!("line_edit"),
        commands: &["clear", "select_all"],
    }
}

/// A `LineEdit` that enforces an e-mail shape and reports `valid`.
pub(crate) fn email_input_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::LineEdit,
        canonical_name: "email_input",
        aliases: &["email_edit", "email_field"],
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
        // The payload-free expand/collapse/toggle verbs act on the **focused** node, because a
        // command with no payload cannot name another row. The spellings are the ones the sibling
        // hierarchical control (`cascader`) already publishes, so a host sees one vocabulary for
        // "open the current branch" rather than one per control.
        commands: &["clear_selection", "clear_focused_node", "expand", "collapse", "toggle"],
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
        // Audit 20261026: `select_all` is payload-free and runs the control's real
        // `select_all`, so it is published here rather than left as a name no arm answered.
        // `set_data_source` binds an `Arc<dyn IncrementalTableDataSource>`, which no property value
        // can carry; `set_current_cell` / `move_current_cell` name a position and the published
        // `current_cell` is read-only, so all three stay unpublished.
        commands: &[
            "clear_data_source",
            "set_scroll_row",
            "set_scroll_column",
            "set_row_height",
            "set_column_width",
            "set_frozen_columns",
            "select_all",
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
        // Audit 20261026: `select_row` stays unpublished — the writable `selected_row` property is
        // the same assignment (principle #18). `set_model` binds an `Arc<dyn ...>` model, which no
        // property value can carry; `expand_row`/`collapse_row` name a row and the table publishes
        // no expandable-row property, so all three stay unpublished.
        commands: &["clear_model"],
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
        // Audit 20261026: `set_data_source` binds an `Arc<dyn ...>` source no property value can
        // carry, and `select_cell` names a position while the table publishes only scroll offsets
        // (no cell-selection property), so both stay unpublished (principle #18).
        commands: &[
            "clear_data_source",
            "set_scroll_row",
            "set_scroll_column",
            "set_row_height",
            "set_column_width",
            "set_overscan_rows",
            "set_overscan_columns",
            "fetch_visible_window",
            "clear_selection",
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
        // `context_menu` is no longer an alias here: it is its own canonical row over the same
        // control (see `context_menu_capability`), and a name may not be both.
        // `drop_down_menu` is gone for the same reason: it normalises to `dropdownmenu`,
        // which is the canonical name of the separate `dropdown_menu` control. Keeping it
        // made the name resolve by registration order rather than by meaning.
        aliases: &[],
        properties: MENU_PROPERTIES,
        events: events_of!("menu"),
        // Audit 20261026: `add_action` stays unpublished — it takes a label the bare command
        // channel cannot carry. `set_items` is the published form of a whole-list write: its rows
        // travel through the `items` records property (B26.03). `add_separator` stays published —
        // its arm runs the control's zero-argument builder.
        commands: &["clear", "add_separator", "set_items"],
    }
}

#[cfg(not(alloc_frugal))]
/// `context_menu` — the role spelling of [`menu`](Self::menu).
///
/// # Why this is a name, not a second control
///
/// `src/widget/mod.rs` already records the design decision with `pub type ContextMenu = Menu;`:
/// "Context menus are ordinary menus shown at the pointer; the difference is in how the caller
/// shows them, not in the type." A second type would duplicate the popup placement, the entry
/// model and the activation events for no behavioural gain (rules #54 and #101), so this is the
/// same row `table_widget` / `table` and `time_edit` / `time_picker` use: one control, two
/// canonical names, so a caller that thinks in roles can address it.
pub(crate) fn context_menu_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Menu,
        canonical_name: "context_menu",
        aliases: &["context_actions"],
        properties: MENU_PROPERTIES,
        events: events_of!("menu"),
        // Audit 20261026: `add_action` stays unpublished — it takes a label the bare command
        // channel cannot carry. `set_items` is the published form of a whole-list write: its rows
        // travel through the `items` records property (B26.03). `add_separator` stays published —
        // the arm is shared with `menu`.
        commands: &["clear", "add_separator", "set_items"],
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
        // Audit 20261026: `add_menu`, `remove_menu` stay unpublished — each takes a payload the
        // bare command channel cannot carry. `set_entries` is the published form of a whole-bar
        // write: its rows travel through the `entries` records property (B26.03).
        commands: &["clear", "set_entries"],
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
        // Audit 20261026: `add_action` stays unpublished — it takes an id and a label the bare
        // command channel cannot carry. `set_items` is the published form of a whole-strip write:
        // its rows travel through the `items` records property (B26.03). `add_separator` stays
        // published — its arm runs the control's zero-argument builder.
        commands: &["clear", "add_separator", "set_items"],
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
        // Audit 20261026: `add_tab` stays unpublished — it takes a title the bare command channel
        // cannot carry; `tabs` is the published property and `set_tabs` its command form
        // (B26.03). `add_group`, `add_item`, `add_large_item` stay unpublished: they address a
        // position inside a tab/group nesting a flat property cannot express.
        commands: &["clear", "set_tabs"],
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
        // Audit 20261026: `set_hex` and `apply_preset` stay unpublished — the writable `hex_rgba`
        // property is the colour write, and a preset is applied by writing that property to the
        // preset's value, so both are reachable through the property route (principle #18).
        commands: &[],
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
        // Audit 20261026: `set_hex` and `apply_preset` stay unpublished — the dialog publishes the
        // writable `hex_rgba` and `preset_index` properties, which are exactly the colour and the
        // preset a caller writes, so both names are reachable through the property route
        // (principle #18).
        commands: &[],
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
        // set_markers now carries its list through the `markers` records property, and the caret is
        // writable through `cursor_line` / `cursor_column` (B26.03). `append_line` has no property
        // route (the document is the writable `text`), and `set_cursor` names no `cursor` property
        // — its two components are the honest route.
        commands: &[
            "set_text",
            "set_markers",
            "set_cursor_line",
            "set_cursor_column",
            "undo",
            "redo",
            "copy",
            "cut",
            "paste",
            "select_all",
            "find",
            "replace",
            "find_next",
            "find_previous",
            "toggle_comment",
            "toggle_block_comment",
            "duplicate_line",
            "delete_line",
            "indent",
            "outdent",
            "sort_lines",
            "fold_all",
            "unfold_all",
            "complete",
            "close_find",
        ],
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
        // `set_viewport` stays unpublished — the viewport is the writable
        // `viewport_start`/`viewport_end` pair (principle #18). `set_tasks` names its payload
        // through the `tasks` records property (B26.03), whose `dependencies` field carries each
        // relationship kind as an `<id>:<fts|sts|ftf>` token, so a schedule write cannot lose a
        // link. `select_task` names a task while `selected_id` is read-only, so it stays
        // unpublished.
        commands: &["zoom", "set_tasks"],
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
        // Audit 20261026: `append_output` stays unpublished — output is an append-only stream and
        // the control publishes only its line count (`output_line_count`), with no property that
        // carries the buffer itself; the control's own `append_output` API is the write path
        // (principle #18).
        commands: &["submit"],
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
        // Audit 20261026: `show` stays unpublished — the snackbar publishes the writable `message`
        // and `visible` properties, and that pair is exactly what `show` assigns (principle #18).
        // `show_with_action` additionally takes an action label, and `action_label` is read-only,
        // so it has no complete property route and also stays unpublished.
        commands: &["dismiss"],
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
        // set_markers now carries its list through the `markers` records property (B26.03); the
        // bare command reports the value is what is missing.
        commands: &["set_center_x", "set_center_y", "set_zoom", "set_markers"],
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
        // Audit 20261026: `seek_to` stays unpublished — the position is the writable
        // `position_ms` property, so a seek is that property's write (principle #18).
        commands: &["set_source", "clear_source", "play", "pause", "set_volume"],
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
        // Audit 20261026: `push_segment` stays unpublished — the whole trail is the writable
        // `segments` records property and `set_segments` is its published command form, so an
        // appended segment is a whole-trail write (principle #18).
        commands: &["set_segments", "clear_segments"],
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
        // Audit 20261026: `set_items` is published now that the `items` records property is its
        // payload channel (B26.03). `set_viewport` stays unpublished: the name does not resolve to
        // a `viewport` property — the two bounds are the writable `viewport_start` /
        // `viewport_end` properties, which is the honest route. `select_index` names a position
        // with no property route.
        commands: &["zoom", "set_items"],
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
        // set_entries now carries its list through the `entries` records property (B26.03);
        // move_highlight takes a signed delta with no property route, so it stays unpublished.
        commands: &["set_query", "clear_query", "activate_highlighted", "set_entries"],
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
        // Audit 20261026: `push` and `set_read` stay unpublished — their payloads are an item and
        // an id+flag, neither of which the bare command channel can carry; the `items` records
        // property is that payload channel (B26.03), and `set_items` is its published command
        // form. `select_index` names a position with no property route.
        commands: &["clear", "mark_all_read", "activate_selected", "set_items"],
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
        // Audit 20261026: `set_texts` stays unpublished because its name resolves to no property —
        // the two writable properties `left_text` / `right_text` each write through `set_texts`
        // and recompute the diff, so the pair is the honest route (principle #18). `select_index`
        // names a line with no property route; `selected_index` is read-only.
        commands: &[],
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
        // Audit 20261026: `append_line` stays unpublished — the document is the writable `text`
        // property, so an appended line is a whole-document write; there is no separate line
        // buffer property (principle #18). `undo` stays published — its arm runs the control's
        // real undo stack.
        commands: &["set_text", "toggle_preview_mode", "set_preview_mode", "undo"],
    }
}

/// The markdown editor's formatting strip (SP-022).
///
/// A separate control rather than a mode of `tool_bar`: its action set is the markdown token
/// vocabulary, its cells execute through [`MarkdownToolbar::execute`], and its single published
/// event carries the activated token. It shares `WidgetKind::ToolBar` with `tool_bar` and
/// `mac_toolbar` because the theme role and the accessibility role are the same, so the concrete
/// type is what distinguishes the three.
///
/// [`MarkdownToolbar::execute`]: crate::widget::special_widgets::markdown_editor::MarkdownToolbar::execute
#[cfg(not(alloc_frugal))]
pub(crate) fn markdown_toolbar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ToolBar,
        canonical_name: "markdown_toolbar",
        aliases: &["md_toolbar"],
        properties: MARKDOWN_TOOLBAR_PROPERTIES,
        events: events_of!("markdown_toolbar"),
        // The action list is the writable `actions` StringList, so `set_actions` is a property
        // write. `execute` takes the token itself and has no property carrier, so it stays a
        // Rust-side entry point (principle #18).
        commands: &[],
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
        // Audit 20261026: `push` and `select_index` stay unpublished — a toast is a whole item the
        // stack owns and the stack publishes neither an item list nor a writable selection
        // (`toast_count` / `selected_id` are read-only); the control's own API is the write path
        // (principle #18).
        commands: &["clear", "activate_selected", "dismiss_selected"],
    }
}

// Found by the registration-fidelity gate rather than by hand: `GridTableWidget`
// is a complete virtualised table that was exported but never registered, so it
// joins the six above as a seventh wiring-only fix. It reports `WidgetKind::GridTable`,
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
        // Audit 20261026: `set_actions` is published now that the `actions` StringList property is
        // its payload channel (B26.03). `show` stays published — it is payload-free and runs the
        // control's real `show`.
        commands: &["dismiss", "show", "activate_action", "set_actions"],
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
        // `set_logo` carries its descriptor through the writable `logo` property, so the command
        // form is the payload-free half (B26.03).
        commands: &["finish", "set_progress", "set_title", "set_logo"],
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
        // Audit 20261026: `set_range` stays unpublished — the range is the writable
        // `minimum`/`maximum` pair, so a whole-range write is reachable through the property route
        // (principle #18).
        commands: &["scroll_rows", "set_value", "set_step"],
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
        // Audit 20261026: `set_data_source` binds an `Arc<dyn IncrementalTableDataSource>` no
        // property value can carry; `toggle_sort_column` names a column and the table's sort state
        // is published as read-only derived names, so both stay unpublished (principle #18).
        commands: &["clear_data_source", "set_scroll_row", "set_scroll_column", "clear_selection"],
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
        // Audit 20261026: `add_action` stays unpublished — it takes an action the bare command
        // channel cannot carry. `set_actions` is the published form of a whole-list write: its
        // rows travel through the `actions` records property (B26.03). `trigger_primary` stays
        // published — its arm runs the control's real primary trigger.
        commands: &["open_menu", "close_menu", "trigger_primary", "set_actions"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn segmented_control_capability() -> WidgetCapability {
    WidgetCapability {
        // Its own kind. This row used to declare `WidgetKind::ToggleButton`, which left
        // `capability_by_kind(ToggleButton)` ambiguous between two controls and made the
        // accessibility role of a segmented bar answer "button".
        kind: WidgetKind::SegmentedControl,
        canonical_name: "segmented_control",
        aliases: &[],
        properties: SEGMENTED_CONTROL_PROPERTIES,
        events: events_of!("segmented_control"),
        // set_items carries its payload through the `items` records property (B26.03): the bare
        // command reports that a value is what is missing, and write_property("items", ...) is
        // where the list actually goes. move_selection takes a signed delta and has no property
        // route, so it stays unpublished.
        commands: &["set_items"],
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
        // Audit 20261026: `toggle_index` and `move_focus` stay unpublished — each takes a payload
        // (an index, a signed delta) the bare command channel cannot carry; a selection toggle is
        // expressed by writing the whole `items` list with `selected` flipped, and the keyboard
        // focus by writing `focused_index`, whose `set_focused_index` command form is published.
        commands: &["set_items", "set_focused_index"],
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
        // Audit 20261026: `set_current_font` stays unpublished — the selected font is a `Font`
        // value and the dialog publishes no property that carries one (only the option bits); the
        // control's own `set_current_font` API is the write path (principle #18).
        commands: &["accept", "reject"],
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
        // Audit 20261026: `set_range` stays unpublished — the range is the writable
        // `minimum`/`maximum` pair, so a whole-range write is reachable through the property route
        // (principle #18).
        commands: &["set_value", "set_title", "set_label_text"],
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
        // Audit 20261026: `set_content_widget` stays unpublished — its payload is an `ObjectId` of
        // a mounted widget and no property value can carry a widget reference (principle #18).
        commands: &[],
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
        // Audit 20261026: `add_tab`, `remove_tab` stay unpublished — a tab carries the page widget
        // it shows, and no property value can carry a widget reference; `tab_count` and the
        // text/current-index properties report the strip instead (principle #18).
        commands: &[
            "set_current_index",
            "set_closable",
            "set_movable",
            "set_tab_position",
            "set_tab_shape",
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
        // Audit 20261026: `add_widget`, `remove_widget` stay unpublished — each names a mounted
        // widget (or the `ObjectId` of one), and no property value can carry a widget reference;
        // `current_index` is the writable position property (principle #18).
        commands: &["set_current_index"],
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
        // Audit 20261026: `set_features`, `set_allowed_areas` stay unpublished — each assigns a
        // bit-flag set (`DockFeatures` / `DockAreas`) and the control publishes no property that
        // carries one; the control's own setters are the write path (principle #18).
        commands: &["set_title", "set_floating"],
    }
}

/// The dock host's published events (C-001).
///
/// [`DockArea`](crate::widget::container_widgets::DockArea) is a layout owner rather than a
/// `Widget`: it holds child ids and arranges the docks registered with it, so it has no
/// `WidgetKind` of its own and cannot carry a capability row under its own kind. Its three
/// host-facing signals therefore travel on this row, which shares the dock family's kind and
/// property schema because the constructible member of that family is the dock widget.
///
/// The events are resolved by `DockArea::event_signal_dyn`, not by `DockWidget`: the area owns
/// `active_dock_changed`, `dock_removed` and `layout_changed`, and a host that owns the area
/// connects those names through [`WidgetFactory::connect_event`](crate::widget::WidgetFactory).
#[cfg(not(alloc_frugal))]
pub(crate) fn dock_area_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::DockWidget,
        canonical_name: "dock_area",
        aliases: &["dock_host"],
        properties: DOCK_WIDGET_PROPERTIES,
        events: events_of!("dock_area"),
        commands: &[],
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
        // Audit 20261026: `add_subwindow`, `remove_subwindow`, `activate_subwindow`
        // stay unpublished — a sub-window is a mounted widget (or its index) and the control
        // publishes `active_subwindow` as read-only; `set_view_mode` stays published — the
        // property route runs the control's real `set_view_mode` (principle #18).
        // writes the control's real view mode.
        commands: &["set_view_mode"],
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
        // Audit 20261026: `load_url` stays unpublished — the address is the writable `url`
        // property and `set_url` is its published command form, so loading a page is a property
        // write (principle #18).
        commands: &["set_url", "reload", "go_back", "go_forward", "stop"],
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
        // Audit 20261026: `add_item`, `remove_item` stay unpublished — each takes a payload the bare
        // command channel cannot carry, and a whole-list write covers the same ground. `set_items`
        // is the published form of that write: its value travels through the `items` records
        // property (B26.03).
        commands: &["set_radius", "set_current_index", "set_items"],
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
        // Audit 20261026: `set_range` stays unpublished — the range is the writable
        // `minimum`/`maximum` pair, so a whole-range write is reachable through the property route
        // (principle #18).
        commands: &["set_value"],
    }
}

pub(crate) fn spinner_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Spinner,
        canonical_name: "spinner",
        aliases: &["spinner_widget", "loading_spinner"],
        properties: SPINNER_PROPERTIES,
        events: events_of!("spinner"),
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
        // Audit 20261026: `set_items` is published again now that the `items` StringList property
        // is its payload channel (B26.03): the bare command reports that a value is missing, and
        // write_property("items", [...]) is where the list goes.
        commands: &["set_selected_index", "set_expanded", "toggle", "set_items"],
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
        // `gauge` is a separate canonical control (`gauge_capability`), so it cannot also
        // be a `meter` alias: normalized names must have exactly one owner.
        aliases: &["meter_widget"],
        properties: METER_PROPERTIES,
        events: events_of!("meter"),
        // Audit 20261026: `set_range` stays unpublished — the range is the writable
        // `minimum`/`maximum` pair, so a whole-range write is reachable through the property route
        // (principle #18).
        // command arm stays as a defensive refusal.
        commands: &["set_value"],
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
        // Audit 20261026: `set_axes`, `set_series`, `add_series` stay unpublished — a series carries
        // a numeric sample list and an axis carries a label list, and the chart publishes only the
        // writable `series_names` StringList plus derived counts; the sample matrix has no property
        // carrier (principle #18).
        commands: &[],
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
        // Audit 20261026: `set_data`, `set_cell` stay unpublished — the matrix is a nested numeric
        // grid with no property carrier (a `NumberList` is flat), and `set_cell` names a position
        // while the published cell properties are the scale and viewport, not a cell value
        // (principle #18).
        commands: &["clear"],
    }
}

/// `Mention` — completes `@`-mentions from a candidate list.
///
/// Registered separately from `auto_complete_edit`: the completion model differs
/// (token before the caret, many mentions per field), which is the plan's §3 A2
/// judgment.
#[cfg(not(alloc_frugal))]
pub(crate) fn mention_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Mention,
        canonical_name: "mention",
        aliases: &["mention_edit", "at_mention"],
        properties: MENTION_PROPERTIES,
        events: events_of!("mention"),
        // Audit 20261026: `set_candidates` is published now that the `candidates` records property
        // is its payload channel (B26.03). `set_trigger` was already published and the `trigger`
        // property is writable, so the earlier note that claimed it was withdrawn was wrong;
        // `complete` stays published — its arm completes the highlighted candidate and refuses
        // when nothing is highlighted.
        commands: &["set_trigger", "open_popup", "complete", "set_candidates"],
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
        // Audit 20261026: `set_glyphs` and `set_categories` are published again now that the
        // `glyphs` records and `categories` StringList properties are their payload channels
        // (B26.03). `choose` takes a glyph id and has no property route, so it stays unpublished.
        commands: &["set_search", "set_glyphs", "set_categories"],
    }
}

/// `QueryBuilder` — edits a recursive `FilterExpr` as condition rows.
///
/// Registered separately from `data_grid`: the model is shared (both produce a
/// `FilterExpr`) but the rendering is not, which is exactly the case the plan's
/// §4 B5 describes.
#[cfg(not(alloc_frugal))]
pub(crate) fn query_builder_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::QueryBuilder,
        canonical_name: "query_builder",
        aliases: &["filter_builder"],
        properties: QUERY_BUILDER_PROPERTIES,
        events: events_of!("query_builder"),
        // Audit 20261026: `set_fields` and `set_rows` are published now that the `fields` / `rows`
        // records properties are their payload channels (B26.03). `add_row` and `remove_row` stay
        // unpublished — their payloads are a field index and a row index, and a whole-list write
        // covers both. `toggle_conjunction` stays published — its arm runs the control's real
        // toggle.
        commands: &["toggle_conjunction", "set_fields", "set_rows"],
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
        // Audit 20261026: `set_selected_path` is published now that the `selected_path`
        // NumberList property is its payload channel (B26.03). `set_options` stays unpublished:
        // an option carries `children`, and a record field cannot carry a nested record list, so
        // the tree has no property route.
        commands: &["expand", "collapse", "set_selected_path"],
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
        // `cancel_drag` is payload-free; `set_columns` / `set_cards` name their payload through
        // the `columns` / `cards` records properties (B26.03).
        commands: &["cancel_drag", "set_columns", "set_cards"],
    }
}

pub(crate) fn mini_chart_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::MiniChart,
        canonical_name: "mini_chart",
        aliases: &["mini_chart_widget"],
        properties: MINI_CHART_PROPERTIES,
        events: events_of!("mini_chart"),
        // Audit 20261026: `set_data` is published now that the `data` records property is its
        // payload channel (B26.03). `set_range` stays unpublished: its name resolves to no
        // property — the two bounds travel through the writable `min_value` / `max_value` scalars.
        commands: &["set_chart_type", "set_data"],
    }
}

pub(crate) fn image_view_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ImageView,
        canonical_name: "image_view",
        aliases: &["image_viewer"],
        properties: IMAGE_VIEW_PROPERTIES,
        events: events_of!("image_view"),
        // Audit 20261026: `set_image` stays unpublished — its payload is a decoded image and the
        // control publishes only the view state (`scaled`, rotation, zoom, pan), with no property
        // that carries the image itself; the control's own `set_image` API is the write path
        // (principle #18).
        commands: &["set_scaled"],
    }
}

pub(crate) fn mini_canvas_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::MiniCanvas,
        canonical_name: "mini_canvas",
        aliases: &[],
        properties: MINI_CANVAS_PROPERTIES,
        events: events_of!("mini_canvas"),
        // Audit 20261026: `fill_rect`, `draw_rect`, `draw_line`, `fill_circle` stay unpublished —
        // each appends a drawing instruction (a rectangle, a segment, a circle) and the control
        // publishes no property that carries one; the control's own drawing API is the write path
        // (principle #18).
        commands: &["clear"],
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
        // Audit 20261026: `set_zoom`, `set_center` stay unpublished — this control is a
        // command-recording drawing surface that replays `RenderCommand`s, not a pan/zoom view;
        // its published state is the queued-command count, so neither name has a property carrier
        // here (principle #18).
        commands: &["clear"],
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
        events: events_of!("badge"),
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
        events: events_of!("avatar"),
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
        aliases: &["tag_editor"],
        properties: TAG_INPUT_PROPERTIES,
        events: events_of!("tag_input"),
        // Audit 20261026: `add_tag`, `remove_tag` stay unpublished — each takes a label or an
        // index the bare command channel cannot carry; the whole committed list is the writable
        // `items` records property, so the two are reachable as a whole-list write (principle
        // #18).
        commands: &["set_placeholder"],
    }
}

/// The chip input's capability (SP-121).
///
/// It used to be an alias of `tag_input`, which made four events published under `tag_input`
/// resolve against `TagInput` even though only `ChipInput` emits them — the capability table and
/// the control disagreed. A canonical row of its own lets each control own its schema and its
/// events, and the alias is removed rather than left to resolve to the wrong struct.
#[cfg(not(alloc_frugal))]
pub(crate) fn chip_input_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::TextEdit,
        canonical_name: "chip_input",
        aliases: &[],
        properties: CHIP_INPUT_PROPERTIES,
        events: events_of!("chip_input"),
        commands: &["commit", "clear"],
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
        events: events_of!("masonry_layout"),
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
        // `OutOfRange` answer pointed the caller at a write the schema refuses —
        // it is no longer published, since an index needs a payload the command
        // channel cannot carry. `finish` runs the same path the Finish button
        // does: every step is marked completed and `finished` is emitted.
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
        // Audit 20261026: `push` stays unpublished — a page is a mounted widget and no property
        // value can carry a widget reference; the control's own `push` API is the write path
        // (principle #18).
        // command arm stays as a defensive refusal.
        commands: &["pop", "set_current_page"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn progress_circle_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ProgressCircle,
        canonical_name: "progress_circle",
        aliases: &["circular_progress"],
        properties: PROGRESS_CIRCLE_PROPERTIES,
        events: events_of!("progress_circle"),
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
        events: events_of!("popover"),
        // Audit 20261026: `set_text` stays unpublished — the popover's `text` property is
        // deliberately read-only (it mirrors the installed content widget), and the documented
        // write path is the control's own `set_content` (principle #18).
        commands: &["set_visible"],
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
        // Audit 20261026: `set_range` stays unpublished — the range is the writable
        // `min_value`/`max_value` pair, so a whole-range write is reachable through the property
        // route (principle #18).
        commands: &["set_lower", "set_upper"],
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
        commands: &["next", "previous"],
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

/// The chord recorder (`KeySequenceEdit`), published separately from `ShortcutEditor`.
///
/// The two controls capture different things: `ShortcutEditor` edits a command id plus a list of
/// keys, while `KeySequenceEdit` records one chord. Its published names are the ones that control
/// emits, so a subscriber is not told about an event the other control would have to fake.
#[cfg(not(alloc_frugal))]
pub(crate) fn key_sequence_editor_capability() -> WidgetCapability {
    WidgetCapability {
        // `KeySequenceEdit`'s base widget reports the line-edit kind; the capability follows the
        // control it describes rather than inventing a second kind for a text-shaped editor.
        kind: WidgetKind::LineEdit,
        canonical_name: "key_sequence_editor",
        aliases: &["key_sequence_edit"],
        properties: KEY_SEQUENCE_EDITOR_PROPERTIES,
        events: events_of!("key_sequence_editor"),
        commands: &[],
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
        events: events_of!("line_chart"),
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
        events: events_of!("sparkline"),
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
        events: events_of!("bar_chart"),
        commands: &["set_bar_spacing"],
    }
}

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
        // Audit 20261026: `set_series` stays unpublished — a depth series is a nested
        // price/volume list with no property carrier; the published names are the display knobs
        // (`depth`, colours) the view does carry (principle #18).
        // Mutations go through the property route or the widget's own API; the
        // command arm stays as a defensive refusal.
        commands: &[],
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
        // Audit 20261026: `set_series` stays unpublished — an order book is a nested price/size
        // list with no property carrier; the published names are the display knobs the view does
        // carry (principle #18).
        // Mutations go through the property route or the widget's own API; the
        // command arm stays as a defensive refusal.
        commands: &[],
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
        // Audit 20261026: `set_series` stays unpublished — a quote board is a row list with no
        // property carrier; the published names are the sort and display controls it does carry
        // (principle #18).
        // Mutations go through the property route or the widget's own API; the
        // command arm stays as a defensive refusal.
        commands: &[],
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
        events: events_of!("pie_chart"),
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
        events: events_of!("audio_visualizer"),
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
        events: events_of!("camera_preview"),
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

/// The grid status strip's capability (COMP-A23).
///
/// It shares `WidgetKind::StatusBar` with the message bar, so it needs its own canonical name and
/// a concrete-type tie-break; its events are the strip's own `status_clicked`.
#[cfg(not(alloc_frugal))]
pub(crate) fn grid_status_bar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::StatusBar,
        canonical_name: "grid_status_bar",
        aliases: &["table_status_bar"],
        properties: GRID_STATUS_BAR_PROPERTIES,
        events: events_of!("grid_status_bar"),
        commands: &["set_text"],
    }
}

/// The group avatar's capability (MISC-010).
///
/// It shares `WidgetKind::Avatar` with the single `Avatar`; membership is a method call
/// (`push`/`clear`), so the schema is the base contract and no event is published.
#[cfg(not(alloc_frugal))]
pub(crate) fn group_avatar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Avatar,
        canonical_name: "group_avatar",
        aliases: &[],
        properties: GROUP_AVATAR_PROPERTIES,
        events: &[],
        commands: &[],
    }
}

/// Unified dropdown popup host (COMP-A21).
#[cfg(not(alloc_frugal))]
pub(crate) fn drop_down_popup_host_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Panel,
        canonical_name: "drop_down_popup_host",
        aliases: &[],
        properties: DROP_DOWN_POPUP_HOST_PROPERTIES,
        events: events_of!("drop_down_popup_host"),
        commands: &[],
    }
}

/// Command palette host (COMP-A30).
#[cfg(not(alloc_frugal))]
pub(crate) fn command_palette_host_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ListView,
        canonical_name: "command_palette_host",
        aliases: &[],
        properties: COMMAND_PALETTE_HOST_PROPERTIES,
        events: events_of!("command_palette_host"),
        commands: &[],
    }
}

/// Header menu (COMP-A32).
#[cfg(not(alloc_frugal))]
pub(crate) fn header_menu_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Menu,
        canonical_name: "header_menu",
        aliases: &[],
        properties: HEADER_MENU_PROPERTIES,
        events: events_of!("header_menu"),
        commands: &[],
    }
}

/// Chart reference line (COMP-A1).
///
/// It shares `WidgetKind::LineChart` with the chart it annotates, so it needs its own canonical
/// name and a concrete-type tie-break; the annotations are attached by method call, and the line
/// publishes no factory event of its own.
#[cfg(not(alloc_frugal))]
pub(crate) fn chart_reference_line_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::LineChart,
        canonical_name: "chart_reference_line",
        aliases: &[],
        properties: CHART_REFERENCE_LINE_PROPERTIES,
        events: events_of!("chart_reference_line"),
        commands: &[],
    }
}

/// Terminal scrollback find bar (COMP-A2).
///
/// It shares `WidgetKind::SearchBar` with the search field; the bar attaches to a `TerminalView`
/// and reports no factory event of its own.
#[cfg(not(alloc_frugal))]
pub(crate) fn terminal_find_bar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::SearchBar,
        canonical_name: "terminal_find_bar",
        aliases: &[],
        properties: TERMINAL_FIND_BAR_PROPERTIES,
        events: events_of!("terminal_find_bar"),
        commands: &[],
    }
}

/// Map attribution overlay (COMP-A3).
///
/// It shares `WidgetKind::Canvas` with the map's own scene canvas and with the overview below, so
/// each of the three needs its own concrete-type tie-break.
#[cfg(not(alloc_frugal))]
pub(crate) fn map_attribution_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Canvas,
        canonical_name: "map_attribution",
        aliases: &[],
        properties: MAP_ATTRIBUTION_PROPERTIES,
        events: events_of!("map_attribution"),
        commands: &[],
    }
}

/// Gantt task column (COMP-A4).
///
/// It shares `WidgetKind::Chart` with the chart controls; the column is the row-aligned task list
/// the gantt chart draws beside its bars.
#[cfg(not(alloc_frugal))]
pub(crate) fn gantt_task_grid_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Chart,
        canonical_name: "gantt_task_grid",
        aliases: &[],
        properties: GANTT_TASK_GRID_PROPERTIES,
        events: events_of!("gantt_task_grid"),
        commands: &[],
    }
}

/// Web history panel (COMP-A5).
///
/// It shares `WidgetKind::WebEngineView` with the web view it lists history for.
#[cfg(not(alloc_frugal))]
pub(crate) fn web_history_panel_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::WebEngineView,
        canonical_name: "web_history_panel",
        aliases: &[],
        properties: WEB_HISTORY_PANEL_PROPERTIES,
        events: events_of!("web_history_panel"),
        commands: &[],
    }
}

/// Media chapter list (COMP-A6).
///
/// It shares `WidgetKind::VideoPlayer` with the player whose chapters it lists.
#[cfg(not(alloc_frugal))]
pub(crate) fn chapter_list_panel_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::VideoPlayer,
        canonical_name: "chapter_list_panel",
        aliases: &[],
        properties: CHAPTER_LIST_PANEL_PROPERTIES,
        events: events_of!("chapter_list_panel"),
        commands: &[],
    }
}

/// Map overview (COMP-A7).
///
/// It shares `WidgetKind::Canvas` with the scene canvas and the attribution overlay.
#[cfg(not(alloc_frugal))]
pub(crate) fn map_overview_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Canvas,
        canonical_name: "map_overview",
        aliases: &[],
        properties: MAP_OVERVIEW_PROPERTIES,
        events: events_of!("map_overview"),
        commands: &[],
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
        // Audit 20261026: `set_properties` is published now that the `properties` records property
        // is its payload channel (B26.03). `add_property` and `set_value` stay unpublished — their
        // payloads (a row, an index+value) have no property route of their own; a whole-list write
        // covers both.
        commands: &["clear", "set_selected_index", "set_properties"],
    }
}

// ── Layout family ───────────────────────────────────────────────────────────
//
// The wrapper controls (`Spacer`, `Center`, `Align`, `SizedBox`, `Padding`, `Expanded`,
// `Flexible`, `IntrinsicWidth`, `AspectRatio`) and the distribution controls (`Flex`,
// `FlowLayout`, `WrapLayout`). Each owns a single- or multi-item model and positions it by the
// rule its name states; see `container_widgets::layout_family` and `container_widgets::
// linear_layouts` for why each is a control rather than a mode of another.

#[cfg(not(alloc_frugal))]
pub(crate) fn spacer_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Spacer,
        canonical_name: "spacer",
        aliases: &["flexible_space"],
        properties: SPACER_PROPERTIES,
        events: &[],
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn center_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Center,
        canonical_name: "center",
        aliases: &["centre", "center_widget"],
        properties: CENTER_PROPERTIES,
        events: &[],
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn align_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Align,
        canonical_name: "align",
        aliases: &["alignment_widget"],
        properties: ALIGN_PROPERTIES,
        events: &[],
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn sized_box_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::SizedBox,
        canonical_name: "sized_box",
        aliases: &["size_box"],
        properties: SIZED_BOX_PROPERTIES,
        events: &[],
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn padding_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Padding,
        canonical_name: "padding",
        aliases: &["padding_widget"],
        properties: PADDING_PROPERTIES,
        events: &[],
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn expanded_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Expanded,
        canonical_name: "expanded",
        aliases: &["expand"],
        properties: EXPANDED_PROPERTIES,
        events: &[],
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn flexible_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Flexible,
        canonical_name: "flexible",
        aliases: &["flex_child"],
        properties: FLEXIBLE_PROPERTIES,
        events: &[],
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn intrinsic_width_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::IntrinsicWidth,
        canonical_name: "intrinsic_width",
        aliases: &["shrink_wrap"],
        properties: INTRINSIC_WIDTH_PROPERTIES,
        events: &[],
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn aspect_ratio_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::AspectRatio,
        canonical_name: "aspect_ratio",
        aliases: &["ratio_box"],
        properties: ASPECT_RATIO_PROPERTIES,
        events: &[],
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn flex_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Flex,
        canonical_name: "flex",
        aliases: &["flex_box"],
        properties: FLEX_PROPERTIES,
        events: events_of!("flex"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn flow_layout_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::FlowLayout,
        canonical_name: "flow_layout",
        aliases: &["flow"],
        properties: FLOW_LAYOUT_PROPERTIES,
        events: events_of!("flow_layout"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn wrap_layout_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::WrapLayout,
        canonical_name: "wrap_layout",
        aliases: &["wrap"],
        properties: WRAP_LAYOUT_PROPERTIES,
        events: events_of!("wrap_layout"),
        commands: &[],
    }
}

/// `split_layout` — the two-pane spelling of [`splitter`](Self::splitter).
///
/// # Why a second name over the same control
///
/// A split *layout* is two panes and a draggable divider; a splitter is that same widget with
/// an arbitrary pane count. The concrete type, the properties and the events are therefore
/// identical, and the established pattern for "one control, two names" (`table_widget` /
/// `table`, `tool_box` / `toolbox`) applies. A separate type would duplicate the drag
/// machinery for no behavioural gain.
#[cfg(not(alloc_frugal))]
pub(crate) fn split_layout_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Splitter,
        canonical_name: "split_layout",
        aliases: &["split_view"],
        properties: SPLITTER_PROPERTIES,
        events: events_of!("splitter"),
        commands: &["set_orientation"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn accordion_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Accordion,
        canonical_name: "accordion",
        aliases: &["collapsible_list"],
        properties: ACCORDION_PROPERTIES,
        events: events_of!("accordion"),
        commands: &[],
    }
}

// ── Small display controls ──────────────────────────────────────────────────

#[cfg(not(alloc_frugal))]
pub(crate) fn tag_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Tag,
        canonical_name: "tag",
        aliases: &["label_tag"],
        properties: TAG_PROPERTIES,
        events: events_of!("tag"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn clock_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Clock,
        canonical_name: "clock",
        aliases: &["analog_clock"],
        properties: CLOCK_PROPERTIES,
        events: events_of!("clock"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn gauge_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Gauge,
        canonical_name: "gauge",
        aliases: &["dial_gauge"],
        properties: GAUGE_PROPERTIES,
        events: events_of!("gauge"),
        commands: &[],
    }
}

// ── Menus and dialogs ───────────────────────────────────────────────────────

#[cfg(not(alloc_frugal))]
pub(crate) fn alert_dialog_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::AlertDialog,
        canonical_name: "alert_dialog",
        aliases: &["notice_dialog"],
        properties: ALERT_DIALOG_PROPERTIES,
        events: events_of!("alert_dialog"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn confirm_dialog_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ConfirmDialog,
        canonical_name: "confirm_dialog",
        aliases: &["confirmation_dialog"],
        properties: CONFIRM_DIALOG_PROPERTIES,
        events: events_of!("confirm_dialog"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn about_dialog_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::AboutDialog,
        canonical_name: "about_dialog",
        aliases: &["about_box"],
        properties: ABOUT_DIALOG_PROPERTIES,
        events: events_of!("about_dialog"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `time_picker` — the kind's own spelling of [`time_edit`](Self::time_edit).
///
/// # Why a second name over the same control
///
/// `WidgetKind::TimePicker` is the variant the time field reports, and `time_edit` was already
/// published with `time_picker` as an alias. A canonical row makes the kind's own spelling a
/// first-class name — the same shape `table_widget` / `table` and `splitter` / `split_layout`
/// use — without introducing a second time field, which would give one concept two
/// implementations (rules #54 and #101).
pub(crate) fn time_picker_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::TimePicker,
        canonical_name: "time_picker",
        aliases: &["time_field"],
        properties: TIME_EDIT_PROPERTIES,
        events: events_of!("time_edit"),
        commands: &[
            "set_time",
            "set_minimum_time",
            "set_maximum_time",
            "set_display_format",
            "set_clock_popup",
        ],
    }
}

// ── macOS (desktop) component set ───────────────────────────────
//
// Five controls that carry the desktop platform's own shapes beside the touch-styled
// `cupertino` set. Each one declares the kind of the control it refines — a mac segmented
// picker is a segmented picker, a source list is a list view — because the kind is what the
// theme layer and the accessibility role classify by. The capability name is what the factory,
// the design manifest and the property contract address; the kind is shared on purpose and
// resolved by the concrete-type tie-break in `WidgetFactory::widget_matches_capability`.

#[cfg(not(alloc_frugal))]
/// `mac_segmented_control` — the desktop segmented picker.
pub(crate) fn mac_segmented_control_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::SegmentedControl,
        canonical_name: "mac_segmented_control",
        aliases: &[],
        properties: MAC_SEGMENTED_CONTROL_PROPERTIES,
        events: events_of!("mac_segmented_control"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `mac_stepper` — the desktop increment/decrement stepper.
pub(crate) fn mac_stepper_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Stepper,
        canonical_name: "mac_stepper",
        aliases: &[],
        properties: MAC_STEPPER_PROPERTIES,
        events: events_of!("mac_stepper"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `mac_toolbar` — the desktop toolbar strip.
pub(crate) fn mac_toolbar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ToolBar,
        canonical_name: "mac_toolbar",
        aliases: &[],
        properties: MAC_TOOLBAR_PROPERTIES,
        events: events_of!("mac_toolbar"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `mac_source_list` — the desktop sectioned sidebar.
pub(crate) fn mac_source_list_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ListView,
        canonical_name: "mac_source_list",
        aliases: &[],
        properties: MAC_SOURCE_LIST_PROPERTIES,
        events: events_of!("mac_source_list"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `mac_window_chrome` — the desktop title bar with traffic-light controls.
pub(crate) fn mac_window_chrome_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Window,
        canonical_name: "mac_window_chrome",
        aliases: &[],
        properties: MAC_WINDOW_CHROME_PROPERTIES,
        events: events_of!("mac_window_chrome"),
        commands: &[],
    }
}

// ── Companion capabilities (COMP-01/05/07/08/09/10/12/13/14/15/16/17/18/19/20) ──
//
// Each companion is its own control with its own kind, so its capability is a
// first-class row rather than a comment on the primary control it partners.

#[cfg(not(alloc_frugal))]
/// `swatch` — the shared colour-cell grid (`COMP-01`).
pub(crate) fn swatch_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Swatch,
        canonical_name: "swatch",
        aliases: &["color_swatch", "colour_swatch"],
        properties: SWATCH_PROPERTIES,
        events: events_of!("swatch"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `filter_row` — one column filter editor per column (`COMP-05`).
pub(crate) fn filter_row_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::FilterRow,
        canonical_name: "filter_row",
        aliases: &["filter_bar"],
        properties: FILTER_ROW_PROPERTIES,
        events: events_of!("filter_row"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `legend` — a checkable chart series key (`COMP-07`).
pub(crate) fn legend_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Legend,
        canonical_name: "legend",
        aliases: &["chart_legend"],
        properties: LEGEND_PROPERTIES,
        events: events_of!("legend"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `chart_tooltip` — the follow-pointer chart bubble (`COMP-08`).
pub(crate) fn chart_tooltip_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ChartTooltip,
        canonical_name: "chart_tooltip",
        aliases: &["tooltip_bubble"],
        properties: CHART_TOOLTIP_PROPERTIES,
        events: events_of!("chart_tooltip"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `tooltip_manager` — the host hover-to-tooltip pipeline (`COMP-09`).
pub(crate) fn tooltip_manager_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::TooltipManager,
        canonical_name: "tooltip_manager",
        aliases: &["tooltip_pipeline"],
        properties: TOOLTIP_MANAGER_PROPERTIES,
        events: events_of!("tooltip_manager"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `zoom_pan_controller` — zoom/pan/ROI state for an image view (`COMP-10`).
pub(crate) fn zoom_pan_controller_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ZoomPanController,
        canonical_name: "zoom_pan_controller",
        aliases: &["zoom_pan"],
        properties: ZOOM_PAN_CONTROLLER_PROPERTIES,
        events: events_of!("zoom_pan_controller"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `toast_host` — the bounded, expiring toast queue (`COMP-12`).
pub(crate) fn toast_host_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ToastHost,
        canonical_name: "toast_host",
        aliases: &["toast_queue"],
        properties: TOAST_HOST_PROPERTIES,
        events: events_of!("toast_host"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `tile_loader` — tile addresses and their cache (`COMP-13`).
pub(crate) fn tile_loader_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::TileLoader,
        canonical_name: "tile_loader",
        aliases: &["map_tile_loader"],
        properties: TILE_LOADER_PROPERTIES,
        events: events_of!("tile_loader"),
        commands: &["clear_cache"],
    }
}

#[cfg(not(alloc_frugal))]
/// `zoom_control` — the map's three-button zoom strip (`COMP-13`).
pub(crate) fn zoom_control_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ZoomControl,
        canonical_name: "zoom_control",
        aliases: &["map_zoom_control"],
        properties: ZOOM_CONTROL_PROPERTIES,
        events: events_of!("zoom_control"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `scale_bar` — the map's distance legend (`COMP-13`).
pub(crate) fn scale_bar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ScaleBar,
        canonical_name: "scale_bar",
        aliases: &["map_scale_bar"],
        properties: SCALE_BAR_PROPERTIES,
        events: &[],
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `compass` — the map's north-up dial (`COMP-13`).
pub(crate) fn compass_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Compass,
        canonical_name: "compass",
        aliases: &["map_compass"],
        properties: COMPASS_PROPERTIES,
        events: events_of!("compass"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `timeline_axis` — the Gantt date/value axis (`COMP-14`).
pub(crate) fn timeline_axis_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::TimelineAxis,
        canonical_name: "timeline_axis",
        aliases: &["gantt_timeline_axis"],
        properties: TIMELINE_AXIS_PROPERTIES,
        events: events_of!("timeline_axis"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `dependency_editor` — the Gantt dependency edge editor (`COMP-14`).
pub(crate) fn dependency_editor_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::DependencyEditor,
        canonical_name: "dependency_editor",
        aliases: &["gantt_dependency_editor"],
        properties: DEPENDENCY_EDITOR_PROPERTIES,
        events: events_of!("dependency_editor"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `diff_file_list` — the multi-file selector beside a diff (`COMP-15`).
pub(crate) fn diff_file_list_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::DiffFileList,
        canonical_name: "diff_file_list",
        aliases: &["diff_files"],
        properties: DIFF_FILE_LIST_PROPERTIES,
        events: events_of!("diff_file_list"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `markdown_outline_panel` — the markdown editor's heading outline (`COMP-16`).
pub(crate) fn markdown_outline_panel_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::MarkdownOutlinePanel,
        canonical_name: "markdown_outline_panel",
        aliases: &["markdown_outline"],
        properties: MARKDOWN_OUTLINE_PANEL_PROPERTIES,
        events: events_of!("markdown_outline_panel"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `find_bar` — the editor's find/replace strip (`COMP-16`).
///
/// `find_replace_bar` used to be an alias of this control. It is now the canonical name of the
/// companion `FindReplaceBar` capability (`COMP-22`), so the spelling has to name the control a
/// caller can actually mount; keeping the alias would have made one name resolve to two different
/// widgets depending on which row registered last.
pub(crate) fn find_bar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::FindBar,
        canonical_name: "find_bar",
        aliases: &[],
        properties: FIND_BAR_PROPERTIES,
        events: events_of!("find_bar"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `web_find_bar` — the web view's find strip (`COMP-L03`).
pub(crate) fn web_find_bar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::WebFindBar,
        canonical_name: "web_find_bar",
        aliases: &[],
        properties: WEB_FIND_BAR_PROPERTIES,
        events: events_of!("web_find_bar"),
        commands: &["next", "previous", "close"],
    }
}

#[cfg(not(alloc_frugal))]
/// `menu_panel` — a drop-down menu panel a host anchors itself (`COMP-A01`).
pub(crate) fn menu_panel_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::MenuPanel,
        canonical_name: "menu_panel",
        aliases: &[],
        properties: MENU_PANEL_PROPERTIES,
        events: events_of!("menu_panel"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `speed_dial` — a floating trigger that fans out its actions (`COMP-A02`).
pub(crate) fn speed_dial_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::SpeedDial,
        canonical_name: "speed_dial",
        aliases: &[],
        properties: SPEED_DIAL_PROPERTIES,
        events: events_of!("speed_dial"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `swipe_actions` — a multi-action background for a swiped row (`COMP-A03`).
pub(crate) fn swipe_actions_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::SwipeActions,
        canonical_name: "swipe_actions",
        aliases: &[],
        properties: SWIPE_ACTIONS_PROPERTIES,
        events: events_of!("swipe_actions"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `drawer_section_header` — the small heading row that groups a drawer's items (`COMP-A04`).
pub(crate) fn drawer_section_header_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::DrawerSectionHeader,
        canonical_name: "drawer_section_header",
        aliases: &[],
        properties: DRAWER_SECTION_HEADER_PROPERTIES,
        // This heading emits no signals (`drawer_section_header:` in the census), so the list is
        // the empty literal rather than an `events_of!` lookup.
        events: &[],
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `layer_panel` — the companion list for a canvas's layers (`COMP-I01`).
pub(crate) fn layer_panel_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::LayerPanel,
        canonical_name: "layer_panel",
        aliases: &[],
        properties: LAYER_PANEL_PROPERTIES,
        events: events_of!("layer_panel"),
        // `set_entries` names its payload through the `entries` records property (B26.03); the
        // selection / visibility / scroll verbs stay Rust-side because no property carries their
        // layer id.
        commands: &["set_entries"],
    }
}

#[cfg(not(alloc_frugal))]
/// `playlist_panel` — the companion list for a media player's queue (`COMP-I03`).
pub(crate) fn playlist_panel_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::PlaylistPanel,
        canonical_name: "playlist_panel",
        aliases: &[],
        properties: PLAYLIST_PANEL_PROPERTIES,
        events: events_of!("playlist_panel"),
        // `set_tracks` names its payload through the `tracks` records property (B26.03);
        // `track_selected` is an event name rather than a command, and `set_filter` stays Rust-side
        // because the writable `filter` property is its document route.
        commands: &["set_tracks"],
    }
}

#[cfg(not(alloc_frugal))]
/// `shape_canvas` — the free-form shape scene (`COMP-17`).
pub(crate) fn shape_canvas_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ShapeCanvas,
        canonical_name: "shape_canvas",
        aliases: &["shape_board"],
        properties: SHAPE_CANVAS_PROPERTIES,
        events: events_of!("shape_canvas"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `kanban_card_view` — a real card inside a kanban column (`COMP-18`).
pub(crate) fn kanban_card_view_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::KanbanCardView,
        canonical_name: "kanban_card_view",
        aliases: &["kanban_card_control"],
        properties: KANBAN_CARD_VIEW_PROPERTIES,
        events: events_of!("kanban_card_view"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `kanban_column_header` — a real column header for a kanban board (`COMP-18`).
pub(crate) fn kanban_column_header_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::KanbanColumnHeader,
        canonical_name: "kanban_column_header",
        aliases: &["kanban_column_bar"],
        properties: KANBAN_COLUMN_HEADER_PROPERTIES,
        events: events_of!("kanban_column_header"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `range_preset_bar` — quick date-range presets (`COMP-19`).
pub(crate) fn range_preset_bar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::RangePresetBar,
        canonical_name: "range_preset_bar",
        aliases: &["preset_bar"],
        properties: RANGE_PRESET_BAR_PROPERTIES,
        events: events_of!("range_preset_bar"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `grid_view_header` — the shared table header (`COMP-20`).
pub(crate) fn grid_view_header_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::GridViewHeader,
        canonical_name: "grid_view_header",
        aliases: &["table_header_bar"],
        properties: GRID_VIEW_HEADER_PROPERTIES,
        events: events_of!("grid_view_header"),
        commands: &[],
    }
}

// ── Editor/chart/data companion overlays (COMP-21..29) ──
//
// Nine companion controls that report the kind of the control they partner. Each keeps its own
// canonical name, property table and published events; the shared kind is resolved by the
// concrete-type tie-break in `WidgetFactory::widget_matches_capability`.

#[cfg(not(alloc_frugal))]
/// `line_number_gutter` — the editor's line-number rail (`COMP-21`).
pub(crate) fn line_number_gutter_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Panel,
        canonical_name: "line_number_gutter",
        aliases: &[],
        properties: LINE_NUMBER_GUTTER_PROPERTIES,
        events: events_of!("line_number_gutter"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `find_replace_bar` — the editor's find/replace strip (`COMP-22`).
pub(crate) fn find_replace_bar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::SearchBar,
        canonical_name: "find_replace_bar",
        aliases: &[],
        properties: FIND_REPLACE_BAR_PROPERTIES,
        events: events_of!("find_replace_bar"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `list_state_host` — the loading/empty/error surface for a list (`COMP-23`).
pub(crate) fn list_state_host_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::EmptyState,
        canonical_name: "list_state_host",
        aliases: &[],
        properties: LIST_STATE_HOST_PROPERTIES,
        events: events_of!("list_state_host"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `summary_row` — the aggregate row of a table (`COMP-24`).
pub(crate) fn summary_row_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Table,
        canonical_name: "summary_row",
        aliases: &[],
        properties: SUMMARY_ROW_PROPERTIES,
        events: events_of!("summary_row"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `zoom_strip` — the zoom strip beside a chart or a canvas (`COMP-25`).
pub(crate) fn zoom_strip_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Slider,
        canonical_name: "zoom_strip",
        aliases: &[],
        properties: ZOOM_STRIP_PROPERTIES,
        events: events_of!("zoom_strip"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `axis_picker` — per-series axis assignment for a chart (`COMP-26`).
pub(crate) fn axis_picker_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Panel,
        canonical_name: "axis_picker",
        aliases: &[],
        properties: AXIS_PICKER_PROPERTIES,
        events: events_of!("axis_picker"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `chart_data_table` — the tabular view of a chart's series data (`COMP-27`).
///
/// Like every other capability here the table lives in `properties_other.in.rs`; the control
/// itself is built by the `full_widgets` companion module, which is why the constructor and the
/// registration entry sit inside the same gates as the siblings.
pub(crate) fn chart_data_table_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Table,
        canonical_name: "chart_data_table",
        aliases: &[],
        properties: CHART_DATA_TABLE_PROPERTIES,
        events: events_of!("chart_data_table"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `calendar_mark_layer` — per-date marks painted over a calendar (`COMP-28`).
pub(crate) fn calendar_mark_layer_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Panel,
        canonical_name: "calendar_mark_layer",
        aliases: &[],
        properties: CALENDAR_MARK_LAYER_PROPERTIES,
        events: events_of!("calendar_mark_layer"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
/// `drag_ghost_layer` — the translucent proxy card that follows a drag (`COMP-29`).
pub(crate) fn drag_ghost_layer_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Panel,
        canonical_name: "drag_ghost_layer",
        aliases: &[],
        properties: DRAG_GHOST_LAYER_PROPERTIES,
        events: events_of!("drag_ghost_layer"),
        commands: &[],
    }
}
