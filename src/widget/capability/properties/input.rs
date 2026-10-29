// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

// Capability constructors for the `input` widgets.
//
// The property arrays these constructors reference are declared in
// `properties_input.in.rs` and expanded by the parent module. `use super::*` brings
// those arrays and the shared imports into scope; `events_of!` is visible through the
// parent module's textual macro scope.

use super::*;

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
