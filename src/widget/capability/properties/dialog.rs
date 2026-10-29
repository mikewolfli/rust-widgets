// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

// Capability constructors for the `dialog` widgets.
//
// The property arrays these constructors reference are declared in
// `properties_dialog.in.rs` and expanded by the parent module. `use super::*` brings
// those arrays and the shared imports into scope; `events_of!` is visible through the
// parent module's textual macro scope.

use super::*;

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
