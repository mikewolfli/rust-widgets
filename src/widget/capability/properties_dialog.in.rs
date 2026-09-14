// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

// Auto-generated from properties.rs — const arrays for widget properties.
// DO NOT EDIT DIRECTLY.

macro_rules! impl_properties_dialog {
    () => {
        #[cfg(not(alloc_frugal))]
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
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
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
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
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
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
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
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
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
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
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
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const FONT_DIALOG_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "modal",
                value_kind: PropertyValueKind::Bool,
                readable: false,
                writable: false,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
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
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
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
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
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
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
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
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
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
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const POPUP_WINDOW_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "title",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "has_content",
                value_kind: PropertyValueKind::Bool,
                readable: false,
                writable: false,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
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
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const BOTTOM_SHEET_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "expanded",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "peek_height",
                value_kind: PropertyValueKind::Float,
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
                name: "visible",
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
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const WIZARD_DIALOG_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "title",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "current_step",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "step_count",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "can_go_back",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "can_go_forward",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "enabled",
                value_kind: PropertyValueKind::Bool,
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
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const CUPERTINO_ALERT_DIALOG_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "title",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "message",
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
            PropertySchema {
                name: "visible",
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
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const TOOLTIP_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "text",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            // The tooltip's own shown-state. It is published as `shown`, not
            // `visible`, because `visible` is the base widget's visibility — the
            // tooltip's visibility is what `shown` drives, and collapsing the two
            // would leave one of them unreachable.
            PropertySchema {
                name: "shown",
                value_kind: PropertyValueKind::Bool,
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
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const POPOVER_PROPERTIES: &[PropertySchema] = &[
            // See `TOOLTIP_PROPERTIES`: the popup's own shown-state is `shown`,
            // leaving `visible` to mean the base widget's visibility.
            PropertySchema {
                name: "shown",
                value_kind: PropertyValueKind::Bool,
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
                name: "text",
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
            PropertySchema {
                name: "tooltip",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const MODAL_BOTTOM_SHEET_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "visible",
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
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];

        #[cfg(not(alloc_frugal))]
        pub(crate) const FIND_REPLACE_DIALOG_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "find_text",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "replace_text",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "match_case",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "wrap_around",
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
                name: "visible",
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
            PropertySchema {
                name: "geometry",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];
    };
}
