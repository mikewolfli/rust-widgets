// Auto-generated from properties.rs — const arrays for widget properties.
// DO NOT EDIT DIRECTLY.

macro_rules! impl_properties_advanced {
    () => {
        #[cfg(not(feature = "mini"))]
        pub(crate) const TAB_BAR_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "tab_count",
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
                name: "closable",
                value_kind: PropertyValueKind::Bool,
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
                name: "tab_min_width",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tab_max_width",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
        ];

        #[cfg(not(feature = "mini"))]
        pub(crate) const CALENDAR_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "selected_date",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "minimum_date",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "maximum_date",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "first_day_of_week",
                value_kind: PropertyValueKind::Enum,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "grid_visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "navigation_bar_visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "horizontal_header_visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "vertical_header_visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "date_format",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
        ];

        #[cfg(not(feature = "mini"))]
        pub(crate) const DATE_EDIT_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "date",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "minimum_date",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "maximum_date",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "display_format",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "calendar_popup",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
        ];

        #[cfg(not(feature = "mini"))]
        pub(crate) const TIME_EDIT_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "time",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "minimum_time",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "maximum_time",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "display_format",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
        ];

        #[cfg(not(feature = "mini"))]
        pub(crate) const RIBBON_BAR_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "tab_count",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "current_tab",
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
            PropertySchema {
                name: "minimized",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
        ];

        #[cfg(not(feature = "mini"))]
        pub(crate) const PIE_MENU_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "item_count",
                value_kind: PropertyValueKind::UInt,
                readable: false,
                writable: false,
            },
            PropertySchema {
                name: "radius",
                value_kind: PropertyValueKind::Float,
                readable: false,
                writable: false,
            },
            PropertySchema {
                name: "inner_radius",
                value_kind: PropertyValueKind::Float,
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

        #[cfg(not(feature = "mini"))]
        pub(crate) const DATE_TIME_EDIT_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "datetime",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
            PropertySchema {
                name: "display_format",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
            PropertySchema {
                name: "calendar_popup",
                value_kind: PropertyValueKind::Bool,
                readable: false,
                writable: false,
            },
            PropertySchema {
                name: "minimum",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
            PropertySchema {
                name: "maximum",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
        ];
    };
}
