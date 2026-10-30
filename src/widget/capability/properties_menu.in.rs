// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

// Hand-maintained const arrays for widget properties.
//
// This header used to claim the file was auto-generated from `properties.rs` with a
// "DO NOT EDIT DIRECTLY" instruction, but no generator exists in `tools/` or
// `scripts/` that writes `.in.rs` files. The only script that touches them
// (`generate_control_route_matrix.py`) *reads* the `create_widgets_*.in.rs` set to
// emit a report, which is a different family. Edit these directly.

macro_rules! impl_properties_menu {
    () => {
        #[cfg(not(alloc_frugal))]
        pub(crate) const ACTION_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("text", PropertyValueKind::String, true, true),
            PropertySchema::new("icon_text", PropertyValueKind::String, true, true),
            PropertySchema::new("shortcut", PropertyValueKind::String, true, true),
            PropertySchema::new("checkable", PropertyValueKind::Bool, true, true),
            PropertySchema::new("checked", PropertyValueKind::Bool, true, true),
            PropertySchema::new("icon_source", PropertyValueKind::IconSpec, true, true),
            PropertySchema::new("separator", PropertyValueKind::Bool, true, false),
            PropertySchema::new("command_id", PropertyValueKind::String, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const MENU_PROPERTIES: &[PropertySchema] = &[
            // The entry list as records (BLUE26 B26.09): `add_action`/`add_separator` take
            // arguments the property API cannot carry, so the list itself is the property.
            PropertySchema::records(
                "items",
                true,
                true,
                &crate::widget::menu_toolbar::menu::MENU_ITEM_SCHEMA,
            ),
            PropertySchema::new("title", PropertyValueKind::String, true, true),
            PropertySchema::new("item_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("hovered_index", PropertyValueKind::UInt, true, false),
            // Governs the Left/Right arrow keys only: a list of entries runs downward in every
            // locale, but the horizontal pair steps along a line of reading order.
            PropertySchema::enumerated("direction", true, true, &["ltr", "rtl"]),
            // Whether the popup paints its title heading. A menu owned by a menu bar answers
            // `false` (the bar already shows the title); a context menu keeps it.
            PropertySchema::new("show_heading", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
            PropertySchema::new("column_count", PropertyValueKind::UInt, true, true),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const MENU_BAR_PROPERTIES: &[PropertySchema] = &[
            // The entries as records (BLUE26 B26.09): `add_menu`/`remove_menu` take arguments the
            // property API cannot carry, so the list itself is the property.
            PropertySchema::records(
                "entries",
                true,
                true,
                &crate::widget::menu_toolbar::menu_bar::MENU_BAR_ENTRY_SCHEMA,
            ),
            PropertySchema::new("entry_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("active_index", PropertyValueKind::UInt, true, false),
            PropertySchema::new("hovered_index", PropertyValueKind::UInt, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const TOOL_BAR_PROPERTIES: &[PropertySchema] = &[
            // The items as records (BLUE26 B26.09): `add_action`/`add_separator` take arguments
            // the property API cannot carry, so the list itself is the property.
            PropertySchema::records(
                "items",
                true,
                true,
                &crate::widget::menu_toolbar::tool_bar::TOOL_BAR_ITEM_SCHEMA,
            ),
            PropertySchema::enumerated("orientation", true, true, &["horizontal", "vertical"]),
            PropertySchema::new("icon_size", PropertyValueKind::Float, true, true),
            PropertySchema::new("movable", PropertyValueKind::Bool, true, true),
            PropertySchema::new("floatable", PropertyValueKind::Bool, true, true),
            PropertySchema::new("item_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const DROPDOWN_MENU_PROPERTIES: &[PropertySchema] = &[
            // The rows are published so a document can read each item's typed icon; the write side is
            // refused because a row cannot express `DropdownItem::children` (see
            // `DROPDOWN_ITEM_SCHEMA`).
            PropertySchema::records(
                "items",
                true,
                false,
                &crate::widget::menu_toolbar::dropdown_menu::DROPDOWN_ITEM_SCHEMA,
            ),
            PropertySchema::new("item_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("selected_index", PropertyValueKind::UInt, true, true),
            PropertySchema::new("expanded", PropertyValueKind::Bool, true, true),
            PropertySchema::enumerated("alignment", true, true, &["left", "centre", "right"]),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const MENU_BUTTON_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("text", PropertyValueKind::String, true, true),
            PropertySchema::new("icon_source", PropertyValueKind::IconSpec, true, true),
            // The rows are a **view** of the menu items, keyed by `MenuItem::id`; the write side is
            // refused because a row cannot express `MenuItem::submenu` (see `MENU_BUTTON_ITEM_SCHEMA`).
            PropertySchema::records(
                "items",
                true,
                false,
                &crate::widget::menu_toolbar::menu_button::MENU_BUTTON_ITEM_SCHEMA,
            ),
            PropertySchema::new("item_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("expanded", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const TOOL_BUTTON_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("text", PropertyValueKind::String, true, true),
            PropertySchema::new("checked", PropertyValueKind::Bool, true, true),
            PropertySchema::new("icon_source", PropertyValueKind::IconSpec, true, true),
            // The icon's own colour channel. `null` means "follow the label ink", which is what
            // every control did before this property existed.
            PropertySchema::new("icon_tint", PropertyValueKind::Color, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const STATUS_BAR_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("message", PropertyValueKind::String, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];
    };
}
