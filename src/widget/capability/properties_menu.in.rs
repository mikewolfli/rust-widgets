// Auto-generated from properties.rs — const arrays for widget properties.
// DO NOT EDIT DIRECTLY.

macro_rules! impl_properties_menu {
    () => {
        #[cfg(not(feature = "mini"))]
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

        #[cfg(not(feature = "mini"))]
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

        #[cfg(not(feature = "mini"))]
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

        #[cfg(not(feature = "mini"))]
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

        #[cfg(not(feature = "mini"))]
        pub(crate) const DROPDOWN_MENU_PROPERTIES: &[PropertySchema] = &[
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
                writable: true,
            },
            PropertySchema {
                name: "expanded",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
        ];

        #[cfg(not(feature = "mini"))]
        pub(crate) const MENU_BUTTON_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "text",
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
                name: "expanded",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
        ];

        #[cfg(not(feature = "mini"))]
        pub(crate) const TOOL_BUTTON_PROPERTIES: &[PropertySchema] = &[
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
        ];

        #[cfg(not(feature = "mini"))]
        pub(crate) const STATUS_BAR_PROPERTIES: &[PropertySchema] = &[PropertySchema {
            name: "message",
            value_kind: PropertyValueKind::String,
            readable: true,
            writable: true,
        }];
    };
}
