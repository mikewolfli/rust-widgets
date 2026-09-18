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
            PropertySchema::new("separator", PropertyValueKind::Bool, true, false),
            PropertySchema::new("command_id", PropertyValueKind::String, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const MENU_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("title", PropertyValueKind::String, true, true),
            PropertySchema::new("item_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("hovered_index", PropertyValueKind::UInt, true, false),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const MENU_BAR_PROPERTIES: &[PropertySchema] = &[
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
            PropertySchema::new("item_count", PropertyValueKind::UInt, true, false),
            PropertySchema::new("selected_index", PropertyValueKind::UInt, true, true),
            PropertySchema::new("expanded", PropertyValueKind::Bool, true, true),
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const MENU_BUTTON_PROPERTIES: &[PropertySchema] = &[
            PropertySchema::new("text", PropertyValueKind::String, true, true),
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
            PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
            PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
            PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
            PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const STATUS_BAR_PROPERTIES: &[PropertySchema] = &[PropertySchema::new("message", PropertyValueKind::String, true, true),
        PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
        PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
        PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
        PropertySchema::new("geometry", PropertyValueKind::String, false, false),
        ];
    };
}
