// Auto-generated from properties.rs — const arrays for widget properties.
// DO NOT EDIT DIRECTLY.

macro_rules! impl_properties_input {
    () => {
        pub(crate) const SLIDER_PROPERTIES: &[PropertySchema] = &[
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
                name: "value",
                value_kind: PropertyValueKind::Int,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "single_step",
                value_kind: PropertyValueKind::Int,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "page_step",
                value_kind: PropertyValueKind::Int,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "orientation",
                value_kind: PropertyValueKind::Enum,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tick_position",
                value_kind: PropertyValueKind::Enum,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tick_interval",
                value_kind: PropertyValueKind::Int,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "tracking",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "slider_position",
                value_kind: PropertyValueKind::Int,
                readable: true,
                writable: true,
            },
        ];

        pub(crate) const PROGRESS_BAR_PROPERTIES: &[PropertySchema] = &[
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
                name: "value",
                value_kind: PropertyValueKind::Int,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "text_visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "orientation",
                value_kind: PropertyValueKind::Enum,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "inverted_appearance",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "progress",
                value_kind: PropertyValueKind::Float,
                readable: true,
                writable: false,
            },
        ];

        pub(crate) const SCROLL_BAR_PROPERTIES: &[PropertySchema] = &[
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
                name: "value",
                value_kind: PropertyValueKind::Int,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "single_step",
                value_kind: PropertyValueKind::Int,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "page_step",
                value_kind: PropertyValueKind::Int,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "orientation",
                value_kind: PropertyValueKind::Enum,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "slider_size",
                value_kind: PropertyValueKind::Float,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "slider_position",
                value_kind: PropertyValueKind::Float,
                readable: true,
                writable: false,
            },
        ];

        pub(crate) const LIST_BOX_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "item_count",
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
            PropertySchema {
                name: "current_row",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "item_height",
                value_kind: PropertyValueKind::Float,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "selected_count",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
        ];

        pub(crate) const SPIN_BOX_PROPERTIES: &[PropertySchema] = &[
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
                name: "value",
                value_kind: PropertyValueKind::Int,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "single_step",
                value_kind: PropertyValueKind::Int,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "prefix",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "suffix",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "special_value_text",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "wrapping",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
        ];

        pub(crate) const COMBO_BOX_PROPERTIES: &[PropertySchema] = &[
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
                name: "current_text",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "editable",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "max_visible_items",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
        ];

        pub(crate) const DIAL_PROPERTIES: &[PropertySchema] = &[
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
                name: "value",
                value_kind: PropertyValueKind::Int,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "single_step",
                value_kind: PropertyValueKind::Int,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "page_step",
                value_kind: PropertyValueKind::Int,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "notches_visible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "notch_target",
                value_kind: PropertyValueKind::Float,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "wrapping",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
        ];

        pub(crate) const COMMAND_LINK_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "text",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "description",
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
        ];

        pub(crate) const FONT_COMBO_BOX_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "current_font_family",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "item_count",
                value_kind: PropertyValueKind::Int,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "current_index",
                value_kind: PropertyValueKind::Int,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "editable",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "max_visible_items",
                value_kind: PropertyValueKind::Int,
                readable: true,
                writable: true,
            },
        ];

        pub(crate) const LINE_EDIT_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "text",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "placeholder_text",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "max_length",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "read_only",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "cursor_position",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
        ];

        pub(crate) const TEXT_EDIT_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "text",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
            PropertySchema {
                name: "placeholder_text",
                value_kind: PropertyValueKind::String,
                readable: false,
                writable: false,
            },
            PropertySchema {
                name: "max_length",
                value_kind: PropertyValueKind::UInt,
                readable: false,
                writable: false,
            },
            PropertySchema {
                name: "read_only",
                value_kind: PropertyValueKind::Bool,
                readable: false,
                writable: false,
            },
            PropertySchema {
                name: "line_wrap",
                value_kind: PropertyValueKind::Bool,
                readable: false,
                writable: false,
            },
        ];

        pub(crate) const SPINNER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "active",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "thickness",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "speed",
                value_kind: PropertyValueKind::Float,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "size_ratio",
                value_kind: PropertyValueKind::Float,
                readable: true,
                writable: true,
            },
        ];

        pub(crate) const ROLLER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "selected_index",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "visible_count",
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

        pub(crate) const DROPDOWN_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "text",
                value_kind: PropertyValueKind::String,
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

        pub(crate) const TEXT_AREA_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "text",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "placeholder",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "read_only",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
        ];

        pub(crate) const KEYBOARD_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "layout",
                value_kind: PropertyValueKind::Enum,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "lowercase",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
        ];

        pub(crate) const SWITCH_PROPERTIES: &[PropertySchema] = &[PropertySchema {
            name: "checked",
            value_kind: PropertyValueKind::Bool,
            readable: true,
            writable: true,
        }];

        #[cfg(not(feature = "mini"))]
        pub(crate) const SEARCH_BOX_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "text",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "placeholder",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
        ];

        #[cfg(not(feature = "mini"))]
        pub(crate) const TAG_INPUT_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "tags",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "placeholder",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
        ];

        #[cfg(not(feature = "mini"))]
        pub(crate) const IME_PREEDIT_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "text",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "cursor_position",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
        ];

        #[cfg(not(feature = "mini"))]
        pub(crate) const INPLACE_EDITOR_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "text",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "editing",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
        ];

        #[cfg(not(feature = "mini"))]
        pub(crate) const CUPERTINO_SLIDER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "value",
                value_kind: PropertyValueKind::Float,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "min",
                value_kind: PropertyValueKind::Float,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "max",
                value_kind: PropertyValueKind::Float,
                readable: true,
                writable: true,
            },
        ];

        #[cfg(not(feature = "mini"))]
        pub(crate) const MASKED_EDIT_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "text",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "mask",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
        ];

        #[cfg(not(feature = "mini"))]
        pub(crate) const AUTO_COMPLETE_EDIT_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "text",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "suggestion_count",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
        ];

        #[cfg(not(feature = "mini"))]
        pub(crate) const MULTI_SELECT_COMBO_BOX_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "selected_count",
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
        pub(crate) const RANGE_SLIDER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "min_value",
                value_kind: PropertyValueKind::Float,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "max_value",
                value_kind: PropertyValueKind::Float,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "lower",
                value_kind: PropertyValueKind::Float,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "upper",
                value_kind: PropertyValueKind::Float,
                readable: true,
                writable: true,
            },
        ];

        #[cfg(not(feature = "mini"))]
        pub(crate) const FLOATING_LABEL_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "text",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "placeholder",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "focused",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
        ];

        #[cfg(not(feature = "mini"))]
        pub(crate) const CUPERTINO_SEGMENTED_CONTROL_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "selected_index",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "segment_count",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
        ];

        #[cfg(not(feature = "mini"))]
        pub(crate) const CUPERTINO_DATE_PICKER_PROPERTIES: &[PropertySchema] = &[PropertySchema {
            name: "selected_date",
            value_kind: PropertyValueKind::String,
            readable: true,
            writable: true,
        }];

        #[cfg(not(feature = "mini"))]
        pub(crate) const EDITABLE_COMBO_BOX_PROPERTIES: &[PropertySchema] = &[
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
        ];

        #[cfg(not(feature = "mini"))]
        pub(crate) const RICH_EDIT_PROPERTIES: &[PropertySchema] = &[PropertySchema {
            name: "text",
            value_kind: PropertyValueKind::String,
            readable: true,
            writable: true,
        }];

        #[cfg(not(feature = "mini"))]
        pub(crate) const SEARCH_BAR_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "text",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "placeholder",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
        ];

        #[cfg(not(feature = "mini"))]
        pub(crate) const SHORTCUT_EDITOR_PROPERTIES: &[PropertySchema] = &[PropertySchema {
            name: "filter_text",
            value_kind: PropertyValueKind::String,
            readable: true,
            writable: true,
        }];
    };
}
