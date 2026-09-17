// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

// Auto-generated from properties.rs — const arrays for widget properties.
// DO NOT EDIT DIRECTLY.

macro_rules! impl_properties_view {
    () => {
        #[cfg(not(alloc_frugal))]
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

        // `GridTableWidget` is the feature-rich virtualised table (grid lines,
        // headers, per-column widths, sorting). It was implemented and exported but
        // had no capability at all, so it could not be created by name. It shares
        // `WidgetKind::GridTable` with nothing else, hence its own row in the
        // type-based tie-break table is only needed once the kind has a second entry.
        // `NumberPicker` is a scrolling digit wheel: `value` is snapped to a grid
        // anchored at `minimum`, so both bounds stay selectable.
        // `OtpInput` is a fixed-length code entry: `length` and `masked` describe
        // the display, `value` is the real code, and the two derived names let a
        // caller drive a "verify" button without re-deriving them.
        #[cfg(not(alloc_frugal))]
        pub(crate) const OTP_INPUT_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "value",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "length",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "masked",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "separator",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "focused_index",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "is_complete",
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

        // `Banner` is a persistent notice: unlike a snackbar it has no timer, so
        // `dismissed` is read-only state the user drives by clicking.
        #[cfg(not(alloc_frugal))]
        pub(crate) const BANNER_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "text",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "severity",
                value_kind: PropertyValueKind::Enum,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "dismissible",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "dismissed",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "action_count",
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

        // `Pagination` holds no content, only the index into somebody else's data:
        // every number that defines that index is writable, and the two derived from
        // it are read-only.
        #[cfg(not(alloc_frugal))]
        pub(crate) const PAGINATION_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "total",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "page_size",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "page",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "page_count",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "last_page",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "sibling_count",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "show_nav_buttons",
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

        #[cfg(not(alloc_frugal))]
        pub(crate) const NUMBER_PICKER_PROPERTIES: &[PropertySchema] = &[
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
            PropertySchema {
                name: "wrap",
                value_kind: PropertyValueKind::Bool,
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
                name: "row_count",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "selected_row",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "row_height",
                value_kind: PropertyValueKind::UInt,
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
        pub(crate) const GRID_TABLE_WIDGET_PROPERTIES: &[PropertySchema] = &[
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
                name: "selection_mode",
                value_kind: PropertyValueKind::Enum,
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
                name: "selected_cell",
                value_kind: PropertyValueKind::String,
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

        pub(crate) const IMAGE_VIEW_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "scaled",
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

        #[cfg(not(alloc_frugal))]
        pub(crate) const PROPERTIES_PANEL_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "property_count",
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
        pub(crate) const IMAGE_GALLERY_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "current_index",
                value_kind: PropertyValueKind::UInt,
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

        // `Toast` is one transient message. `ToastStack` is the container that
        // queues them, so the properties here describe a single notification rather
        // than a collection: there is no count and no selection.
        #[cfg(not(alloc_frugal))]
        pub(crate) const TOAST_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "message",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "level",
                value_kind: PropertyValueKind::Enum,
                readable: true,
                writable: true,
            },
            // A ttl here is a *hint for the host*: this control does not schedule
            // its own expiry, so the value is data the host reads, not behaviour it
            // triggers. That is why it is writable but has no `expired` sibling.
            PropertySchema {
                name: "ttl_ms",
                value_kind: PropertyValueKind::UInt,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "dismissible",
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

        // `SplashScreen` is dismissed by the program, not the user, so it publishes
        // no `dismissed` state: `progress` is the only live value, and `Null` is the
        // documented way to return it to indeterminate.
        #[cfg(not(alloc_frugal))]
        pub(crate) const SPLASH_SCREEN_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "title",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "subtitle",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "progress",
                value_kind: PropertyValueKind::Float,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "skippable",
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

        // `ColorPicker` split out of `ColorDialog` in BLUE16 phase E-6. The property
        // set is identical to the dialog's because it describes the same picker; what
        // differs is the host, so the schemas stay in step deliberately.
        #[cfg(not(alloc_frugal))]
        pub(crate) const COLOR_DIALOG_PROPERTIES: &[PropertySchema] = &[
            PropertySchema {
                name: "current_color",
                value_kind: PropertyValueKind::String,
                readable: true,
                writable: false,
            },
            PropertySchema {
                name: "modal",
                value_kind: PropertyValueKind::Bool,
                readable: true,
                writable: true,
            },
            PropertySchema {
                name: "options_alpha",
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
