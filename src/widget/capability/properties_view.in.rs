// Auto-generated from properties.rs — const arrays for widget properties.
// DO NOT EDIT DIRECTLY.

macro_rules! impl_properties_view {
    () => {
        pub(crate) const LIST_VIEW_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "has_model",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "row_count",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "focused_row",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "selection_mode",
                value_kind: PropertyValueKind::Enum,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "view_mode",
                value_kind: PropertyValueKind::Enum,
                readable: true,
                writable: true,
            },
        ];

        pub(crate) const TREE_VIEW_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "has_model",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "node_count",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "focused_node",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "selected_node",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
        ];

        pub(crate) const TABLE_WIDGET_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "has_model",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "has_delegate",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "row_count",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "column_count",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "selection_mode",
                value_kind: PropertyValueKind::Enum,
                readable: true,
                writable: true,
            },
        ];

        pub(crate) const DATA_GRID_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "has_data_source",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "row_count",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "column_count",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "scroll_row",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "scroll_column",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "row_height",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "column_width",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "frozen_columns",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "sort_spec_count",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "filter_count",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "sort_specs",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "filters",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
        ];

        pub(crate) const TREE_TABLE_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "has_model",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "row_count",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "column_count",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "selected_row",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "row_height",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "column_width",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
        ];

        pub(crate) const VIRTUAL_TABLE_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "has_data_source",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "row_count",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "column_count",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "scroll_row",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "scroll_column",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "row_height",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "column_width",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "overscan_rows",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "overscan_columns",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible_window",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: false,
            },
        ];

        pub(crate) const VIRTUAL_LIST_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "has_data_source",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "row_count",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "scroll_row",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "row_height",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "overscan",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "selected_row",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
        ];

        pub(crate) const IMAGE_VIEW_PROPERTIES: &[PropertySchema] = &[PropertySchema {
            name: "scaled",
            value_kind: PropertyValueKind::Bool,
            readable: true,
            writable: true,
        }];

        #[cfg(not(feature = "mini"))]
        pub(crate) const PROPERTIES_PANEL_PROPERTIES: &[PropertySchema] = &[PropertySchema {
            name: "property_count",
            value_kind: PropertyValueKind::UInt,
            readable: true,
            writable: false,
        }];

        #[cfg(not(feature = "mini"))]
        pub(crate) const IMAGE_GALLERY_PROPERTIES: &[PropertySchema] = &[PropertySchema {
            name: "current_index",
            value_kind: PropertyValueKind::UInt,
            readable: true,
            writable: true,
        }];

        pub(crate) const PROPERTY_GRID_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "property_count",
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
        ];
    };
}
