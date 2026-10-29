// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

// Capability constructors for the `menu` widgets.
//
// The property arrays these constructors reference are declared in
// `properties_menu.in.rs` and expanded by the parent module. `use super::*` brings
// those arrays and the shared imports into scope; `events_of!` is visible through the
// parent module's textual macro scope.

use super::*;

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
