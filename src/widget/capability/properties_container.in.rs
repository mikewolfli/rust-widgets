// Auto-generated from properties.rs — const arrays for widget properties.
// DO NOT EDIT DIRECTLY.

macro_rules! impl_properties_container {
    () => {
        pub(crate) const GROUP_BOX_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "title",
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
        ];

        pub(crate) const SPLITTER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "orientation",
                value_kind: PropertyValueKind::Enum,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "pane_count",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
        ];

        pub(crate) const TOOL_BOX_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "item_count",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "current_index",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "orientation",
                value_kind: PropertyValueKind::Enum,
                readable: true,
                writable: true,
            },
        ];

        pub(crate) const SCROLL_AREA_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "widget_resizable",
                value_kind: PropertyValueKind::Bool,
                readable: false,
                writable: false,
            },
            PropertySchema {
                name: "horizontal_scroll_bar_policy",
                value_kind: PropertyValueKind::Enum,
                readable: false,
                writable: false,
            },
            PropertySchema {
                name: "vertical_scroll_bar_policy",
                value_kind: PropertyValueKind::Enum,
                readable: false,
                writable: false,
            },
            PropertySchema {
                name: "scroll_position_x",
                value_kind: PropertyValueKind::Int,
                readable: false,
                writable: false,
            },
            PropertySchema {
                name: "scroll_position_y",
                value_kind: PropertyValueKind::Int,
                readable: false,
                writable: false,
            },
        ];

        pub(crate) const TAB_WIDGET_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "tab_count",
                value_kind: PropertyValueKind::UInt,
                readable: false,
                writable: false,
            },
            PropertySchema {
                name: "current_index",
                value_kind: PropertyValueKind::UInt,
                readable: false,
                writable: false,
            },
            PropertySchema {
                name: "closable",
                value_kind: PropertyValueKind::Bool,
                readable: false,
                writable: false,
            },
            PropertySchema {
                name: "movable",
                value_kind: PropertyValueKind::Bool,
                readable: false,
                writable: false,
            },
            PropertySchema {
                name: "tab_position",
                value_kind: PropertyValueKind::Enum,
                readable: false,
                writable: false,
            },
        ];

        pub(crate) const STACKED_WIDGET_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "widget_count",
                value_kind: PropertyValueKind::UInt,
                readable: false,
                writable: false,
            },
            PropertySchema {
                name: "current_index",
                value_kind: PropertyValueKind::UInt,
                readable: false,
                writable: false,
            },
        ];

        pub(crate) const COLLAPSIBLE_PANE_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "title",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
            PropertySchema {
                name: "collapsed",
                value_kind: PropertyValueKind::Bool,
                readable: false,
                writable: false,
            },
        ];

        pub(crate) const DOCK_WIDGET_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "title",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
            PropertySchema {
                name: "floating",
                value_kind: PropertyValueKind::Bool,
                readable: false,
                writable: false,
            },
            PropertySchema {
                name: "docked",
                value_kind: PropertyValueKind::Bool,
                readable: false,
                writable: false,
            },
        ];

        pub(crate) const MDI_AREA_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "subwindow_count",
                value_kind: PropertyValueKind::UInt,
                readable: false,
                writable: false,
            },
            PropertySchema {
                name: "active_subwindow",
                value_kind: PropertyValueKind::UInt,
                readable: false,
                writable: false,
            },
            PropertySchema {
                name: "view_mode",
                value_kind: PropertyValueKind::Enum,
                readable: false,
                writable: false,
            },
        ];

        pub(crate) const TILE_VIEW_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "current_page",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "page_count",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
        ];

        #[cfg(not(feature = "mini"))]
        pub(crate) const STEPPER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "value",
                value_kind: PropertyValueKind::Int,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "minimum",
                value_kind: PropertyValueKind::Int,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "maximum",
                value_kind: PropertyValueKind::Int,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "step",
                value_kind: PropertyValueKind::Int,
                readable: true,
                writable: true,
            },
        ];

        #[cfg(not(feature = "mini"))]
        pub(crate) const MASONRY_LAYOUT_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "column_count",
                value_kind: PropertyValueKind::UInt,
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
        pub(crate) const SAFE_AREA_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "top_inset",
                value_kind: PropertyValueKind::Float,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "bottom_inset",
                value_kind: PropertyValueKind::Float,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "left_inset",
                value_kind: PropertyValueKind::Float,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "right_inset",
                value_kind: PropertyValueKind::Float,
                readable: true,
                writable: true,
            },
        ];

        #[cfg(not(feature = "mini"))]
        pub(crate) const CAROUSEL_PROPERTIES: &[PropertySchema] = &[PropertySchema {
            name: "page_count",
            value_kind: PropertyValueKind::UInt,
            readable: true,
            writable: false,
        }];

        #[cfg(not(feature = "mini"))]
        pub(crate) const PAGER_PAGE_VIEW_PROPERTIES: &[PropertySchema] = &[PropertySchema {
            name: "current_page",
            value_kind: PropertyValueKind::UInt,
            readable: true,
            writable: true,
        }];
    };
}
