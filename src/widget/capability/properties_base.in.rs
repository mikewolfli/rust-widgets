// Auto-generated from properties.rs — const arrays for widget properties.
// DO NOT EDIT DIRECTLY.

macro_rules! impl_properties_base {
    () => {
        pub(crate) const BUTTON_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "text",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "pressed",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "default",
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
        ];

        pub(crate) const LABEL_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "text",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "alignment",
                value_kind: PropertyValueKind::Enum,
                readable: true,
                writable: true,
            },
        ];

        pub(crate) const CHECK_BOX_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "text",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "state",
                value_kind: PropertyValueKind::Enum,
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
                name: "tristate_enabled",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
        ];

        pub(crate) const RADIO_BUTTON_PROPERTIES: &[PropertySchema] = &[
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
            PropertySchema {
                name: "group_id",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
        ];

        #[cfg(not(feature = "mini"))]
        pub(crate) const TOGGLE_BUTTON_PROPERTIES: &[PropertySchema] = &[
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
            PropertySchema {
                name: "state",
                value_kind: PropertyValueKind::Enum,
                readable: true,
                writable: false,
            },
        ];

        pub(crate) const LINE_PROPERTIES: &[PropertySchema] = &[PropertySchema {
            name: "orientation",
            value_kind: PropertyValueKind::Enum,
            readable: true,
            writable: true,
        }];
    };
}
